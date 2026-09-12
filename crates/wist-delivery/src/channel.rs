//! 接收端「质量处理通道」：接入层（ingress）的单一入口。
//!
//! 把记录进入 ETL 之前必须完成的质量动作串成一次调用：
//! `seq 去重 → 缺口检测 → 主动过滤（带外丢弃区间）→ 质量计数`。
//!
//! 调用方（gateway / center 数据平台）按 `agent_id` 分组，每个 agent 维护一个
//! [`QualityChannel`]。记录与丢弃区间喂入后，只把「被接受」的记录交给 ETL。

use crate::report::{DroppedRange, StreamLossReport};
use crate::watermark::{ObserveOutcome, WatermarkTracker};

/// 一条普通记录的处置结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acceptance {
    /// 接受（非重复、非丢失、非过滤），交给 ETL。
    Accepted,
    /// 重复（`seq` 命中）或已被过滤，丢弃。
    Rejected,
}

/// 单个 `agent_id` 流的质量处理状态。
#[derive(Debug, Clone)]
pub struct QualityChannel {
    watermark: WatermarkTracker,
    report: StreamLossReport,
    dropped_ranges: Vec<DroppedRange>,
}

impl QualityChannel {
    /// 新建通道。`lag` 为乱序容忍窗口大小。
    pub fn new(agent_id: impl Into<String>, lag: u64) -> Self {
        Self {
            watermark: WatermarkTracker::new(lag),
            report: StreamLossReport::new(agent_id),
            dropped_ranges: Vec::new(),
        }
    }

    /// 处理一条普通记录。
    ///
    /// 顺序：`seq` 去重 → 缺口判定 → 接受计数。
    pub fn on_record(&mut self, seq: u64) -> Acceptance {
        let result = self.watermark.observe(seq);
        if result.outcome == ObserveOutcome::Duplicate {
            return Acceptance::Rejected; // seq 重复
        }
        self.report.lost_count += result.lost.len() as u64;

        self.report.accepted_count += 1;
        Acceptance::Accepted
    }

    /// 处理一段带外丢弃区间（主动过滤，如 `debug` 内容级过滤）。
    ///
    /// 主动过滤不插 in-band 标记，按连续 `seq` 区间批量上报：推进水位线跳过该区间
    /// （不计丢失）、计入 `filtered_count`、并保留区间明细供对账。
    pub fn on_dropped_range(
        &mut self,
        range_start: u64,
        range_end: u64,
        drop_reason: impl Into<String>,
    ) {
        let range = DroppedRange::new(range_start, range_end, drop_reason);
        self.report.filtered_count += range.count();
        let lost = self.watermark.commit_range(range_start, range_end);
        self.report.lost_count += lost.len() as u64;
        self.dropped_ranges.push(range);
    }

    /// 交付计数快照（含带外丢弃区间明细，可序列化上报）。
    pub fn report(&self) -> StreamLossReport {
        let mut report = self.report.clone();
        report.dropped_ranges = self.dropped_ranges.clone();
        report
    }

    /// 带外丢弃区间明细。
    pub fn dropped_ranges(&self) -> &[DroppedRange] {
        &self.dropped_ranges
    }

    /// 当前已结算前缀（所有 `< committed` 已见 / 已判丢失 / 已过滤）。
    pub fn committed(&self) -> u64 {
        self.watermark.committed()
    }

    /// 当前窗口内尚未结算的已观测 `seq` 数（衡量乱序积压）。
    pub fn pending_count(&self) -> usize {
        self.watermark.pending_count()
    }
}

#[cfg(test)]
mod tests {
    use super::{Acceptance, QualityChannel};

    #[test]
    fn in_order_records_are_all_accepted() {
        let mut channel = QualityChannel::new("agent-001", 4);
        for seq in 0..5 {
            assert_eq!(channel.on_record(seq), Acceptance::Accepted);
        }
        let report = channel.report();
        assert_eq!(report.agent_id, "agent-001");
        assert_eq!(report.accepted_count, 5);
        assert_eq!(report.lost_count, 0);
        assert_eq!(report.filtered_count, 0);
        assert_eq!(channel.committed(), 5);
    }

    #[test]
    fn out_of_order_within_window_is_not_lost() {
        let mut channel = QualityChannel::new("agent-001", 4);
        channel.on_record(0);
        channel.on_record(2);
        let report = channel.report();
        assert_eq!(report.lost_count, 0);
        // seq 1 迟到仍接受。
        assert_eq!(channel.on_record(1), Acceptance::Accepted);
        assert_eq!(channel.committed(), 3);
    }

    #[test]
    fn gap_becomes_loss_only_after_window_slide() {
        let mut channel = QualityChannel::new("agent-001", 2);
        channel.on_record(0);
        channel.on_record(3);
        assert_eq!(channel.report().lost_count, 0);
        channel.on_record(4);
        // seq 1 滑出窗口仍缺失 → 判丢。
        assert_eq!(channel.report().lost_count, 1);
    }

    #[test]
    fn seq_duplicate_is_rejected() {
        let mut channel = QualityChannel::new("agent-001", 4);
        channel.on_record(0);
        assert_eq!(channel.on_record(0), Acceptance::Rejected);
        assert_eq!(channel.report().accepted_count, 1);
    }

    #[test]
    fn dropped_range_is_filtered_not_lost() {
        let mut channel = QualityChannel::new("agent-001", 2);
        channel.on_record(0);
        channel.on_record(1);
        // 丢弃 [2, 100]（高量级过滤），随后 101 到达，不应判丢。
        channel.on_dropped_range(2, 100, "debug");
        channel.on_record(101);

        let report = channel.report();
        assert_eq!(report.lost_count, 0);
        assert_eq!(report.filtered_count, 99);
        assert_eq!(report.accepted_count, 3);
        assert_eq!(report.dropped_ranges.len(), 1);
        assert_eq!(report.dropped_ranges[0].drop_reason, "debug");
    }

    #[test]
    fn dropped_range_reconciliation_gap_matching_range_is_not_loss() {
        // 丢弃区间与缺口重叠时，重叠部分不计丢失。
        let mut channel = QualityChannel::new("agent-001", 2);
        channel.on_record(0);
        // 缺口 seq 1 真实丢失，但 seq 2..5 为丢弃区间。
        channel.on_dropped_range(2, 5, "debug");
        channel.on_record(6);

        let report = channel.report();
        // 只有 seq 1 是真实丢失；2..5 被丢弃区间覆盖，不计丢失。
        assert_eq!(report.lost_count, 1);
        assert_eq!(report.filtered_count, 4);
    }
}
