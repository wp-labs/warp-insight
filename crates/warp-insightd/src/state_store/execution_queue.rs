//! `execution_queue.json` store.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warp_insight_contracts::SCHEMA_VERSION_V1;
use warp_insight_shared::fs::{read_json, write_json_atomic};
use warp_insight_shared::paths::EXECUTION_QUEUE_FILE;
use warp_insight_shared::time::now_rfc3339;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::moju_derive::MoJu)]
#[serde(deny_unknown_fields)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Execute")]
pub struct ExecutionQueueState {
    pub schema_version: String,
    pub updated_at: String,
    pub items: Vec<ExecutionQueueItem>,
}

impl ExecutionQueueState {
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            updated_at: now_rfc3339(),
            items: Vec::new(),
        }
    }

    pub fn enqueue(&mut self, item: ExecutionQueueItem) {
        // The local execution queue is expected to stay small. Keep insertion
        // simple and stable: lower numeric priority runs first, equal priority
        // preserves FIFO order.
        let insert_at = self
            .items
            .iter()
            .position(|entry| item.priority < entry.priority)
            .unwrap_or(self.items.len());
        self.items.insert(insert_at, item);
        self.updated_at = now_rfc3339();
    }

    pub fn remove(&mut self, execution_id: &str) {
        self.items.retain(|item| item.execution_id != execution_id);
        self.updated_at = now_rfc3339();
    }
}

impl Default for ExecutionQueueState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::moju_derive::MoJu)]
#[serde(deny_unknown_fields)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Execute")]
pub struct ExecutionQueueItem {
    pub execution_id: String,
    pub action_id: String,
    pub plan_digest: String,
    pub request_id: String,
    pub priority: u32,
    pub queued_at: String,
    pub deadline_at: Option<String>,
    pub cancelable: bool,
    pub risk_level: Option<String>,
}

impl ExecutionQueueItem {
    pub fn builder(
        execution_id: String,
        action_id: String,
        plan_digest: String,
        request_id: String,
        queued_at: String,
    ) -> ExecutionQueueItemBuilder {
        ExecutionQueueItemBuilder {
            execution_id,
            action_id,
            plan_digest,
            request_id,
            priority: 100,
            queued_at,
            deadline_at: None,
            cancelable: true,
            risk_level: None,
        }
    }
}

#[derive(Debug, Clone, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Execute")]
pub struct ExecutionQueueItemBuilder {
    execution_id: String,
    action_id: String,
    plan_digest: String,
    request_id: String,
    priority: u32,
    queued_at: String,
    deadline_at: Option<String>,
    cancelable: bool,
    risk_level: Option<String>,
}

impl ExecutionQueueItemBuilder {
    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn deadline_at(mut self, deadline_at: Option<String>) -> Self {
        self.deadline_at = deadline_at;
        self
    }

    pub fn cancelable(mut self, cancelable: bool) -> Self {
        self.cancelable = cancelable;
        self
    }

    pub fn risk_level(mut self, risk_level: Option<String>) -> Self {
        self.risk_level = risk_level;
        self
    }

    pub fn build(self) -> ExecutionQueueItem {
        ExecutionQueueItem {
            execution_id: self.execution_id,
            action_id: self.action_id,
            plan_digest: self.plan_digest,
            request_id: self.request_id,
            priority: self.priority,
            queued_at: self.queued_at,
            deadline_at: self.deadline_at,
            cancelable: self.cancelable,
            risk_level: self.risk_level,
        }
    }
}

pub fn path_for(state_dir: &Path) -> PathBuf {
    state_dir.join(EXECUTION_QUEUE_FILE)
}

pub fn load_or_default(path: &Path) -> io::Result<ExecutionQueueState> {
    if path.exists() {
        read_json(path)
    } else {
        Ok(ExecutionQueueState::new())
    }
}

pub fn store(path: &Path, state: &ExecutionQueueState) -> io::Result<()> {
    write_json_atomic(path, state)
}

#[cfg(test)]
mod tests {
    use super::{ExecutionQueueItem, ExecutionQueueState};

    fn item(execution_id: &str, priority: u32, queued_at: &str) -> ExecutionQueueItem {
        ExecutionQueueItem::builder(
            execution_id.to_string(),
            format!("act_{execution_id}"),
            format!("digest_{execution_id}"),
            format!("req_{execution_id}"),
            queued_at.to_string(),
        )
        .priority(priority)
        .build()
    }

    #[test]
    fn enqueue_preserves_fifo_for_equal_priority_items() {
        let mut queue = ExecutionQueueState::new();
        queue.enqueue(item("exec_1", 100, "2026-04-13T10:00:00Z"));
        queue.enqueue(item("exec_2", 100, "2026-04-13T10:00:01Z"));
        queue.enqueue(item("exec_3", 100, "2026-04-13T10:00:02Z"));

        let ids: Vec<&str> = queue
            .items
            .iter()
            .map(|item| item.execution_id.as_str())
            .collect();
        assert_eq!(ids, vec!["exec_1", "exec_2", "exec_3"]);
    }

    #[test]
    fn enqueue_places_higher_priority_items_first() {
        let mut queue = ExecutionQueueState::new();
        queue.enqueue(item("exec_low", 200, "2026-04-13T10:00:00Z"));
        queue.enqueue(item("exec_high", 50, "2026-04-13T10:00:01Z"));
        queue.enqueue(item("exec_mid", 100, "2026-04-13T10:00:02Z"));

        let ids: Vec<&str> = queue
            .items
            .iter()
            .map(|item| item.execution_id.as_str())
            .collect();
        assert_eq!(ids, vec!["exec_high", "exec_mid", "exec_low"]);
    }
}
