//! Build runtime capability reports from resolved config and current implementation limits.

use std::collections::BTreeSet;

use warp_insight_contracts::agent_config::AgentConfigContract;
use warp_insight_contracts::capability_report::{
    CapabilityLimits, CapabilityReportContract, CapabilityReportSections, ExecCapabilities,
    LogsCapabilities, MetricsCapabilities, UpgradeCapabilities,
};
use warp_insight_shared::time::now_rfc3339;

pub fn build_capability_report(config: &AgentConfigContract) -> CapabilityReportContract {
    CapabilityReportContract::new(CapabilityReportSections {
        agent_id: config
            .agent
            .agent_id
            .clone()
            .unwrap_or_else(|| "local-agent".to_string()),
        instance_id: config
            .agent
            .instance_name
            .clone()
            .unwrap_or_else(|| "local".to_string()),
        reported_at: now_rfc3339(),
        exec: exec_capabilities(),
        metrics: metrics_capabilities(),
        logs: logs_capabilities(config),
        upgrade: upgrade_capabilities(),
        limits: limits(config),
    })
}

fn exec_capabilities() -> ExecCapabilities {
    ExecCapabilities {
        opcodes: vec![
            "process.list".to_string(),
            "process.stat".to_string(),
            "socket.check".to_string(),
            "service.status".to_string(),
            "file.read_range".to_string(),
            "file.tail".to_string(),
            "config.inspect".to_string(),
            "agent.health_check".to_string(),
        ],
        execution_profiles: vec!["agent_exec_v1".to_string()],
    }
}

fn metrics_capabilities() -> MetricsCapabilities {
    MetricsCapabilities {
        discovery_modes: vec!["local_runtime".to_string()],
        ..MetricsCapabilities::default()
    }
}

fn logs_capabilities(config: &AgentConfigContract) -> Option<LogsCapabilities> {
    if config.telemetry.logs.file_inputs.is_empty() {
        return None;
    }

    let mut multiline_modes = BTreeSet::new();
    for input in &config.telemetry.logs.file_inputs {
        multiline_modes.insert(match input.multiline_mode.as_str() {
            "indented" => "indented".to_string(),
            _ => "off".to_string(),
        });
    }

    Some(LogsCapabilities {
        file_inputs: vec!["tail_file_v1".to_string()],
        parsers: vec!["raw".to_string()],
        multiline_modes: multiline_modes.into_iter().collect(),
        watcher_modes: vec!["poll".to_string()],
    })
}

fn upgrade_capabilities() -> UpgradeCapabilities {
    UpgradeCapabilities {
        supported: true,
        features: vec![
            "prepare".to_string(),
            "verify".to_string(),
            "rollback".to_string(),
        ],
    }
}

fn limits(config: &AgentConfigContract) -> CapabilityLimits {
    CapabilityLimits {
        max_running_actions: Some(config.execution.max_running_actions),
        max_stdout_bytes: Some(config.execution.default_stdout_limit_bytes),
        max_stderr_bytes: Some(config.execution.default_stderr_limit_bytes),
        max_memory_bytes: Some(config.telemetry.logs.in_memory_buffer_bytes),
        max_metrics_targets: None,
    }
}

#[cfg(test)]
mod tests {
    use super::build_capability_report;
    use warp_insight_contracts::agent_config::{
        AgentConfigContract, AgentSection, ControlPlaneSection, ExecutionSection,
        LogFileInputSection, LogsFileOutputSection, LogsOutputSection, LogsSection, PathsSection,
        TelemetrySection,
    };

    fn config_with_logs() -> AgentConfigContract {
        AgentConfigContract::new(
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
                tls_mode: None,
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
                default_stdout_limit_bytes: 1_048_576,
                default_stderr_limit_bytes: 1_048_576,
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
                        input_id: "stack".to_string(),
                        path: "/tmp/root/stack.log".to_string(),
                        startup_position: "head".to_string(),
                        multiline_mode: "indented".to_string(),
                    },
                ],
                in_memory_buffer_bytes: 65_536,
                spool_dir: "/tmp/root/state/spool/logs".to_string(),
                output: LogsOutputSection {
                    kind: "file".to_string(),
                    file: LogsFileOutputSection {
                        path: "/tmp/root/log/records.ndjson".to_string(),
                    },
                    ..LogsOutputSection::default()
                },
            },
        })
    }

    #[test]
    fn build_capability_report_includes_logs_when_file_inputs_are_configured() {
        let report = build_capability_report(&config_with_logs());

        let logs = report.logs.expect("logs capabilities");
        assert_eq!(report.agent_id, "agent-001");
        assert_eq!(report.instance_id, "instance-001");
        assert_eq!(report.exec.execution_profiles, vec!["agent_exec_v1"]);
        assert_eq!(report.metrics.discovery_modes, vec!["local_runtime"]);
        assert_eq!(logs.file_inputs, vec!["tail_file_v1"]);
        assert_eq!(logs.parsers, vec!["raw"]);
        assert_eq!(logs.watcher_modes, vec!["poll"]);
        assert_eq!(
            logs.multiline_modes,
            vec!["indented".to_string(), "off".to_string()]
        );
        assert_eq!(report.limits.max_memory_bytes, Some(65_536));
    }

    #[test]
    fn build_capability_report_omits_logs_when_no_file_inputs_are_enabled() {
        let report = build_capability_report(&AgentConfigContract::new(
            AgentSection {
                agent_id: Some("agent-001".to_string()),
                environment_id: None,
                instance_name: Some("instance-001".to_string()),
            },
            ControlPlaneSection {
                enabled: false,
                endpoint: None,
                enrollment_token: None,
                credential_request: None,
                tls_mode: None,
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
                default_stdout_limit_bytes: 1_048_576,
                default_stderr_limit_bytes: 1_048_576,
            },
        ));

        assert!(report.logs.is_none());
        assert_eq!(report.metrics.discovery_modes, vec!["local_runtime"]);
    }
}
