use crate::{
    job::{
        job_runner::{AddWebSocketAddr, JobRunner, QueryJobData},
        JobData, JobManager, JobStatus, MonitorQueuedJob,
    },
    messages::*,
};
use actix::prelude::*;
use actix_web_actors::ws::{self, CloseCode, CloseReason};

pub struct WsConnection {
    job_manager: Addr<JobManager>,
    job: Option<Addr<JobRunner>>,
    job_id: JobId,
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
            log::info!("Received JobRunner.");
            self.job = Some(msg.0.clone());
            self.job_addr_handshake(msg.0, ctx);
        }
    }
}

impl Handler<JobData> for WsConnection {
    type Result = <JobData as actix::Message>::Result;

    fn handle(&mut self, msg: JobData, ctx: &mut Self::Context) -> Self::Result {
        log::info!("Sending JobDataUpdate for job {}", self.job_id);
        ctx.text(serde_json::to_string(&WsJobDataUpdate::from(msg.clone())).unwrap());
        match msg.status {
            JobStatus::Finished => {
                ctx.close(Some(CloseReason {
                    code: CloseCode::Normal,
                    description: None,
                }));
            }
            JobStatus::Failed(_e) => {
                ctx.close(Some(CloseReason {
                    code: CloseCode::Error,
                    description: None,
                }));
            }
            _ => (),
        }
    }
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

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
        }
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        log::info!("Closed WebSocket connection for job {}", self.job_id);
    }
}

impl WsConnection {
    pub fn new(job_manager: Addr<JobManager>, job: Option<Addr<JobRunner>>, job_id: JobId) -> Self {
        Self {
            job,
            job_manager,
            job_id,
        }
    }
    // fn query_job(&self, _job_id: JobId) {

    // }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
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
            Ok(ws::Message::Ping(_msg)) => {
                ctx.pong(&[]);
                log::info!("{} - Replying with \"Pong\"", &self.job_id);
            }
            Ok(ws::Message::Text(_text)) => {
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
            Ok(ws::Message::Binary(_bin)) => {
                log::info!("{} - Ignoring incoming binary message.", &self.job_id);
                //ctx.binary(bin)
            }
            _ => {
                // log::info!("{} - Ignoring incoming message.", &self.job_id);
            }
        }
    }
}
