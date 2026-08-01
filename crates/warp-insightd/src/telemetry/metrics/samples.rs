//! Minimal metrics sample view built from runtime snapshot outcomes.
//!
//! Step 3: grouped by collection_kind + target, with plain value format and status.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::runtime::{MetricsCollectionOutcome, MetricsRuntimeSnapshot};

static METRICS_BATCH_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ::moju_derive::MoJu)]
#[serde(deny_unknown_fields)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct MetricsSamplesSnapshot {
    pub batch_seq: u64,
    pub collected_at: String,
    #[serde(default)]
    pub groups: Vec<MetricsSampleGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ::moju_derive::MoJu)]
#[serde(deny_unknown_fields)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct MetricsSampleGroup {
    pub kind: String,
    pub target_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_ref: Option<String>,
    #[serde(default)]
    pub samples: Vec<MetricsSampleRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ::moju_derive::MoJu)]
#[serde(deny_unknown_fields)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct MetricsSampleRecord {
    pub name: String,
    pub value: Value,
    #[serde(rename = "type")]
    pub value_type: String,
    pub unit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
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
            let Some((metric_name, unit, value_type)) =
                map_runtime_fact_to_metric(&outcome.collection_kind, fact.key.as_str())
            else {
                continue;
            };
            samples.push(MetricsSampleRecord {
                name: metric_name.to_string(),
                value: sample_value(value_type, &fact.value),
                value_type: value_type.to_string(),
                unit: unit.to_string(),
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

fn map_runtime_fact_to_metric(
    collection_kind: &str,
    key: &str,
) -> Option<(&'static str, &'static str, &'static str)> {
    match (collection_kind, key) {
        ("host_metrics", "host.target.count") => Some(("system.target.count", "1", "gauge_i64")),
        ("host_metrics", "host.loadavg.1m") => Some(("system.load_average.1m", "1", "gauge_f64")),
        ("host_metrics", "host.loadavg.5m") => Some(("system.load_average.5m", "1", "gauge_f64")),
        ("host_metrics", "host.loadavg.15m") => Some(("system.load_average.15m", "1", "gauge_f64")),
        ("host_metrics", "host.uptime.seconds") => Some(("system.uptime", "s", "gauge_f64")),
        ("host_metrics", "host.memory.total_kb") => {
            Some(("system.memory.total", "KiBy", "gauge_i64"))
        }
        ("host_metrics", "host.memory.available_kb") => {
            Some(("system.memory.available", "KiBy", "gauge_i64"))
        }
        ("process_metrics", "process.cpu.user_ticks") => {
            Some(("process.cpu.time.user", "ticks", "gauge_i64"))
        }
        ("process_metrics", "process.cpu.system_ticks") => {
            Some(("process.cpu.time.system", "ticks", "gauge_i64"))
        }
        ("process_metrics", "process.memory.rss_pages") => {
            Some(("process.memory.rss", "pages", "gauge_i64"))
        }
        ("process_metrics", "process.memory.rss_kb") => {
            Some(("process.memory.rss", "KiBy", "gauge_i64"))
        }
        ("process_metrics", "process.state") => Some(("process.state", "state", "gauge_string")),
        ("container_metrics", "process.cpu.user_ticks") => {
            Some(("container.cpu.time.user", "ticks", "gauge_i64"))
        }
        ("container_metrics", "process.cpu.system_ticks") => {
            Some(("container.cpu.time.system", "ticks", "gauge_i64"))
        }
        ("container_metrics", "process.memory.rss_pages") => {
            Some(("container.memory.rss", "pages", "gauge_i64"))
        }
        ("container_metrics", "process.memory.rss_kb") => {
            Some(("container.memory.rss", "KiBy", "gauge_i64"))
        }
        ("container_metrics", "process.state") => {
            Some(("container.state", "state", "gauge_string"))
        }
        ("container_metrics", "container.pid") => Some(("container.pid", "1", "gauge_i64")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use warp_insight_contracts::discovery::StringKeyValue;

    use super::build_samples_snapshot;
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
}
