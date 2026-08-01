//! Discovery runtime orchestration skeleton.

use std::io;
use std::path::Path;

use warp_insight_contracts::discovery::{DiscoveryCacheMeta, DiscoverySnapshotContract};
use warp_insight_shared::time::now_rfc3339;

use super::DiscoveryProbe;
use super::cache::{
    DiscoveryCacheLoadFailure, DiscoveryCachePaths, load_meta, load_snapshot, store_snapshot,
};

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
pub struct DiscoveryRuntime {
    probes: Vec<Box<dyn DiscoveryProbe + Send + Sync>>,
    latest_snapshot: Option<DiscoverySnapshotContract>,
}

impl DiscoveryRuntime {
    pub fn new(probes: Vec<Box<dyn DiscoveryProbe + Send + Sync>>) -> Self {
        Self {
            probes,
            latest_snapshot: None,
        }
    }

    pub fn probe_count(&self) -> usize {
        self.probes.len()
    }

    pub fn latest_snapshot(&self) -> Option<&DiscoverySnapshotContract> {
        self.latest_snapshot.as_ref()
    }

    pub fn set_latest_snapshot(&mut self, snapshot: DiscoverySnapshotContract) {
        self.latest_snapshot = Some(snapshot);
    }

    pub fn load_from_state_dir(
        &mut self,
        state_dir: &Path,
    ) -> io::Result<(
        Option<DiscoverySnapshotContract>,
        Option<DiscoveryCacheLoadFailure>,
    )> {
        let paths = DiscoveryCachePaths::under_state_dir(state_dir);
        let (snapshot, failure) = load_snapshot(&paths);
        if let Some(snapshot) = snapshot.as_ref() {
            self.latest_snapshot = Some(snapshot.clone());
        }
        Ok((snapshot, failure))
    }

    pub fn load_meta_from_state_dir(
        &self,
        state_dir: &Path,
    ) -> io::Result<(
        Option<DiscoveryCacheMeta>,
        Option<DiscoveryCacheLoadFailure>,
    )> {
        let paths = DiscoveryCachePaths::under_state_dir(state_dir);
        Ok(load_meta(&paths))
    }

    pub fn refresh_and_store(&mut self, state_dir: &Path) -> io::Result<DiscoveryRefreshResult> {
        let mut result = self.refresh_all();
        let paths = DiscoveryCachePaths::under_state_dir(state_dir);
        if let Err(err) = store_snapshot(
            &paths,
            &result.persisted_snapshot,
            result.last_success_at.as_deref(),
            result.last_error.clone(),
        ) {
            result.record_store_error(err);
        }
        Ok(result)
    }

    pub fn refresh_all(&mut self) -> DiscoveryRefreshResult {
        let now = std::time::SystemTime::now();
        let mut resources = Vec::new();
        let mut targets = Vec::new();
        let mut origins = Vec::new();
        let mut errors = Vec::new();
        let mut successful_probes = Vec::new();

        for probe in &self.probes {
            match probe.refresh(now) {
                Ok(mut output) => {
                    let origin_idx = origins.len();
                    for resource in &mut output.resources {
                        resource.origin_idx = origin_idx;
                    }
                    for target in &mut output.targets {
                        target.origin_idx = origin_idx;
                    }
                    successful_probes.push(SuccessfulProbeRefresh {
                        probe: output.probe.clone(),
                        source: output.source,
                        resource_count: output.resources.len(),
                        target_count: output.targets.len(),
                    });
                    origins.push(output.origin);
                    resources.extend(output.resources);
                    targets.extend(output.targets);
                }
                Err(err) => errors.push(err),
            }
        }

        let previous_snapshot = self.latest_snapshot.clone();
        let previous_last_success_at = previous_snapshot
            .as_ref()
            .map(|snapshot| snapshot.generated_at.clone());
        let revision = previous_snapshot
            .as_ref()
            .map_or(1, |snapshot| snapshot.revision + 1);
        let generated_at = now_rfc3339();
        let snapshot_id = format!("discovery:{revision}:{generated_at}");
        let mut refreshed_snapshot =
            DiscoverySnapshotContract::new(snapshot_id, revision, generated_at.clone());
        refreshed_snapshot.origins = origins;
        refreshed_snapshot.resources = resources;
        refreshed_snapshot.targets = targets;

        let has_successful_probe_output = !refreshed_snapshot.resources.is_empty()
            || !refreshed_snapshot.targets.is_empty()
            || errors.len() < self.probes.len();
        let persisted_snapshot = if has_successful_probe_output {
            refreshed_snapshot.clone()
        } else {
            previous_snapshot
                .clone()
                .unwrap_or_else(|| refreshed_snapshot.clone())
        };
        self.latest_snapshot = Some(persisted_snapshot.clone());
        let last_error = errors.first().map(|error| error.detail.clone());

        DiscoveryRefreshResult {
            refreshed_snapshot,
            persisted_snapshot,
            errors,
            last_success_at: if has_successful_probe_output {
                Some(generated_at)
            } else {
                previous_last_success_at
            },
            last_error,
            used_cached_snapshot: !has_successful_probe_output && previous_snapshot.is_some(),
            had_successful_refresh: has_successful_probe_output,
            successful_probes,
            store_failure: None,
        }
    }
}

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
pub struct DiscoveryRefreshResult {
    pub refreshed_snapshot: DiscoverySnapshotContract,
    pub persisted_snapshot: DiscoverySnapshotContract,
    pub errors: Vec<super::DiscoveryProbeError>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub used_cached_snapshot: bool,
    pub had_successful_refresh: bool,
    pub successful_probes: Vec<SuccessfulProbeRefresh>,
    pub store_failure: Option<DiscoveryStoreFailure>,
}

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
pub struct SuccessfulProbeRefresh {
    pub probe: String,
    pub source: super::DiscoverySourceKind,
    pub resource_count: usize,
    pub target_count: usize,
}

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
pub struct DiscoveryStoreFailure {
    pub phase: &'static str,
    pub detail: String,
}

impl DiscoveryRefreshResult {
    fn record_store_error(&mut self, err: io::Error) {
        let detail = format!("discovery cache store failed: {err}");
        self.last_error = Some(detail.clone());
        self.store_failure = Some(DiscoveryStoreFailure {
            phase: "cache_store",
            detail,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::{Duration, SystemTime};
    use std::{fs, path::PathBuf};

    use warp_insight_contracts::discovery::{
        DiscoveredResource, DiscoveryOrigin, DiscoverySnapshotContract,
    };

    use crate::discovery::{DiscoveryProbeError, DiscoverySourceKind, ProbeOutput};

    use super::DiscoveryRuntime;

    struct StubProbe {
        name: &'static str,
        source: DiscoverySourceKind,
        output: Result<ProbeOutput, DiscoveryProbeError>,
    }

    impl crate::discovery::DiscoveryProbe for StubProbe {
        fn name(&self) -> &'static str {
            self.name
        }

        fn source(&self) -> DiscoverySourceKind {
            self.source
        }

        fn refresh_interval(&self) -> Duration {
            Duration::from_secs(1)
        }

        fn refresh(&self, _now: SystemTime) -> Result<ProbeOutput, DiscoveryProbeError> {
            self.output.clone()
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("warp-insight-discovery-runtime-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn refresh_all_collects_probe_outputs_and_advances_revision() {
        let output = ProbeOutput {
            probe: "host".to_string(),
            source: DiscoverySourceKind::LocalRuntime,
            refreshed_at: "2026-04-19T00:00:00Z".to_string(),
            origin: DiscoveryOrigin {
                origin_id: "origin-1".to_string(),
                probe: "host".to_string(),
                source: "local_runtime".to_string(),
                observed_at: "2026-04-19T00:00:00Z".to_string(),
            },
            resources: vec![DiscoveredResource {
                resource_id: "host-1".to_string(),
                kind: "host".to_string(),
                origin_idx: 0,
                attributes: BTreeMap::from([("host.id".to_string(), "host-1".to_string())]),
                discovered_at: "2026-04-19T00:00:00Z".to_string(),
                last_seen_at: "2026-04-19T00:00:00Z".to_string(),
                health: "healthy".to_string(),
                source: "local_runtime".to_string(),
            }],
            targets: Vec::new(),
        };

        let mut runtime = DiscoveryRuntime::new(vec![Box::new(StubProbe {
            name: "host",
            source: DiscoverySourceKind::LocalRuntime,
            output: Ok(output),
        })]);

        let first = runtime.refresh_all();
        assert_eq!(first.refreshed_snapshot.revision, 1);
        assert_eq!(first.persisted_snapshot.revision, 1);
        assert_eq!(first.persisted_snapshot.resources.len(), 1);
        assert!(first.errors.is_empty());
        assert!(!first.used_cached_snapshot);
        assert!(first.had_successful_refresh);

        let second = runtime.refresh_all();
        assert_eq!(second.refreshed_snapshot.revision, 2);
        assert_eq!(second.persisted_snapshot.revision, 2);
    }

    #[test]
    fn refresh_all_keeps_snapshot_when_one_probe_fails() {
        let output = ProbeOutput {
            probe: "host".to_string(),
            source: DiscoverySourceKind::LocalRuntime,
            refreshed_at: "2026-04-19T00:00:00Z".to_string(),
            origin: DiscoveryOrigin {
                origin_id: "origin-1".to_string(),
                probe: "host".to_string(),
                source: "local_runtime".to_string(),
                observed_at: "2026-04-19T00:00:00Z".to_string(),
            },
            resources: vec![DiscoveredResource {
                resource_id: "host-1".to_string(),
                kind: "host".to_string(),
                origin_idx: 0,
                attributes: BTreeMap::from([("host.id".to_string(), "host-1".to_string())]),
                discovered_at: "2026-04-19T00:00:00Z".to_string(),
                last_seen_at: "2026-04-19T00:00:00Z".to_string(),
                health: "healthy".to_string(),
                source: "local_runtime".to_string(),
            }],
            targets: Vec::new(),
        };

        let mut runtime = DiscoveryRuntime::new(vec![
            Box::new(StubProbe {
                name: "host",
                source: DiscoverySourceKind::LocalRuntime,
                output: Ok(output),
            }),
            Box::new(StubProbe {
                name: "k8s",
                source: DiscoverySourceKind::K8s,
                output: Err(DiscoveryProbeError::new(
                    "k8s",
                    DiscoverySourceKind::K8s,
                    "k8s unavailable",
                )),
            }),
        ]);

        let result = runtime.refresh_all();
        assert_eq!(result.persisted_snapshot.resources.len(), 1);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].source, DiscoverySourceKind::K8s);
        assert!(!result.used_cached_snapshot);
        assert!(result.had_successful_refresh);
    }

    #[test]
    fn refresh_all_keeps_last_successful_snapshot_when_all_probes_fail() {
        let mut previous = DiscoverySnapshotContract::new(
            "snapshot-1".to_string(),
            1,
            "2026-04-19T00:00:00Z".to_string(),
        );
        previous.resources = vec![DiscoveredResource {
            resource_id: "host-1".to_string(),
            kind: "host".to_string(),
            origin_idx: 0,
            attributes: BTreeMap::from([("host.id".to_string(), "host-1".to_string())]),
            discovered_at: "2026-04-19T00:00:00Z".to_string(),
            last_seen_at: "2026-04-19T00:00:00Z".to_string(),
            health: "healthy".to_string(),
            source: "local_runtime".to_string(),
        }];

        let mut runtime = DiscoveryRuntime::new(vec![Box::new(StubProbe {
            name: "process",
            source: DiscoverySourceKind::LocalRuntime,
            output: Err(DiscoveryProbeError::new(
                "process",
                DiscoverySourceKind::LocalRuntime,
                "process discovery failed",
            )),
        })]);
        runtime.set_latest_snapshot(previous.clone());

        let result = runtime.refresh_all();

        assert!(result.refreshed_snapshot.resources.is_empty());
        assert_eq!(result.persisted_snapshot, previous);
        assert_eq!(
            result.last_success_at.as_deref(),
            Some("2026-04-19T00:00:00Z")
        );
        assert!(result.used_cached_snapshot);
        assert!(!result.had_successful_refresh);
    }

    #[test]
    fn refresh_and_store_persists_last_error_when_probe_fails_without_success_snapshot() {
        let state_dir = temp_dir("persist-last-error");
        let mut runtime = DiscoveryRuntime::new(vec![Box::new(StubProbe {
            name: "process",
            source: DiscoverySourceKind::LocalRuntime,
            output: Err(DiscoveryProbeError::new(
                "process",
                DiscoverySourceKind::LocalRuntime,
                "process discovery failed",
            )),
        })]);

        let result = runtime
            .refresh_and_store(&state_dir)
            .expect("refresh and store");
        let (meta, meta_failure) = runtime
            .load_meta_from_state_dir(&state_dir)
            .expect("load meta");
        let meta = meta.expect("meta exists");

        assert_eq!(
            result.last_error.as_deref(),
            Some("process discovery failed")
        );
        assert_eq!(meta.last_error.as_deref(), Some("process discovery failed"));
        assert_eq!(meta.last_success_at, None);
        assert_eq!(meta_failure, None);
    }
}
