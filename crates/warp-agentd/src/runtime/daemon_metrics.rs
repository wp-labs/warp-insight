use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use wist_contracts::telemetry_record::DataFrame;
use wist_shared::fs::read_json;
use wist_shared::time::{now_rfc3339, now_ts_ms};

use crate::self_observability::MetricsHealthSnapshot;
use crate::telemetry::metrics::{
    runtime::{self, MetricsRuntimeSnapshot},
    samples,
    target_view::{self, MetricsTargetView},
};
use crate::telemetry::warp_parse::TelemetryRecordSink;

#[derive(::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub(super) struct MetricsTick {
    pub(super) snapshot: Option<MetricsRuntimeSnapshot>,
    pub(super) failures: Vec<MetricsFailure>,
    pub(super) target_view_loaded: bool,
    pub(super) used_cached_snapshot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MetricsFailureKind {
    TargetViewLoad,
    RuntimeSnapshotLoad,
    RuntimeSnapshotStore,
}

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub(super) struct MetricsFailure {
    pub(super) kind: MetricsFailureKind,
    pub(super) phase: String,
    pub(super) path: String,
    pub(super) detail: String,
}

impl MetricsTick {
    pub(super) fn is_active(&self) -> bool {
        !self.failures.is_empty()
    }

    pub(super) fn health_snapshot(&self) -> MetricsHealthSnapshot {
        let (
            total_targets,
            host_targets,
            process_targets,
            container_targets,
            attempted_targets,
            succeeded_targets,
            failed_targets,
            updated_at,
        ) = match &self.snapshot {
            Some(snapshot) => (
                snapshot.total_targets,
                snapshot.host_targets,
                snapshot.process_targets,
                snapshot.container_targets,
                snapshot
                    .outcomes
                    .iter()
                    .map(|outcome| outcome.attempted_targets)
                    .sum(),
                snapshot
                    .outcomes
                    .iter()
                    .map(|outcome| outcome.succeeded_targets)
                    .sum(),
                snapshot
                    .outcomes
                    .iter()
                    .map(|outcome| outcome.failed_targets)
                    .sum(),
                Some(snapshot.generated_at.clone()),
            ),
            None => (0, 0, 0, 0, 0, 0, 0, None),
        };

        MetricsHealthSnapshot {
            target_view_loaded: self.target_view_loaded,
            used_cached_snapshot: self.used_cached_snapshot,
            total_targets,
            host_targets,
            process_targets,
            container_targets,
            attempted_targets,
            succeeded_targets,
            failed_targets,
            failure_count: self.failures.len(),
            last_error: self
                .failures
                .last()
                .map(|failure| format!("{}: {}", failure.phase, failure.detail)),
            updated_at,
        }
    }
}

/// 把指标运行时快照规范化成样本并拍平成 VM JSON lines 逐帧上送。无样本时跳过（避免空帧占用 uplink）。
pub(super) async fn write_metrics_uplink(
    sink: &mut TelemetryRecordSink,
    agent_id: &str,
    snapshot: &MetricsRuntimeSnapshot,
) -> io::Result<()> {
    let samples = samples::build_samples_snapshot(snapshot);
    let lines = samples::build_vm_metric_lines(&samples, agent_id, now_ts_ms());
    if lines.is_empty() {
        return Ok(());
    }
    let observed_at = now_rfc3339();
    for (index, line) in lines.iter().enumerate() {
        // TODO(W2)：信封 `seq` 应统一为 agent 级全局 `seq`（与日志同源、跨重启不回退）；
        // 在 W2 全局计数器落地前，以 batch_seq 为基 + 批内序号占位，保证批内各帧 `(agent, seq)` 唯一。
        let seq = samples
            .batch_seq
            .saturating_mul(10_000)
            .saturating_add(index as u64);
        let envelope = DataFrame::new(agent_id, observed_at.clone(), seq);
        sink.write_metrics(&envelope, line).await?;
    }
    Ok(())
}

pub(super) fn process_metrics_tick(state_dir: &Path) -> MetricsTick {
    let target_view_path = target_view::path_for(state_dir);
    let runtime_snapshot_path = runtime::path_for(state_dir);

    match read_json::<MetricsTargetView>(&target_view_path) {
        Ok(view) => {
            let snapshot = runtime::build_runtime_snapshot_from_view(&view);
            let mut failures = Vec::new();
            if let Err(err) = runtime::store(&runtime_snapshot_path, &snapshot) {
                failures.push(metrics_failure(
                    MetricsFailureKind::RuntimeSnapshotStore,
                    "runtime_snapshot_store",
                    &runtime_snapshot_path,
                    err,
                ));
            }
            MetricsTick {
                snapshot: Some(snapshot),
                failures,
                target_view_loaded: true,
                used_cached_snapshot: false,
            }
        }
        Err(err) => {
            let mut failures = vec![metrics_failure(
                MetricsFailureKind::TargetViewLoad,
                "target_view_load",
                &target_view_path,
                err,
            )];
            let cached_snapshot =
                load_cached_runtime_snapshot(&runtime_snapshot_path, &mut failures);

            MetricsTick {
                snapshot: cached_snapshot.clone(),
                failures,
                target_view_loaded: false,
                used_cached_snapshot: cached_snapshot.is_some(),
            }
        }
    }
}

pub(super) fn emit_metrics_tick(tick: &MetricsTick) {
    let health = tick.health_snapshot();
    eprintln!(
        "event=MetricsRuntimeUpdated target_view_loaded={} used_cached_snapshot={} total_targets={} host_targets={} process_targets={} container_targets={} attempted_targets={} succeeded_targets={} failed_targets={} failures={} updated_at={}",
        health.target_view_loaded,
        health.used_cached_snapshot,
        health.total_targets,
        health.host_targets,
        health.process_targets,
        health.container_targets,
        health.attempted_targets,
        health.succeeded_targets,
        health.failed_targets,
        health.failure_count,
        health.updated_at.as_deref().unwrap_or("-"),
    );
}

pub(super) fn emit_metrics_failures(failures: &[MetricsFailure]) {
    for failure in failures {
        emit_metrics_failure(failure);
    }
}

pub(super) fn emit_metrics_failure(failure: &MetricsFailure) {
    eprintln!(
        "event=MetricsRuntimeFailed kind={:?} phase={} path={} error={}",
        failure.kind, failure.phase, failure.path, failure.detail
    );
}

pub(super) fn failure_signatures(failures: &[MetricsFailure]) -> BTreeSet<String> {
    failures.iter().map(failure_signature).collect()
}

pub(super) fn filter_new_failures<'a>(
    failures: &'a [MetricsFailure],
    previous: &BTreeSet<String>,
) -> Vec<&'a MetricsFailure> {
    failures
        .iter()
        .filter(|failure| !previous.contains(&failure_signature(failure)))
        .collect()
}

fn failure_signature(failure: &MetricsFailure) -> String {
    format!(
        "{:?}|{}|{}|{}",
        failure.kind, failure.phase, failure.path, failure.detail
    )
}

fn load_cached_runtime_snapshot(
    path: &Path,
    failures: &mut Vec<MetricsFailure>,
) -> Option<MetricsRuntimeSnapshot> {
    if !path.exists() {
        return None;
    }

    match read_json(path) {
        Ok(snapshot) => Some(snapshot),
        Err(err) => {
            failures.push(metrics_failure(
                MetricsFailureKind::RuntimeSnapshotLoad,
                "runtime_snapshot_load",
                path,
                err,
            ));
            None
        }
    }
}

fn metrics_failure(
    kind: MetricsFailureKind,
    phase: &str,
    path: &Path,
    err: io::Error,
) -> MetricsFailure {
    MetricsFailure {
        kind,
        phase: phase.to_string(),
        path: path.display().to_string(),
        detail: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use wist_contracts::discovery::StringKeyValue;
    use wist_shared::fs::read_json;

    use super::{process_metrics_tick, write_metrics_uplink};
    use crate::telemetry::metrics::runtime::{
        MetricsCollectionOutcome, MetricsCollectionTargetSample, MetricsRuntimeSnapshot,
        path_for as runtime_path_for,
    };
    use crate::telemetry::metrics::target_view::{
        MetricsTargetView, MetricsTargetViewEntry, path_for as target_view_path_for, store,
    };
    use crate::telemetry::warp_parse::{TcpFraming, TcpRecordSink, TelemetryRecordSink};

    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("warp-insight-daemon-metrics-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn process_metrics_tick_builds_and_stores_runtime_snapshot_from_target_view() {
        let state_dir = temp_dir("build");
        let view = MetricsTargetView {
            generated_at: "2026-04-19T00:00:00Z".to_string(),
            targets: vec![
                MetricsTargetViewEntry {
                    candidate_id: "host-1".to_string(),
                    collection_kind: "host_metrics".to_string(),
                    target_ref: "host-1:host".to_string(),
                    resource_ref: "host-1".to_string(),
                    execution_hints: vec![StringKeyValue::new("host.name", "host-a")],
                },
                MetricsTargetViewEntry {
                    candidate_id: "proc-1".to_string(),
                    collection_kind: "process_metrics".to_string(),
                    target_ref: "proc-1".to_string(),
                    resource_ref: "proc-1".to_string(),
                    execution_hints: vec![StringKeyValue::new("process.pid", "42")],
                },
            ],
        };
        store(&target_view_path_for(&state_dir), &view).expect("store target view");

        let tick = process_metrics_tick(&state_dir);
        let stored: MetricsRuntimeSnapshot =
            read_json(&runtime_path_for(&state_dir)).expect("load runtime snapshot");

        assert!(tick.target_view_loaded);
        assert!(!tick.used_cached_snapshot);
        assert!(tick.failures.is_empty());
        assert_eq!(tick.snapshot, Some(stored.clone()));
        assert_eq!(stored.total_targets, 2);
        assert_eq!(stored.host_targets, 1);
        assert_eq!(stored.process_targets, 1);
        assert_eq!(stored.container_targets, 0);
    }

    #[test]
    fn process_metrics_tick_uses_cached_runtime_snapshot_when_target_view_is_missing() {
        let state_dir = temp_dir("cached");
        let cached = MetricsRuntimeSnapshot {
            generated_at: "2026-04-19T00:00:00Z".to_string(),
            total_targets: 3,
            host_targets: 1,
            process_targets: 1,
            container_targets: 1,
            outcomes: vec![
                crate::telemetry::metrics::runtime::MetricsCollectionOutcome {
                    collection_kind: "host_metrics".to_string(),
                    status: "succeeded".to_string(),
                    attempted_targets: 1,
                    succeeded_targets: 1,
                    failed_targets: 0,
                    last_error: None,
                    runtime_facts: vec![StringKeyValue::new("host.loadavg.1m", "0.10")],
                    sample_targets: Vec::new(),
                },
                crate::telemetry::metrics::runtime::MetricsCollectionOutcome {
                    collection_kind: "process_metrics".to_string(),
                    status: "succeeded".to_string(),
                    attempted_targets: 1,
                    succeeded_targets: 1,
                    failed_targets: 0,
                    last_error: None,
                    runtime_facts: vec![StringKeyValue::new("process.pid", "42")],
                    sample_targets: Vec::new(),
                },
                crate::telemetry::metrics::runtime::MetricsCollectionOutcome {
                    collection_kind: "container_metrics".to_string(),
                    status: "succeeded".to_string(),
                    attempted_targets: 1,
                    succeeded_targets: 1,
                    failed_targets: 0,
                    last_error: None,
                    runtime_facts: vec![StringKeyValue::new("container.runtime", "containerd")],
                    sample_targets: Vec::new(),
                },
            ],
        };
        crate::telemetry::metrics::runtime::store(&runtime_path_for(&state_dir), &cached)
            .expect("store cached runtime snapshot");

        let tick = process_metrics_tick(&state_dir);

        assert!(!tick.target_view_loaded);
        assert!(tick.used_cached_snapshot);
        assert_eq!(tick.snapshot, Some(cached));
        assert_eq!(tick.failures.len(), 1);
        assert_eq!(tick.failures[0].phase, "target_view_load");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn write_metrics_uplink_sends_metrics_frame() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(err) => panic!("bind listener: {err}"),
        };
        let port = listener.local_addr().expect("listener addr").port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buf = vec![0u8; 2048];
            let n = socket.read(&mut buf).await.expect("read");
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        let mut sink = TelemetryRecordSink::Tcp(TcpRecordSink::new(
            "127.0.0.1".to_string(),
            port,
            TcpFraming::Line,
        ));

        let snapshot = MetricsRuntimeSnapshot {
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
                    execution_hints: vec![StringKeyValue::new("host.name", "host-a")],
                    runtime_facts: vec![StringKeyValue::new("host.loadavg.1m", "0.25")],
                }],
            }],
        };

        write_metrics_uplink(&mut sink, "agent-001", &snapshot)
            .await
            .expect("write metrics uplink");

        let body = server.await.expect("join");
        assert!(body.contains(" METRICS: "), "frame: {body}");
        assert!(body.contains("\"agent\":\"agent-001\""), "frame: {body}");
        assert!(
            body.contains("system.load_average.1m"),
            "normalized sample: {body}"
        );
    }
}
