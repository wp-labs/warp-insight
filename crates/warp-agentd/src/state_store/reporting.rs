//! `reporting/<execution_id>.json` store.

use std::io;
use std::path::{Path, PathBuf};

use crate::fs_async::{read_json_async, write_json_atomic_async};
use serde::{Deserialize, Serialize};
use wist_contracts::SCHEMA_VERSION_V1;
use wist_shared::fs::{read_json, write_json_atomic};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportingState {
    pub schema_version: String,
    pub execution_id: String,
    pub action_id: String,
    pub plan_digest: String,
    pub request_id: String,
    pub final_state: String,
    pub result_path: String,
    pub report_envelope_path: Option<String>,
    pub result_digest: Option<String>,
    pub result_signature: Option<String>,
    pub report_attempt: u32,
    pub last_report_at: Option<String>,
    pub last_report_error: Option<String>,
}

impl ReportingState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        execution_id: String,
        action_id: String,
        plan_digest: String,
        request_id: String,
        final_state: String,
        result_path: String,
        report_envelope_path: Option<String>,
        result_digest: Option<String>,
        result_signature: Option<String>,
        report_attempt: u32,
        last_report_at: Option<String>,
        last_report_error: Option<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            execution_id,
            action_id,
            plan_digest,
            request_id,
            final_state,
            result_path,
            report_envelope_path,
            result_digest,
            result_signature,
            report_attempt,
            last_report_at,
            last_report_error,
        }
    }
}

pub fn path_for(state_dir: &Path, execution_id: &str) -> PathBuf {
    state_dir
        .join("reporting")
        .join(format!("{execution_id}.json"))
}

pub fn load(path: &Path) -> io::Result<ReportingState> {
    read_json(path)
}

pub fn store(path: &Path, state: &ReportingState) -> io::Result<()> {
    write_json_atomic(path, state)
}

pub async fn load_async(path: &Path) -> io::Result<ReportingState> {
    read_json_async(path).await
}

pub async fn store_async(path: &Path, state: &ReportingState) -> io::Result<()> {
    write_json_atomic_async(path, state).await
}
