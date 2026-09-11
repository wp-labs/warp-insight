use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::state_store::log_checkpoint_state::{LogCheckpointState, TrackedFileCheckpoint};
use crate::telemetry::logs::files::file_reader::{
    ObservedFileIdentity, checkpoint_probe_async, inspect_path_async, stable_file_id_async,
};

pub(super) fn checkpoint_for_path(
    state: &LogCheckpointState,
    source_path: &Path,
) -> Option<TrackedFileCheckpoint> {
    let source_path = source_path.display().to_string();
    state
        .files
        .iter()
        .find(|entry| entry.path == source_path)
        .cloned()
}

pub(super) fn relocate_checkpoint_path(
    state: &mut LogCheckpointState,
    previous: &TrackedFileCheckpoint,
    rotated_path: &Path,
) {
    let rotated_path = rotated_path.display().to_string();
    if let Some(existing) = state
        .files
        .iter_mut()
        .find(|entry| entry.file_id == previous.file_id || entry.path == previous.path)
    {
        existing.path = rotated_path;
    }
}

pub(super) async fn find_rotated_path_async(
    source_path: &Path,
    previous: &TrackedFileCheckpoint,
) -> io::Result<Option<PathBuf>> {
    let Some(parent) = source_path.parent() else {
        return Ok(None);
    };
    let mut dir = tokio::fs::read_dir(parent).await?;
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path == source_path {
            continue;
        }
        if rotated_entry_matches(previous, &path, &entry).await? {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// 判断目录中的某个条目是否为 `previous` 轮转后的文件：优先按 device/inode 精确匹配，
/// 缺失 inode 时退化为按 fingerprint 匹配。
async fn rotated_entry_matches(
    previous: &TrackedFileCheckpoint,
    path: &Path,
    entry: &tokio::fs::DirEntry,
) -> io::Result<bool> {
    let metadata = entry.metadata().await?;
    if inode_pair_matches(
        previous.device_id,
        previous.inode,
        metadata_device_id(&metadata),
        metadata_inode(&metadata),
    ) {
        return Ok(true);
    }
    Ok(matches_by_fingerprint(previous, path).await)
}

async fn matches_by_fingerprint(previous: &TrackedFileCheckpoint, path: &Path) -> bool {
    if previous.device_id.is_some() || previous.inode.is_some() {
        return false;
    }
    let Some(fingerprint) = previous.fingerprint.as_deref() else {
        return false;
    };
    match inspect_path_async(path).await {
        Ok(identity) => identity.fingerprint.as_deref() == Some(fingerprint),
        Err(_) => false,
    }
}

/// 当两侧 device/inode 均存在且相等时返回 true，否则 false。
fn inode_pair_matches(
    left_device: Option<u64>,
    left_inode: Option<u64>,
    right_device: Option<u64>,
    right_inode: Option<u64>,
) -> bool {
    match (left_device, left_inode, right_device, right_inode) {
        (Some(left_device), Some(left_inode), Some(right_device), Some(right_inode)) => {
            left_device == right_device && left_inode == right_inode
        }
        _ => false,
    }
}

pub(super) async fn upsert_checkpoint_async(
    state: &mut LogCheckpointState,
    source_path: &Path,
    identity: &ObservedFileIdentity,
    checkpoint_offset: u64,
    observed_at: &str,
    rotated_from_path: Option<String>,
) {
    let source_path = source_path.display().to_string();
    let file_id = stable_file_id_async(Path::new(&source_path), identity).await;
    let checkpoint_probe = checkpoint_probe_async(Path::new(&source_path), checkpoint_offset)
        .await
        .ok()
        .flatten();
    if let Some(existing) = state.files.iter_mut().find(|entry| {
        entry.file_id == file_id || stored_identity_matches(entry, identity, &source_path)
    }) {
        let path = existing.path.clone();
        *existing = checkpoint_from_observation(
            file_id,
            path,
            identity,
            checkpoint_offset,
            checkpoint_probe,
            observed_at,
            rotated_from_path,
        );
        return;
    }

    state.files.push(checkpoint_from_observation(
        file_id,
        source_path,
        identity,
        checkpoint_offset,
        checkpoint_probe,
        observed_at,
        rotated_from_path,
    ));
}

/// 由一次观测结果构造/更新 checkpoint：除 `file_id`/`path` 外全部取观测值。
fn checkpoint_from_observation(
    file_id: String,
    path: String,
    identity: &ObservedFileIdentity,
    checkpoint_offset: u64,
    checkpoint_probe: Option<String>,
    observed_at: &str,
    rotated_from_path: Option<String>,
) -> TrackedFileCheckpoint {
    TrackedFileCheckpoint {
        file_id,
        path,
        device_id: identity.device_id,
        inode: identity.inode,
        fingerprint: identity.fingerprint.clone(),
        checkpoint_offset,
        checkpoint_probe,
        last_size: Some(identity.file_len),
        last_read_at: Some(observed_at.to_string()),
        last_commit_point_at: Some(observed_at.to_string()),
        rotated_from_path,
    }
}

#[cfg(test)]
pub(super) fn upsert_checkpoint(
    state: &mut LogCheckpointState,
    source_path: &Path,
    identity: &ObservedFileIdentity,
    checkpoint_offset: u64,
    observed_at: &str,
    rotated_from_path: Option<String>,
) {
    block_on(upsert_checkpoint_async(
        state,
        source_path,
        identity,
        checkpoint_offset,
        observed_at,
        rotated_from_path,
    ))
}

#[cfg(test)]
fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
        .block_on(future)
}

fn stored_identity_matches(
    checkpoint: &TrackedFileCheckpoint,
    identity: &ObservedFileIdentity,
    source_path: &str,
) -> bool {
    if inode_pair_matches(
        checkpoint.device_id,
        checkpoint.inode,
        identity.device_id,
        identity.inode,
    ) {
        return true;
    }
    checkpoint.path == source_path
        && checkpoint.fingerprint.is_some()
        && checkpoint.fingerprint == identity.fingerprint
}

#[cfg(unix)]
fn metadata_device_id(metadata: &fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.dev())
}

#[cfg(not(unix))]
fn metadata_device_id(_metadata: &fs::Metadata) -> Option<u64> {
    None
}

#[cfg(unix)]
fn metadata_inode(metadata: &fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.ino())
}

#[cfg(not(unix))]
fn metadata_inode(_metadata: &fs::Metadata) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::{
        checkpoint_for_path, find_rotated_path_async, relocate_checkpoint_path,
        stored_identity_matches, upsert_checkpoint,
    };
    use crate::state_store::log_checkpoint_state::{LogCheckpointState, TrackedFileCheckpoint};
    use crate::telemetry::logs::files::file_reader::{ObservedFileIdentity, inspect_path_async};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("warp-agentd-checkpoint-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn checkpoint() -> TrackedFileCheckpoint {
        TrackedFileCheckpoint {
            file_id: String::new(),
            path: String::new(),
            device_id: None,
            inode: None,
            fingerprint: None,
            checkpoint_offset: 0,
            checkpoint_probe: None,
            last_size: None,
            last_read_at: None,
            last_commit_point_at: None,
            rotated_from_path: None,
        }
    }

    fn identity(
        device_id: Option<u64>,
        inode: Option<u64>,
        fingerprint: Option<&str>,
    ) -> ObservedFileIdentity {
        ObservedFileIdentity {
            device_id,
            inode,
            fingerprint: fingerprint.map(str::to_string),
            file_len: 0,
        }
    }

    #[test]
    fn checkpoint_for_path_returns_matching_entry() {
        let mut state =
            LogCheckpointState::new("input-app".to_string(), "2026-04-13T00:00:00Z".to_string());
        state.files.push(TrackedFileCheckpoint {
            path: "/tmp/a.log".to_string(),
            ..checkpoint()
        });
        state.files.push(TrackedFileCheckpoint {
            path: "/tmp/b.log".to_string(),
            ..checkpoint()
        });

        let found = checkpoint_for_path(&state, Path::new("/tmp/b.log"));
        assert_eq!(found.map(|entry| entry.path).as_deref(), Some("/tmp/b.log"));
        assert!(checkpoint_for_path(&state, Path::new("/tmp/missing.log")).is_none());
    }

    #[test]
    fn relocate_checkpoint_path_updates_entry_by_file_id() {
        let mut state =
            LogCheckpointState::new("input-app".to_string(), "2026-04-13T00:00:00Z".to_string());
        state.files.push(TrackedFileCheckpoint {
            file_id: "dev:1:ino:2".to_string(),
            path: "/tmp/a.log".to_string(),
            ..checkpoint()
        });

        let previous = TrackedFileCheckpoint {
            file_id: "dev:1:ino:2".to_string(),
            path: "/tmp/a.log".to_string(),
            ..checkpoint()
        };
        relocate_checkpoint_path(&mut state, &previous, Path::new("/tmp/a.log.1"));

        assert_eq!(state.files[0].path, "/tmp/a.log.1");
    }

    #[test]
    fn relocate_checkpoint_path_updates_entry_by_path() {
        let mut state =
            LogCheckpointState::new("input-app".to_string(), "2026-04-13T00:00:00Z".to_string());
        state.files.push(TrackedFileCheckpoint {
            file_id: "dev:1:ino:2".to_string(),
            path: "/tmp/a.log".to_string(),
            ..checkpoint()
        });

        // previous 的 file_id 与条目不同，但 path 相同 → 仍应命中并按 path 迁移。
        let previous = TrackedFileCheckpoint {
            file_id: "dev:9:ino:9".to_string(),
            path: "/tmp/a.log".to_string(),
            ..checkpoint()
        };
        relocate_checkpoint_path(&mut state, &previous, Path::new("/tmp/a.log.2"));

        assert_eq!(state.files[0].path, "/tmp/a.log.2");
    }

    #[tokio::test]
    async fn find_rotated_path_matches_by_fingerprint() {
        let dir = temp_dir("rotated-fingerprint");
        let source = dir.join("app.log");
        let rotated = dir.join("app.log.1");
        fs::write(&source, "new content").expect("write source");
        fs::write(&rotated, "old content").expect("write rotated");

        let rotated_identity = inspect_path_async(&rotated).await.expect("inspect rotated");
        let previous = TrackedFileCheckpoint {
            path: source.display().to_string(),
            fingerprint: rotated_identity.fingerprint,
            ..checkpoint()
        };

        let found = find_rotated_path_async(&source, &previous)
            .await
            .expect("find");
        assert_eq!(found.as_deref(), Some(rotated.as_path()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn find_rotated_path_returns_none_when_no_entry_matches() {
        let dir = temp_dir("rotated-none");
        let source = dir.join("app.log");
        fs::write(&source, "content").expect("write source");

        let previous = TrackedFileCheckpoint {
            path: source.display().to_string(),
            device_id: Some(1),
            inode: Some(1),
            ..checkpoint()
        };

        let found = find_rotated_path_async(&source, &previous)
            .await
            .expect("find");
        assert_eq!(found, None);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn find_rotated_path_returns_none_when_source_has_no_parent() {
        let previous = checkpoint();
        let found = find_rotated_path_async(Path::new("/"), &previous)
            .await
            .expect("find");
        assert_eq!(found, None);
    }

    #[test]
    fn stored_identity_matches_prefers_inode_when_present() {
        let checkpoint = TrackedFileCheckpoint {
            device_id: Some(1),
            inode: Some(2),
            fingerprint: Some("old".to_string()),
            ..checkpoint()
        };
        let same = identity(Some(1), Some(2), Some("new"));
        assert!(stored_identity_matches(&checkpoint, &same, "/tmp/app.log"));

        let different_inode = identity(Some(1), Some(3), Some("new"));
        assert!(!stored_identity_matches(
            &checkpoint,
            &different_inode,
            "/tmp/app.log"
        ));
    }

    #[test]
    fn stored_identity_matches_falls_back_to_path_and_fingerprint() {
        let checkpoint = TrackedFileCheckpoint {
            path: "/tmp/app.log".to_string(),
            fingerprint: Some("abc".to_string()),
            ..checkpoint()
        };
        let same = identity(None, None, Some("abc"));
        assert!(stored_identity_matches(&checkpoint, &same, "/tmp/app.log"));

        // path 不同 → 不匹配。
        assert!(!stored_identity_matches(
            &checkpoint,
            &same,
            "/tmp/other.log"
        ));
        // fingerprint 不同 → 不匹配。
        let different_fingerprint = identity(None, None, Some("def"));
        assert!(!stored_identity_matches(
            &checkpoint,
            &different_fingerprint,
            "/tmp/app.log"
        ));
    }

    #[test]
    fn upsert_checkpoint_merges_entries_with_same_inode() {
        let observed_at = "2026-04-13T00:00:00Z";
        let mut state = LogCheckpointState::new("input-app".to_string(), observed_at.to_string());

        upsert_checkpoint(
            &mut state,
            Path::new("/tmp/app.log"),
            &identity(Some(1), Some(2), Some("abc")),
            0,
            observed_at,
            None,
        );
        // 同 inode、不同路径（轮转后）→ 合并为一条，保留原 path，更新 offset。
        upsert_checkpoint(
            &mut state,
            Path::new("/tmp/app.log.1"),
            &identity(Some(1), Some(2), Some("abc")),
            5,
            observed_at,
            Some("/tmp/app.log".to_string()),
        );

        assert_eq!(state.files.len(), 1);
        assert_eq!(state.files[0].path, "/tmp/app.log");
        assert_eq!(state.files[0].checkpoint_offset, 5);
        assert_eq!(
            state.files[0].rotated_from_path.as_deref(),
            Some("/tmp/app.log")
        );
    }

    #[test]
    fn upsert_checkpoint_keeps_distinct_inodes_separate() {
        let observed_at = "2026-04-13T00:00:00Z";
        let mut state = LogCheckpointState::new("input-app".to_string(), observed_at.to_string());

        upsert_checkpoint(
            &mut state,
            Path::new("/tmp/app.log"),
            &identity(Some(1), Some(2), None),
            0,
            observed_at,
            None,
        );
        upsert_checkpoint(
            &mut state,
            Path::new("/tmp/app.log.1"),
            &identity(Some(1), Some(3), None),
            0,
            observed_at,
            None,
        );

        assert_eq!(state.files.len(), 2);
    }
}
