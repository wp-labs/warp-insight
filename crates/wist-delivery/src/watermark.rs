//! 乱序容忍的有界窗口缺口检测（watermark）。
//!
//! 单个 `agent_id` 流的 `seq` 单调递增，但多连接 / 多 gateway /
//! 异步重放下**不保证按序到达**。本结构用有界窗口（`lag`）容忍乱序：
//! 只有某 `seq` 滑出窗口下界仍未到达，才判定丢失（延迟确认）。
//!
//! 除普通记录外，本结构还支持**带外丢弃区间**（见 [`WatermarkTracker::commit_range`]）：
//! 内容级过滤（如 debug）产生的高量级丢弃不在数据流里插 in-band 标记，而是按连续 `seq`
//! 区间一次性上报，让水位线跳过该区间、不误判为丢失。

use std::collections::{BTreeMap, BTreeSet};

/// 观测一条 `seq` 后的判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObserveOutcome {
    /// 新记录，接受。
    New,
    /// 重复：该 `seq` 已结算（`< committed`）、已在窗口内、或已被带外丢弃区间覆盖。
    Duplicate,
}

/// 观测结果：判定 + 本次判定为丢失的 `seq`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveResult {
    pub outcome: ObserveOutcome,
    /// 本次观测推进窗口后，被判定为丢失的 `seq`（升序）。
    pub lost: Vec<u64>,
}

/// 单个 `agent_id` 流的缺口检测器。
///
/// 状态语义：
/// - `committed`：已结算的连续前缀上界，所有 `< committed` 的 `seq` 要么已见、要么已判丢失、
///   要么已被带外丢弃区间覆盖。
/// - `observed`：窗口内已观测到、但尚未结算的 `seq`（≥ `committed`）。
/// - `high`：已观测到的最大 `seq`（含带外丢弃区间上界）。
/// - `dropped_ranges`：尚未结算的带外丢弃区间（`start -> end`，含两端，互不重叠）。
///
/// 当 `high - committed > lag`（窗口溢出）时，`committed` 处的缺口（必为缺失）被判定丢失并强制推进。
#[derive(Debug, Clone)]
pub struct WatermarkTracker {
    lag: u64,
    committed: u64,
    observed: BTreeSet<u64>,
    high: u64,
    dropped_ranges: BTreeMap<u64, u64>,
}

impl WatermarkTracker {
    /// 新建检测器。`lag` 为乱序容忍窗口（`lag = 0` 退化为严格顺序，见 `observe` 后立即判丢）。
    pub fn new(lag: u64) -> Self {
        Self {
            lag,
            committed: 0,
            observed: BTreeSet::new(),
            high: 0,
            dropped_ranges: BTreeMap::new(),
        }
    }

    /// 以指定已结算前缀新建（用于中途接入已有流，避免把历史 `seq` 误判为丢失）。
    pub fn with_committed(lag: u64, committed: u64) -> Self {
        Self {
            lag,
            committed,
            observed: BTreeSet::new(),
            high: committed,
            dropped_ranges: BTreeMap::new(),
        }
    }

    /// 观测一条 `seq`，返回判定与本次判定的丢失。
    pub fn observe(&mut self, seq: u64) -> ObserveResult {
        // 已结算（迟到/重复）。
        if seq < self.committed {
            return ObserveResult {
                outcome: ObserveOutcome::Duplicate,
                lost: Vec::new(),
            };
        }
        // 已在窗口内（乱序重复）。
        if self.observed.contains(&seq) {
            return ObserveResult {
                outcome: ObserveOutcome::Duplicate,
                lost: Vec::new(),
            };
        }
        // 已被带外丢弃区间覆盖（不应再以普通记录出现）。
        if self.is_in_dropped_range(seq) {
            return ObserveResult {
                outcome: ObserveOutcome::Duplicate,
                lost: Vec::new(),
            };
        }

        self.observed.insert(seq);
        self.high = self.high.max(seq);

        let lost = self.settle();
        ObserveResult {
            outcome: ObserveOutcome::New,
            lost,
        }
    }

    /// 标记一段连续 `seq` 为「带外丢弃」，推进水位线并返回本次判定的丢失。
    ///
    /// 与 `observe` 的区别：不把区间内每条 `seq` 塞进 `observed`（避免高量级过滤时
    /// 内存/时间随丢弃量线性膨胀），而是以区间形式合并记录、批量跳过。
    ///
    /// - `end < committed`：区间已结算，无操作。
    /// - 区间与已结算前缀相邻时：直接推进 `committed` 跳过，不判丢。
    /// - 区间与已结算前缀之间仍有未到记录时：该缺口按普通窗口语义处理（可能判丢）。
    pub fn commit_range(&mut self, start: u64, end: u64) -> Vec<u64> {
        if end < start || end < self.committed {
            return Vec::new();
        }
        let start = start.max(self.committed);

        self.insert_dropped_range(start, end);
        self.high = self.high.max(end);

        self.settle()
    }

    /// 当前已结算前缀（所有 `< committed` 已见或已判丢失）。
    pub fn committed(&self) -> u64 {
        self.committed
    }

    /// 当前窗口内尚未结算的已观测 `seq` 数（衡量乱序积压）。
    pub fn pending_count(&self) -> usize {
        self.observed.len()
    }

    /// 尚未结算的带外丢弃区间数（衡量未消费的过滤区间积压）。
    pub fn pending_ranges(&self) -> usize {
        self.dropped_ranges.len()
    }

    fn is_in_dropped_range(&self, seq: u64) -> bool {
        self.dropped_ranges
            .range(..=seq)
            .next_back()
            .map(|(_, &end)| seq <= end)
            .unwrap_or(false)
    }

    fn insert_dropped_range(&mut self, start: u64, end: u64) {
        let mut s = start;
        let mut e = end;
        // 与已有区间重叠或相邻（`re + 1 >= s` 且 `rs <= e + 1`）则合并，保持互不重叠。
        let overlapped: Vec<u64> = self
            .dropped_ranges
            .range(..=e.saturating_add(1))
            .filter_map(|(&rs, &re)| {
                if re.saturating_add(1) >= s {
                    s = s.min(rs);
                    e = e.max(re);
                    Some(rs)
                } else {
                    None
                }
            })
            .collect();
        for key in overlapped {
            self.dropped_ranges.remove(&key);
        }
        self.dropped_ranges.insert(s, e);
    }

    /// 结算连续可见前缀 + 带外丢弃区间前缀 + 窗口滑动的丢失判定。
    fn settle(&mut self) -> Vec<u64> {
        let mut lost = Vec::new();
        loop {
            // 1. 结算连续可见前缀。
            while self.observed.remove(&self.committed) {
                self.committed += 1;
            }
            // 2. 结算覆盖 `committed` 的带外丢弃区间（跳过整段，不判丢）。
            if let Some((&start, &end)) = self.dropped_ranges.range(..=self.committed).next_back() {
                if self.committed <= end {
                    self.dropped_ranges.remove(&start);
                    self.committed = end + 1;
                    continue;
                }
            }
            // 3. 窗口溢出：`committed` 落后 `high` 超过 `lag`，该缺口判定丢失。
            if self.high.saturating_sub(self.committed) > self.lag {
                lost.push(self.committed);
                self.committed += 1;
            } else {
                break;
            }
        }
        lost
    }
}

#[cfg(test)]
mod tests {
    use super::{ObserveOutcome, WatermarkTracker};

    #[test]
    fn in_order_commits_without_loss() {
        let mut tracker = WatermarkTracker::new(4);
        for seq in 0..5 {
            let result = tracker.observe(seq);
            assert_eq!(result.outcome, ObserveOutcome::New);
            assert!(result.lost.is_empty());
        }
        assert_eq!(tracker.committed(), 5);
        assert_eq!(tracker.pending_count(), 0);
    }

    #[test]
    fn out_of_order_within_window_is_not_lost() {
        let mut tracker = WatermarkTracker::new(4);
        tracker.observe(0);
        tracker.observe(2);
        // seq 1 迟到但仍在窗口内，不算丢失。
        let result = tracker.observe(1);
        assert_eq!(result.outcome, ObserveOutcome::New);
        assert!(result.lost.is_empty());
        assert_eq!(tracker.committed(), 3);
    }

    #[test]
    fn gap_is_declared_lost_only_after_window_slides_past() {
        let mut tracker = WatermarkTracker::new(2);
        tracker.observe(0);
        let result = tracker.observe(3);
        // 窗口 lag=2：此时 high=3, committed=1，缺口尚未滑过，不判丢。
        assert!(result.lost.is_empty());

        // seq 4 推进窗口，seq 1 滑出窗口仍缺失 → 判丢。
        let result = tracker.observe(4);
        assert_eq!(result.lost, vec![1]);
    }

    #[test]
    fn late_arrival_after_committed_is_duplicate() {
        let mut tracker = WatermarkTracker::new(2);
        tracker.observe(0);
        tracker.observe(3);
        tracker.observe(4); // seq 1 判丢，committed 推进到 2
        let result = tracker.observe(1);
        assert_eq!(result.outcome, ObserveOutcome::Duplicate);
        assert!(result.lost.is_empty());
    }

    #[test]
    fn duplicate_within_window_is_rejected() {
        let mut tracker = WatermarkTracker::new(4);
        tracker.observe(0);
        tracker.observe(2);
        let result = tracker.observe(2);
        assert_eq!(result.outcome, ObserveOutcome::Duplicate);
    }

    #[test]
    fn mid_stream_join_uses_committed_prefix() {
        // 已结算前缀 10：seq 0..10 视为已处理，从 10 续接。
        let mut tracker = WatermarkTracker::with_committed(4, 10);
        let result = tracker.observe(10);
        assert_eq!(result.outcome, ObserveOutcome::New);
        assert!(result.lost.is_empty());
        assert_eq!(tracker.committed(), 11);

        let result = tracker.observe(11);
        assert_eq!(result.outcome, ObserveOutcome::New);
        assert!(result.lost.is_empty());
        assert_eq!(tracker.committed(), 12);
    }

    #[test]
    fn commit_range_adjacent_to_committed_is_not_lost() {
        let mut tracker = WatermarkTracker::new(4);
        for seq in 0..5 {
            tracker.observe(seq);
        }
        // committed = 5，丢弃区间 [5, 9] 与之相邻 → 跳过，不判丢。
        let lost = tracker.commit_range(5, 9);
        assert!(lost.is_empty());
        assert_eq!(tracker.committed(), 10);
        assert_eq!(tracker.pending_ranges(), 0);
    }

    #[test]
    fn commit_range_bridges_gap_without_loss_cascade() {
        let mut tracker = WatermarkTracker::new(2);
        tracker.observe(0);
        tracker.observe(1);
        // 丢弃 [2, 100]（高量级过滤），随后 101 到达：
        // 若未按区间跳过，会因窗口 lag=2 把 2..100 误判为丢失。
        let lost = tracker.commit_range(2, 100);
        assert!(lost.is_empty());
        assert_eq!(tracker.committed(), 101);

        let result = tracker.observe(101);
        assert_eq!(result.outcome, ObserveOutcome::New);
        assert!(result.lost.is_empty());
        assert_eq!(tracker.committed(), 102);
    }

    #[test]
    fn commit_range_already_settled_is_noop() {
        let mut tracker = WatermarkTracker::new(4);
        tracker.observe(0);
        tracker.observe(1);
        // 区间 [0, 1] 已结算，重复上报应无操作、无丢失。
        let lost = tracker.commit_range(0, 1);
        assert!(lost.is_empty());
        assert_eq!(tracker.committed(), 2);
    }

    #[test]
    fn overlapping_ranges_are_merged() {
        // 用足够大的窗口（lag=100），避免区间被立即结算，便于观察合并后的区间数。
        let mut tracker = WatermarkTracker::new(100);
        tracker.commit_range(5, 9);
        tracker.commit_range(10, 12); // 与 [5, 9] 相邻 → 合并为 [5, 12]
        assert_eq!(tracker.pending_ranges(), 1);

        // 再提交与 [5, 12] 重叠的 [8, 20] → 仍合并为一个区间。
        tracker.commit_range(8, 20);
        assert_eq!(tracker.pending_ranges(), 1);

        // 结算：先补齐前缀 0..5，使区间与 committed 相邻，验证整段被跳过、不判丢。
        let mut t = WatermarkTracker::new(2);
        t.observe(0);
        let lost = t.commit_range(1, 6);
        assert!(lost.is_empty());
        assert_eq!(t.committed(), 7);
        assert_eq!(t.pending_ranges(), 0);
    }

    #[test]
    fn observe_inside_dropped_range_is_duplicate() {
        let mut tracker = WatermarkTracker::new(4);
        tracker.commit_range(5, 9);
        // 已声明丢弃的 seq 再以普通记录出现 → 判重复，不重复计数。
        let result = tracker.observe(7);
        assert_eq!(result.outcome, ObserveOutcome::Duplicate);
        assert!(result.lost.is_empty());
    }
}
