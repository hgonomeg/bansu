use actix::prelude::*;
use actix_web_actors::ws;
use crate::job::{job_runner::{AddRecipient, JobRunner}, JobData};


pub struct WsConnection {
    job: Addr<JobRunner>
}

impl Handler<JobData> for WsConnection {
    type Result = <JobData as actix::Message>::Result;

    fn handle(&mut self, msg: JobData, ctx: &mut Self::Context) -> Self::Result {
        // todo: send info on websocket
    }
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Fuck!!! They have no .recipient() here
        //self.job.send(AddRecipient(ctx.address()));
    }
}

impl WsConnection {
    pub fn new(job: Addr<JobRunner>) -> Self {
        Self {
            job
        }
    }
}

/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(_msg)) => {
                ctx.pong(&[]);
            }
            Ok(ws::Message::Text(_text)) => {
                //ctx.text(text)
            }
            Ok(ws::Message::Binary(_bin)) => {
                //ctx.binary(bin)
            }
            _ => (),
        }
    }
}
