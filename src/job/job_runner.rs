use super::job_handle::JobHandle;
use super::job_type::{Job, JobSpawnError};
use super::{JobData, JobFailureReason, JobOutput, JobStatus};
use crate::{utils::*, ws_connection::WsConnection};
use actix::prelude::*;
use anyhow::Context as AnyhowContext;
use std::{process::Output, sync::Arc, time::Duration};
use tokio::sync::OwnedSemaphorePermit;

use thiserror::Error;
use tokio::time::timeout;

pub enum OutputKind {
    CIF,
}

#[derive(Debug, Error)]
pub enum OutputRequestError {
    #[error("IOError {{?}}")]
    IOError(#[from] std::io::Error),
    #[error("Job is still running / is queued")]
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
    workdir: Option<WorkDir>,
    job_object: Arc<dyn Job>,
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

#[derive(Message)]
#[rtype(result = "()")]
pub struct InitializeQueuedJob(pub Option<OwnedSemaphorePermit>);

impl Handler<InitializeQueuedJob> for JobRunner {
    type Result = ();

    fn handle(&mut self, msg: InitializeQueuedJob, ctx: &mut Self::Context) -> Self::Result {
        let id = self.id.clone();
        let jo = self.job_object.clone();
        let fut = actix::fut::wrap_future::<_, Self>(async move {
            log::debug!("Initializing queued job");
            Self::try_init(&id, &jo).await
        })
        .map(|res, actor, ctx| {
            match res {
                Ok((workdir, jhandle)) => {
                    log::debug!("Queued job initialized successfully (ID={})!", &actor.id);
                    actor.workdir = Some(workdir);
                    actor.data.status = JobStatus::Pending;
                    let timeout_val = actor.job_object.timeout_value();
                    let worker = JobRunner::worker(
                        jhandle,
                        ctx.address(),
                        actor.id.clone(),
                        timeout_val,
                        msg.0,
                    );
                    actix_rt::spawn(worker);
                }
                Err(e) => {
                    log::warn!(
                        "Queued job failed to initialize (ID={}): {:#}",
                        &actor.id,
                        &e
                    );
                    actor.data.status =
                        JobStatus::Failed(JobFailureReason::SetupError(format!("{:#}", e)));
                }
            }
            log::debug!("{} - Status updated: {:?}", actor.id, &actor.data.status);
            for i in &actor.websocket_addrs {
                i.do_send(actor.data.clone());
            }
        });
        ctx.spawn(fut);
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
        if self.data.status == JobStatus::Pending || self.data.status == JobStatus::Queued {
            log::info!(
                "{} - Turning down request for output - the job is still pending / queued.",
                self.id
            );
            return Box::pin(async { Err(OutputRequestError::JobStillPending) }.into_actor(self));
        }
        let jo = &self.job_object;
        let Some(filepath) = self
            .workdir
            .as_ref()
            .map(|x| jo.output_filename(&x.path, msg.kind))
            .flatten()
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
    async fn worker(
        handle: JobHandle,
        addr: Addr<Self>,
        id: String,
        timeout_value: Duration,
        semaphore_permit: Option<OwnedSemaphorePermit>,
    ) {
        log::info!("{} - Started worker", &id);
        let worker_fut = async move {
            // TODO: Implement updating job output realtime
            handle.join().await
        };
        let res = timeout(timeout_value, worker_fut).await;
        drop(semaphore_permit);
        let _res = addr.send(WorkerResult(res)).await;
        log::info!("{} - Worker terminates", id);
    }
    async fn try_init(
        id: &str,
        job_object: &Arc<dyn Job>,
    ) -> Result<(WorkDir, JobHandle), JobSpawnError> {
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
        Ok((workdir, jhandle))
    }
    /// Creates a regular (non-queued) job
    pub async fn try_create_job(
        id: String,
        job_object: Arc<dyn Job>,
        semaphore_permit: Option<OwnedSemaphorePermit>,
    ) -> Result<Addr<JobRunner>, JobSpawnError> {
        let (workdir, jhandle) = Self::try_init(&id, &job_object).await?;
        let timeout_val = job_object.timeout_value();
        let ret = Self {
            id: id.clone(),
            workdir: Some(workdir),
            job_object,
            data: JobData {
                status: JobStatus::Pending,
                job_output: None,
            },
            websocket_addrs: vec![],
        };
        Ok(JobRunner::create(|ctx: &mut Context<JobRunner>| {
            let worker =
                JobRunner::worker(jhandle, ctx.address(), id, timeout_val, semaphore_permit);
            actix_rt::spawn(worker);
            ret
        }))
    }

    /// In case of errors, spawns errored-out job
    pub fn create_queued_job(
        id: String,
        job_object: Arc<dyn Job>,
        semaphore_permit: Option<OwnedSemaphorePermit>,
    ) -> Addr<JobRunner> {
        let ret = Self {
            id: id.clone(),
            workdir: None,
            job_object,
            data: JobData {
                status: JobStatus::Queued,
                job_output: None,
            },
            websocket_addrs: vec![],
        }
        .start();
        ret.do_send(InitializeQueuedJob(semaphore_permit));
        ret
    }
}
