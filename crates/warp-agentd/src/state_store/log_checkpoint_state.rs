//! Private file-input checkpoint state persisted by `warp-agentd`.

use serde::{Deserialize, Serialize};
use wist_contracts::SCHEMA_VERSION_V1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[serde(deny_unknown_fields)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub(crate) struct LogCheckpointState {
    pub schema_version: String,
    pub input_id: String,
    pub updated_at: String,
    /// 【已废弃，保留兼容】历史遗留的 per-input `seq` 字段，仅用于兼容旧 checkpoint 反序列化
    /// （`deny_unknown_fields`）；全局 `seq` 高水位已迁移到独立文件 `state/logs/seq.json`
    /// （见 `state_store::log_seq_state`），本字段不再读写。
    #[serde(default)]
    pub next_seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_multiline: Option<PendingMultilineState>,
    pub files: Vec<TrackedFileCheckpoint>,
}

impl LogCheckpointState {
    pub(crate) fn new(input_id: String, updated_at: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            input_id,
            updated_at,
            next_seq: 0,
            pending_multiline: None,
            files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[serde(deny_unknown_fields)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub(crate) struct PendingMultilineState {
    pub source_path: String,
    pub body: String,
    pub start_offset: u64,
    pub end_offset: u64,
    pub last_updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[serde(deny_unknown_fields)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub(crate) struct TrackedFileCheckpoint {
    pub file_id: String,
    pub path: String,
    pub device_id: Option<u64>,
    pub inode: Option<u64>,
    pub fingerprint: Option<String>,
    pub checkpoint_offset: u64,
    pub checkpoint_probe: Option<String>,
    pub last_size: Option<u64>,
    pub last_read_at: Option<String>,
    pub last_commit_point_at: Option<String>,
    pub rotated_from_path: Option<String>,
}
