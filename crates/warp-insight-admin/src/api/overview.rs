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

const ONLINE_WINDOW_SECONDS: i64 = 300;

pub fn agent_overview(state: &ApiState) -> AgentOverview {
    let stored_agents = state
        .store
        .load()
        .map(|snapshot| snapshot.agents.into_values().collect::<Vec<_>>())
        .unwrap_or_default();
    let memory_agents = state
        .runtime
        .lock()
        .expect("runtime state poisoned")
        .recent_online_agents
        .clone();
    let mut recent_online_agents = recent_online_agents_from_store(stored_agents);
    for agent in memory_agents {
        if !recent_online_agents
            .iter()
            .any(|existing| existing.agent_id == agent.agent_id)
        {
            recent_online_agents.push(agent);
        }
    }
    recent_online_agents.sort_by(|left, right| right.registered_at.cmp(&left.registered_at));
    recent_online_agents.truncate(6);

    let now = DateTime::now();
    let total_agents = recent_online_agents.len() as i64;
    let online_agents = recent_online_agents
        .iter()
        .filter(|agent| {
            let lag = agent.online_since.seconds_until(&now);
            (0..=ONLINE_WINDOW_SECONDS).contains(&lag)
        })
        .count() as i64;
    let last_seen_lag_seconds = recent_online_agents
        .iter()
        .map(|agent| agent.online_since.seconds_until(&now))
        .max()
        .unwrap_or(0)
        .max(0);

    AgentOverview {
        metrics: AgentOverviewMetrics {
            total_agents,
            online_agents,
            unhealthy_agents: 0,
            last_seen_lag_seconds,
        },
        recent_online_agents,
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

