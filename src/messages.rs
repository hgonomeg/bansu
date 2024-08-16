use crate::job::{JobFailureReason, JobOutput, JobStatus};
use serde::{Deserialize, Serialize};

pub type JobId = String;

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Copy, Debug)]
pub enum JobStatusInfo {
    Pending,
    Finished,
    Failed,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Copy, Debug)]
pub enum JobFailureInfo {
    TimedOut,
    AcedrgError,
    IOError,
}

impl From<&JobFailureReason> for JobFailureInfo {
    fn from(value: &JobFailureReason) -> Self {
        match value {
            JobFailureReason::TimedOut => JobFailureInfo::TimedOut,
            JobFailureReason::IOError(_) => JobFailureInfo::IOError,
            JobFailureReason::AcedrgError => JobFailureInfo::AcedrgError,
        }
    }
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
pub struct WsJobDataUpdate {
    pub status: JobStatusInfo,
    pub job_output: Option<JobOutput>,
    pub failure_reason: Option<JobFailureInfo>,
}

impl From<crate::job::JobData> for WsJobDataUpdate {
    fn from(value: crate::job::JobData) -> Self {
        Self {
            failure_reason: {
                match &value.status {
                    JobStatus::Failed(f) => Some(JobFailureInfo::from(f)),
                    _ => None,
                }
            },
            status: JobStatusInfo::from(value.status),
            job_output: value.job_output,
        }
    }
}

// #[derive(Clone, Debug, Deserialize, Serialize)]
// pub enum WsClientMessageKind {
//     QueryJob,
//     // This is a bad idea
//     //SpawnAcedrg,

//     //GetCIF
// }

// #[derive(Clone, Debug, Deserialize, Serialize)]
// pub struct WsClientMessage {
//     pub kind: WsClientMessageKind,
//     /// for querying only
//     pub job_id: Option<JobId>,
// }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GenericErrorMessage {
    pub error_message: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct JobSpawnReply {
    pub job_id: Option<JobId>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AcedrgArgs {
    pub smiles: String,
    pub commandline_args: Vec<String>,
}
