use super::usage_statistics_entity::{jobs, prelude::*, requests};
use chrono::{DateTime, Local};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    DatabaseConnection,
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

pub struct RequestStatCommiter<'a> {
    connection: &'a DatabaseConnection,
    init_time: DateTime<Local>,
    route: &'a str,
    ip_address: &'a IpAddr,
}

impl<'a> RequestStatCommiter<'a> {
    pub fn new(connection: &'a DatabaseConnection, route: &'a str, ip_address: &'a IpAddr) -> Self {
        Self {
            connection,
            init_time: Local::now(),
            route,
            ip_address,
        }
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
            ip_address: Set(ip_addr_to_blob(self.ip_address)),
            time_sent: Set(self.init_time.naive_local().to_owned()),
            time_to_process: Set(time_to_process),
            job_queue_len: Set(job_queue_len),
            num_of_jobs_running: Set(num_of_jobs_running),
            error_message: Set(error_message_opt),
        };

        match request.insert(self.connection).await {
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to commit request statistics: {}", e);
            }
        }
    }
}
