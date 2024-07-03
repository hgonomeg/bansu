use serde::{Deserialize, Serialize};

pub type JobId = String;

#[derive(Deserialize, Serialize)]
pub struct AcedrgArgs {
    pub smiles: String,
    pub commandline_args: Vec<String>,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
pub enum JobStatusInfo {
    Pending,
    Finished,
    Failed,
}

#[derive(Deserialize, Serialize)]
pub struct AcedrgQueryReply {
    pub status: JobStatusInfo,
    pub error_message: Option<String>,
    /// Base64-encoded data
    pub cif_data: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct AcedrgSpawnReply {
    pub job_id: Option<JobId>,
    pub error_message: Option<String>,
}
