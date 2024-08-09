use super::messages::JobId;
use actix::prelude::*;
// use futures_util::FutureExt;
use job_runner::JobRunner;
use job_type::Job;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobFailureReason {
    TimedOut,
    IOError(std::io::ErrorKind),
    AcedrgError,
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

pub struct JobManager {
    jobs: BTreeMap<JobId, Addr<JobRunner>>,
}

impl Actor for JobManager {
    type Context = Context<Self>;
}

pub struct NewJob(pub Box<dyn Job>);
impl Message for NewJob {
    type Result = anyhow::Result<(JobId, Addr<JobRunner>)>;
}

pub struct LookupJob(pub JobId);
impl Message for LookupJob {
    type Result = Option<Addr<JobRunner>>;
}

impl Handler<LookupJob> for JobManager {
    type Result = <LookupJob as actix::Message>::Result;

    fn handle(&mut self, msg: LookupJob, _ctx: &mut Self::Context) -> Self::Result {
        //log::debug!("Jobs={:?}", self.jobs.keys().collect::<Vec<_>>());
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

    fn handle(&mut self, msg: NewJob, _ctx: &mut Self::Context) -> Self::Result {
        let id = loop {
            let new_id = uuid::Uuid::new_v4();
            let id = new_id.to_string();
            if !self.jobs.contains_key(&id) {
                break id;
            }
        };
        let tm = msg.0.timeout_value();
        Box::pin(
            async move {
                let job_object = msg.0;
                JobRunner::create_job(id.clone(), job_object)
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
                    (jid, job)
                })
            }),
        )
    }
}

impl JobManager {
    pub fn new() -> Self {
        log::info!("Initializing JobManager.");
        Self {
            jobs: BTreeMap::new(),
        }
    }
}
