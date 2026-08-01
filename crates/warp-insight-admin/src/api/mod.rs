// @moju generated
// @moju hash=2ff63c2da808b5ca

use std::sync::{Arc, Mutex};

use axum::{
    routing::{get, post},
    Router,
};

use crate::infra::{AdminConfig, AdminStore};

mod admin_auth;
mod admin_ops;
mod agent_ops;
mod enrollment;
mod install;
mod overview;
mod rate_limit;

pub mod warp_insight_admin_public_install_interface;
pub use warp_insight_admin_public_install_interface::*;
pub mod warp_insight_admin_management_interface;
pub use warp_insight_admin_management_interface::*;
pub mod wp_agent_online_registration_interface;
pub use wp_agent_online_registration_interface::*;

use admin_ops::{get_agent_runtime_status, pause_agent, upgrade_agent};
use agent_ops::{
    poll_control_commands, renew_agent_credential, report_action_result, submit_agent_status,
};
use enrollment::enroll_agent;
use install::{
    download_agent_package, get_agent_initial_config_with_token, get_agent_install_code,
    get_agent_install_script, get_agent_install_script_signature,
};
use overview::{get_agent_overview, RecentOnlineRegisteredAgent};

#[derive(Debug, Clone)]
pub struct ApiState {
    pub config: AdminConfig,
    pub store: AdminStore,
    pub runtime: Arc<Mutex<AdminRuntimeState>>,
    pub rate_limits: Arc<Mutex<rate_limit::RateLimitState>>,
}

#[derive(Debug, Default)]
pub struct AdminRuntimeState {
    pub recent_online_agents: Vec<RecentOnlineRegisteredAgent>,
}

pub fn router(config: AdminConfig) -> Router {
    let store = AdminStore::new(config.store_file.clone());
    Router::new()
        .route("/api/v1/agent/install-code", get(get_agent_install_code))
        .route(
            "/api/v1/agent/install/:arch/install.sh",
            get(get_agent_install_script),
        )
        .route(
            "/api/v1/agent/install/:arch/install.sh.sig",
            get(get_agent_install_script_signature),
        )
        .route(
            "/api/v1/agent/initial-config",
            get(get_agent_initial_config_with_token),
        )
        .route(
            "/api/v1/agent/packages/current",
            get(download_agent_package),
        )
        .route("/api/v1/agent/enroll", post(enroll_agent))
        .route("/api/v1/agent/status", post(submit_agent_status))
        .route(
            "/api/v1/agent/credentials:renew",
            post(renew_agent_credential),
        )
        .route(
            "/api/v1/agent/control-commands:poll",
            post(poll_control_commands),
        )
        .route("/api/v1/agent/action-results", post(report_action_result))
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
        .with_state(ApiState {
            config,
            store,
            runtime: Arc::new(Mutex::new(AdminRuntimeState::default())),
            rate_limits: Arc::new(Mutex::new(rate_limit::RateLimitState::default())),
        })
}

#[cfg(test)]
mod tests;
