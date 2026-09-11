use std::io;
use std::path::{Path, PathBuf};

use wist_contracts::agent_config::{AgentConfigContract, LogFileInputSection};

use crate::telemetry::logs::files::file_reader::ReadLimits;
use crate::telemetry::logs::files::file_watcher::StartupPosition;
use crate::telemetry::logs::files::{FileInputConfig, ProcessOutcome};
use crate::telemetry::logs::multiline::MultilineMode;
use crate::telemetry::spool;
use crate::telemetry::warp_parse::{RecordSink, TelemetryRecordSink};

use super::{TelemetryFailure, TelemetryFailureKind};

pub(super) const SPOOL_REPLAY_BATCH_SIZE: usize = 128;

pub(super) fn build_record_sink(config: &AgentConfigContract) -> io::Result<TelemetryRecordSink> {
    TelemetryRecordSink::from_logs_output(&config.telemetry.logs.output)
}

pub(super) async fn replay_spool_only<S: RecordSink>(
    config: &AgentConfigContract,
    input: &LogFileInputSection,
    sink: &mut S,
) -> io::Result<Option<ProcessOutcome>> {
    let spool_path = spool_path_for(config, input);
    if !spool::has_records_async(&spool_path).await? {
        return Ok(None);
    }

    let replayed = spool::replay_records_async(&spool_path, sink, SPOOL_REPLAY_BATCH_SIZE).await?;
    Ok(Some(ProcessOutcome::spool_replay_only(replayed)))
}

pub(super) fn build_file_input_config(
    config: &AgentConfigContract,
    input: &LogFileInputSection,
    source_path: PathBuf,
) -> FileInputConfig {
    FileInputConfig {
        input_id: input.input_id.clone(),
        source_path,
        state_dir: PathBuf::from(&config.paths.state_dir),
        spool_path: spool_path_for(config, input),
        startup_position: startup_position_for(input),
        multiline_mode: multiline_mode_for(input),
        in_memory_budget_bytes: config.telemetry.logs.in_memory_buffer_bytes as usize,
        read_limits: ReadLimits::new(
            config.telemetry.logs.max_line_bytes as usize,
            config.telemetry.logs.max_read_bytes_per_tick as usize,
            config.telemetry.logs.max_lines_per_tick as usize,
        ),
        spool_max_bytes: config.telemetry.logs.spool_max_bytes,
    }
}

pub(super) fn invalid_output_failure(
    input: &LogFileInputSection,
    detail: String,
) -> TelemetryFailure {
    TelemetryFailure {
        kind: TelemetryFailureKind::InvalidOutput,
        input_id: input.input_id.clone(),
        path: input.path.clone(),
        detail,
    }
}

pub(super) fn missing_input_failure(input: &LogFileInputSection) -> TelemetryFailure {
    TelemetryFailure {
        kind: TelemetryFailureKind::MissingInput,
        input_id: input.input_id.clone(),
        path: input.path.clone(),
        detail: "source path does not exist".to_string(),
    }
}

pub(super) fn processing_failure(input: &LogFileInputSection, detail: String) -> TelemetryFailure {
    TelemetryFailure {
        kind: TelemetryFailureKind::ProcessingFailed,
        input_id: input.input_id.clone(),
        path: input.path.clone(),
        detail,
    }
}

pub(super) fn spool_paused_reason(spool_bytes: u64) -> String {
    format!("spool over limit ({spool_bytes} bytes); source read paused")
}

fn spool_path_for(config: &AgentConfigContract, input: &LogFileInputSection) -> PathBuf {
    Path::new(&config.telemetry.logs.spool_dir).join(format!("{}.ndjson", input.input_id))
}

fn multiline_mode_for(input: &LogFileInputSection) -> MultilineMode {
    match input.multiline_mode.as_str() {
        "indented" => MultilineMode::IndentedContinuation,
        _ => MultilineMode::None,
    }
}

fn startup_position_for(input: &LogFileInputSection) -> StartupPosition {
    match input.startup_position.as_str() {
        "tail" => StartupPosition::Tail,
        _ => StartupPosition::Head,
    }
}
