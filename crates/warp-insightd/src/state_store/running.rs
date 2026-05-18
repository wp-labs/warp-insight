//! `running/<execution_id>.json` store.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warp_insight_contracts::SCHEMA_VERSION_V1;
use warp_insight_shared::fs::{read_json, write_json_atomic};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunningExecutionState {
    pub schema_version: String,
    pub execution_id: String,
    pub action_id: String,
    pub plan_digest: String,
    pub request_id: String,
    pub state: String,
    pub workdir: String,
    pub pid: Option<u32>,
    #[serde(default)]
    pub process_identity: Option<String>,
    pub started_at: String,
    pub deadline_at: Option<String>,
    pub current_step_id: Option<String>,
    pub attempt: Option<u32>,
    pub cancel_requested_at: Option<String>,
    pub kill_requested_at: Option<String>,
    pub updated_at: String,
}

impl RunningExecutionState {
    pub fn builder(
        execution_id: String,
        action_id: String,
        state: String,
        workdir: String,
    ) -> RunningExecutionStateBuilder {
        RunningExecutionStateBuilder {
            execution_id,
            action_id,
            plan_digest: String::new(),
            request_id: String::new(),
            state,
            workdir,
            pid: None,
            process_identity: None,
            started_at: String::new(),
            deadline_at: None,
            current_step_id: None,
            attempt: None,
            cancel_requested_at: None,
            kill_requested_at: None,
            updated_at: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunningExecutionStateBuilder {
    execution_id: String,
    action_id: String,
    plan_digest: String,
    request_id: String,
    state: String,
    workdir: String,
    pid: Option<u32>,
    process_identity: Option<String>,
    started_at: String,
    deadline_at: Option<String>,
    current_step_id: Option<String>,
    attempt: Option<u32>,
    cancel_requested_at: Option<String>,
    kill_requested_at: Option<String>,
    updated_at: String,
}

impl RunningExecutionStateBuilder {
    pub fn plan_digest(mut self, plan_digest: String) -> Self {
        self.plan_digest = plan_digest;
        self
    }

    pub fn request_id(mut self, request_id: String) -> Self {
        self.request_id = request_id;
        self
    }

    pub fn pid(mut self, pid: Option<u32>) -> Self {
        self.pid = pid;
        self
    }

    pub fn process_identity(mut self, process_identity: Option<String>) -> Self {
        self.process_identity = process_identity;
        self
    }

    pub fn started_at(mut self, started_at: String) -> Self {
        self.started_at = started_at;
        self
    }

    pub fn deadline_at(mut self, deadline_at: Option<String>) -> Self {
        self.deadline_at = deadline_at;
        self
    }

    pub fn current_step_id(mut self, current_step_id: Option<String>) -> Self {
        self.current_step_id = current_step_id;
        self
    }

    pub fn attempt(mut self, attempt: Option<u32>) -> Self {
        self.attempt = attempt;
        self
    }

    pub fn cancel_requested_at(mut self, cancel_requested_at: Option<String>) -> Self {
        self.cancel_requested_at = cancel_requested_at;
        self
    }

    pub fn kill_requested_at(mut self, kill_requested_at: Option<String>) -> Self {
        self.kill_requested_at = kill_requested_at;
        self
    }

    pub fn updated_at(mut self, updated_at: String) -> Self {
        self.updated_at = updated_at;
        self
    }

    pub fn build(self) -> RunningExecutionState {
        RunningExecutionState {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            execution_id: self.execution_id,
            action_id: self.action_id,
            plan_digest: self.plan_digest,
            request_id: self.request_id,
            state: self.state,
            workdir: self.workdir,
            pid: self.pid,
            process_identity: self.process_identity,
            started_at: self.started_at,
            deadline_at: self.deadline_at,
            current_step_id: self.current_step_id,
            attempt: self.attempt,
            cancel_requested_at: self.cancel_requested_at,
            kill_requested_at: self.kill_requested_at,
            updated_at: self.updated_at,
        }
    }
}

pub fn path_for(state_dir: &Path, execution_id: &str) -> PathBuf {
    state_dir
        .join("running")
        .join(format!("{execution_id}.json"))
}

pub fn load(path: &Path) -> io::Result<RunningExecutionState> {
    read_json(path)
}

pub fn store(path: &Path, state: &RunningExecutionState) -> io::Result<()> {
    write_json_atomic(path, state)
}

pub fn remove(path: &Path) -> io::Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
