use std::net::SocketAddr;

use axum::{
    extract::{connect_info::ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::domain::messages::{
    AdminAgentRuntimeStatusReturned, AdminPauseAgentDispatchReturned,
    AdminUpgradeAgentDispatchReturned,
};
use crate::domain::types::{AgentRuntimeStatusView, DateTime, DispatchReceipt};

use super::{admin_auth::require_admin_bearer, rate_limit, ApiState};

#[derive(Debug, Clone, Deserialize)]
pub struct PauseAgentRequest {
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpgradeAgentRequest {
    pub requested_by: Option<String>,
    pub target_version: Option<String>,
}

pub async fn get_agent_runtime_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    client: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    let snapshot = match state.store.load() {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load agent store: {err}"),
            )
                .into_response();
        }
    };
    let Some(agent) = snapshot.agents.get(&agent_id).cloned() else {
        return (
            StatusCode::NOT_FOUND,
            format!("unknown agent {agent_id}"),
        )
            .into_response();
    };
    Json(AdminAgentRuntimeStatusReturned {
        status: runtime_status(
            &agent.agent_id,
            &agent.instance_id,
            &agent.version,
            "online",
            "healthy",
        ),
    })
    .into_response()
}

pub async fn pause_agent(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<PauseAgentRequest>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    if let Some(response) = agent_not_found_response(&state, &agent_id) {
        return response;
    }
    let requested_by = input
        .requested_by
        .unwrap_or_else(|| "admin-operator".to_string());
    (
        StatusCode::ACCEPTED,
        Json(AdminPauseAgentDispatchReturned {
            result: dispatch_receipt(&agent_id, "pause", &requested_by),
        }),
    )
        .into_response()
}

pub async fn upgrade_agent(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<UpgradeAgentRequest>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    if let Some(response) = agent_not_found_response(&state, &agent_id) {
        return response;
    }
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
        .into_response()
}

fn agent_not_found_response(state: &ApiState, agent_id: &str) -> Option<Response> {
    match state.store.load() {
        Ok(snapshot) if snapshot.agents.contains_key(agent_id) => None,
        Ok(_) => Some(
            (
                StatusCode::NOT_FOUND,
                format!("unknown agent {agent_id}"),
            )
                .into_response(),
        ),
        Err(err) => Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load agent store: {err}"),
            )
                .into_response(),
        ),
    }
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
