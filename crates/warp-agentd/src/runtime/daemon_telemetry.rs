use std::io;
use std::path::PathBuf;

use wist_contracts::agent_config::{AgentConfigContract, LogFileInputSection};
use wist_shared::time::now_rfc3339;

use crate::telemetry::logs::files::{FileInputProcessor, ProcessOutcome};
use crate::telemetry::warp_parse::RecordSink;

#[path = "daemon_telemetry_support.rs"]
mod support;

use support::{
    build_file_input_config, build_record_sink, invalid_output_failure, missing_input_failure,
    processing_failure, replay_spool_only, spool_paused_reason,
};

#[derive(::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub(super) struct TelemetryTick {
    pub(super) outcomes: Vec<ProcessOutcome>,
    pub(super) failures: Vec<TelemetryFailure>,
    /// 本 tick 处于暂停（spool 超限）的输入，作为“当前状态事实”供 daemon 跨 tick 差值。
    pub(super) notifications: Vec<TelemetryWorkState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TelemetryFailureKind {
    MissingInput,
    ProcessingFailed,
    InvalidOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub(super) struct TelemetryFailure {
    pub(super) kind: TelemetryFailureKind,
    pub(super) input_id: String,
    pub(super) path: String,
    pub(super) detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WorkState {
    Paused,
    Resumed,
}

/// 工作状态通知（非告警、非失败）。`Paused` 由采集 tick 产生，`Resumed` 由 daemon 跨 tick 差值合成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TelemetryWorkState {
    pub(super) input_id: String,
    pub(super) state: WorkState,
    pub(super) reason: String,
    pub(super) at: String,
}

impl TelemetryTick {
    pub(super) fn is_active(&self) -> bool {
        !self.failures.is_empty()
            || self.outcomes.iter().any(|outcome| {
                outcome.records_processed > 0
                    || outcome.replayed_spool > 0
                    || outcome.spooled > 0
                    || outcome.paused
            })
    }
}

pub(super) async fn process_telemetry_inputs(config: &AgentConfigContract) -> TelemetryTick {
    let mut outcomes = Vec::new();
    let mut failures = Vec::new();
    let mut notifications = Vec::new();
    let mut sink = match build_record_sink(config) {
        Ok(sink) => sink,
        Err(err) => {
            for input in &config.telemetry.logs.file_inputs {
                failures.push(invalid_output_failure(input, err.to_string()));
            }
            return TelemetryTick {
                outcomes,
                failures,
                notifications,
            };
        }
    };

    for input in &config.telemetry.logs.file_inputs {
        process_telemetry_input(
            config,
            input,
            &mut sink,
            &mut outcomes,
            &mut failures,
            &mut notifications,
        )
        .await;
    }

    TelemetryTick {
        outcomes,
        failures,
        notifications,
    }
}

async fn process_telemetry_input<S: RecordSink>(
    config: &AgentConfigContract,
    input: &LogFileInputSection,
    sink: &mut S,
    outcomes: &mut Vec<ProcessOutcome>,
    failures: &mut Vec<TelemetryFailure>,
    notifications: &mut Vec<TelemetryWorkState>,
) {
    let source_path = PathBuf::from(&input.path);
    if !source_path.exists() {
        failures.push(missing_input_failure(input));
        match replay_spool_only(config, input, sink).await {
            Ok(Some(outcome)) => outcomes.push(outcome),
            Ok(None) => {}
            Err(err) => failures.push(processing_failure(
                input,
                format!("failed to replay spool: {err}"),
            )),
        }
        return;
    }

    match process_input_with_sink(config, input, source_path, sink).await {
        Ok(outcome) => {
            if outcome.paused {
                notifications.push(TelemetryWorkState {
                    input_id: input.input_id.clone(),
                    state: WorkState::Paused,
                    reason: spool_paused_reason(outcome.spool_bytes),
                    at: now_rfc3339(),
                });
            }
            outcomes.push(outcome);
        }
        Err(err) => failures.push(processing_failure(input, err.to_string())),
    }
}

async fn process_input_with_sink<S: RecordSink>(
    config: &AgentConfigContract,
    input: &LogFileInputSection,
    source_path: PathBuf,
    sink: &mut S,
) -> io::Result<ProcessOutcome> {
    let mut processor =
        FileInputProcessor::new(build_file_input_config(config, input, source_path), sink);
    processor.process_once_async().await
}
