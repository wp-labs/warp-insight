//! Startup enrollment client for managed agents.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use warp_insight_contracts::agent_config::AgentConfigContract;
use warp_insight_contracts::enrollment::{
    AgentEnrollmentResult, AgentEnrollmentResultReturned, AgentEnrollmentResultStatus,
    AgentHostProfile, SubmitEnrollmentRequest,
};
use warp_insight_contracts::state_exec::{AgentRuntimeState, RuntimeMode};
use warp_insight_shared::time::now_rfc3339;

use crate::state_store;

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
    Rejected {
        status: AgentEnrollmentResultStatus,
        reason_code: Option<String>,
    },
    InvalidAcceptedResult(&'static str),
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
            Self::Http(err) => write!(f, "enrollment http error: {err}"),
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
        }
    }
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
    if has_config_identity(config) {
        return Ok(EnrollmentDecision::ExistingConfigIdentity);
    }
    if load_state_identity(config, state_dir)? {
        return Ok(EnrollmentDecision::ExistingStateIdentity);
    }
    if !config.control_plane.enabled {
        return Ok(EnrollmentDecision::Disabled);
    }

    let endpoint = required_option(config.control_plane.endpoint.as_deref())
        .ok_or(EnrollmentError::MissingEndpoint)?;
    let token = required_option(config.control_plane.enrollment_token.as_deref())
        .ok_or(EnrollmentError::MissingEnrollmentToken)?;
    let request = build_enrollment_request(config, token.to_string());
    let returned = post_enrollment(endpoint, &request).await?;
    apply_enrollment_result(config, state_dir, returned.result)?;
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

    config.agent.agent_id = Some(runtime_state.agent_id);
    config.agent.instance_name = Some(runtime_state.instance_id);
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
    endpoint: &str,
    request: &SubmitEnrollmentRequest,
) -> Result<AgentEnrollmentResultReturned, EnrollmentError> {
    let url = format!("{}/api/v1/agent/enroll", endpoint.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .json(request)
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json::<AgentEnrollmentResultReturned>().await?)
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
    config.agent.agent_id = Some(agent_id.clone());
    config.agent.instance_name = Some(instance_id.clone());

    let runtime_path = state_store::agent_runtime::path_for(state_dir);
    let runtime_state = AgentRuntimeState::new(
        agent_id,
        instance_id,
        env!("CARGO_PKG_VERSION").to_string(),
        RuntimeMode::Normal,
        now_rfc3339(),
    );
    state_store::agent_runtime::store(&runtime_path, &runtime_state)?;
    Ok(())
}

fn is_registered_agent_id(value: &str) -> bool {
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
        AgentEnrollmentResult, AgentEnrollmentResultStatus, AgentIdentity, AgentIdentityStatus,
    };

    use super::{
        EnrollmentDecision, EnrollmentError, build_enrollment_request, ensure_enrolled,
        hostname_from_sources,
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
                tls_mode: None,
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
            credential_bundle: None,
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
        assert!(crate::state_store::agent_runtime::path_for(&state_dir).exists());
    }
}
