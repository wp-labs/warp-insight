//! `history/<execution_id>.json` quarantine store.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warp_insight_contracts::SCHEMA_VERSION_V1;
use warp_insight_shared::fs::{read_json, write_json_atomic};
use warp_insight_shared::time::now_rfc3339;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::moju_derive::MoJu)]
#[serde(deny_unknown_fields)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
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
