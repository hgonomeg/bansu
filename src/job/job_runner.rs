use super::job_handle::JobHandle;
use super::job_type::Job;
use super::{JobData, JobFailureReason, JobOutput, JobStatus};
use crate::utils::*;
use crate::ws_connection::WsConnection;
use actix::prelude::*;
use anyhow::Context as AnyhowContext;
use std::process::Output;
use std::time::Duration;

use thiserror::Error;
use tokio::time::timeout;

pub enum OutputKind {
    CIF,
}

#[derive(Debug, Error)]
pub enum OutputRequestError {
    #[error("IOError {{?}}")]
    IOError(#[from] std::io::Error),
    #[error("Job is still running")]
    JobStillPending,
    #[error("This output kind is not supported by this job type")]
    OutputKindNotSupported,
}

pub struct OutputFileRequest {
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

impl Message for OutputFileRequest {
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

struct WorkerResult(Result<anyhow::Result<Output>, tokio::time::error::Elapsed>);
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
                log::error!("{} - Job timed out.", self.id);
                self.data.status = JobStatus::Failed(JobFailureReason::TimedOut);
            }
            Ok(Err(e)) => {
                let msg = format!("{:#}", e);
                log::error!("{} - Job handling error: {}", self.id, &msg);
                self.data.status = JobStatus::Failed(JobFailureReason::SetupError(msg));
            }
            Ok(Ok(output)) => {
                self.data.status = if output.status.success() {
                    log::info!("{} - Job finished.", self.id);
                    JobStatus::Finished
                } else {
                    log::warn!("{} - Job failed.", self.id);
                    JobStatus::Failed(JobFailureReason::JobProcessError)
                };
                self.data.job_output = Some(JobOutput {
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                });
            }
        }
        log::debug!("{} - Status updated: {:?}", self.id, &self.data.status);
        for i in &self.websocket_addrs {
            i.do_send(self.data.clone());
        }
    }
}

impl Handler<OutputFileRequest> for JobRunner {
    type Result = ResponseActFuture<Self, <OutputFileRequest as actix::Message>::Result>;

    fn handle(&mut self, msg: OutputFileRequest, _ctx: &mut Self::Context) -> Self::Result {
        if self.data.status == JobStatus::Pending {
            log::info!(
                "{} - Turning down request for output - the job is still pending.",
                self.id
            );
            return Box::pin(async { Err(OutputRequestError::JobStillPending) }.into_actor(self));
        }
        let Some(filepath) = self
            .job_object
            .output_filename(&self.workdir.path, msg.kind)
        else {
            return Box::pin(
                async { Err(OutputRequestError::OutputKindNotSupported) }.into_actor(self),
            );
        };

        Box::pin(
            async move {
                Ok(tokio::fs::OpenOptions::new()
                    .read(true)
                    .open(filepath)
                    .await?)
            }
            .into_actor(self),
        )
    }
}

impl JobRunner {
    async fn worker(handle: JobHandle, addr: Addr<Self>, id: String, timeout_value: Duration) {
        log::info!("{} - Started worker", &id);
        let res = timeout(timeout_value, handle.join()).await;
        let _res = addr.send(WorkerResult(res)).await;
        log::info!("{} - Worker terminates", id);
    }

    pub async fn create_job(
        id: String,
        job_object: Box<dyn Job>,
    ) -> anyhow::Result<Addr<JobRunner>> {
        log::info!("Creating new {} job - {}", job_object.name(), &id);
        job_object.validate_input()?;
        let workdir = mkworkdir()
            .await
            .with_context(|| "Could not create working directory")?;
        let input_path = job_object
            .write_input(&workdir.path)
            .await
            .with_context(|| "Could not write input for job")?;
        log::info!("{} - Starting job", &id);
        let jhandle = job_object
            .launch(&workdir.path, &input_path)
            .await
            .with_context(|| "Could not start job")?;
        let timeout_val = job_object.timeout_value();

        let ret = Self {
            id: id.clone(),
            workdir,
            job_object,
            data: JobData {
                status: JobStatus::Pending,
                job_output: None,
            },
            websocket_addrs: vec![],
        };

        Ok(JobRunner::create(|ctx: &mut Context<JobRunner>| {
            let worker = JobRunner::worker(jhandle, ctx.address(), id, timeout_val);
            let fut = actix::fut::wrap_future(worker);
            ctx.spawn(fut);
            ret
        }))
    }
}
