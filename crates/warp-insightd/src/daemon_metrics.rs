use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use warp_insight_shared::fs::read_json;

use crate::self_observability::MetricsHealthSnapshot;
use crate::telemetry::metrics::{
    runtime::{self, MetricsRuntimeSnapshot},
    target_view::{self, MetricsTargetView},
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
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

    use warp_insight_contracts::discovery::StringKeyValue;
    use warp_insight_shared::fs::read_json;

    use super::process_metrics_tick;
    use crate::telemetry::metrics::runtime::{
        MetricsRuntimeSnapshot, path_for as runtime_path_for,
    };
    use crate::telemetry::metrics::target_view::{
        MetricsTargetView, MetricsTargetViewEntry, path_for as target_view_path_for, store,
    };

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
}
