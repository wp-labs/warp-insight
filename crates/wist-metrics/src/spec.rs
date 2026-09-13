//! 指标声明（spec）：把采集侧 runtime fact 规范化成 OTel 风格指标的声明式映射。
//!
//! 这是 W4 三层结构的「契约层」——声明「采什么、叫什么、什么单位/类型」。新增一个指标 =
//! 在 [`METRIC_SPECS`] 里加一行，不改采集（provider）与规范化逻辑。

/// 一个指标的声明：某个采集 kind 的某个 runtime fact 规范化成什么指标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricSpec {
    /// 所属采集 kind（与 `MetricProvider::collection_kind` 对应，即 provider 维度）。
    pub collection_kind: &'static str,
    /// 采集侧 runtime fact 键（如 `host.loadavg.1m`）。
    pub fact_key: &'static str,
    /// 规范化后的指标名（如 `system.load_average.1m`）。
    pub name: &'static str,
    /// 单位。
    pub unit: &'static str,
    /// 值类型（`gauge_i64` / `gauge_f64` / `gauge_string`）。
    pub value_type: &'static str,
}

/// Batch A 指标声明表（新增指标 = 在这里加一行）。
pub const METRIC_SPECS: &[MetricSpec] = &[
    MetricSpec {
        collection_kind: "host_metrics",
        fact_key: "host.target.count",
        name: "system.target.count",
        unit: "1",
        value_type: "gauge_i64",
    },
    MetricSpec {
        collection_kind: "host_metrics",
        fact_key: "host.loadavg.1m",
        name: "system.load_average.1m",
        unit: "1",
        value_type: "gauge_f64",
    },
    MetricSpec {
        collection_kind: "host_metrics",
        fact_key: "host.loadavg.5m",
        name: "system.load_average.5m",
        unit: "1",
        value_type: "gauge_f64",
    },
    MetricSpec {
        collection_kind: "host_metrics",
        fact_key: "host.loadavg.15m",
        name: "system.load_average.15m",
        unit: "1",
        value_type: "gauge_f64",
    },
    MetricSpec {
        collection_kind: "host_metrics",
        fact_key: "host.uptime.seconds",
        name: "system.uptime",
        unit: "s",
        value_type: "gauge_f64",
    },
    MetricSpec {
        collection_kind: "host_metrics",
        fact_key: "host.memory.total_kb",
        name: "system.memory.total",
        unit: "KiBy",
        value_type: "gauge_i64",
    },
    MetricSpec {
        collection_kind: "host_metrics",
        fact_key: "host.memory.available_kb",
        name: "system.memory.available",
        unit: "KiBy",
        value_type: "gauge_i64",
    },
    MetricSpec {
        collection_kind: "host_metrics",
        fact_key: "host.disk.usage_percent",
        name: "system.disk.usage",
        unit: "percent",
        value_type: "gauge_f64",
    },
    MetricSpec {
        collection_kind: "host_metrics",
        fact_key: "host.disk.total_kb",
        name: "system.disk.total",
        unit: "KiBy",
        value_type: "gauge_i64",
    },
    MetricSpec {
        collection_kind: "host_metrics",
        fact_key: "host.disk.available_kb",
        name: "system.disk.available",
        unit: "KiBy",
        value_type: "gauge_i64",
    },
    MetricSpec {
        collection_kind: "process_metrics",
        fact_key: "process.memory.rss_pages",
        name: "process.memory.rss",
        unit: "pages",
        value_type: "gauge_i64",
    },
    MetricSpec {
        collection_kind: "process_metrics",
        fact_key: "process.memory.rss_kb",
        name: "process.memory.rss",
        unit: "KiBy",
        value_type: "gauge_i64",
    },
    MetricSpec {
        collection_kind: "process_metrics",
        fact_key: "process.state",
        name: "process.state",
        unit: "state",
        value_type: "gauge_string",
    },
    MetricSpec {
        collection_kind: "container_metrics",
        fact_key: "process.memory.rss_pages",
        name: "container.memory.rss",
        unit: "pages",
        value_type: "gauge_i64",
    },
    MetricSpec {
        collection_kind: "container_metrics",
        fact_key: "process.memory.rss_kb",
        name: "container.memory.rss",
        unit: "KiBy",
        value_type: "gauge_i64",
    },
    MetricSpec {
        collection_kind: "container_metrics",
        fact_key: "process.state",
        name: "container.state",
        unit: "state",
        value_type: "gauge_string",
    },
    MetricSpec {
        collection_kind: "container_metrics",
        fact_key: "container.pid",
        name: "container.pid",
        unit: "1",
        value_type: "gauge_i64",
    },
];

/// 按 `(collection_kind, fact_key)` 查找指标声明。
pub fn find_metric_spec(collection_kind: &str, fact_key: &str) -> Option<&'static MetricSpec> {
    METRIC_SPECS
        .iter()
        .find(|spec| spec.collection_kind == collection_kind && spec.fact_key == fact_key)
}

#[cfg(test)]
mod tests {
    use super::{find_metric_spec, METRIC_SPECS};

    #[test]
    fn finds_spec_by_kind_and_fact_key() {
        let spec = find_metric_spec("host_metrics", "host.loadavg.1m").expect("spec");
        assert_eq!(spec.name, "system.load_average.1m");
        assert_eq!(spec.unit, "1");
        assert_eq!(spec.value_type, "gauge_f64");
    }

    #[test]
    fn unknown_fact_key_is_none() {
        assert!(find_metric_spec("host_metrics", "no.such.fact").is_none());
    }

    #[test]
    fn specs_are_unique_per_kind_and_fact_key() {
        // 声明表不能有重复的 (collection_kind, fact_key)，否则规范化结果不确定。
        let mut seen = std::collections::HashSet::new();
        for spec in METRIC_SPECS {
            let key = (spec.collection_kind, spec.fact_key);
            assert!(seen.insert(key), "duplicate spec key: {key:?}");
        }
    }
}
