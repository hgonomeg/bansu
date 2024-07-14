use actix_web::{get, http::StatusCode, post, web, App, HttpResponse, HttpServer};
use actix_web_actors::ws;
pub mod job;
use job::job_runner::{JobRunner, OutputKind, OutputPathRequest};
pub mod messages;
pub mod utils;
use messages::*;
use tokio::io::AsyncReadExt;

#[get("/get_cif/{job_id}")]
async fn get_cif(path: web::Path<JobId>) -> HttpResponse {
    let job_id = path.into_inner();
    let jm_lock = job::JobManager::acquire_lock().await;
    let job_data_opt = jm_lock.query_job(&job_id).map(|x| x.clone());
    drop(jm_lock);
    let Some(job_data) = job_data_opt else {
        return HttpResponse::NotFound().finish();
    };
    let file_res = job_data
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

#[post("/run_acedrg")]
async fn run_acedrg(args: web::Json<AcedrgArgs>) -> HttpResponse {
    let args = args.into_inner();
    // todo: sanitize input in create_job()!!!
    match JobRunner::create_job(vec![], &args).await {
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
    HttpServer::new(|| App::new().service(run_acedrg).service(get_cif))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
