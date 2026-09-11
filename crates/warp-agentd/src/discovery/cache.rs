//! Persistent discovery cache helpers.

use std::io;
use std::path::{Path, PathBuf};

use orion_error::prelude::*;

use crate::error::{DiscoveryReason, DiscoveryResult};
use crate::fs_async::{read_json_async, write_json_atomic_async};
use wist_contracts::discovery::{DiscoveryCacheMeta, DiscoverySnapshotContract};
use wist_shared::fs::{read_json, write_json_atomic};

pub const DISCOVERY_STATE_DIR: &str = "discovery";
pub const DISCOVERY_RESOURCES_FILE: &str = "resources.json";
pub const DISCOVERY_TARGETS_FILE: &str = "targets.json";
pub const DISCOVERY_META_FILE: &str = "meta.json";

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
pub struct DiscoveryCachePaths {
    pub root: PathBuf,
    pub resources: PathBuf,
    pub targets: PathBuf,
    pub meta: PathBuf,
}

impl DiscoveryCachePaths {
    pub fn under_state_dir(state_dir: &Path) -> Self {
        let root = state_dir.join(DISCOVERY_STATE_DIR);
        Self {
            resources: root.join(DISCOVERY_RESOURCES_FILE),
            targets: root.join(DISCOVERY_TARGETS_FILE),
            meta: root.join(DISCOVERY_META_FILE),
            root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
pub struct DiscoveryCacheLoadFailure {
    pub phase: &'static str,
    pub detail: String,
}

pub fn load_snapshot(
    paths: &DiscoveryCachePaths,
) -> (
    Option<DiscoverySnapshotContract>,
    Option<DiscoveryCacheLoadFailure>,
) {
    if !paths.meta.exists() || !paths.resources.exists() || !paths.targets.exists() {
        return (None, None);
    }

    let meta: DiscoveryCacheMeta = match read_json(&paths.meta) {
        Ok(meta) => meta,
        Err(err) => {
            return (
                None,
                Some(DiscoveryCacheLoadFailure {
                    phase: "cache_load_meta",
                    detail: format!("discovery cache load failed: {err}"),
                }),
            );
        }
    };
    let resources = match read_json(&paths.resources) {
        Ok(resources) => resources,
        Err(err) => {
            return (
                None,
                Some(DiscoveryCacheLoadFailure {
                    phase: "cache_load_resources",
                    detail: format!("discovery cache load failed: {err}"),
                }),
            );
        }
    };
    let targets = match read_json(&paths.targets) {
        Ok(targets) => targets,
        Err(err) => {
            return (
                None,
                Some(DiscoveryCacheLoadFailure {
                    phase: "cache_load_targets",
                    detail: format!("discovery cache load failed: {err}"),
                }),
            );
        }
    };
    (
        Some(DiscoverySnapshotContract {
            schema_version: meta.schema_version,
            snapshot_id: meta.snapshot_id,
            revision: meta.revision,
            generated_at: meta.generated_at,
            origins: meta.origins,
            resources,
            targets,
        }),
        None,
    )
}

pub fn load_meta(
    paths: &DiscoveryCachePaths,
) -> (
    Option<DiscoveryCacheMeta>,
    Option<DiscoveryCacheLoadFailure>,
) {
    if !paths.meta.exists() {
        return (None, None);
    }
    match read_json(&paths.meta) {
        Ok(meta) => (Some(meta), None),
        Err(err) => (
            None,
            Some(DiscoveryCacheLoadFailure {
                phase: "cache_load_meta",
                detail: format!("discovery cache load failed: {err}"),
            }),
        ),
    }
}

pub fn store_snapshot(
    paths: &DiscoveryCachePaths,
    snapshot: &DiscoverySnapshotContract,
    last_success_at: Option<&str>,
    last_error: Option<String>,
) -> DiscoveryResult<()> {
    let meta = DiscoveryCacheMeta::new(
        snapshot.snapshot_id.clone(),
        snapshot.revision,
        snapshot.generated_at.clone(),
        snapshot.origins.clone(),
        last_success_at.map(str::to_string),
        last_error,
    );
    write_json_atomic(&paths.resources, &snapshot.resources)
        .source_err(DiscoveryReason::CacheIo, "write discovery resources")?;
    write_json_atomic(&paths.targets, &snapshot.targets)
        .source_err(DiscoveryReason::CacheIo, "write discovery targets")?;
    write_json_atomic(&paths.meta, &meta)
        .source_err(DiscoveryReason::CacheIo, "write discovery meta")
}

pub async fn load_snapshot_async(
    paths: &DiscoveryCachePaths,
) -> (
    Option<DiscoverySnapshotContract>,
    Option<DiscoveryCacheLoadFailure>,
) {
    let (meta_exists, meta_err) = metadata_exists(&paths.meta, "cache_load_meta").await;
    if let Some(failure) = meta_err {
        return (None, Some(failure));
    }
    let (resources_exists, resources_err) =
        metadata_exists(&paths.resources, "cache_load_resources").await;
    if let Some(failure) = resources_err {
        return (None, Some(failure));
    }
    let (targets_exists, targets_err) = metadata_exists(&paths.targets, "cache_load_targets").await;
    if let Some(failure) = targets_err {
        return (None, Some(failure));
    }
    if !meta_exists || !resources_exists || !targets_exists {
        return (None, None);
    }

    let meta: DiscoveryCacheMeta = match read_json_async(&paths.meta).await {
        Ok(meta) => meta,
        Err(err) => {
            return (
                None,
                Some(DiscoveryCacheLoadFailure {
                    phase: "cache_load_meta",
                    detail: format!("discovery cache load failed: {err}"),
                }),
            );
        }
    };
    let resources = match read_json_async(&paths.resources).await {
        Ok(resources) => resources,
        Err(err) => {
            return (
                None,
                Some(DiscoveryCacheLoadFailure {
                    phase: "cache_load_resources",
                    detail: format!("discovery cache load failed: {err}"),
                }),
            );
        }
    };
    let targets = match read_json_async(&paths.targets).await {
        Ok(targets) => targets,
        Err(err) => {
            return (
                None,
                Some(DiscoveryCacheLoadFailure {
                    phase: "cache_load_targets",
                    detail: format!("discovery cache load failed: {err}"),
                }),
            );
        }
    };
    (
        Some(DiscoverySnapshotContract {
            schema_version: meta.schema_version,
            snapshot_id: meta.snapshot_id,
            revision: meta.revision,
            generated_at: meta.generated_at,
            origins: meta.origins,
            resources,
            targets,
        }),
        None,
    )
}

pub async fn load_meta_async(
    paths: &DiscoveryCachePaths,
) -> (
    Option<DiscoveryCacheMeta>,
    Option<DiscoveryCacheLoadFailure>,
) {
    let (meta_exists, meta_err) = metadata_exists(&paths.meta, "cache_load_meta").await;
    if let Some(failure) = meta_err {
        return (None, Some(failure));
    }
    if !meta_exists {
        return (None, None);
    }
    match read_json_async(&paths.meta).await {
        Ok(meta) => (Some(meta), None),
        Err(err) => (
            None,
            Some(DiscoveryCacheLoadFailure {
                phase: "cache_load_meta",
                detail: format!("discovery cache load failed: {err}"),
            }),
        ),
    }
}

pub async fn store_snapshot_async(
    paths: &DiscoveryCachePaths,
    snapshot: &DiscoverySnapshotContract,
    last_success_at: Option<&str>,
    last_error: Option<String>,
) -> DiscoveryResult<()> {
    let meta = DiscoveryCacheMeta::new(
        snapshot.snapshot_id.clone(),
        snapshot.revision,
        snapshot.generated_at.clone(),
        snapshot.origins.clone(),
        last_success_at.map(str::to_string),
        last_error,
    );
    write_json_atomic_async(&paths.resources, &snapshot.resources)
        .await
        .source_err(DiscoveryReason::CacheIo, "write discovery resources")?;
    write_json_atomic_async(&paths.targets, &snapshot.targets)
        .await
        .source_err(DiscoveryReason::CacheIo, "write discovery targets")?;
    write_json_atomic_async(&paths.meta, &meta)
        .await
        .source_err(DiscoveryReason::CacheIo, "write discovery meta")
}

async fn metadata_exists(
    path: &Path,
    phase: &'static str,
) -> (bool, Option<DiscoveryCacheLoadFailure>) {
    match tokio::fs::metadata(path).await {
        Ok(_) => (true, None),
        Err(err) if err.kind() == io::ErrorKind::NotFound => (false, None),
        Err(err) => (
            false,
            Some(DiscoveryCacheLoadFailure {
                phase,
                detail: format!("discovery cache load failed: {err}"),
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use wist_contracts::discovery::{DiscoveryCacheMeta, DiscoverySnapshotContract};

    use super::{DiscoveryCachePaths, load_meta, load_snapshot, store_snapshot};

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("warp-insight-discovery-cache-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn store_and_load_snapshot_round_trip() {
        let state_dir = temp_dir("round-trip");
        let paths = DiscoveryCachePaths::under_state_dir(&state_dir);
        let snapshot = DiscoverySnapshotContract::new(
            "snapshot-1".to_string(),
            1,
            "2026-04-19T00:00:00Z".to_string(),
        );

        store_snapshot(
            &paths,
            &snapshot,
            Some("2026-04-19T00:00:00Z"),
            Some("probe failed".to_string()),
        )
        .expect("store snapshot");
        let (loaded, load_failure) = load_snapshot(&paths);
        let (meta, meta_failure) = load_meta(&paths);

        assert_eq!(loaded, Some(snapshot));
        assert_eq!(load_failure, None);
        assert_eq!(
            meta,
            Some(DiscoveryCacheMeta::new(
                "snapshot-1".to_string(),
                1,
                "2026-04-19T00:00:00Z".to_string(),
                Vec::new(),
                Some("2026-04-19T00:00:00Z".to_string()),
                Some("probe failed".to_string()),
            ))
        );
        assert_eq!(meta_failure, None);
    }

    #[test]
    fn load_snapshot_reports_corrupt_meta_without_failing() {
        let state_dir = temp_dir("corrupt-meta");
        let paths = DiscoveryCachePaths::under_state_dir(&state_dir);
        fs::create_dir_all(&paths.root).expect("create discovery root");
        fs::write(&paths.meta, "{not-json}\n").expect("write meta");
        fs::write(&paths.resources, "[]\n").expect("write resources");
        fs::write(&paths.targets, "[]\n").expect("write targets");

        let (snapshot, failure) = load_snapshot(&paths);

        assert_eq!(snapshot, None);
        assert_eq!(failure.expect("load failure").phase, "cache_load_meta");
    }
}
