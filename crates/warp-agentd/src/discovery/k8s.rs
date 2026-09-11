//! Kubernetes discovery probe skeleton.

use orion_error::conversion::ToStructError;

use super::{DiscoveryError, DiscoveryProbe, DiscoveryReason, DiscoverySourceKind, ProbeOutput};

#[derive(::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
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

    fn refresh(&self, _now: std::time::SystemTime) -> Result<ProbeOutput, DiscoveryError> {
        Err(DiscoveryReason::NotImplemented
            .to_err()
            .with_detail("k8s discovery probe is not implemented")
            .with_context(super::probe_error_context(self.name(), self.source())))
    }
}
