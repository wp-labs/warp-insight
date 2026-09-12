//! 丢失/过滤/接收计数的可序列化报告类型。
//!
//! 接收端把本结构序列化后随状态/控制面通道上报，供 [`crate::reconcile`] 对账。

use serde::{Deserialize, Serialize};

/// 单个 `agent_id` 流的交付计数快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamLossReport {
    /// 生产者标识（全局唯一）。
    #[serde(rename = "agent")]
    pub agent_id: String,
    /// watermark 判定丢失的条数（累计）。
    pub lost_count: u64,
    /// 主动过滤条数（带外丢弃区间，累计）。
    pub filtered_count: u64,
    /// 接受（非重复、非丢失、非过滤）的条数。
    pub accepted_count: u64,
    /// 带外丢弃区间明细（主动过滤，如 debug），供 center 对账按 reason 拆分。
    #[serde(default)]
    pub dropped_ranges: Vec<DroppedRange>,
}

impl StreamLossReport {
    /// 新建一份全零的流报告。
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            lost_count: 0,
            filtered_count: 0,
            accepted_count: 0,
            dropped_ranges: Vec::new(),
        }
    }
}

/// 一段被主动过滤（内容级，如 debug）丢弃的 `seq` 区间。
///
/// 由 ETL 层（warp-parse 解析出 level 后过滤）产出，随控制面通道上报，供 center 对账时
/// 从数据流缺口中扣除（区分「过滤」与「丢失」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroppedRange {
    /// 区间起始 `seq`（含）。
    #[serde(rename = "start")]
    pub range_start: u64,
    /// 区间结束 `seq`（含）。
    #[serde(rename = "end")]
    pub range_end: u64,
    /// 丢弃原因（如 `debug`、`sample`、`filter:x`）。
    #[serde(rename = "reason")]
    pub drop_reason: String,
}

impl DroppedRange {
    /// 新建一段丢弃区间。
    pub fn new(range_start: u64, range_end: u64, drop_reason: impl Into<String>) -> Self {
        Self {
            range_start,
            range_end,
            drop_reason: drop_reason.into(),
        }
    }

    /// 区间内的丢弃条数（含两端）。
    pub fn count(&self) -> u64 {
        self.range_end
            .saturating_sub(self.range_start)
            .saturating_add(1)
    }
}

#[cfg(test)]
mod tests {
    use super::{DroppedRange, StreamLossReport};

    #[test]
    fn report_round_trips_json() {
        let report = StreamLossReport {
            agent_id: "agent-001".to_string(),
            lost_count: 3,
            filtered_count: 1,
            accepted_count: 100,
            dropped_ranges: vec![DroppedRange::new(5, 9, "debug")],
        };
        let encoded = serde_json::to_string(&report).expect("serialize");
        let decoded: StreamLossReport = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, report);
    }

    #[test]
    fn report_serializes_agent_short_name_without_input() {
        let report = StreamLossReport::new("agent-001");
        let json = serde_json::to_string(&report).expect("serialize");
        // 帧短名 `agent`；已去掉 `input_id`。
        assert!(json.contains(r#""agent":"agent-001""#), "json = {json}");
        assert!(!json.contains("input_id"), "json = {json}");
    }

    #[test]
    fn dropped_range_serializes_short_names() {
        let range = DroppedRange::new(2, 5, "debug");
        let json = serde_json::to_string(&range).expect("serialize");
        assert_eq!(json, r#"{"start":2,"end":5,"reason":"debug"}"#);
    }

    #[test]
    fn dropped_range_count_is_inclusive() {
        assert_eq!(DroppedRange::new(5, 9, "debug").count(), 5);
        assert_eq!(DroppedRange::new(5, 5, "debug").count(), 1);
    }

    #[test]
    fn report_without_dropped_ranges_deserializes() {
        // 旧版报告（无 dropped_ranges 字段）仍可反序列化（#[serde(default)]）。
        let json = r#"{"agent":"a","lost_count":0,"filtered_count":0,"accepted_count":1}"#;
        let decoded: StreamLossReport = serde_json::from_str(json).expect("deserialize");
        assert!(decoded.dropped_ranges.is_empty());
        assert_eq!(decoded.agent_id, "a");
        assert_eq!(decoded.accepted_count, 1);
    }
}
