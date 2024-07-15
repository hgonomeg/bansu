use crate::{
    job::{job_runner::JobRunner, JobData, JobManager},
    messages::*,
};
use actix::prelude::*;
use actix_web_actors::ws;

pub struct WsConnection {
    job_manager: Addr<JobManager>,
    job: Addr<JobRunner>,
}

impl Handler<JobData> for WsConnection {
    type Result = <JobData as actix::Message>::Result;

    fn handle(&mut self, _msg: JobData, _ctx: &mut Self::Context) -> Self::Result {
        // todo: send info on websocket
    }
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

    // fn started(&mut self, ctx: &mut Self::Context) {

    // }
}

impl WsConnection {
    pub fn new(job_manager: Addr<JobManager>, job: Addr<JobRunner>) -> Self {
        Self { job_manager, job }
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
            Ok(ws::Message::Text(text)) => {
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
