//! Discovery runtime skeleton.

pub mod cache;
pub mod container;
pub mod endpoint;
pub mod host;
pub mod k8s;
pub mod network;
pub mod process;
pub mod runtime;

use std::time::SystemTime;

use warp_insight_contracts::discovery::{DiscoveredResource, DiscoveredTarget, DiscoveryOrigin};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverySourceKind {
    LocalRuntime,
    Static,
    File,
    K8s,
}

impl DiscoverySourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalRuntime => "local_runtime",
            Self::Static => "static",
            Self::File => "file",
            Self::K8s => "k8s",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
pub struct ProbeOutput {
    pub probe: String,
    pub source: DiscoverySourceKind,
    pub refreshed_at: String,
    pub origin: DiscoveryOrigin,
    pub resources: Vec<DiscoveredResource>,
    pub targets: Vec<DiscoveredTarget>,
}

pub trait DiscoveryProbe {
    fn name(&self) -> &'static str;
    fn source(&self) -> DiscoverySourceKind;
    fn refresh_interval(&self) -> std::time::Duration;
    fn refresh(&self, now: SystemTime) -> Result<ProbeOutput, DiscoveryProbeError>;
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
pub struct DiscoveryProbeError {
    pub probe: String,
    pub source: DiscoverySourceKind,
    pub detail: String,
}

impl DiscoveryProbeError {
    pub fn new(
        probe: impl Into<String>,
        source: DiscoverySourceKind,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            probe: probe.into(),
            source,
            detail: detail.into(),
        }
    }
}
