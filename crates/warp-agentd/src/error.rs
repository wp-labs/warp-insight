//! Structured error surface for the edge agent daemon, built on `orion-error`.
//!
//! Each domain owns a small reason enum (derived with [`OrionError`]) that
//! carries only stable, unit-shaped business identities. Dynamic diagnostics
//! (paths, raw backend messages, validation codes, credential schemes, ...)
//! live on the [`StructError`] carrier as detail, context, or source, never
//! inside the reason value itself.
//!
//! The crate-level [`AgentdReason`] lifts the domain reasons at the process
//! boundary (`run` / `run_daemon`). Domain errors are promoted with
//! [`ConvErr::conv_err`], which preserves detail, context, and source chains
//! while only remapping the reason namespace.

use std::fmt;
use std::io;

use orion_error::{
    conversion::{ConvStructError, ToStructError},
    prelude::*,
};

/// Error carrier for config loading and validation.
pub type ConfigError = StructError<ConfigReason>;
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Error carrier for control-plane enrollment.
pub type EnrollmentError = StructError<EnrollmentReason>;
pub type EnrollmentResult<T> = Result<T, EnrollmentError>;

/// Error carrier for the top-level daemon boundary.
pub type AgentdError = StructError<AgentdReason>;
pub type AgentdResult<T> = Result<T, AgentdError>;

/// Error carrier for discovery probes and cache persistence.
pub type DiscoveryError = StructError<DiscoveryReason>;
pub type DiscoveryResult<T> = Result<T, DiscoveryError>;

/// Error carrier for the runtime layer (exec / reporting / scheduler / daemon / telemetry).
///
/// This is a newtype over [`StructError`] rather than a bare type alias so the
/// runtime layer can lift raw `io::Error` into the structured system with a
/// blanket `From` impl (the orphan rule forbids `impl From<io::Error> for
/// StructError<_>`). It keeps `?` propagation on `io::Result` working when a
/// function's return type changes to `RuntimeResult`.
#[derive(Debug, Clone)]
pub struct RuntimeError(StructError<RuntimeReason>);

pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Reason namespace for config loading and validation.
#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum ConfigReason {
    #[orion_error(identity = "conf.warp.agentd.config.io")]
    Io,
    #[orion_error(identity = "conf.warp.agentd.config.parse_toml")]
    ParseToml,
    #[orion_error(identity = "conf.warp.agentd.config.missing_env_var")]
    MissingEnvVar,
    #[orion_error(identity = "conf.warp.agentd.config.validation")]
    Validation,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

/// Reason namespace for control-plane enrollment.
#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum EnrollmentReason {
    #[orion_error(identity = "conf.warp.agentd.enroll.io")]
    Io,
    #[orion_error(identity = "biz.warp.agentd.enroll.missing_endpoint")]
    MissingEndpoint,
    #[orion_error(identity = "biz.warp.agentd.enroll.missing_token")]
    MissingEnrollmentToken,
    #[orion_error(identity = "sys.warp.agentd.enroll.http")]
    Http,
    #[orion_error(identity = "conf.warp.agentd.enroll.invalid_trust_bundle")]
    InvalidTrustBundle,
    #[orion_error(identity = "biz.warp.agentd.enroll.rejected")]
    Rejected,
    #[orion_error(identity = "biz.warp.agentd.enroll.invalid_result")]
    InvalidAcceptedResult,
    #[orion_error(identity = "conf.warp.agentd.enroll.unsupported_scheme")]
    UnsupportedCredentialScheme,
    #[orion_error(identity = "conf.warp.agentd.enroll.invalid_tls_mode")]
    InvalidTlsMode,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

/// Reason namespace for the top-level daemon boundary.
#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum AgentdReason {
    #[orion_error(identity = "biz.warp.agentd.cli.invalid_args")]
    InvalidArgs,
    #[orion_error(identity = "biz.warp.agentd.runtime.identity_conflict")]
    IdentityConflict,
    #[orion_error(identity = "sys.warp.agentd.exec_bin_unavailable")]
    ExecBinUnavailable,
    #[orion_error(identity = "biz.warp.agentd.config")]
    Config,
    #[orion_error(identity = "biz.warp.agentd.enrollment")]
    Enrollment,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

/// Reason namespace for discovery probes and cache persistence.
#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum DiscoveryReason {
    #[orion_error(identity = "sys.warp.agentd.discovery.probe_failed")]
    ProbeFailed,
    #[orion_error(identity = "sys.warp.agentd.discovery.not_implemented")]
    NotImplemented,
    #[orion_error(identity = "sys.warp.agentd.discovery.cache_io")]
    CacheIo,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

/// Reason namespace for the runtime layer.
///
/// The local `state_store` still surfaces raw `io::Error` (documented as
/// leftover); those are converted here with `source_err(RuntimeReason::Io, ...)`
/// at the runtime-layer boundary until the store adopts orion-error itself.
#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum RuntimeReason {
    #[orion_error(identity = "sys.warp.agentd.runtime.io")]
    Io,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

impl From<RuntimeReason> for AgentdReason {
    fn from(value: RuntimeReason) -> Self {
        match value {
            RuntimeReason::General(reason) => AgentdReason::General(reason),
            RuntimeReason::Io => AgentdReason::system_error(),
        }
    }
}

impl RuntimeError {
    /// Build a runtime error from a reason and detail, without a source.
    pub fn reason(reason: RuntimeReason, detail: impl Into<String>) -> Self {
        Self(reason.to_err().with_detail(detail))
    }

    pub fn into_inner(self) -> StructError<RuntimeReason> {
        self.0
    }

    /// Best-effort `io::ErrorKind` of the underlying source, so callers that
    /// previously inspected `io::Error::kind()` can keep doing so during the
    /// migration.
    pub fn kind(&self) -> io::ErrorKind {
        self.0
            .source_ref()
            .and_then(|src| src.downcast_ref::<io::Error>())
            .map(io::Error::kind)
            .unwrap_or(io::ErrorKind::Other)
    }
}

impl From<io::Error> for RuntimeError {
    fn from(err: io::Error) -> Self {
        let detail = err.to_string();
        Self(
            StructError::builder(RuntimeReason::Io)
                .detail(detail)
                .source_std(err)
                .finish(),
        )
    }
}

impl From<RuntimeReason> for RuntimeError {
    fn from(reason: RuntimeReason) -> Self {
        Self(reason.to_err())
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source_ref()
    }
}

impl From<RuntimeError> for AgentdError {
    fn from(err: RuntimeError) -> Self {
        err.into_inner().conv()
    }
}

impl From<ConfigReason> for AgentdReason {
    fn from(value: ConfigReason) -> Self {
        match value {
            ConfigReason::General(reason) => AgentdReason::General(reason),
            _ => AgentdReason::Config,
        }
    }
}

impl From<EnrollmentReason> for AgentdReason {
    fn from(value: EnrollmentReason) -> Self {
        match value {
            EnrollmentReason::General(reason) => AgentdReason::General(reason),
            _ => AgentdReason::Enrollment,
        }
    }
}
