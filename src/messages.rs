use crate::{
    job::JobManagerVibeCheckReply,
    job::{JobFailureReason, JobOutput, JobStatus},
    state::State,
};
use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

pub type JobId = String;

#[derive(Deserialize, Serialize, Clone, Debug)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(example = json!({
    "bansu_version": "0.4.0",
    "queue_length": 12,
    "max_queue_length": 30,
    "active_jobs": 13,
    "max_concurrent_jobs": 10,
    "uptime": 986986
})))]
/// Response to a vibe check request
pub struct VibeCheckResponse {
    /// Bansu version
    pub bansu_version: String,
    /// Length of the queue or null if queue disabled
    pub queue_length: Option<usize>,
    /// Max length of the queue or null if queue disabled
    pub max_queue_length: Option<usize>,
    /// Number of jobs currently being processed (or still available for downloading job results)
    pub active_jobs: usize,
    /// Max number of jobs to be run in parallel
    pub max_concurrent_jobs: Option<usize>,
    /// Uptime in seconds
    pub uptime: u64,
}

impl VibeCheckResponse {
    pub fn build(jmvc: JobManagerVibeCheckReply, state: &State) -> Self {
        Self {
            bansu_version: state.version.to_owned(),
            queue_length: jmvc.queue_length,
            max_queue_length: jmvc.max_queue_length,
            active_jobs: jmvc.active_jobs,
            max_concurrent_jobs: state.max_concurrent_jobs,
            uptime: state.uptime(),
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Copy, Debug)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub enum JobStatusInfo {
    Pending,
    Finished,
    Failed,
    Queued,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Copy, Debug)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
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
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(example = json!({
    "status": "Pending | Finished | Failed | Queued",
    "queue_position": "number | null",
    "job_output": {
        "stdout": "A string",
        "stderr": "A string"
    },
    "error_message": "Some error message",
    "failure_reason": "TimedOut | JobProcessError | SetupError"
})))]
pub struct WsJobDataUpdate {
    /// Job status
    pub status: JobStatusInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Will be null if the job is still pending, if it timed-out or the child process failed due to an I/O error
    pub job_output: Option<JobOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Only not-null if the job failed
    pub failure_reason: Option<JobFailureInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Only not-null if the job is queued
    pub queue_position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Can be not-null only if the job failed. Currently, this only has value for `SetupError` failure reason
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

#[derive(Deserialize, Serialize, Clone, Debug)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(example = json!({
    "job_id": "UUID of your job",
    "error_message": "Error message if the request failed",
    "queue_position": 12
})))]
pub struct JobSpawnReply {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// UUID of the spawned job. Null on error
    pub job_id: Option<JobId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Null on success
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Position counted from 0. Null if the job is being processed without a queue.
    pub queue_position: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(
    description = "Contains either an input SMILES string or an input mmCIF file (base64-encoded) and an array of additional arguments passed to Acedrg.", 
    example = json!({"smiles": "Your SMILES string", "commandline_args": ["-z", "--something"]})
))]
pub struct AcedrgArgs {
    /// Input SMILES string
    pub smiles: Option<String>,
    /// Input mmCIF file content, base64-encoded
    pub input_mmcif_base64: Option<String>,
    /// Array of arguments for Acedrg . Note: not all Acedrg arguments are currently available
    pub commandline_args: Vec<String>,
}
