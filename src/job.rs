use super::messages::{JobId, AcedrgArgs};
use std::collections::BTreeMap;

pub struct JobData {

}

pub struct JobManager {
    jobs: BTreeMap<JobId, JobData>

}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: BTreeMap::new()
        }
    }
    pub async fn create_job(args: &AcedrgArgs) -> anyhow::Result<()> {
        Ok(())
    }
}