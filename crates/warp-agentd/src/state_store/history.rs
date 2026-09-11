//! `history/<execution_id>.json` quarantine store.

use std::io;
use std::path::{Path, PathBuf};

use crate::fs_async::{read_json_async, write_json_atomic_async};
use serde::{Deserialize, Serialize};
use wist_contracts::SCHEMA_VERSION_V1;
use wist_shared::fs::{read_json, write_json_atomic};
use wist_shared::time::now_rfc3339;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[serde(deny_unknown_fields)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
pub struct ExecutionHistoryRecord {
    pub schema_version: String,
    pub execution_id: String,
    pub action_id: Option<String>,
    pub plan_digest: Option<String>,
    pub request_id: Option<String>,
    pub state: String,
    pub detail: String,
    pub recorded_at: String,
}

impl ExecutionHistoryRecord {
    pub fn quarantined(
        execution_id: String,
        action_id: Option<String>,
        plan_digest: Option<String>,
        request_id: Option<String>,
        detail: String,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            execution_id,
            action_id,
            plan_digest,
            request_id,
            state: "quarantined".to_string(),
            detail,
            recorded_at: now_rfc3339(),
        }
    }
}

pub fn path_for(state_dir: &Path, execution_id: &str) -> PathBuf {
    state_dir
        .join("history")
        .join(format!("{execution_id}.json"))
}

pub fn load(path: &Path) -> io::Result<ExecutionHistoryRecord> {
    read_json(path)
}

pub fn store(path: &Path, record: &ExecutionHistoryRecord) -> io::Result<()> {
    write_json_atomic(path, record)
}

pub async fn load_async(path: &Path) -> io::Result<ExecutionHistoryRecord> {
    read_json_async(path).await
}

pub async fn store_async(path: &Path, record: &ExecutionHistoryRecord) -> io::Result<()> {
    write_json_atomic_async(path, record).await
}
