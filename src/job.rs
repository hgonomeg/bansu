use super::messages::JobId;
use actix::prelude::*;
// use futures_util::FutureExt;
use job_runner::JobRunner;
use std::{collections::BTreeMap, time::Duration};
pub mod job_runner;

pub const ACEDRG_OUTPUT_FILENAME: &'static str = "acedrg_output";

#[derive(Clone, Debug)]
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

#[derive(Debug, Clone)]
pub struct JobData {
    pub status: JobStatus,
    /// Gets filled when the job completes.
    /// If the job fails, it will only be filled
    /// if the error came from acedrg itself
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

pub struct AddJob(pub Addr<JobRunner>);
impl Message for AddJob {
    type Result = JobId;
}

pub struct QueryJob(pub JobId);
impl Message for QueryJob {
    type Result = Option<Addr<JobRunner>>;
}

impl Handler<QueryJob> for JobManager {
    type Result = <QueryJob as actix::Message>::Result;

    fn handle(&mut self, msg: QueryJob, _ctx: &mut Self::Context) -> Self::Result {
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
    }
}

impl Handler<AddJob> for JobManager {
    type Result = <AddJob as actix::Message>::Result;

    fn handle(&mut self, msg: AddJob, ctx: &mut Self::Context) -> Self::Result {
        let new_id = uuid::Uuid::new_v4();
        let id = new_id.to_string();
        self.jobs.insert(id.clone(), msg.0);

        // Cleanup task
        // Make sure to keep this longer than the job timeout
        // We don't have to care if the job is still running or not.
        // In the worst-case scenario, it should have timed-out a long time ago.
        ctx.notify_later(RemoveJob(id.clone()), Duration::from_secs(15 * 60));

        id
    }
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: BTreeMap::new(),
        }
    }
}
