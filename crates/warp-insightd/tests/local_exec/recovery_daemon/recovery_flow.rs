use std::fs;

use warp_insight_shared::fs::{read_json, write_json_atomic};
use warp_insightd::bootstrap;
use warp_insightd::daemon;
use warp_insightd::reporting_pipeline;
use warp_insightd::scheduler::{
    DrainRequest, SchedulerRequest, drain_next_with_report, submit_local_plan,
};
use warp_insightd::state_store::{execution_queue, history, reporting, running};

use super::super::common::{rfc3339_before_now, sample_plan, temp_dir, write_exec_wrapper};

#[cfg(unix)]
#[test]
fn recovery_turns_incomplete_running_state_into_reporting() {
    let root = temp_dir("recover");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    let submitted = submit_local_plan(&SchedulerRequest {
        run_dir: run_dir.clone(),
        state_dir: state_dir.clone(),
        plan: sample_plan(),
    })
    .expect("submit local plan");

    let workdir = run_dir.join("actions").join(&submitted.execution_id);
    let running_path = running::path_for(&state_dir, &submitted.execution_id);
    let running_state = running::RunningExecutionState::builder(
        submitted.execution_id.clone(),
        "act_001".to_string(),
        "running".to_string(),
        workdir.display().to_string(),
    )
    .plan_digest(submitted.plan_digest.clone())
    .request_id("req_001".to_string())
    .started_at("2026-04-12T10:00:00Z".to_string())
    .updated_at("2026-04-12T10:00:01Z".to_string())
    .pid(Some(4242))
    .process_identity(None)
    .deadline_at(Some("2026-04-12T10:05:00Z".to_string()))
    .current_step_id(Some("step_collect".to_string()))
    .attempt(Some(1))
    .cancel_requested_at(None)
    .kill_requested_at(None)
    .build();
    write_json_atomic(&running_path, &running_state).expect("write running state");

    daemon::recover_incomplete_executions(&state_dir, "instance-001").expect("recover");

    let reporting_path = reporting::path_for(&state_dir, &submitted.execution_id);
    let report_envelope_path =
        reporting_pipeline::envelope_path_for(&state_dir, &submitted.execution_id);
    let reporting_state: reporting::ReportingState =
        read_json(&reporting_path).expect("read reporting state");
    let report_envelope: warp_insight_contracts::gateway::ReportActionResult =
        read_json(&report_envelope_path).expect("read report envelope");

    assert!(!running_path.exists());
    assert_eq!(reporting_state.final_state, "failed");
    assert_eq!(reporting_state.report_attempt, 0);
    assert!(reporting_state.last_report_at.is_none());
    assert_eq!(
        report_envelope.result.exit_reason.as_deref(),
        Some("agentd_recovered_incomplete_execution")
    );
    assert_eq!(
        report_envelope.report_id,
        format!("rep_{}_1", submitted.execution_id)
    );
}

#[cfg(unix)]
#[test]
fn recovery_reuses_reporting_state_and_rebuilds_report_attempt() {
    let root = temp_dir("recover-rebuild-envelope");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    let plan = sample_plan();
    let submitted = submit_local_plan(&SchedulerRequest {
        run_dir: run_dir.clone(),
        state_dir: state_dir.clone(),
        plan: plan.clone(),
    })
    .expect("submit local plan");

    let local_outcome =
        warp_insightd::local_exec::execute(&warp_insightd::local_exec::LocalExecRequest {
            execution_id: submitted.execution_id.clone(),
            run_dir: run_dir.clone(),
            state_dir: state_dir.clone(),
            exec_bin: super::super::common::test_exec_bin(&root),
            cancel_grace_ms: 5_000,
            stdout_limit_bytes: 1_048_576,
            stderr_limit_bytes: 1_048_576,
            plan_digest: submitted.plan_digest.clone(),
            request_id: "req_001".to_string(),
            plan: plan.clone(),
        })
        .expect("seed local exec result");

    reporting_pipeline::prepare_local_report(reporting_pipeline::ReportingRequest {
        state_dir: &state_dir,
        execution_id: &submitted.execution_id,
        action_id: "act_001",
        request_id: "req_001",
        plan_digest: &submitted.plan_digest,
        agent_id: &plan.target.agent_id,
        instance_id: "instance-001",
        final_state: "succeeded",
        result_path: &local_outcome.workdir.join("result.json"),
        result: &local_outcome.result,
    })
    .expect("prepare local report");

    let running_path = running::path_for(&state_dir, &submitted.execution_id);
    let reporting_path = reporting::path_for(&state_dir, &submitted.execution_id);
    let envelope_path = reporting_pipeline::envelope_path_for(&state_dir, &submitted.execution_id);
    let mut reporting_state: reporting::ReportingState =
        read_json(&reporting_path).expect("read reporting state");
    reporting_state.report_attempt = 2;
    reporting_state.last_report_at = Some("2026-04-12T10:00:09Z".to_string());
    reporting_state.last_report_error = Some("cp unavailable".to_string());
    reporting::store(&reporting_path, &reporting_state).expect("store reporting state");
    fs::write(&envelope_path, "{bad json").expect("corrupt envelope");

    daemon::recover_incomplete_executions(&state_dir, "instance-001").expect("recover");

    let rebuilt_reporting_state: reporting::ReportingState =
        read_json(&reporting_path).expect("read rebuilt reporting state");
    let rebuilt_envelope: warp_insight_contracts::gateway::ReportActionResult =
        read_json(&envelope_path).expect("read rebuilt envelope");

    assert!(!running_path.exists());
    assert_eq!(rebuilt_reporting_state.report_attempt, 2);
    assert_eq!(
        rebuilt_reporting_state.last_report_at.as_deref(),
        Some("2026-04-12T10:00:09Z")
    );
    assert_eq!(
        rebuilt_reporting_state.last_report_error.as_deref(),
        Some("cp unavailable")
    );
    assert_eq!(rebuilt_envelope.report_attempt, 3);
    assert_eq!(
        rebuilt_envelope.report_id,
        format!("rep_{}_3", submitted.execution_id)
    );
}

#[cfg(unix)]
#[test]
fn recovery_quarantines_execution_when_report_preparation_fails() {
    let root = temp_dir("recover-report-prep-fail");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    let submitted = submit_local_plan(&SchedulerRequest {
        run_dir: run_dir.clone(),
        state_dir: state_dir.clone(),
        plan: sample_plan(),
    })
    .expect("submit local plan");

    let workdir = run_dir.join("actions").join(&submitted.execution_id);
    let running_path = running::path_for(&state_dir, &submitted.execution_id);
    let running_state = running::RunningExecutionState::builder(
        submitted.execution_id.clone(),
        "act_001".to_string(),
        "running".to_string(),
        workdir.display().to_string(),
    )
    .plan_digest(submitted.plan_digest.clone())
    .request_id("req_001".to_string())
    .started_at("2026-04-12T10:00:00Z".to_string())
    .updated_at("2026-04-12T10:00:01Z".to_string())
    .pid(Some(4242))
    .process_identity(None)
    .deadline_at(Some("2026-04-12T10:05:00Z".to_string()))
    .current_step_id(Some("step_collect".to_string()))
    .attempt(Some(1))
    .cancel_requested_at(None)
    .kill_requested_at(None)
    .build();
    write_json_atomic(&running_path, &running_state).expect("write running state");

    let reporting_path = reporting::path_for(&state_dir, &submitted.execution_id);
    let blocked_state_tmp_path = reporting_path.with_extension("tmp");
    fs::create_dir(&blocked_state_tmp_path).expect("block reporting state tmp path");

    daemon::recover_incomplete_executions(&state_dir, "instance-001").expect("recover");

    let quarantine_path = history::path_for(&state_dir, &submitted.execution_id);
    let quarantine: history::ExecutionHistoryRecord =
        read_json(&quarantine_path).expect("read quarantine record");
    let envelope_path = reporting_pipeline::envelope_path_for(&state_dir, &submitted.execution_id);

    assert!(!running_path.exists());
    assert!(!reporting_path.exists());
    assert!(!envelope_path.exists());
    assert_eq!(quarantine.state, "quarantined");
    assert_eq!(
        quarantine.plan_digest.as_deref(),
        Some(submitted.plan_digest.as_str())
    );
    assert!(
        quarantine
            .detail
            .contains("running execution report preparation failed")
    );
}

#[cfg(unix)]
#[test]
fn recovery_keeps_expired_execution_blocked_when_process_cannot_be_terminated() {
    let root = temp_dir("recover-expired-live-pid");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    let submitted = submit_local_plan(&SchedulerRequest {
        run_dir: run_dir.clone(),
        state_dir: state_dir.clone(),
        plan: sample_plan(),
    })
    .expect("submit local plan");

    let workdir = run_dir.join("actions").join(&submitted.execution_id);
    let running_path = running::path_for(&state_dir, &submitted.execution_id);
    let reporting_path = reporting::path_for(&state_dir, &submitted.execution_id);
    let running_state = running::RunningExecutionState::builder(
        submitted.execution_id.clone(),
        "act_001".to_string(),
        "running".to_string(),
        workdir.display().to_string(),
    )
    .plan_digest(submitted.plan_digest.clone())
    .request_id("req_001".to_string())
    .started_at(rfc3339_before_now(30))
    .updated_at(rfc3339_before_now(5))
    .pid(Some(1))
    .process_identity(None)
    .deadline_at(Some(rfc3339_before_now(5)))
    .current_step_id(Some("step_collect".to_string()))
    .attempt(Some(1))
    .cancel_requested_at(None)
    .kill_requested_at(None)
    .build();
    write_json_atomic(&running_path, &running_state).expect("write running state");

    daemon::recover_incomplete_executions(&state_dir, "instance-001").expect("recover");

    let running_state = running::load(&running_path).expect("load running state");
    assert!(running_state.kill_requested_at.is_some());
    assert!(running_path.exists());
    assert!(!reporting_path.exists());
}

#[cfg(unix)]
#[test]
fn recovery_continues_past_corrupt_running_state() {
    let root = temp_dir("recover-corrupt-running");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    fs::write(state_dir.join("running").join("broken.json"), "{bad json")
        .expect("write broken running state");

    let submitted = submit_local_plan(&SchedulerRequest {
        run_dir: run_dir.clone(),
        state_dir: state_dir.clone(),
        plan: sample_plan(),
    })
    .expect("submit local plan");
    let workdir = run_dir.join("actions").join(&submitted.execution_id);
    let running_path = running::path_for(&state_dir, &submitted.execution_id);
    let running_state = running::RunningExecutionState::builder(
        submitted.execution_id.clone(),
        "act_001".to_string(),
        "running".to_string(),
        workdir.display().to_string(),
    )
    .plan_digest(submitted.plan_digest.clone())
    .request_id("req_001".to_string())
    .started_at("2026-04-12T10:00:00Z".to_string())
    .updated_at("2026-04-12T10:00:01Z".to_string())
    .pid(Some(4242))
    .process_identity(None)
    .deadline_at(Some("2026-04-12T10:05:00Z".to_string()))
    .current_step_id(Some("step_collect".to_string()))
    .attempt(Some(1))
    .cancel_requested_at(None)
    .kill_requested_at(None)
    .build();
    write_json_atomic(&running_path, &running_state).expect("write valid running state");

    daemon::recover_incomplete_executions(&state_dir, "instance-001").expect("recover");

    let quarantine_path = history::path_for(&state_dir, "broken");
    let quarantine: history::ExecutionHistoryRecord =
        read_json(&quarantine_path).expect("read quarantine record");
    let reporting_path = reporting::path_for(&state_dir, &submitted.execution_id);

    assert_eq!(quarantine.state, "quarantined");
    assert!(quarantine.detail.contains("running state unreadable"));
    assert!(!state_dir.join("running").join("broken.json").exists());
    assert!(reporting_path.exists());
}

#[cfg(unix)]
#[test]
fn recovery_quarantines_running_execution_when_plan_is_missing() {
    let root = temp_dir("recover-missing-plan");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    let submitted = submit_local_plan(&SchedulerRequest {
        run_dir: run_dir.clone(),
        state_dir: state_dir.clone(),
        plan: sample_plan(),
    })
    .expect("submit local plan");

    let workdir = run_dir.join("actions").join(&submitted.execution_id);
    let running_path = running::path_for(&state_dir, &submitted.execution_id);
    let running_state = running::RunningExecutionState::builder(
        submitted.execution_id.clone(),
        "act_001".to_string(),
        "running".to_string(),
        workdir.display().to_string(),
    )
    .plan_digest(submitted.plan_digest.clone())
    .request_id("req_001".to_string())
    .started_at("2026-04-12T10:00:00Z".to_string())
    .updated_at("2026-04-12T10:00:01Z".to_string())
    .pid(Some(4242))
    .process_identity(None)
    .deadline_at(Some("2026-04-12T10:05:00Z".to_string()))
    .current_step_id(Some("step_collect".to_string()))
    .attempt(Some(1))
    .cancel_requested_at(None)
    .kill_requested_at(None)
    .build();
    write_json_atomic(&running_path, &running_state).expect("write running state");
    fs::remove_file(workdir.join("plan.json")).expect("remove plan");

    daemon::recover_incomplete_executions(&state_dir, "instance-001").expect("recover");

    let quarantine_path = history::path_for(&state_dir, &submitted.execution_id);
    let quarantine: history::ExecutionHistoryRecord =
        read_json(&quarantine_path).expect("read quarantine record");
    let reporting_path = reporting::path_for(&state_dir, &submitted.execution_id);

    assert_eq!(quarantine.state, "quarantined");
    assert_eq!(
        quarantine.plan_digest.as_deref(),
        Some(submitted.plan_digest.as_str())
    );
    assert!(
        quarantine
            .detail
            .contains("running execution plan unavailable")
    );
    assert!(!running_path.exists());
    assert!(!reporting_path.exists());
}

#[cfg(unix)]
#[test]
fn drain_does_not_rerun_expired_execution_when_process_cannot_be_terminated() {
    let root = temp_dir("drain-recover-expired");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    let submitted = submit_local_plan(&SchedulerRequest {
        run_dir: run_dir.clone(),
        state_dir: state_dir.clone(),
        plan: sample_plan(),
    })
    .expect("submit local plan");

    let workdir = run_dir.join("actions").join(&submitted.execution_id);
    let running_path = running::path_for(&state_dir, &submitted.execution_id);
    let running_state = running::RunningExecutionState::builder(
        submitted.execution_id.clone(),
        "act_001".to_string(),
        "running".to_string(),
        workdir.display().to_string(),
    )
    .plan_digest(submitted.plan_digest.clone())
    .request_id("req_001".to_string())
    .started_at(rfc3339_before_now(30))
    .updated_at(rfc3339_before_now(5))
    .pid(Some(1))
    .process_identity(None)
    .deadline_at(Some(rfc3339_before_now(5)))
    .current_step_id(Some("step_collect".to_string()))
    .attempt(Some(1))
    .cancel_requested_at(None)
    .kill_requested_at(None)
    .build();
    write_json_atomic(&running_path, &running_state).expect("write running state");

    let rerun_flag = root.join("rerun.flag");
    let exec_bin = write_exec_wrapper(
        &root,
        &format!("echo rerun > \"{}\"\nexit 88", rerun_flag.display()),
    );

    let outcome = drain_next_with_report(&DrainRequest {
        run_dir: run_dir.clone(),
        state_dir: state_dir.clone(),
        exec_bin,
        instance_id: "instance-001".to_string(),
        cancel_grace_ms: 5_000,
        stdout_limit_bytes: 1_048_576,
        stderr_limit_bytes: 1_048_576,
    })
    .expect("drain expired running execution");

    let queue_path = execution_queue::path_for(&state_dir);
    let queue_state = execution_queue::load_or_default(&queue_path).expect("queue state");
    let running_state = running::load(&running_path).expect("load running state");
    assert!(outcome.is_none());
    assert!(!rerun_flag.exists());
    assert!(running_state.kill_requested_at.is_some());
    assert!(running_path.exists());
    assert_eq!(queue_state.items.len(), 1);
}
