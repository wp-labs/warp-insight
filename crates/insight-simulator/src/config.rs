// 模拟器运行配置（手写参数解析，与 wist-agentd 一致，无 clap）。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Gateway,
    Agentd,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gateway => "gateway",
            Self::Agentd => "agentd",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimConfig {
    pub role: Role,
    /// 上游节点基地址：Gateway 角色 = WarpInsightCenter；Agentd 角色 = WarpGateWay。
    pub upstream_url: String,
    /// Gateway 角色 = gateway_id；Agentd 角色 = agent_id。
    pub id: String,
    pub instance_id: String,
    pub token: String,
    /// 上报间隔秒；0 = 只发一次。
    pub interval_secs: u64,
    pub version: String,
    /// 接受无效 TLS 证书（连自签 dev 网关）。
    pub insecure: bool,
    /// Gateway 角色上报的在线状态（默认 online）。
    pub status: Option<String>,
    /// Gateway 角色上报的健康状态（默认 healthy）。
    pub health: Option<String>,
    /// Gateway 角色：启动时拉一次初始配置。
    pub fetch_config: bool,
    /// Gateway 角色：每次一并上报其下 2 个模拟 Agent 状态。
    pub report_agents: bool,
    /// Agentd 角色：每次一并上报一个动作结果。
    pub report_action: bool,
    /// Gateway 角色 onboarding 时把 config.toml / runtime-token 落盘到 run_dir/gateways/<gw>/<instance>/。
    pub run_dir: Option<String>,
}

impl SimConfig {
    pub fn usage() -> &'static str {
        r#"insight-simulator <role> [options]

role:
  gateway   扮演 WarpGateWay，向 WarpInsightCenter 发数据
  agentd    扮演 WpAgent，向 WarpGateWay 发数据

gateway options:
  --center-url <url>    WarpInsightCenter 基地址（默认 http://127.0.0.1:3100）
  --gateway-id <id>      网关 ID（默认 gw-sim）
  --instance-id <id>     实例 ID（默认 inst-sim）
  --token <token>        网关凭证 bearer token（必填）
  --interval <secs>      上报间隔秒（默认 5；0 = 只发一次）
  --once                 只发一次（等价 --interval 0）
  --status <s>           在线状态 online|offline（默认 online）
  --health <h>           健康 healthy|degraded|unhealthy（默认 healthy）
  --version <v>          版本（默认 v2.4.1）
  --fetch-config         启动时拉一次初始配置（GET /api/v1/gateway/initial-config）
  --report-agents        每次一并上报其下 2 个模拟 Agent 状态
  --run-dir <dir>        onboarding 时把 config.toml / runtime-token 写盘到 <dir>/gateways/<gw>/<instance>/
  --insecure             接受无效 TLS 证书

agentd options:
  --gateway-url <url>    WarpGateWay 基地址（默认 https://127.0.0.1:3000）
  --agent-id <id>        Agent ID（默认 agent-sim）
  --instance-id <id>     实例 ID（默认 inst-sim）
  --token <token>        Agent bearer 凭证（必填，需网关已注册该 agent）
  --interval <secs>      上报间隔秒（默认 5；0 = 只发一次）
  --version <v>          版本（默认 v0.3.2）
  --report-action        每次一并上报一个动作结果
  --insecure             接受无效 TLS 证书

通用：
  --help                 打印帮助
"#
    }

    pub fn parse(args: &[String]) -> Result<Self, String> {
        if args.is_empty()
            || args.iter().any(|a| a == "--help" || a == "-h" || a == "help")
        {
            return Err(Self::usage().to_string());
        }
        let role = match args[0].as_str() {
            "gateway" => Role::Gateway,
            "agentd" => Role::Agentd,
            other => return Err(format!("unknown role {other:?}; expected gateway or agentd")),
        };
        let mut cfg = SimConfig {
            role,
            upstream_url: match role {
                Role::Gateway => "http://127.0.0.1:3100".to_string(),
                Role::Agentd => "https://127.0.0.1:3000".to_string(),
            },
            id: match role {
                Role::Gateway => "gw-sim".to_string(),
                Role::Agentd => "agent-sim".to_string(),
            },
            instance_id: "inst-sim".to_string(),
            token: String::new(),
            interval_secs: 5,
            version: match role {
                Role::Gateway => "v2.4.1".to_string(),
                Role::Agentd => "v0.3.2".to_string(),
            },
            insecure: false,
            status: None,
            health: None,
            fetch_config: false,
            report_agents: false,
            report_action: false,
            run_dir: None,
        };

        let rest = &args[1..];
        let mut index = 0;
        while index < rest.len() {
            let key = rest[index].as_str();
            match key {
                "--insecure"
                | "--fetch-config"
                | "--once"
                | "--report-agents"
                | "--report-action" => {
                    match key {
                        "--insecure" => cfg.insecure = true,
                        "--fetch-config" => cfg.fetch_config = true,
                        "--once" => cfg.interval_secs = 0,
                        "--report-agents" => cfg.report_agents = true,
                        _ => cfg.report_action = true,
                    }
                    index += 1;
                }
                _ => {
                    let value = rest
                        .get(index + 1)
                        .ok_or_else(|| format!("missing value for option {key}"))?;
                    match key {
                        "--center-url" | "--gateway-url" => cfg.upstream_url = value.clone(),
                        "--gateway-id" | "--agent-id" => cfg.id = value.clone(),
                        "--instance-id" => cfg.instance_id = value.clone(),
                        "--token" => cfg.token = value.clone(),
                        "--interval" => {
                            cfg.interval_secs = value.parse().map_err(|_| {
                                format!("invalid --interval value {value:?}")
                            })?;
                        }
                        "--version" => cfg.version = value.clone(),
                        "--status" => cfg.status = Some(value.clone()),
                        "--run-dir" => cfg.run_dir = Some(value.clone()),
                        "--health" => cfg.health = Some(value.clone()),
                        other => return Err(format!("unknown option {other}")),
                    }
                    index += 2;
                }
            }
        }

        if cfg.token.is_empty() {
            return Err("missing required option --token".to_string());
        }
        if cfg.upstream_url.trim().is_empty() {
            return Err("missing upstream url".to_string());
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_gateway_role_with_defaults() {
        let cfg = SimConfig::parse(&args(&[
            "gateway",
            "--token",
            "sim-token",
            "--gateway-id",
            "gw-x",
        ]))
        .expect("parse");
        assert_eq!(cfg.role, Role::Gateway);
        assert_eq!(cfg.id, "gw-x");
        assert_eq!(cfg.upstream_url, "http://127.0.0.1:3100");
        assert_eq!(cfg.interval_secs, 5);
    }

    #[test]
    fn parses_agentd_flags_without_value() {
        let cfg = SimConfig::parse(&args(&[
            "agentd",
            "--token",
            "sim-token",
            "--insecure",
            "--report-action",
            "--interval",
            "0",
        ]))
        .expect("parse");
        assert_eq!(cfg.role, Role::Agentd);
        assert!(cfg.insecure);
        assert!(cfg.report_action);
        assert_eq!(cfg.interval_secs, 0);
        assert_eq!(cfg.upstream_url, "https://127.0.0.1:3000");
    }

    #[test]
    fn rejects_unknown_role_and_missing_token() {
        assert!(SimConfig::parse(&args(&["web", "--token", "x"])).is_err());
        assert!(SimConfig::parse(&args(&["gateway"])).is_err());
        assert!(SimConfig::parse(&args(&["gateway", "--unknown", "v"])).is_err());
    }
}
