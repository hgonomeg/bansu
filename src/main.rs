use actix::prelude::*;
use actix_web::{
    get, /*http::StatusCode*/ post,
    web::{self, Data},
    App, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
pub mod job;
use job::{
    job_runner::{JobRunner, OutputKind, OutputPathRequest},
    AddJob, JobManager, QueryJob,
};
pub mod messages;
pub mod utils;
pub mod ws_connection;
use messages::*;
use tokio::io::AsyncReadExt;
use ws_connection::WsConnection;
// use log::{info,warn,error,debug};

#[get("/get_cif/{job_id}")]
async fn get_cif(path: web::Path<JobId>, job_manager: web::Data<Addr<JobManager>>) -> HttpResponse {
    let job_id = path.into_inner();

    let Some(job) = job_manager.send(QueryJob(job_id)).await.unwrap() else {
        return HttpResponse::NotFound().finish();
    };

    let file_res = job
        .send(OutputPathRequest {
            kind: OutputKind::CIF,
        })
        .await
        .unwrap();

    match file_res {
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
        Ok(mut file) => {
            let (tx, rx) = tokio::sync::mpsc::channel::<Result<web::Bytes, std::io::Error>>(64);
            tokio::task::spawn(async move {
                loop {
                    let mut buf = web::BytesMut::with_capacity(65536);
                    let read_res = file.read_buf(&mut buf).await;
                    match read_res {
                        Ok(n) => {
                            if n == 0 {
                                // end of file
                                break;
                            } else {
                                let _ = tx.send(Ok(buf.into())).await;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e)).await;
                            break;
                        }
                    }
                }
            });

            HttpResponse::Ok().streaming(tokio_stream::wrappers::ReceiverStream::new(rx))
        }
    }
}

#[get("/ws/{job_id}")]
async fn job_ws(
    path: web::Path<JobId>,
    req: HttpRequest,
    stream: web::Payload,
    job_manager: web::Data<Addr<JobManager>>,
) -> Result<HttpResponse, actix_web::Error> {
    let job_id = path.into_inner();
    let Some(job) = job_manager.send(QueryJob(job_id)).await.ok().flatten() else {
        return Ok(HttpResponse::NotFound().finish());
    };
    ws::start(WsConnection::new(job), &req, stream)
}

#[post("/run_acedrg")]
async fn run_acedrg(
    args: web::Json<AcedrgArgs>,
    job_manager: web::Data<Addr<JobManager>>,
) -> HttpResponse {
    let args = args.into_inner();
    // todo: sanitize input in create_job()!!!
    match JobRunner::create_job(vec![], &args).await {
        Ok(new_job) => {
            let job_id = job_manager.send(AddJob(new_job)).await.unwrap();
            HttpResponse::Created().json(AcedrgSpawnReply {
                job_id: Some(job_id),
                error_message: None,
            })
        }
        Err(e) => {
            // todo: different error types?
            HttpResponse::InternalServerError().json(AcedrgSpawnReply {
                job_id: None,
                error_message: Some(e.to_string()),
            })
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    HttpServer::new(|| {
        App::new()
            .app_data(Data::new(JobManager::new().start()))
            .service(run_acedrg)
            .service(get_cif)
            .service(job_ws)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
