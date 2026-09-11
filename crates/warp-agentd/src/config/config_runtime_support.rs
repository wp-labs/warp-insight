use std::env;
use std::path::{Component, Path, PathBuf};

use orion_error::conversion::ToStructError;
use wist_contracts::agent_config::AgentConfigContract;

use crate::config_runtime::{ConfigError, ConfigReason};

pub(super) fn default_file_config_text() -> String {
    r#"schema_version = "v1"

[telemetry.logs]
in_memory_buffer_bytes = 1048576
spool_dir = "state/spool/logs"

[telemetry.logs.output]
kind = "file"

[telemetry.logs.output.file]
path = "log/warp-parse-records.ndjson"

[discovery]
# 默认保留 host + network + endpoint + process discovery，便于本地 metrics / action target 建模。
# container 属于更高基数发现，只有启用对应场景时再显式打开。
host_enabled = true
network_enabled = true
endpoint_enabled = true
process_enabled = true
container_enabled = false

# 可选：
# [agent]
# # 为空时会自动生成实例名；如需显式指定可取消注释。
# # instance_name = "monitoring-host-01"
#
# 可选：
# [control_plane]
# # install.sh 可写入管理端地址和 enrollment token；注册成功后 daemon 会把正式 agent_id 写入 state。
# enabled = true
# endpoint = "https://10.0.1.1"
# enrollment_token = "${WARP_INSIGHT_ENROLLMENT_TOKEN}"
# tls_mode = "https"
# trust_bundle = ""
# auth_mode = "enrollment_token"
#
# 可选：
# [paths]
# # 下面这些默认分别是 ".", "run", "state", "log"
# # root_dir = "."
# # run_dir = "run"
# # state_dir = "state"
# # log_dir = "log"
#
# 采集任务与运行设定的稳定度不同：可以把 [[telemetry.logs.file_inputs]] 清单移到独立文件，
# 在 [telemetry.logs] 内用 file_inputs_file 引用（路径相对本配置文件，与内联二选一）：
# file_inputs_file = "tasks/apps.toml"
#   tasks/apps.toml 内容示例：
#   [[file_inputs]]
#   input_id = "monitoring-app"
#   path = "/var/log/monitoring/app.log"
#   startup_position = "tail"
#   multiline_mode = "none"
#
# 示例：把某个监控系统日志文件送到本地 warp-parse record 输出文件。
# 取消注释后，把 path 改成你的真实日志路径。
#
# [[telemetry.logs.file_inputs]]
# input_id = "monitoring-app"
# path = "/var/log/monitoring/app.log"
# startup_position = "head"
# multiline_mode = "none"
#
# macOS P0 采集清单（示例，按需取消注释）：只支持“可追加的单个文本文件”。
# 需要 root/Full Disk Access 才能读的路径，agent 用户态读取失败会被跳过/进本地缓冲重试，
# 生产请用 root helper 采集；新增文件（.ips）与统一日志/audit 需 Phase 2 source。
# 注：install.log / launchd.log 体量大，默认 tail（只收新增行）；需要首次回放历史时改 head。
#
# [[telemetry.logs.file_inputs]]
# input_id = "macos_install_log"
# path = "/var/log/install.log"
# startup_position = "tail"
# multiline_mode = "none"
#
# [[telemetry.logs.file_inputs]]
# input_id = "macos_launchd"
# path = "/var/log/com.apple.xpc.launchd/launchd.log"
# startup_position = "tail"
# multiline_mode = "none"
#
# [[telemetry.logs.file_inputs]]
# input_id = "macos_shutdown_monitor"
# path = "/var/log/shutdown_monitor.log"
# startup_position = "tail"
# multiline_mode = "none"
#
# [[telemetry.logs.file_inputs]]
# input_id = "macos_fsck"
# path = "/var/log/fsck_apfs.log"
# startup_position = "head"
# multiline_mode = "none"
#
# [[telemetry.logs.file_inputs]]
# input_id = "macos_wifi"
# path = "/var/log/wifi.log"
# startup_position = "tail"
# multiline_mode = "none"
#
# 示例：把日志通过 TCP 发到本机 WarpParse 的 tcp_src。
# [telemetry.logs.output]
# kind = "tcp"
#
# [telemetry.logs.output.tcp]
# addr = "127.0.0.1"
# port = 9000
# framing = "line"
"#
    .to_string()
}

pub(super) fn expand_env_contract(
    mut config: AgentConfigContract,
) -> Result<AgentConfigContract, ConfigError> {
    config.agent.agent_id = expand_optional(config.agent.agent_id)?;
    config.agent.environment_id = expand_optional(config.agent.environment_id)?;
    config.agent.instance_name = expand_optional(config.agent.instance_name)?;
    config.control_plane.endpoint = expand_optional(config.control_plane.endpoint)?;
    config.control_plane.enrollment_token = expand_optional(config.control_plane.enrollment_token)?;
    config.control_plane.credential_request =
        expand_optional(config.control_plane.credential_request)?;
    config.control_plane.credential_id = expand_optional(config.control_plane.credential_id)?;
    config.control_plane.bearer_token = expand_optional(config.control_plane.bearer_token)?;
    config.control_plane.credential_expires_at =
        expand_optional(config.control_plane.credential_expires_at)?;
    config.control_plane.tls_mode = expand_optional(config.control_plane.tls_mode)?;
    config.control_plane.trust_bundle = expand_optional(config.control_plane.trust_bundle)?;
    config.control_plane.auth_mode = expand_optional(config.control_plane.auth_mode)?;
    config.paths.root_dir = expand_string(config.paths.root_dir)?;
    config.paths.run_dir = expand_string(config.paths.run_dir)?;
    config.paths.state_dir = expand_string(config.paths.state_dir)?;
    config.paths.log_dir = expand_string(config.paths.log_dir)?;
    config.telemetry.logs.spool_dir = expand_string(config.telemetry.logs.spool_dir)?;
    config.telemetry.logs.output.kind = expand_string(config.telemetry.logs.output.kind)?;
    config.telemetry.logs.output.file.path = expand_string(config.telemetry.logs.output.file.path)?;
    config.telemetry.logs.output.tcp.addr = expand_string(config.telemetry.logs.output.tcp.addr)?;
    config.telemetry.logs.output.tcp.framing =
        expand_string(config.telemetry.logs.output.tcp.framing)?;
    for input in &mut config.telemetry.logs.file_inputs {
        input.input_id = expand_string(std::mem::take(&mut input.input_id))?;
        input.path = expand_string(std::mem::take(&mut input.path))?;
        input.startup_position = expand_string(std::mem::take(&mut input.startup_position))?;
        input.multiline_mode = expand_string(std::mem::take(&mut input.multiline_mode))?;
    }
    Ok(config)
}

pub(super) fn resolve_paths(
    mut config: AgentConfigContract,
    config_path: &Path,
) -> AgentConfigContract {
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
    let root_dir = absolutize(config_dir, &config.paths.root_dir);

    config.paths.root_dir = root_dir.display().to_string();
    config.paths.run_dir = absolutize(&root_dir, &config.paths.run_dir)
        .display()
        .to_string();
    config.paths.state_dir = absolutize(&root_dir, &config.paths.state_dir)
        .display()
        .to_string();
    config.paths.log_dir = absolutize(&root_dir, &config.paths.log_dir)
        .display()
        .to_string();
    config.telemetry.logs.spool_dir = absolutize(&root_dir, &config.telemetry.logs.spool_dir)
        .display()
        .to_string();
    config.telemetry.logs.output.file.path =
        absolutize(&root_dir, &config.telemetry.logs.output.file.path)
            .display()
            .to_string();
    config.telemetry.logs.file_inputs = config
        .telemetry
        .logs
        .file_inputs
        .into_iter()
        .map(|mut input| {
            input.path = absolutize(&root_dir, &input.path).display().to_string();
            input
        })
        .collect();
    config
}

pub(super) fn absolutize(base: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    normalize_path(joined)
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn expand_optional(value: Option<String>) -> Result<Option<String>, ConfigError> {
    value.map(expand_string).transpose()
}

pub(super) fn expand_string(value: String) -> Result<String, ConfigError> {
    let mut out = String::with_capacity(value.len());
    let mut cursor = 0usize;
    while let Some(start) = value[cursor..].find("${") {
        let start = cursor + start;
        out.push_str(&value[cursor..start]);
        let rest = &value[start + 2..];
        let Some(end_rel) = rest.find('}') else {
            out.push_str(&value[start..]);
            return Ok(out);
        };
        let end = start + 2 + end_rel;
        let name = &value[start + 2..end];
        let expanded =
            env::var(name).map_err(|_| ConfigReason::MissingEnvVar.to_err().with_detail(name))?;
        out.push_str(&expanded);
        cursor = end + 1;
    }
    out.push_str(&value[cursor..]);
    Ok(out)
}
