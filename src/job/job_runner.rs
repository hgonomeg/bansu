use super::{JobData, JobFailureReason, JobOutput, JobStatus, ACEDRG_OUTPUT_FILENAME};
use crate::{utils::*, AcedrgArgs};
use actix::prelude::*;
use std::process::Output;
use std::{process::Stdio, time::Duration};
use thiserror::Error;
use tokio::{
    process::{Child, Command},
    time::timeout,
};

pub enum OutputKind {
    CIF,
}

#[derive(Debug, Error)]
pub enum OutputRequestError {
    #[error("IOError {{?}}")]
    IOError(#[from] std::io::Error),
    #[error("Job is still running")]
    JobStillPending,
}

pub struct OutputPathRequest {
    pub kind: OutputKind,
}

pub struct JobRunner {
    id: String,
    data: JobData,
    workdir: WorkDir,
    /// Event propagation
    recipients: Vec<Recipient<JobData>>,
}

impl Actor for JobRunner {
    type Context = Context<Self>;
}

impl Message for OutputPathRequest {
    type Result = Result<tokio::fs::File, OutputRequestError>;
}

pub struct AddRecipient(pub Recipient<JobData>);
impl Message for AddRecipient {
    type Result = ();
}

impl Handler<AddRecipient> for JobRunner {
    type Result = ();

    fn handle(&mut self, msg: AddRecipient, _ctx: &mut Self::Context) -> Self::Result {
        self.recipients.push(msg.0);
    }
}

struct WorkerResult(Result<std::io::Result<Output>, tokio::time::error::Elapsed>);
impl Message for WorkerResult {
    type Result = ();
}

impl Handler<WorkerResult> for JobRunner {
    type Result = ();

    fn handle(&mut self, msg: WorkerResult, _ctx: &mut Self::Context) -> Self::Result {
        //log::debug!("JobRunner got result {:?}", &msg.0);
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
        log::info!("JobRunner - job status updated: {:?}", &self.data.status);
        for i in &self.recipients {
            i.do_send(self.data.clone());
        }
    }
}

impl Handler<OutputPathRequest> for JobRunner {
    type Result = ResponseActFuture<Self, <OutputPathRequest as actix::Message>::Result>;

    fn handle(&mut self, msg: OutputPathRequest, _ctx: &mut Self::Context) -> Self::Result {
        if self.data.status == JobStatus::Pending {
            log::info!("Turning down request for job output - the job is still pending.");
            return Box::pin(async { Err(OutputRequestError::JobStillPending) }.into_actor(self));
        }
        Box::pin(Self::open_output_file(msg.kind, self.workdir.path.clone()).into_actor(self))
    }
}

impl JobRunner {
    async fn worker(child: Child, addr: Addr<Self>) {
        log::info!("Started JobRunner worker.");
        let res = timeout(Duration::from_secs(5 * 60), child.wait_with_output()).await;
        let _res = addr.send(WorkerResult(res)).await;
        log::info!("JobRunner worker terminates.");
    }
    async fn open_output_file(
        kind: OutputKind,
        workdir_filepath: std::path::PathBuf,
    ) -> Result<tokio::fs::File, OutputRequestError> {
        let mut filepath = workdir_filepath;
        match kind {
            OutputKind::CIF => {
                filepath.push(format!("{}.cif", ACEDRG_OUTPUT_FILENAME));
            }
        }
        Ok(tokio::fs::OpenOptions::new()
            .read(true)
            .open(filepath)
            .await?)
    }

    pub async fn create_job(
        id: String,
        recipients: Vec<Recipient<JobData>>,
        args: &AcedrgArgs,
    ) -> std::io::Result<Addr<JobRunner>> {
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

        let ret = Self {
            id,
            workdir,
            data: JobData {
                status: JobStatus::Pending,
                job_output: None,
            },
            recipients,
        };

        Ok(JobRunner::create(|ctx: &mut Context<JobRunner>| {
            let worker = JobRunner::worker(child, ctx.address());
            let fut = actix::fut::wrap_future(worker);
            ctx.spawn(fut);
            ret
        }))
    }
}
