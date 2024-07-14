use actix::prelude::*;
use crate::{utils::*, AcedrgArgs};
use super::{JobFailureReason, JobOutput, JobStatus, ACEDRG_OUTPUT_FILENAME};
use std::process::Output;
use std::{process::Stdio, time::Duration};
use tokio::{process::{Child, Command}, time::timeout};


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

pub struct JobRunner {
    data: JobData,
    workdir: WorkDir,
    /// Event propagation
    recipients: Vec<Recipient<JobData>>
}

impl Actor for JobRunner {
    type Context = Context<Self>;
    
}

struct WorkerResult(Result<std::io::Result<Output>, tokio::time::error::Elapsed>);
impl Message for WorkerResult {
    type Result = ();
}

impl Handler<WorkerResult> for JobRunner {
    type Result = ();

    fn handle(&mut self, msg: WorkerResult, _ctx: &mut Self::Context) -> Self::Result {
        match msg.0 {
            Err(_elapsed) => {
                self.data.status = JobStatus::Failed(JobFailureReason::TimedOut);
            }
            Ok(Err(e)) => {
                self.data.status = JobStatus::Failed(JobFailureReason::IOError(e.kind()));
            }
            Ok(Ok(output)) => {
                self.data.status = if output.status.success() {
                    JobStatus::Finished
                } else {
                    JobStatus::Failed(JobFailureReason::AcedrgError)
                };
                self.data.job_output = Some(JobOutput {
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                });
            }
        }
        for i in &self.recipients {
            i.do_send(self.data.clone());
        }
    }
}

impl JobRunner {
    async fn worker(child: Child, addr: Addr<Self>) {
        let res = timeout(Duration::from_secs(5 * 60), child.wait_with_output()).await;
        let _res = addr.send(WorkerResult(res)).await;
    }

    pub async fn create_job(recipients: Vec<Recipient<JobData>>, args: &AcedrgArgs) -> std::io::Result<Addr<JobRunner>> {
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
            data: JobData{
                status: JobStatus::Pending,
                job_output: None,
            },
            recipients
        };

        

        Ok(JobRunner::create(|ctx: &mut Context<JobRunner>| {
            let worker = JobRunner::worker(child, ctx.address());
            let fut = actix::fut::wrap_future(worker);
            ctx.spawn(fut);
            ret
        }))
    }
}