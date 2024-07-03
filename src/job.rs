use super::messages::{AcedrgArgs, JobId, JobStatusInfo};
use super::utils::*;
use lazy_static::lazy_static;
use std::sync::Arc;
use std::{collections::BTreeMap, process::Stdio};
use tokio::{process::Command, sync::Mutex};

lazy_static! {
    static ref GLOBAL_JOB_MANAGER: Arc<Mutex<JobManager>> =
        Arc::from(Mutex::from(JobManager::new()));
}

pub struct JobData {
    workdir: WorkDir,
    pub status: JobStatusInfo,
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
            .arg("acedrg_output")
            .spawn()?;
        let ret = Arc::from(Mutex::from(JobData {
            workdir,
            status: JobStatusInfo::Pending,
        }));
        let marc = ret.clone();
        let _ = tokio::task::spawn(async move {
            let output_res = child.wait_with_output().await;
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
        id
    }
    pub fn query_job(&self, job_id: &JobId) -> Option<&Arc<Mutex<JobData>>> {
        let job_opt = self.jobs.get(job_id);
        job_opt
    }
}
