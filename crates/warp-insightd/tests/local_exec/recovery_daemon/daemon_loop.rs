use std::fs;

use warp_insight_shared::fs::{read_json, write_json_atomic};
use warp_insightd::bootstrap;
use warp_insightd::daemon;
use warp_insightd::scheduler::{SchedulerRequest, submit_local_plan};
use warp_insightd::state_store::{execution_queue, history, reporting, running};

use super::super::common::{
    rfc3339_after_now, rfc3339_before_now, sample_plan, temp_dir, test_exec_bin, write_exec_wrapper,
};

#[cfg(unix)]
#[test]
fn daemon_run_once_quarantines_execution_local_failure_without_stopping_loop() {
    let root = temp_dir("run-once-quarantine-exec-failure");
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

    let exec_bin = write_exec_wrapper(
        &root,
        "workdir=\"$3\"\nprintf '{bad json' > \"$workdir/result.json\"\nexit 0",
    );
    let config = super::super::common::standalone_config(&root);
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &exec_bin,
    })
    .expect("daemon run once");

    let queue_state =
        execution_queue::load_or_default(&execution_queue::path_for(&state_dir)).expect("queue");
    let running_path = running::path_for(&state_dir, &submitted.execution_id);
    let quarantine_path = history::path_for(&state_dir, &submitted.execution_id);
    let quarantine: history::ExecutionHistoryRecord =
        read_json(&quarantine_path).expect("read quarantine record");
    let reporting_path = reporting::path_for(&state_dir, &submitted.execution_id);

    assert_eq!(
        snapshot.state,
        warp_insightd::self_observability::HealthState::Idle
    );
    assert_eq!(snapshot.queue_depth, 0);
    assert_eq!(snapshot.running_count, 0);
    assert_eq!(snapshot.reporting_count, 0);
    assert!(queue_state.items.is_empty());
    assert!(!running_path.exists());
    assert!(!reporting_path.exists());
    assert_eq!(quarantine.state, "quarantined");
    assert!(quarantine.detail.contains("local execution failed"));
}

#[cfg(unix)]
#[test]
fn daemon_run_once_returns_error_when_execution_queue_is_corrupt() {
    let root = temp_dir("run-once-corrupt-queue");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    fs::write(execution_queue::path_for(&state_dir), "{bad json").expect("corrupt queue");

    let config = super::super::common::standalone_config(&root);
    let err = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect_err("corrupt queue should be fatal");

    assert_eq!(err.kind(), std::io::ErrorKind::Other);
}

#[cfg(unix)]
#[test]
fn daemon_run_once_does_not_rerun_live_execution_from_queue() {
    let root = temp_dir("run-once-live-execution");
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
    .started_at(rfc3339_before_now(1))
    .updated_at(rfc3339_before_now(1))
    .pid(Some(std::process::id()))
    .process_identity(None)
    .deadline_at(Some(rfc3339_after_now(60)))
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
    let config = super::super::common::standalone_config(&root);
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &exec_bin,
    })
    .expect("daemon run once");

    let queue_path = execution_queue::path_for(&state_dir);
    let queue_state = execution_queue::load_or_default(&queue_path).expect("queue state");
    assert!(!rerun_flag.exists());
    assert!(running_path.exists());
    assert_eq!(queue_state.items.len(), 1);
    assert_eq!(snapshot.queue_depth, 1);
    assert_eq!(snapshot.running_count, 1);
}

#[cfg(unix)]
#[test]
fn daemon_run_once_reports_active_when_execution_is_running() {
    let root = temp_dir("run-once-health-running");
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
    .started_at(rfc3339_before_now(1))
    .updated_at(rfc3339_before_now(1))
    .pid(Some(std::process::id()))
    .process_identity(None)
    .deadline_at(Some(rfc3339_after_now(60)))
    .current_step_id(Some("step_collect".to_string()))
    .attempt(Some(1))
    .cancel_requested_at(None)
    .kill_requested_at(None)
    .build();
    write_json_atomic(&running_path, &running_state).expect("write running state");

    let config = super::super::common::standalone_config(&root);
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once");

    assert_eq!(
        snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert_eq!(snapshot.running_count, 1);
    assert_eq!(snapshot.reporting_count, 0);
}

#[cfg(unix)]
#[test]
fn daemon_run_once_reports_active_when_reporting_backlog_exists() {
    let root = temp_dir("run-once-health-reporting");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    let reporting_path = reporting::path_for(&state_dir, "exec_reporting");
    reporting::store(
        &reporting_path,
        &reporting::ReportingState::new(
            "exec_reporting".to_string(),
            "act_001".to_string(),
            "digest_reporting".to_string(),
            "req_001".to_string(),
            "failed".to_string(),
            run_dir
                .join("actions")
                .join("exec_reporting")
                .join("result.json")
                .display()
                .to_string(),
            None,
            None,
            None,
            1,
            Some("2026-04-12T10:00:00Z".to_string()),
            Some("cp unavailable".to_string()),
        ),
    )
    .expect("store reporting state");

    let config = super::super::common::standalone_config(&root);
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once");

    assert_eq!(
        snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert_eq!(snapshot.queue_depth, 0);
    assert_eq!(snapshot.running_count, 0);
    assert_eq!(snapshot.reporting_count, 1);
}

#[cfg(unix)]
#[test]
fn daemon_run_once_quarantines_corrupt_running_execution_without_rerun() {
    let root = temp_dir("run-once-corrupt-running-no-rerun");
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

    let running_path = running::path_for(&state_dir, &submitted.execution_id);
    fs::write(&running_path, "{bad json").expect("write corrupt running state");

    let rerun_flag = root.join("rerun.flag");
    let exec_bin = write_exec_wrapper(
        &root,
        &format!("echo rerun > \"{}\"\nexit 88", rerun_flag.display()),
    );
    let config = super::super::common::standalone_config(&root);
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &exec_bin,
    })
    .expect("daemon run once");

    let queue_state =
        execution_queue::load_or_default(&execution_queue::path_for(&state_dir)).expect("queue");
    let quarantine_path = history::path_for(&state_dir, &submitted.execution_id);
    let quarantine: history::ExecutionHistoryRecord =
        read_json(&quarantine_path).expect("read quarantine record");
    let reporting_path = reporting::path_for(&state_dir, &submitted.execution_id);

    assert!(!rerun_flag.exists());
    assert_eq!(snapshot.queue_depth, 0);
    assert_eq!(snapshot.running_count, 0);
    assert!(queue_state.items.is_empty());
    assert_eq!(quarantine.state, "quarantined");
    assert_eq!(
        quarantine.plan_digest.as_deref(),
        Some(submitted.plan_digest.as_str())
    );
    assert!(quarantine.detail.contains("running state unreadable"));
    assert!(!running_path.exists());
    assert!(!reporting_path.exists());
}
