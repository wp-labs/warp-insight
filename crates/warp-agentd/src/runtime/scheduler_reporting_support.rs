use std::io;
use std::path::Path;

use crate::error::RuntimeResult;
use crate::fs_async::{read_json_async, write_json_atomic_async};
use wist_contracts::action_plan::ActionPlanContract;
use wist_contracts::action_result::ActionResultContract;
use wist_contracts::gateway::ReportActionResult;
use wist_shared::paths::{WORKDIR_PLAN_FILE, WORKDIR_RESULT_FILE};

use crate::execution_support::final_state_name;
use crate::local_exec::LocalExecOutcome;
use crate::recovery::synthesize_recovery_result;
use crate::reporting_pipeline::{
    PreparedReport, ReportingRequest, ensure_local_report_async, load_complete_local_report_async,
    prepare_local_report_async,
};
use crate::scheduler::{DrainOutcome, DrainRequest};
use crate::state_store::execution_queue::ExecutionQueueItem;
use crate::state_store::running;

use super::QueueHeadContext;

pub(super) async fn read_queued_plan_async(workdir: &Path) -> RuntimeResult<ActionPlanContract> {
    Ok(read_json_async(&workdir.join(WORKDIR_PLAN_FILE)).await?)
}

pub(super) async fn prepare_queue_head_report_async(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
    plan: &ActionPlanContract,
    local_result: &LocalExecOutcome,
) -> RuntimeResult<PreparedReport> {
    prepare_local_report_async(ReportingRequest {
        state_dir: &request.state_dir,
        execution_id: &item.execution_id,
        action_id: &item.action_id,
        request_id: &item.request_id,
        plan_digest: &item.plan_digest,
        agent_id: &plan.target.agent_id,
        instance_id: &request.instance_id,
        final_state: final_state_name(&local_result.result),
        result_path: &local_result.workdir.join(WORKDIR_RESULT_FILE),
        result: &local_result.result,
    })
    .await
}

pub(super) async fn recover_stale_execution_async(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
    head: &QueueHeadContext,
    running_state: &running::RunningExecutionState,
) -> RuntimeResult<DrainOutcome> {
    let result_path = head.workdir.join(WORKDIR_RESULT_FILE);
    let recovered = synthesize_recovery_result(running_state);
    write_json_atomic_async(&result_path, &recovered).await?;
    let prepared = ensure_local_report_async(ReportingRequest {
        state_dir: &request.state_dir,
        execution_id: &item.execution_id,
        action_id: &item.action_id,
        request_id: &item.request_id,
        plan_digest: &item.plan_digest,
        agent_id: &head.plan.target.agent_id,
        instance_id: &request.instance_id,
        final_state: final_state_name(&recovered),
        result_path: &result_path,
        result: &recovered,
    })
    .await?;
    Ok(drain_outcome(item, prepared.envelope))
}

pub(super) async fn reconcile_completed_execution_async(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
    plan: &ActionPlanContract,
    workdir: &Path,
) -> RuntimeResult<Option<DrainOutcome>> {
    let result_path = workdir.join(WORKDIR_RESULT_FILE);
    if let Some(prepared) =
        load_complete_local_report_async(&request.state_dir, &item.execution_id).await?
    {
        return Ok(Some(drain_outcome(item, prepared.envelope)));
    }

    match tokio::fs::metadata(&result_path).await {
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    }

    let result: ActionResultContract = read_json_async(&result_path).await?;
    let final_state = final_state_name(&result);
    let prepared = ensure_local_report_async(ReportingRequest {
        state_dir: &request.state_dir,
        execution_id: &item.execution_id,
        action_id: &item.action_id,
        request_id: &item.request_id,
        plan_digest: &item.plan_digest,
        agent_id: &plan.target.agent_id,
        instance_id: &request.instance_id,
        final_state,
        result_path: &result_path,
        result: &result,
    })
    .await?;

    Ok(Some(drain_outcome(item, prepared.envelope)))
}

fn drain_outcome(item: &ExecutionQueueItem, report: ReportActionResult) -> DrainOutcome {
    DrainOutcome {
        execution_id: item.execution_id.clone(),
        plan_digest: item.plan_digest.clone(),
        report,
    }
}
