use super::messages::JobId;
use crate::ws_connection::{SetRunner, WsConnection};
use actix::prelude::*;
// use futures_util::FutureExt;
use job_runner::JobRunner;
use job_type::{Job, JobSpawnError};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
pub mod docker;
pub mod job_handle;
pub mod job_runner;
pub mod job_type;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobOutput {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Finished,
    Failed(JobFailureReason),
    Queued,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobFailureReason {
    TimedOut,
    SetupError(String),
    JobProcessError,
}

#[derive(Debug, Clone, MessageResponse)]
pub struct JobData {
    pub status: JobStatus,
    /// Gets filled when the job completes.
    /// If the job fails, it will only be filled
    /// if the error came from the job executable itself
    pub job_output: Option<JobOutput>,
}

impl Message for JobData {
    type Result = ();
}

impl Actor for JobManager {
    type Context = Context<Self>;
}

pub enum JobEntry {
    Spawned(Addr<JobRunner>),
    Queued(usize),
}

pub struct NewJobResponse {
    pub id: JobId,
    pub entry: JobEntry,
}

pub struct NewJob(pub Box<dyn Job>);

impl Message for NewJob {
    type Result = Result<NewJobResponse, JobSpawnError>;
}

pub struct LookupJob(pub JobId);
impl Message for LookupJob {
    type Result = Option<JobEntry>;
}

pub struct MonitorQueuedJob(pub JobId, pub Addr<WsConnection>);
impl Message for MonitorQueuedJob {
    type Result = ();
}

struct QueuedJob {
    pub id: JobId,
    pub job_object: Box<dyn Job>,
    pub monitors: Vec<Addr<WsConnection>>,
}

impl QueuedJob {
    pub fn new(id: JobId, job_object: Box<dyn Job>) -> Self {
        Self {
            id,
            job_object,
            monitors: Vec::new(),
        }
    }
}

struct JobQueue {
    pub max_len: usize,
    pub data: VecDeque<QueuedJob>,
}

pub struct JobManager {
    jobs: BTreeMap<JobId, Addr<JobRunner>>,
    concurrent_jobs_semaphore: Option<Arc<Semaphore>>,
    job_queue: Option<JobQueue>,
}

impl JobManager {
    fn handle_spawn_job(
        &self,
        job_object: Box<dyn Job>,
        id: JobId,
        perm: Option<OwnedSemaphorePermit>,
    ) -> ResponseActFuture<Self, <NewJob as actix::Message>::Result> {
        let tm = job_object.timeout_value();
        Box::pin(
            async move {
                JobRunner::create_job(id.clone(), job_object, perm)
                    .await
                    .map(|addr| (id, addr))
            }
            .into_actor(self)
            .map(move |job_res, actor, ctx| {
                job_res.map(|(jid, job)| {
                    actor.jobs.insert(jid.clone(), job.clone());

                    // Cleanup task
                    // Make sure to keep this longer than the job timeout
                    // We don't have to care if the job is still running or not.
                    // In the worst-case scenario, it should have timed-out a long time ago.
                    ctx.notify_later(RemoveJob(jid.clone()), tm * 2);

                    log::info!("Added job with ID={}", &jid);
                    NewJobResponse {
                        id: jid,
                        entry: JobEntry::Spawned(job),
                    }
                })
            }),
        )
    }
    fn enqueue_job(
        &mut self,
        ctx: &mut <Self as actix::Actor>::Context,
        job_object: Box<dyn Job>,
        id: JobId,
    ) -> ResponseActFuture<Self, <NewJob as actix::Message>::Result> {
        log::info!("Enqueuing job with ID={}", &id);
        let queue_mut = self.job_queue.as_mut().unwrap();
        queue_mut
            .data
            .push_back(QueuedJob::new(id.clone(), job_object));
        let queue_pos = queue_mut.data.len();

        let semaphore = self.concurrent_jobs_semaphore.clone().unwrap();
        let fut = actix::fut::wrap_future::<_, Self>(async move {
            let perm = semaphore.acquire_owned().await.unwrap();
            perm
        })
        .then(move |perm, actor, _ctx| {
            log::debug!("Semaphore permit obtained for queued job. Unqueueing a job...");
            let queued_job = actor.job_queue.as_mut().unwrap().data.pop_front().unwrap();
            log::info!(
                "Processing next job from the queue (ID={}). Jobs remaining in queue: {}",
                &queued_job.id,
                actor.job_queue.as_ref().map(|x| x.data.len()).unwrap_or(0)
            );
            actor
                .handle_spawn_job(queued_job.job_object, queued_job.id.clone(), Some(perm))
                .map(move |new_job_result, a, ctx| match new_job_result {
                    Ok(nj) => {
                        let JobEntry::Spawned(job) = nj.entry else {
                            panic!("JobEntry::Spawned was expected after job has been spawned and unqueued.");
                        };
                        log::debug!("Successfully spawned unqueued job with ID={}", &nj.id);
                        for i in queued_job.monitors {
                            let m_job = job.clone();
                            let id = nj.id.clone();
                            ctx.spawn(async move {
                                log::debug!("Notifying WsConnection about job being unqueued (ID={}).", id);
                                let _ = i.send(SetRunner(m_job)).await;
                            }.into_actor(a));
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to spawn queued job with ID={}, error: {:#}",
                            queued_job.id,
                            &e
                        );
                        log::error!("TODO: Handle failed queued job (the user needs to have a way of knowing): {:#}", e);
                    }
                })
        });
        ctx.spawn(fut);
        return Box::pin(
            async move {
                Ok(NewJobResponse {
                    id,
                    entry: JobEntry::Queued(queue_pos),
                })
            }
            .into_actor(self),
        );
    }
}

impl Handler<LookupJob> for JobManager {
    type Result = <LookupJob as actix::Message>::Result;

    fn handle(&mut self, msg: LookupJob, _ctx: &mut Self::Context) -> Self::Result {
        match self.jobs.get(&msg.0) {
            Some(j) => Some(JobEntry::Spawned(j.clone())),
            None => self
                .job_queue
                .as_ref()
                .map(|q| {
                    q.data
                        .iter()
                        .enumerate()
                        .find(|(_ord_num, qj)| *qj.id == msg.0)
                })
                .flatten()
                .map(|(ord_num, _rest)| JobEntry::Queued(ord_num + 1)),
        }
    }
}

impl Handler<MonitorQueuedJob> for JobManager {
    type Result = <MonitorQueuedJob as actix::Message>::Result;

    fn handle(&mut self, msg: MonitorQueuedJob, ctx: &mut Self::Context) -> Self::Result {
        if let Some(q_job) = self
            .job_queue
            .as_mut()
            .map(|q| q.data.iter_mut().find(|qj| qj.id == msg.0))
            .flatten()
        {
            q_job.monitors.push(msg.1);
        } else {
            if let Some(job) = self.jobs.get(&msg.0).cloned() {
                let id = msg.0;
                ctx.spawn(
                    async move {
                        log::debug!(
                            "Immediately notifying WsConnection about job being unqueued (ID={}).",
                            id
                        );
                        let _ = msg.1.send(SetRunner(job)).await;
                    }
                    .into_actor(self),
                );
            } else {
                log::error!("Monitoring of queued job requested but the job is neither in queue nor in the jobs map (ID={})", msg.0);
            }
        }
    }
}

struct RemoveJob(pub JobId);
impl Message for RemoveJob {
    type Result = ();
}

impl Handler<RemoveJob> for JobManager {
    type Result = <RemoveJob as actix::Message>::Result;

    fn handle(&mut self, msg: RemoveJob, _ctx: &mut Self::Context) -> Self::Result {
        self.jobs.remove(&msg.0);
        log::info!("Removed job with ID={}", msg.0);
    }
}

impl Handler<NewJob> for JobManager {
    type Result = ResponseActFuture<Self, <NewJob as actix::Message>::Result>;

    fn handle(&mut self, msg: NewJob, ctx: &mut Self::Context) -> Self::Result {
        let mk_id = || loop {
            let new_id = uuid::Uuid::new_v4();
            let id = new_id.to_string();
            if !self.jobs.contains_key(&id)
                && self
                    .job_queue
                    .as_ref()
                    .map(|q| q.data.iter().all(|qj| qj.id != id))
                    .unwrap_or(true)
            {
                break id;
            }
        };
        let Ok(perm) = self
            .concurrent_jobs_semaphore
            .clone()
            .map(|sem| sem.try_acquire_owned())
            .transpose()
        else {
            // Too many concurrent jobs.
            // We have to either queue the job or drop it.
            if let Some(queue) = self.job_queue.as_ref() {
                if queue.data.len() < queue.max_len {
                    let id = mk_id();
                    //drop(queue);
                    return self.enqueue_job(ctx, msg.0, id);
                }
                log::info!("Dropping new job: Queue is full");
            } else {
                log::info!("Dropping new job: Too many jobs, queue is disabled");
            }
            return Box::pin(async move { Err(JobSpawnError::TooManyJobs) }.into_actor(self));
        };
        let id = mk_id();
        self.handle_spawn_job(msg.0, id, perm)
    }
}

impl JobManager {
    pub fn new(max_jobs: Option<usize>, max_queue_length: Option<usize>) -> Self {
        log::info!("Initializing JobManager.");
        Self {
            jobs: BTreeMap::new(),
            concurrent_jobs_semaphore: max_jobs.map(|x| Arc::from(Semaphore::new(x))),
            job_queue: max_queue_length.map(|x| JobQueue {
                max_len: x,
                data: VecDeque::new(),
            }),
        }
    }
}
