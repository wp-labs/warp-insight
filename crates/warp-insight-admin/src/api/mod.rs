// @moju generated
// @moju hash=2ff63c2da808b5ca

pub mod warp_insight_admin_public_install_interface;
pub use warp_insight_admin_public_install_interface::*;
pub mod warp_insight_admin_management_interface;
pub use warp_insight_admin_management_interface::*;
pub mod wp_agent_online_registration_interface;
pub use wp_agent_online_registration_interface::*;

use axum::{
    extract::Path,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::domain::messages::{
    AdminAgentInstallCodeReturned, AdminAgentRuntimeStatusReturned,
    AdminPauseAgentDispatchReturned, AdminUpgradeAgentDispatchReturned,
};
use crate::domain::types::{
    AgentBootstrapBundle, AgentInstallCode, AgentRuntimeStatusView, DateTime, DispatchReceipt,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOverviewMetrics {
    pub total_agents: i64,
    pub online_agents: i64,
    pub unhealthy_agents: i64,
    pub last_seen_lag_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOverview {
    pub metrics: AgentOverviewMetrics,
    pub abnormal_agents: Vec<AgentRuntimeStatusView>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PauseAgentRequest {
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpgradeAgentRequest {
    pub requested_by: Option<String>,
    pub target_version: Option<String>,
}

pub fn router() -> Router {
    Router::new()
        .route("/api/v1/agent/install-code", get(get_agent_install_code))
        .route("/api/v1/admin/agents/overview", get(get_agent_overview))
        .route(
            "/api/v1/admin/agents/:agent_id/runtime-status",
            get(get_agent_runtime_status),
        )
        .route("/api/v1/admin/agents/:agent_id/pause", post(pause_agent))
        .route(
            "/api/v1/admin/agents/:agent_id/upgrade",
            post(upgrade_agent),
        )
}

async fn get_agent_install_code() -> Json<AdminAgentInstallCodeReturned> {
    Json(AdminAgentInstallCodeReturned {
        install_code: AgentInstallCode {
            x86_linux_install_code:
                "curl -fsSL http://127.0.0.1:3000/api/v1/agent/install/x86/install.sh | bash"
                    .to_string(),
            arm_linux_install_code:
                "curl -fsSL http://127.0.0.1:3000/api/v1/agent/install/arm/install.sh | bash"
                    .to_string(),
            bootstrap_bundle: AgentBootstrapBundle {
                bundle_id: "agent-bootstrap-default".to_string(),
                install_script_url: "http://127.0.0.1:3000/api/v1/agent/install/x86/install.sh"
                    .to_string(),
                agent_package_url: "http://127.0.0.1:3000/api/v1/agent/packages/current"
                    .to_string(),
                control_endpoint: "http://127.0.0.1:3000".to_string(),
                trust_bundle: "internal-ca-stub".to_string(),
                tenant_id: "tenant-default".to_string(),
                environment_id: "env-default".to_string(),
                expires_at: DateTime::now(),
            },
        },
    })
}

async fn get_agent_overview() -> Json<AgentOverview> {
    Json(AgentOverview {
        metrics: AgentOverviewMetrics {
            total_agents: 9,
            online_agents: 7,
            unhealthy_agents: 2,
            last_seen_lag_seconds: 42,
        },
        abnormal_agents: vec![
            runtime_status(
                "agent-prod-001",
                "i-0a12c9f8",
                "v0.3.1",
                "online",
                "degraded",
            ),
            runtime_status(
                "agent-edge-014",
                "edge-node-014",
                "v0.2.8",
                "offline",
                "unhealthy",
            ),
        ],
    })
}

async fn get_agent_runtime_status(
    Path(agent_id): Path<String>,
) -> Json<AdminAgentRuntimeStatusReturned> {
    Json(AdminAgentRuntimeStatusReturned {
        status: runtime_status(&agent_id, "i-stub-runtime", "v0.3.1", "online", "healthy"),
    })
}

async fn pause_agent(
    Path(agent_id): Path<String>,
    Json(input): Json<PauseAgentRequest>,
) -> (StatusCode, Json<AdminPauseAgentDispatchReturned>) {
    let requested_by = input
        .requested_by
        .unwrap_or_else(|| "admin-operator".to_string());
    (
        StatusCode::ACCEPTED,
        Json(AdminPauseAgentDispatchReturned {
            result: dispatch_receipt(&agent_id, "pause", &requested_by),
        }),
    )
}

async fn upgrade_agent(
    Path(agent_id): Path<String>,
    Json(input): Json<UpgradeAgentRequest>,
) -> (StatusCode, Json<AdminUpgradeAgentDispatchReturned>) {
    let requested_by = input
        .requested_by
        .unwrap_or_else(|| "admin-operator".to_string());
    let target_version = input.target_version.unwrap_or_else(|| "v0.3.2".to_string());
    (
        StatusCode::ACCEPTED,
        Json(AdminUpgradeAgentDispatchReturned {
            result: dispatch_receipt(
                &agent_id,
                &format!("upgrade-{target_version}"),
                &requested_by,
            ),
        }),
    )
}

fn runtime_status(
    agent_id: &str,
    instance_id: &str,
    version: &str,
    status: &str,
    health: &str,
) -> AgentRuntimeStatusView {
    AgentRuntimeStatusView {
        agent_id: agent_id.to_string(),
        instance_id: instance_id.to_string(),
        version: version.to_string(),
        status: status.to_string(),
        health: health.to_string(),
        last_seen_at: DateTime::now(),
    }
}

fn dispatch_receipt(agent_id: &str, kind: &str, requested_by: &str) -> DispatchReceipt {
    DispatchReceipt {
        agent_id: agent_id.to_string(),
        command_id: format!("admin-{kind}-command-{requested_by}"),
        dispatch_id: format!("admin-{kind}-dispatch"),
        status: "accepted".to_string(),
        created_at: DateTime::now(),
    }
}
