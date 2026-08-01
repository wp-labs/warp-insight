// @moju generated
// @moju hash=decaea78429c5f4f

pub type UserId = String;
pub type OrderId = String;
pub type CartId = String;
pub type PaymentIntentId = String;
pub type Money = i64;
pub type Sku = String;
pub type Quantity = i64;
pub type Address = String;
pub type CartItemList = String;
pub type Int = i64;
pub type Bool = bool;
pub type Float = f64;
pub type AgentCredentialVerificationStatus = String;
pub type AgentEnrollmentResultStatus = String;
pub type AgentEnrollmentTokenStatus = String;
pub type AgentEnrollmentTokenValidationStatus = String;
pub type AgentIdentityStatus = String;
pub type HealthState = String;

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, ::serde::Serialize, ::serde::Deserialize,
)]
pub struct DateTime(chrono::DateTime<chrono::Utc>);

impl DateTime {
    pub fn now() -> Self {
        Self(chrono::Utc::now())
    }

    pub fn from_rfc3339(value: &str) -> Option<Self> {
        chrono::DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|value| Self(value.with_timezone(&chrono::Utc)))
    }

    pub fn seconds_until(&self, later: &Self) -> i64 {
        later.0.signed_duration_since(self.0).num_seconds().max(0)
    }
}

impl std::fmt::Debug for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct Secret(String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}
pub mod daemon_loop;
pub use daemon_loop::*;
pub mod action_plan_meta;
pub use action_plan_meta::*;
pub mod prepared_report;
pub use prepared_report::*;
pub mod action_plan_step;
pub use action_plan_step::*;
pub mod agent_control_command;
pub use agent_control_command::*;
pub mod capability_limits;
pub use capability_limits::*;
pub mod runtime_health_snapshot;
pub use runtime_health_snapshot::*;
pub mod telemetry_failure_kind;
pub use telemetry_failure_kind::*;
pub mod final_status;
pub use final_status::*;
pub mod agent_enrollment_token;
pub use agent_enrollment_token::*;
pub mod agent_enrollment_token_validation;
pub use agent_enrollment_token_validation::*;
pub mod agent_install_code;
pub use agent_install_code::*;
pub mod action_output_item;
pub use action_output_item::*;
pub mod metrics_health_snapshot;
pub use metrics_health_snapshot::*;
pub mod exporter_source;
pub use exporter_source::*;
pub mod reporting_request;
pub use reporting_request::*;
pub mod discovery_report_mode;
pub use discovery_report_mode::*;
pub mod metrics_tick;
pub use metrics_tick::*;
pub mod local_report_inspection;
pub use local_report_inspection::*;
pub mod prepared_report_origin;
pub use prepared_report_origin::*;
pub mod agent_enrollment_auth_profile;
pub use agent_enrollment_auth_profile::*;
pub mod upgrade_capabilities;
pub use upgrade_capabilities::*;
pub mod export_result;
pub use export_result::*;
pub mod risk_level;
pub use risk_level::*;
pub mod approval_mode;
pub use approval_mode::*;
pub mod result_attestation;
pub use result_attestation::*;
pub mod action_plan_contract;
pub use action_plan_contract::*;
pub mod agent_bootstrap_bundle;
pub use agent_bootstrap_bundle::*;
pub mod telemetry_tick;
pub use telemetry_tick::*;
pub mod telemetry_failure;
pub use telemetry_failure::*;
pub mod execution_resource_usage;
pub use execution_resource_usage::*;
pub mod discovery_health;
pub use discovery_health::*;
pub mod telemetry_record_contract;
pub use telemetry_record_contract::*;
pub mod agent_instance;
pub use agent_instance::*;
pub mod action_plan_target;
pub use action_plan_target::*;
pub mod action_plan_constraints;
pub use action_plan_constraints::*;
pub mod metrics_capabilities;
pub use metrics_capabilities::*;
pub mod action_plan_program;
pub use action_plan_program::*;
pub mod agent_credential_verification_result;
pub use agent_credential_verification_result::*;
pub mod exec_capabilities;
pub use exec_capabilities::*;
pub mod discovery_probe_health;
pub use discovery_probe_health::*;
pub mod agent_runtime_status_view;
pub use agent_runtime_status_view::*;
pub mod agent_credential_bundle;
pub use agent_credential_bundle::*;
pub mod step_action_record;
pub use step_action_record::*;
pub mod discovery_ingest_ack_status;
pub use discovery_ingest_ack_status::*;
pub mod signal_request_kind;
pub use signal_request_kind::*;
pub mod agent_initial_config;
pub use agent_initial_config::*;
pub mod action_result_receipt;
pub use action_result_receipt::*;
pub mod action_result_contract;
pub use action_result_contract::*;
pub mod local_report_issue;
pub use local_report_issue::*;
pub mod dispatch_receipt;
pub use dispatch_receipt::*;
pub mod metrics_failure;
pub use metrics_failure::*;
pub mod agent_host_profile;
pub use agent_host_profile::*;
pub mod exporter_output;
pub use exporter_output::*;
pub mod agent_identity;
pub use agent_identity::*;
pub mod metrics_failure_kind;
pub use metrics_failure_kind::*;
pub mod disc_row_context;
pub use disc_row_context::*;
pub mod action_outputs;
pub use action_outputs::*;
pub mod ack_status;
pub use ack_status::*;
pub mod agent_policy_binding;
pub use agent_policy_binding::*;
pub mod capability_report_sections;
pub use capability_report_sections::*;
pub mod agent_enrollment_result;
pub use agent_enrollment_result::*;
pub mod agent_control_auth_profile;
pub use agent_control_auth_profile::*;
pub mod step_status;
pub use step_status::*;
pub mod discovery_readiness;
pub use discovery_readiness::*;
pub mod running_state_status;
pub use running_state_status::*;
pub mod logs_capabilities;
pub use logs_capabilities::*;
pub mod capability_report_contract;
pub use capability_report_contract::*;
pub mod management_endpoint_trust_bundle;
pub use management_endpoint_trust_bundle::*;
pub mod execution_history_record;
pub use execution_history_record::*;
pub mod action_plan_ack_builder;
pub use action_plan_ack_builder::*;
pub mod discovery_health_snapshot;
pub use discovery_health_snapshot::*;
pub mod health_state;
#[allow(unused_imports)]
pub use health_state::*;
pub mod agent_enrollment_result_status;
#[allow(unused_imports)]
pub use agent_enrollment_result_status::*;
pub mod agent_credential_verification_status;
#[allow(unused_imports)]
pub use agent_credential_verification_status::*;
pub mod agent_identity_status;
#[allow(unused_imports)]
pub use agent_identity_status::*;
pub mod agent_upstream_message_type;
#[allow(unused_imports)]
pub use agent_upstream_message_type::*;
pub mod agent_enrollment_token_status;
#[allow(unused_imports)]
pub use agent_enrollment_token_status::*;
pub mod agent_downstream_message_type;
#[allow(unused_imports)]
pub use agent_downstream_message_type::*;
pub mod agent_enrollment_token_validation_status;
#[allow(unused_imports)]
pub use agent_enrollment_token_validation_status::*;
