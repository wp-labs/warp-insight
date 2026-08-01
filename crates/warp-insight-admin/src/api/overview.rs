use std::net::SocketAddr;

use axum::{
    extract::{connect_info::ConnectInfo, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::domain::types::{AgentRuntimeStatusView, DateTime};
use crate::infra::StoredAgentRegistration;

use super::admin_auth::require_admin_bearer;
use super::{rate_limit, AdminRuntimeState, ApiState};

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
pub struct RecentOnlineRegisteredAgent {
    pub agent_id: String,
    pub instance_id: String,
    pub version: String,
    pub registered_at: DateTime,
    pub online_since: DateTime,
    pub online_duration_seconds: i64,
    pub source: RecentOnlineRegisteredAgentSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecentOnlineRegisteredAgentSource {
    Real,
    Example,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOverview {
    pub metrics: AgentOverviewMetrics,
    pub recent_online_agents: Vec<RecentOnlineRegisteredAgent>,
    pub abnormal_agents: Vec<AgentRuntimeStatusView>,
}

pub async fn get_agent_overview(
    State(state): State<ApiState>,
    headers: HeaderMap,
    client: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let client_key = rate_limit::client_key(client);
    if let Err(response) = require_admin_bearer(&state, &headers, &client_key) {
        return response;
    }
    Json(agent_overview(&state)).into_response()
}

pub fn agent_overview(state: &ApiState) -> AgentOverview {
    let stored_agents = state
        .store
        .load()
        .map(|snapshot| recent_online_agents_from_store(snapshot.agents.into_values().collect()))
        .unwrap_or_default();
    let memory_agents = state
        .runtime
        .lock()
        .expect("runtime state poisoned")
        .recent_online_agents
        .clone();
    let recent_online_agents = merge_recent_online_agents(stored_agents, memory_agents);
    let total_agents = recent_online_agents.len() as i64;
    let display_recent_online_agents = display_recent_online_agents(recent_online_agents);

    AgentOverview {
        metrics: AgentOverviewMetrics {
            total_agents,
            online_agents: total_agents,
            unhealthy_agents: 0,
            last_seen_lag_seconds: 0,
        },
        recent_online_agents: display_recent_online_agents,
        abnormal_agents: Vec::new(),
    }
}

fn recent_online_agents_from_store(
    mut agents: Vec<StoredAgentRegistration>,
) -> Vec<RecentOnlineRegisteredAgent> {
    agents.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));
    agents
        .into_iter()
        .map(|agent| {
            let registered_at =
                DateTime::from_rfc3339(&agent.registered_at).unwrap_or_else(DateTime::now);
            let online_since =
                DateTime::from_rfc3339(&agent.last_seen_at).unwrap_or_else(DateTime::now);
            let online_duration_seconds = registered_at.seconds_until(&online_since);
            recent_online_registered_agent_at(
                &agent.agent_id,
                &agent.instance_id,
                &agent.version,
                registered_at,
                online_since,
                online_duration_seconds,
                RecentOnlineRegisteredAgentSource::Real,
            )
        })
        .collect()
}

fn merge_recent_online_agents(
    stored_agents: Vec<RecentOnlineRegisteredAgent>,
    memory_agents: Vec<RecentOnlineRegisteredAgent>,
) -> Vec<RecentOnlineRegisteredAgent> {
    let mut merged = memory_agents;
    for agent in stored_agents {
        if !merged
            .iter()
            .any(|existing| existing.agent_id == agent.agent_id)
        {
            merged.push(agent);
        }
    }
    merged
}

pub fn record_recent_online_agent(
    runtime: &std::sync::Arc<std::sync::Mutex<AdminRuntimeState>>,
    agent_id: &str,
    instance_id: &str,
    version: &str,
    requested_at: &str,
) {
    let now = DateTime::now();
    let registered_at = DateTime::from_rfc3339(requested_at).unwrap_or_else(DateTime::now);
    let online_duration_seconds = registered_at.seconds_until(&now);
    let agent = recent_online_registered_agent_at(
        agent_id,
        instance_id,
        version,
        registered_at,
        now,
        online_duration_seconds,
        RecentOnlineRegisteredAgentSource::Real,
    );
    let mut state = runtime.lock().expect("runtime state poisoned");
    state
        .recent_online_agents
        .retain(|existing| existing.agent_id != agent_id);
    state.recent_online_agents.insert(0, agent);
    state.recent_online_agents.truncate(6);
}

fn recent_online_registered_agent_at(
    agent_id: &str,
    instance_id: &str,
    version: &str,
    registered_at: DateTime,
    online_since: DateTime,
    online_duration_seconds: i64,
    source: RecentOnlineRegisteredAgentSource,
) -> RecentOnlineRegisteredAgent {
    RecentOnlineRegisteredAgent {
        agent_id: agent_id.to_string(),
        instance_id: instance_id.to_string(),
        version: version.to_string(),
        registered_at,
        online_since,
        online_duration_seconds,
        source,
    }
}

fn display_recent_online_agents(
    mut real_agents: Vec<RecentOnlineRegisteredAgent>,
) -> Vec<RecentOnlineRegisteredAgent> {
    let target_count = 3;
    if real_agents.len() >= target_count {
        real_agents.truncate(target_count);
        return real_agents;
    }

    let existing_agent_ids: std::collections::HashSet<String> = real_agents
        .iter()
        .map(|agent| agent.agent_id.clone())
        .collect();
    let needed = target_count - real_agents.len();
    real_agents.extend(
        example_recent_online_agents()
            .into_iter()
            .filter(|agent| !existing_agent_ids.contains(&agent.agent_id))
            .take(needed),
    );
    real_agents
}

fn example_recent_online_agents() -> Vec<RecentOnlineRegisteredAgent> {
    vec![
        example_recent_online_agent("example-agent-control-01", "control-node-01", "v0.1.0", 930),
        example_recent_online_agent("example-agent-edge-02", "edge-node-02", "v0.1.0", 1_860),
        example_recent_online_agent("example-agent-build-03", "build-node-03", "v0.1.0", 3_420),
    ]
}

fn example_recent_online_agent(
    agent_id: &str,
    instance_id: &str,
    version: &str,
    online_duration_seconds: i64,
) -> RecentOnlineRegisteredAgent {
    let now = DateTime::now();
    recent_online_registered_agent_at(
        agent_id,
        instance_id,
        version,
        now.clone(),
        now,
        online_duration_seconds,
        RecentOnlineRegisteredAgentSource::Example,
    )
}
