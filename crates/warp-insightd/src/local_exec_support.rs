use std::io;
use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;

use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Child;
use tokio::task::JoinHandle;
use warp_insight_contracts::action_result::{
    ActionOutputs, ActionResultContract, FinalStatus, StepActionRecord, StepStatus,
};
use warp_insight_contracts::state_exec::ExecProgressState;
use warp_insight_shared::fs::write_json_atomic;
use warp_insight_shared::paths::WORKDIR_STATE_FILE;
use warp_insight_shared::time::now_rfc3339;

use crate::local_exec::LocalExecRequest;
use crate::process_control::{SignalRequestKind, record_signal_request, send_terminate};

const STREAM_TRUNCATED_MARKER: &str = "\n[truncated by warp-insightd]\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExitClassification {
    Completed(ExitStatus),
    CompletedAfterTimeout(ExitStatus),
    TimedOut,
}

pub(super) fn write_timed_out_result(
    request: &LocalExecRequest,
    workdir: &Path,
    result_path: &Path,
) -> io::Result<ActionResultContract> {
    let result = synthesize_result(
        request,
        FinalStatus::TimedOut,
        "agentd_total_timeout",
        "timed_out",
    );
    write_json_atomic(result_path, &result)?;
    write_exec_state(
        workdir,
        &request.execution_id,
        &request.plan.meta.action_id,
        "timed_out",
        Some("agentd_total_timeout".to_string()),
        "agentd timed out execution and synthesized final result",
    )?;
    Ok(result)
}

pub(super) async fn wait_for_child(
    child: &mut Child,
    timeout_ms: u64,
    cancel_grace_ms: u64,
    running_path: &Path,
) -> io::Result<ExitClassification> {
    if let Some(status) = child.try_wait()? {
        return Ok(ExitClassification::Completed(status));
    }

    if let Ok(status) =
        tokio::time::timeout(Duration::from_millis(timeout_ms.max(1)), child.wait()).await
    {
        return Ok(ExitClassification::Completed(status?));
    }

    if let Some(status) = child.try_wait()? {
        return Ok(ExitClassification::Completed(status));
    }

    record_signal_request(running_path, SignalRequestKind::Cancel)?;
    if let Some(pid) = child.id()
        && let Err(err) = send_terminate(pid)
    {
        if let Some(status) = child.try_wait()? {
            return Ok(ExitClassification::CompletedAfterTimeout(status));
        }
        if err.kind() != io::ErrorKind::InvalidInput {
            return Err(err);
        }
    }

    match tokio::time::timeout(Duration::from_millis(cancel_grace_ms.max(1)), child.wait()).await {
        Ok(status) => Ok(ExitClassification::CompletedAfterTimeout(status?)),
        Err(_) => {
            record_signal_request(running_path, SignalRequestKind::Kill)?;
            if let Err(err) = child.start_kill() {
                if child.try_wait()?.is_none() && err.kind() != io::ErrorKind::InvalidInput {
                    return Err(err);
                }
            }
            let _ = child.wait().await?;
            Ok(ExitClassification::TimedOut)
        }
    }
}

pub(super) fn spawn_stream_capture<R>(
    mut reader: R,
    mut output: File,
    limit_bytes: u64,
) -> JoinHandle<io::Result<()>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buf = [0u8; 8192];
        let mut written = 0u64;
        let mut truncated = false;

        loop {
            let read = reader.read(&mut buf).await?;
            if read == 0 {
                break;
            }

            if written < limit_bytes {
                let remaining = (limit_bytes - written) as usize;
                let to_write = read.min(remaining);
                if to_write > 0 {
                    output.write_all(&buf[..to_write]).await?;
                    written += to_write as u64;
                }
                if to_write < read && !truncated {
                    output.write_all(STREAM_TRUNCATED_MARKER.as_bytes()).await?;
                    truncated = true;
                }
            }
        }

        output.sync_all().await?;
        Ok(())
    })
}

pub(super) async fn join_capture(
    handle: JoinHandle<io::Result<()>>,
    stream_name: &str,
) -> io::Result<()> {
    match handle.await {
        Ok(result) => result,
        Err(_) => Err(io::Error::other(format!(
            "{stream_name} capture thread panicked"
        ))),
    }
}

pub(super) async fn terminate_child(child: &mut Child) -> io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }

    match child.start_kill() {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::InvalidInput => {}
        Err(err) => return Err(err),
    }
    let _ = child.wait().await?;
    Ok(())
}

pub(super) fn synthesize_result(
    request: &LocalExecRequest,
    final_status: FinalStatus,
    error_code: &str,
    step_status: &str,
) -> ActionResultContract {
    let finished_at = now_rfc3339();
    let step_status = match step_status {
        "timed_out" => StepStatus::TimedOut,
        "cancelled" => StepStatus::Cancelled,
        _ => StepStatus::Failed,
    };
    ActionResultContract {
        request_id: Some(request.request_id.clone()),
        exit_reason: Some(error_code.to_string()),
        step_records: vec![StepActionRecord {
            step_id: request.plan.program.entry.clone(),
            attempt: 1,
            op: request
                .plan
                .program
                .steps
                .iter()
                .find(|step| step.id == request.plan.program.entry)
                .and_then(|step| step.op.clone()),
            status: step_status,
            started_at: finished_at.clone(),
            finished_at: Some(finished_at.clone()),
            duration_ms: None,
            error_code: Some(error_code.to_string()),
            stdout_summary: None,
            stderr_summary: None,
            resource_usage: None,
        }],
        outputs: ActionOutputs::default(),
        started_at: Some(finished_at.clone()),
        finished_at: Some(finished_at),
        ..ActionResultContract::new(
            request.plan.meta.action_id.clone(),
            request.execution_id.clone(),
            final_status,
        )
    }
}

pub(super) fn write_exec_state(
    workdir: &Path,
    execution_id: &str,
    action_id: &str,
    state: &str,
    reason_code: Option<String>,
    detail: &str,
) -> io::Result<()> {
    let state_path = workdir.join(WORKDIR_STATE_FILE);
    write_json_atomic(
        &state_path,
        &ExecProgressState {
            execution_id: execution_id.to_string(),
            action_id: action_id.to_string(),
            state: state.to_string(),
            updated_at: now_rfc3339(),
            step_id: None,
            attempt: None,
            reason_code,
            detail: Some(detail.to_string()),
        },
    )
}
