use super::messages::JobId;
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

pub enum NewJobInfo {
    Spawned(Addr<JobRunner>),
    Queued(usize),
}

pub struct NewJobResponse {
    pub id: JobId,
    pub info: NewJobInfo,
}

pub struct NewJob(pub Box<dyn Job>);

impl Message for NewJob {
    type Result = Result<NewJobResponse, JobSpawnError>;
}

pub struct LookupJob(pub JobId);
impl Message for LookupJob {
    type Result = Option<Addr<JobRunner>>;
}

struct JobQueue {
    max_len: usize,
    data: VecDeque<(JobId, Box<dyn Job>)>,
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
                        info: NewJobInfo::Spawned(job),
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
        queue_mut.data.push_back((id.clone(), job_object));
        let queue_pos = queue_mut.data.len();

        let semaphore = self.concurrent_jobs_semaphore.clone().unwrap();
        let fut = actix::fut::wrap_future::<_, Self>(async move {
            let perm = semaphore.acquire_owned().await.unwrap();
            perm
        })
        .then(move |perm, actor, _ctx| {
            log::debug!("Semaphore permit obtained for queued job. Unqueueing a job...");
            let (id, jo) = actor.job_queue.as_mut().unwrap().data.pop_front().unwrap();
            log::info!(
                "Processing next job from the queue (ID={}). Jobs remaining in queue: {}",
                &id,
                actor.job_queue.as_ref().map(|x| x.data.len()).unwrap_or(0)
            );
            actor
                .handle_spawn_job(jo, id, Some(perm))
                .map(move |new_job_result, _actor, _ctx| match new_job_result {
                    Ok(nj) => {
                        log::debug!("Successfully unqueued job with ID={}", &nj.id);
                    }
                    Err(e) => {
                        log::error!("TODO: Handle failed queued job: {:#}", e);
                    }
                })
        });
        ctx.spawn(fut);
        return Box::pin(
            async move {
                Ok(NewJobResponse {
                    id,
                    info: NewJobInfo::Queued(queue_pos),
                })
            }
            .into_actor(self),
        );
    }
}

impl Handler<LookupJob> for JobManager {
    type Result = <LookupJob as actix::Message>::Result;

    fn handle(&mut self, msg: LookupJob, _ctx: &mut Self::Context) -> Self::Result {
        //log::debug!("Jobs={:?}", self.jobs.keys().collect::<Vec<_>>());
        log::warn!("TODO: LookupJob has to work with queued jobs!");
        self.jobs.get(&msg.0).cloned()
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
                    .map(|q| q.data.iter().all(|(qj_id, _)| qj_id != &id))
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
