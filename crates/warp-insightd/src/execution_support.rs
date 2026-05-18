//! Shared execution-state helpers.

use std::fs;
use std::io;
use std::path::Path;

use warp_insight_contracts::action_result::ActionResultContract;
use warp_insight_shared::paths::REPORT_ENVELOPE_SUFFIX;

use crate::state_store::execution_queue::{self, ExecutionQueueItem};
use crate::state_store::{history, reporting, running};

pub fn final_state_name(result: &ActionResultContract) -> &'static str {
    result.final_status.as_state_name()
}

pub fn find_duplicate_execution(
    state_dir: &Path,
    action_id: &str,
    plan_digest: &str,
) -> io::Result<Option<String>> {
    let queue_path = execution_queue::path_for(state_dir);
    let queue = execution_queue::load_or_default(&queue_path)?;
    if let Some(item) = queue
        .items
        .iter()
        .find(|item| item.action_id == action_id && item.plan_digest == plan_digest)
    {
        return Ok(Some(item.execution_id.clone()));
    }

    if let Some(execution_id) =
        find_duplicate_running_execution(&state_dir.join("running"), action_id, plan_digest)?
    {
        return Ok(Some(execution_id));
    }

    if let Some(execution_id) =
        find_duplicate_reporting_execution(&state_dir.join("reporting"), action_id, plan_digest)?
    {
        return Ok(Some(execution_id));
    }

    find_duplicate_history_execution(&state_dir.join("history"), action_id, plan_digest)
}

pub fn lookup_queued_execution(
    state_dir: &Path,
    execution_id: &str,
) -> io::Result<Option<ExecutionQueueItem>> {
    let queue_path = execution_queue::path_for(state_dir);
    if !queue_path.exists() {
        return Ok(None);
    }

    let queue = execution_queue::load_or_default(&queue_path)?;
    Ok(queue
        .items
        .into_iter()
        .find(|item| item.execution_id == execution_id))
}

fn find_duplicate_running_execution(
    dir: &Path,
    action_id: &str,
    plan_digest: &str,
) -> io::Result<Option<String>> {
    if !dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let state = match running::load(&path) {
            Ok(state) => state,
            Err(_) => continue,
        };
        if state.action_id == action_id && state.plan_digest == plan_digest {
            return Ok(Some(state.execution_id));
        }
    }

    Ok(None)
}

fn find_duplicate_reporting_execution(
    dir: &Path,
    action_id: &str,
    plan_digest: &str,
) -> io::Result<Option<String>> {
    if !dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(REPORT_ENVELOPE_SUFFIX))
        {
            continue;
        }

        let state = match reporting::load(&path) {
            Ok(state) => state,
            Err(_) => continue,
        };
        if state.action_id == action_id && state.plan_digest == plan_digest {
            return Ok(Some(state.execution_id));
        }
    }

    Ok(None)
}

fn find_duplicate_history_execution(
    dir: &Path,
    action_id: &str,
    plan_digest: &str,
) -> io::Result<Option<String>> {
    if !dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let record = match history::load(&path) {
            Ok(record) => record,
            Err(_) => continue,
        };
        if record.action_id.as_deref() == Some(action_id)
            && record.plan_digest.as_deref() == Some(plan_digest)
        {
            return Ok(Some(record.execution_id));
        }
    }

    Ok(None)
}
