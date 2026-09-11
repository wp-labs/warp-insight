//! Edge daemon skeleton.

pub mod bootstrap;
pub mod config;
pub mod control;
pub mod discovery;
pub mod error;
pub mod exec;
pub(crate) mod fs_async;
pub mod reporting;
pub mod runtime;
pub mod state_store;
pub(crate) mod telemetry;

pub use config::config_runtime;
pub use control::{capability_report, enrollment};
pub use exec::{
    execution_support, local_exec, planner_bridge, process_control, quarantine, recovery,
};
pub use reporting::{exporter, reporting_pipeline};
pub use runtime::{daemon, scheduler, self_observability};

pub async fn run() -> error::AgentdResult<()> {
    control::run().await
}
