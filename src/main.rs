use actix_web::{get, post, web, App, HttpResponse, HttpServer};
pub mod job;
pub mod messages;
pub mod utils;
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
    let r = AcedrgQueryReply {
        // todo
        error_message: None,
        status: job_data.status,
        // todo
        cif_data: None,
    };
    HttpResponse::Ok().json(r)
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
            HttpResponse::BadRequest().json(AcedrgSpawnReply {
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
