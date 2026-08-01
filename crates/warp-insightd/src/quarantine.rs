//! Shared quarantine helpers.

use std::io;
use std::path::Path;

use crate::execution_support::lookup_queued_execution;
use crate::reporting_pipeline::remove_local_report_artifacts;
use crate::state_store::execution_queue::ExecutionQueueItem;
use crate::state_store::running::RunningExecutionState;
use crate::state_store::{execution_queue, history, running};

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Execute")]
pub struct QuarantineRequest<'a> {
    pub state_dir: &'a Path,
    pub execution_id: &'a str,
    pub action_id: Option<&'a str>,
    pub plan_digest: Option<&'a str>,
    pub request_id: Option<&'a str>,
    pub detail: String,
    pub running_path: Option<&'a Path>,
    pub remove_from_queue: bool,
}

impl<'a> QuarantineRequest<'a> {
    pub fn queued_item(
        state_dir: &'a Path,
        item: &'a ExecutionQueueItem,
        detail: String,
        running_path: Option<&'a Path>,
    ) -> Self {
        Self {
            state_dir,
            execution_id: &item.execution_id,
            action_id: Some(&item.action_id),
            plan_digest: Some(&item.plan_digest),
            request_id: Some(&item.request_id),
            detail,
            running_path,
            remove_from_queue: true,
        }
    }

    pub fn running_state(
        state_dir: &'a Path,
        state: &'a RunningExecutionState,
        detail: String,
        running_path: &'a Path,
    ) -> Self {
        Self {
            state_dir,
            execution_id: &state.execution_id,
            action_id: Some(&state.action_id),
            plan_digest: Some(&state.plan_digest),
            request_id: Some(&state.request_id),
            detail,
            running_path: Some(running_path),
            remove_from_queue: true,
        }
    }

    pub fn unreadable_running(
        state_dir: &'a Path,
        execution_id: &'a str,
        detail: String,
        running_path: &'a Path,
    ) -> Self {
        Self {
            state_dir,
            execution_id,
            action_id: None,
            plan_digest: None,
            request_id: None,
            detail,
            running_path: Some(running_path),
            remove_from_queue: true,
        }
    }
}

pub fn quarantine_execution(request: QuarantineRequest<'_>) -> io::Result<()> {
    let queued = lookup_queued_execution(request.state_dir, request.execution_id)?;
    let record = history::ExecutionHistoryRecord::quarantined(
        request.execution_id.to_string(),
        request
            .action_id
            .map(str::to_string)
            .or_else(|| queued.as_ref().map(|item| item.action_id.clone())),
        request
            .plan_digest
            .map(str::to_string)
            .or_else(|| queued.as_ref().map(|item| item.plan_digest.clone())),
        request
            .request_id
            .map(str::to_string)
            .or_else(|| queued.as_ref().map(|item| item.request_id.clone())),
        request.detail,
    );
    remove_local_report_artifacts(request.state_dir, request.execution_id)?;
    history::store(
        &history::path_for(request.state_dir, request.execution_id),
        &record,
    )?;
    if request.remove_from_queue {
        remove_queued_execution(request.state_dir, request.execution_id)?;
    }
    if let Some(running_path) = request.running_path {
        running::remove(running_path)?;
    }
    Ok(())
}

pub fn remove_queued_execution(state_dir: &Path, execution_id: &str) -> io::Result<()> {
    let queue_path = execution_queue::path_for(state_dir);
    if !queue_path.exists() {
        return Ok(());
    }

    let mut queue = execution_queue::load_or_default(&queue_path)?;
    let original_len = queue.items.len();
    queue.remove(execution_id);
    if queue.items.len() == original_len {
        return Ok(());
    }
    execution_queue::store(&queue_path, &queue)
}
