use std::env;

use actix::prelude::*;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{
    get,
    middleware::Condition,
    /*http::StatusCode*/ post,
    web::{self, Data},
    App, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
pub mod job;
use anyhow::Context;
use job::{
    job_runner::{OutputFileRequest, OutputKind, OutputRequestError},
    job_type::{acedrg::AcedrgJob, JobSpawnError},
    JobManager, LookupJob, NewJob, NewJobInfo,
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
        Ok(resp) => match resp.info {
            NewJobInfo::Spawned(_job) => HttpResponse::Created().json(JobSpawnReply {
                job_id: Some(resp.id),
                error_message: None,
                queue_position: None,
            }),
            NewJobInfo::Queued(queue_pos) => HttpResponse::Accepted().json(JobSpawnReply {
                job_id: Some(resp.id),
                error_message: None,
                queue_position: Some(queue_pos),
            }),
        },
        Err(JobSpawnError::InputValidation(e)) => {
            log::warn!("/run_acedrg - Could not create job: {:#}", &e);
            HttpResponse::BadRequest().json(JobSpawnReply {
                job_id: None,
                error_message: Some(format!("{:#}", e)),
                queue_position: None,
            })
        }
        Err(JobSpawnError::TooManyJobs) => {
            log::warn!("/run_acedrg - Could not create job: Too many jobs");
            HttpResponse::ServiceUnavailable().json(JobSpawnReply {
                job_id: None,
                error_message: Some("Server is at capacity. Please try again later.".to_string()),
                queue_position: None,
            })
        }
        Err(JobSpawnError::Other(e)) => {
            log::error!("/run_acedrg - Could not create job: {:#}", &e);
            HttpResponse::InternalServerError().json(JobSpawnReply {
                job_id: None,
                error_message: Some(format!("{:#}", e)),
                queue_position: None,
            })
        }
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
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
        .map(|port_str| port_str.parse::<u16>())
        .transpose()
        .with_context(|| "Could not parse port number")?
        .unwrap_or(8080);

    if let Ok(docker_image_name) = env::var("BANSU_DOCKER") {
        log::info!("Testing Docker configuration...");
        if let Err(e) = utils::test_docker(&docker_image_name).await {
            log::error!("Docker test failed - {:#}. Disabling Docker support.", e);
            env::remove_var("BANSU_DOCKER");
        } else {
            log::info!("Docker test successful.");
        }
    } else {
        log::info!("Docker configuration was not provided.");
    }

    if env::var("BANSU_DOCKER").is_err() {
        if env::var("BANSU_DISALLOW_DOCKERLESS").is_ok() {
            let e = anyhow::anyhow!("No (valid) Docker configuration was provided and BANSU_DISALLOW_DOCKERLESS is set. Refusing to continue.");
            log::error!("{}", &e);
            return Err(e);
        }
        log::info!("Testing environment configuration...");
        if let Err(e) = utils::test_dockerless().await {
            log::error!("Environment test failed: {} Refusing to continue without usable 'acedrg' and 'servalcat'.", &e);
            return Err(e);
        }
    }

    if env::var("BANSU_DOCKER").is_ok() {
        log::info!("Starting with Docker support.");
    } else {
        log::info!("Starting without Docker support.");
    }

    let max_queue_length = env::var("BANSU_MAX_JOB_QUEUE_LENGTH")
        .ok()
        .map(|port_str| port_str.parse::<usize>())
        .transpose()
        .with_context(|| "Could not parse max job queue length number")?
        .map(|raw_num| if raw_num == 0 { None } else { Some(raw_num) })
        .unwrap_or(Some(20));

    let max_concurrent_jobs = env::var("BANSU_MAX_CONCURRENT_JOBS")
        .ok()
        .map(|port_str| port_str.parse::<usize>())
        .transpose()
        .with_context(|| "Could not parse max concurrent job number")?
        .map(|raw_num| if raw_num == 0 { None } else { Some(raw_num) })
        .unwrap_or(Some(20));

    log::info!(
        "Max concurrent job limit: {}. Max job queue length: {}.",
        match max_concurrent_jobs.as_ref() {
            Some(v) => v.to_string(),
            None => "No limit".to_string(),
        },
        match max_queue_length.as_ref() {
            Some(v) => v.to_string(),
            None => "No limit".to_string(),
        }
    );

    let job_manager = JobManager::new(max_concurrent_jobs, max_queue_length).start();

    let governor_conf = if env::var("BANSU_DISABLE_RATELIMIT").is_err() {
        let burst_size = env::var("BANSU_RATELIMIT_BURST_SIZE")
            .ok()
            .map(|port_str| port_str.parse::<u32>())
            .transpose()
            .with_context(|| "Could not parse rate-limiter burst size")?
            .unwrap_or(10);

        let sec_per_rq = env::var("BANSU_RATELIMIT_SECONDS_PER_REQUEST")
            .ok()
            .map(|port_str| port_str.parse::<u64>())
            .transpose()
            .with_context(|| "Could not parse rate-limiter seconds per request")?
            .unwrap_or(15);

        log::info!(
            "Rate-limiter configuration: burst_size={} seconds_per_request={}",
            burst_size,
            sec_per_rq
        );

        GovernorConfigBuilder::default()
            .seconds_per_request(sec_per_rq)
            .burst_size(burst_size)
            .finish()
            .with_context(|| "Invalid rate limiter configuration")?
    } else {
        log::info!("Rate-limiter disabled");
        GovernorConfigBuilder::default()
            // just in case
            .permissive(true)
            .finish()
            .unwrap()
    };

    log::info!("Initializing HTTP server...");
    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Condition::new(
                env::var("BANSU_DISABLE_RATELIMIT").is_err(),
                Governor::new(&governor_conf),
            ))
            .app_data(Data::new(job_manager.clone()))
            .service(run_acedrg)
            .service(get_cif)
            .service(job_ws)
    })
    .bind((addr, port))?
    .run()
    .await?)
}
