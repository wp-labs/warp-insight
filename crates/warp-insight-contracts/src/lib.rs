//! Versioned contract objects shared by edge and center components.

pub mod action_plan;
pub mod action_result;
pub mod agent_config;
pub mod capability_report;
pub mod discovery;
pub mod enrollment;
pub mod exporter;
pub mod gateway;
pub mod ingest;
pub mod state_exec;
pub mod telemetry_record;

pub const API_VERSION_V1: &str = "v1";
pub const SCHEMA_VERSION_V1: &str = "v1";
