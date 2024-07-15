use serde::{Deserialize, Serialize};

use crate::job::JobStatus;

pub type JobId = String;

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
pub enum JobStatusInfo {
    Pending,
    Finished,
    Failed,
}

impl From<JobStatus> for JobStatusInfo {
    fn from(value: JobStatus) -> Self {
        match value {
            JobStatus::Pending => JobStatusInfo::Pending,
            JobStatus::Finished => JobStatusInfo::Finished,
            JobStatus::Failed(_) => JobStatusInfo::Failed,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ClientMessageKind {
    QueryJob,
    SpawnAcedrg,
    //GetCIF
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientMessage {
    pub kind: ClientMessageKind,
    /// for querying only
    pub job_id: Option<JobId>,
    pub acedrg_data: Option<AcedrgArgs>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GenericErrorMessage {
    pub error_message: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct AcedrgSpawnReply {
    pub job_id: Option<JobId>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AcedrgArgs {
    pub smiles: String,
    pub commandline_args: Vec<String>,
}
