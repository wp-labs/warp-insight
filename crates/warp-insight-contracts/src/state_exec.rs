//! Execution-related local state contract types.

use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION_V1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRuntimeState {
    pub schema_version: String,
    pub agent_id: String,
    pub instance_id: String,
    pub version: String,
    pub mode: RuntimeMode,
    pub updated_at: String,
}

impl AgentRuntimeState {
    pub fn new(
        agent_id: String,
        instance_id: String,
        version: String,
        mode: RuntimeMode,
        updated_at: String,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            agent_id,
            instance_id,
            version,
            mode,
            updated_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeMode {
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "degraded")]
    Degraded,
    #[serde(rename = "protect")]
    Protect,
    #[serde(rename = "upgrade_in_progress")]
    UpgradeInProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecRuntimeContext {
    pub execution_id: String,
    pub spawned_at: String,
    pub deadline_at: Option<String>,
    pub agent_id: String,
    pub node_id: String,
    pub workdir: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecProgressState {
    pub execution_id: String,
    pub action_id: String,
    pub state: String,
    pub updated_at: String,
    pub step_id: Option<String>,
    pub attempt: Option<u32>,
    pub reason_code: Option<String>,
    pub detail: Option<String>,
}
