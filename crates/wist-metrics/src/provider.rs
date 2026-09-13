//! 采集层抽象：`MetricProvider` trait 与一次采集的结果类型。
//!
//! provider 只产出样本（runtime fact 键值），不做上报决策；规范化/上报在 `warp-agentd`
//! 的 samples 层完成。`MetricsCollectionOutcome` / `MetricsCollectionTargetSample` 是
//! collect → samples 之间的中转结构，这里与 `warp-agentd` 共享。

use serde::{Deserialize, Serialize};
use wist_contracts::discovery::StringKeyValue;

use crate::target::MetricsTargetViewEntry;

/// 一类指标采集的抽象（编译期注册 provider）。
pub trait MetricProvider {
    /// 采集 kind（与 `MetricsTargetViewEntry.collection_kind` 对应）。
    fn collection_kind(&self) -> &'static str;

    /// 对一组目标采集，产出该 kind 的采集结果。
    fn collect(&self, targets: Vec<&MetricsTargetViewEntry>) -> MetricsCollectionOutcome;
}

/// 一类指标的一次采集结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[serde(deny_unknown_fields)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct MetricsCollectionOutcome {
    pub collection_kind: String,
    pub status: String,
    pub attempted_targets: usize,
    pub succeeded_targets: usize,
    pub failed_targets: usize,
    pub last_error: Option<String>,
    #[serde(default)]
    pub runtime_facts: Vec<StringKeyValue>,
    #[serde(default)]
    pub sample_targets: Vec<MetricsCollectionTargetSample>,
}

/// 单个采集目标的采集结果（含该目标产出的 runtime fact）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[serde(deny_unknown_fields)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct MetricsCollectionTargetSample {
    pub candidate_id: String,
    pub target_ref: String,
    pub status: String,
    pub last_error: Option<String>,
    pub resource_ref: String,
    #[serde(default)]
    pub execution_hints: Vec<StringKeyValue>,
    #[serde(default)]
    pub runtime_facts: Vec<StringKeyValue>,
}
