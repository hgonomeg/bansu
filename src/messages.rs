use serde::{Serialize, Deserialize};


pub type JobId = String;

#[derive(Deserialize, Serialize)]
pub struct AcedrgArgs {
    smiles: String,
    commandline_args: Vec<String>
}

#[derive(Deserialize, Serialize)]
pub enum JobStatusInfo {
    Pending,
    Finished,
    Failed
}

#[derive(Deserialize, Serialize)]
pub struct AcedrgQueryReply {
    status: JobStatusInfo,
    error_message: Option<String>,
    /// Base64-encoded data
    cif_data: Option<String>
}


#[derive(Deserialize, Serialize)]
pub struct AcedrgSpawnReply {
    job_id: JobId,
    error_message: Option<String>
}