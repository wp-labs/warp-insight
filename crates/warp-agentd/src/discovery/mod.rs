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

use orion_error::{conversion::ToStructError, runtime::OperationContext};

use wist_contracts::discovery::{DiscoveredResource, DiscoveredTarget, DiscoveryOrigin};

pub use crate::error::{DiscoveryError, DiscoveryReason};

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

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
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
    fn refresh(&self, now: SystemTime) -> Result<ProbeOutput, DiscoveryError>;
}

/// Structured context shared by every probe failure, so observability can
/// recover the originating probe and source from the [`DiscoveryError`].
pub(crate) fn probe_error_context(probe: &str, source: DiscoverySourceKind) -> OperationContext {
    OperationContext::doing("discover")
        .with_meta("probe", probe)
        .with_meta("source", source.as_str())
}

/// Build a [`DiscoveryError`] for a probe whose refresh failed.
pub(crate) fn probe_failed(
    probe: &str,
    source: DiscoverySourceKind,
    detail: impl Into<String>,
) -> DiscoveryError {
    DiscoveryReason::ProbeFailed
        .to_err()
        .with_detail(detail)
        .with_context(probe_error_context(probe, source))
}
