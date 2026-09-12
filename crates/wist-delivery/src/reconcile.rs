//! 多跳差值定位与端到端交付账本。
//!
//! 利用端到端不变的 `seq`：gateway 观测第一段缺口、center 观测累计缺口，
//! 二者做差即可定位丢失发生在哪一跳，无需每跳重复全量检测。

/// 多跳丢失的差值定位。
///
/// - `first_segment`：gateway 判定的 `agentd → gateway` 段丢失数（`G`）。
/// - `cumulative`：center 数据平台判定的端到端累计丢失数（`C`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HopLosses {
    pub first_segment: u64,
    pub cumulative: u64,
}

impl HopLosses {
    /// 第二段（`gateway → center`）丢失 = 累计 − 第一段。
    pub fn second_segment(&self) -> u64 {
        self.cumulative.saturating_sub(self.first_segment)
    }

    /// 端到端总丢失 = 累计。
    pub fn total(&self) -> u64 {
        self.cumulative
    }
}

/// 端到端交付账本，恒等式：
/// `produced == stored + filtered + source_dropped + silent_loss`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeliveryLedger {
    /// 生产总数（`seq` 分配总数）。
    pub produced: u64,
    /// 成功落库数。
    pub stored: u64,
    /// 主动过滤（带外丢弃区间）数。
    pub filtered: u64,
    /// 源头显式丢弃数（截断/背压/读失败）。
    pub source_dropped: u64,
    /// 静默丢失数（watermark 判定的累计丢失）。
    pub silent_loss: u64,
}

impl DeliveryLedger {
    /// 账本是否平衡（忽略溢出，用饱和加法）。
    pub fn balanced(&self) -> bool {
        self.produced
            == self
                .stored
                .saturating_add(self.filtered)
                .saturating_add(self.source_dropped)
                .saturating_add(self.silent_loss)
    }
}

#[cfg(test)]
mod tests {
    use super::{DeliveryLedger, HopLosses};

    #[test]
    fn second_segment_is_cumulative_minus_first() {
        let losses = HopLosses {
            first_segment: 2,
            cumulative: 5,
        };
        assert_eq!(losses.second_segment(), 3);
        assert_eq!(losses.total(), 5);
    }

    #[test]
    fn second_segment_saturates_when_cumulative_smaller() {
        // 第一段缺口可能暂时大于累计（乱序/统计时序差异），做差应饱和到 0。
        let losses = HopLosses {
            first_segment: 5,
            cumulative: 3,
        };
        assert_eq!(losses.second_segment(), 0);
    }

    #[test]
    fn balanced_ledger_holds_identity() {
        let ledger = DeliveryLedger {
            produced: 100,
            stored: 90,
            filtered: 5,
            source_dropped: 3,
            silent_loss: 2,
        };
        assert!(ledger.balanced());
    }

    #[test]
    fn unbalanced_ledger_is_detected() {
        let ledger = DeliveryLedger {
            produced: 100,
            stored: 90,
            filtered: 5,
            source_dropped: 3,
            silent_loss: 1,
        };
        assert!(!ledger.balanced());
    }
}
