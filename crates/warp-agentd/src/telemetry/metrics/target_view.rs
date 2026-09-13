//! Read-only metrics target view built from planner candidates.

use std::io;
use std::path::Path;

use crate::fs_async::write_json_atomic_async;
use serde::{Deserialize, Serialize};
use wist_contracts::discovery::CandidateCollectionTarget;

use crate::state_store::planner_candidates;

pub use wist_metrics::target::MetricsTargetViewEntry;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[serde(deny_unknown_fields)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct MetricsTargetView {
    pub generated_at: String,
    #[serde(default)]
    pub targets: Vec<MetricsTargetViewEntry>,
}

#[cfg(test)]
pub fn build_metrics_target_view(
    state_dir: &Path,
    generated_at: &str,
) -> io::Result<MetricsTargetView> {
    let mut targets = Vec::new();

    for path in [
        planner_candidates::host_metrics_path_for(state_dir),
        planner_candidates::process_metrics_path_for(state_dir),
        planner_candidates::container_metrics_path_for(state_dir),
    ] {
        let candidates = planner_candidates::load_or_default(&path)?;
        targets.extend(candidates.into_iter().map(map_candidate));
    }

    Ok(MetricsTargetView {
        generated_at: generated_at.to_string(),
        targets,
    })
}

pub fn path_for(state_dir: &Path) -> std::path::PathBuf {
    state_dir.join("telemetry").join("metrics_target_view.json")
}

#[cfg(test)]
pub fn store(path: &Path, view: &MetricsTargetView) -> io::Result<()> {
    wist_shared::fs::write_json_atomic(path, view)
}

pub async fn build_metrics_target_view_async(
    state_dir: &Path,
    generated_at: &str,
) -> io::Result<MetricsTargetView> {
    let mut targets = Vec::new();

    for path in [
        planner_candidates::host_metrics_path_for(state_dir),
        planner_candidates::process_metrics_path_for(state_dir),
        planner_candidates::container_metrics_path_for(state_dir),
    ] {
        let candidates = planner_candidates::load_or_default_async(&path).await?;
        targets.extend(candidates.into_iter().map(map_candidate));
    }

    Ok(MetricsTargetView {
        generated_at: generated_at.to_string(),
        targets,
    })
}

pub async fn store_async(path: &Path, view: &MetricsTargetView) -> io::Result<()> {
    write_json_atomic_async(path, view).await
}

fn map_candidate(candidate: CandidateCollectionTarget) -> MetricsTargetViewEntry {
    MetricsTargetViewEntry {
        candidate_id: candidate.candidate_id,
        collection_kind: candidate.collection_kind,
        target_ref: candidate.target_ref,
        resource_ref: candidate.resource_ref,
        execution_hints: candidate.execution_hints,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use wist_contracts::discovery::{CandidateCollectionTarget, StringKeyValue};
    use wist_shared::fs::read_json;

    use super::{build_metrics_target_view, path_for, store};
    use crate::state_store::planner_candidates;

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("warp-insight-metrics-target-view-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn build_metrics_target_view_collects_batch_a_candidates() {
        let state_dir = temp_dir("view");
        planner_candidates::store(
            &planner_candidates::host_metrics_path_for(&state_dir),
            &[CandidateCollectionTarget {
                candidate_id: "host-1:host:host_metrics".to_string(),
                target_ref: "host-1:host".to_string(),
                collection_kind: "host_metrics".to_string(),
                resource_ref: "host-1".to_string(),
                execution_hints: vec![StringKeyValue::new("host.name", "host-a")],
                generated_at: "2026-04-19T00:00:00Z".to_string(),
            }],
        )
        .expect("store host candidates");
        planner_candidates::store(
            &planner_candidates::process_metrics_path_for(&state_dir),
            &[CandidateCollectionTarget {
                candidate_id: "proc-1:process_metrics".to_string(),
                target_ref: "proc-1".to_string(),
                collection_kind: "process_metrics".to_string(),
                resource_ref: "proc-1".to_string(),
                execution_hints: vec![StringKeyValue::new("process.pid", "42")],
                generated_at: "2026-04-19T00:00:00Z".to_string(),
            }],
        )
        .expect("store process candidates");

        let view = build_metrics_target_view(&state_dir, "2026-04-19T00:00:00Z")
            .expect("build metrics target view");

        assert_eq!(view.targets.len(), 2);
        assert!(
            view.targets
                .iter()
                .any(|target| target.collection_kind == "host_metrics")
        );
        assert!(
            view.targets
                .iter()
                .any(|target| target.collection_kind == "process_metrics")
        );
        assert!(view.targets.iter().any(|target| {
            target.collection_kind == "process_metrics"
                && target
                    .execution_hints
                    .iter()
                    .any(|hint| hint.key == "process.pid")
        }));
    }

    #[test]
    fn store_metrics_target_view_round_trip() {
        let state_dir = temp_dir("store");
        let view = build_metrics_target_view(&state_dir, "2026-04-19T00:00:00Z")
            .expect("empty target view");
        let view_path = path_for(&state_dir);

        store(&view_path, &view).expect("store target view");
        let loaded: super::MetricsTargetView = read_json(&view_path).expect("load target view");

        assert_eq!(loaded, view);
    }
}
