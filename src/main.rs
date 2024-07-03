use actix_web::{get, http::StatusCode, post, web, App, HttpResponse, HttpServer};
pub mod job;
pub mod messages;
pub mod utils;
use job::JobFailureReason;
use messages::*;

#[get("/query_acedrg/{job_id}")]
async fn query_acedrg(path: web::Path<JobId>) -> HttpResponse {
    let job_id = path.into_inner();
    let jm_lock = job::JobManager::acquire_lock().await;
    let job_data_opt = jm_lock.query_job(&job_id).map(|x| x.clone());
    drop(jm_lock);
    let Some(job_data_arc) = job_data_opt else {
        return HttpResponse::NotFound().finish();
    };
    let job_data = job_data_arc.lock().await;
    let mut error_message = None;
    let mut http_status_code = StatusCode::OK;
    if let Some(failure_reason) = job_data.failure_reason.as_ref() {
        match failure_reason {
            JobFailureReason::TimedOut => {
                error_message = Some("Timed-out".to_string());
                http_status_code = StatusCode::FAILED_DEPENDENCY;
            }
            JobFailureReason::IOError(e) => {
                error_message = Some(format!("IO Error: {}", e));
                http_status_code = StatusCode::INTERNAL_SERVER_ERROR;
            }
            JobFailureReason::AcedrgError =>{
                let job_output = job_data.job_output.as_ref().unwrap();
                error_message = Some(format!("Acedrg Error:\nSTDERR:\n{}\nSTDOUT:\n{}", job_output.stderr, job_output.stdout));
                http_status_code = StatusCode::FAILED_DEPENDENCY;
            },
        }
    }
    let r_json = AcedrgQueryReply {
        error_message,
        status: job_data.status,
        // todo
        cif_data: None,
    };
    HttpResponse::build(http_status_code)
        .json(r_json)
}

#[post("/spawn_acedrg")]
async fn spawn_acedrg(args: web::Json<AcedrgArgs>) -> HttpResponse {
    let args = args.into_inner();
    // todo: sanitize input in create_job()!!!
    match job::JobManager::create_job(&args).await {
        Ok(new_job) => {
            let mut jm_lock = job::JobManager::acquire_lock().await;
            let job_id = jm_lock.add_job(new_job);
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
    HttpServer::new(|| App::new().service(query_acedrg).service(spawn_acedrg))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
