//! End-to-end standalone file-input processing.

use std::io;
use std::path::PathBuf;

use warp_insight_contracts::telemetry_record::TelemetryRecordContract;
use warp_insight_shared::time::now_rfc3339;

use crate::state_store::log_checkpoint_state::{PendingMultilineState, TrackedFileCheckpoint};
use crate::state_store::log_checkpoints;
use crate::telemetry::logs::file_reader::{inspect_path, read_from_offset};
use crate::telemetry::logs::file_watcher::{StartupPosition, decide_resume};
use crate::telemetry::logs::multiline::MultilineMode;
use crate::telemetry::warp_parse::RecordSink;

#[path = "file_input_checkpoint_support.rs"]
mod checkpoint_support;
#[path = "file_input_delivery_support.rs"]
mod delivery_support;
#[path = "file_input_multiline_support.rs"]
mod multiline_support;
#[path = "file_input_state.rs"]
mod state_support;

use checkpoint_support::{
    checkpoint_for_path, find_rotated_path, relocate_checkpoint_path, upsert_checkpoint,
};
use delivery_support::{deliver_records, replay_spool_if_present};
use multiline_support::{
    flush_pending_if_source_changes, pending_should_flush, rebind_pending_source_on_rotate,
    records_from_pending, records_from_read,
};
use state_support::{CollectedReadBatch, DeliveryOutcome, PendingCheckpoint, RuntimeState};

const SPOOL_REPLAY_BATCH_SIZE: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct FileInputConfig {
    pub input_id: String,
    pub source_path: PathBuf,
    pub state_dir: PathBuf,
    pub spool_path: PathBuf,
    pub startup_position: StartupPosition,
    pub multiline_mode: MultilineMode,
    pub in_memory_budget_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessOutcomeKind {
    SourceBatch,
    SpoolReplayOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct ProcessOutcome {
    pub kind: ProcessOutcomeKind,
    pub records_processed: usize,
    pub emitted_directly: usize,
    pub spooled: usize,
    pub checkpoint_offset: u64,
    pub replayed_spool: usize,
    pub truncated: bool,
    pub rotated: bool,
}

impl ProcessOutcome {
    fn from_delivery(
        delivery: DeliveryOutcome,
        checkpoint_offset: u64,
        replayed_spool: usize,
        truncated: bool,
        rotated: bool,
    ) -> Self {
        Self {
            kind: ProcessOutcomeKind::SourceBatch,
            records_processed: delivery.records_processed,
            emitted_directly: delivery.emitted_directly,
            spooled: delivery.spooled,
            checkpoint_offset,
            replayed_spool,
            truncated,
            rotated,
        }
    }

    pub(crate) fn spool_replay_only(replayed_spool: usize) -> Self {
        Self {
            kind: ProcessOutcomeKind::SpoolReplayOnly,
            records_processed: 0,
            emitted_directly: 0,
            spooled: 0,
            checkpoint_offset: 0,
            replayed_spool,
            truncated: false,
            rotated: false,
        }
    }
}

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct FileInputProcessor<S> {
    config: FileInputConfig,
    sink: S,
}

impl<S> FileInputProcessor<S>
where
    S: RecordSink,
{
    pub fn new(config: FileInputConfig, sink: S) -> Self {
        Self { config, sink }
    }

    pub async fn process_once_async(&mut self) -> io::Result<ProcessOutcome> {
        let mut runtime = self.load_runtime_state_async().await?;
        let batch = self.collect_read_batch(&mut runtime)?;
        let CollectedReadBatch {
            records,
            pending_multiline,
            checkpoints,
            checkpoint_offset,
            resume,
        } = batch;
        let delivery = self.deliver_records_async(records).await?;
        self.commit_log_state(&mut runtime, checkpoints, pending_multiline)?;

        Ok(ProcessOutcome::from_delivery(
            delivery,
            checkpoint_offset,
            runtime.replayed_spool,
            resume.truncated,
            resume.rotated,
        ))
    }

    #[cfg(test)]
    pub fn process_once(&mut self) -> io::Result<ProcessOutcome> {
        block_on_io(self.process_once_async())
    }

    async fn load_runtime_state_async(&mut self) -> io::Result<RuntimeState> {
        let checkpoint_path =
            log_checkpoints::path_for(&self.config.state_dir, &self.config.input_id);
        Ok(RuntimeState {
            log_state: log_checkpoints::load_or_default_from_path(
                &checkpoint_path,
                &self.config.input_id,
            )?,
            checkpoint_path,
            observed_at: now_rfc3339(),
            replayed_spool: replay_spool_if_present(
                &mut self.sink,
                &self.config.spool_path,
                SPOOL_REPLAY_BATCH_SIZE,
            )
            .await?,
        })
    }

    fn collect_read_batch(&self, runtime: &mut RuntimeState) -> io::Result<CollectedReadBatch> {
        let current = inspect_path(&self.config.source_path)?;
        let tracked = checkpoint_for_path(&runtime.log_state, &self.config.source_path);
        let resume = decide_resume(
            &self.config.source_path,
            &current,
            tracked.as_ref(),
            self.config.startup_position,
        );
        let mut batch = CollectedReadBatch::new(resume);
        batch.pending_multiline = runtime.log_state.pending_multiline.take();
        let mut saw_new_lines = self.collect_rotated_tail(runtime, tracked.as_ref(), &mut batch)?;

        if batch.resume.rotated || batch.resume.truncated {
            batch.records.extend(records_from_pending(
                &runtime.observed_at,
                &self.config.input_id,
                batch.pending_multiline.take(),
            ));
        } else {
            flush_pending_if_source_changes(
                &mut batch.records,
                &mut batch.pending_multiline,
                &runtime.observed_at,
                &self.config.input_id,
                &self.config.source_path,
            );
        }

        let active_read = read_from_offset(&self.config.source_path, batch.resume.start_offset)?;
        saw_new_lines |= !active_read.lines.is_empty();
        batch.pending_multiline = records_from_read(
            &mut batch.records,
            &runtime.observed_at,
            &self.config.input_id,
            &self.config.source_path,
            self.config.multiline_mode,
            active_read.lines,
            batch.pending_multiline,
        );
        batch.checkpoint_offset = active_read.committed_end_offset;
        batch.checkpoints.push(PendingCheckpoint {
            source_path: self.config.source_path.clone(),
            identity: active_read.identity,
            checkpoint_offset: batch.checkpoint_offset,
            rotated_from_path: batch.resume.rotated_from_path.clone(),
        });

        if !saw_new_lines
            && pending_should_flush(batch.pending_multiline.as_ref(), &runtime.observed_at)
        {
            batch.records.extend(records_from_pending(
                &runtime.observed_at,
                &self.config.input_id,
                batch.pending_multiline.take(),
            ));
        }

        Ok(batch)
    }

    fn collect_rotated_tail(
        &self,
        runtime: &mut RuntimeState,
        tracked: Option<&TrackedFileCheckpoint>,
        batch: &mut CollectedReadBatch,
    ) -> io::Result<bool> {
        if !batch.resume.rotated {
            return Ok(false);
        }
        let Some(previous) = tracked else {
            return Ok(false);
        };
        let Some(rotated_path) = find_rotated_path(&self.config.source_path, previous)? else {
            return Ok(false);
        };

        relocate_checkpoint_path(&mut runtime.log_state, previous, &rotated_path);
        rebind_pending_source_on_rotate(
            &mut batch.pending_multiline,
            &self.config.source_path,
            &rotated_path,
        );
        let rotated_read = read_from_offset(&rotated_path, previous.checkpoint_offset)?;
        let saw_new_lines = !rotated_read.lines.is_empty();
        batch.pending_multiline = records_from_read(
            &mut batch.records,
            &runtime.observed_at,
            &self.config.input_id,
            &rotated_path,
            self.config.multiline_mode,
            rotated_read.lines,
            batch.pending_multiline.take(),
        );
        batch.checkpoints.push(PendingCheckpoint {
            source_path: rotated_path,
            identity: rotated_read.identity,
            checkpoint_offset: rotated_read.committed_end_offset,
            rotated_from_path: None,
        });
        Ok(saw_new_lines)
    }

    async fn deliver_records_async(
        &mut self,
        records: Vec<TelemetryRecordContract>,
    ) -> io::Result<DeliveryOutcome> {
        deliver_records(
            &mut self.sink,
            &self.config.spool_path,
            self.config.in_memory_budget_bytes,
            records,
        )
        .await
    }

    fn commit_log_state(
        &self,
        runtime: &mut RuntimeState,
        checkpoints: Vec<PendingCheckpoint>,
        pending_multiline: Option<PendingMultilineState>,
    ) -> io::Result<()> {
        for checkpoint in checkpoints {
            upsert_checkpoint(
                &mut runtime.log_state,
                &checkpoint.source_path,
                &checkpoint.identity,
                checkpoint.checkpoint_offset,
                &runtime.observed_at,
                checkpoint.rotated_from_path,
            );
        }
        runtime.log_state.pending_multiline = pending_multiline;
        runtime.log_state.updated_at = runtime.observed_at.clone();
        log_checkpoints::store(&runtime.checkpoint_path, &runtime.log_state)
    }
}

#[cfg(test)]
fn block_on_io<T>(future: impl std::future::Future<Output = io::Result<T>>) -> io::Result<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(future)
}

#[cfg(test)]
#[path = "file_input_tests/mod.rs"]
mod tests;
