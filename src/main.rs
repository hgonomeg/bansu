use actix::prelude::*;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{
    get,
    middleware::Condition,
    /*http::StatusCode*/ post,
    web::{self, Data},
    App, HttpRequest, HttpResponse, HttpServer,
};
use actix_ws::handle as ws_handle;
use std::{env, sync::Arc};
use utoipa::OpenApi;
pub mod job;
use anyhow::Context;
use job::{
    job_runner::{OutputFileRequest, OutputKind, OutputRequestError},
    job_type::{acedrg::AcedrgJob, JobSpawnError},
    JobEntry, JobManager, LookupJob, NewJob,
};
pub mod messages;
pub mod utils;
pub mod ws_connection;
use messages::*;
use tokio::io::AsyncReadExt;
use ws_connection::WsConnection;
// use log::{info,warn,error,debug};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Bansu",
        description = "Server-side computation API for Moorhen"
    ),
    paths(get_cif, run_acedrg, job_ws)
)]
struct ApiDoc;

#[utoipa::path(
    responses(
        // todo: fix this
        (status = 200, description = "Ok")
    ),
    params(
        ("job_id", description = "Job ID")
    )
)]
#[get("/get_cif/{job_id}")]
async fn get_cif(path: web::Path<JobId>, job_manager: web::Data<Addr<JobManager>>) -> HttpResponse {
    let job_id = path.into_inner();

    let Some(job_entry) = job_manager.send(LookupJob(job_id.clone())).await.unwrap() else {
        log::error!("/get_cif/{} - Job not found", job_id);
        return HttpResponse::NotFound().finish();
    };

    let JobEntry::Spawned(job) = job_entry else {
        log::error!("/get_cif/{} - Job is still queued.", job_id);
        return HttpResponse::BadRequest().finish();
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
            actix_rt::spawn(async move {
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

#[utoipa::path(
    responses(
        // todo: fix this
        (status = 200, description = "Ok")
    ),
    params(
        ("job_id", description = "Job ID")
    )
)]
#[get("/ws/{job_id}")]
async fn job_ws(
    path: web::Path<JobId>,
    req: HttpRequest,
    payload: web::Payload,
    job_manager: web::Data<Addr<JobManager>>,
) -> Result<HttpResponse, actix_web::Error> {
    let job_id = path.into_inner();
    let Some(job_entry) = job_manager
        .send(LookupJob(job_id.clone()))
        .await
        .ok()
        .flatten()
    else {
        log::error!("/ws/{} - Job not found", job_id);
        return Ok(HttpResponse::NotFound().finish());
    };
    let jm = job_manager.get_ref().clone();
    let job_opt = match job_entry {
        JobEntry::Spawned(job) => Some(job),
        JobEntry::Queued(_queue_pos) => None,
    };
    let (response, session, msg_stream) = ws_handle(&req, payload)?;
    WsConnection::new(jm, job_opt, job_id, session, msg_stream);
    Ok(response)
}

#[utoipa::path(
    responses(
        // todo: fix this
        (status = 200, description = "Ok", body = JobSpawnReply)
    ),
    // todo: payload
    // params(
    //     ("job_id", description = "Job ID")
    // )
)]
#[post("/run_acedrg")]
async fn run_acedrg(
    args: web::Json<AcedrgArgs>,
    job_manager: web::Data<Addr<JobManager>>,
) -> HttpResponse {
    let args = args.into_inner();
    let jo = Arc::from(AcedrgJob { args });

    match job_manager.send(NewJob(jo)).await.unwrap() {
        Ok(resp) => match resp.entry {
            JobEntry::Spawned(_job) => HttpResponse::Created().json(JobSpawnReply {
                job_id: Some(resp.id),
                error_message: None,
                queue_position: None,
            }),
            JobEntry::Queued(queue_pos) => HttpResponse::Accepted().json(JobSpawnReply {
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
            .unwrap_or(45);

        let sec_per_rq = env::var("BANSU_RATELIMIT_SECONDS_PER_REQUEST")
            .ok()
            .map(|port_str| port_str.parse::<u64>())
            .transpose()
            .with_context(|| "Could not parse rate-limiter seconds per request")?
            .unwrap_or(10);

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

    async fn apidoc_json(api: Data<utoipa::openapi::OpenApi>) -> HttpResponse {
        HttpResponse::Ok().json(api)
    }

    async fn apidoc_yaml(api: Data<utoipa::openapi::OpenApi>) -> HttpResponse {
        match api.as_ref().to_yaml() {
            Ok(yaml) => HttpResponse::Ok().body(yaml),
            Err(e) => {
                let err_msg = format!("{}", e);
                log::error!("/api-docs/openapi.yaml - {}", err_msg);
                HttpResponse::InternalServerError().body(format!("{}", err_msg))
            }
        }
    }

    fn configure_base_url(cfg: &mut actix_web::web::ServiceConfig) {
        fn configure_content(
            cfg: &mut actix_web::web::ServiceConfig,
            apidoc: utoipa::openapi::OpenApi,
        ) {
            cfg.service(run_acedrg)
                .service(get_cif)
                .service(job_ws)
                .service(
                    // Do we want/need this scope?
                    web::scope("/api-docs")
                        .app_data(Data::new(apidoc.clone()))
                        .route("/openapi.json", web::get().to(apidoc_json))
                        .route("/openapi.yaml", web::get().to(apidoc_yaml)),
                );
        }

        let apidoc = ApiDoc::openapi();
        match env::var("BANSU_BASE_URL") {
            Ok(base_url) => {
                // log::info!("Enabling base url: {}", &base_url);
                cfg.service(web::scope(&base_url).configure(|cfg| configure_content(cfg, apidoc)));
            }
            Err(_) => {
                cfg.configure(|cfg| configure_content(cfg, apidoc));
            }
        }
    }

    log::info!("Initializing HTTP server...");
    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Condition::new(
                env::var("BANSU_DISABLE_RATELIMIT").is_err(),
                Governor::new(&governor_conf),
            ))
            .app_data(Data::new(job_manager.clone()))
            .configure(configure_base_url)
    })
    .bind((addr, port))?
    .run()
    .await?)
}
