//! Result writer helpers.

use std::io;

use warp_insight_contracts::action_result::ActionResultContract;
use warp_insight_contracts::state_exec::ExecProgressState;
use warp_insight_shared::time::now_rfc3339;

use crate::workdir::ExecutionWorkdir;

pub fn write(workdir: &ExecutionWorkdir, result: &ActionResultContract) -> io::Result<()> {
    workdir.write_result(result)?;
    workdir.write_state(&ExecProgressState {
        execution_id: result.execution_id.clone(),
        action_id: result.action_id.clone(),
        state: result.final_status.as_state_name().to_string(),
        updated_at: now_rfc3339(),
        step_id: None,
        attempt: None,
        reason_code: result.exit_reason.clone(),
        detail: Some("final result persisted".to_string()),
    })?;
    Ok(())
}
