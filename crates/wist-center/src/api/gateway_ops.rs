// ReceiveGatewayStatusReport 接收链路：POST /api/v1/gateway/status。
// 镜像 warp-gateway 的 submit_agent_status：Bearer 鉴权（sha256 常数时间比较）→
// store 落库最新状态 → 返回 GatewayStatusAcceptedReturned。

use std::net::SocketAddr;

use axum::{
    extract::{connect_info::ConnectInfo, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use insight_control::types::DateTime;
use insight_control::{
    GatewayCredentialBundle, GatewayCredentialVerificationResult, GatewayEnrollmentResult,
    GatewayEnrollmentResultReturned, GatewayInitialConfig, GatewayInitializationStatus,
    GatewayInstanceLifecycleState, GatewayStatusAccepted, GatewayStatusAcceptedReturned,
    QueryGatewayInitializationStatus, RegisterGateway, ReportGatewayStatus,
    VerifyGatewayCredential,
};

use crate::infra::{
    derive_regist_token, new_secret_token, sha256_hex, EnrollmentTokenIssue, GatewayStatusUpdate,
    StoreReason, StoredAgent, StoredGateway, StoredGatewayCredentialStatus,
};

use super::{build_control_center_trust_bundle, control_center_tls_required, rate_limit, ApiState};

const GATEWAY_AUTH_SCOPE: &str = "gateway";
/// 注册自携带 token 鉴权，无网关身份可查，独立限流桶防 token 暴力枚举。
const GATEWAY_REGISTER_SCOPE: &str = "gateway-register";

/// 允许 Gateway Web 以 Authorization Header 直接调用初始化端点。
/// 该端点不使用 Cookie，因此使用通配来源不会扩大用户会话权限；Token 仍只在 Header 中传输。
/// 处理浏览器对 Gateway 初始化请求的 Authorization 预检。
pub async fn options_gateway_initial_config() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

/// Gateway 上报其下 Agent 状态（POST /api/v1/gateway/agents/status）。
#[derive(serde::Deserialize)]
pub struct AgentStatusReportRequest {
    pub gateway_id: String,
    pub agents: Vec<AgentStatusEntry>,
}

#[derive(serde::Deserialize)]
pub struct AgentStatusEntry {
    pub agent_id: String,
    pub instance_id: String,
    pub version: String,
    pub status: String,
    pub health: String,
    #[serde(default)]
    pub memory_bytes: Option<i64>,
    #[serde(default)]
    pub cpu_percent: Option<f64>,
    #[serde(default)]
    pub admin_latency_ms: Option<i64>,
    pub last_seen_at: insight_control::types::DateTime,
}

/// 接收 Gateway 上报的 Agent 状态：Bearer 按 gateway_id 凭证鉴权 → store upsert → VM 推送。
pub async fn submit_agent_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<AgentStatusReportRequest>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    match authenticate_gateway(&state, &headers, &input.gateway_id, &client_key).await {
        Ok(_) => {
            let stored: Vec<StoredAgent> = input
                .agents
                .iter()
                .map(|agent| StoredAgent {
                    agent_id: agent.agent_id.clone(),
                    gateway_id: input.gateway_id.clone(),
                    instance_id: agent.instance_id.clone(),
                    version: agent.version.clone(),
                    status: agent.status.clone(),
                    health: agent.health.clone(),
                    memory_bytes: agent.memory_bytes,
                    cpu_percent: agent.cpu_percent,
                    admin_latency_ms: agent.admin_latency_ms,
                    last_seen_at: agent.last_seen_at.clone(),
                })
                .collect();
            if let Err(err) = state
                .store
                .upsert_agent_status(&input.gateway_id, &stored)
                .await
            {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to update agent status: {err}"),
                )
                    .into_response();
            }
            if let Some(vm_url) = &state.config.victoriametrics_url {
                if let Err(err) = crate::infra::vm::push_agent_status(
                    vm_client(),
                    vm_url,
                    &input.gateway_id,
                    &stored,
                )
                .await
                {
                    eprintln!("warn agent_status vm push failed: {err}");
                }
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "gateway_id": input.gateway_id,
                    "agents_accepted": stored.len(),
                })),
            )
                .into_response()
        }
        Err(response) => response,
    }
}

/// VM 推送全局 HTTP 客户端（进程内复用连接池）。
fn vm_client() -> &'static reqwest::Client {
    crate::infra::vm::shared_vm_client()
}

/// 下载本地镜像的制品：GET /api/v1/releases/artifact/:component/:version/:filename。
pub async fn download_release_artifact(
    State(state): State<ApiState>,
    Path((component, version, filename)): Path<(String, String, String)>,
) -> Response {
    let path = state
        .config
        .artifact_dir
        .join(&component)
        .join(&version)
        .join(&filename);
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "artifact not found").into_response(),
    }
}

/// 网关注册：POST /api/v1/gateway/register。
/// 对应模型 `RegisterGateway` + `RegisterGatewayFlow`：WarpGateway 持一次性
/// RegistToken（enrollment token）提交注册。消费 token（防重放/限量/吊销/过期）后
/// **签发独立运行期凭据（RUNTIME_TOKEN）**：RegistToken 只用于本次注册，
/// 运行期 Bearer（initial-config / status）以新签发的凭据为准。
pub async fn register_gateway(
    State(state): State<ApiState>,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<RegisterGateway>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    // 注册自携带 token 鉴权（无网关身份可查），独立限流桶防 token 暴力枚举。
    if let Some(response) =
        rate_limit::check_rate_limit(&state, &client_key, GATEWAY_REGISTER_SCOPE)
    {
        return response;
    }
    let consumed = match state
        .store
        .consume_enrollment_token(&input.enrollment_token)
        .await
    {
        Ok(token) => token,
        Err(err) if err.reason() == &StoreReason::Enrollment => {
            rate_limit::record_auth_failure(&state, &client_key, GATEWAY_REGISTER_SCOPE);
            let reason = err
                .detail()
                .as_deref()
                .unwrap_or("enrollment token rejected");
            return (
                StatusCode::UNAUTHORIZED,
                format!("enrollment token rejected: {reason}"),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to consume enrollment token: {err}"),
            )
                .into_response();
        }
    };
    // token 绑定的网关必须已创建。
    let gateway_exists = match state.store.get_gateway(&consumed.gateway_id).await {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load gateway store: {err}"),
            )
                .into_response();
        }
    };
    if !gateway_exists {
        rate_limit::record_auth_failure(&state, &client_key, GATEWAY_REGISTER_SCOPE);
        return (
            StatusCode::UNAUTHORIZED,
            "enrollment token bound to unknown gateway".to_string(),
        )
            .into_response();
    }
    // 注册成功：签发独立运行期凭据（RUNTIME_TOKEN），覆盖引导后的空运行期凭据。
    // 镜像 warp-gateway renew_agent_credential：新 token + credential_id + 过期，原子替换。
    let (bundle, credential_hash, credential_expires_at) =
        match issue_runtime_credential(&state.config, &consumed.gateway_id, &input.instance_id) {
            Ok(issued) => issued,
            Err(reason) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, reason).into_response();
            }
        };
    let updated = state
        .store
        .update_gateway_credential(
            &consumed.gateway_id,
            &credential_hash,
            credential_expires_at,
        )
        .await
        .unwrap_or(false);
    if !updated {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to persist gateway runtime credential".to_string(),
        )
            .into_response();
    }
    // 生命周期：Provisioned → Initializing（注册成功即进入初始化）。
    if let Err(err) = state
        .store
        .mark_gateway_initializing(&consumed.gateway_id)
        .await
    {
        eprintln!("warn mark gateway initializing failed: {err}");
    }
    rate_limit::clear_auth_failures(&state, &client_key, GATEWAY_REGISTER_SCOPE);
    Json(GatewayEnrollmentResultReturned {
        result: GatewayEnrollmentResult {
            status: "accepted".to_string(),
            gateway_id: consumed.gateway_id.clone(),
            instance_id: input.instance_id,
            credential_id: bundle.credential_id.clone(),
            initial_config: "v1".to_string(),
            credential_bundle: bundle,
        },
    })
    .into_response()
}

/// 签发独立运行期凭据（RUNTIME_TOKEN）：随机 bearer + credential_id + 过期时间。
/// 返回 (credential bundle, sha256(bearer) 落库用, expires_at rfc3339)。
fn issue_runtime_credential(
    config: &crate::config::CenterConfig,
    gateway_id: &str,
    instance_id: &str,
) -> Result<(GatewayCredentialBundle, String, Option<String>), String> {
    let bearer_token = new_secret_token("wic")?;
    let credential_id = new_secret_token("cred")?;
    let issued_at_time = chrono::Utc::now();
    let issued_at = issued_at_time.to_rfc3339();
    let expires_at =
        (issued_at_time + chrono::Duration::seconds(config.credential_ttl_seconds)).to_rfc3339();
    let bundle = GatewayCredentialBundle {
        credential_id: credential_id.clone(),
        gateway_id: gateway_id.to_string(),
        instance_id: instance_id.to_string(),
        auth_scheme: "bearer".to_string(),
        bearer_token: bearer_token.clone(),
        issued_at: DateTime::from_rfc3339(&issued_at).unwrap_or_else(DateTime::now),
        expires_at: DateTime::from_rfc3339(&expires_at).unwrap_or_else(DateTime::now),
    };
    Ok((bundle, sha256_hex(&bearer_token), Some(expires_at)))
}

#[derive(serde::Deserialize)]
pub struct InitialConfigQueryParams {
    pub instance_id: Option<String>,
}

/// 拉取网关初始配置：GET /api/v1/gateway/initial-config。
/// 对齐模型 `ProvisionGatewayFlow`；gateway 面 Bearer 鉴权。两种状态：
/// - **未初始化**（有 bootstrap、无运行期凭据）：Bearer 为一次性 BootstrapToken，
///   携带 X-Gateway-Identity-Token → 派生 RegistToken 落 enrollment → 消费 bootstrap → 出 config.toml。
/// - **已初始化**（有运行期凭据）：现有 authenticate_gateway（Bearer RUNTIME_TOKEN）→ 出同一 config.toml。
/// 返回 `application/json`：`config` 为 GatewayInitialConfig，置备态同时返回明文 RegistToken。
pub async fn get_gateway_initial_config(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Query(params): Query<InitialConfigQueryParams>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    let Some(instance_id) = params.instance_id.as_deref() else {
        return (StatusCode::BAD_REQUEST, "missing instance_id").into_response();
    };
    // 安全 #1（fail-closed）：TLS 开启但未配置信任根 → 拒绝服务，
    // 避免网关在无法校验中心证书的情况下继续初始化（可被中间人）。
    if control_center_tls_required(&state.config)
        && build_control_center_trust_bundle(&state.config, instance_id).is_none()
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "TLS is required but the control center trust root is not configured",
        )
            .into_response();
    }
    let gateway = match state.store.get_gateway(instance_id).await {
        Ok(Some(gateway)) => gateway,
        Ok(None) => {
            rate_limit::record_auth_failure(&state, &client_key, GATEWAY_AUTH_SCOPE);
            return (StatusCode::UNAUTHORIZED, "unknown gateway").into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load gateway store: {err}"),
            )
                .into_response();
        }
    };
    // 未初始化 → 置备路径。
    if gateway.credential_token_hash.is_empty() && !gateway.bootstrap_token_hash.is_empty() {
        return provision_gateway_initial_config(
            &state,
            &headers,
            instance_id,
            &gateway,
            &client_key,
        )
        .await;
    }
    // 已初始化 → 现有运行期凭据鉴权。
    match authenticate_gateway(&state, &headers, instance_id, &client_key).await {
        Ok(gateway) => {
            let enrollment_token_id = state
                .store
                .get_enrollment_token_for_gateway(instance_id)
                .await
                .ok()
                .flatten()
                .map(|token| token.token_id)
                .unwrap_or_default();
            let config =
                build_initial_config_json(&state, &gateway, instance_id, &enrollment_token_id);
            Json(InitialConfigReturned {
                config,
                regist_token: None,
            })
            .into_response()
        }
        Err(response) => response,
    }
}

/// 置备路径：Bearer 一次性 BootstrapToken 鉴权 + X-Gateway-Identity-Token 派生
/// RegistToken → 落 enrollment token（供 /register 消费）→ 成功后消费 bootstrap → 出 config.toml。
async fn provision_gateway_initial_config(
    state: &ApiState,
    headers: &HeaderMap,
    instance_id: &str,
    gateway: &StoredGateway,
    client_key: &str,
) -> Response {
    let Some(bootstrap_token) = bearer_token(headers) else {
        return (StatusCode::UNAUTHORIZED, "missing bearer credential").into_response();
    };
    if sha256_hex(bootstrap_token) != gateway.bootstrap_token_hash {
        rate_limit::record_auth_failure(state, client_key, GATEWAY_AUTH_SCOPE);
        return (StatusCode::UNAUTHORIZED, "invalid bootstrap token").into_response();
    }
    let identity_token = headers
        .get("x-gateway-identity-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(identity_token) = identity_token else {
        return (
            StatusCode::BAD_REQUEST,
            "missing X-Gateway-Identity-Token".to_string(),
        )
            .into_response();
    };
    // 派生 RegistToken：HMAC(center_secret, "gateway-reg:" + gateway_id + ":" + identity_token)。
    let regist_token = derive_regist_token(&state.config.hmac_secret, instance_id, identity_token);
    // 落 enrollment token（sha256(regist_token)，max_uses=1，Active），供 /register 一次性消费。
    let enrollment = match state
        .store
        .create_enrollment_token(
            instance_id,
            &EnrollmentTokenIssue {
                token: regist_token.clone(),
                issued_by: "provision".to_string(),
                control_center_trust_bundle: state.config.ca_cert.clone(),
            },
        )
        .await
    {
        Ok(token) => token,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to issue regist token: {err}"),
            )
                .into_response();
        }
    };
    // 成功落库 RegistToken 后才消费 bootstrap（一次性；网络抖动可重试置备）。
    match state
        .store
        .consume_bootstrap_token(instance_id, bootstrap_token)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::CONFLICT,
                "bootstrap token already consumed or gateway initialized".to_string(),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to consume bootstrap token: {err}"),
            )
                .into_response();
        }
    }
    // 生命周期：Provisioned → Initializing。
    if let Err(err) = state.store.mark_gateway_initializing(instance_id).await {
        eprintln!("warn mark gateway initializing failed: {err}");
    }
    rate_limit::clear_auth_failures(state, client_key, GATEWAY_AUTH_SCOPE);
    let config = build_initial_config_json(state, &gateway, instance_id, &enrollment.token_id);
    Json(InitialConfigReturned {
        config,
        regist_token: Some(regist_token),
    })
    .into_response()
}

/// initial-config 响应（JSON 契约）：中心下发的控制面连接配置 + 派生 RegistToken。
/// 网关/simulator 据此生成本地 config.toml（配置文件由网关侧落盘）。
#[derive(serde::Serialize, serde::Deserialize)]
pub struct InitialConfigReturned {
    pub config: GatewayInitialConfig,
    /// 置备路径：RegistToken 明文（注册用）；已初始化路径：None（已用运行期凭据）。
    pub regist_token: Option<String>,
}

/// 构建 initial-config 的 config 部分（JSON 字段，网关侧据此写 config.toml）。
fn build_initial_config_json(
    state: &ApiState,
    gateway: &StoredGateway,
    instance_id: &str,
    enrollment_token_id: &str,
) -> GatewayInitialConfig {
    GatewayInitialConfig {
        gateway_id: gateway.gateway_id.clone(),
        control_center_endpoint: state.config.public_url.trim_end_matches('/').to_string(),
        trust_bundle: build_control_center_trust_bundle(&state.config, instance_id),
        server_tls_required: control_center_tls_required(&state.config),
        protocol_version: state.config.protocol_version.clone(),
        enrollment_token_id: enrollment_token_id.to_string(),
    }
}

pub async fn submit_gateway_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<ReportGatewayStatus>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    match authenticate_gateway(&state, &headers, &input.gateway_id, &client_key).await {
        Ok(_) => {
            let accepted_at = input.reported_at.clone();
            let update = GatewayStatusUpdate {
                gateway_id: input.gateway_id.clone(),
                instance_id: input.instance_id.clone(),
                version: input.version.clone(),
                status: input.status.clone(),
                health: input.health.clone(),
                memory_bytes: input.memory_bytes,
                cpu_percent: input.cpu_percent,
                last_seen_at: accepted_at.clone(),
            };
            let update_result = state.store.upsert_gateway_status(&update).await;
            if let Err(err) = update_result {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to update gateway status: {err}"),
                )
                    .into_response();
            }
            // 时序历史：配置了 VictoriaMetrics 则推送指标（失败仅告警，不影响上报成功）。
            if let Some(vm_url) = &state.config.victoriametrics_url {
                if let Err(err) =
                    crate::infra::vm::push_gateway_status(vm_client(), vm_url, &update).await
                {
                    eprintln!("warn gateway_status vm push failed: {err}");
                }
            }
            (
                StatusCode::OK,
                Json(GatewayStatusAcceptedReturned {
                    receipt: GatewayStatusAccepted {
                        gateway_id: input.gateway_id,
                        instance_id: input.instance_id,
                        accepted_at,
                    },
                }),
            )
                .into_response()
        }
        Err(response) => response,
    }
}

/// 按 binding 的 `actor_identity WarpGateway.id from credential.gateway_id`：
/// bearer token → 匹配 store 中该 gateway 的凭证 hash（常数时间比较）→
/// Active 且未过期 → 与上报 gateway_id 一致。
/// 续期运行期凭据请求体：以当前 RUNTIME_TOKEN 鉴权后签发新凭据。
#[derive(serde::Deserialize)]
pub struct RenewGatewayCredentialRequest {
    pub gateway_id: String,
    pub instance_id: String,
}

/// 续期运行期凭据：POST /api/v1/gateway/credentials:renew。
/// 以当前 RUNTIME_TOKEN 鉴权（authenticate_gateway）→ 签发新 RUNTIME_TOKEN + credential_id
/// → 原子替换 hash → 旧 token 立即失效。镜像 warp-gateway `renew_agent_credential`。
pub async fn renew_gateway_credential(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<RenewGatewayCredentialRequest>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    match authenticate_gateway(&state, &headers, &input.gateway_id, &client_key).await {
        Ok(_) => {
            let (bundle, credential_hash, credential_expires_at) = match issue_runtime_credential(
                &state.config,
                &input.gateway_id,
                &input.instance_id,
            ) {
                Ok(issued) => issued,
                Err(reason) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, reason).into_response();
                }
            };
            let updated = state
                .store
                .update_gateway_credential(
                    &input.gateway_id,
                    &credential_hash,
                    credential_expires_at,
                )
                .await
                .unwrap_or(false);
            if !updated {
                return (
                    StatusCode::UNAUTHORIZED,
                    "invalid gateway credential".to_string(),
                )
                    .into_response();
            }
            rate_limit::clear_auth_failures(&state, &client_key, GATEWAY_AUTH_SCOPE);
            (StatusCode::OK, Json(bundle)).into_response()
        }
        Err(response) => response,
    }
}

async fn authenticate_gateway(
    state: &ApiState,
    headers: &HeaderMap,
    gateway_id: &str,
    client_key: &str,
) -> Result<StoredGateway, Response> {
    if let Some(response) = rate_limit::check_rate_limit(state, client_key, GATEWAY_AUTH_SCOPE) {
        return Err(response);
    }
    let Some(token) = bearer_token(headers) else {
        // 缺 token 属未认证请求，不计入暴力尝试。
        return Err((StatusCode::UNAUTHORIZED, "missing bearer credential").into_response());
    };
    let token_hash = sha256_hex(token);
    let gateway = match state.store.get_gateway(gateway_id).await {
        Ok(gateway) => gateway,
        Err(err) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load gateway credential store: {err}"),
            )
                .into_response());
        }
    };
    let Some(gateway) = gateway else {
        rate_limit::record_auth_failure(state, client_key, GATEWAY_AUTH_SCOPE);
        return Err((StatusCode::UNAUTHORIZED, "unknown gateway credential").into_response());
    };
    // 网关身份由 `actor_identity WarpGateway.id from credential.gateway_id` 表达：
    // 这里按上报 gateway_id 查到的凭证做常数时间 token 比较，即身份一致性校验。
    if !constant_time_eq(
        gateway.credential_token_hash.as_bytes(),
        token_hash.as_bytes(),
    ) {
        rate_limit::record_auth_failure(state, client_key, GATEWAY_AUTH_SCOPE);
        return Err((StatusCode::UNAUTHORIZED, "invalid gateway credential").into_response());
    }
    if gateway.credential_status != StoredGatewayCredentialStatus::Active {
        return Err((StatusCode::UNAUTHORIZED, "gateway credential is not active").into_response());
    }
    if let Some(expires_at) = &gateway.credential_expires_at {
        if credential_is_expired(expires_at) {
            return Err((StatusCode::UNAUTHORIZED, "gateway credential is expired").into_response());
        }
    }
    rate_limit::clear_auth_failures(state, client_key, GATEWAY_AUTH_SCOPE);
    Ok(gateway)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn credential_is_expired(expires_at: &str) -> bool {
    let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(expires_at) else {
        return true;
    };
    chrono::Utc::now() >= expires_at.with_timezone(&chrono::Utc)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut diff = left.len() ^ right.len();
    let max_len = left.len().max(right.len());
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        diff |= (left_byte ^ right_byte) as usize;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::post, Router};
    use http_body_util::BodyExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::ServiceExt;

    use crate::{api::ApiState, config::GatewayCredentialSeed, infra::FileStore};

    fn test_state() -> ApiState {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wic-api-test-{nanos}.json"));
        let store = FileStore::new(path);
        store
            .seed(&[GatewayCredentialSeed {
                gateway_id: "gw-001".to_string(),
                token: "secret-token-1".to_string(),
                expires_at: None,
            }])
            .expect("seed");
        ApiState {
            config: crate::config::CenterConfig {
                listen_addr: "127.0.0.1:3100".to_string(),
                store_path: std::env::temp_dir().join(format!("wic-api-test-{nanos}.json")),
                gateway_credentials: Vec::new(),
                admin_token_hash: None,
                database_url: None,
                victoriametrics_url: None,
                public_url: "http://127.0.0.1:3100".to_string(),
                gateway_image: "warp-gateway:latest".to_string(),
                artifact_dir: std::env::temp_dir().join("wic-artifacts"),
                object_storage: None,
                ca_cert: None,
                protocol_version: "1.0".to_string(),
                hmac_secret: "test-hmac-secret".to_string(),
                credential_ttl_seconds: 3600,
            },
            store: std::sync::Arc::new(store),
            artifact_store: std::sync::Arc::new(crate::infra::LocalArtifactStore::new(
                std::env::temp_dir().join("wic-artifacts"),
                "http://127.0.0.1:3100",
            )),
            rate_limits: std::sync::Arc::new(std::sync::Mutex::new(
                super::super::rate_limit::RateLimitState::default(),
            )),
        }
    }

    fn router() -> Router {
        let state = test_state();
        Router::new()
            .route("/api/v1/gateway/status", post(submit_gateway_status))
            .with_state(state)
    }

    /// 构造带 seed 网关 + 注册 Token 的完整路由（register 端点走 router_for）。
    fn register_state(enrollment_token: &str) -> ApiState {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wic-register-{nanos}.json"));
        let store = FileStore::new(&path);
        store
            .seed(&[GatewayCredentialSeed {
                gateway_id: "gw-001".to_string(),
                token: "cred-tok".to_string(),
                expires_at: None,
            }])
            .expect("seed");
        store
            .create_enrollment_token(
                "gw-001",
                &crate::infra::EnrollmentTokenIssue {
                    token: enrollment_token.to_string(),
                    issued_by: "test".to_string(),
                    control_center_trust_bundle: None,
                },
            )
            .expect("enroll");
        ApiState {
            config: crate::config::CenterConfig {
                listen_addr: "127.0.0.1:3100".to_string(),
                store_path: std::env::temp_dir().join(format!("wic-register-{nanos}.json")),
                gateway_credentials: Vec::new(),
                admin_token_hash: None,
                database_url: None,
                victoriametrics_url: None,
                public_url: "http://127.0.0.1:3100".to_string(),
                gateway_image: "warp-gateway:latest".to_string(),
                artifact_dir: std::env::temp_dir().join("wic-artifacts"),
                object_storage: None,
                ca_cert: None,
                protocol_version: "1.0".to_string(),
                hmac_secret: "test-hmac-secret".to_string(),
                credential_ttl_seconds: 3600,
            },
            store: std::sync::Arc::new(store),
            artifact_store: std::sync::Arc::new(crate::infra::LocalArtifactStore::new(
                std::env::temp_dir().join("wic-artifacts"),
                "http://127.0.0.1:3100",
            )),
            rate_limits: std::sync::Arc::new(std::sync::Mutex::new(
                super::super::rate_limit::RateLimitState::default(),
            )),
        }
    }

    /// 构造"已创建未置备"网关（create_gateway 存 bootstrap hash、无运行期凭据）的完整路由。
    fn provision_state(bootstrap_token: &str) -> ApiState {
        // 临时文件用「纳秒 + 原子计数器」命名，避免并发测试同纳秒撞同一路径导致 Conflict。
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("wic-provision-{nanos}-{counter}.json"));
        let store = FileStore::new(&path);
        store
            .create_gateway("gw-p", bootstrap_token)
            .expect("create");
        ApiState {
            config: crate::config::CenterConfig {
                listen_addr: "127.0.0.1:3100".to_string(),
                store_path: path.clone(),
                gateway_credentials: Vec::new(),
                admin_token_hash: None,
                database_url: None,
                victoriametrics_url: None,
                public_url: "http://127.0.0.1:3100".to_string(),
                gateway_image: "warp-gateway:latest".to_string(),
                artifact_dir: std::env::temp_dir().join("wic-artifacts"),
                object_storage: None,
                ca_cert: None,
                protocol_version: "1.0".to_string(),
                hmac_secret: "test-hmac-secret".to_string(),
                credential_ttl_seconds: 3600,
            },
            store: std::sync::Arc::new(store),
            artifact_store: std::sync::Arc::new(crate::infra::LocalArtifactStore::new(
                std::env::temp_dir().join("wic-artifacts"),
                "http://127.0.0.1:3100",
            )),
            rate_limits: std::sync::Arc::new(std::sync::Mutex::new(
                super::super::rate_limit::RateLimitState::default(),
            )),
        }
    }

    fn status_payload(gateway_id: &str) -> String {
        format!(
            r#"{{"gateway_id":"{gateway_id}","instance_id":"inst-1","version":"v2.4.1","status":"online","health":"healthy","reported_at":"2026-08-07T12:00:00Z"}}"#
        )
    }

    #[tokio::test]
    async fn accepts_status_report_with_valid_credential() {
        let response = router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/status")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer secret-token-1")
                    .body(Body::from(status_payload("gw-001")))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let returned: GatewayStatusAcceptedReturned = serde_json::from_slice(&body).expect("json");
        assert_eq!(returned.receipt.gateway_id, "gw-001");
        assert_eq!(returned.receipt.instance_id, "inst-1");
    }

    #[tokio::test]
    async fn rejects_report_with_invalid_credential() {
        let response = router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/status")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer wrong-token")
                    .body(Body::from(status_payload("gw-001")))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rejects_report_with_unknown_gateway() {
        let response = router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/status")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer secret-token-1")
                    .body(Body::from(status_payload("gw-999")))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    fn register_payload(token: &str) -> String {
        format!(
            r#"{{"enrollment_token":"{token}","instance_id":"inst-1","requested_at":"2026-08-11T00:00:00Z"}}"#
        )
    }

    #[tokio::test]
    async fn register_consumes_token_once_then_rejects_replay() {
        let app = super::super::router_for(register_state("enroll-tok-a"));

        // 首次注册 → 200 accepted + 注册回执。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/register")
                    .header("content-type", "application/json")
                    .body(Body::from(register_payload("enroll-tok-a")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let response_body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let returned: GatewayEnrollmentResultReturned =
            serde_json::from_slice(&response_body).expect("json");
        assert_eq!(returned.result.status, "accepted");
        assert_eq!(returned.result.gateway_id, "gw-001");
        assert_eq!(returned.result.instance_id, "inst-1");
        // 注册后签发独立运行期凭据（RUNTIME_TOKEN）：随机 bearer + 过期时间。
        let bundle = &returned.result.credential_bundle;
        assert!(
            bundle.bearer_token.starts_with("wic_"),
            "runtime token: {}",
            bundle.bearer_token
        );
        assert_eq!(bundle.auth_scheme, "bearer");
        assert_eq!(bundle.gateway_id, "gw-001");
        assert_eq!(bundle.instance_id, "inst-1");
        assert!(bundle.expires_at > bundle.issued_at);

        // 防重放：同一 token 二次注册 → 401（Exhausted）。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/register")
                    .header("content-type", "application/json")
                    .body(Body::from(register_payload("enroll-tok-a")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // 运行期分离核心断言：注册后只有新签发的 RUNTIME_TOKEN 有效。
        let runtime_token = bundle.bearer_token.clone();
        // RegistToken（enroll-tok-a）作 Bearer 调 status → 401（已消费，且非运行期凭据）。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/status")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer enroll-tok-a")
                    .body(Body::from(status_payload("gw-001")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        // 种子运行期凭据（cred-tok）被新凭据替换 → 401。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/status")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer cred-tok")
                    .body(Body::from(status_payload("gw-001")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        // 新签发的 RUNTIME_TOKEN → 200。
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/status")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {runtime_token}"))
                    .body(Body::from(status_payload("gw-001")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn provision_initial_config_derives_regist_and_consumes_bootstrap() {
        use axum::{body::Body, http::Request};
        use http_body_util::BodyExt;
        use tower::ServiceExt;
        let app = super::super::router_for(provision_state("boot-tok-p"));

        // 未置备：Bearer 一次性 bootstrap + X-Gateway-Identity-Token → 200 + config.toml（含派生 RegistToken）。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/gateway/initial-config?instance_id=gw-p")
                    .header("authorization", "Bearer boot-tok-p")
                    .header("x-gateway-identity-token", "identity-p")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(
            content_type.contains("application/json"),
            "content-type: {content_type}"
        );
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let returned: InitialConfigReturned = serde_json::from_slice(&body).expect("json");
        // 派生 RegistToken 通过 JSON 响应的 regist_token 字段返回。
        let expected_regist =
            crate::infra::derive_regist_token("test-hmac-secret", "gw-p", "identity-p");
        assert_eq!(
            returned.regist_token.as_deref(),
            Some(expected_regist.as_str())
        );
        assert!(!returned.config.server_tls_required);
        assert!(returned
            .config
            .enrollment_token_id
            .starts_with("enroll-gw-p"));

        // bootstrap 一次性：置备成功后复用 → 401（已消费，且无运行期凭据）。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/gateway/initial-config?instance_id=gw-p")
                    .header("authorization", "Bearer boot-tok-p")
                    .header("x-gateway-identity-token", "identity-p")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // 错 bootstrap → 401（不落 enrollment、不消费）。
        let app2 = super::super::router_for(provision_state("boot-2"));
        let response = app2
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/gateway/initial-config?instance_id=gw-p")
                    .header("authorization", "Bearer wrong-boot")
                    .header("x-gateway-identity-token", "identity-p")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn renew_gateway_credential_rotates_and_invalidates_old() {
        use axum::{body::Body, http::Request};
        use http_body_util::BodyExt;
        use tower::ServiceExt;
        // 以 seed 运行期凭据作为当前凭据（authenticate_gateway 可过），renew 轮换。
        let app = super::super::router_for(register_state("enroll-r"));
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/credentials:renew")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer cred-tok")
                    .body(Body::from(
                        r#"{"gateway_id":"gw-001","instance_id":"inst-1"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let renewed: GatewayCredentialBundle = serde_json::from_slice(&body).expect("json");
        assert!(renewed.bearer_token.starts_with("wic_"));
        assert_ne!(renewed.bearer_token, "cred-tok");

        // 旧凭据立即失效 → 401；新凭据 → 200。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/status")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer cred-tok")
                    .body(Body::from(status_payload("gw-001")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/status")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", renewed.bearer_token))
                    .body(Body::from(status_payload("gw-001")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn initial_config_preflight_allows_gateway_web_authorization_header() {
        let response = super::super::router_for(register_state("enroll-tok-c"))
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/v1/gateway/initial-config?instance_id=gw-001")
                    .header("origin", "http://127.0.0.1:5174")
                    .header("access-control-request-method", "GET")
                    .header("access-control-request-headers", "authorization")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .and_then(|value| value.to_str().ok()),
            Some("*")
        );
        assert!(response
            .headers()
            .get("access-control-allow-headers")
            .expect("allow headers")
            .to_str()
            .expect("header value")
            .contains("authorization"));
    }

    #[tokio::test]
    async fn register_rejects_unknown_token() {
        let app = super::super::router_for(register_state("enroll-tok-a"));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/register")
                    .header("content-type", "application/json")
                    .body(Body::from(register_payload("enroll-tok-unknown")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn register_rejects_revoked_token() {
        // 构造 FileStore → seed 网关 + 签发 token → 吊销 → 再注册 → 401。
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wic-register-revoked-{nanos}.json"));
        let file_store = FileStore::new(&path);
        file_store
            .seed(&[GatewayCredentialSeed {
                gateway_id: "gw-001".to_string(),
                token: "cred-tok".to_string(),
                expires_at: None,
            }])
            .expect("seed");
        let token = file_store
            .create_enrollment_token(
                "gw-001",
                &crate::infra::EnrollmentTokenIssue {
                    token: "enroll-tok-b".to_string(),
                    issued_by: "test".to_string(),
                    control_center_trust_bundle: None,
                },
            )
            .expect("enroll");
        file_store
            .revoke_enrollment_token("gw-001", &token.token_id)
            .expect("revoke");
        let state = ApiState {
            config: crate::config::CenterConfig {
                listen_addr: "127.0.0.1:3100".to_string(),
                store_path: std::env::temp_dir().join(format!("wic-register-revoked-{nanos}.json")),
                gateway_credentials: Vec::new(),
                admin_token_hash: None,
                database_url: None,
                victoriametrics_url: None,
                public_url: "http://127.0.0.1:3100".to_string(),
                gateway_image: "warp-gateway:latest".to_string(),
                artifact_dir: std::env::temp_dir().join("wic-artifacts"),
                object_storage: None,
                ca_cert: None,
                protocol_version: "1.0".to_string(),
                hmac_secret: "test-hmac-secret".to_string(),
                credential_ttl_seconds: 3600,
            },
            store: std::sync::Arc::new(file_store),
            artifact_store: std::sync::Arc::new(crate::infra::LocalArtifactStore::new(
                std::env::temp_dir().join("wic-artifacts"),
                "http://127.0.0.1:3100",
            )),
            rate_limits: std::sync::Arc::new(std::sync::Mutex::new(
                super::super::rate_limit::RateLimitState::default(),
            )),
        };
        let app = super::super::router_for(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/register")
                    .header("content-type", "application/json")
                    .body(Body::from(register_payload("enroll-tok-b")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn query_initialization_status_tracks_lifecycle() {
        let state = provision_state("boot-tok");
        let store = state.store.clone();
        let app = super::super::router_for(state);
        // 未初始化：Provisioned → initialized = false。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/gateway/initialization-status?instance_id=gw-p")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let status: GatewayInitializationStatus = serde_json::from_slice(&body).expect("json");
        assert_eq!(status.gateway_id, "gw-p");
        assert_eq!(
            status.lifecycle_state,
            GatewayInstanceLifecycleState::Provisioned
        );
        assert!(!status.initialized);
        // initial-config 置备流程将 Center 侧生命周期推进到 Initializing。
        store
            .mark_gateway_initializing("gw-p")
            .await
            .expect("mark initializing");
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/gateway/initialization-status?instance_id=gw-p")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let status: GatewayInitializationStatus = serde_json::from_slice(&body).expect("json");
        assert_eq!(
            status.lifecycle_state,
            GatewayInstanceLifecycleState::Initializing
        );
        assert!(status.initialized);
    }
}

/// 校验网关通讯凭据（VerifyGatewayCredentialFlow）：网关持 bearer 访问，中心比对
/// 存储的 token hash（authenticate_gateway），通过即返回 valid 结果。
pub async fn verify_gateway_credential(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<VerifyGatewayCredential>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    match authenticate_gateway(&state, &headers, &input.gateway_id, &client_key).await {
        Ok(_) => Json(GatewayCredentialVerificationResult {
            gateway_id: input.gateway_id,
            credential_id: input.credential_id,
            status: "valid".to_string(),
            verified_at: DateTime::now(),
        })
        .into_response(),
        Err(response) => response,
    }
}

/// 查询网关初始化状态（QueryGatewayInitializationStatus）：GET /api/v1/gateway/initialization-status。
/// 供网关/前端初始化页判断是否已初始化：initialized = lifecycle_state != Provisioned；
/// lifecycle_state 未记录（None）时按未初始化（Provisioned）处理。
pub async fn query_gateway_initialization_status(
    State(state): State<ApiState>,
    Query(params): Query<QueryGatewayInitializationStatus>,
) -> Response {
    let Some(gateway_id) = params.instance_id.as_deref() else {
        return (StatusCode::BAD_REQUEST, "missing instance_id").into_response();
    };
    let gateway = match state.store.get_gateway(gateway_id).await {
        Ok(Some(gateway)) => gateway,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                format!("unknown gateway `{gateway_id}`"),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load gateway store: {err}"),
            )
                .into_response();
        }
    };
    let lifecycle_state = gateway
        .lifecycle_state
        .unwrap_or(GatewayInstanceLifecycleState::Provisioned);
    Json(GatewayInitializationStatus {
        gateway_id: gateway.gateway_id.clone(),
        instance_id: Some(gateway.instance_id.clone()),
        lifecycle_state,
        initialized: lifecycle_state != GatewayInstanceLifecycleState::Provisioned,
    })
    .into_response()
}
