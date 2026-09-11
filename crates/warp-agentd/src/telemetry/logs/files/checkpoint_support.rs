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
        let metadata = entry.metadata().await?;
        let device_id = metadata_device_id(&metadata);
        let inode = metadata_inode(&metadata);
        if previous.device_id.is_some()
            && previous.inode.is_some()
            && previous.device_id == device_id
            && previous.inode == inode
        {
            return Ok(Some(path));
        }
        if previous.device_id.is_none() && previous.inode.is_none() {
            if let Some(fingerprint) = previous.fingerprint.as_deref() {
                if let Ok(identity) = inspect_path_async(&path).await {
                    if identity.fingerprint.as_deref() == Some(fingerprint) {
                        return Ok(Some(path));
                    }
                }
            }
        }
    }
    Ok(None)
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
        existing.file_id = file_id;
        existing.device_id = identity.device_id;
        existing.inode = identity.inode;
        existing.fingerprint = identity.fingerprint.clone();
        existing.checkpoint_offset = checkpoint_offset;
        existing.checkpoint_probe = checkpoint_probe;
        existing.last_size = Some(identity.file_len);
        existing.last_read_at = Some(observed_at.to_string());
        existing.last_commit_point_at = Some(observed_at.to_string());
        existing.rotated_from_path = rotated_from_path;
        return;
    }

    state.files.push(TrackedFileCheckpoint {
        file_id,
        path: source_path,
        device_id: identity.device_id,
        inode: identity.inode,
        fingerprint: identity.fingerprint.clone(),
        checkpoint_offset,
        checkpoint_probe,
        last_size: Some(identity.file_len),
        last_read_at: Some(observed_at.to_string()),
        last_commit_point_at: Some(observed_at.to_string()),
        rotated_from_path,
    });
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
    if checkpoint.device_id.is_some()
        && checkpoint.inode.is_some()
        && identity.device_id.is_some()
        && identity.inode.is_some()
    {
        return checkpoint.device_id == identity.device_id && checkpoint.inode == identity.inode;
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
