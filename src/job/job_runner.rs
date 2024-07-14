use actix::prelude::*;
use crate::{utils::*, AcedrgArgs};
use super::{JobFailureReason, JobOutput, JobStatus, ACEDRG_OUTPUT_FILENAME};
use std::sync::Arc;
use std::{process::Stdio, time::Duration};
use tokio::{process::Command, sync::Mutex, time::timeout};

pub struct JobRunner {
    pub workdir: WorkDir,
    /// Use this to check if the job is still running
    pub status: JobStatus,
    /// Gets filled when the job completes.
    /// If the job fails, it will only be filled
    /// if the error came from acedrg itself
    pub job_output: Option<JobOutput>,
}

impl Actor for JobRunner {
    type Context = Context<Self>;

    // fn create<F>(f: F) -> Addr<Self>
    // where
    // Self: Actor<Context = Context<Self>>,
    // F: FnOnce(&mut Context<Self>) -> Self {

    //     let 
    // }
    
}

impl JobRunner {
    pub async fn create_job(args: &AcedrgArgs) -> std::io::Result<Addr<JobRunner>> {
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

        let ret = Self{
            workdir,
            status: JobStatus::Pending,
            job_output: None,
        };

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
                    m_data.failure_reason = Some(JobFailureReason::IOError(e));
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
}