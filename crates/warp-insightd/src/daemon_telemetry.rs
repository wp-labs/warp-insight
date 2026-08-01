use std::io;
use std::path::PathBuf;

use warp_insight_contracts::agent_config::{AgentConfigContract, LogFileInputSection};

use crate::telemetry::logs::file_input::{FileInputProcessor, ProcessOutcome};
use crate::telemetry::warp_parse::RecordSink;

#[path = "daemon_telemetry_support.rs"]
mod support;

use support::{
    build_file_input_config, build_record_sink, invalid_output_failure, missing_input_failure,
    processing_failure, replay_spool_only,
};

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub(super) struct TelemetryTick {
    pub(super) outcomes: Vec<ProcessOutcome>,
    pub(super) failures: Vec<TelemetryFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TelemetryFailureKind {
    MissingInput,
    ProcessingFailed,
    InvalidOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub(super) struct TelemetryFailure {
    pub(super) kind: TelemetryFailureKind,
    pub(super) input_id: String,
    pub(super) path: String,
    pub(super) detail: String,
}

impl TelemetryTick {
    pub(super) fn is_active(&self) -> bool {
        !self.failures.is_empty()
            || self.outcomes.iter().any(|outcome| {
                outcome.records_processed > 0 || outcome.replayed_spool > 0 || outcome.spooled > 0
            })
    }
}

pub(super) async fn process_telemetry_inputs(config: &AgentConfigContract) -> TelemetryTick {
    let mut outcomes = Vec::new();
    let mut failures = Vec::new();
    let mut sink = match build_record_sink(config) {
        Ok(sink) => sink,
        Err(err) => {
            for input in &config.telemetry.logs.file_inputs {
                failures.push(invalid_output_failure(input, err.to_string()));
            }
            return TelemetryTick { outcomes, failures };
        }
    };

    for input in &config.telemetry.logs.file_inputs {
        process_telemetry_input(config, input, &mut sink, &mut outcomes, &mut failures).await;
    }

    TelemetryTick { outcomes, failures }
}

async fn process_telemetry_input<S: RecordSink>(
    config: &AgentConfigContract,
    input: &LogFileInputSection,
    sink: &mut S,
    outcomes: &mut Vec<ProcessOutcome>,
    failures: &mut Vec<TelemetryFailure>,
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
        Ok(outcome) => outcomes.push(outcome),
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
