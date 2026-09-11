use std::io;
use std::path::{Path, PathBuf};

use wist_contracts::action_plan::ActionPlanContract;
use wist_shared::paths::ACTIONS_DIR;

use crate::error::RuntimeResult;
use crate::local_exec::{LocalExecRequest, execute_async as execute_local_async};
use crate::process_control::{
    RunningStateStatus, handle_expired_running_state_async, inspect_running_state,
};
use crate::quarantine::{QuarantineRequest, quarantine_execution_async};
use crate::scheduler::{DrainOutcome, DrainRequest};
use crate::state_store::execution_queue::ExecutionQueueItem;
use crate::state_store::running;

#[path = "scheduler_reporting_support.rs"]
mod reporting_support;

use reporting_support::{
    prepare_queue_head_report_async, read_queued_plan_async, reconcile_completed_execution_async,
    recover_stale_execution_async,
};

#[derive(::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Execute")]
pub(super) struct QueueHeadContext {
    workdir: PathBuf,
    running_path: PathBuf,
    plan: ActionPlanContract,
}

pub(super) enum QueueHeadDisposition {
    Blocked,
    ReloadQueue,
    Completed(Box<DrainOutcome>),
}

pub(super) async fn handle_queue_head_async(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
) -> RuntimeResult<QueueHeadDisposition> {
    let Some(head) = load_queue_head_context_async(request, item).await? else {
        return Ok(QueueHeadDisposition::ReloadQueue);
    };
    if let Some(disposition) = reconcile_queue_head_async(request, item, &head).await? {
        return Ok(disposition);
    }
    execute_queue_head_async(request, item, &head).await
}

async fn load_queue_head_context_async(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
) -> RuntimeResult<Option<QueueHeadContext>> {
    let workdir = request.run_dir.join(ACTIONS_DIR).join(&item.execution_id);
    let running_path = running::path_for(&request.state_dir, &item.execution_id);
    let plan = match read_queued_plan_async(&workdir).await {
        Ok(plan) => plan,
        Err(err) => {
            quarantine_queue_head_async(
                request,
                item,
                &running_path,
                format!("queued execution plan unavailable: {err}"),
            )
            .await?;
            return Ok(None);
        }
    };
    Ok(Some(QueueHeadContext {
        workdir,
        running_path,
        plan,
    }))
}

async fn reconcile_queue_head_async(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
    head: &QueueHeadContext,
) -> RuntimeResult<Option<QueueHeadDisposition>> {
    match tokio::fs::metadata(&head.running_path).await {
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(reconcile_completed_execution_async(
                request,
                item,
                &head.plan,
                &head.workdir,
            )
            .await?
            .map(|outcome| QueueHeadDisposition::Completed(Box::new(outcome))));
        }
        Err(err) => return Err(err.into()),
    }

    let mut state = match running::load_async(&head.running_path).await {
        Ok(state) => state,
        Err(err) => {
            quarantine_queue_head_async(
                request,
                item,
                &head.running_path,
                format!("queued execution state unavailable: {err}"),
            )
            .await?;
            return Ok(Some(QueueHeadDisposition::ReloadQueue));
        }
    };
    match inspect_running_state(&state)? {
        RunningStateStatus::Active => return Ok(Some(QueueHeadDisposition::Blocked)),
        RunningStateStatus::Expired => {
            if handle_expired_running_state_async(&mut state, &head.running_path).await? {
                return Ok(Some(QueueHeadDisposition::Blocked));
            }
        }
        RunningStateStatus::Inactive => {}
    }
    if let Some(outcome) =
        reconcile_completed_execution_async(request, item, &head.plan, &head.workdir).await?
    {
        running::remove_async(&head.running_path).await?;
        return Ok(Some(QueueHeadDisposition::Completed(Box::new(outcome))));
    }

    let recovered = recover_stale_execution_async(request, item, head, &state).await?;
    running::remove_async(&head.running_path).await?;
    Ok(Some(QueueHeadDisposition::Completed(Box::new(recovered))))
}

async fn execute_queue_head_async(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
    head: &QueueHeadContext,
) -> RuntimeResult<QueueHeadDisposition> {
    let local_result = match execute_local_async(&LocalExecRequest {
        execution_id: item.execution_id.clone(),
        run_dir: request.run_dir.clone(),
        state_dir: request.state_dir.clone(),
        exec_bin: request.exec_bin.clone(),
        cancel_grace_ms: request.cancel_grace_ms,
        stdout_limit_bytes: request.stdout_limit_bytes,
        stderr_limit_bytes: request.stderr_limit_bytes,
        plan_digest: item.plan_digest.clone(),
        request_id: item.request_id.clone(),
        plan: head.plan.clone(),
    })
    .await
    {
        Ok(local_result) => local_result,
        Err(err) => {
            quarantine_queue_head_async(
                request,
                item,
                &head.running_path,
                format!("local execution failed: {err}"),
            )
            .await?;
            return Ok(QueueHeadDisposition::ReloadQueue);
        }
    };

    let prepared =
        match prepare_queue_head_report_async(request, item, &head.plan, &local_result).await {
            Ok(prepared) => prepared,
            Err(err) => {
                quarantine_queue_head_async(
                    request,
                    item,
                    &head.running_path,
                    format!("local execution report preparation failed: {err}"),
                )
                .await?;
                return Ok(QueueHeadDisposition::ReloadQueue);
            }
        };

    running::remove_async(&head.running_path).await?;
    Ok(QueueHeadDisposition::Completed(Box::new(DrainOutcome {
        execution_id: item.execution_id.clone(),
        plan_digest: item.plan_digest.clone(),
        report: prepared.envelope,
    })))
}

async fn quarantine_queue_head_async(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
    running_path: &Path,
    reason: String,
) -> RuntimeResult<()> {
    quarantine_execution_async(QuarantineRequest::queued_item(
        &request.state_dir,
        item,
        reason,
        Some(running_path),
    ))
    .await
}
