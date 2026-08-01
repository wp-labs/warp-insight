use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use std::{os::unix::fs::PermissionsExt, path::Path};

use serde::Deserialize;
use time::Duration as TimeDuration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use warp_insight_contracts::action_plan::{
    ActionPlanConstraints, ActionPlanContract, ActionPlanMeta, ActionPlanProgram, ActionPlanStep,
    ActionPlanTarget, ApprovalMode, RiskLevel,
};
use warp_insight_contracts::agent_config::{
    AgentConfigContract, AgentSection, ControlPlaneSection, ExecutionSection, LogFileInputSection,
    LogsFileOutputSection, LogsOutputSection, LogsSection, LogsTcpOutputSection, PathsSection,
    TelemetrySection,
};

#[derive(Debug, Deserialize)]
pub(crate) struct TestLogCheckpointState {
    pub(crate) files: Vec<TestTrackedFileCheckpoint>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TestTrackedFileCheckpoint {
    pub(crate) checkpoint_offset: u64,
}

pub(crate) fn temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("duration")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("warp-insightd-local-exec-{name}-{suffix}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[cfg(unix)]
pub(crate) fn test_exec_bin(root: &Path) -> PathBuf {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let wrapper = root.join("warp-insight-exec-wrapper.sh");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\ncd \"{}\"\nexec cargo run -q -p warp-insight-exec -- \"$@\"\n",
            workspace_root.display()
        ),
    )
    .expect("write wrapper");
    let mut perms = fs::metadata(&wrapper)
        .expect("wrapper metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&wrapper, perms).expect("set wrapper permissions");
    wrapper
}

#[cfg(unix)]
pub(crate) fn write_exec_wrapper(root: &Path, body: &str) -> PathBuf {
    let wrapper = root.join("warp-insight-exec-test-wrapper.sh");
    fs::write(&wrapper, format!("#!/bin/sh\n{body}\n")).expect("write wrapper");
    let mut perms = fs::metadata(&wrapper)
        .expect("wrapper metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&wrapper, perms).expect("set wrapper permissions");
    wrapper
}

pub(crate) fn sample_plan() -> ActionPlanContract {
    ActionPlanContract::new(
        ActionPlanMeta {
            action_id: "act_001".to_string(),
            request_id: "req_001".to_string(),
            template_id: None,
            tenant_id: "tenant_a".to_string(),
            environment_id: "prod-cn".to_string(),
            plan_version: 1,
            compiled_at: "2026-04-12T10:00:00Z".to_string(),
            expires_at: "2026-04-12T10:05:00Z".to_string(),
        },
        ActionPlanTarget {
            agent_id: "agent-001".to_string(),
            instance_id: Some("instance-001".to_string()),
            node_id: "node-001".to_string(),
            host_name: None,
            platform: "linux".to_string(),
            arch: "amd64".to_string(),
            selectors: Default::default(),
        },
        ActionPlanConstraints {
            risk_level: RiskLevel::R1,
            approval_ref: None,
            approval_mode: ApprovalMode::Required,
            requested_by: "alice@example.com".to_string(),
            reason: None,
            max_total_duration_ms: 30_000,
            step_timeout_default_ms: 10_000,
            execution_profile: "agent_exec_v1".to_string(),
            required_capabilities: vec!["process.list".to_string()],
        },
        ActionPlanProgram {
            entry: "step_collect".to_string(),
            steps: vec![ActionPlanStep {
                id: "step_collect".to_string(),
                kind: "invoke".to_string(),
                op: Some("process.list".to_string()),
            }],
        },
    )
}

pub(crate) fn sample_plan_with_ids(action_id: &str, request_id: &str) -> ActionPlanContract {
    let mut plan = sample_plan();
    plan.meta.action_id = action_id.to_string();
    plan.meta.request_id = request_id.to_string();
    plan
}

pub(crate) fn standalone_config(root: &std::path::Path) -> AgentConfigContract {
    AgentConfigContract::new(
        AgentSection {
            agent_id: Some("agent-001".to_string()),
            environment_id: Some("prod-cn".to_string()),
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
            root_dir: root.display().to_string(),
            run_dir: root.join("run").display().to_string(),
            state_dir: root.join("state").display().to_string(),
            log_dir: root.join("log").display().to_string(),
        },
        ExecutionSection {
            max_running_actions: 1,
            cancel_grace_ms: 5_000,
            default_stdout_limit_bytes: 1_048_576,
            default_stderr_limit_bytes: 1_048_576,
        },
    )
}

pub(crate) fn standalone_config_with_file_input(
    root: &std::path::Path,
    input_path: &std::path::Path,
) -> AgentConfigContract {
    standalone_config(root).with_telemetry(TelemetrySection {
        logs: LogsSection {
            file_inputs: vec![LogFileInputSection {
                input_id: "app".to_string(),
                path: input_path.display().to_string(),
                startup_position: "head".to_string(),
                multiline_mode: "none".to_string(),
            }],
            in_memory_buffer_bytes: 1_048_576,
            spool_dir: root
                .join("state")
                .join("spool")
                .join("logs")
                .display()
                .to_string(),
            output: LogsOutputSection {
                kind: "file".to_string(),
                file: LogsFileOutputSection {
                    path: root
                        .join("log")
                        .join("warp-parse-records.ndjson")
                        .display()
                        .to_string(),
                },
                ..LogsOutputSection::default()
            },
        },
    })
}

pub(crate) fn standalone_config_with_file_inputs(
    root: &std::path::Path,
    file_inputs: Vec<LogFileInputSection>,
) -> AgentConfigContract {
    standalone_config(root).with_telemetry(TelemetrySection {
        logs: LogsSection {
            file_inputs,
            in_memory_buffer_bytes: 1_048_576,
            spool_dir: root
                .join("state")
                .join("spool")
                .join("logs")
                .display()
                .to_string(),
            output: LogsOutputSection {
                kind: "file".to_string(),
                file: LogsFileOutputSection {
                    path: root
                        .join("log")
                        .join("warp-parse-records.ndjson")
                        .display()
                        .to_string(),
                },
                ..LogsOutputSection::default()
            },
        },
    })
}

pub(crate) fn standalone_config_with_tcp_file_input(
    root: &std::path::Path,
    input_path: &std::path::Path,
    addr: &str,
    port: u16,
    framing: &str,
) -> AgentConfigContract {
    standalone_config(root).with_telemetry(TelemetrySection {
        logs: LogsSection {
            file_inputs: vec![LogFileInputSection {
                input_id: "app".to_string(),
                path: input_path.display().to_string(),
                startup_position: "head".to_string(),
                multiline_mode: "none".to_string(),
            }],
            in_memory_buffer_bytes: 1_048_576,
            spool_dir: root
                .join("state")
                .join("spool")
                .join("logs")
                .display()
                .to_string(),
            output: LogsOutputSection {
                kind: "tcp".to_string(),
                tcp: LogsTcpOutputSection {
                    addr: addr.to_string(),
                    port,
                    framing: framing.to_string(),
                },
                ..LogsOutputSection::default()
            },
        },
    })
}

pub(crate) fn rfc3339_before_now(seconds: i64) -> String {
    (OffsetDateTime::now_utc() - TimeDuration::seconds(seconds))
        .format(&Rfc3339)
        .expect("format timestamp")
}

pub(crate) fn rfc3339_after_now(seconds: i64) -> String {
    (OffsetDateTime::now_utc() + TimeDuration::seconds(seconds))
        .format(&Rfc3339)
        .expect("format timestamp")
}
