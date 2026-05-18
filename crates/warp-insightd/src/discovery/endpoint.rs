//! Runtime service endpoint discovery probe.

use std::collections::BTreeMap;

#[cfg(target_os = "linux")]
mod endpoint_linux;

use warp_insight_contracts::discovery::{DiscoveredResource, DiscoveredTarget, DiscoveryOrigin};
use warp_insight_shared::time::now_rfc3339;

use super::host::default_host_id;
use super::{DiscoveryProbe, DiscoveryProbeError, DiscoverySourceKind, ProbeOutput};

pub struct EndpointDiscoveryProbe;

impl DiscoveryProbe for EndpointDiscoveryProbe {
    fn name(&self) -> &'static str {
        "endpoint"
    }

    fn source(&self) -> DiscoverySourceKind {
        DiscoverySourceKind::LocalRuntime
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(30)
    }

    fn refresh(&self, _now: std::time::SystemTime) -> Result<ProbeOutput, DiscoveryProbeError> {
        let discovered_at = now_rfc3339();
        let host_id = default_host_id();
        let source = self.source().as_str().to_string();
        let observed_at = discovered_at.clone();
        let origin_id = format!("{}:{}:{}", source, self.name(), observed_at);
        let endpoints = discover_service_endpoints().map_err(|err| {
            DiscoveryProbeError::new(
                self.name(),
                self.source(),
                format!("endpoint discovery failed: {err}"),
            )
        })?;

        let mut resources = Vec::with_capacity(endpoints.len());
        let mut targets = Vec::with_capacity(endpoints.len());
        for endpoint in endpoints {
            let resource_id = endpoint.resource_id(&host_id);
            let attrs = endpoint.attributes(&host_id);

            resources.push(DiscoveredResource {
                resource_id: resource_id.clone(),
                kind: "service_endpoint".to_string(),
                origin_idx: 0,
                attributes: attrs.clone(),
                discovered_at: discovered_at.clone(),
                last_seen_at: discovered_at.clone(),
                health: "healthy".to_string(),
                source: self.name().to_string(),
            });
            targets.push(DiscoveredTarget {
                target_id: format!("{resource_id}:service_endpoint"),
                kind: "service_endpoint".to_string(),
                origin_idx: 0,
                resource_ref: resource_id,
                execution_hints: attrs,
                state: "active".to_string(),
            });
        }

        Ok(ProbeOutput {
            probe: self.name().to_string(),
            source: self.source(),
            refreshed_at: discovered_at,
            origin: DiscoveryOrigin {
                origin_id,
                probe: self.name().to_string(),
                source,
                observed_at,
            },
            resources,
            targets,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ObservedServiceEndpoint {
    pub(super) protocol: String,
    pub(super) local_ip: String,
    pub(super) local_port: u16,
    pub(super) socket_inode: String,
    pub(super) pid: Option<u32>,
    pub(super) process_identity: Option<String>,
    pub(super) process_name: Option<String>,
    pub(super) cgroup_path: Option<String>,
    pub(super) container_id: Option<String>,
    pub(super) owner_count: Option<usize>,
    pub(super) binding_confidence: Option<String>,
}

impl ObservedServiceEndpoint {
    fn resource_id(&self, host_id: &str) -> String {
        format!(
            "{host_id}:endpoint:{}:{}:{}:{}",
            self.protocol,
            encode_id_segment(&self.local_ip),
            self.local_port,
            self.socket_inode
        )
    }

    fn process_ref(&self, host_id: &str) -> Option<String> {
        let pid = self.pid?;
        Some(match &self.process_identity {
            Some(identity) => format!("{host_id}:pid:{pid}:{identity}"),
            None => format!("{host_id}:pid:{pid}"),
        })
    }

    fn attributes(&self, host_id: &str) -> BTreeMap<String, String> {
        let mut attrs = BTreeMap::from([
            ("host.id".to_string(), host_id.to_string()),
            ("endpoint.protocol".to_string(), self.protocol.clone()),
            ("endpoint.bind.ip".to_string(), self.local_ip.clone()),
            (
                "endpoint.bind.port".to_string(),
                self.local_port.to_string(),
            ),
            ("socket.inode".to_string(), self.socket_inode.clone()),
        ]);
        if let Some(pid) = self.pid {
            attrs.insert("process.pid".to_string(), pid.to_string());
            attrs.insert(
                "runtime.binding.evidence".to_string(),
                "socket_inode_owner".to_string(),
            );
        }
        if let Some(process_ref) = self.process_ref(host_id) {
            attrs.insert("process.ref".to_string(), process_ref);
        }
        if let Some(identity) = &self.process_identity {
            attrs.insert("process.identity".to_string(), identity.clone());
        }
        if let Some(process_name) = &self.process_name {
            attrs.insert("process.executable.name".to_string(), process_name.clone());
        }
        if let Some(cgroup_path) = &self.cgroup_path {
            attrs.insert("cgroup.path".to_string(), cgroup_path.clone());
        }
        if let Some(container_id) = &self.container_id {
            attrs.insert("container.id".to_string(), container_id.clone());
            attrs.insert("container.ref".to_string(), container_id.clone());
            attrs.insert(
                "runtime.binding.container.evidence".to_string(),
                "process_cgroup".to_string(),
            );
        }
        if let Some(owner_count) = self.owner_count {
            attrs.insert(
                "runtime.binding.owner.count".to_string(),
                owner_count.to_string(),
            );
        }
        if let Some(confidence) = &self.binding_confidence {
            attrs.insert("runtime.binding.confidence".to_string(), confidence.clone());
        }
        attrs
    }
}

fn encode_id_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value.as_bytes() {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

#[cfg(target_os = "linux")]
fn discover_service_endpoints() -> std::io::Result<Vec<ObservedServiceEndpoint>> {
    endpoint_linux::discover_service_endpoints()
}

#[cfg(not(target_os = "linux"))]
fn discover_service_endpoints() -> std::io::Result<Vec<ObservedServiceEndpoint>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::ObservedServiceEndpoint;

    #[test]
    fn endpoint_resource_id_contains_socket_identity() {
        let endpoint = ObservedServiceEndpoint {
            protocol: "tcp".to_string(),
            local_ip: "127.0.0.1".to_string(),
            local_port: 8080,
            socket_inode: "12345".to_string(),
            pid: Some(42),
            process_identity: Some("linux_proc_start:99".to_string()),
            process_name: Some("demo".to_string()),
            cgroup_path: Some("/kubepods/test".to_string()),
            container_id: Some("container-a".to_string()),
            owner_count: Some(1),
            binding_confidence: Some("single_owner".to_string()),
        };

        assert_eq!(
            endpoint.resource_id("host-1"),
            "host-1:endpoint:tcp:3132372e302e302e31:8080:12345"
        );
        assert_eq!(
            endpoint.process_ref("host-1").as_deref(),
            Some("host-1:pid:42:linux_proc_start:99")
        );
    }

    #[test]
    fn endpoint_resource_id_encodes_ipv6_address() {
        let endpoint = ObservedServiceEndpoint {
            protocol: "tcp".to_string(),
            local_ip: "::1".to_string(),
            local_port: 8080,
            socket_inode: "12345".to_string(),
            pid: None,
            process_identity: None,
            process_name: None,
            cgroup_path: None,
            container_id: None,
            owner_count: None,
            binding_confidence: None,
        };

        assert_eq!(
            endpoint.resource_id("host-1"),
            "host-1:endpoint:tcp:3a3a31:8080:12345"
        );
    }

    #[test]
    fn endpoint_attributes_include_runtime_binding_evidence() {
        let endpoint = ObservedServiceEndpoint {
            protocol: "tcp".to_string(),
            local_ip: "127.0.0.1".to_string(),
            local_port: 8080,
            socket_inode: "12345".to_string(),
            pid: Some(42),
            process_identity: Some("linux_proc_start:99".to_string()),
            process_name: Some("demo".to_string()),
            cgroup_path: Some("/kubepods/test".to_string()),
            container_id: Some("container-a".to_string()),
            owner_count: Some(2),
            binding_confidence: Some("shared_socket".to_string()),
        };

        let attrs = endpoint.attributes("host-1");
        assert_eq!(
            attrs.get("runtime.binding.evidence").map(String::as_str),
            Some("socket_inode_owner")
        );
        assert_eq!(
            attrs
                .get("runtime.binding.container.evidence")
                .map(String::as_str),
            Some("process_cgroup")
        );
        assert_eq!(
            attrs.get("process.ref").map(String::as_str),
            Some("host-1:pid:42:linux_proc_start:99")
        );
        assert_eq!(
            attrs.get("runtime.binding.owner.count").map(String::as_str),
            Some("2")
        );
        assert_eq!(
            attrs.get("runtime.binding.confidence").map(String::as_str),
            Some("shared_socket")
        );
    }
}
