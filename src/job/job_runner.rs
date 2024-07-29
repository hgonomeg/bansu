use super::job_type::acedrg::AcedrgJob;
use super::job_type::Job;
use super::{JobData, JobFailureReason, JobOutput, JobStatus, ACEDRG_OUTPUT_FILENAME};
use crate::job::ACEDRG_TIMEOUT;
use crate::ws_connection::WsConnection;
use crate::{utils::*, AcedrgArgs};
use actix::prelude::*;
use std::process::Output;
use std::process::Stdio;
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
    job_object: Box<dyn Job>,
    /// Event propagation
    websocket_addrs: Vec<Addr<WsConnection>>,
}

impl Actor for JobRunner {
    type Context = Context<Self>;
}

impl Message for OutputPathRequest {
    type Result = Result<tokio::fs::File, OutputRequestError>;
}

pub struct AddWebSocketAddr(pub Addr<WsConnection>);
impl Message for AddWebSocketAddr {
    type Result = ();
}

impl Handler<AddWebSocketAddr> for JobRunner {
    type Result = ();

    fn handle(&mut self, msg: AddWebSocketAddr, _ctx: &mut Self::Context) -> Self::Result {
        self.websocket_addrs.push(msg.0);
    }
}

struct WorkerResult(Result<std::io::Result<Output>, tokio::time::error::Elapsed>);
impl Message for WorkerResult {
    type Result = ();
}

#[derive(Message)]
#[rtype(result = "JobData")]
pub struct QueryJobData;

impl Handler<QueryJobData> for JobRunner {
    type Result = JobData;

    fn handle(&mut self, _msg: QueryJobData, _ctx: &mut Self::Context) -> Self::Result {
        self.data.clone()
    }
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
        log::info!(
            "JobRunner:job:{} - Status updated: {:?}",
            self.id,
            &self.data.status
        );
        for i in &self.websocket_addrs {
            i.do_send(self.data.clone());
        }
    }
}

impl Handler<OutputPathRequest> for JobRunner {
    type Result = ResponseActFuture<Self, <OutputPathRequest as actix::Message>::Result>;

    fn handle(&mut self, msg: OutputPathRequest, _ctx: &mut Self::Context) -> Self::Result {
        if self.data.status == JobStatus::Pending {
            log::info!(
                "JobRunner:job:{} - Turning down request for output - the job is still pending.",
                self.id
            );
            return Box::pin(async { Err(OutputRequestError::JobStillPending) }.into_actor(self));
        }
        Box::pin(Self::open_output_file(msg.kind, self.workdir.path.clone()).into_actor(self))
    }
}

impl JobRunner {
    async fn worker(child: Child, addr: Addr<Self>, id: String) {
        log::info!("JobRunner:job:{} - Started worker", &id);
        let res = timeout(ACEDRG_TIMEOUT, child.wait_with_output()).await;
        let _res = addr.send(WorkerResult(res)).await;
        log::info!("JobRunner:job:{} - Worker terminates", id);
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

    pub async fn create_job(id: String, args: &AcedrgArgs) -> std::io::Result<Addr<JobRunner>> {
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
            id: id.clone(),
            workdir,
            job_object: Box::from(AcedrgJob { args: args.clone() }),
            data: JobData {
                status: JobStatus::Pending,
                job_output: None,
            },
            websocket_addrs: vec![],
        };

        Ok(JobRunner::create(|ctx: &mut Context<JobRunner>| {
            let worker = JobRunner::worker(child, ctx.address(), id);
            let fut = actix::fut::wrap_future(worker);
            ctx.spawn(fut);
            ret
        }))
    }
}
