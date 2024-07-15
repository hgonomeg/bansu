use crate::{
    job::{JobData, JobManager, NewJob},
    messages::*,
};
use actix::prelude::*;
use actix_web_actors::ws;
// use serde_json::{from_str};

pub struct WsConnection {
    job_manager: Addr<JobManager>,
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
    pub fn new(job_manager: Addr<JobManager>) -> Self {
        Self { job_manager }
    }
    fn spawn_acedrg(
        &self,
        ctx: &mut <WsConnection as actix::Actor>::Context,
        acedrg_data: AcedrgArgs,
    ) {
        let jm = self.job_manager.clone();
        ctx.spawn(
            async move {
                match jm.send(NewJob(acedrg_data)).await.unwrap() {
                    Ok((job_id, _new_job)) => {
                        let reply = AcedrgSpawnReply {
                            job_id: Some(job_id),
                            error_message: None,
                        };
                        reply
                    }
                    Err(e) => {
                        log::error!("spawn_acedrg - {}", &e);
                        let reply = AcedrgSpawnReply {
                            job_id: None,
                            error_message: Some(e.to_string()),
                        };
                        // todo: different error types?
                        // HttpResponse::InternalServerError().json()
                        reply
                    }
                }
            }
            .into_actor(self)
            .map(|reply, _actor, ctx| {
                ctx.text(serde_json::to_string(&reply).unwrap());
            }),
        );
    }
    fn query_job(&self, _job_id: JobId) {
        // todo
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let mut error_out = |error_msg: &str| {
            ctx.text(
                serde_json::to_string(&GenericErrorMessage {
                    error_message: Some(error_msg.to_owned()),
                })
                .unwrap(),
            );
            log::error!("{}", error_msg);
        };

        match msg {
            Ok(ws::Message::Ping(_msg)) => {
                ctx.pong(&[]);
            }
            Ok(ws::Message::Text(text)) => {
                let client_message = serde_json::from_str::<ClientMessage>(&text);
                match client_message {
                    Ok(client_message) => match client_message.kind {
                        ClientMessageKind::QueryJob => {
                            let Some(job_id) = client_message.job_id else {
                                error_out("Missing job_id in QueryJob request.");
                                return;
                            };
                            self.query_job(job_id);
                        }
                        ClientMessageKind::SpawnAcedrg => {
                            let Some(acedrg_data) = client_message.acedrg_data else {
                                error_out("Missing acedrg_data in SpawnAcedrg request.");
                                return;
                            };
                            self.spawn_acedrg(ctx, acedrg_data);
                        }
                    },
                    Err(_e) => {
                        error_out("Unrecognized request kind.");
                    }
                }
            }
            Ok(ws::Message::Binary(_bin)) => {
                //ctx.binary(bin)
            }
            _ => (),
        }
    }
}
