use std::fs;
use std::path::PathBuf;

use warp_insight_contracts::action_plan::ActionPlanContract;
use warp_insight_contracts::action_result::{
    ActionOutputs, ActionResultContract, FinalStatus, StepActionRecord, StepStatus,
};
use warp_insight_contracts::agent_config::{
    AgentConfigContract, AgentSection, ControlPlaneSection, DiscoverySection, ExecutionSection,
    LogFileInputSection, LogsFileOutputSection, LogsOutputSection, LogsSection,
    LogsTcpOutputSection, PathsSection, TelemetrySection,
};
use warp_insight_contracts::gateway::{
    AckStatus, ActionPlanAck, DispatchActionPlan, ReportActionResult, ResultAttestation,
};
use warp_insight_contracts::state_exec::AgentRuntimeState;
use warp_insight_validate::action_plan::validate_action_plan;
use warp_insight_validate::action_result::validate_action_result;
use warp_insight_validate::config::validate_config;
use warp_insight_validate::gateway::{
    validate_action_plan_ack, validate_dispatch_action_plan, validate_report_action_result,
};
use warp_insight_validate::state::validate_execution_state;

fn fixture_text(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(relative);
    fs::read_to_string(path).expect("read fixture")
}

fn config_fixture(relative: &str) -> AgentConfigContract {
    toml::from_str(&fixture_text(relative)).expect("deserialize config fixture")
}

#[test]
fn action_plan_valid_fixture_passes() {
    let fixture: ActionPlanContract =
        serde_json::from_str(&fixture_text("contracts/action-plan/valid/basic.json"))
            .expect("deserialize action plan fixture");

    validate_action_plan(&fixture).expect("valid action plan");
}

#[test]
fn action_plan_invalid_kind_fixture_fails() {
    let fixture: ActionPlanContract =
        serde_json::from_str(&fixture_text("contracts/action-plan/invalid/bad-kind.json"))
            .expect("deserialize action plan fixture");

    let err = validate_action_plan(&fixture).expect_err("invalid action plan");
    assert_eq!(err.code, "invalid_kind");
}

#[test]
fn action_plan_invalid_window_fixture_fails() {
    let fixture: ActionPlanContract = serde_json::from_str(&fixture_text(
        "contracts/action-plan/invalid/expired-window.json",
    ))
    .expect("deserialize action plan fixture");

    let err = validate_action_plan(&fixture).expect_err("invalid action plan");
    assert_eq!(err.code, "expired_or_non_increasing_window");
}

#[test]
fn action_plan_invalid_step_kind_fixture_fails() {
    let fixture: ActionPlanContract = serde_json::from_str(&fixture_text(
        "contracts/action-plan/invalid/bad-step-kind.json",
    ))
    .expect("deserialize action plan fixture");

    let err = validate_action_plan(&fixture).expect_err("invalid action plan");
    assert_eq!(err.code, "invalid_step_kind");
}

#[test]
fn action_result_valid_fixture_passes() {
    let fixture: ActionResultContract =
        serde_json::from_str(&fixture_text("contracts/action-result/valid/basic.json"))
            .expect("deserialize action result fixture");

    validate_action_result(&fixture).expect("valid action result");
}

#[test]
fn action_result_invalid_fixture_fails() {
    let fixture: ActionResultContract = serde_json::from_str(&fixture_text(
        "contracts/action-result/invalid/missing-step-records.json",
    ))
    .expect("deserialize action result fixture");

    let err = validate_action_result(&fixture).expect_err("invalid action result");
    assert_eq!(err.code, "missing_step_records");
}

#[test]
fn config_valid_fixture_passes() {
    let fixture = config_fixture("contracts/config/valid/standalone.toml");

    validate_config(&fixture).expect("valid config");
}

#[test]
fn config_invalid_fixture_fails() {
    let fixture = config_fixture("contracts/config/invalid/managed-missing-endpoint.toml");

    let err = validate_config(&fixture).expect_err("invalid config");
    assert_eq!(err.code, "missing_control_plane_endpoint");
}

#[test]
fn runtime_state_valid_fixture_passes() {
    let fixture: AgentRuntimeState =
        serde_json::from_str(&fixture_text("contracts/state/runtime-valid.json"))
            .expect("deserialize runtime state fixture");

    validate_execution_state(&fixture).expect("valid runtime state");
}

fn sample_action_result(final_status: FinalStatus) -> ActionResultContract {
    ActionResultContract {
        api_version: "v1".to_string(),
        kind: "action_result".to_string(),
        action_id: "act_001".to_string(),
        execution_id: "exec_001".to_string(),
        request_id: Some("req_001".to_string()),
        final_status,
        exit_reason: None,
        step_records: vec![StepActionRecord {
            step_id: "step_collect".to_string(),
            attempt: 1,
            op: Some("process.list".to_string()),
            status: StepStatus::Succeeded,
            started_at: "2026-04-12T10:00:00Z".to_string(),
            finished_at: Some("2026-04-12T10:00:01Z".to_string()),
            duration_ms: Some(1),
            error_code: None,
            stdout_summary: None,
            stderr_summary: None,
            resource_usage: None,
        }],
        outputs: ActionOutputs::default(),
        resource_usage: None,
        started_at: Some("2026-04-12T10:00:00Z".to_string()),
        finished_at: Some("2026-04-12T10:00:01Z".to_string()),
    }
}

fn sample_action_plan() -> ActionPlanContract {
    serde_json::from_str(&fixture_text("contracts/action-plan/valid/basic.json"))
        .expect("deserialize action plan fixture")
}

fn sample_report_action_result() -> ReportActionResult {
    let result = sample_action_result(FinalStatus::Succeeded);
    ReportActionResult::new(
        "rep_001".to_string(),
        result.action_id.clone(),
        1,
        result.final_status,
        result.execution_id.clone(),
        "sha256:abc123".to_string(),
        "agent-001".to_string(),
        "instance-001".to_string(),
        ResultAttestation {
            result_digest: "sha256:def456".to_string(),
            signature: "sig_dev_placeholder".to_string(),
            issued_by: "dev-placeholder:agent-001".to_string(),
            attested_at: "2026-04-12T10:00:02Z".to_string(),
        },
        "2026-04-12T10:00:03Z".to_string(),
        result,
    )
}

#[test]
fn dispatch_action_plan_valid_fixture_passes() {
    let fixture = DispatchActionPlan::new("dsp_001".to_string(), sample_action_plan());

    validate_dispatch_action_plan(&fixture).expect("valid dispatch envelope");
}

#[test]
fn report_action_result_valid_fixture_passes() {
    let fixture = sample_report_action_result();

    validate_report_action_result(&fixture).expect("valid report envelope");
}

#[test]
fn report_action_result_with_mismatched_final_status_fails() {
    let mut fixture = sample_report_action_result();
    fixture.final_status = FinalStatus::Failed;

    let err = validate_report_action_result(&fixture).expect_err("invalid report envelope");
    assert_eq!(err.code, "mismatched_final_status");
}

#[test]
fn report_action_result_with_mismatched_execution_id_fails() {
    let mut fixture = sample_report_action_result();
    fixture.execution_id = "exec_other".to_string();

    let err = validate_report_action_result(&fixture).expect_err("invalid report envelope");
    assert_eq!(err.code, "mismatched_execution_id");
}

#[test]
fn action_plan_ack_queued_without_queue_position_fails() {
    let fixture = ActionPlanAck {
        api_version: "v1".to_string(),
        kind: "action_plan_ack".to_string(),
        dispatch_id: "dsp_001".to_string(),
        action_id: "act_001".to_string(),
        plan_digest: "sha256:abc123".to_string(),
        agent_id: "agent-001".to_string(),
        instance_id: "instance-001".to_string(),
        execution_id: Some("exec_001".to_string()),
        ack_status: AckStatus::Queued,
        reason_code: None,
        reason_message: None,
        queue_position: None,
        received_at: "2026-04-12T10:00:00Z".to_string(),
        acknowledged_at: "2026-04-12T10:00:01Z".to_string(),
    };

    let err = validate_action_plan_ack(&fixture).expect_err("invalid ack envelope");
    assert_eq!(err.code, "missing_queue_position");
}

#[test]
fn action_plan_ack_accepted_with_queue_position_fails() {
    let fixture = ActionPlanAck {
        api_version: "v1".to_string(),
        kind: "action_plan_ack".to_string(),
        dispatch_id: "dsp_001".to_string(),
        action_id: "act_001".to_string(),
        plan_digest: "sha256:abc123".to_string(),
        agent_id: "agent-001".to_string(),
        instance_id: "instance-001".to_string(),
        execution_id: Some("exec_001".to_string()),
        ack_status: AckStatus::Accepted,
        reason_code: None,
        reason_message: None,
        queue_position: Some(1),
        received_at: "2026-04-12T10:00:00Z".to_string(),
        acknowledged_at: "2026-04-12T10:00:01Z".to_string(),
    };

    let err = validate_action_plan_ack(&fixture).expect_err("invalid ack envelope");
    assert_eq!(err.code, "queue_position_not_allowed_for_accepted");
}

#[test]
fn action_result_succeeded_with_failed_step_fails() {
    let mut fixture = sample_action_result(FinalStatus::Succeeded);
    fixture.step_records[0].status = StepStatus::Failed;
    fixture.exit_reason = Some("exec_exit_1".to_string());

    let err = validate_action_result(&fixture).expect_err("invalid action result");
    assert_eq!(err.code, "succeeded_result_has_exit_reason");
}

#[test]
fn action_result_rejected_with_success_step_fails() {
    let fixture = sample_action_result(FinalStatus::Rejected);

    let err = validate_action_result(&fixture).expect_err("invalid action result");
    assert_eq!(err.code, "rejected_result_has_success_step");
}

#[test]
fn action_result_timed_out_without_timed_out_step_fails() {
    let fixture = sample_action_result(FinalStatus::TimedOut);

    let err = validate_action_result(&fixture).expect_err("invalid action result");
    assert_eq!(err.code, "timed_out_result_has_no_timed_out_step");
}

#[test]
fn action_result_cancelled_without_cancelled_step_fails() {
    let fixture = sample_action_result(FinalStatus::Cancelled);

    let err = validate_action_result(&fixture).expect_err("invalid action result");
    assert_eq!(err.code, "cancelled_result_has_no_cancelled_step");
}

#[test]
fn action_result_failed_without_failed_step_fails_when_only_cancelled() {
    let mut fixture = sample_action_result(FinalStatus::Failed);
    fixture.step_records[0].status = StepStatus::Cancelled;
    fixture.exit_reason = Some("cancelled_by_agent".to_string());

    let err = validate_action_result(&fixture).expect_err("invalid action result");
    assert_eq!(err.code, "failed_result_has_no_failed_step");
}

#[test]
fn action_result_failed_without_failed_step_fails_when_only_timed_out() {
    let mut fixture = sample_action_result(FinalStatus::Failed);
    fixture.step_records[0].status = StepStatus::TimedOut;
    fixture.exit_reason = Some("agentd_total_timeout".to_string());

    let err = validate_action_result(&fixture).expect_err("invalid action result");
    assert_eq!(err.code, "failed_result_has_no_failed_step");
}

#[test]
fn config_with_more_than_one_running_action_is_rejected() {
    let fixture = AgentConfigContract::new(
        AgentSection {
            agent_id: Some("agent-001".to_string()),
            environment_id: Some("prod".to_string()),
            instance_name: Some("instance-001".to_string()),
        },
        ControlPlaneSection {
            enabled: false,
            endpoint: None,
            enrollment_token: None,
            credential_request: None,
            credential_id: None,
            bearer_token: None,
            credential_expires_at: None,
            tls_mode: None,
            trust_bundle: None,
            auth_mode: None,
        },
        PathsSection {
            root_dir: "/tmp/root".to_string(),
            run_dir: "/tmp/root/run".to_string(),
            state_dir: "/tmp/root/state".to_string(),
            log_dir: "/tmp/root/log".to_string(),
        },
        ExecutionSection {
            max_running_actions: 2,
            cancel_grace_ms: 5_000,
            default_stdout_limit_bytes: 1024,
            default_stderr_limit_bytes: 1024,
        },
    );

    let err = validate_config(&fixture).expect_err("config should be rejected");
    assert_eq!(err.code, "unsupported_max_running_actions");
}

#[test]
fn config_with_all_discovery_probes_disabled_is_rejected() {
    let mut fixture = AgentConfigContract::new(
        AgentSection {
            agent_id: Some("agent-001".to_string()),
            environment_id: Some("prod".to_string()),
            instance_name: Some("instance-001".to_string()),
        },
        ControlPlaneSection {
            enabled: false,
            endpoint: None,
            enrollment_token: None,
            credential_request: None,
            credential_id: None,
            bearer_token: None,
            credential_expires_at: None,
            tls_mode: None,
            trust_bundle: None,
            auth_mode: None,
        },
        PathsSection {
            root_dir: "/tmp/root".to_string(),
            run_dir: "/tmp/root/run".to_string(),
            state_dir: "/tmp/root/state".to_string(),
            log_dir: "/tmp/root/log".to_string(),
        },
        ExecutionSection {
            max_running_actions: 1,
            cancel_grace_ms: 5_000,
            default_stdout_limit_bytes: 1024,
            default_stderr_limit_bytes: 1024,
        },
    );
    fixture.discovery = DiscoverySection {
        host_enabled: false,
        network_enabled: false,
        endpoint_enabled: false,
        process_enabled: false,
        container_enabled: false,
    };

    let err = validate_config(&fixture).expect_err("config should be rejected");
    assert_eq!(err.code, "missing_discovery_probe");
}

#[test]
fn config_with_duplicate_log_input_ids_is_rejected() {
    let fixture = AgentConfigContract::new(
        AgentSection {
            agent_id: Some("agent-001".to_string()),
            environment_id: Some("prod".to_string()),
            instance_name: Some("instance-001".to_string()),
        },
        ControlPlaneSection {
            enabled: false,
            endpoint: None,
            enrollment_token: None,
            credential_request: None,
            credential_id: None,
            bearer_token: None,
            credential_expires_at: None,
            tls_mode: None,
            trust_bundle: None,
            auth_mode: None,
        },
        PathsSection {
            root_dir: "/tmp/root".to_string(),
            run_dir: "/tmp/root/run".to_string(),
            state_dir: "/tmp/root/state".to_string(),
            log_dir: "/tmp/root/log".to_string(),
        },
        ExecutionSection {
            max_running_actions: 1,
            cancel_grace_ms: 5_000,
            default_stdout_limit_bytes: 1024,
            default_stderr_limit_bytes: 1024,
        },
    )
    .with_telemetry(TelemetrySection {
        logs: LogsSection {
            file_inputs: vec![
                LogFileInputSection {
                    input_id: "app".to_string(),
                    path: "/tmp/root/app.log".to_string(),
                    startup_position: "head".to_string(),
                    multiline_mode: "none".to_string(),
                },
                LogFileInputSection {
                    input_id: "app".to_string(),
                    path: "/tmp/root/other.log".to_string(),
                    startup_position: "head".to_string(),
                    multiline_mode: "none".to_string(),
                },
            ],
            in_memory_buffer_bytes: 1024,
            spool_dir: "/tmp/root/state/spool/logs".to_string(),
            output: LogsOutputSection {
                kind: "file".to_string(),
                file: LogsFileOutputSection {
                    path: "/tmp/root/log/records.ndjson".to_string(),
                },
                ..LogsOutputSection::default()
            },
        },
    });

    let err = validate_config(&fixture).expect_err("config should be rejected");
    assert_eq!(err.code, "duplicate_log_input_id");
}

#[test]
fn config_with_invalid_log_startup_position_is_rejected() {
    let fixture = AgentConfigContract::new(
        AgentSection {
            agent_id: Some("agent-001".to_string()),
            environment_id: Some("prod".to_string()),
            instance_name: Some("instance-001".to_string()),
        },
        ControlPlaneSection {
            enabled: false,
            endpoint: None,
            enrollment_token: None,
            credential_request: None,
            credential_id: None,
            bearer_token: None,
            credential_expires_at: None,
            tls_mode: None,
            trust_bundle: None,
            auth_mode: None,
        },
        PathsSection {
            root_dir: "/tmp/root".to_string(),
            run_dir: "/tmp/root/run".to_string(),
            state_dir: "/tmp/root/state".to_string(),
            log_dir: "/tmp/root/log".to_string(),
        },
        ExecutionSection {
            max_running_actions: 1,
            cancel_grace_ms: 5_000,
            default_stdout_limit_bytes: 1024,
            default_stderr_limit_bytes: 1024,
        },
    )
    .with_telemetry(TelemetrySection {
        logs: LogsSection {
            file_inputs: vec![LogFileInputSection {
                input_id: "app".to_string(),
                path: "/tmp/root/app.log".to_string(),
                startup_position: "middle".to_string(),
                multiline_mode: "none".to_string(),
            }],
            in_memory_buffer_bytes: 1024,
            spool_dir: "/tmp/root/state/spool/logs".to_string(),
            output: LogsOutputSection {
                kind: "file".to_string(),
                file: LogsFileOutputSection {
                    path: "/tmp/root/log/records.ndjson".to_string(),
                },
                ..LogsOutputSection::default()
            },
        },
    });

    let err = validate_config(&fixture).expect_err("config should be rejected");
    assert_eq!(err.code, "invalid_log_startup_position");
}

#[test]
fn config_with_invalid_tcp_output_framing_is_rejected() {
    let fixture = AgentConfigContract::new(
        AgentSection {
            agent_id: Some("agent-001".to_string()),
            environment_id: Some("prod".to_string()),
            instance_name: Some("instance-001".to_string()),
        },
        ControlPlaneSection {
            enabled: false,
            endpoint: None,
            enrollment_token: None,
            credential_request: None,
            credential_id: None,
            bearer_token: None,
            credential_expires_at: None,
            tls_mode: None,
            trust_bundle: None,
            auth_mode: None,
        },
        PathsSection {
            root_dir: "/tmp/root".to_string(),
            run_dir: "/tmp/root/run".to_string(),
            state_dir: "/tmp/root/state".to_string(),
            log_dir: "/tmp/root/log".to_string(),
        },
        ExecutionSection {
            max_running_actions: 1,
            cancel_grace_ms: 5_000,
            default_stdout_limit_bytes: 1024,
            default_stderr_limit_bytes: 1024,
        },
    )
    .with_telemetry(TelemetrySection {
        logs: LogsSection {
            file_inputs: vec![LogFileInputSection {
                input_id: "app".to_string(),
                path: "/tmp/root/app.log".to_string(),
                startup_position: "head".to_string(),
                multiline_mode: "none".to_string(),
            }],
            in_memory_buffer_bytes: 1024,
            spool_dir: "/tmp/root/state/spool/logs".to_string(),
            output: LogsOutputSection {
                kind: "tcp".to_string(),
                tcp: LogsTcpOutputSection {
                    addr: "127.0.0.1".to_string(),
                    port: 9000,
                    framing: "auto".to_string(),
                },
                ..LogsOutputSection::default()
            },
        },
    });

    let err = validate_config(&fixture).expect_err("config should be rejected");
    assert_eq!(err.code, "invalid_logs_output_tcp_framing");
}
