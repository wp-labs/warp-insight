//! Startup enrollment client for managed agents.

use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use warp_insight_contracts::agent_config::AgentConfigContract;
use warp_insight_contracts::enrollment::{
    AgentCredentialRenewed, AgentEnrollmentResult, AgentEnrollmentResultReturned,
    AgentEnrollmentResultStatus, AgentHostProfile, RenewAgentCredential, SubmitEnrollmentRequest,
};
use warp_insight_contracts::state_exec::{AgentRuntimeState, RuntimeMode};
use warp_insight_shared::fs::write_bytes_private_atomic;
use warp_insight_shared::time::now_rfc3339;

use crate::state_store;

const ENROLLMENT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const ENROLLMENT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const ENROLLMENT_MAX_ATTEMPTS: u32 = 3;
/// Renew proactively once the credential is inside this window of its expiry.
const CREDENTIAL_RENEWAL_WINDOW: time::Duration = time::Duration::days(7);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentDecision {
    Disabled,
    ExistingConfigIdentity,
    ExistingStateIdentity,
    Enrolled,
}

#[derive(Debug)]
pub enum EnrollmentError {
    Io(io::Error),
    MissingEndpoint,
    MissingEnrollmentToken,
    Http(reqwest::Error),
    InvalidTrustBundle(String),
    Rejected {
        status: AgentEnrollmentResultStatus,
        reason_code: Option<String>,
    },
    InvalidAcceptedResult(&'static str),
    UnsupportedCredentialScheme(String),
    InvalidTlsMode(String),
}

impl fmt::Display for EnrollmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "enrollment io error: {err}"),
            Self::MissingEndpoint => write!(f, "control_plane.endpoint is required for enrollment"),
            Self::MissingEnrollmentToken => {
                write!(
                    f,
                    "control_plane.enrollment_token is required for enrollment"
                )
            }
            Self::Http(err) => write!(f, "enrollment http error: {}", error_chain(err)),
            Self::InvalidTrustBundle(reason) => {
                write!(f, "invalid control_plane.trust_bundle: {reason}")
            }
            Self::Rejected {
                status,
                reason_code,
            } => write!(
                f,
                "enrollment rejected with status {:?} reason {}",
                status,
                reason_code.as_deref().unwrap_or("unknown")
            ),
            Self::InvalidAcceptedResult(field) => {
                write!(f, "accepted enrollment response is missing {field}")
            }
            Self::UnsupportedCredentialScheme(scheme) => {
                write!(f, "unsupported credential auth_scheme: {scheme}")
            }
            Self::InvalidTlsMode(mode) => {
                write!(f, "invalid control_plane.tls_mode: {mode}")
            }
        }
    }
}

fn error_chain(err: &(dyn StdError + 'static)) -> String {
    let mut message = err.to_string();
    let mut source = err.source();
    while let Some(err) = source {
        message.push_str(": ");
        message.push_str(&err.to_string());
        source = err.source();
    }
    message
}

impl std::error::Error for EnrollmentError {}

impl From<io::Error> for EnrollmentError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<reqwest::Error> for EnrollmentError {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value)
    }
}

pub async fn ensure_enrolled(
    config: &mut AgentConfigContract,
    state_dir: &Path,
) -> Result<EnrollmentDecision, EnrollmentError> {
    ensure_enrolled_with_optional_config_path(config, state_dir, None).await
}

pub async fn ensure_enrolled_with_config_path(
    config: &mut AgentConfigContract,
    state_dir: &Path,
    config_path: &Path,
) -> Result<EnrollmentDecision, EnrollmentError> {
    ensure_enrolled_with_optional_config_path(config, state_dir, Some(config_path)).await
}

async fn ensure_enrolled_with_optional_config_path(
    config: &mut AgentConfigContract,
    state_dir: &Path,
    config_path: Option<&Path>,
) -> Result<EnrollmentDecision, EnrollmentError> {
    if has_config_identity(config) {
        if let Some(config_path) = config_path {
            scrub_enrollment_token_from_config_file(config_path)?;
        }
        return Ok(EnrollmentDecision::ExistingConfigIdentity);
    }
    if load_state_identity(config, state_dir)? {
        renew_state_credential_if_needed(config, state_dir).await;
        if let Some(config_path) = config_path {
            scrub_enrollment_token_from_config_file(config_path)?;
        }
        return Ok(EnrollmentDecision::ExistingStateIdentity);
    }
    if !config.control_plane.enabled {
        return Ok(EnrollmentDecision::Disabled);
    }

    let endpoint = required_option(config.control_plane.endpoint.as_deref())
        .ok_or(EnrollmentError::MissingEndpoint)?
        .to_string();
    let token = required_option(config.control_plane.enrollment_token.as_deref())
        .ok_or(EnrollmentError::MissingEnrollmentToken)?
        .to_string();
    let request = build_enrollment_request(config, token);
    let returned = post_enrollment(config, &endpoint, &request).await?;
    apply_enrollment_result(config, state_dir, returned.result)?;
    if let Some(config_path) = config_path {
        scrub_enrollment_token_from_config_file(config_path)?;
    }
    Ok(EnrollmentDecision::Enrolled)
}

fn has_config_identity(config: &AgentConfigContract) -> bool {
    required_option(config.agent.agent_id.as_deref()).is_some()
}

fn load_state_identity(
    config: &mut AgentConfigContract,
    state_dir: &Path,
) -> Result<bool, EnrollmentError> {
    let runtime_path = state_store::agent_runtime::path_for(state_dir);
    if !runtime_path.exists() {
        return Ok(false);
    }
    let runtime_state = state_store::agent_runtime::load_or_default(&runtime_path)?;
    if !is_registered_agent_id(&runtime_state.agent_id) {
        return Ok(false);
    }

    config.agent.agent_id = Some(runtime_state.agent_id.clone());
    config.agent.instance_name = Some(runtime_state.instance_id.clone());
    if let Some(credential_id) = runtime_state
        .credential_id
        .filter(|value| !value.trim().is_empty())
    {
        config.control_plane.credential_id = Some(credential_id);
    }
    if let Some(bearer_token) = runtime_state
        .bearer_token
        .filter(|value| !value.trim().is_empty())
    {
        config.control_plane.bearer_token = Some(bearer_token);
        config.control_plane.auth_mode = Some("bearer".to_string());
        config.control_plane.enrollment_token = None;
    }
    if let Some(expires_at) = runtime_state
        .credential_expires_at
        .filter(|value| !value.trim().is_empty())
    {
        config.control_plane.credential_expires_at = Some(expires_at);
    }
    Ok(true)
}

fn build_enrollment_request(
    config: &AgentConfigContract,
    token: String,
) -> SubmitEnrollmentRequest {
    SubmitEnrollmentRequest::new(
        token,
        config
            .control_plane
            .credential_request
            .clone()
            .unwrap_or_else(|| "none".to_string()),
        build_host_profile(config),
        "warp-insightd:discovery,telemetry,local-exec".to_string(),
        now_rfc3339(),
    )
}

fn build_host_profile(config: &AgentConfigContract) -> AgentHostProfile {
    let hostname = hostname_from_sources(
        std::env::var("HOSTNAME").ok().as_deref(),
        std::env::var("COMPUTERNAME").ok().as_deref(),
        hostname_from_file().as_deref(),
    );
    let machine_id = machine_id_from_file().unwrap_or_else(|| "unknown".to_string());
    let node_id = first_non_empty([
        config.agent.instance_name.as_deref(),
        Some(machine_id.as_str()),
        Some(hostname.as_str()),
    ])
    .unwrap_or("local-node")
    .to_string();

    AgentHostProfile {
        node_id,
        hostname,
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        machine_id,
        cloud_instance_id: std::env::var("WARP_INSIGHT_CLOUD_INSTANCE_ID").ok(),
        k8s_node_uid: std::env::var("WARP_INSIGHT_K8S_NODE_UID").ok(),
        ip_addresses: Vec::new(),
    }
}

async fn post_enrollment(
    config: &AgentConfigContract,
    endpoint: &str,
    request: &SubmitEnrollmentRequest,
) -> Result<AgentEnrollmentResultReturned, EnrollmentError> {
    let url = format!("{}/api/v1/agent/enroll", endpoint.trim_end_matches('/'));
    let client = enrollment_http_client(config)?;
    let response = send_with_retry(&client, |client| client.post(&url).json(request)).await?;
    let response = response.error_for_status().map_err(EnrollmentError::Http)?;
    Ok(response
        .json::<AgentEnrollmentResultReturned>()
        .await
        .map_err(EnrollmentError::Http)?)
}

/// Send the request with bounded retries on transport errors (connect refused,
/// DNS, timeout). HTTP status outcomes (4xx/5xx, rejected) are returned to the
/// caller without retry so single-use bootstrap tokens are not double-spent.
async fn send_with_retry(
    client: &reqwest::Client,
    build: impl Fn(&reqwest::Client) -> reqwest::RequestBuilder,
) -> Result<reqwest::Response, EnrollmentError> {
    for attempt in 0..ENROLLMENT_MAX_ATTEMPTS {
        match build(client).send().await {
            Ok(response) => return Ok(response),
            Err(_) if attempt + 1 < ENROLLMENT_MAX_ATTEMPTS => {
                tokio::time::sleep(retry_backoff(attempt)).await;
            }
            Err(err) => return Err(EnrollmentError::Http(err)),
        }
    }
    unreachable!("send_with_retry loop always returns")
}

fn retry_backoff(attempt: u32) -> Duration {
    Duration::from_millis(200 * 2u64.pow(attempt))
}

/// Best-effort credential rotation when the restored credential is expired or
/// within [`CREDENTIAL_RENEWAL_WINDOW`] of expiry. Failures are logged and the
/// existing credential is kept so the daemon still starts.
async fn renew_state_credential_if_needed(
    config: &mut AgentConfigContract,
    state_dir: &Path,
) {
    let Some(expires_at) = config.control_plane.credential_expires_at.as_deref() else {
        return;
    };
    let Ok(expires_at) = time::OffsetDateTime::parse(
        expires_at,
        &time::format_description::well_known::Rfc3339,
    ) else {
        return;
    };
    if time::OffsetDateTime::now_utc() + CREDENTIAL_RENEWAL_WINDOW < expires_at {
        return;
    }
    if let Err(err) = renew_credential(config, state_dir).await {
        eprintln!(
            "warp-insightd credential renewal failed (continuing with existing credential): {err}"
        );
    }
}

async fn renew_credential(
    config: &mut AgentConfigContract,
    state_dir: &Path,
) -> Result<(), EnrollmentError> {
    let Some(endpoint) = required_option(config.control_plane.endpoint.as_deref()) else {
        return Err(EnrollmentError::MissingEndpoint);
    };
    let Some(bearer_token) = required_option(config.control_plane.bearer_token.as_deref()) else {
        return Ok(());
    };
    let Some(agent_id) = required_option(config.agent.agent_id.as_deref()) else {
        return Ok(());
    };
    let instance_id = required_option(config.agent.instance_name.as_deref())
        .unwrap_or_default()
        .to_string();
    let request = RenewAgentCredential::new(
        agent_id.to_string(),
        instance_id,
        config
            .control_plane
            .credential_request
            .clone()
            .unwrap_or_else(|| "bearer".to_string()),
        now_rfc3339(),
    );
    let client = enrollment_http_client(config)?;
    let url = format!(
        "{}/api/v1/agent/credentials:renew",
        endpoint.trim_end_matches('/')
    );
    let response = send_with_retry(&client, |client| {
        client
            .post(&url)
            .bearer_auth(bearer_token)
            .json(&request)
    })
    .await?;
    let response = response.error_for_status().map_err(EnrollmentError::Http)?;
    let renewed: AgentCredentialRenewed =
        response.json().await.map_err(EnrollmentError::Http)?;
    let credential = renewed.credential_bundle;

    apply_credential_to_config(config, &credential)?;
    let runtime_path = state_store::agent_runtime::path_for(state_dir);
    let mut runtime_state = state_store::agent_runtime::load_or_default(&runtime_path)?;
    apply_credential_to_runtime_state(&mut runtime_state, credential);
    state_store::agent_runtime::store(&runtime_path, &runtime_state)?;
    Ok(())
}

fn enrollment_http_client(
    config: &AgentConfigContract,
) -> Result<reqwest::Client, EnrollmentError> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(ENROLLMENT_CONNECT_TIMEOUT)
        .timeout(ENROLLMENT_REQUEST_TIMEOUT);
    let endpoint = config.control_plane.endpoint.as_deref().unwrap_or_default();
    let effective_mode = match config.control_plane.tls_mode.as_deref() {
        Some(mode) => mode,
        None if endpoint.starts_with("https://") => "https",
        None => "http",
    };
    let mut loaded_trust_bundle = false;
    match effective_mode {
        // Verify the control-plane certificate; use trust_bundle when provided,
        // otherwise fall back to the platform root store.
        "https" | "verify" => {
            if let Some(trust_bundle) = required_option(config.control_plane.trust_bundle.as_deref())
            {
                let certificate = reqwest::Certificate::from_pem(trust_bundle.as_bytes())
                    .map_err(|err| EnrollmentError::InvalidTrustBundle(err.to_string()))?;
                builder = builder.add_root_certificate(certificate);
                loaded_trust_bundle = true;
            }
        }
        // Explicitly disable TLS certificate verification (lab / self-signed only).
        "none" => {
            builder = builder.danger_accept_invalid_certs(true);
        }
        // Plain HTTP, no TLS.
        "http" => {}
        other => {
            return Err(EnrollmentError::InvalidTlsMode(other.to_string()));
        }
    }
    builder.build().map_err(|err| {
        if loaded_trust_bundle {
            EnrollmentError::InvalidTrustBundle(err.to_string())
        } else {
            EnrollmentError::Http(err)
        }
    })
}

fn apply_enrollment_result(
    config: &mut AgentConfigContract,
    state_dir: &Path,
    result: AgentEnrollmentResult,
) -> Result<(), EnrollmentError> {
    if result.status != AgentEnrollmentResultStatus::Accepted {
        return Err(EnrollmentError::Rejected {
            status: result.status,
            reason_code: result.reason_code,
        });
    }

    let agent_id = result
        .agent_id
        .or_else(|| {
            result
                .issued_identity
                .as_ref()
                .map(|identity| identity.agent_id.clone())
        })
        .filter(|value| !value.trim().is_empty())
        .ok_or(EnrollmentError::InvalidAcceptedResult("agent_id"))?;
    let instance_id = result
        .instance_id
        .or_else(|| {
            result
                .issued_identity
                .as_ref()
                .map(|identity| identity.instance_id.clone())
        })
        .filter(|value| !value.trim().is_empty())
        .ok_or(EnrollmentError::InvalidAcceptedResult("instance_id"))?;

    if let Some(identity) = result.issued_identity.as_ref() {
        config.agent.environment_id = Some(identity.environment_id.clone());
    }
    let issued_credential = result.credential_bundle.clone();
    if let Some(credential) = issued_credential.as_ref() {
        apply_credential_to_config(config, credential)?;
    }
    config.agent.agent_id = Some(agent_id.clone());
    config.agent.instance_name = Some(instance_id.clone());

    let runtime_path = state_store::agent_runtime::path_for(state_dir);
    let mut runtime_state = AgentRuntimeState::new(
        agent_id,
        instance_id,
        env!("CARGO_PKG_VERSION").to_string(),
        RuntimeMode::Normal,
        now_rfc3339(),
    );
    if let Some(credential) = issued_credential {
        apply_credential_to_runtime_state(&mut runtime_state, credential);
    }
    state_store::agent_runtime::store(&runtime_path, &runtime_state)?;
    Ok(())
}

/// Apply an issued credential bundle to the in-memory agent config. Both the
/// enrollment and renewal paths share this so the auth_scheme handling stays
/// consistent.
fn apply_credential_to_config(
    config: &mut AgentConfigContract,
    credential: &warp_insight_contracts::enrollment::AgentCredentialBundle,
) -> Result<(), EnrollmentError> {
    config.control_plane.credential_id = Some(credential.credential_id.clone());
    match credential.auth_scheme.as_deref() {
        Some("bearer") | None => {
            let token = credential
                .bearer_token
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let Some(token) = token else {
                return Err(EnrollmentError::InvalidAcceptedResult(
                    "credential bearer_token",
                ));
            };
            config.control_plane.bearer_token = Some(token.to_string());
            config.control_plane.auth_mode = Some("bearer".to_string());
            config.control_plane.enrollment_token = None;
        }
        Some(scheme) => {
            return Err(EnrollmentError::UnsupportedCredentialScheme(
                scheme.to_string(),
            ));
        }
    }
    if let Some(expires_at) = credential.not_after.as_ref() {
        config.control_plane.credential_expires_at = Some(expires_at.clone());
    }
    Ok(())
}

fn apply_credential_to_runtime_state(
    runtime_state: &mut AgentRuntimeState,
    credential: warp_insight_contracts::enrollment::AgentCredentialBundle,
) {
    runtime_state.credential_id = Some(credential.credential_id);
    match credential.auth_scheme.as_deref() {
        Some("bearer") | None => {
            runtime_state.bearer_token = credential.bearer_token;
            runtime_state.credential_expires_at = credential.not_after;
        }
        Some(_) => {
            // Unsupported scheme: apply_credential_to_config rejected this earlier.
        }
    }
}

pub(crate) fn is_registered_agent_id(value: &str) -> bool {
    let normalized = value.trim();
    !normalized.is_empty()
        && !matches!(
            normalized,
            "local-agent" | "unregistered-agent" | "unknown" | "unknown-agent"
        )
}

fn required_option(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> Option<&'a str> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

fn hostname_from_sources(
    hostname_env: Option<&str>,
    computername_env: Option<&str>,
    hostname_file: Option<&str>,
) -> String {
    first_non_empty([hostname_env, computername_env, hostname_file])
        .unwrap_or("local-host")
        .to_string()
}

#[cfg(unix)]
fn hostname_from_file() -> Option<String> {
    fs::read_to_string("/etc/hostname")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(unix))]
fn hostname_from_file() -> Option<String> {
    None
}

#[cfg(unix)]
fn machine_id_from_file() -> Option<String> {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .into_iter()
        .find_map(|path| fs::read_to_string(path).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(unix))]
fn machine_id_from_file() -> Option<String> {
    None
}

fn scrub_enrollment_token_from_config_file(path: &Path) -> Result<(), EnrollmentError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let scrubbed: Vec<&str> = text
        .lines()
        .filter(|line| !is_enrollment_scoped_line(line))
        .collect();
    if scrubbed.len() == text.lines().count() {
        return Ok(());
    }
    write_bytes_private_atomic(path, scrubbed.join("\n").as_bytes())?;
    Ok(())
}

/// True for lines that must be removed once enrollment completes: the bootstrap
/// `enrollment_token` itself and a stale `auth_mode = "enrollment_token"` that the
/// runtime overrides with `bearer` on every startup.
fn is_enrollment_scoped_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("enrollment_token")
        || (trimmed.starts_with("auth_mode") && trimmed.contains("enrollment_token"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use warp_insight_contracts::agent_config::{
        AgentConfigContract, AgentSection, ControlPlaneSection, ExecutionSection, PathsSection,
    };
    use warp_insight_contracts::enrollment::{
        AgentCredentialBundle, AgentEnrollmentResult, AgentEnrollmentResultStatus, AgentIdentity,
        AgentIdentityStatus,
    };

    use super::{
        EnrollmentDecision, EnrollmentError, build_enrollment_request, enrollment_http_client,
        ensure_enrolled, ensure_enrolled_with_config_path, hostname_from_sources, post_enrollment,
        renew_credential,
    };

    fn config() -> AgentConfigContract {
        AgentConfigContract::new(
            AgentSection {
                agent_id: None,
                environment_id: None,
                instance_name: Some("host-a".to_string()),
            },
            ControlPlaneSection {
                enabled: true,
                endpoint: Some("http://127.0.0.1:1".to_string()),
                enrollment_token: Some("token-a".to_string()),
                credential_request: None,
                credential_id: None,
                bearer_token: None,
                credential_expires_at: None,
                tls_mode: None,
                trust_bundle: None,
                auth_mode: None,
            },
            PathsSection {
                root_dir: ".".to_string(),
                run_dir: "run".to_string(),
                state_dir: "state".to_string(),
                log_dir: "log".to_string(),
            },
            ExecutionSection {
                max_running_actions: 1,
                cancel_grace_ms: 5_000,
                default_stdout_limit_bytes: 1,
                default_stderr_limit_bytes: 1,
            },
        )
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("warp-insightd-enrollment-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn build_enrollment_request_uses_configured_token_and_instance_name() {
        let config = config();

        let request = build_enrollment_request(&config, "token-a".to_string());

        assert_eq!(request.token, "token-a");
        assert_eq!(request.host_profile.node_id, "host-a");
        assert_eq!(request.credential_request, "none");
    }

    #[test]
    fn hostname_from_sources_prefers_env_values() {
        assert_eq!(
            hostname_from_sources(Some("host-env"), Some("pc-env"), Some("file-host")),
            "host-env"
        );
        assert_eq!(
            hostname_from_sources(None, None, Some("file-host")),
            "file-host"
        );
    }

    #[test]
    fn https_enrollment_client_rejects_invalid_trust_bundle() {
        let mut config = config();
        config.control_plane.endpoint = Some("https://control.example".to_string());
        config.control_plane.trust_bundle = Some(
            "-----BEGIN CERTIFICATE-----\nnot-base64\n-----END CERTIFICATE-----\n".to_string(),
        );

        let err = enrollment_http_client(&config).expect_err("invalid trust bundle");

        assert!(matches!(err, EnrollmentError::InvalidTrustBundle(_)));
    }

    #[test]
    fn http_enrollment_client_ignores_invalid_trust_bundle() {
        let mut config = config();
        config.control_plane.endpoint = Some("http://127.0.0.1:3000".to_string());
        config.control_plane.trust_bundle = Some("not a pem certificate".to_string());

        enrollment_http_client(&config).expect("http client");
    }

    #[test]
    fn tls_mode_none_disables_verification_even_for_https_endpoint() {
        let mut config = config();
        config.control_plane.endpoint = Some("https://control.example".to_string());
        config.control_plane.tls_mode = Some("none".to_string());
        config.control_plane.trust_bundle = Some("not a valid certificate".to_string());

        enrollment_http_client(&config).expect("tls_mode none client");
    }

    #[test]
    fn tls_mode_verify_still_requires_a_valid_trust_bundle() {
        let mut config = config();
        config.control_plane.endpoint = Some("https://control.example".to_string());
        config.control_plane.tls_mode = Some("verify".to_string());
        config.control_plane.trust_bundle = Some(
            "-----BEGIN CERTIFICATE-----\nnot-base64\n-----END CERTIFICATE-----\n".to_string(),
        );

        let err = enrollment_http_client(&config).expect_err("invalid trust bundle");

        assert!(matches!(err, EnrollmentError::InvalidTrustBundle(_)));
    }

    #[test]
    fn tls_mode_invalid_value_is_rejected() {
        let mut config = config();
        config.control_plane.tls_mode = Some("mutual".to_string());

        let err = enrollment_http_client(&config).expect_err("unsupported mode");

        assert!(matches!(err, EnrollmentError::InvalidTlsMode(_)));
    }

    #[tokio::test]
    async fn post_enrollment_to_unreachable_endpoint_returns_error() {
        let mut config = config();
        config.control_plane.endpoint = Some("http://127.0.0.1:1".to_string());
        let request = build_enrollment_request(&config, "token-a".to_string());

        let err = post_enrollment(&config, "http://127.0.0.1:1", &request)
            .await
            .expect_err("unreachable endpoint");

        assert!(matches!(err, EnrollmentError::Http(_)));
    }

    #[tokio::test]
    async fn renew_credential_rotates_bearer_and_updates_state() {
        let state_dir = temp_dir("renew-credential");
        let runtime_path = crate::state_store::agent_runtime::path_for(&state_dir);
        let mut runtime = warp_insight_contracts::state_exec::AgentRuntimeState::new(
            "agent-x".to_string(),
            "instance-x".to_string(),
            "0.1.0".to_string(),
            warp_insight_contracts::state_exec::RuntimeMode::Normal,
            "2026-07-01T00:00:00Z".to_string(),
        );
        runtime.credential_id = Some("cred-old".to_string());
        runtime.bearer_token = Some("wic_old_token".to_string());
        runtime.credential_expires_at = Some("2026-08-01T00:00:00Z".to_string());
        crate::state_store::agent_runtime::store(&runtime_path, &runtime).expect("store state");

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let endpoint = format!("http://{}", listener.local_addr().expect("addr"));
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut request_bytes = Vec::new();
            loop {
                let mut chunk = [0u8; 1024];
                let read = socket.read(&mut chunk).await.expect("read");
                if read == 0 {
                    break;
                }
                request_bytes.extend_from_slice(&chunk[..read]);
                if request_is_complete(&request_bytes) {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request_bytes);
            assert!(request.contains("credentials:renew"));
            assert!(request
                .to_lowercase()
                .contains("authorization: bearer wic_old_token"));
            let body = r#"{"credential_bundle":{"credential_id":"cred-new","agent_id":"agent-x","instance_id":"instance-x","auth_scheme":"bearer","bearer_token":"wic_new_token","certificate":null,"private_key_ref":null,"ca_bundle":null,"issued_at":"2026-08-01T00:00:00Z","not_before":"2026-08-01T00:00:00Z","not_after":"2026-09-01T00:00:00Z"}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        });
        let mut config = config();
        config.agent.agent_id = Some("agent-x".to_string());
        config.control_plane.endpoint = Some(endpoint);
        config.control_plane.bearer_token = Some("wic_old_token".to_string());
        config.control_plane.credential_id = Some("cred-old".to_string());
        config.control_plane.credential_expires_at = Some("2026-08-01T00:00:00Z".to_string());

        renew_credential(&mut config, &state_dir).await.expect("renew");
        server.await.expect("server task");

        assert_eq!(
            config.control_plane.bearer_token.as_deref(),
            Some("wic_new_token")
        );
        assert_eq!(
            config.control_plane.credential_id.as_deref(),
            Some("cred-new")
        );
        let runtime =
            crate::state_store::agent_runtime::load_or_default(&runtime_path).expect("load runtime");
        assert_eq!(runtime.bearer_token.as_deref(), Some("wic_new_token"));
        assert_eq!(runtime.credential_id.as_deref(), Some("cred-new"));
    }

    #[tokio::test]
    async fn ensure_enrolled_uses_existing_state_identity() {
        let state_dir = temp_dir("existing-state");
        let runtime_path = crate::state_store::agent_runtime::path_for(&state_dir);
        crate::state_store::agent_runtime::store(
            &runtime_path,
            &warp_insight_contracts::state_exec::AgentRuntimeState::new(
                "agent-state".to_string(),
                "instance-state".to_string(),
                "0.1.0".to_string(),
                warp_insight_contracts::state_exec::RuntimeMode::Normal,
                "2026-07-27T00:00:00Z".to_string(),
            ),
        )
        .expect("store state");
        let mut config = config();

        let decision = ensure_enrolled(&mut config, &state_dir)
            .await
            .expect("ensure");

        assert_eq!(decision, EnrollmentDecision::ExistingStateIdentity);
        assert_eq!(config.agent.agent_id.as_deref(), Some("agent-state"));
    }

    #[tokio::test]
    async fn ensure_enrolled_restores_existing_state_credential() {
        let state_dir = temp_dir("existing-state-credential");
        let runtime_path = crate::state_store::agent_runtime::path_for(&state_dir);
        let mut runtime = warp_insight_contracts::state_exec::AgentRuntimeState::new(
            "agent-state".to_string(),
            "instance-state".to_string(),
            "0.1.0".to_string(),
            warp_insight_contracts::state_exec::RuntimeMode::Normal,
            "2026-07-27T00:00:00Z".to_string(),
        );
        runtime.credential_id = Some("cred-state".to_string());
        runtime.bearer_token = Some("bearer-state".to_string());
        runtime.credential_expires_at = Some("2026-08-27T00:00:00Z".to_string());
        crate::state_store::agent_runtime::store(&runtime_path, &runtime).expect("store state");
        let mut config = config();

        let decision = ensure_enrolled(&mut config, &state_dir)
            .await
            .expect("ensure");

        assert_eq!(decision, EnrollmentDecision::ExistingStateIdentity);
        assert_eq!(
            config.control_plane.credential_id.as_deref(),
            Some("cred-state")
        );
        assert_eq!(
            config.control_plane.bearer_token.as_deref(),
            Some("bearer-state")
        );
        assert_eq!(config.control_plane.auth_mode.as_deref(), Some("bearer"));
        assert_eq!(
            config.control_plane.credential_expires_at.as_deref(),
            Some("2026-08-27T00:00:00Z")
        );
        assert!(config.control_plane.enrollment_token.is_none());
    }

    #[tokio::test]
    async fn ensure_enrolled_requires_token_without_existing_identity() {
        let state_dir = temp_dir("missing-token");
        let mut config = config();
        config.control_plane.enrollment_token = None;

        let err = ensure_enrolled(&mut config, &state_dir)
            .await
            .expect_err("missing token");

        assert!(matches!(err, EnrollmentError::MissingEnrollmentToken));
    }

    #[tokio::test]
    async fn ensure_enrolled_posts_request_and_persists_identity() {
        let state_dir = temp_dir("http-register");
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let endpoint = format!("http://{}", listener.local_addr().expect("addr"));
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut request_bytes = Vec::new();
            loop {
                let mut chunk = [0u8; 1024];
                let read = socket.read(&mut chunk).await.expect("read");
                if read == 0 {
                    break;
                }
                request_bytes.extend_from_slice(&chunk[..read]);
                if request_is_complete(&request_bytes) {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request_bytes);
            assert!(request.starts_with("POST /api/v1/agent/enroll "));
            assert!(request.contains("\"token\":\"token-a\""));
            let body = r#"{"result":{"status":"accepted","reason_code":null,"agent_id":"agent-http","instance_id":"instance-http","issued_identity":null,"credential_bundle":null,"initial_config":null,"policy_binding":null}}"#;
            let response = format!(
                "HTTP/1.1 201 Created\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        });
        let mut config = config();
        config.control_plane.endpoint = Some(endpoint);

        let decision = ensure_enrolled(&mut config, &state_dir)
            .await
            .expect("ensure");
        server.await.expect("server task");

        assert_eq!(decision, EnrollmentDecision::Enrolled);
        assert_eq!(config.agent.agent_id.as_deref(), Some("agent-http"));
        assert!(crate::state_store::agent_runtime::path_for(&state_dir).exists());
    }

    #[tokio::test]
    async fn ensure_enrolled_scrubs_enrollment_token_from_config_file() {
        let state_dir = temp_dir("scrub-token");
        let config_path = state_dir.join("insightd.toml");
        fs::write(
            &config_path,
            r#"schema_version = "v1"

[control_plane]
enabled = true
endpoint = "http://127.0.0.1:3000"
enrollment_token = "token-a"
credential_request = "bearer"
"#,
        )
        .expect("write config");
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let endpoint = format!("http://{}", listener.local_addr().expect("addr"));
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut request_bytes = Vec::new();
            loop {
                let mut chunk = [0u8; 1024];
                let read = socket.read(&mut chunk).await.expect("read");
                if read == 0 {
                    break;
                }
                request_bytes.extend_from_slice(&chunk[..read]);
                if request_is_complete(&request_bytes) {
                    break;
                }
            }
            let body = r#"{"result":{"status":"accepted","reason_code":null,"agent_id":"agent-http","instance_id":"instance-http","issued_identity":null,"credential_bundle":null,"initial_config":null,"policy_binding":null}}"#;
            let response = format!(
                "HTTP/1.1 201 Created\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        });
        let mut config = config();
        config.control_plane.endpoint = Some(endpoint);

        let decision = ensure_enrolled_with_config_path(&mut config, &state_dir, &config_path)
            .await
            .expect("ensure");
        server.await.expect("server task");

        assert_eq!(decision, EnrollmentDecision::Enrolled);
        let text = fs::read_to_string(&config_path).expect("read config");
        assert!(!text.contains("enrollment_token"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(&config_path)
                .expect("config metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    fn request_is_complete(bytes: &[u8]) -> bool {
        let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            return false;
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .or_else(|| {
                headers
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
            })
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        bytes.len() >= header_end + 4 + content_length
    }

    #[test]
    fn accepted_result_updates_config_and_runtime_state() {
        let state_dir = temp_dir("accepted-result");
        let mut config = config();
        let result = AgentEnrollmentResult {
            status: AgentEnrollmentResultStatus::Accepted,
            reason_code: None,
            agent_id: None,
            instance_id: None,
            issued_identity: Some(AgentIdentity {
                agent_id: "agent-issued".to_string(),
                instance_id: "instance-issued".to_string(),
                tenant_id: "tenant-a".to_string(),
                environment_id: "env-a".to_string(),
                node_id: "node-a".to_string(),
                issued_at: "2026-07-27T00:00:00Z".to_string(),
                expires_at: None,
                status: AgentIdentityStatus::Active,
            }),
            credential_bundle: Some(AgentCredentialBundle {
                credential_id: "cred-issued".to_string(),
                agent_id: "agent-issued".to_string(),
                instance_id: "instance-issued".to_string(),
                auth_scheme: Some("bearer".to_string()),
                bearer_token: Some("bearer-issued".to_string()),
                certificate: None,
                private_key_ref: None,
                ca_bundle: None,
                issued_at: "2026-07-27T00:00:00Z".to_string(),
                not_before: Some("2026-07-27T00:00:00Z".to_string()),
                not_after: Some("2026-08-27T00:00:00Z".to_string()),
            }),
            initial_config: None,
            policy_binding: None,
        };

        super::apply_enrollment_result(&mut config, &state_dir, result).expect("apply");

        assert_eq!(config.agent.agent_id.as_deref(), Some("agent-issued"));
        assert_eq!(
            config.agent.instance_name.as_deref(),
            Some("instance-issued")
        );
        assert_eq!(config.agent.environment_id.as_deref(), Some("env-a"));
        assert_eq!(
            config.control_plane.credential_id.as_deref(),
            Some("cred-issued")
        );
        assert_eq!(
            config.control_plane.bearer_token.as_deref(),
            Some("bearer-issued")
        );
        assert_eq!(config.control_plane.auth_mode.as_deref(), Some("bearer"));
        assert_eq!(
            config.control_plane.credential_expires_at.as_deref(),
            Some("2026-08-27T00:00:00Z")
        );
        assert!(config.control_plane.enrollment_token.is_none());
        let runtime = crate::state_store::agent_runtime::load_or_default(
            &crate::state_store::agent_runtime::path_for(&state_dir),
        )
        .expect("load runtime");
        assert_eq!(runtime.credential_id.as_deref(), Some("cred-issued"));
        assert_eq!(runtime.bearer_token.as_deref(), Some("bearer-issued"));
        assert_eq!(
            runtime.credential_expires_at.as_deref(),
            Some("2026-08-27T00:00:00Z")
        );
    }

    #[tokio::test]
    async fn ensure_enrolled_state_restore_scrubs_leftover_token_from_config() {
        let state_dir = temp_dir("state-restore-scrub");
        let runtime_path = crate::state_store::agent_runtime::path_for(&state_dir);
        let mut runtime = warp_insight_contracts::state_exec::AgentRuntimeState::new(
            "agent-state".to_string(),
            "instance-state".to_string(),
            "0.1.0".to_string(),
            warp_insight_contracts::state_exec::RuntimeMode::Normal,
            "2026-07-27T00:00:00Z".to_string(),
        );
        runtime.credential_id = Some("cred-state".to_string());
        runtime.bearer_token = Some("bearer-state".to_string());
        runtime.credential_expires_at = Some("2026-08-27T00:00:00Z".to_string());
        crate::state_store::agent_runtime::store(&runtime_path, &runtime).expect("store state");

        // Simulate a crash window where state was persisted but the config scrub did not run.
        let config_path = state_dir.join("insightd.toml");
        fs::write(
            &config_path,
            r#"schema_version = "v1"

[control_plane]
enabled = true
endpoint = "http://127.0.0.1:3000"
enrollment_token = "leftover-token"
credential_request = "bearer"
"#,
        )
        .expect("write config");
        let mut config = config();

        let decision =
            ensure_enrolled_with_config_path(&mut config, &state_dir, &config_path)
                .await
                .expect("ensure");

        assert_eq!(decision, EnrollmentDecision::ExistingStateIdentity);
        let text = fs::read_to_string(&config_path).expect("read config");
        assert!(!text.contains("enrollment_token"));
    }

    #[tokio::test]
    async fn ensure_enrolled_config_identity_scrubs_leftover_token_from_config() {
        let state_dir = temp_dir("config-identity-scrub");
        let config_path = state_dir.join("insightd.toml");
        fs::write(
            &config_path,
            r#"schema_version = "v1"

[agent]
agent_id = "pre-provisioned-agent"

[control_plane]
enabled = false
endpoint = "http://127.0.0.1:3000"
enrollment_token = "leftover-token"
credential_request = "bearer"
auth_mode = "enrollment_token"
"#,
        )
        .expect("write config");
        let mut config = config();
        config.agent.agent_id = Some("pre-provisioned-agent".to_string());
        config.control_plane.enabled = false;

        let decision =
            ensure_enrolled_with_config_path(&mut config, &state_dir, &config_path)
                .await
                .expect("ensure");

        assert_eq!(decision, EnrollmentDecision::ExistingConfigIdentity);
        let text = fs::read_to_string(&config_path).expect("read config");
        assert!(!text.contains("enrollment_token"));
        assert!(!text.contains("auth_mode"));
    }

    #[test]
    fn accepted_result_rejects_unsupported_credential_scheme() {
        let state_dir = temp_dir("unsupported-scheme");
        let mut config = config();
        let result = AgentEnrollmentResult {
            status: AgentEnrollmentResultStatus::Accepted,
            reason_code: None,
            agent_id: Some("agent-x".to_string()),
            instance_id: Some("instance-x".to_string()),
            issued_identity: None,
            credential_bundle: Some(AgentCredentialBundle {
                credential_id: "cred-x".to_string(),
                agent_id: "agent-x".to_string(),
                instance_id: "instance-x".to_string(),
                auth_scheme: Some("mtls".to_string()),
                bearer_token: Some("token-should-not-persist".to_string()),
                certificate: None,
                private_key_ref: None,
                ca_bundle: None,
                issued_at: "2026-07-27T00:00:00Z".to_string(),
                not_before: None,
                not_after: Some("2026-08-27T00:00:00Z".to_string()),
            }),
            initial_config: None,
            policy_binding: None,
        };

        let err =
            super::apply_enrollment_result(&mut config, &state_dir, result).expect_err("reject");

        assert!(matches!(
            err,
            EnrollmentError::UnsupportedCredentialScheme(_)
        ));
        assert!(config.control_plane.bearer_token.is_none());
        assert!(config.control_plane.auth_mode.is_none());
        let runtime = crate::state_store::agent_runtime::load_or_default(
            &crate::state_store::agent_runtime::path_for(&state_dir),
        )
        .expect("load runtime");
        assert!(runtime.bearer_token.is_none());
    }
}
