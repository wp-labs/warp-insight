//! Kubernetes discovery probe skeleton.

use super::{DiscoveryProbe, DiscoveryProbeError, DiscoverySourceKind, ProbeOutput};

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
pub struct K8sDiscoveryProbe;

impl DiscoveryProbe for K8sDiscoveryProbe {
    fn name(&self) -> &'static str {
        "k8s"
    }

    fn source(&self) -> DiscoverySourceKind {
        DiscoverySourceKind::K8s
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(30)
    }

    fn refresh(&self, _now: std::time::SystemTime) -> Result<ProbeOutput, DiscoveryProbeError> {
        Err(DiscoveryProbeError::new(
            self.name(),
            self.source(),
            "k8s discovery probe is not implemented",
        ))
    }
}
