use std::io;
use std::path::{Path, PathBuf};

use crate::error::RuntimeResult;
use crate::fs_async::{read_json_async, write_json_atomic_async};
use wist_shared::paths::{WORKDIR_PLAN_FILE, WORKDIR_RESULT_FILE};

use crate::execution_support::final_state_name;
use crate::process_control::{
    RunningStateStatus, handle_expired_running_state_async, inspect_running_state,
};
use crate::quarantine::{QuarantineRequest, quarantine_execution_async};
use crate::recovery::synthesize_recovery_result;
use crate::reporting_pipeline::{ReportingRequest, ensure_local_report_async};
use crate::state_store::running;

pub(super) async fn recover_incomplete_executions_impl_async(
    state_dir: &Path,
    instance_id: &str,
) -> RuntimeResult<()> {
    let running_dir = state_dir.join("running");
    match tokio::fs::metadata(&running_dir).await {
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    }

    let mut entries = tokio::fs::read_dir(&running_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let mut state: running::RunningExecutionState = match read_json_async(&path).await {
            Ok(state) => state,
            Err(err) => {
                quarantine_execution_async(QuarantineRequest::unreadable_running(
                    state_dir,
                    path.file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("unknown-execution"),
                    format!("running state unreadable: {err}"),
                    &path,
                ))
                .await?;
                continue;
            }
        };
        let workdir = PathBuf::from(&state.workdir);
        let result_path = workdir.join(WORKDIR_RESULT_FILE);
        let plan = match read_queued_plan_async(&workdir).await {
            Ok(plan) => plan,
            Err(err) => {
                quarantine_running_state_async(
                    state_dir,
                    &path,
                    &state,
                    format!("running execution plan unavailable: {err}"),
                )
                .await?;
                continue;
            }
        };
        let result = match tokio::fs::metadata(&result_path).await {
            Ok(_) => match read_json_async(&result_path).await {
                Ok(result) => result,
                Err(err) => {
                    quarantine_running_state_async(
                        state_dir,
                        &path,
                        &state,
                        format!("running execution result unavailable: {err}"),
                    )
                    .await?;
                    continue;
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                if execution_is_still_running_async(&mut state, &path).await? {
                    continue;
                } else {
                    let recovered = synthesize_recovery_result(&state);
                    write_json_atomic_async(&result_path, &recovered).await?;
                    recovered
                }
            }
            Err(err) => return Err(err.into()),
        };

        if let Err(err) = ensure_local_report_async(ReportingRequest {
            state_dir,
            execution_id: &state.execution_id,
            action_id: &state.action_id,
            request_id: &state.request_id,
            plan_digest: &state.plan_digest,
            agent_id: &plan.target.agent_id,
            instance_id,
            final_state: final_state_name(&result),
            result_path: &result_path,
            result: &result,
        })
        .await
        {
            quarantine_running_state_async(
                state_dir,
                &path,
                &state,
                format!("running execution report preparation failed: {err}"),
            )
            .await?;
            continue;
        }
        running::remove_async(&path).await?;
    }

    Ok(())
}

async fn execution_is_still_running_async(
    state: &mut running::RunningExecutionState,
    running_path: &Path,
) -> RuntimeResult<bool> {
    match inspect_running_state(state)? {
        RunningStateStatus::Active => Ok(true),
        RunningStateStatus::Expired => {
            Ok(handle_expired_running_state_async(state, running_path).await?)
        }
        RunningStateStatus::Inactive => Ok(false),
    }
}

async fn read_queued_plan_async(
    workdir: &Path,
) -> RuntimeResult<wist_contracts::action_plan::ActionPlanContract> {
    Ok(read_json_async(&workdir.join(WORKDIR_PLAN_FILE)).await?)
}

async fn quarantine_running_state_async(
    state_dir: &Path,
    running_path: &Path,
    state: &running::RunningExecutionState,
    detail: String,
) -> RuntimeResult<()> {
    quarantine_execution_async(QuarantineRequest::running_state(
        state_dir,
        state,
        detail,
        running_path,
    ))
    .await
}
