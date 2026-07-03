use crate::{
    messages::JobId,
    ws_connection::{SetRunner, WsConnection},
};
use actix::prelude::*;
// use futures_util::FutureExt;
use job_handle::JobHandleConfiguration;
use job_runner::JobRunner;
use job_type::{Job, JobSpawnError};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;
pub mod docker;
pub mod job_handle;
pub mod job_runner;
pub mod job_type;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
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

pub struct NewJob(pub Arc<dyn Job>);
unsafe impl Send for NewJob {}

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
    pub job_object: Arc<dyn Job>,
    pub monitors: Vec<Addr<WsConnection>>,
}

impl QueuedJob {
    pub fn new(id: JobId, job_object: Arc<dyn Job>) -> Self {
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
    job_handle_configuration: JobHandleConfiguration,
    usage_stats_db: Option<DatabaseConnection>,
}

impl JobManager {
    fn handle_spawn_job(
        &self,
        job_object: Arc<dyn Job>,
        id: JobId,
        perm: Option<OwnedSemaphorePermit>,
    ) -> ResponseActFuture<Self, <NewJob as actix::Message>::Result> {
        let tm = job_object.timeout_value();
        let jh_config = self.job_handle_configuration.clone();
        let usdb = self.usage_stats_db.clone();
        Box::pin(
            async move {
                JobRunner::try_create_job(id.clone(), job_object, perm, jh_config, usdb)
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
    fn handle_job_from_queue(
        &mut self,
        ctx: &mut <Self as actix::Actor>::Context,
        job_object: Arc<dyn Job>,
        id: JobId,
        perm: Option<OwnedSemaphorePermit>,
    ) -> Addr<JobRunner> {
        let tm = job_object.timeout_value();
        let jh_config = self.job_handle_configuration.clone();
        let usdb = self.usage_stats_db.clone();
        let runner = JobRunner::create_queued_job(id.clone(), job_object, perm, jh_config, usdb);
        self.jobs.insert(id.clone(), runner.clone());

        // Cleanup task
        // Make sure to keep this longer than the job timeout
        // We don't have to care if the job is still running or not.
        // In the worst-case scenario, it should have timed-out a long time ago.
        ctx.notify_later(RemoveJob(id.clone()), tm * 2);

        log::debug!("Job with ID={} moved from queue", id);
        runner
    }
    fn enqueue_job(
        &mut self,
        ctx: &mut <Self as actix::Actor>::Context,
        job_object: Arc<dyn Job>,
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
        .map(move |perm, actor, ctx| {
            log::debug!("Semaphore permit obtained for queued job. Unqueueing a job...");
            let queued_job = actor.job_queue.as_mut().unwrap().data.pop_front().unwrap();
            log::info!(
                "Processing next job from the queue (ID={}). Jobs remaining in queue: {}",
                &queued_job.id,
                actor.job_queue.as_ref().map(|x| x.data.len()).unwrap_or(0)
            );

            let job = actor.handle_job_from_queue(
                ctx,
                queued_job.job_object,
                queued_job.id.clone(),
                Some(perm),
            );

            for i in queued_job.monitors {
                let m_job = job.clone();
                let id = queued_job.id.clone();
                actix_rt::spawn(
                    async move {
                        log::debug!(
                            "Notifying WsConnection about job being unqueued (ID={}).",
                            &id
                        );
                        if let Err(e) = i.send(SetRunner(m_job)).await {
                            log::warn!("WsConnection could not be notified about job being unqueued (ID={}): {}.",
                            &id, e);
                        }
                    }
                );
            }
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

    fn handle(&mut self, msg: MonitorQueuedJob, _ctx: &mut Self::Context) -> Self::Result {
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
                actix_rt::spawn(async move {
                    log::debug!(
                        "Immediately notifying WsConnection about job being unqueued (ID={}).",
                        id
                    );
                    if let Err(e) = msg.1.send(SetRunner(job)).await {
                        log::warn!(
                            "WsConnection could not be notified about job being unqueued (ID={}): {}.",
                            &id,
                            e
                        );
                    }
                });
            } else {
                // This should never happen
                log::error!(
                    "Monitoring of queued job requested but the job is neither in queue nor in the jobs map (ID={})",
                    msg.0
                );
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

/// [JobManager]'s part of the VibeCheckResponse.
#[derive(MessageResponse)]
pub struct JobManagerVibeCheckReply {
    pub queue_length: Option<usize>,
    pub max_queue_length: Option<usize>,
    pub active_jobs: usize,
}

pub struct JobManagerVibeCheck;
impl Message for JobManagerVibeCheck {
    type Result = JobManagerVibeCheckReply;
}

impl Handler<JobManagerVibeCheck> for JobManager {
    type Result = <JobManagerVibeCheck as actix::Message>::Result;

    fn handle(&mut self, _msg: JobManagerVibeCheck, _ctx: &mut Self::Context) -> Self::Result {
        JobManagerVibeCheckReply {
            queue_length: self.job_queue.as_ref().map(|q| q.data.len()),
            max_queue_length: self.job_queue.as_ref().map(|q| q.max_len),
            active_jobs: self.jobs.len(),
        }
    }
}

impl JobManager {
    pub fn new(
        max_jobs: Option<usize>,
        max_queue_length: Option<usize>,
        jh_config: JobHandleConfiguration,
        usage_stats_db: Option<DatabaseConnection>,
    ) -> Self {
        log::info!("Initializing JobManager.");
        Self {
            jobs: BTreeMap::new(),
            concurrent_jobs_semaphore: max_jobs.map(|x| Arc::from(Semaphore::new(x))),
            job_queue: max_queue_length.map(|x| JobQueue {
                max_len: x,
                data: VecDeque::new(),
            }),
            job_handle_configuration: jh_config,
            usage_stats_db,
        }
    }
}
