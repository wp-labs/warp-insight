//! Minimal metrics sample view built from runtime snapshot outcomes.
//!
//! Step 3: grouped by collection_kind + target, with plain value format and status.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::runtime::{MetricsCollectionOutcome, MetricsRuntimeSnapshot};
use super::spec::find_metric_spec;

static METRICS_BATCH_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[serde(deny_unknown_fields)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct MetricsSamplesSnapshot {
    pub batch_seq: u64,
    pub collected_at: String,
    #[serde(default)]
    pub groups: Vec<MetricsSampleGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[serde(deny_unknown_fields)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct MetricsSampleGroup {
    pub kind: String,
    pub target_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_ref: Option<String>,
    #[serde(default)]
    pub samples: Vec<MetricsSampleRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[serde(deny_unknown_fields)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct MetricsSampleRecord {
    pub name: String,
    pub value: Value,
    #[serde(rename = "type")]
    pub value_type: String,
    pub unit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// VM（VictoriaMetrics）`/api/v1/import` 的 JSON line 导入格式的单条指标。
///
/// 结构：`{"metric":{"__name__":"<name>","<label>":"<v>",...},"values":[<number>],"timestamps":[<ms>]}`。
/// 一个 sample 拍平成一行；`metric` 里的 `__name__` 是指标名，其余是标签。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VmMetricLine {
    pub metric: VmMetricLabels,
    pub values: Vec<f64>,
    pub timestamps: Vec<i64>,
}

/// VM JSON line 的 `metric` 对象：`__name__` 为指标名，其余字段为标签。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VmMetricLabels {
    #[serde(rename = "__name__")]
    pub name: String,
    pub agent: String,
    pub kind: String,
    pub target_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_ref: Option<String>,
    pub unit: String,
}

/// 把样本快照拍平成 VM JSON lines（一 sample 一行）。
///
/// 只保留数值型样本（i64/f64）；`gauge_string` 等字符串样本不是数值指标，不进 VM 通道
/// （center 侧结构化通道另行处理）。`timestamp_ms` 为采集时间戳（毫秒）。
pub fn build_vm_metric_lines(
    snapshot: &MetricsSamplesSnapshot,
    agent_id: &str,
    timestamp_ms: i64,
) -> Vec<VmMetricLine> {
    let mut lines = Vec::new();
    for group in &snapshot.groups {
        for sample in &group.samples {
            if !is_numeric_sample(&sample.value_type) {
                continue;
            }
            // VM 只收数值指标，统一归一成 f64（i64→f64 无损）；非数值（解析失败）直接跳过。
            let Some(value) = sample.value.as_f64() else {
                continue;
            };
            lines.push(VmMetricLine {
                metric: VmMetricLabels {
                    name: sample.name.clone(),
                    agent: agent_id.to_string(),
                    kind: group.kind.clone(),
                    target_ref: group.target_ref.clone(),
                    resource_ref: group.resource_ref.clone(),
                    unit: sample.unit.clone(),
                },
                values: vec![value],
                timestamps: vec![timestamp_ms],
            });
        }
    }
    lines
}

fn is_numeric_sample(value_type: &str) -> bool {
    matches!(
        value_type,
        "gauge_i64" | "counter_i64" | "gauge_f64" | "counter_f64"
    )
}

pub fn build_samples_snapshot(runtime: &MetricsRuntimeSnapshot) -> MetricsSamplesSnapshot {
    let seq = METRICS_BATCH_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut groups = Vec::new();

    for outcome in &runtime.outcomes {
        build_outcome_groups(outcome, &mut groups);
    }

    MetricsSamplesSnapshot {
        batch_seq: seq,
        collected_at: runtime.generated_at.clone(),
        groups,
    }
}

fn build_outcome_groups(outcome: &MetricsCollectionOutcome, groups: &mut Vec<MetricsSampleGroup>) {
    for target in &outcome.sample_targets {
        let mut samples = Vec::new();

        for fact in &target.runtime_facts {
            let Some(spec) = find_metric_spec(&outcome.collection_kind, fact.key.as_str()) else {
                continue;
            };
            samples.push(MetricsSampleRecord {
                name: spec.name.to_string(),
                value: sample_value(spec.value_type, &fact.value),
                value_type: spec.value_type.to_string(),
                unit: spec.unit.to_string(),
                status: if target.status == "succeeded" {
                    None
                } else {
                    Some(target.status.clone())
                },
            });
        }

        if !samples.is_empty() {
            groups.push(MetricsSampleGroup {
                kind: outcome.collection_kind.clone(),
                target_ref: target.target_ref.clone(),
                resource_ref: Some(target.resource_ref.clone()),
                samples,
            });
        }
    }
}

fn sample_value(value_type: &str, raw: &str) -> Value {
    match value_type {
        "gauge_i64" | "counter_i64" => raw
            .parse::<i64>()
            .map(|v| Value::Number(v.into()))
            .unwrap_or_else(|_| Value::String(raw.to_string())),
        "gauge_f64" | "counter_f64" => raw
            .parse::<f64>()
            .ok()
            .and_then(|v| serde_json::Number::from_f64(v).map(Value::Number))
            .unwrap_or_else(|| Value::String(raw.to_string())),
        _ => Value::String(raw.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use wist_contracts::discovery::StringKeyValue;

    use super::{
        MetricsSampleGroup, MetricsSampleRecord, MetricsSamplesSnapshot, build_samples_snapshot,
        build_vm_metric_lines,
    };
    use crate::telemetry::metrics::runtime::{
        MetricsCollectionOutcome, MetricsCollectionTargetSample, MetricsRuntimeSnapshot,
    };

    #[test]
    fn build_samples_snapshot_groups_by_collection_kind_and_target() {
        let runtime = MetricsRuntimeSnapshot {
            generated_at: "2026-04-19T00:00:00Z".to_string(),
            total_targets: 1,
            host_targets: 1,
            process_targets: 0,
            container_targets: 0,
            outcomes: vec![MetricsCollectionOutcome {
                collection_kind: "host_metrics".to_string(),
                status: "succeeded".to_string(),
                attempted_targets: 1,
                succeeded_targets: 1,
                failed_targets: 0,
                last_error: None,
                runtime_facts: vec![StringKeyValue::new("host.loadavg.1m", "0.25")],
                sample_targets: vec![MetricsCollectionTargetSample {
                    candidate_id: "host-1".to_string(),
                    target_ref: "host-1:host".to_string(),
                    status: "succeeded".to_string(),
                    last_error: None,
                    resource_ref: "host-1".to_string(),
                    execution_hints: vec![StringKeyValue::new("host.name", "local-host")],
                    runtime_facts: vec![
                        StringKeyValue::new("host.loadavg.1m", "0.25"),
                        StringKeyValue::new("host.uptime.seconds", "3600"),
                    ],
                }],
            }],
        };

        let snapshot = build_samples_snapshot(&runtime);

        assert_eq!(snapshot.groups.len(), 1);
        let group = &snapshot.groups[0];
        assert_eq!(group.kind, "host_metrics");
        assert_eq!(group.target_ref, "host-1:host");
        assert_eq!(group.resource_ref, Some("host-1".to_string()));
        assert_eq!(group.samples.len(), 2);

        let load_sample = group
            .samples
            .iter()
            .find(|s| s.name == "system.load_average.1m")
            .expect("load sample");
        assert_eq!(load_sample.value, serde_json::json!(0.25));
        assert_eq!(load_sample.value_type, "gauge_f64");
        assert!(load_sample.status.is_none());

        let uptime_sample = group
            .samples
            .iter()
            .find(|s| s.name == "system.uptime")
            .expect("uptime sample");
        assert_eq!(uptime_sample.value, serde_json::json!(3600.0));
        assert_eq!(uptime_sample.unit, "s");
    }

    #[test]
    fn value_format_uses_plain_number_instead_of_tagged_enum() {
        let runtime = MetricsRuntimeSnapshot {
            generated_at: "2026-04-19T00:00:00Z".to_string(),
            total_targets: 1,
            host_targets: 1,
            process_targets: 0,
            container_targets: 0,
            outcomes: vec![MetricsCollectionOutcome {
                collection_kind: "host_metrics".to_string(),
                status: "succeeded".to_string(),
                attempted_targets: 1,
                succeeded_targets: 1,
                failed_targets: 0,
                last_error: None,
                runtime_facts: vec![],
                sample_targets: vec![MetricsCollectionTargetSample {
                    candidate_id: "host-1".to_string(),
                    target_ref: "host-1:host".to_string(),
                    status: "succeeded".to_string(),
                    last_error: None,
                    resource_ref: "host-1".to_string(),
                    execution_hints: vec![],
                    runtime_facts: vec![
                        StringKeyValue::new("host.loadavg.1m", "0.25"),
                        StringKeyValue::new("host.memory.total_kb", "8388608"),
                        StringKeyValue::new("host.memory.available_kb", "4194304"),
                    ],
                }],
            }],
        };

        let snapshot = build_samples_snapshot(&runtime);
        let group = &snapshot.groups[0];

        let f64_sample = group
            .samples
            .iter()
            .find(|s| s.name == "system.load_average.1m")
            .expect("f64 sample");
        assert!(f64_sample.value.is_f64());

        let i64_sample = group
            .samples
            .iter()
            .find(|s| s.name == "system.memory.total")
            .expect("i64 sample");
        assert!(i64_sample.value.is_number());

        let avail = group
            .samples
            .iter()
            .find(|s| s.name == "system.memory.available")
            .expect("i64 sample");
        assert!(avail.value.is_number());
    }

    #[test]
    fn build_vm_metric_lines_flattens_numeric_samples_and_skips_strings() {
        let snapshot = MetricsSamplesSnapshot {
            batch_seq: 3,
            collected_at: "2026-04-19T00:00:00Z".to_string(),
            groups: vec![MetricsSampleGroup {
                kind: "host_metrics".to_string(),
                target_ref: "host-1:host".to_string(),
                resource_ref: Some("host-1".to_string()),
                samples: vec![
                    MetricsSampleRecord {
                        name: "system.load_average.1m".to_string(),
                        value: serde_json::json!(0.25),
                        value_type: "gauge_f64".to_string(),
                        unit: "1".to_string(),
                        status: None,
                    },
                    MetricsSampleRecord {
                        name: "process.state".to_string(),
                        value: serde_json::json!("running"),
                        value_type: "gauge_string".to_string(),
                        unit: "state".to_string(),
                        status: None,
                    },
                ],
            }],
        };

        let lines = build_vm_metric_lines(&snapshot, "agent-001", 1_234_567_890_000);

        assert_eq!(lines.len(), 1, "string sample should be skipped");
        let line = &lines[0];
        assert_eq!(line.metric.name, "system.load_average.1m");
        assert_eq!(line.metric.agent, "agent-001");
        assert_eq!(line.metric.kind, "host_metrics");
        assert_eq!(line.metric.target_ref, "host-1:host");
        assert_eq!(line.metric.resource_ref, Some("host-1".to_string()));
        assert_eq!(line.metric.unit, "1");
        assert_eq!(line.values, vec![0.25]);
        assert_eq!(line.timestamps, vec![1_234_567_890_000]);
    }
}
