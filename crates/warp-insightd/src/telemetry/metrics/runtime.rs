//! Minimal metrics runtime tick built from target view.

use std::fs;
use std::io;
use std::path::Path;
#[cfg(unix)]
use std::process::Command;

use serde::{Deserialize, Serialize};
use warp_insight_contracts::discovery::StringKeyValue;
use warp_insight_shared::fs::write_json_atomic;

#[cfg(test)]
use super::target_view::path_for as target_view_path_for;
use super::target_view::{MetricsTargetView, MetricsTargetViewEntry};
#[cfg(test)]
use warp_insight_shared::fs::read_json;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::moju_derive::MoJu)]
#[serde(deny_unknown_fields)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct MetricsRuntimeSnapshot {
    pub generated_at: String,
    pub total_targets: usize,
    pub host_targets: usize,
    pub process_targets: usize,
    pub container_targets: usize,
    #[serde(default)]
    pub outcomes: Vec<MetricsCollectionOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::moju_derive::MoJu)]
#[serde(deny_unknown_fields)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::moju_derive::MoJu)]
#[serde(deny_unknown_fields)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
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

#[cfg(test)]
pub fn build_runtime_snapshot(state_dir: &Path) -> io::Result<MetricsRuntimeSnapshot> {
    let view: MetricsTargetView = read_json(&target_view_path_for(state_dir))?;
    Ok(build_runtime_snapshot_from_view(&view))
}

pub fn build_runtime_snapshot_from_view(view: &MetricsTargetView) -> MetricsRuntimeSnapshot {
    let mut host_targets = 0;
    let mut process_targets = 0;
    let mut container_targets = 0;
    let mut host_samples = Vec::new();
    let mut process_samples = Vec::new();
    let mut container_samples = Vec::new();

    for target in &view.targets {
        match target.collection_kind.as_str() {
            "host_metrics" => {
                host_targets += 1;
                maybe_push_sample(&mut host_samples, target);
            }
            "process_metrics" => {
                process_targets += 1;
                maybe_push_sample(&mut process_samples, target);
            }
            "container_metrics" => {
                container_targets += 1;
                maybe_push_sample(&mut container_samples, target);
            }
            _ => {}
        }
    }

    let host_outcome = build_host_outcome(host_targets, host_samples);
    let process_outcome = build_process_outcome(process_entries_from_view(view));
    let container_outcome = build_container_outcome(container_entries_from_view(view));

    MetricsRuntimeSnapshot {
        generated_at: view.generated_at.clone(),
        total_targets: host_targets + process_targets + container_targets,
        host_targets,
        process_targets,
        container_targets,
        outcomes: vec![host_outcome, process_outcome, container_outcome],
    }
}

pub fn path_for(state_dir: &Path) -> std::path::PathBuf {
    state_dir
        .join("telemetry")
        .join("metrics_runtime_snapshot.json")
}

pub fn store(path: &Path, snapshot: &MetricsRuntimeSnapshot) -> io::Result<()> {
    write_json_atomic(path, snapshot)
}

fn build_host_outcome(
    attempted_targets: usize,
    mut sample_targets: Vec<MetricsCollectionTargetSample>,
) -> MetricsCollectionOutcome {
    if attempted_targets == 0 {
        return MetricsCollectionOutcome {
            collection_kind: "host_metrics".to_string(),
            status: "idle".to_string(),
            attempted_targets: 0,
            succeeded_targets: 0,
            failed_targets: 0,
            last_error: None,
            runtime_facts: vec![StringKeyValue::new("host.target.count", "0")],
            sample_targets,
        };
    }

    let runtime_facts = collect_host_runtime_facts(&sample_targets);
    if runtime_facts.is_empty() {
        for sample in &mut sample_targets {
            sample.status = "failed".to_string();
            sample.last_error = Some("host metrics probe produced no runtime facts".to_string());
        }
        MetricsCollectionOutcome {
            collection_kind: "host_metrics".to_string(),
            status: "failed".to_string(),
            attempted_targets,
            succeeded_targets: 0,
            failed_targets: attempted_targets,
            last_error: Some("host metrics probe produced no runtime facts".to_string()),
            runtime_facts: vec![StringKeyValue::new(
                "host.target.count",
                attempted_targets.to_string(),
            )],
            sample_targets,
        }
    } else {
        let mut runtime_facts = runtime_facts;
        push_fact_if_absent(
            &mut runtime_facts,
            StringKeyValue::new("host.target.count", attempted_targets.to_string()),
        );
        for sample in &mut sample_targets {
            sample.status = "succeeded".to_string();
            sample.runtime_facts = runtime_facts.clone();
        }
        MetricsCollectionOutcome {
            collection_kind: "host_metrics".to_string(),
            status: "succeeded".to_string(),
            attempted_targets,
            succeeded_targets: attempted_targets,
            failed_targets: 0,
            last_error: None,
            runtime_facts,
            sample_targets,
        }
    }
}

fn build_process_outcome(targets: Vec<&MetricsTargetViewEntry>) -> MetricsCollectionOutcome {
    if targets.is_empty() {
        return MetricsCollectionOutcome {
            collection_kind: "process_metrics".to_string(),
            status: "idle".to_string(),
            attempted_targets: 0,
            succeeded_targets: 0,
            failed_targets: 0,
            last_error: None,
            runtime_facts: Vec::new(),
            sample_targets: Vec::new(),
        };
    }

    let mut succeeded_targets = 0usize;
    let mut failed_targets = 0usize;
    let mut sample_targets = Vec::new();
    let mut last_error = None;

    for target in &targets {
        let probed = probe_process_target(target);
        if probed.status == "succeeded" {
            succeeded_targets += 1;
        } else {
            failed_targets += 1;
            if last_error.is_none() {
                last_error = probed.last_error.clone();
            }
        }
        if sample_targets.len() < 3 {
            sample_targets.push(probed);
        }
    }

    let status = if failed_targets == 0 {
        "succeeded"
    } else if succeeded_targets == 0 {
        "failed"
    } else {
        "partial"
    };

    MetricsCollectionOutcome {
        collection_kind: "process_metrics".to_string(),
        status: status.to_string(),
        attempted_targets: targets.len(),
        succeeded_targets,
        failed_targets,
        last_error,
        runtime_facts: vec![
            StringKeyValue::new("process.probe.mode", process_probe_mode()),
            StringKeyValue::new("process.probe.success_count", succeeded_targets.to_string()),
            StringKeyValue::new("process.probe.failed_count", failed_targets.to_string()),
        ],
        sample_targets,
    }
}

fn build_container_outcome(targets: Vec<&MetricsTargetViewEntry>) -> MetricsCollectionOutcome {
    if targets.is_empty() {
        return MetricsCollectionOutcome {
            collection_kind: "container_metrics".to_string(),
            status: "idle".to_string(),
            attempted_targets: 0,
            succeeded_targets: 0,
            failed_targets: 0,
            last_error: None,
            runtime_facts: Vec::new(),
            sample_targets: Vec::new(),
        };
    }

    let mut succeeded_targets = 0usize;
    let mut failed_targets = 0usize;
    let mut sample_targets = Vec::new();
    let mut last_error = None;

    for target in &targets {
        let probed = probe_container_target(target);
        if probed.status == "failed" {
            failed_targets += 1;
            if last_error.is_none() {
                last_error = probed.last_error.clone();
            }
        } else {
            succeeded_targets += 1;
        }
        if sample_targets.len() < 3 {
            sample_targets.push(probed);
        }
    }

    let status = if failed_targets == 0 {
        "succeeded"
    } else if succeeded_targets == 0 {
        "failed"
    } else {
        "partial"
    };

    MetricsCollectionOutcome {
        collection_kind: "container_metrics".to_string(),
        status: status.to_string(),
        attempted_targets: targets.len(),
        succeeded_targets,
        failed_targets,
        last_error,
        runtime_facts: vec![
            StringKeyValue::new("container.probe.mode", container_probe_mode()),
            StringKeyValue::new(
                "container.probe.success_count",
                succeeded_targets.to_string(),
            ),
            StringKeyValue::new("container.probe.failed_count", failed_targets.to_string()),
        ],
        sample_targets,
    }
}

fn collect_host_runtime_facts(
    sample_targets: &[MetricsCollectionTargetSample],
) -> Vec<StringKeyValue> {
    let mut facts = Vec::new();

    for sample in sample_targets {
        for hint in &sample.execution_hints {
            if hint.key == "host.name" {
                push_fact_if_absent(&mut facts, hint.clone());
            }
        }
    }

    if let Ok(loadavg) = fs::read_to_string("/proc/loadavg") {
        let mut parts = loadavg.split_whitespace();
        if let Some(value) = parts.next() {
            push_fact_if_absent(&mut facts, StringKeyValue::new("host.loadavg.1m", value));
        }
        if let Some(value) = parts.next() {
            push_fact_if_absent(&mut facts, StringKeyValue::new("host.loadavg.5m", value));
        }
        if let Some(value) = parts.next() {
            push_fact_if_absent(&mut facts, StringKeyValue::new("host.loadavg.15m", value));
        }
    }

    if let Ok(uptime) = fs::read_to_string("/proc/uptime") {
        if let Some(value) = uptime.split_whitespace().next() {
            push_fact_if_absent(
                &mut facts,
                StringKeyValue::new("host.uptime.seconds", value),
            );
        }
    }

    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if let Some(value) = parse_meminfo_kb(line, "MemTotal:") {
                push_fact_if_absent(
                    &mut facts,
                    StringKeyValue::new("host.memory.total_kb", value),
                );
            } else if let Some(value) = parse_meminfo_kb(line, "MemAvailable:") {
                push_fact_if_absent(
                    &mut facts,
                    StringKeyValue::new("host.memory.available_kb", value),
                );
            }
        }
    }

    facts
}

fn parse_meminfo_kb<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix(key)
        .map(str::trim)
        .and_then(|value| value.strip_suffix(" kB").or(Some(value)))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn push_fact_if_absent(facts: &mut Vec<StringKeyValue>, candidate: StringKeyValue) {
    if facts.iter().any(|fact| fact.key == candidate.key) {
        return;
    }
    facts.push(candidate);
}

fn maybe_push_sample(
    samples: &mut Vec<MetricsCollectionTargetSample>,
    target: &MetricsTargetViewEntry,
) {
    const SAMPLE_LIMIT: usize = 3;

    if samples.len() >= SAMPLE_LIMIT {
        return;
    }

    samples.push(MetricsCollectionTargetSample {
        candidate_id: target.candidate_id.clone(),
        target_ref: target.target_ref.clone(),
        status: "pending".to_string(),
        last_error: None,
        resource_ref: target.resource_ref.clone(),
        execution_hints: target.execution_hints.clone(),
        runtime_facts: Vec::new(),
    });
}

fn process_entries_from_view(view: &MetricsTargetView) -> Vec<&MetricsTargetViewEntry> {
    view.targets
        .iter()
        .filter(|target| target.collection_kind == "process_metrics")
        .collect()
}

fn container_entries_from_view(view: &MetricsTargetView) -> Vec<&MetricsTargetViewEntry> {
    view.targets
        .iter()
        .filter(|target| target.collection_kind == "container_metrics")
        .collect()
}

fn probe_process_target(target: &MetricsTargetViewEntry) -> MetricsCollectionTargetSample {
    let mut sample = MetricsCollectionTargetSample {
        candidate_id: target.candidate_id.clone(),
        target_ref: target.target_ref.clone(),
        status: "failed".to_string(),
        last_error: None,
        resource_ref: target.resource_ref.clone(),
        execution_hints: target.execution_hints.clone(),
        runtime_facts: Vec::new(),
    };

    let Some(pid) = target
        .execution_hints
        .iter()
        .find(|hint| hint.key == "process.pid")
        .and_then(|hint| hint.value.parse::<u32>().ok())
    else {
        sample.last_error = Some("process.pid execution hint missing".to_string());
        return sample;
    };

    match collect_process_runtime_facts(pid, &sample.execution_hints) {
        Ok(runtime_facts) => {
            sample.status = "succeeded".to_string();
            sample.runtime_facts = runtime_facts;
            sample
        }
        Err(err) => {
            sample.last_error = Some(err.to_string());
            sample
        }
    }
}

fn probe_container_target(target: &MetricsTargetViewEntry) -> MetricsCollectionTargetSample {
    let mut sample = MetricsCollectionTargetSample {
        candidate_id: target.candidate_id.clone(),
        target_ref: target.target_ref.clone(),
        status: "failed".to_string(),
        last_error: None,
        resource_ref: target.resource_ref.clone(),
        execution_hints: target.execution_hints.clone(),
        runtime_facts: Vec::new(),
    };

    let mut facts = Vec::new();
    for hint in &sample.execution_hints {
        if matches!(
            hint.key.as_str(),
            "container.runtime"
                | "container.runtime.namespace"
                | "cgroup.path"
                | "k8s.namespace.name"
                | "k8s.pod.uid"
                | "k8s.pod.name"
                | "k8s.container.name"
        ) {
            push_fact_if_absent(&mut facts, hint.clone());
        }
    }

    let mut process_probe_error = None;
    if let Some(pid) = sample
        .execution_hints
        .iter()
        .find(|hint| hint.key == "pid")
        .and_then(|hint| hint.value.parse::<u32>().ok())
    {
        match collect_process_runtime_facts(pid, &[]) {
            Ok(process_facts) => {
                push_fact_if_absent(
                    &mut facts,
                    StringKeyValue::new("container.pid", pid.to_string()),
                );
                for fact in process_facts {
                    push_fact_if_absent(&mut facts, fact);
                }
            }
            Err(err) => {
                process_probe_error = Some(err.to_string());
            }
        }
    }

    if facts.is_empty() {
        sample.last_error = process_probe_error
            .or_else(|| Some("container metrics probe produced no runtime facts".to_string()));
        return sample;
    }

    sample.status = if process_probe_error.is_some() {
        "partial".to_string()
    } else {
        "succeeded".to_string()
    };
    sample.last_error = process_probe_error;
    sample.runtime_facts = facts;
    sample
}

fn collect_process_runtime_facts(
    pid: u32,
    execution_hints: &[StringKeyValue],
) -> io::Result<Vec<StringKeyValue>> {
    let mut facts = Vec::new();
    push_fact_if_absent(
        &mut facts,
        StringKeyValue::new("process.pid", pid.to_string()),
    );
    for hint in execution_hints {
        if matches!(
            hint.key.as_str(),
            "process.identity" | "discovery.identity_strength" | "discovery.identity_status"
        ) {
            push_fact_if_absent(&mut facts, hint.clone());
        }
    }

    #[cfg(target_os = "linux")]
    {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
        if let Some(state) = parse_linux_proc_state(&stat) {
            push_fact_if_absent(
                &mut facts,
                StringKeyValue::new("process.state", state.to_string()),
            );
        }
        if let Some((utime, stime, rss_pages)) = parse_linux_proc_metrics(&stat) {
            push_fact_if_absent(
                &mut facts,
                StringKeyValue::new("process.cpu.user_ticks", utime.to_string()),
            );
            push_fact_if_absent(
                &mut facts,
                StringKeyValue::new("process.cpu.system_ticks", stime.to_string()),
            );
            push_fact_if_absent(
                &mut facts,
                StringKeyValue::new("process.memory.rss_pages", rss_pages.to_string()),
            );
        }
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let output = Command::new("ps")
            .args(["-o", "state=,rss=,comm=", "-p", &pid.to_string()])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other("ps did not exit successfully"));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.trim();
        if line.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "ps returned no process row",
            ));
        }
        let mut parts = line.split_whitespace();
        if let Some(state) = parts.next() {
            push_fact_if_absent(&mut facts, StringKeyValue::new("process.state", state));
        }
        if let Some(rss_kb) = parts.next() {
            push_fact_if_absent(
                &mut facts,
                StringKeyValue::new("process.memory.rss_kb", rss_kb),
            );
        }
        if let Some(comm) = parts.next() {
            push_fact_if_absent(
                &mut facts,
                StringKeyValue::new("process.executable.name", comm),
            );
        }
    }

    #[cfg(not(unix))]
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "process metrics probe unsupported on this platform",
        ));
    }

    Ok(facts)
}

#[cfg(target_os = "linux")]
fn parse_linux_proc_state(stat: &str) -> Option<char> {
    let (_, tail) = stat.rsplit_once(") ")?;
    tail.chars().next()
}

#[cfg(target_os = "linux")]
fn parse_linux_proc_metrics(stat: &str) -> Option<(u64, u64, i64)> {
    let (_, tail) = stat.rsplit_once(") ")?;
    let fields: Vec<&str> = tail.split_whitespace().collect();
    if fields.len() <= 21 {
        return None;
    }
    let utime = fields.get(11)?.parse().ok()?;
    let stime = fields.get(12)?.parse().ok()?;
    let rss_pages = fields.get(21)?.parse().ok()?;
    Some((utime, stime, rss_pages))
}

fn process_probe_mode() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "procfs"
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        "ps"
    }
    #[cfg(not(unix))]
    {
        "unsupported"
    }
}

fn container_probe_mode() -> &'static str {
    "discovery_hints"
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{MetricsRuntimeSnapshot, build_runtime_snapshot, path_for, store};
    use crate::telemetry::metrics::target_view::{MetricsTargetView, MetricsTargetViewEntry};
    use warp_insight_contracts::discovery::StringKeyValue;

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("warp-insight-metrics-runtime-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn build_runtime_snapshot_counts_batch_a_targets() {
        let state_dir = temp_dir("counts");
        let target_view = MetricsTargetView {
            generated_at: "2026-04-19T00:00:00Z".to_string(),
            targets: vec![
                MetricsTargetViewEntry {
                    candidate_id: "host-1".to_string(),
                    collection_kind: "host_metrics".to_string(),
                    target_ref: "host-1:host".to_string(),
                    resource_ref: "host-1".to_string(),
                    execution_hints: vec![StringKeyValue::new("host.name", "local-host")],
                },
                MetricsTargetViewEntry {
                    candidate_id: "proc-1".to_string(),
                    collection_kind: "process_metrics".to_string(),
                    target_ref: "proc-1".to_string(),
                    resource_ref: "proc-1".to_string(),
                    execution_hints: vec![StringKeyValue::new("process.pid", "42")],
                },
                MetricsTargetViewEntry {
                    candidate_id: "container-1".to_string(),
                    collection_kind: "container_metrics".to_string(),
                    target_ref: "container-1".to_string(),
                    resource_ref: "container-1".to_string(),
                    execution_hints: vec![
                        StringKeyValue::new("container.runtime", "containerd"),
                        StringKeyValue::new("pid", std::process::id().to_string()),
                    ],
                },
            ],
        };
        crate::telemetry::metrics::target_view::store(
            &crate::telemetry::metrics::target_view::path_for(&state_dir),
            &target_view,
        )
        .expect("store target view");

        let snapshot = build_runtime_snapshot(&state_dir).expect("build runtime snapshot");

        assert_eq!(snapshot.generated_at, "2026-04-19T00:00:00Z");
        assert_eq!(snapshot.total_targets, 3);
        assert_eq!(snapshot.host_targets, 1);
        assert_eq!(snapshot.process_targets, 1);
        assert_eq!(snapshot.container_targets, 1);

        let host_outcome = snapshot
            .outcomes
            .iter()
            .find(|outcome| outcome.collection_kind == "host_metrics")
            .expect("host outcome");
        assert_eq!(host_outcome.status, "succeeded");
        assert_eq!(host_outcome.attempted_targets, 1);
        assert_eq!(host_outcome.succeeded_targets, 1);
        assert_eq!(host_outcome.failed_targets, 0);
        assert!(host_outcome.last_error.is_none());
        assert!(!host_outcome.runtime_facts.is_empty());
        assert_eq!(host_outcome.sample_targets.len(), 1);

        let process_outcome = snapshot
            .outcomes
            .iter()
            .find(|outcome| outcome.collection_kind == "process_metrics")
            .expect("process outcome");
        assert!(matches!(
            process_outcome.status.as_str(),
            "succeeded" | "partial" | "failed"
        ));
        assert_eq!(process_outcome.attempted_targets, 1);
        assert_eq!(
            process_outcome.succeeded_targets + process_outcome.failed_targets,
            1
        );
        assert!(!process_outcome.runtime_facts.is_empty());
        assert_eq!(process_outcome.sample_targets.len(), 1);
        assert_eq!(
            process_outcome.sample_targets[0].status,
            if process_outcome.succeeded_targets == 1 {
                "succeeded"
            } else {
                "failed"
            }
        );

        let container_outcome = snapshot
            .outcomes
            .iter()
            .find(|outcome| outcome.collection_kind == "container_metrics")
            .expect("container outcome");
        assert!(matches!(
            container_outcome.status.as_str(),
            "succeeded" | "partial" | "failed"
        ));
        assert_eq!(container_outcome.attempted_targets, 1);
        assert_eq!(
            container_outcome.succeeded_targets + container_outcome.failed_targets,
            1
        );
        assert!(!container_outcome.runtime_facts.is_empty());
        assert_eq!(container_outcome.sample_targets.len(), 1);
    }

    #[test]
    fn store_runtime_snapshot_round_trip() {
        let state_dir = temp_dir("store");
        let snapshot = MetricsRuntimeSnapshot {
            generated_at: "2026-04-19T00:00:00Z".to_string(),
            total_targets: 0,
            host_targets: 0,
            process_targets: 0,
            container_targets: 0,
            outcomes: vec![
                super::MetricsCollectionOutcome {
                    collection_kind: "host_metrics".to_string(),
                    status: "idle".to_string(),
                    attempted_targets: 0,
                    succeeded_targets: 0,
                    failed_targets: 0,
                    last_error: None,
                    runtime_facts: Vec::new(),
                    sample_targets: Vec::new(),
                },
                super::MetricsCollectionOutcome {
                    collection_kind: "process_metrics".to_string(),
                    status: "idle".to_string(),
                    attempted_targets: 0,
                    succeeded_targets: 0,
                    failed_targets: 0,
                    last_error: None,
                    runtime_facts: Vec::new(),
                    sample_targets: Vec::new(),
                },
                super::MetricsCollectionOutcome {
                    collection_kind: "container_metrics".to_string(),
                    status: "idle".to_string(),
                    attempted_targets: 0,
                    succeeded_targets: 0,
                    failed_targets: 0,
                    last_error: None,
                    runtime_facts: Vec::new(),
                    sample_targets: Vec::new(),
                },
            ],
        };
        let snapshot_path = path_for(&state_dir);

        store(&snapshot_path, &snapshot).expect("store runtime snapshot");
        let loaded: MetricsRuntimeSnapshot =
            warp_insight_shared::fs::read_json(&snapshot_path).expect("load runtime snapshot");

        assert_eq!(loaded, snapshot);
    }

    #[test]
    fn build_runtime_snapshot_limits_samples_per_collection_kind() {
        let state_dir = temp_dir("sample-limit");
        let target_view = MetricsTargetView {
            generated_at: "2026-04-19T00:00:00Z".to_string(),
            targets: (0..5)
                .map(|idx| MetricsTargetViewEntry {
                    candidate_id: format!("proc-{idx}"),
                    collection_kind: "process_metrics".to_string(),
                    target_ref: format!("proc-{idx}"),
                    resource_ref: format!("proc-{idx}"),
                    execution_hints: vec![StringKeyValue::new("process.pid", idx.to_string())],
                })
                .collect(),
        };
        crate::telemetry::metrics::target_view::store(
            &crate::telemetry::metrics::target_view::path_for(&state_dir),
            &target_view,
        )
        .expect("store target view");

        let snapshot = build_runtime_snapshot(&state_dir).expect("build runtime snapshot");
        let process_outcome = snapshot
            .outcomes
            .iter()
            .find(|outcome| outcome.collection_kind == "process_metrics")
            .expect("process outcome");

        assert_eq!(process_outcome.attempted_targets, 5);
        assert_eq!(
            process_outcome.succeeded_targets + process_outcome.failed_targets,
            5
        );
        assert!(matches!(
            process_outcome.status.as_str(),
            "succeeded" | "partial" | "failed"
        ));
        assert_eq!(process_outcome.sample_targets.len(), 3);
    }

    #[test]
    fn build_runtime_snapshot_marks_process_target_failed_when_pid_hint_missing() {
        let state_dir = temp_dir("process-missing-pid");
        let target_view = MetricsTargetView {
            generated_at: "2026-04-19T00:00:00Z".to_string(),
            targets: vec![MetricsTargetViewEntry {
                candidate_id: "proc-1".to_string(),
                collection_kind: "process_metrics".to_string(),
                target_ref: "proc-1".to_string(),
                resource_ref: "proc-1".to_string(),
                execution_hints: vec![StringKeyValue::new("process.identity", "demo")],
            }],
        };
        crate::telemetry::metrics::target_view::store(
            &crate::telemetry::metrics::target_view::path_for(&state_dir),
            &target_view,
        )
        .expect("store target view");

        let snapshot = build_runtime_snapshot(&state_dir).expect("build runtime snapshot");
        let process_outcome = snapshot
            .outcomes
            .iter()
            .find(|outcome| outcome.collection_kind == "process_metrics")
            .expect("process outcome");

        assert_eq!(process_outcome.status, "failed");
        assert_eq!(process_outcome.attempted_targets, 1);
        assert_eq!(process_outcome.succeeded_targets, 0);
        assert_eq!(process_outcome.failed_targets, 1);
        assert!(
            process_outcome
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("process.pid"))
        );
        assert_eq!(process_outcome.sample_targets.len(), 1);
        assert_eq!(process_outcome.sample_targets[0].status, "failed");
    }

    #[test]
    fn build_runtime_snapshot_marks_container_target_failed_when_hints_missing() {
        let state_dir = temp_dir("container-missing-hints");
        let target_view = MetricsTargetView {
            generated_at: "2026-04-19T00:00:00Z".to_string(),
            targets: vec![MetricsTargetViewEntry {
                candidate_id: "container-1".to_string(),
                collection_kind: "container_metrics".to_string(),
                target_ref: "container-1".to_string(),
                resource_ref: "container-1".to_string(),
                execution_hints: Vec::new(),
            }],
        };
        crate::telemetry::metrics::target_view::store(
            &crate::telemetry::metrics::target_view::path_for(&state_dir),
            &target_view,
        )
        .expect("store target view");

        let snapshot = build_runtime_snapshot(&state_dir).expect("build runtime snapshot");
        let container_outcome = snapshot
            .outcomes
            .iter()
            .find(|outcome| outcome.collection_kind == "container_metrics")
            .expect("container outcome");

        assert_eq!(container_outcome.status, "failed");
        assert_eq!(container_outcome.attempted_targets, 1);
        assert_eq!(container_outcome.succeeded_targets, 0);
        assert_eq!(container_outcome.failed_targets, 1);
        assert!(
            container_outcome
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("no runtime facts"))
        );
        assert_eq!(container_outcome.sample_targets.len(), 1);
        assert_eq!(container_outcome.sample_targets[0].status, "failed");
    }
}
