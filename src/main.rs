use std::env;

use actix::prelude::*;
use actix_web::{
    get, /*http::StatusCode*/ post,
    web::{self, Data},
    App, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
pub mod job;
use job::{
    job_runner::{OutputFileRequest, OutputKind, OutputRequestError},
    job_type::acedrg::AcedrgJob,
    JobManager, LookupJob, NewJob,
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

    let Some(job) = job_manager.send(LookupJob(job_id.clone())).await.unwrap() else {
        log::error!("/get_cif/{} - Job not found", job_id);
        return HttpResponse::NotFound().finish();
    };

    let file_res = job
        .send(OutputFileRequest {
            kind: OutputKind::CIF,
        })
        .await
        .unwrap();

    match file_res {
        Err(OutputRequestError::IOError(e)) => {
            log::error!("/get_cif/{} - Could not open output - {}", job_id, &e);
            HttpResponse::InternalServerError().body(e.to_string())
        }
        Err(OutputRequestError::JobStillPending) => {
            log::warn!("/get_cif/{} - Job is still pending.", job_id);
            HttpResponse::BadRequest().finish()
        }
        Err(OutputRequestError::OutputKindNotSupported) => {
            log::error!("/get_cif/{} - This job does not support CIF output", job_id);
            HttpResponse::BadRequest().finish()
        }
        Ok(mut file) => {
            let (tx, rx) = tokio::sync::mpsc::channel::<Result<web::Bytes, std::io::Error>>(64);
            tokio::task::spawn(async move {
                log::info!("/get_cif/{} - Replying with CIF file", job_id);
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
    let Some(job) = job_manager
        .send(LookupJob(job_id.clone()))
        .await
        .ok()
        .flatten()
    else {
        log::error!("/ws/{} - Job not found", job_id);
        return Ok(HttpResponse::NotFound().finish());
    };
    let jm = job_manager.get_ref().clone();
    // log::info!("/ws/{} - Establishing ws connection", &job_id);
    ws::start(WsConnection::new(jm, job, job_id), &req, stream)
}

#[post("/run_acedrg")]
async fn run_acedrg(
    args: web::Json<AcedrgArgs>,
    job_manager: web::Data<Addr<JobManager>>,
) -> HttpResponse {
    let args = args.into_inner();
    let jo = Box::from(AcedrgJob { args });

    match job_manager.send(NewJob(jo)).await.unwrap() {
        Ok((job_id, _new_job)) => HttpResponse::Created().json(JobSpawnReply {
            job_id: Some(job_id),
            error_message: None,
        }),
        Err(e) => {
            log::error!("/run_acedrg - Could not create job: {:#}", &e);
            // todo: different error types?
            HttpResponse::InternalServerError().json(JobSpawnReply {
                job_id: None,
                error_message: Some(format!("{:#}", e)),
            })
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    eprintln!(
        "Bansu Server, v{} \n\nAuthors: {}\nLicense: {}\nCopyright (C) 2024, Global Phasing Ltd.",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_AUTHORS"),
        env!("CARGO_PKG_LICENSE")
    );

    simple_logger::SimpleLogger::new().env().init().unwrap();

    let addr = env::var("BANSU_ADDRESS").unwrap_or("127.0.0.1".to_string());
    let port: u16 = env::var("BANSU_PORT")
        .ok()
        .and_then(|port_str| port_str.parse::<u16>().ok())
        .unwrap_or(8080);

    if let Ok(docker_image_name) = env::var("BANSU_DOCKER") {
        log::info!("Testing Docker configuration...");
        if let Err(e) = job::docker::test_docker(&docker_image_name).await {
            log::error!("Docker test failed - {:#}. Disabling Docker support.", e);
            env::remove_var("BANSU_DOCKER");
        } else {
            log::info!("Starting with Docker support.");
        }
    } else {
        log::info!("Starting without Docker support.");
    }

    let job_manager = JobManager::new().start();
    log::info!("Initializing HTTP server...");
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(job_manager.clone()))
            .service(run_acedrg)
            .service(get_cif)
            .service(job_ws)
    })
    .bind((addr, port))?
    .run()
    .await
}
