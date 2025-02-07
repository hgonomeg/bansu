use crate::job::{JobFailureReason, JobOutput, JobStatus};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub type JobId = String;

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Copy, Debug)]
pub enum JobStatusInfo {
    Pending,
    Finished,
    Failed,
    Queued,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Copy, Debug)]
pub enum JobFailureInfo {
    TimedOut,
    JobProcessError,
    SetupError,
}

impl From<&JobFailureReason> for JobFailureInfo {
    fn from(value: &JobFailureReason) -> Self {
        match value {
            JobFailureReason::TimedOut => JobFailureInfo::TimedOut,
            JobFailureReason::SetupError(_) => JobFailureInfo::SetupError,
            JobFailureReason::JobProcessError => JobFailureInfo::JobProcessError,
        }
    }
}

impl From<JobStatus> for JobStatusInfo {
    fn from(value: JobStatus) -> Self {
        match value {
            JobStatus::Pending => JobStatusInfo::Pending,
            JobStatus::Finished => JobStatusInfo::Finished,
            JobStatus::Failed(_) => JobStatusInfo::Failed,
            JobStatus::Queued => JobStatusInfo::Queued,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WsJobDataUpdate {
    pub status: JobStatusInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_output: Option<JobOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<JobFailureInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl WsJobDataUpdate {
    pub fn new_from_queue_pos(queue_pos: usize) -> Self {
        Self {
            status: JobStatusInfo::Queued,
            job_output: None,
            failure_reason: None,
            queue_position: Some(queue_pos),
            error_message: None,
        }
    }
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
            error_message: match &value.status {
                JobStatus::Failed(f) => match f {
                    JobFailureReason::TimedOut => None,
                    JobFailureReason::SetupError(e) => Some(e.to_owned()),
                    JobFailureReason::JobProcessError => None,
                },
                _ => None,
            },
            status: JobStatusInfo::from(value.status),
            job_output: value.job_output,
            queue_position: None,
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

#[derive(Deserialize, Serialize, Clone, Debug, ToSchema)]
pub struct JobSpawnReply {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<JobId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_position: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct AcedrgArgs {
    pub smiles: String,
    pub commandline_args: Vec<String>,
}
