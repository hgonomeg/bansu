use super::messages::{AcedrgArgs, JobId, JobStatusInfo};
use super::utils::*;
use lazy_static::lazy_static;
use std::sync::Arc;
use std::{collections::BTreeMap, process::Stdio, time::Duration};
use tokio::{process::Command, sync::Mutex, time::timeout};
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

pub struct JobData {
    pub workdir: WorkDir,
    /// Use this to check if the job is still running
    pub status: JobStatusInfo,
    /// Only present if the job failed
    pub failure_reason: Option<JobFailureReason>,
    /// Gets filled when the job completes.
    /// If the job fails, it will only be filled
    /// if the error came from acedrg itself
    pub job_output: Option<JobOutput>,
}

pub struct JobManager {
    jobs: BTreeMap<JobId, Arc<Mutex<JobData>>>,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: BTreeMap::new(),
        }
    }
    pub async fn create_job(args: &AcedrgArgs) -> std::io::Result<Arc<Mutex<JobData>>> {
        let workdir = mkworkdir().await?;
        let smiles_file_path = workdir.path.join("acedrg_smiles_input");
        dump_string_to_file(&smiles_file_path, &args.smiles).await?;
        let child = Command::new("acedrg")
            .current_dir(&workdir.path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-i")
            .arg(&smiles_file_path)
            // TODO: SANITIZE INPUT!
            .args(args.commandline_args.clone())
            .arg("-o")
            .arg(ACEDRG_OUTPUT_FILENAME)
            .spawn()?;

        let ret = Arc::from(Mutex::from(JobData {
            workdir,
            status: JobStatusInfo::Pending,
            job_output: None,
            failure_reason: None,
        }));

        // Worker task
        let marc = ret.clone();
        let _ = tokio::task::spawn(async move {
            let output_timeout_res =
                timeout(Duration::from_secs(5 * 60), child.wait_with_output()).await;
            let mut m_data = marc.lock().await;
            match output_timeout_res {
                Err(_elapsed) => {
                    m_data.status = JobStatusInfo::Failed;
                    m_data.failure_reason = Some(JobFailureReason::TimedOut);
                }
                Ok(Err(e)) => {
                    m_data.status = JobStatusInfo::Failed;
                    m_data.failure_reason = Some(JobFailureReason::IOError(e.kind()));
                }
                Ok(Ok(output)) => {
                    m_data.status = if output.status.success() {
                        JobStatusInfo::Finished
                    } else {
                        JobStatusInfo::Failed
                    };
                    if !output.status.success() {
                        m_data.failure_reason = Some(JobFailureReason::AcedrgError);
                    }
                    m_data.job_output = Some(JobOutput {
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    });
                }
            }
        });
        Ok(ret)
    }
    pub async fn acquire_lock<'a>() -> tokio::sync::MutexGuard<'a, Self> {
        let r = GLOBAL_JOB_MANAGER.lock().await;
        r
    }
    pub fn add_job(&mut self, job: Arc<Mutex<JobData>>) -> JobId {
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
    pub fn query_job(&self, job_id: &JobId) -> Option<&Arc<Mutex<JobData>>> {
        let job_opt = self.jobs.get(job_id);
        job_opt
    }
}
