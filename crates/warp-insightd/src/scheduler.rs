//! Local execution queue scheduler.

use std::io;
use std::path::PathBuf;

use warp_insight_contracts::action_plan::{ActionPlanContract, RiskLevel};
use warp_insight_shared::integrity::digest_json;
use warp_insight_shared::paths::{ACTIONS_DIR, WORKDIR_PLAN_FILE};
use warp_insight_shared::time::{after_millis_rfc3339, now_rfc3339};

use crate::execution_support::find_duplicate_execution;
use crate::local_exec::next_execution_id;
use crate::state_store::execution_queue::{self, ExecutionQueueItem};

#[path = "scheduler_queue_head.rs"]
mod queue_head_support;

use queue_head_support::{QueueHeadDisposition, handle_queue_head_async};

#[derive(Debug, Clone, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Execute")]
pub struct SchedulerRequest {
    pub run_dir: PathBuf,
    pub state_dir: PathBuf,
    pub plan: ActionPlanContract,
}

#[derive(Debug, Clone, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Execute")]
pub struct SchedulerOutcome {
    pub execution_id: String,
    pub plan_digest: String,
}

#[derive(Debug, Clone, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Execute")]
pub struct DrainRequest {
    pub run_dir: PathBuf,
    pub state_dir: PathBuf,
    pub exec_bin: PathBuf,
    pub instance_id: String,
    pub cancel_grace_ms: u64,
    pub stdout_limit_bytes: u64,
    pub stderr_limit_bytes: u64,
}

#[derive(Debug, Clone, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Execute")]
pub struct DrainOutcome {
    pub execution_id: String,
    pub plan_digest: String,
    pub report: warp_insight_contracts::gateway::ReportActionResult,
}

pub fn submit_local_plan(request: &SchedulerRequest) -> io::Result<SchedulerOutcome> {
    let execution_id = next_execution_id();
    let plan_digest = digest_json(&request.plan)?;
    let deadline_at = Some(after_millis_rfc3339(
        request.plan.constraints.max_total_duration_ms,
    ));
    if let Some(existing_execution_id) = find_duplicate_execution(
        &request.state_dir,
        &request.plan.meta.action_id,
        &plan_digest,
    )? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "duplicate action plan already tracked locally: action_id={} plan_digest={} execution_id={existing_execution_id}",
                request.plan.meta.action_id, plan_digest
            ),
        ));
    }

    let workdir = request.run_dir.join(ACTIONS_DIR).join(&execution_id);
    std::fs::create_dir_all(&workdir)?;
    warp_insight_shared::fs::write_json_atomic(&workdir.join(WORKDIR_PLAN_FILE), &request.plan)?;

    let queue_path = execution_queue::path_for(&request.state_dir);
    let queue_write = (|| -> io::Result<()> {
        let mut queue = execution_queue::load_or_default(&queue_path)?;
        // Lower numeric priority runs earlier; scheduler uses the neutral
        // default until dispatch constraints expose dynamic prioritization.
        queue.enqueue(
            ExecutionQueueItem::builder(
                execution_id.clone(),
                request.plan.meta.action_id.clone(),
                plan_digest.clone(),
                request.plan.meta.request_id.clone(),
                now_rfc3339(),
            )
            .deadline_at(deadline_at)
            .risk_level(Some(
                risk_level_name(request.plan.constraints.risk_level).to_string(),
            ))
            .build(),
        );
        execution_queue::store(&queue_path, &queue)
    })();
    if let Err(err) = queue_write {
        let _ = std::fs::remove_dir_all(&workdir);
        return Err(err);
    }

    Ok(SchedulerOutcome {
        execution_id,
        plan_digest,
    })
}

pub async fn drain_next_async(request: &DrainRequest) -> io::Result<bool> {
    Ok(drain_next_with_report_async(request).await?.is_some())
}

pub async fn drain_next_with_report_async(
    request: &DrainRequest,
) -> io::Result<Option<DrainOutcome>> {
    let queue_path = execution_queue::path_for(&request.state_dir);
    let mut queue = execution_queue::load_or_default(&queue_path)?;
    loop {
        let Some(item) = queue.items.first().cloned() else {
            return Ok(None);
        };

        match handle_queue_head_async(request, &item).await? {
            QueueHeadDisposition::Blocked => return Ok(None),
            QueueHeadDisposition::ReloadQueue => {
                queue = execution_queue::load_or_default(&queue_path)?;
            }
            QueueHeadDisposition::Completed(outcome) => {
                queue.remove(&item.execution_id);
                execution_queue::store(&queue_path, &queue)?;
                return Ok(Some(*outcome));
            }
        }
    }
}

pub fn drain_next(request: &DrainRequest) -> io::Result<bool> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(drain_next_async(request))
}

pub fn drain_next_with_report(request: &DrainRequest) -> io::Result<Option<DrainOutcome>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(drain_next_with_report_async(request))
}

fn risk_level_name(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::R0 => "R0",
        RiskLevel::R1 => "R1",
        RiskLevel::R2 => "R2",
        RiskLevel::R3 => "R3",
    }
}
