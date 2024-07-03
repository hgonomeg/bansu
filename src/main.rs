use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
pub mod messages;
pub mod job;
use messages::*;
use lazy_static::lazy_static;

lazy_static! {
    static ref JOB_MANAGER: job::JobManager = job::JobManager::new();
}

#[get("/query_acedrg/{job_id}")]
async fn query_acedrg(path: web::Path<JobId>) -> impl Responder {
    let job_id = path.into_inner();
    HttpResponse::Ok().body("Hello world!")
}

#[post("/spawn_acedrg")]
async fn spawn_acedrg(args: web::Json<AcedrgArgs>) -> impl Responder {
    HttpResponse::Ok()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(query_acedrg)
            .service(spawn_acedrg)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}