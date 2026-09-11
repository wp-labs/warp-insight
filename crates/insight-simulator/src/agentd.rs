// Agentd 角色：向 WarpGateWay 上报 Agent 状态 / 动作结果。

use insight_control::types::DateTime;
use insight_control::{AgentHello, ReportActionResult};
use wist_reporting::{ActionResultContract, ResultAttestation};

use crate::{client, config::SimConfig};

/// 上报 Agent 状态：POST {gateway}/api/v1/agent/status（AgentHello）。
pub async fn report_agent_status(
    client: &reqwest::Client,
    config: &SimConfig,
) -> Result<(), String> {
    let url = format!(
        "{}/api/v1/agent/status",
        config.upstream_url.trim_end_matches('/')
    );
    let hello = build_agent_hello(config);
    let response = client::send_json(|| client.post(&url).json(&hello), &config.token).await?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "agent status report rejected: HTTP {status} body={}",
            response.text().await.unwrap_or_default()
        ));
    }
    println!(
        "event=AgentStatusReported status={status} agent_id={} instance_id={}",
        config.id, config.instance_id
    );
    Ok(())
}

/// 上报动作结果：POST {gateway}/api/v1/agent/action-results（ReportActionResult）。
/// result 字段按网关契约传 JSON 字符串（本地信封序列化）。
pub async fn report_action_result(
    client: &reqwest::Client,
    config: &SimConfig,
    sequence: u64,
) -> Result<(), String> {
    let url = format!(
        "{}/api/v1/agent/action-results",
        config.upstream_url.trim_end_matches('/')
    );
    let report = build_action_report(config, sequence, DateTime::now());
    let response = client::send_json(|| client.post(&url).json(&report), &config.token).await?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "agent action result report rejected: HTTP {status} body={}",
            response.text().await.unwrap_or_default()
        ));
    }
    println!(
        "event=ActionResultReported status={status} agent_id={} execution_id={}",
        config.id, report.execution_id
    );
    Ok(())
}

/// 构造 AgentHello（wire 形状：agent_id/instance_id/version + 可选指标）。
fn build_agent_hello(config: &SimConfig) -> AgentHello {
    AgentHello {
        agent_id: config.id.clone(),
        instance_id: config.instance_id.clone(),
        version: config.version.clone(),
        // 模拟运行指标（512MB 内存、~20% CPU、~8ms 时延）。
        memory_bytes: Some(512 * 1024 * 1024),
        cpu_percent: Some(20.0 + (config.id.len() as f64) * 2.5),
        admin_latency_ms: Some(8),
        work_state_changes: None,
    }
}

/// 构造 ReportActionResult（wire 形状：result 为 JSON 字符串、final_status 为字符串）。
fn build_action_report(config: &SimConfig, sequence: u64, now: DateTime) -> ReportActionResult {
    let execution_id = format!("exec-sim-{sequence}");
    let action_id = format!("action-sim-{sequence}");
    let contract = ActionResultContract {
        step_records: "[]".to_string(),
        execution_id: execution_id.clone(),
        exit_reason: String::new(),
        started_at: wist_shared::time::now_rfc3339(),
        outputs: "{}".to_string(),
        api_version: "v1".to_string(),
        resource_usage: "{}".to_string(),
        action_id: action_id.clone(),
        kind: "command".to_string(),
        request_id: String::new(),
        finished_at: wist_shared::time::now_rfc3339(),
        final_status: "succeeded".to_string(),
    };
    ReportActionResult {
        execution_id,
        kind: "command".to_string(),
        agent_id: config.id.clone(),
        result_attestation: ResultAttestation {
            issued_by: config.id.clone(),
            attested_at: now.clone(),
            result_digest: "sha256:simulated".to_string(),
            signature: "simulated".to_string(),
        },
        action_id,
        reported_at: now,
        final_status: "succeeded".to_string(),
        result: serde_json::to_string(&contract).unwrap_or_else(|_| "{}".to_string()),
        dispatch_id: format!("dispatch-sim-{sequence}"),
        plan_digest: "sha256:simulated-plan".to_string(),
        report_attempt: 1,
        report_id: format!("report-sim-{sequence}"),
        api_version: "v1".to_string(),
        instance_id: config.instance_id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SimConfig {
        SimConfig {
            role: crate::config::Role::Agentd,
            upstream_url: "https://127.0.0.1:3000".to_string(),
            id: "agent-sim".to_string(),
            instance_id: "inst-sim".to_string(),
            token: "sim-token".to_string(),
            interval_secs: 0,
            version: "v0.3.2".to_string(),
            insecure: false,
            status: None,
            health: None,
            fetch_config: false,
            report_agents: false,
            report_action: true,
            run_dir: None,
        }
    }

    #[test]
    fn agent_hello_serializes_wire_shape() {
        let hello = build_agent_hello(&test_config());
        let json = serde_json::to_value(&hello).expect("serialize");
        assert_eq!(json["agent_id"], "agent-sim");
        assert_eq!(json["instance_id"], "inst-sim");
        assert_eq!(json["version"], "v0.3.2");
        // 网关 handler 解码 insight_control::AgentHello，字段名即 snake_case。
        assert_eq!(json["memory_bytes"].as_i64(), Some(512 * 1024 * 1024));
    }

    #[test]
    fn action_report_serializes_wire_shape() {
        let report = build_action_report(&test_config(), 3, DateTime::now());
        let json = serde_json::to_value(&report).expect("serialize");
        assert_eq!(json["agent_id"], "agent-sim");
        assert_eq!(json["execution_id"], "exec-sim-3");
        assert_eq!(json["final_status"], "succeeded");
        assert_eq!(json["api_version"], "v1");
        // result 是 JSON 字符串（网关契约要求）。
        let result: serde_json::Value = serde_json::from_str(json["result"].as_str().unwrap_or(""))
            .expect("result is json string");
        assert_eq!(result["action_id"], "action-sim-3");
    }
}
