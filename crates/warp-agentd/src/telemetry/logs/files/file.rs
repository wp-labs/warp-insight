//! End-to-end standalone file-input processing.

use std::io;
use std::path::PathBuf;

use wist_contracts::telemetry_record::TelemetryRecordContract;
use wist_shared::time::now_rfc3339;

use crate::state_store::log_checkpoint_state::{PendingMultilineState, TrackedFileCheckpoint};
use crate::state_store::log_checkpoints;
use crate::telemetry::logs::files::file_reader::{
    ReadLimits, inspect_path_async, read_from_offset_async,
};
use crate::telemetry::logs::files::file_watcher::{StartupPosition, decide_resume_async};
use crate::telemetry::logs::multiline::MultilineMode;
use crate::telemetry::spool;
use crate::telemetry::warp_parse::RecordSink;

#[path = "checkpoint_support.rs"]
mod checkpoint_support;
#[path = "delivery_support.rs"]
mod delivery_support;
#[path = "multiline_support.rs"]
mod multiline_support;
#[path = "state_support.rs"]
mod state_support;

use checkpoint_support::{
    checkpoint_for_path, find_rotated_path_async, relocate_checkpoint_path, upsert_checkpoint_async,
};

#[cfg(test)]
use checkpoint_support::upsert_checkpoint;
use delivery_support::{deliver_records, replay_spool_if_present};
use multiline_support::{
    flush_pending_if_source_changes, pending_should_flush, rebind_pending_source_on_rotate,
    records_from_pending, records_from_read,
};
use state_support::{CollectedReadBatch, DeliveryOutcome, PendingCheckpoint, RuntimeState};

const SPOOL_REPLAY_BATCH_SIZE: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct FileInputConfig {
    pub input_id: String,
    pub source_path: PathBuf,
    pub state_dir: PathBuf,
    pub spool_path: PathBuf,
    pub startup_position: StartupPosition,
    pub multiline_mode: MultilineMode,
    pub in_memory_budget_bytes: usize,
    /// 单次读取限制（长行截断 + 大文件分块回放）。
    pub read_limits: ReadLimits,
    /// spool 上限（字节），用于超限保护。
    pub spool_max_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessOutcomeKind {
    SourceBatch,
    SpoolReplayOnly,
    /// spool 已达上限且无法回放：暂停读源，保完整不丢数据。
    SpoolPaused,
}

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct ProcessOutcome {
    pub kind: ProcessOutcomeKind,
    pub records_processed: usize,
    pub emitted_directly: usize,
    pub spooled: usize,
    pub checkpoint_offset: u64,
    pub replayed_spool: usize,
    pub truncated: bool,
    /// 本次因超长被截断提交的行数（每行已作为一条记录提交）。
    pub truncated_lines: usize,
    pub rotated: bool,
    /// 是否因 spool 超限而暂停采集。
    pub paused: bool,
    /// 暂停时的 spool 字节数。
    pub spool_bytes: u64,
}

impl ProcessOutcome {
    fn from_delivery(
        delivery: DeliveryOutcome,
        checkpoint_offset: u64,
        replayed_spool: usize,
        truncated: bool,
        truncated_lines: usize,
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
            truncated_lines,
            rotated,
            paused: false,
            spool_bytes: 0,
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
            truncated_lines: 0,
            rotated: false,
            paused: false,
            spool_bytes: 0,
        }
    }

    /// spool 超限暂停结果。
    pub(crate) fn paused(spool_bytes: u64) -> Self {
        Self {
            kind: ProcessOutcomeKind::SpoolPaused,
            records_processed: 0,
            emitted_directly: 0,
            spooled: 0,
            checkpoint_offset: 0,
            replayed_spool: 0,
            truncated: false,
            truncated_lines: 0,
            rotated: false,
            paused: true,
            spool_bytes,
        }
    }
}

#[derive(::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
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
        match replay_spool_if_present(
            &mut self.sink,
            &self.config.spool_path,
            SPOOL_REPLAY_BATCH_SIZE,
        )
        .await
        {
            Ok(replayed) => runtime.replayed_spool = replayed,
            Err(err) => {
                // 回放失败：若 spool 已达上限则进入暂停（保完整、不丢数据），
                // 否则维持原有错误语义。
                return match self.paused_outcome_async().await? {
                    Some(paused) => Ok(paused),
                    None => Err(err),
                };
            }
        }
        let batch = self.collect_read_batch(&mut runtime).await?;
        let CollectedReadBatch {
            records,
            pending_multiline,
            checkpoints,
            checkpoint_offset,
            truncated_lines,
            resume,
        } = batch;
        let delivery = self.deliver_records_async(records).await?;
        self.commit_log_state(&mut runtime, checkpoints, pending_multiline)
            .await?;

        Ok(ProcessOutcome::from_delivery(
            delivery,
            checkpoint_offset,
            runtime.replayed_spool,
            resume.truncated,
            truncated_lines,
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
            log_state: log_checkpoints::load_or_default_from_path_async(
                &checkpoint_path,
                &self.config.input_id,
            )
            .await?,
            checkpoint_path,
            observed_at: now_rfc3339(),
            replayed_spool: 0,
        })
    }

    /// spool 达到上限时返回暂停结果（不读源、不推进 checkpoint）。
    ///
    /// 说明：`spool_over_limit = "drop_oldest"` 已可配置，但当前仅实现 `pause` 语义，
    /// 未实现按 input 优先级丢弃，以“保完整、不丢数据”优先。
    async fn paused_outcome_async(&self) -> io::Result<Option<ProcessOutcome>> {
        let spool_bytes = spool::size_async(&self.config.spool_path).await?;
        if spool_bytes >= self.config.spool_max_bytes {
            Ok(Some(ProcessOutcome::paused(spool_bytes)))
        } else {
            Ok(None)
        }
    }

    async fn collect_read_batch(
        &self,
        runtime: &mut RuntimeState,
    ) -> io::Result<CollectedReadBatch> {
        let current = inspect_path_async(&self.config.source_path).await?;
        let tracked = checkpoint_for_path(&runtime.log_state, &self.config.source_path);
        let resume = decide_resume_async(
            &self.config.source_path,
            &current,
            tracked.as_ref(),
            self.config.startup_position,
        )
        .await;
        let mut batch = CollectedReadBatch::new(resume);
        batch.pending_multiline = runtime.log_state.pending_multiline.take();
        let mut saw_new_lines = self
            .collect_rotated_tail(runtime, tracked.as_ref(), &mut batch)
            .await?;

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

        let active_read = read_from_offset_async(
            &self.config.source_path,
            batch.resume.start_offset,
            self.config.read_limits,
        )
        .await?;
        saw_new_lines |= !active_read.lines.is_empty();
        batch.truncated_lines += active_read.truncated_lines;
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

    async fn collect_rotated_tail(
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
        let Some(rotated_path) =
            find_rotated_path_async(&self.config.source_path, previous).await?
        else {
            return Ok(false);
        };

        relocate_checkpoint_path(&mut runtime.log_state, previous, &rotated_path);
        rebind_pending_source_on_rotate(
            &mut batch.pending_multiline,
            &self.config.source_path,
            &rotated_path,
        );
        let rotated_read = read_from_offset_async(
            &rotated_path,
            previous.checkpoint_offset,
            self.config.read_limits,
        )
        .await?;
        let saw_new_lines = !rotated_read.lines.is_empty();
        batch.truncated_lines += rotated_read.truncated_lines;
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

    async fn commit_log_state(
        &self,
        runtime: &mut RuntimeState,
        checkpoints: Vec<PendingCheckpoint>,
        pending_multiline: Option<PendingMultilineState>,
    ) -> io::Result<()> {
        for checkpoint in checkpoints {
            upsert_checkpoint_async(
                &mut runtime.log_state,
                &checkpoint.source_path,
                &checkpoint.identity,
                checkpoint.checkpoint_offset,
                &runtime.observed_at,
                checkpoint.rotated_from_path,
            )
            .await;
        }
        runtime.log_state.pending_multiline = pending_multiline;
        runtime.log_state.updated_at = runtime.observed_at.clone();
        log_checkpoints::store_async(&runtime.checkpoint_path, &runtime.log_state).await
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
#[path = "tests/mod.rs"]
mod tests;
