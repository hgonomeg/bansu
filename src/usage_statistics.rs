use super::usage_statistics_entity::{jobs, requests};
use crate::{messages::JobId, state::State};
use chrono::{DateTime, Local};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use std::net::IpAddr;

pub fn ip_addr_to_blob(ip: &IpAddr) -> Vec<u8> {
    match ip {
        std::net::IpAddr::V4(ipv4) => ipv4.octets().to_vec(),
        std::net::IpAddr::V6(ipv6) => ipv6.octets().to_vec(),
    }
}

pub fn blob_to_ip_addr(blob: &[u8]) -> Option<IpAddr> {
    match blob.len() {
        4 => {
            let octets: [u8; 4] = blob.try_into().ok()?;
            Some(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)))
        }
        16 => {
            let octets: [u8; 16] = blob.try_into().ok()?;
            Some(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)))
        }
        _ => None,
    }
}

/// Used for writing statistics for freshly created jobs
pub struct FreshJobCommiter {
    connection: DatabaseConnection,
    init_time: DateTime<Local>,
    ip_address: IpAddr,
}

impl FreshJobCommiter {
    pub fn with_state_and_request(
        state: &State,
        http_req: &actix_web::HttpRequest,
    ) -> Option<Self> {
        state.on_usage_stats_db(|db| {
            let ip = http_req
                .connection_info()
                .realip_remote_addr()
                .and_then(|s| s.parse().ok())
                .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
            FreshJobCommiter::new(db.clone(), ip)
        })
    }
    pub fn new(connection: DatabaseConnection, ip_address: IpAddr) -> Self {
        Self {
            connection,
            init_time: Local::now(),
            ip_address,
        }
    }

    pub async fn commit_fresh_job(self, job_id: Option<JobId>, error_message_opt: Option<String>) {
        // It only makes sense to store processing time if we know right away that the job failed to start at this point.
        // If the job started successfully, we will update the processing time when finalizing the job statistics.
        let processing_time_opt = if job_id.is_none() {
            Some((Local::now() - self.init_time).num_seconds())
        } else {
            None
        };

        let job = jobs::ActiveModel {
            id: NotSet,
            successful: Set(Some(job_id.is_some() as i64)),
            job_id: Set(job_id),
            start_time: Set(self.init_time.naive_local().to_owned()),
            processing_time: Set(processing_time_opt),
            ip_address: Set(ip_addr_to_blob(&self.ip_address)),
            error_message: Set(error_message_opt),
        };

        match job.insert(&self.connection).await {
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to commit fresh job statistics: {}", e);
            }
        }
    }
}

pub async fn finalize_job_statistics(
    connection: DatabaseConnection,
    job_id: JobId,
    successful: bool,
    error_message_opt: Option<String>,
) {
    let Ok(mut db_entries) = jobs::Entity::find()
        .filter(jobs::Column::JobId.eq(job_id.to_string()))
        .all(&connection)
        .await
    else {
        log::error!("Failed to find job statistics entry for job ID {}", job_id);
        return;
    };
    if db_entries.is_empty() {
        log::error!("No job statistics entry found for job ID {}", job_id);
        return;
    } else if db_entries.len() > 1 {
        log::warn!(
            "Multiple job statistics entries found for job ID {}",
            job_id
        );
        // Retain only the youngest job from the list
        let Some(youngest_start_time) = db_entries
            .iter()
            .max_by_key(|entry| entry.start_time)
            .map(|e| e.start_time)
        else {
            log::error!(
                "Failed to determine youngest job statistics entry for job ID {}",
                job_id
            );
            return;
        };
        db_entries.retain(|entry| entry.start_time == youngest_start_time);
    }
    let db_entry = &db_entries[0];

    let processing_time = (Local::now().naive_local() - db_entry.start_time).num_seconds();
    let mut active_model: jobs::ActiveModel = db_entry.clone().into();
    active_model.processing_time = Set(Some(processing_time));
    active_model.successful = Set(Some(successful as i64));
    active_model.error_message = Set(error_message_opt);
    match active_model.update(&connection).await {
        Ok(_) => {}
        Err(e) => {
            log::error!(
                "Failed to update job statistics for job ID {}: {}",
                job_id,
                e
            );
        }
    }
}

/// Convenient struct for writing request statistics to the database
pub struct RequestStatCommiter {
    connection: DatabaseConnection,
    init_time: DateTime<Local>,
    route: String,
    ip_address: IpAddr,
}

impl RequestStatCommiter {
    pub fn new(connection: DatabaseConnection, route: &str, ip_address: IpAddr) -> Self {
        Self {
            connection,
            init_time: Local::now(),
            route: route.to_string(),
            ip_address,
        }
    }
    #[inline]
    pub fn with_state_and_request(
        state: &State,
        http_req: &actix_web::HttpRequest,
    ) -> Option<Self> {
        state.on_usage_stats_db(|db| {
            let ip = http_req
                .connection_info()
                .realip_remote_addr()
                .and_then(|s| s.parse().ok())
                .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
            RequestStatCommiter::new(db.clone(), http_req.path(), ip)
        })
    }

    pub async fn commit_successful(self, job_queue_len: i64, num_of_jobs_running: i64) {
        self.commit(job_queue_len, num_of_jobs_running, true, None)
            .await;
    }

    pub async fn commit_failed(
        self,
        job_queue_len: i64,
        num_of_jobs_running: i64,
        error_message_opt: Option<String>,
    ) {
        self.commit(job_queue_len, num_of_jobs_running, false, error_message_opt)
            .await;
    }

    async fn commit(
        &self,
        job_queue_len: i64,
        num_of_jobs_running: i64,
        successful: bool,
        error_message_opt: Option<String>,
    ) {
        let time_to_process = (Local::now() - self.init_time)
            .num_microseconds()
            .unwrap_or_default();

        let request = requests::ActiveModel {
            id: NotSet,
            api_route: Set(self.route.to_string()),
            successful: Set(successful as i64),
            ip_address: Set(ip_addr_to_blob(&self.ip_address)),
            time_sent: Set(self.init_time.naive_local().to_owned()),
            time_to_process: Set(time_to_process),
            job_queue_len: Set(job_queue_len),
            num_of_jobs_running: Set(num_of_jobs_running),
            error_message: Set(error_message_opt),
        };

        match request.insert(&self.connection).await {
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to commit request statistics: {}", e);
            }
        }
    }
}

#[allow(async_fn_in_trait)]
pub trait RequestStatCommiterConsumer {
    async fn commit_successful(self, jm_addr: &actix::Addr<crate::job::JobManager>);
    async fn commit_failed(
        self,
        jm_addr: &actix::Addr<crate::job::JobManager>,
        error_message_opt: Option<String>,
    );
}

impl RequestStatCommiterConsumer for Option<RequestStatCommiter> {
    async fn commit_successful(self, jm_addr: &actix::Addr<crate::job::JobManager>) {
        if let Some(commiter) = self {
            // It should be safe to unwrap here because JobManagerVibeCheck does not fail
            let jmvcr = jm_addr.send(crate::job::JobManagerVibeCheck).await.unwrap();
            commiter
                .commit_successful(
                    jmvcr.queue_length.unwrap_or(0) as i64,
                    jmvcr.active_jobs as i64,
                )
                .await;
        }
    }

    async fn commit_failed(
        self,
        jm_addr: &actix::Addr<crate::job::JobManager>,
        error_message_opt: Option<String>,
    ) {
        if let Some(commiter) = self {
            // It should be safe to unwrap here because JobManagerVibeCheck does not fail
            let jmvcr = jm_addr.send(crate::job::JobManagerVibeCheck).await.unwrap();
            commiter
                .commit_failed(
                    jmvcr.queue_length.unwrap_or(0) as i64,
                    jmvcr.active_jobs as i64,
                    error_message_opt,
                )
                .await;
        }
    }
}
