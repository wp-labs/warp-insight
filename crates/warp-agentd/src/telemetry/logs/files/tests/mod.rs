use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use wist_shared::fs::{read_json, write_json_atomic};

use super::{FileInputConfig, FileInputProcessor, upsert_checkpoint};
use crate::state_store::log_checkpoint_state::LogCheckpointState;
use crate::state_store::log_checkpoints;
use crate::telemetry::logs::files::file_reader::{ObservedFileIdentity, ReadLimits};
use crate::telemetry::logs::files::file_watcher::StartupPosition;
use crate::telemetry::logs::multiline::MultilineMode;
use crate::telemetry::spool;
use crate::telemetry::warp_parse::RecordSink;

mod basic;
mod checkpoint_state;
mod missing;
mod multiline;
#[cfg(unix)]
mod permissions;
mod rotation;
mod spool_replay;

fn temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("duration")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("warp-agentd-file-input-{name}-{suffix}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[derive(Default)]
struct TestSink {
    records: Vec<wist_contracts::telemetry_record::TelemetryRecordContract>,
    fail_writes: bool,
}

impl RecordSink for TestSink {
    async fn write_records(
        &mut self,
        records: &[wist_contracts::telemetry_record::TelemetryRecordContract],
    ) -> io::Result<()> {
        if self.fail_writes {
            return Err(io::Error::other("sink unavailable"));
        }
        self.records.extend_from_slice(records);
        Ok(())
    }
}

fn config(root: &Path, source_path: &Path) -> FileInputConfig {
    FileInputConfig {
        agent_id: "agent-test".to_string(),
        input_id: "input-app".to_string(),
        source_path: source_path.to_path_buf(),
        state_dir: root.join("state"),
        spool_path: root.join("state").join("spool").join("input-app.ndjson"),
        startup_position: StartupPosition::Head,
        multiline_mode: MultilineMode::None,
        in_memory_budget_bytes: 4096,
        read_limits: ReadLimits::new(1_048_576, 4_194_304, 4096),
        spool_max_bytes: 268_435_456,
    }
}

fn read_output_records(
    path: &Path,
) -> Vec<wist_contracts::telemetry_record::TelemetryRecordContract> {
    fs::read_to_string(path)
        .expect("read output")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse record"))
        .collect()
}
