use crate::{
    job::{
        job_runner::{AddWebSocketAddr, JobRunner, QueryJobData},
        JobData, JobManager, JobStatus,
    },
    messages::*,
};
use actix::prelude::*;
use actix_web_actors::ws::{self, CloseCode, CloseReason};

pub struct WsConnection {
    _job_manager: Addr<JobManager>,
    job: Addr<JobRunner>,
    job_id: JobId,
}

impl Handler<JobData> for WsConnection {
    type Result = <JobData as actix::Message>::Result;

    fn handle(&mut self, msg: JobData, ctx: &mut Self::Context) -> Self::Result {
        log::info!("Sending JobDataUpdate for job {}", self.job_id);
        ctx.text(serde_json::to_string(&WsJobDataUpdate::from(msg.clone())).unwrap());
        if msg.status == JobStatus::Finished {
            ctx.close(Some(CloseReason {
                code: CloseCode::Normal,
                description: None,
            }));
        }
    }
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.job.do_send(AddWebSocketAddr(ctx.address()));
        log::info!("Initializing WebSocket connection for job {}", self.job_id);
        let job = self.job.clone();
        ctx.spawn(
            async move {
                let data = job.send(QueryJobData).await.unwrap();
                data
            }
            .into_actor(self)
            .map(|data, _a, ctx| {
                ctx.notify(data);
            }),
        );
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        log::info!("Closed WebSocket connection for job {}", self.job_id);
    }
}

impl WsConnection {
    pub fn new(job_manager: Addr<JobManager>, job: Addr<JobRunner>, job_id: JobId) -> Self {
        Self {
            job,
            _job_manager: job_manager,
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
            }
            Ok(ws::Message::Text(_text)) => {
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
                //ctx.binary(bin)
            }
            _ => (),
        }
    }
}
