use crate::{
    job::{
        job_runner::{AddWebSocketAddr, JobRunner, QueryJobData},
        JobData, JobEntry, JobManager, LookupJob, MonitorQueuedJob,
    },
    messages::*,
};
use actix::prelude::*;
use actix_ws::{
    CloseCode, CloseReason, Message as WsMessage, MessageStream, ProtocolError as WsProtocolError,
    Session,
};

pub struct WsConnection {
    job_manager: Addr<JobManager>,
    job: Option<Addr<JobRunner>>,
    job_id: JobId,
    session: Session,
}

impl WsConnection {
    fn job_addr_handshake(&self, job: Addr<JobRunner>, ctx: &mut <Self as actix::Actor>::Context) {
        log::debug!(
            "Registering WsConnection on JobRunner (ID={})",
            &self.job_id
        );
        job.do_send(AddWebSocketAddr(ctx.address()));
        let job_id = self.job_id.clone();
        ctx.spawn(
            async move {
                log::debug!("Performing initial fetch of JobData (ID={})", job_id);
                let data = job.send(QueryJobData).await.unwrap();
                (data, job_id)
            }
            .into_actor(self)
            .map(|(data, job_id), _a, ctx| {
                ctx.notify(data);
                log::debug!("Initial fetch of JobData completed (ID={})", job_id);
            }),
        );
    }
    async fn handle_status_update(
        job_id: JobId,
        msg: WsJobDataUpdate,
        mut session: Session,
    ) -> Result<(), actix_ws::Closed> {
        log::debug!("Sending JobDataUpdate for job {}", job_id);
        session.text(serde_json::to_string(&msg).unwrap()).await?;
        match msg.status {
            JobStatusInfo::Finished => {
                session
                    .close(Some(CloseReason {
                        code: CloseCode::Normal,
                        description: None,
                    }))
                    .await?;
            }
            JobStatusInfo::Failed => {
                session
                    .close(Some(CloseReason {
                        code: CloseCode::Error,
                        description: None,
                    }))
                    .await?;
            }
            _ => (),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct SetRunner(pub Addr<JobRunner>);

impl Handler<SetRunner> for WsConnection {
    type Result = <SetRunner as actix::Message>::Result;

    fn handle(&mut self, msg: SetRunner, ctx: &mut Self::Context) -> Self::Result {
        if self.job.is_some() {
            log::error!("WsConnection already has JobRunner set!");
        } else {
            log::debug!("Received JobRunner.");
            self.job = Some(msg.0.clone());
            self.job_addr_handshake(msg.0, ctx);
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct PeriodicUpdateTrigger;

impl StreamHandler<PeriodicUpdateTrigger> for WsConnection {
    fn handle(&mut self, _item: PeriodicUpdateTrigger, ctx: &mut Self::Context) {
        let job_id = self.job_id.clone();
        if let Some(job) = self.job.clone() {
            ctx.spawn(
                async move {
                    log::debug!("Performing periodic fetch of JobData (ID={})", job_id);
                    let data = job.send(QueryJobData).await.unwrap();
                    (data, job_id)
                }
                .into_actor(self)
                .map(|(data, job_id), _a, ctx| {
                    ctx.notify(data);
                    log::debug!("Periodic fetch of JobData completed (ID={})", job_id);
                }),
            );
        } else {
            let mgr = self.job_manager.clone();
            ctx.spawn(actix::fut::wrap_future(async move {
                log::debug!("Performing periodic lookup of queued job (ID={})", &job_id);
                match mgr.send(LookupJob(job_id.clone())).await {
                    Ok(Some(JobEntry::Queued(queue_pos))) => {
                        return(job_id, Some(queue_pos));
                    },
                    Ok(Some(JobEntry::Spawned(_))) => {
                        log::warn!("Ignoring spawned job address sent from queued-job lookup. It should soon be registered anyway. ID={}", &job_id);
                    },
                    Ok(None) => {
                        log::error!("Job not found during periodic lookup of queued job! (ID={})", &job_id);
                    },
                    Err(e) => {
                        log::error!("Periodic lookup of queued job failed! {}", &e);
                    },
                }
                (job_id, None)
            }).map(|(job_id, queue_pos_opt), actor: &mut Self, _ctx| {
                if let Some(queue_pos) = queue_pos_opt {
                    log::debug!("Periodic lookup of queued job completed (ID={})", job_id);
                    actix::spawn(Self::handle_status_update(job_id, WsJobDataUpdate::new_from_queue_pos(queue_pos), actor.session.clone()));
                }
            }));
        }
    }
}

impl Handler<JobData> for WsConnection {
    type Result = <JobData as actix::Message>::Result;

    fn handle(&mut self, msg: JobData, _ctx: &mut Self::Context) -> Self::Result {
        let id = self.job_id.clone();
        actix_rt::spawn(Self::handle_status_update(
            id,
            WsJobDataUpdate::from(msg),
            self.session.clone(),
        ));
    }
}

impl Actor for WsConnection {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("Initializing WebSocket connection for job {}", &self.job_id);
        if let Some(job) = self.job.clone() {
            self.job_addr_handshake(job, ctx);
        } else {
            let jm = self.job_manager.clone();
            let id = self.job_id.clone();
            let addr = ctx.address();
            ctx.spawn(
                async move {
                    log::debug!("Sending a request to monitor queued job (ID={}", &id);
                    jm.send(MonitorQueuedJob(id, addr)).await.unwrap();
                }
                .into_actor(self),
            );
            ctx.add_stream(futures_util::stream::once(async {
                PeriodicUpdateTrigger {}
            }));
        }

        let sleep_dur = std::time::Duration::from_secs(
            std::env::var("BANSU_PERIODIC_WS_UPDATE_INTERVAL")
                .ok()
                .map(|port_str| {
                    port_str
                        .parse::<u64>()
                        .inspect_err(|e| {
                            log::warn!("Invalid BANSU_PERIODIC_WS_UPDATE_INTERVAL value: {}", e)
                        })
                        .ok()
                })
                .flatten()
                .unwrap_or(16),
        );
        ctx.add_stream(futures_util::stream::unfold(
            sleep_dur,
            |sleep_dur| async move {
                tokio::time::sleep(sleep_dur.clone()).await;
                Some((PeriodicUpdateTrigger, sleep_dur))
            },
        ));
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        log::info!("Closed WebSocket connection for job {}", self.job_id);
    }
}

impl WsConnection {
    pub fn new(
        job_manager: Addr<JobManager>,
        job: Option<Addr<JobRunner>>,
        job_id: JobId,
        session: Session,
        msg_stream: MessageStream,
    ) -> Addr<Self> {
        Self::create(|ctx| {
            ctx.add_stream(msg_stream);
            Self {
                job,
                job_manager,
                job_id,
                session,
            }
        })
    }
}

impl StreamHandler<Result<WsMessage, WsProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<WsMessage, WsProtocolError>, _ctx: &mut Self::Context) {
        // let mut error_out = |error_msg: &str| {
        //     ctx.text(
        //         serde_json::to_string(&GenericErrorMessage {
        //             error_message: Some(error_msg.to_owned()),
        //         })
        //         .unwrap(),
        //     );
        //     log::error!("{}", error_msg);
        // };

        match msg {
            Ok(WsMessage::Ping(_msg)) => {
                let mut ses = self.session.clone();
                let fut = async move {
                    let _ = ses.pong(&[]).await;
                };
                actix_rt::spawn(fut);
                log::info!("{} - Replying with \"Pong\"", &self.job_id);
            }
            Ok(WsMessage::Text(_text)) => {
                log::info!("{} - Ignoring incoming text message.", &self.job_id);
                // let client_message = serde_json::from_str::<WsClientMessage>(&text);
                // match client_message {
                //     Ok(client_message) => match client_message.kind {
                //         WsClientMessageKind::QueryJob => {
                //             let Some(job_id) = client_message.job_id else {
                //                 error_out("Missing job_id in QueryJob request.");
                //                 return;
                //             };
                //             self.query_job(job_id);
                //         }
                //     },
                //     Err(_e) => {
                //         error_out("Unrecognized request kind.");
                //     }
                // }
            }
            Ok(WsMessage::Binary(_bin)) => {
                log::info!("{} - Ignoring incoming binary message.", &self.job_id);
                //ctx.binary(bin)
            }
            _ => {
                // log::info!("{} - Ignoring incoming message.", &self.job_id);
            }
        }
    }
}
