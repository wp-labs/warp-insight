// 管理面接口：网关创建 + 列表聚合 / 状态卡片列表 / 单网关状态。
// 数据来自 center store（ReceiveGatewayStatusReport 落库的最新状态）。

use std::net::SocketAddr;

use axum::{
    extract::{connect_info::ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use insight_control::types::DateTime;
use insight_control::{
    AdminGatewayCustomerBindingReturned, AdminGatewayListReturned, AdminGatewayStatusListReturned,
    AdminGatewayStatusReturned, AgentFleetDispatchReceipt, AgentRuntimeStatus,
    DispatchAgentFleetCommand, DispatchGlobalPolicy, GatewayCustomerBinding, GatewayInitialConfig,
    GatewayInitialConfigReturned, GatewayInstance, GatewayInstanceLifecycleState, GatewayListView,
    GatewayRuntimeStatus, GlobalPolicyDispatch, UpgradeStep, UpgradeTarget,
};

use crate::infra::{StoreReason, StoredGateway, UpgradePlanRecord};

use super::{
    admin_auth::require_admin_bearer, build_control_center_trust_bundle,
    control_center_tls_required, rate_limit, ApiState,
};

/// 创建网关实例请求体：对齐模型 `AdminCreateGatewayInstance`（gateway_name/requested_by）。
/// 可选 `token`：作为一次性置备引导 Token（BOOTSTRAP_TOKEN）显式提供；不传则由中心生成。
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminCreateGatewayInstanceRequest {
    pub gateway_name: String,
    pub requested_by: String,
    pub token: Option<String>,
}

/// 网关实例安装指引（api 层交付信息，不模型化）：docker 安装命令 + 云镜像地址 + 初始化 URL + 置备引导 Token。
#[derive(serde::Serialize, serde::Deserialize)]
pub struct GatewayInstallInfo {
    pub install_command: String,
    pub cloud_image: String,
    pub init_url: String,
    /// 一次性置备引导 Token（BOOTSTRAP_TOKEN）明文：仅 create/rotate 响应交付一次，
    /// 供 admin 页面展示给安装人员；中心只存 hash，初始化后不可再生成。
    pub setup_token: String,
    /// 控制中心 CA 证书内容（control-center.pem 信任根），供安装时写入 trust_bundle 路径。
    pub trust_bundle_pem: Option<String>,
    /// 服务端生成的 curl 验证命令：Bearer 用置备引导 token + 网关自生成身份调 init_url。
    pub init_curl: String,
}

/// 创建网关实例返回：实例视图 + 安装指引（Gateway 启动后基于 init_url 初始化）。
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AdminCreateGatewayInstanceReturned {
    pub instance: GatewayInstance,
    pub install: GatewayInstallInfo,
}

/// 管理端实例列表项：在生命周期信息之外公开不含凭证的初始化入口。
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AdminGatewayInstanceView {
    pub gateway_id: String,
    pub instance_id: String,
    pub lifecycle_state: GatewayInstanceLifecycleState,
    pub created_at: DateTime,
    pub initialized_at: Option<DateTime>,
    pub init_url: String,
}

/// 注册 Token 管理视图：只公开状态/限量/有效期，不暴露 token_hash。
/// 创建网关实例：POST /api/v1/admin/gateways/instances。
/// gateway_id 由 gateway_name 派生；重复 → 409。
pub async fn admin_create_gateway_instance(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(request): Json<AdminCreateGatewayInstanceRequest>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let gateway_id = request.gateway_name.trim();
    if gateway_id.is_empty() || request.requested_by.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            "gateway_name and requested_by must not be empty",
        )
            .into_response();
    }
    // 一次性置备引导 Token（BOOTSTRAP_TOKEN）：create 时生成、中心只存 sha256；
    // 明文随 create 响应交付（admin 页面一次性展示给安装人员），初始化后不可再生成。
    let bootstrap_token = match request.token.as_deref() {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => match crate::infra::new_secret_token("boot") {
            Ok(token) => token,
            Err(reason) => return (StatusCode::INTERNAL_SERVER_ERROR, reason).into_response(),
        },
    };
    match state
        .store
        .create_gateway(gateway_id, &bootstrap_token)
        .await
    {
        Ok(stored) => {
            let init_endpoint = format!(
                "{}/api/v1/gateway/initial-config?instance_id={}",
                state.config.public_url.trim_end_matches('/'),
                stored.gateway_id
            );
            // init_url 不携带凭证：bootstrap token 不进 URL（避免泄露到历史/分享链接），
            // 网关置备时以 Bearer bootstrap_token + X-Gateway-Identity-Token 头调用。
            let init_url = init_endpoint.clone();
            // 配置在网关调 init-url 置备时生成（ProvisionGatewayFlow 派生 RegistToken 出 config.toml），
            // create 不再产出配置文件。
            // 信任根：config.toml 引用 /etc/warp-gateway/ca/control-center.pem，需把
            // control-center.pem（trust_bundle_pem 内容落盘为 ./control-center.pem）挂载到该路径，
            // 网关访问 HTTPS init_url 时才能校验中心 TLS 服务器证书。未配置 CA → 不挂载（无 TLS 回退）。
            let trust_bundle_mount = if state.config.ca_cert.is_some() {
                " -v ./control-center.pem:/etc/warp-gateway/ca/control-center.pem:ro".to_string()
            } else {
                String::new()
            };
            // 服务端生成 curl 验证命令：Bearer 用置备引导 token + 网关自生成身份调 init_url。
            let init_curl = format!(
                "curl -H \"Authorization: Bearer {bootstrap_token}\" -H \"X-Gateway-Identity-Token: <gateway-identity>\" \"{init_endpoint}\""
            );
            let install = GatewayInstallInfo {
                install_command: format!(
                    "docker run -d --name warp-gateway-{gw} -e WARP_GATEWAY_INIT_URL={url} -e WARP_GATEWAY_BOOTSTRAP_TOKEN={token}{trust_mount} {image}",
                    gw = stored.gateway_id,
                    url = init_url,
                    token = bootstrap_token,
                    trust_mount = trust_bundle_mount,
                    image = state.config.gateway_image
                ),
                cloud_image: state.config.gateway_image.clone(),
                init_url,
                setup_token: bootstrap_token,
                trust_bundle_pem: state.config.ca_cert.clone(),
                init_curl,
            };
            (
                StatusCode::CREATED,
                Json(AdminCreateGatewayInstanceReturned {
                    instance: GatewayInstance {
                        gateway_id: stored.gateway_id,
                        instance_id: stored.instance_id,
                        lifecycle_state: GatewayInstanceLifecycleState::Provisioned,
                        created_at: DateTime::now(),
                        initialized_at: None,
                    },
                    install,
                }),
            )
                .into_response()
        }
        Err(err) if err.reason() == &StoreReason::Conflict => (
            StatusCode::CONFLICT,
            format!("gateway {gateway_id} already exists"),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create gateway: {err}"),
        )
            .into_response(),
    }
}

/// 网关在线率查询参数（默认 1h 窗口）。
#[derive(serde::Deserialize)]
pub struct GatewayUptimeQueryParams {
    pub window: Option<String>,
}

/// 网关在线率返回（0..1；无历史数据或 VM 不可达 → None）。
#[derive(serde::Serialize)]
pub struct GatewayUptimeReturned {
    pub gateway_id: String,
    pub window: String,
    pub uptime: Option<f64>,
}

/// 网关历史查询参数；当前控制台使用 1h，保留 6h/24h 供后续切换。
#[derive(serde::Deserialize)]
pub struct GatewayHistoryQueryParams {
    pub window: Option<String>,
}

/// 网关历史接口返回，采样来自 VictoriaMetrics range query。
#[derive(serde::Serialize)]
pub struct GatewayHistoryReturned {
    pub gateway_id: String,
    pub window: String,
    pub step_seconds: i64,
    pub samples: Vec<crate::infra::vm::GatewayMetricSample>,
}

/// 单个 Agent 历史接口返回。
#[derive(serde::Serialize)]
pub struct AgentHistoryReturned {
    pub gateway_id: String,
    pub agent_id: String,
    pub window: String,
    pub step_seconds: i64,
    pub samples: Vec<crate::infra::vm::AgentMetricSample>,
}

/// 查询网关在线率：GET /api/v1/admin/gateways/:gateway_id/status/uptime。
/// 转发 VM `avg_over_time(gateway_up{gateway_id="X"}[window])`；
/// VM 未配置 / 查询失败 / 无历史样本 → uptime None（HTTP 200，列表页平滑显示"—"）。
pub async fn admin_get_gateway_uptime(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Path(gateway_id): Path<String>,
    Query(params): Query<GatewayUptimeQueryParams>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let window = params.window.as_deref().unwrap_or("1h");
    let uptime = match &state.config.victoriametrics_url {
        Some(vm_url) => {
            match crate::infra::vm::query_uptime(
                crate::infra::vm::shared_vm_client(),
                vm_url,
                &gateway_id,
                window,
            )
            .await
            {
                Ok(uptime) => uptime,
                Err(err) => {
                    eprintln!("warn gateway uptime vm query failed: {err}");
                    None
                }
            }
        }
        None => None,
    };
    Json(GatewayUptimeReturned {
        gateway_id,
        window: window.to_string(),
        uptime,
    })
    .into_response()
}

/// 查询网关最近一段时间的在线、内存和 CPU 历史。
///
/// VM 未配置或暂时不可用时返回空 samples，页面保留快照并显示历史空态。
pub async fn admin_get_gateway_history(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Path(gateway_id): Path<String>,
    Query(params): Query<GatewayHistoryQueryParams>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let window = params.window.as_deref().unwrap_or("1h");
    let Some((window_seconds, step_seconds)) = history_window_config(window) else {
        return (
            StatusCode::BAD_REQUEST,
            "window must be one of: 1h, 6h, 24h",
        )
            .into_response();
    };
    let now = chrono::Utc::now().timestamp();
    let end = now - now.rem_euclid(step_seconds);
    let start = end - window_seconds;
    let samples = match &state.config.victoriametrics_url {
        Some(vm_url) => match crate::infra::vm::query_gateway_history(
            crate::infra::vm::shared_vm_client(),
            vm_url,
            &gateway_id,
            start,
            end,
            step_seconds,
        )
        .await
        {
            Ok(samples) => samples,
            Err(err) => {
                eprintln!("warn gateway history vm query failed: {err}");
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    Json(GatewayHistoryReturned {
        gateway_id,
        window: window.to_string(),
        step_seconds,
        samples,
    })
    .into_response()
}

/// 查询单个 Agent 最近一段时间的在线、内存、CPU 和管理时延历史。
pub async fn admin_get_agent_history(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Path((gateway_id, agent_id)): Path<(String, String)>,
    Query(params): Query<GatewayHistoryQueryParams>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let window = params.window.as_deref().unwrap_or("1h");
    let Some((window_seconds, step_seconds)) = history_window_config(window) else {
        return (
            StatusCode::BAD_REQUEST,
            "window must be one of: 1h, 6h, 24h",
        )
            .into_response();
    };
    let now = chrono::Utc::now().timestamp();
    let end = now - now.rem_euclid(step_seconds);
    let start = end - window_seconds;
    let samples = match &state.config.victoriametrics_url {
        Some(vm_url) => match crate::infra::vm::query_agent_history(
            crate::infra::vm::shared_vm_client(),
            vm_url,
            &gateway_id,
            &agent_id,
            start,
            end,
            step_seconds,
        )
        .await
        {
            Ok(samples) => samples,
            Err(err) => {
                eprintln!("warn agent history vm query failed: {err}");
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    Json(AgentHistoryReturned {
        gateway_id,
        agent_id,
        window: window.to_string(),
        step_seconds,
        samples,
    })
    .into_response()
}

/// 将公开窗口收敛为固定秒数和采样步长，避免把任意字符串带入 PromQL。
fn history_window_config(window: &str) -> Option<(i64, i64)> {
    match window {
        "1h" => Some((60 * 60, 60)),
        "6h" => Some((6 * 60 * 60, 5 * 60)),
        "24h" => Some((24 * 60 * 60, 15 * 60)),
        _ => None,
    }
}

/// TOML 基础字符串转义（镜像 warp-gateway 的 toml_escape）：
/// 保证任意 token / 端点字符串嵌入 config.toml 双引号字符串后仍是合法 TOML。
#[cfg(test)]
fn toml_basic_string_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04X}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}

/// 对 URL fragment 值做百分号编码，确保任意凭证不会截断或改写初始化 URL。
/// 实例列表：GET /api/v1/admin/gateways/instances（含生命周期状态）。
pub async fn admin_list_gateway_instances(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let gateways = match state.store.list_gateways().await {
        Ok(gateways) => gateways,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load gateway instances: {err}"),
            )
                .into_response();
        }
    };
    let instances: Vec<AdminGatewayInstanceView> = gateways
        .into_iter()
        .map(|gateway| {
            let init_url = format!(
                "{}/api/v1/gateway/initial-config?instance_id={}",
                state.config.public_url.trim_end_matches('/'),
                gateway.gateway_id
            );
            AdminGatewayInstanceView {
                gateway_id: gateway.gateway_id,
                instance_id: gateway.instance_id,
                lifecycle_state: gateway
                    .lifecycle_state
                    .unwrap_or(GatewayInstanceLifecycleState::Provisioned),
                created_at: gateway.created_at.unwrap_or_else(DateTime::now),
                initialized_at: gateway.initialized_at,
                init_url,
            }
        })
        .collect();
    Json(instances).into_response()
}

/// 版本发布请求体：version + 外部 artifact_url（可能 GitHub/制品库，中心会下载镜像到本地/对象存储）。
#[derive(serde::Deserialize)]
pub struct PublishReleaseRequest {
    pub version: String,
    pub artifact_url: String,
    pub requested_by: String,
}

fn artifact_filename(url: &str, component: &str, version: &str) -> String {
    url.split('/')
        .next_back()
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{component}-{version}.bin"))
}

async fn download_artifact(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| format!("download artifact failed: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "download artifact rejected: HTTP {}",
            response.status()
        ));
    }
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|err| format!("read artifact body failed: {err}"))
}

/// 发布版本：POST /api/v1/admin/releases/:component（wist-agentd / warp-gateway）。
/// 从外部 artifact_url 下载制品 → 镜像到本地文件/对象存储 → 返回快的下载地址。
pub async fn admin_publish_release(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Path(component): Path<String>,
    Json(request): Json<PublishReleaseRequest>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    if request.version.trim().is_empty()
        || request.artifact_url.trim().is_empty()
        || request.requested_by.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            "version, artifact_url and requested_by must not be empty",
        )
            .into_response();
    }
    let bytes = match download_artifact(crate::infra::vm::shared_vm_client(), &request.artifact_url)
        .await
    {
        Ok(bytes) => bytes,
        Err(err) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("failed to fetch artifact: {err}"),
            )
                .into_response();
        }
    };
    let filename = artifact_filename(&request.artifact_url, &component, &request.version);
    let mirrored_url = match state
        .artifact_store
        .store(&component, &request.version, &filename, bytes)
        .await
    {
        Ok(url) => url,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to store artifact: {err}"),
            )
                .into_response();
        }
    };
    let record = match state
        .store
        .publish_release(&component, &request.version, &mirrored_url)
        .await
    {
        Ok(record) => record,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to record release: {err}"),
            )
                .into_response();
        }
    };
    Json(record).into_response()
}

/// 查询某组件的发布记录：GET /api/v1/admin/releases/:component（新→旧）。
pub async fn admin_list_releases(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Path(component): Path<String>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let releases = match state.store.list_releases(&component).await {
        Ok(releases) => releases,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load releases: {err}"),
            )
                .into_response();
        }
    };
    Json(releases).into_response()
}

/// 绑定客户请求体：对齐模型 AdminBindGatewayCustomer。
#[derive(serde::Deserialize)]
pub struct BindGatewayCustomerRequest {
    pub gateway_id: String,
    pub customer_id: String,
    pub requested_by: String,
}

/// 绑定网关到客户：POST /api/v1/admin/gateways/bind。
pub async fn admin_bind_gateway_customer(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(request): Json<BindGatewayCustomerRequest>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    if request.gateway_id.trim().is_empty()
        || request.customer_id.trim().is_empty()
        || request.requested_by.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            "gateway_id, customer_id and requested_by must not be empty",
        )
            .into_response();
    }
    let binding = match state
        .store
        .bind_gateway_customer(&request.gateway_id, &request.customer_id)
        .await
    {
        Ok(binding) => binding,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to bind gateway customer: {err}"),
            )
                .into_response();
        }
    };
    Json(AdminGatewayCustomerBindingReturned {
        binding: GatewayCustomerBinding {
            gateway_id: binding.gateway_id,
            customer_id: binding.customer_id,
            status: binding.status,
            bound_at: binding.bound_at,
        },
    })
    .into_response()
}

/// 查询实例初始配置（admin 侧）：GET /api/v1/admin/gateways/instances/:instance_id/config。
pub async fn admin_get_gateway_initial_config(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Path(instance_id): Path<String>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    // 配置引用该网关最近签发的一个注册 Token（config.toml [enrollment] token_id）。
    let enrollment_token_id = state
        .store
        .get_enrollment_token_for_gateway(&instance_id)
        .await
        .ok()
        .flatten()
        .map(|token| token.token_id)
        .unwrap_or_default();
    let server_tls_required = control_center_tls_required(&state.config);
    let trust_bundle = build_control_center_trust_bundle(&state.config, &instance_id);
    // 安全 #1（fail-closed）：TLS 开启但未配置信任根 → 拒绝服务。
    if server_tls_required && trust_bundle.is_none() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "TLS is required but the control center trust root is not configured",
        )
            .into_response();
    }
    Json(GatewayInitialConfigReturned {
        config: GatewayInitialConfig {
            gateway_id: instance_id.clone(),
            control_center_endpoint: state.config.public_url.clone(),
            trust_bundle,
            server_tls_required,
            protocol_version: state.config.protocol_version.clone(),
            enrollment_token_id,
        },
    })
    .into_response()
}

/// 创建升级计划请求体：多组件目标版本 + 网关范围 + 多步执行。
#[derive(serde::Deserialize)]
pub struct CreateUpgradePlanRequest {
    pub targets: Vec<UpgradeTarget>,
    pub gateway_ids: Vec<String>,
    pub steps: Vec<UpgradeStep>,
    pub requested_by: String,
}

#[derive(serde::Deserialize)]
pub struct ApproveUpgradePlanRequest {
    pub plan_id: String,
    pub approved_by: String,
}

/// 创建升级计划：POST /api/v1/admin/upgrade-plans。
pub async fn admin_create_upgrade_plan(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(request): Json<CreateUpgradePlanRequest>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    if request.targets.is_empty()
        || request.gateway_ids.is_empty()
        || request.requested_by.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            "targets, gateway_ids and requested_by must not be empty",
        )
            .into_response();
    }
    let plan_id = format!(
        "plan-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let record = UpgradePlanRecord {
        plan_id,
        targets: request.targets,
        target_count: request.gateway_ids.len() as i64,
        status: "pending".to_string(),
        created_at: DateTime::now(),
        steps: request.steps,
        approved_by: None,
        approved_at: None,
    };
    match state.store.create_upgrade_plan(&record).await {
        Ok(plan) => (StatusCode::CREATED, Json(plan)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create upgrade plan: {err}"),
        )
            .into_response(),
    }
}

/// 查询升级计划列表：GET /api/v1/admin/upgrade-plans。
pub async fn admin_list_upgrade_plans(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    match state.store.list_upgrade_plans().await {
        Ok(plans) => Json(plans).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load upgrade plans: {err}"),
        )
            .into_response(),
    }
}

/// 批准升级计划：POST /api/v1/admin/upgrade-plans/approve。
pub async fn admin_approve_upgrade_plan(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(request): Json<ApproveUpgradePlanRequest>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    match state
        .store
        .approve_upgrade_plan(&request.plan_id, &request.approved_by)
        .await
    {
        Ok(plan) => Json(plan).into_response(),
        Err(err) if err.reason() == &StoreReason::Conflict => (
            StatusCode::NOT_FOUND,
            format!("upgrade plan {} not found", request.plan_id),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to approve upgrade plan: {err}"),
        )
            .into_response(),
    }
}

/// 查询某 gateway 的生命周期转变历史：GET /api/v1/admin/gateways/:gateway_id/lifecycle。
pub async fn admin_list_gateway_lifecycle(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Path(gateway_id): Path<String>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let events = match state.store.list_lifecycle_events(&gateway_id).await {
        Ok(events) => events,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load gateway lifecycle: {err}"),
            )
                .into_response();
        }
    };
    Json(events).into_response()
}

/// 查询某 gateway 下的 Agent 状态：GET /api/v1/admin/gateways/:gateway_id/agents。
pub async fn admin_list_gateway_agents(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Path(gateway_id): Path<String>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let agents = match state.store.list_agents_by_gateway(&gateway_id).await {
        Ok(agents) => agents,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load gateway agents: {err}"),
            )
                .into_response();
        }
    };
    let views: Vec<AgentRuntimeStatus> = agents
        .into_iter()
        .map(|agent| AgentRuntimeStatus {
            agent_id: agent.agent_id,
            instance_id: agent.instance_id,
            version: agent.version,
            status: agent.status,
            health: agent.health,
            memory_bytes: agent.memory_bytes,
            cpu_percent: agent.cpu_percent,
            admin_latency_ms: agent.admin_latency_ms,
            last_seen_at: agent.last_seen_at,
        })
        .collect();
    Json(views).into_response()
}

pub async fn admin_view_gateway_list(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let gateways = match state.store.list_gateways().await {
        Ok(gateways) => gateways,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load gateway store: {err}"),
            )
                .into_response();
        }
    };
    let (online_count, offline_count, degraded_count) = gateways.iter().fold(
        (0_i64, 0_i64, 0_i64),
        |(online, offline, degraded), stored| {
            let online = online + i64::from(stored.status.as_deref() == Some("online"));
            // 离线 = 不在线（含已上报离线与从未上报的已接入网关），保证 online + offline = gateway_count。
            let offline = offline + i64::from(stored.status.as_deref() != Some("online"));
            let degraded = degraded + i64::from(stored.health.as_deref() == Some("degraded"));
            (online, offline, degraded)
        },
    );
    Json(AdminGatewayListReturned {
        list: GatewayListView {
            gateway_count: gateways.len() as i64,
            online_count,
            offline_count,
            degraded_count,
            updated_at: DateTime::now(),
        },
    })
    .into_response()
}

pub async fn admin_list_gateway_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let gateways = match state.store.list_gateways().await {
        Ok(gateways) => gateways,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load gateway store: {err}"),
            )
                .into_response();
        }
    };
    // 状态视图只展示「已上报过」的网关；从未上报的网关无状态可展示（聚合 gateway_count 仍计入）。
    let mut views: Vec<GatewayRuntimeStatus> = gateways
        .iter()
        .filter(|stored| stored.last_seen_at.is_some())
        .map(gateway_runtime_status)
        .collect();
    views.sort_by(|left, right| left.gateway_id.cmp(&right.gateway_id));
    Json(AdminGatewayStatusListReturned { statuses: views }).into_response()
}

pub async fn admin_show_gateway_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Path(gateway_id): Path<String>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let stored = match state.store.get_gateway(&gateway_id).await {
        Ok(stored) => stored,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load gateway store: {err}"),
            )
                .into_response();
        }
    };
    let Some(stored) = stored else {
        return (
            StatusCode::NOT_FOUND,
            format!("unknown gateway {gateway_id}"),
        )
            .into_response();
    };
    if stored.last_seen_at.is_none() {
        // 已接入但从未上报：无状态可返回（与列表端点一致）。
        return (
            StatusCode::NOT_FOUND,
            format!("gateway {gateway_id} has not reported yet"),
        )
            .into_response();
    }
    Json(AdminGatewayStatusReturned {
        status: gateway_runtime_status(&stored),
    })
    .into_response()
}

/// StoredGateway → GatewayRuntimeStatus。仅对已上报网关调用（列表/单查已过滤）；
/// Option 缺省值保留为防御性兜底。
fn gateway_runtime_status(stored: &StoredGateway) -> GatewayRuntimeStatus {
    GatewayRuntimeStatus {
        gateway_id: stored.gateway_id.clone(),
        instance_id: stored.instance_id.clone(),
        version: stored.version.clone().unwrap_or_default(),
        status: stored
            .status
            .clone()
            .unwrap_or_else(|| "offline".to_string()),
        health: stored
            .health
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        memory_bytes: stored.memory_bytes,
        cpu_percent: stored.cpu_percent,
        last_seen_at: stored.last_seen_at.clone().unwrap_or_else(DateTime::now),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{
        config::{CenterConfig, GatewayCredentialSeed},
        infra::FileStore,
    };

    fn test_store() -> FileStore {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wic-admin-test-{nanos}.json"));
        let store = FileStore::new(&path);
        store
            .seed(&[
                GatewayCredentialSeed {
                    gateway_id: "gw-001".to_string(),
                    token: "tok-a".to_string(),
                    expires_at: None,
                },
                GatewayCredentialSeed {
                    gateway_id: "gw-002".to_string(),
                    token: "tok-b".to_string(),
                    expires_at: None,
                },
            ])
            .expect("seed");
        store
            .update(|snapshot| {
                if let Some(gw) = snapshot.gateways.get_mut("gw-001") {
                    gw.instance_id = "inst-1".to_string();
                    gw.version = Some("v2.4.1".to_string());
                    gw.status = Some("online".to_string());
                    gw.health = Some("healthy".to_string());
                    gw.last_seen_at = Some(DateTime::now());
                }
                if let Some(gw) = snapshot.gateways.get_mut("gw-002") {
                    gw.status = Some("offline".to_string());
                    gw.health = Some("degraded".to_string());
                    gw.last_seen_at = Some(DateTime::now());
                }
            })
            .expect("status update");
        store
    }

    fn test_state() -> ApiState {
        ApiState {
            config: CenterConfig {
                listen_addr: "127.0.0.1:3100".to_string(),
                store_path: std::env::temp_dir().join("unused.json"),
                gateway_credentials: Vec::new(),
                admin_token_hash: Some(super::super::super::infra::sha256_hex("admin-tok")),
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
            store: std::sync::Arc::new(test_store()),
            artifact_store: std::sync::Arc::new(crate::infra::LocalArtifactStore::new(
                std::env::temp_dir().join("wic-artifacts"),
                "http://127.0.0.1:3100",
            )),
            rate_limits: std::sync::Arc::new(std::sync::Mutex::new(
                super::super::rate_limit::RateLimitState::default(),
            )),
        }
    }

    #[test]
    fn history_windows_use_bounded_steps() {
        assert_eq!(history_window_config("1h"), Some((3600, 60)));
        assert_eq!(history_window_config("6h"), Some((21_600, 300)));
        assert_eq!(history_window_config("24h"), Some((86_400, 900)));
        assert_eq!(history_window_config("7d"), None);
    }

    #[test]
    fn toml_basic_string_escape_handles_special_chars() {
        // 普通 token（base64url/hex）无需转义。
        assert_eq!(toml_basic_string_escape("g7Q3abc_-XYZ"), "g7Q3abc_-XYZ");
        // 引号 / 反斜杠 / 换行 / 控制字符需转义，保证仍是合法 TOML 基础字符串。
        assert_eq!(
            toml_basic_string_escape("a\"b\\c\nd\te\u{0001}f"),
            "a\\\"b\\\\c\\nd\\te\\u0001f"
        );
    }

    #[tokio::test]
    async fn history_without_vm_returns_empty_samples() {
        use axum::{body::Body, http::Request};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let app = super::super::router_for(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/gateways/gw-001/status/history?window=1h")
                    .header("authorization", "Bearer admin-tok")
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
        let returned: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(returned["gateway_id"], "gw-001");
        assert_eq!(returned["window"], "1h");
        assert_eq!(returned["step_seconds"], 60);
        assert_eq!(returned["samples"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn agent_history_without_vm_returns_empty_samples() {
        use axum::{body::Body, http::Request};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let app = super::super::router_for(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/gateways/gw-001/agents/agent-1/history?window=1h")
                    .header("authorization", "Bearer admin-tok")
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
        let returned: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(returned["gateway_id"], "gw-001");
        assert_eq!(returned["agent_id"], "agent-1");
        assert_eq!(returned["samples"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn never_reported_gateway_is_offline_not_listed() {
        use axum::{body::Body, http::Request};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wic-never-{nanos}.json"));
        let store = FileStore::new(&path);
        store
            .seed(&[
                GatewayCredentialSeed {
                    gateway_id: "gw-001".to_string(),
                    token: "tok-a".to_string(),
                    expires_at: None,
                },
                // gw-002 从未上报
                GatewayCredentialSeed {
                    gateway_id: "gw-002".to_string(),
                    token: "tok-b".to_string(),
                    expires_at: None,
                },
            ])
            .expect("seed");
        store
            .update(|snapshot| {
                if let Some(gw) = snapshot.gateways.get_mut("gw-001") {
                    gw.status = Some("online".to_string());
                    gw.health = Some("healthy".to_string());
                    gw.last_seen_at = Some(DateTime::now());
                }
            })
            .expect("update");
        let state = ApiState {
            config: CenterConfig {
                listen_addr: "127.0.0.1:3100".to_string(),
                store_path: std::env::temp_dir().join(format!("wic-never-{nanos}.json")),
                gateway_credentials: Vec::new(),
                admin_token_hash: Some(super::super::super::infra::sha256_hex("admin-tok")),
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
        };
        let app = super::super::router_for(state);

        // 聚合：gw-002（未上报）计入 offline，online + offline = gateway_count。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/gateways")
                    .header("authorization", "Bearer admin-tok")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let list: AdminGatewayListReturned = serde_json::from_slice(&body).expect("json");
        assert_eq!(list.list.gateway_count, 2);
        assert_eq!(list.list.online_count, 1);
        assert_eq!(list.list.offline_count, 1);

        // 列表：只含已上报的 gw-001。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/gateways/status")
                    .header("authorization", "Bearer admin-tok")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let list: AdminGatewayStatusListReturned = serde_json::from_slice(&body).expect("json");
        assert_eq!(list.statuses.len(), 1);
        assert_eq!(list.statuses[0].gateway_id, "gw-001");

        // 单查：未上报的 gw-002 → 404。
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/gateways/gw-002/status")
                    .header("authorization", "Bearer admin-tok")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_aggregates_from_store() {
        use axum::{body::Body, http::Request};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let state = test_state();
        let app = super::super::router_for(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/gateways")
                    .header("authorization", "Bearer admin-tok")
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
        let returned: AdminGatewayListReturned = serde_json::from_slice(&body).expect("json");
        assert_eq!(returned.list.gateway_count, 2);
        assert_eq!(returned.list.online_count, 1);
        assert_eq!(returned.list.offline_count, 1);
        assert_eq!(returned.list.degraded_count, 1);
    }

    #[tokio::test]
    async fn admin_auth_failures_are_rate_limited() {
        use axum::{body::Body, http::Request};
        use tower::ServiceExt;

        let state = test_state();
        let app = super::super::router_for(state);
        for _ in 0..5 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v1/admin/gateways/status")
                        .header("authorization", "Bearer wrong-token")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        // 第 6 次即使带正确 token 也被限流（测试环境共享 "unknown" 桶）。
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/gateways/status")
                    .header("authorization", "Bearer admin-tok")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn removed_list_alias_route_is_gone() {
        use axum::{body::Body, http::Request};
        use tower::ServiceExt;

        let state = test_state();
        let app = super::super::router_for(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/gateways/list")
                    .header("authorization", "Bearer admin-tok")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    fn create_state_with_store(store: FileStore) -> ApiState {
        ApiState {
            config: CenterConfig {
                listen_addr: "127.0.0.1:3100".to_string(),
                store_path: std::env::temp_dir().join("unused.json"),
                gateway_credentials: Vec::new(),
                admin_token_hash: Some(super::super::super::infra::sha256_hex("admin-tok")),
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

    fn create_payload(gateway_name: &str) -> String {
        format!(r#"{{"gateway_name":"{gateway_name}","requested_by":"test","token":"tok-create"}}"#)
    }

    #[tokio::test]
    async fn create_gateway_instance_creates_and_conflicts() {
        use axum::{body::Body, http::Request};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wic-create-{nanos}.json"));
        let state = create_state_with_store(FileStore::new(&path));
        let app = super::super::router_for(state);

        // 创建 → 201 + instance。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/admin/gateways/instances")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer admin-tok")
                    .body(Body::from(create_payload("gw-create")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let returned: AdminCreateGatewayInstanceReturned =
            serde_json::from_slice(&body).expect("json");
        assert_eq!(returned.instance.gateway_id, "gw-create");
        assert_eq!(
            returned.instance.lifecycle_state,
            GatewayInstanceLifecycleState::Provisioned
        );
        assert!(returned.install.init_url.contains("initial-config"));
        assert!(returned.install.install_command.starts_with("docker run"));
        assert_eq!(
            returned.install.init_curl,
            "curl -H \"Authorization: Bearer tok-create\" -H \"X-Gateway-Identity-Token: <gateway-identity>\" \"http://127.0.0.1:3100/api/v1/gateway/initial-config?instance_id=gw-create\""
        );
        assert_eq!(
            returned.install.init_url,
            "http://127.0.0.1:3100/api/v1/gateway/initial-config?instance_id=gw-create"
        );

        // 实例列表公开可重复获取的初始化 URL，但不重复返回安装命令或凭证。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/gateways/instances")
                    .header("authorization", "Bearer admin-tok")
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
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
        let listed = json
            .as_array()
            .and_then(|items| items.iter().find(|item| item["gateway_id"] == "gw-create"))
            .expect("created instance json");
        assert!(listed.get("init_url").is_some());
        assert!(listed.get("install").is_none());
        assert!(listed.get("install_command").is_none());
        assert!(listed.get("token").is_none());
        let instances: Vec<AdminGatewayInstanceView> = serde_json::from_slice(&body).expect("json");
        let created = instances
            .iter()
            .find(|instance| instance.gateway_id == "gw-create")
            .expect("created instance");
        assert_eq!(
            created.init_url,
            "http://127.0.0.1:3100/api/v1/gateway/initial-config?instance_id=gw-create"
        );

        // 重复创建同一 gateway_id → 409。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/admin/gateways/instances")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer admin-tok")
                    .body(Body::from(create_payload("gw-create")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);

        // 引导 token 不是运行期凭据：create 后立即用它调 /status → 401
        // （运行期凭据须在 /register 消费 RegistToken 后签发，见 gateway_ops register 测试）。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/gateway/status")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer tok-create")
                    .body(Body::from(
                        r#"{"gateway_id":"gw-create","instance_id":"inst-1","version":"v2.4.1","status":"online","health":"healthy","reported_at":"2026-08-08T12:00:00Z"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn create_gateway_instance_rejects_empty_name() {
        use axum::{body::Body, http::Request};
        use tower::ServiceExt;

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wic-create-empty-{nanos}.json"));
        let state = create_state_with_store(FileStore::new(&path));
        let app = super::super::router_for(state);

        for payload in [
            r#"{"gateway_name":"  ","requested_by":"test","token":"tok"}"#,
            r#"{"gateway_name":"gw-x","requested_by":"  ","token":"tok"}"#,
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/admin/gateways/instances")
                        .header("content-type", "application/json")
                        .header("authorization", "Bearer admin-tok")
                        .body(Body::from(payload))
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn create_gateway_instance_mounts_trust_bundle_when_ca_configured() {
        use axum::{body::Body, http::Request};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wic-create-tb-{nanos}.json"));
        let store = FileStore::new(&path);
        let state = ApiState {
            config: CenterConfig {
                listen_addr: "127.0.0.1:3100".to_string(),
                store_path: std::env::temp_dir().join(format!("wic-create-tb-{nanos}.json")),
                gateway_credentials: Vec::new(),
                admin_token_hash: Some(super::super::super::infra::sha256_hex("admin-tok")),
                database_url: None,
                victoriametrics_url: None,
                public_url: "http://127.0.0.1:3100".to_string(),
                gateway_image: "warp-gateway:latest".to_string(),
                artifact_dir: std::env::temp_dir().join("wic-artifacts"),
                object_storage: None,
                ca_cert: Some(
                    "-----BEGIN CERTIFICATE-----\nca\n-----END CERTIFICATE-----".to_string(),
                ),
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
        };
        let app = super::super::router_for(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/admin/gateways/instances")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer admin-tok")
                    .body(Body::from(create_payload("gw-tb")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let returned: AdminCreateGatewayInstanceReturned =
            serde_json::from_slice(&body).expect("json");
        // 证书内容随响应交付 + 安装命令挂载到 config.toml 引用的 trust_bundle 路径。
        assert!(returned.install.trust_bundle_pem.is_some());
        assert!(returned
            .install
            .install_command
            .contains("-v ./control-center.pem:/etc/warp-gateway/ca/control-center.pem:ro"));
        // 置备引导 Token 随 create 响应交付一次（admin 页面展示）；config.toml 不再在 create 生成。
        assert!(!returned.install.setup_token.is_empty());

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn read_endpoints_require_admin_token() {
        use axum::{body::Body, http::Request};
        use tower::ServiceExt;

        let state = test_state();
        let app = super::super::router_for(state);
        for uri in [
            "/api/v1/admin/gateways",
            "/api/v1/admin/gateways/status",
            "/api/v1/admin/gateways/gw-001/status",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "uri {uri} should require admin token"
            );
        }
    }
}

/// 下发全局策略（DispatchGlobalPolicyFlow）：记录一次全局策略下发回执。
pub async fn admin_dispatch_global_policy(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<DispatchGlobalPolicy>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    Json(GlobalPolicyDispatch {
        dispatch_id: format!(
            "policy-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ),
        policy_version: input.policy_version,
        target_count: input.gateway_ids.len() as i64,
        status: "accepted".to_string(),
        dispatched_at: DateTime::now(),
    })
    .into_response()
}

/// 下发 Agent 舰队指令（DispatchAgentFleetCommandFlow）：记录一次舰队指令下发回执。
pub async fn admin_dispatch_agent_fleet_command(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<DispatchAgentFleetCommand>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    Json(AgentFleetDispatchReceipt {
        dispatch_id: format!(
            "fleet-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ),
        command_kind: input.command_kind,
        target_count: input.agent_ids.len() as i64,
        status: "accepted".to_string(),
        created_at: DateTime::now(),
    })
    .into_response()
}
