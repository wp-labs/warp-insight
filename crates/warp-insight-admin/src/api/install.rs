use std::net::SocketAddr;

use axum::{
    extract::{connect_info::ConnectInfo, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::domain::messages::AdminAgentInstallCodeReturned;
use crate::domain::types::{AgentBootstrapBundle, AgentInstallCode, DateTime};
use crate::infra::{
    bytes_sha256_hex, new_secret_token, sha256_hex, sign_install_script, AdminConfig, AdminStore,
    StoredEnrollmentToken, StoredEnrollmentTokenStatus,
};

use super::ApiState;
use super::{admin_auth::require_admin_bearer, rate_limit};

const INSTALL_SCRIPT_TEMPLATE: &str = include_str!("install.sh");
const ENROLLMENT_TOKEN_RESERVATION_TTL_SECONDS: i64 = 60;
const BOOTSTRAP_AUTH_SCOPE: &str = "bootstrap";
const NO_STORE: &str = "no-store";

pub async fn get_agent_install_code(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    match issue_agent_install_code(&state.config, &state.store) {
        Ok(install_code) => (
            [(header::CACHE_CONTROL, NO_STORE)],
            Json(AdminAgentInstallCodeReturned { install_code }),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to issue install code: {err}"),
        )
            .into_response(),
    }
}

pub async fn get_agent_install_script(
    State(state): State<ApiState>,
    Path(arch): Path<String>,
) -> Response {
    let Ok(arch) = supported_agent_arch(&arch) else {
        return unknown_arch_response();
    };
    (
        [
            (header::CONTENT_TYPE, "text/x-shellscript; charset=utf-8"),
            (header::CACHE_CONTROL, NO_STORE),
        ],
        install_script(&state.config, arch),
    )
        .into_response()
}

pub async fn get_agent_install_script_signature(
    State(state): State<ApiState>,
    Path(arch): Path<String>,
) -> Response {
    let Ok(arch) = supported_agent_arch(&arch) else {
        return unknown_arch_response();
    };
    match install_script_signature(&state.config, &arch) {
        Ok(signature) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (header::CACHE_CONTROL, NO_STORE),
            ],
            signature,
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to sign install script: {err}"),
        )
            .into_response(),
    }
}

fn supported_agent_arch(arch: &str) -> Result<&'static str, ()> {
    match arch {
        "x86" => Ok("x86"),
        "arm" => Ok("arm"),
        _ => Err(()),
    }
}

fn unknown_arch_response() -> Response {
    (
        StatusCode::NOT_FOUND,
        [(header::CACHE_CONTROL, NO_STORE)],
        "unknown agent architecture",
    )
        .into_response()
}

pub async fn get_agent_initial_config_with_token(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Some(response) =
        rate_limit::check_rate_limit(&state, &client_key, BOOTSTRAP_AUTH_SCOPE)
    {
        return response;
    }
    let Some(token) = bootstrap_bearer_token(&headers) else {
        rate_limit::record_auth_failure(&state, &client_key, BOOTSTRAP_AUTH_SCOPE);
        return unauthorized_no_store("agent initial config requires a bootstrap bearer token");
    };

    match validate_bootstrap_token_for_config(&state.config, &state.store, token) {
        Ok(()) => {
            rate_limit::clear_auth_failures(&state, &client_key, BOOTSTRAP_AUTH_SCOPE);
            (
                [
                    (header::CONTENT_TYPE, "application/toml; charset=utf-8"),
                    (header::CACHE_CONTROL, NO_STORE),
                ],
                agent_initial_config_toml(&state.config, token),
            )
                .into_response()
        }
        Err(reason) => {
            rate_limit::record_auth_failure(&state, &client_key, BOOTSTRAP_AUTH_SCOPE);
            unauthorized_no_store(reason)
        }
    }
}

pub async fn download_agent_package(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Some(response) =
        rate_limit::check_rate_limit(&state, &client_key, BOOTSTRAP_AUTH_SCOPE)
    {
        return response;
    }
    let Some(token) = bootstrap_bearer_token(&headers) else {
        rate_limit::record_auth_failure(&state, &client_key, BOOTSTRAP_AUTH_SCOPE);
        return unauthorized_no_store("agent package download requires a bootstrap bearer token");
    };
    if let Err(reason) = validate_bootstrap_token_for_config(&state.config, &state.store, token) {
        rate_limit::record_auth_failure(&state, &client_key, BOOTSTRAP_AUTH_SCOPE);
        return unauthorized_no_store(reason);
    }
    rate_limit::clear_auth_failures(&state, &client_key, BOOTSTRAP_AUTH_SCOPE);
    match std::fs::read(&state.config.agent_package_file) {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (header::CACHE_CONTROL, NO_STORE),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"warp-insightd\"",
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "failed to read agent package {}: {err}",
                state.config.agent_package_file.display()
            ),
        )
            .into_response(),
    }
}

fn unauthorized_no_store(message: impl Into<String>) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::CACHE_CONTROL, NO_STORE)],
        message.into(),
    )
        .into_response()
}

pub fn issue_agent_install_code(
    config: &AdminConfig,
    store: &AdminStore,
) -> Result<AgentInstallCode, String> {
    let token = new_enrollment_token()?;
    let token_hash = token_hash(&token);
    let issued_at = chrono::Utc::now();
    let expires_at = issued_at + chrono::Duration::seconds(config.bootstrap_token_ttl_seconds);
    let stored = StoredEnrollmentToken {
        token_hash: token_hash.clone(),
        tenant_id: config.tenant_id.clone(),
        environment_id: config.environment_id.clone(),
        max_uses: 1,
        used_count: 0,
        issued_at: issued_at.to_rfc3339(),
        expires_at: expires_at.to_rfc3339(),
        reserved_at: None,
        status: StoredEnrollmentTokenStatus::Active,
    };
    store
        .update(|snapshot| {
            snapshot.enrollment_tokens.insert(token_hash, stored);
        })
        .map_err(|err| err.to_string())?;
    Ok(agent_install_code(config, &token, expires_at))
}

pub fn agent_install_code(
    config: &AdminConfig,
    token: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> AgentInstallCode {
    let x86_install_script_url = config.install_script_url("x86");
    let arm_install_script_url = config.install_script_url("arm");
    AgentInstallCode {
        x86_linux_install_code: install_command(config, &x86_install_script_url),
        bootstrap_enrollment_token: token.to_string(),
        arm_linux_install_code: install_command(config, &arm_install_script_url),
        bootstrap_bundle: AgentBootstrapBundle {
            bundle_id: format!("agent-bootstrap-{}", short_token_id(token)),
            install_script_url: x86_install_script_url,
            agent_package_url: config.agent_package_url(),
            agent_package_sha256: agent_package_sha256(config).unwrap_or_default(),
            control_endpoint: config.public_base_url.clone(),
            trust_bundle: config.trust_bundle.clone(),
            tenant_id: config.tenant_id.clone(),
            environment_id: config.environment_id.clone(),
            expires_at: DateTime::from_rfc3339(&expires_at.to_rfc3339())
                .unwrap_or_else(DateTime::now),
        },
    }
}

pub fn install_script(config: &AdminConfig, arch: &str) -> String {
    INSTALL_SCRIPT_TEMPLATE
        .replace("{{ARCH}}", arch)
        .replace("{{AGENT_PACKAGE_URL}}", &config.agent_package_url())
        .replace(
            "{{AGENT_INITIAL_CONFIG_URL}}",
            &config.agent_initial_config_url(),
        )
        .replace(
            "{{AGENT_PACKAGE_SHA256}}",
            &agent_package_sha256(config).unwrap_or_default(),
        )
}

fn install_command(config: &AdminConfig, script_url: &str) -> String {
    let signature_url = install_script_signature_url(script_url);
    format!(
        r#"set -eu
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT INT TERM
INSTALL_SCRIPT="$WORK_DIR/install.sh"
INSTALL_SIGNATURE="$WORK_DIR/install.sh.sig"
INSTALL_PUBLIC_KEY="$WORK_DIR/install.pub.pem"
curl -fsSL "{script_url}" -o "$INSTALL_SCRIPT"
curl -fsSL "{signature_url}" -o "$INSTALL_SIGNATURE"
cat >"$INSTALL_PUBLIC_KEY" <<'EOF'
{public_key_pem}EOF
openssl pkeyutl -verify -pubin -inkey "$INSTALL_PUBLIC_KEY" -rawin -in "$INSTALL_SCRIPT" -sigfile "$INSTALL_SIGNATURE"
sh "$INSTALL_SCRIPT""#,
        script_url = script_url,
        signature_url = signature_url,
        public_key_pem = config.install_script_signing_public_key_pem,
    )
}

fn install_script_signature_url(script_url: &str) -> String {
    format!("{script_url}.sig")
}

pub(crate) fn install_script_signature(
    config: &AdminConfig,
    arch: &str,
) -> Result<Vec<u8>, String> {
    let script = install_script(config, arch);
    sign_install_script(
        &config.install_script_signing_private_key_file,
        script.as_bytes(),
    )
}

pub fn agent_initial_config_toml(config: &AdminConfig, enrollment_token: &str) -> String {
    format!(
        r#"schema_version = "v1"

[agent]
environment_id = "{environment_id}"

[control_plane]
enabled = true
endpoint = "{endpoint}"
enrollment_token = "{enrollment_token}"
credential_request = "bearer"
tls_mode = "{tls_mode}"
trust_bundle = "{trust_bundle}"
auth_mode = "enrollment_token"

[paths]
root_dir = ".."
run_dir = "run"
state_dir = "state"
log_dir = "log"

[telemetry.logs]
in_memory_buffer_bytes = 1048576
spool_dir = "state/spool/logs"

[telemetry.logs.output]
kind = "file"

[telemetry.logs.output.file]
path = "log/warp-parse-records.ndjson"

[discovery]
host_enabled = true
network_enabled = true
endpoint_enabled = true
process_enabled = true
container_enabled = false
"#,
        environment_id = toml_escape(&config.environment_id),
        endpoint = toml_escape(&config.public_base_url),
        enrollment_token = toml_escape(enrollment_token),
        tls_mode = toml_escape(tls_mode_for_endpoint(&config.public_base_url)),
        trust_bundle = toml_escape(&config.trust_bundle),
    )
}

pub fn validate_bootstrap_token_for_config(
    config: &AdminConfig,
    store: &AdminStore,
    token: &str,
) -> Result<(), String> {
    let token_hash = token_hash(token);
    let validation = store
        .update_result(|snapshot| {
            let Some(stored) = snapshot.enrollment_tokens.get_mut(&token_hash) else {
                return Err("unknown enrollment token".to_string());
            };
            if stored.tenant_id != config.tenant_id
                || stored.environment_id != config.environment_id
            {
                return Err("enrollment token environment mismatch".to_string());
            }
            if stored.token_hash != token_hash {
                return Err("enrollment token hash mismatch".to_string());
            }
            let expires_at = chrono::DateTime::parse_from_rfc3339(&stored.expires_at)
                .map_err(|_| "enrollment token has invalid expiration".to_string())?
                .with_timezone(&chrono::Utc);
            let now = chrono::Utc::now();
            if now >= expires_at {
                stored.status = StoredEnrollmentTokenStatus::Expired;
                return Err("enrollment token is expired".to_string());
            }
            recover_expired_reservation(stored, &now);
            if stored.status != StoredEnrollmentTokenStatus::Active {
                return Err("enrollment token is not active".to_string());
            }
            if stored.used_count >= stored.max_uses {
                return Err("enrollment token is exhausted".to_string());
            }
            Ok(())
        })
        .map_err(|err| err.to_string())?;
    validation
}

pub fn agent_package_sha256(config: &AdminConfig) -> Result<String, String> {
    let bytes = std::fs::read(&config.agent_package_file).map_err(|err| err.to_string())?;
    Ok(bytes_sha256_hex(&bytes))
}

fn tls_mode_for_endpoint(endpoint: &str) -> &'static str {
    if endpoint.starts_with("https://") {
        "https"
    } else {
        "http"
    }
}

fn toml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                escaped.push_str(&format!("\\u{:04X}", ch as u32));
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn new_enrollment_token() -> Result<String, String> {
    new_secret_token("wit")
}

pub fn token_hash(token: &str) -> String {
    sha256_hex(token)
}

fn short_token_id(token: &str) -> String {
    token_hash(token).chars().take(12).collect()
}

fn bootstrap_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub fn recover_expired_reservation(
    token: &mut StoredEnrollmentToken,
    now: &chrono::DateTime<chrono::Utc>,
) {
    if token.status != StoredEnrollmentTokenStatus::Reserved {
        return;
    }
    let reserved_at = token
        .reserved_at
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&chrono::Utc));
    let Some(reserved_at) = reserved_at else {
        token.used_count = token.used_count.saturating_sub(1);
        token.reserved_at = None;
        token.status = StoredEnrollmentTokenStatus::Active;
        return;
    };
    if *now - reserved_at < chrono::Duration::seconds(ENROLLMENT_TOKEN_RESERVATION_TTL_SECONDS) {
        return;
    }
    token.used_count = token.used_count.saturating_sub(1);
    token.reserved_at = None;
    token.status = StoredEnrollmentTokenStatus::Active;
}
