//! `state/logs/file_inputs/*/checkpoints.json` store.

use std::io;
use std::path::{Path, PathBuf};

use crate::fs_async::{read_json_async, write_json_atomic_async};
use wist_shared::time::now_rfc3339;

use crate::state_store::log_checkpoint_state::LogCheckpointState;

pub(crate) fn load_or_default(input_id: &str) -> LogCheckpointState {
    LogCheckpointState::new(input_id.to_string(), now_rfc3339())
}

pub fn path_for(state_dir: &Path, input_id: &str) -> PathBuf {
    state_dir
        .join("logs")
        .join("file_inputs")
        .join(input_id)
        .join("checkpoints.json")
}

pub(crate) async fn load_or_default_from_path_async(
    path: &Path,
    input_id: &str,
) -> io::Result<LogCheckpointState> {
    match tokio::fs::metadata(path).await {
        Ok(_) => read_json_async(path).await,
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(load_or_default(input_id)),
        Err(err) => Err(err),
    }
}

pub(crate) async fn store_async(path: &Path, state: &LogCheckpointState) -> io::Result<()> {
    write_json_atomic_async(path, state).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        std::env::temp_dir().join(format!("warp-agentd-log-checkpoints-{name}-{suffix}"))
    }

    #[tokio::test]
    async fn store_and_load_async_round_trip() {
        let root = temp_dir("round-trip");
        let path = path_for(&root, "app");
        let state = LogCheckpointState::new("app".to_string(), "2026-04-13T00:00:00Z".to_string());

        store_async(&path, &state).await.expect("store");
        let loaded = load_or_default_from_path_async(&path, "app")
            .await
            .expect("load");

        assert_eq!(loaded, state);
        let _ = tokio::fs::remove_dir_all(&root).await;
    }

    #[tokio::test]
    async fn load_async_defaults_when_missing() {
        let root = temp_dir("default");
        let path = path_for(&root, "app");

        let loaded = load_or_default_from_path_async(&path, "app")
            .await
            .expect("load");

        assert_eq!(loaded.input_id, "app");
        assert!(loaded.files.is_empty());
    }
}
