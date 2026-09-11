//! Planner candidate state persistence.

use std::io;
use std::path::{Path, PathBuf};

use crate::fs_async::{read_json_async, write_json_atomic_async};
use wist_contracts::discovery::CandidateCollectionTarget;
use wist_shared::fs::{read_json, write_json_atomic};

const PLANNER_DIR: &str = "planner";
const HOST_METRICS_CANDIDATES_FILE: &str = "host_metrics_candidates.json";
const PROCESS_METRICS_CANDIDATES_FILE: &str = "process_metrics_candidates.json";
const CONTAINER_METRICS_CANDIDATES_FILE: &str = "container_metrics_candidates.json";

pub fn host_metrics_path_for(state_dir: &Path) -> PathBuf {
    state_dir
        .join(PLANNER_DIR)
        .join(HOST_METRICS_CANDIDATES_FILE)
}

pub fn process_metrics_path_for(state_dir: &Path) -> PathBuf {
    state_dir
        .join(PLANNER_DIR)
        .join(PROCESS_METRICS_CANDIDATES_FILE)
}

pub fn container_metrics_path_for(state_dir: &Path) -> PathBuf {
    state_dir
        .join(PLANNER_DIR)
        .join(CONTAINER_METRICS_CANDIDATES_FILE)
}

pub fn load_or_default(path: &Path) -> io::Result<Vec<CandidateCollectionTarget>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    read_json(path)
}

pub fn store(path: &Path, candidates: &[CandidateCollectionTarget]) -> io::Result<()> {
    write_json_atomic(path, &candidates.to_vec())
}

pub async fn load_or_default_async(path: &Path) -> io::Result<Vec<CandidateCollectionTarget>> {
    match tokio::fs::metadata(path).await {
        Ok(_) => read_json_async(path).await,
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err),
    }
}

pub async fn store_async(path: &Path, candidates: &[CandidateCollectionTarget]) -> io::Result<()> {
    write_json_atomic_async(path, &candidates.to_vec()).await
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use wist_contracts::discovery::CandidateCollectionTarget;

    use super::{host_metrics_path_for, load_or_default, process_metrics_path_for, store};

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("warp-insight-planner-candidates-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn store_and_load_candidates_round_trip() {
        let state_dir = temp_dir("round-trip");
        let path = host_metrics_path_for(&state_dir);
        let candidates = vec![CandidateCollectionTarget {
            candidate_id: "host-1:host:host_metrics".to_string(),
            target_ref: "host-1:host".to_string(),
            collection_kind: "host_metrics".to_string(),
            resource_ref: "host-1".to_string(),
            execution_hints: Vec::new(),
            generated_at: "2026-04-19T00:00:00Z".to_string(),
        }];

        store(&path, &candidates).expect("store candidates");
        let loaded = load_or_default(&path).expect("load candidates");

        assert_eq!(loaded, candidates);
    }

    #[test]
    fn path_builders_use_separate_files() {
        let state_dir = temp_dir("paths");

        assert_ne!(
            host_metrics_path_for(&state_dir),
            process_metrics_path_for(&state_dir)
        );
    }
}
