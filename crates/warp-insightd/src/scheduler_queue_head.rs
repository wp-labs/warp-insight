use std::io;
use std::path::{Path, PathBuf};

use warp_insight_contracts::action_plan::ActionPlanContract;
use warp_insight_shared::paths::ACTIONS_DIR;

use crate::local_exec::{LocalExecRequest, execute_async as execute_local_async};
use crate::process_control::{
    RunningStateStatus, handle_expired_running_state, inspect_running_state,
};
use crate::quarantine::{QuarantineRequest, quarantine_execution};
use crate::scheduler::{DrainOutcome, DrainRequest};
use crate::state_store::execution_queue::ExecutionQueueItem;
use crate::state_store::running;

#[path = "scheduler_reporting_support.rs"]
mod reporting_support;

use reporting_support::{read_queued_plan, reconcile_completed_execution, recover_stale_execution};

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Execute")]
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
) -> io::Result<QueueHeadDisposition> {
    let Some(head) = load_queue_head_context(request, item)? else {
        return Ok(QueueHeadDisposition::ReloadQueue);
    };
    if let Some(disposition) = reconcile_queue_head(request, item, &head)? {
        return Ok(disposition);
    }
    execute_queue_head_async(request, item, &head).await
}

fn load_queue_head_context(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
) -> io::Result<Option<QueueHeadContext>> {
    let workdir = request.run_dir.join(ACTIONS_DIR).join(&item.execution_id);
    let running_path = running::path_for(&request.state_dir, &item.execution_id);
    let plan = match read_queued_plan(&workdir) {
        Ok(plan) => plan,
        Err(err) => {
            quarantine_queue_head(
                request,
                item,
                &running_path,
                format!("queued execution plan unavailable: {err}"),
            )?;
            return Ok(None);
        }
    };
    Ok(Some(QueueHeadContext {
        workdir,
        running_path,
        plan,
    }))
}

fn reconcile_queue_head(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
    head: &QueueHeadContext,
) -> io::Result<Option<QueueHeadDisposition>> {
    if !head.running_path.exists() {
        return Ok(
            reconcile_completed_execution(request, item, &head.plan, &head.workdir)?
                .map(|outcome| QueueHeadDisposition::Completed(Box::new(outcome))),
        );
    }

    let mut state = match running::load(&head.running_path) {
        Ok(state) => state,
        Err(err) => {
            quarantine_queue_head(
                request,
                item,
                &head.running_path,
                format!("queued execution state unavailable: {err}"),
            )?;
            return Ok(Some(QueueHeadDisposition::ReloadQueue));
        }
    };
    match inspect_running_state(&state)? {
        RunningStateStatus::Active => return Ok(Some(QueueHeadDisposition::Blocked)),
        RunningStateStatus::Expired => {
            if handle_expired_running_state(&mut state, &head.running_path)? {
                return Ok(Some(QueueHeadDisposition::Blocked));
            }
        }
        RunningStateStatus::Inactive => {}
    }
    if let Some(outcome) = reconcile_completed_execution(request, item, &head.plan, &head.workdir)?
    {
        running::remove(&head.running_path)?;
        return Ok(Some(QueueHeadDisposition::Completed(Box::new(outcome))));
    }

    let recovered = recover_stale_execution(request, item, head, &state)?;
    running::remove(&head.running_path)?;
    Ok(Some(QueueHeadDisposition::Completed(Box::new(recovered))))
}

async fn execute_queue_head_async(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
    head: &QueueHeadContext,
) -> io::Result<QueueHeadDisposition> {
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
            quarantine_queue_head(
                request,
                item,
                &head.running_path,
                format!("local execution failed: {err}"),
            )?;
            return Ok(QueueHeadDisposition::ReloadQueue);
        }
    };

    let prepared = match reporting_support::prepare_queue_head_report(
        request,
        item,
        &head.plan,
        &local_result,
    ) {
        Ok(prepared) => prepared,
        Err(err) => {
            quarantine_queue_head(
                request,
                item,
                &head.running_path,
                format!("local execution report preparation failed: {err}"),
            )?;
            return Ok(QueueHeadDisposition::ReloadQueue);
        }
    };

    running::remove(&head.running_path)?;
    Ok(QueueHeadDisposition::Completed(Box::new(DrainOutcome {
        execution_id: item.execution_id.clone(),
        plan_digest: item.plan_digest.clone(),
        report: prepared.envelope,
    })))
}

fn quarantine_queue_head(
    request: &DrainRequest,
    item: &ExecutionQueueItem,
    running_path: &Path,
    reason: String,
) -> io::Result<()> {
    quarantine_execution(QuarantineRequest::queued_item(
        &request.state_dir,
        item,
        reason,
        Some(running_path),
    ))
}
