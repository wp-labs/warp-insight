use warp_insight_shared::fs::write_json_atomic;
use warp_insightd::bootstrap;
use warp_insightd::daemon;
use warp_insightd::scheduler::{SchedulerRequest, submit_local_plan};
#[cfg(target_os = "linux")]
use warp_insightd::state_store::execution_queue;
use warp_insightd::state_store::{reporting, running};

#[cfg(target_os = "linux")]
use super::super::common::test_exec_bin;
use super::super::common::{rfc3339_after_now, rfc3339_before_now, sample_plan, temp_dir};

#[cfg(unix)]
#[test]
fn recovery_skips_execution_when_pid_is_still_alive() {
    let root = temp_dir("recover-live-pid");
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

    daemon::recover_incomplete_executions(&state_dir, "instance-001").expect("recover");

    assert!(running_path.exists());
    assert!(!reporting_path.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn recovery_does_not_treat_pid_reuse_as_live_when_process_identity_mismatches() {
    let root = temp_dir("recover-mismatched-identity");
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
    .pid(Some(std::process::id()))
    .process_identity(Some("stale-process-identity".to_string()))
    .deadline_at(Some(rfc3339_before_now(5)))
    .current_step_id(Some("step_collect".to_string()))
    .attempt(Some(1))
    .cancel_requested_at(None)
    .kill_requested_at(None)
    .build();
    write_json_atomic(&running_path, &running_state).expect("write running state");

    daemon::recover_incomplete_executions(&state_dir, "instance-001").expect("recover");

    assert!(!running_path.exists());
    assert!(reporting_path.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn daemon_run_once_does_not_block_on_mismatched_process_identity() {
    let root = temp_dir("run-once-mismatched-identity");
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
    .process_identity(Some("stale-process-identity".to_string()))
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

    let queue_state =
        execution_queue::load_or_default(&execution_queue::path_for(&state_dir)).expect("queue");
    let reporting_path = reporting::path_for(&state_dir, &submitted.execution_id);

    assert_eq!(snapshot.queue_depth, 0);
    assert_eq!(snapshot.running_count, 0);
    assert!(queue_state.items.is_empty());
    assert!(!running_path.exists());
    assert!(reporting_path.exists());
}
