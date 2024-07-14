use super::messages::JobId;
use actix::prelude::*;
use job_runner::JobRunner;
use lazy_static::lazy_static;
use std::sync::Arc;
use std::{collections::BTreeMap, time::Duration};
use tokio::sync::Mutex;
pub mod job_runner;

lazy_static! {
    static ref GLOBAL_JOB_MANAGER: Arc<Mutex<JobManager>> =
        Arc::from(Mutex::from(JobManager::new()));
}

pub const ACEDRG_OUTPUT_FILENAME: &'static str = "acedrg_output";

#[derive(Clone, Debug)]
pub struct JobOutput {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug)]
pub enum JobStatus {
    Pending,
    Finished,
    Failed(JobFailureReason),
}

#[derive(Debug, Clone)]
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

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: BTreeMap::new(),
        }
    }
    pub async fn acquire_lock<'a>() -> tokio::sync::MutexGuard<'a, Self> {
        let r = GLOBAL_JOB_MANAGER.lock().await;
        r
    }
    pub fn add_job(&mut self, job: Addr<JobRunner>) -> JobId {
        let new_id = uuid::Uuid::new_v4();
        let id = new_id.to_string();
        self.jobs.insert(id.clone(), job);

        // Cleanup task
        let m_id = id.clone();
        tokio::task::spawn(async move {
            // Make sure to keep this longer than the job timeout
            tokio::time::sleep(Duration::from_secs(15 * 60)).await;
            let mut jm_lock = GLOBAL_JOB_MANAGER.lock().await;
            // We don't have to care if the job is still running or not.
            // In the worst-case scenario, it should have timed-out a long time ago.
            jm_lock.jobs.remove(&m_id);
        });

        id
    }
    pub fn query_job(&self, job_id: &JobId) -> Option<&Addr<JobRunner>> {
        let job_opt = self.jobs.get(job_id);
        job_opt
    }
}
