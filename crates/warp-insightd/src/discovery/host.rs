//! Host discovery probe skeleton.

use std::fs;

use std::collections::BTreeMap;

use warp_insight_contracts::discovery::{DiscoveredResource, DiscoveredTarget, DiscoveryOrigin};
use warp_insight_shared::time::now_rfc3339;

use super::{DiscoveryProbe, DiscoveryProbeError, DiscoverySourceKind, ProbeOutput};

pub struct HostDiscoveryProbe;

impl DiscoveryProbe for HostDiscoveryProbe {
    fn name(&self) -> &'static str {
        "host"
    }

    fn source(&self) -> DiscoverySourceKind {
        DiscoverySourceKind::LocalRuntime
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(300)
    }

    fn refresh(&self, _now: std::time::SystemTime) -> Result<ProbeOutput, DiscoveryProbeError> {
        let discovered_at = now_rfc3339();
        let host_id = default_host_id();
        let host_name = default_host_name();
        let source = self.source().as_str().to_string();
        let origin_id = format!("{}:{}:{}", source, self.name(), discovered_at);

        Ok(ProbeOutput {
            probe: self.name().to_string(),
            source: self.source(),
            refreshed_at: discovered_at.clone(),
            origin: DiscoveryOrigin {
                origin_id: origin_id.clone(),
                probe: self.name().to_string(),
                source: source.clone(),
                observed_at: discovered_at.clone(),
            },
            resources: vec![DiscoveredResource {
                resource_id: host_id.clone(),
                kind: "host".to_string(),
                origin_idx: 0,
                attributes: BTreeMap::from([
                    ("host.id".to_string(), host_id.clone()),
                    ("host.name".to_string(), host_name.clone()),
                ]),
                discovered_at: discovered_at.clone(),
                last_seen_at: discovered_at.clone(),
                health: "healthy".to_string(),
                source: self.name().to_string(),
            }],
            targets: vec![DiscoveredTarget {
                target_id: format!("{host_id}:host"),
                kind: "host".to_string(),
                origin_idx: 0,
                resource_ref: host_id,
                execution_hints: BTreeMap::from([("host.name".to_string(), host_name.clone())]),
                state: "active".to_string(),
            }],
        })
    }
}

pub(crate) fn default_host_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(hostname_from_file)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "local-host".to_string())
}

pub(crate) fn default_host_id() -> String {
    machine_id_from_known_locations()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("hostname:{}", default_host_name()))
}

#[cfg(unix)]
fn hostname_from_file() -> Option<String> {
    fs::read_to_string("/etc/hostname").ok()
}

#[cfg(not(unix))]
fn hostname_from_file() -> Option<String> {
    None
}

#[cfg(unix)]
fn machine_id_from_known_locations() -> Option<String> {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .into_iter()
        .find_map(|path| fs::read_to_string(path).ok())
}

#[cfg(not(unix))]
fn machine_id_from_known_locations() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    fn default_host_name_from_sources(
        hostname_env: Option<&str>,
        computername_env: Option<&str>,
        hostname_file: Option<&str>,
    ) -> String {
        hostname_env
            .or(computername_env)
            .or(hostname_file)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("local-host")
            .to_string()
    }

    #[test]
    fn host_name_prefers_hostname_env() {
        assert_eq!(
            default_host_name_from_sources(Some("host-a"), Some("pc-a"), Some("file-a")),
            "host-a"
        );
    }

    #[test]
    fn host_name_falls_back_to_hostname_file() {
        assert_eq!(
            default_host_name_from_sources(None, None, Some("file-a")),
            "file-a"
        );
    }

    #[test]
    fn host_name_defaults_when_sources_missing() {
        assert_eq!(
            default_host_name_from_sources(None, None, None),
            "local-host"
        );
    }
}
