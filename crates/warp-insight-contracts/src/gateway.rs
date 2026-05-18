//! Gateway envelope contract types.

use serde::{Deserialize, Serialize};

use crate::API_VERSION_V1;
use crate::action_plan::ActionPlanContract;
use crate::action_result::{ActionResultContract, FinalStatus};

pub const DISPATCH_ACTION_PLAN_KIND: &str = "dispatch_action_plan";
pub const ACTION_PLAN_ACK_KIND: &str = "action_plan_ack";
pub const REPORT_ACTION_RESULT_KIND: &str = "report_action_result";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentHello {
    pub agent_id: String,
    pub instance_id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchActionPlan {
    pub api_version: String,
    pub kind: String,
    pub dispatch_id: String,
    pub plan: ActionPlanContract,
}

impl DispatchActionPlan {
    pub fn new(dispatch_id: String, plan: ActionPlanContract) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            kind: DISPATCH_ACTION_PLAN_KIND.to_string(),
            dispatch_id,
            plan,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionPlanAck {
    pub api_version: String,
    pub kind: String,
    pub dispatch_id: String,
    pub action_id: String,
    pub plan_digest: String,
    pub agent_id: String,
    pub instance_id: String,
    pub execution_id: Option<String>,
    pub ack_status: AckStatus,
    pub reason_code: Option<String>,
    pub reason_message: Option<String>,
    pub queue_position: Option<u64>,
    pub received_at: String,
    pub acknowledged_at: String,
}

impl ActionPlanAck {
    pub fn builder(
        dispatch_id: String,
        action_id: String,
        ack_status: AckStatus,
    ) -> ActionPlanAckBuilder {
        ActionPlanAckBuilder {
            dispatch_id,
            action_id,
            plan_digest: String::new(),
            agent_id: String::new(),
            instance_id: String::new(),
            execution_id: None,
            ack_status,
            reason_code: None,
            reason_message: None,
            queue_position: None,
            received_at: String::new(),
            acknowledged_at: String::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dispatch_id: String,
        action_id: String,
        plan_digest: String,
        agent_id: String,
        instance_id: String,
        execution_id: Option<String>,
        ack_status: AckStatus,
        received_at: String,
        acknowledged_at: String,
    ) -> Self {
        Self::builder(dispatch_id, action_id, ack_status)
            .plan_digest(plan_digest)
            .agent_id(agent_id)
            .instance_id(instance_id)
            .execution_id(execution_id)
            .received_at(received_at)
            .acknowledged_at(acknowledged_at)
            .build()
    }
}

#[derive(Debug, Clone)]
pub struct ActionPlanAckBuilder {
    dispatch_id: String,
    action_id: String,
    plan_digest: String,
    agent_id: String,
    instance_id: String,
    execution_id: Option<String>,
    ack_status: AckStatus,
    reason_code: Option<String>,
    reason_message: Option<String>,
    queue_position: Option<u64>,
    received_at: String,
    acknowledged_at: String,
}

impl ActionPlanAckBuilder {
    pub fn plan_digest(mut self, plan_digest: String) -> Self {
        self.plan_digest = plan_digest;
        self
    }

    pub fn agent_id(mut self, agent_id: String) -> Self {
        self.agent_id = agent_id;
        self
    }

    pub fn instance_id(mut self, instance_id: String) -> Self {
        self.instance_id = instance_id;
        self
    }

    pub fn execution_id(mut self, execution_id: Option<String>) -> Self {
        self.execution_id = execution_id;
        self
    }

    pub fn reason_code(mut self, reason_code: Option<String>) -> Self {
        self.reason_code = reason_code;
        self
    }

    pub fn reason_message(mut self, reason_message: Option<String>) -> Self {
        self.reason_message = reason_message;
        self
    }

    pub fn queue_position(mut self, queue_position: Option<u64>) -> Self {
        self.queue_position = queue_position;
        self
    }

    pub fn received_at(mut self, received_at: String) -> Self {
        self.received_at = received_at;
        self
    }

    pub fn acknowledged_at(mut self, acknowledged_at: String) -> Self {
        self.acknowledged_at = acknowledged_at;
        self
    }

    pub fn build(self) -> ActionPlanAck {
        ActionPlanAck {
            api_version: API_VERSION_V1.to_string(),
            kind: ACTION_PLAN_ACK_KIND.to_string(),
            dispatch_id: self.dispatch_id,
            action_id: self.action_id,
            plan_digest: self.plan_digest,
            agent_id: self.agent_id,
            instance_id: self.instance_id,
            execution_id: self.execution_id,
            ack_status: self.ack_status,
            reason_code: self.reason_code,
            reason_message: self.reason_message,
            queue_position: self.queue_position,
            received_at: self.received_at,
            acknowledged_at: self.acknowledged_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportActionResult {
    pub api_version: String,
    pub report_id: String,
    pub kind: String,
    pub dispatch_id: Option<String>,
    pub action_id: String,
    pub report_attempt: u32,
    pub final_status: FinalStatus,
    pub execution_id: String,
    pub plan_digest: String,
    pub agent_id: String,
    pub instance_id: String,
    pub result_attestation: ResultAttestation,
    pub reported_at: String,
    pub result: ActionResultContract,
}

impl ReportActionResult {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        report_id: String,
        action_id: String,
        report_attempt: u32,
        final_status: FinalStatus,
        execution_id: String,
        plan_digest: String,
        agent_id: String,
        instance_id: String,
        result_attestation: ResultAttestation,
        reported_at: String,
        result: ActionResultContract,
    ) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            report_id,
            kind: REPORT_ACTION_RESULT_KIND.to_string(),
            dispatch_id: None,
            action_id,
            report_attempt,
            final_status,
            execution_id,
            plan_digest,
            agent_id,
            instance_id,
            result_attestation,
            reported_at,
            result,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultAttestation {
    /// Development placeholder until real signing and verifier plumbing is implemented.
    pub result_digest: String,
    /// Development placeholder signature. Consumers must not treat this as production attestation.
    pub signature: String,
    /// Development placeholder issuer identity, prefixed as `dev-placeholder:...`.
    pub issued_by: String,
    pub attested_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AckStatus {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "duplicate")]
    Duplicate,
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "busy")]
    Busy,
}
