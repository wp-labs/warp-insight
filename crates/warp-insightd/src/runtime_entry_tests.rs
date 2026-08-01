use super::{
    init_config_message, parse_command, resolve_config_root, resolve_exec_bin_from,
    resolve_requested_config_root, run_from_args, sync_runtime_identity, usage_message,
};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use warp_insight_contracts::agent_config::{
    AgentConfigContract, AgentSection, ControlPlaneSection, ExecutionSection, PathsSection,
};
use warp_insight_contracts::state_exec::{AgentRuntimeState, RuntimeMode};

fn temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("duration")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("warp-insightd-lib-{name}-{suffix}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn resolve_exec_bin_uses_env_override_when_present() {
    let root = temp_dir("override");
    let current_exe = root.join("bin").join("warp-insightd");
    let override_path = root.join("custom").join("warp-insight-exec");
    fs::create_dir_all(current_exe.parent().expect("current_exe parent"))
        .expect("create current_exe parent");
    fs::create_dir_all(override_path.parent().expect("override parent"))
        .expect("create override parent");
    fs::write(&override_path, b"#!/bin/sh\n").expect("write override");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&override_path)
            .expect("override metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&override_path, perms).expect("set override permissions");
    }

    let resolved = resolve_exec_bin_from(&current_exe, Some(Path::new(&override_path)), None)
        .expect("resolve");

    assert_eq!(resolved, override_path);
}

#[cfg(unix)]
#[test]
fn resolve_exec_bin_uses_path_when_sibling_is_missing() {
    let root = temp_dir("path-fallback");
    let current_exe = root.join("bin").join("warp-insightd");
    let path_dir = root.join("path-bin");
    let path_exec = path_dir.join("warp-insight-exec");
    fs::create_dir_all(current_exe.parent().expect("current_exe parent"))
        .expect("create current_exe parent");
    fs::create_dir_all(&path_dir).expect("create path dir");
    fs::write(&path_exec, b"#!/bin/sh\n").expect("write path exec");
    let mut perms = fs::metadata(&path_exec)
        .expect("path exec metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path_exec, perms).expect("set path exec permissions");

    let path_env = std::ffi::OsString::from(path_dir.display().to_string());
    let resolved = resolve_exec_bin_from(&current_exe, None, Some(path_env.as_os_str()))
        .expect("resolve via path");

    assert_eq!(resolved, path_exec);
}

#[cfg(unix)]
#[test]
fn resolve_exec_bin_prefers_sibling_before_path() {
    let root = temp_dir("sibling-before-path");
    let current_exe = root.join("bin").join("warp-insightd");
    let sibling = root.join("bin").join("warp-insight-exec");
    let path_dir = root.join("path-bin");
    let path_exec = path_dir.join("warp-insight-exec");
    fs::create_dir_all(current_exe.parent().expect("current_exe parent"))
        .expect("create current_exe parent");
    fs::create_dir_all(&path_dir).expect("create path dir");
    fs::write(&sibling, b"#!/bin/sh\n").expect("write sibling");
    fs::write(&path_exec, b"#!/bin/sh\n").expect("write path exec");
    for path in [&sibling, &path_exec] {
        let mut perms = fs::metadata(path).expect("exec metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("set exec permissions");
    }

    let path_env = std::ffi::OsString::from(path_dir.display().to_string());
    let resolved = resolve_exec_bin_from(&current_exe, None, Some(path_env.as_os_str()))
        .expect("resolve sibling before path");

    assert_eq!(resolved, sibling);
}

#[test]
fn parse_command_defaults_to_run() {
    assert_eq!(
        parse_command(Vec::<&str>::new()).expect("parse"),
        super::ParsedArgs {
            command: super::Command::Run,
            config_dir: None,
        }
    );
}

#[test]
fn parse_command_accepts_help_variants() {
    assert_eq!(
        parse_command(["help"]).expect("parse"),
        super::ParsedArgs {
            command: super::Command::Help,
            config_dir: None,
        }
    );
    assert_eq!(
        parse_command(["--help"]).expect("parse"),
        super::ParsedArgs {
            command: super::Command::Help,
            config_dir: None,
        }
    );
    assert_eq!(
        parse_command(["-h"]).expect("parse"),
        super::ParsedArgs {
            command: super::Command::Help,
            config_dir: None,
        }
    );
}

#[test]
fn parse_command_accepts_init_config() {
    assert_eq!(
        parse_command(["init-config"]).expect("parse"),
        super::ParsedArgs {
            command: super::Command::InitConfig { stdout_only: false },
            config_dir: None,
        }
    );
}

#[test]
fn parse_command_accepts_init_config_stdout() {
    assert_eq!(
        parse_command(["init-config", "--stdout"]).expect("parse"),
        super::ParsedArgs {
            command: super::Command::InitConfig { stdout_only: true },
            config_dir: None,
        }
    );
}

#[test]
fn parse_command_accepts_global_config_dir() {
    assert_eq!(
        parse_command(["--config-dir", "conf", "init-config"]).expect("parse"),
        super::ParsedArgs {
            command: super::Command::InitConfig { stdout_only: false },
            config_dir: Some(PathBuf::from("conf")),
        }
    );
}

#[test]
fn parse_command_accepts_init_config_stdout_after_config_dir() {
    assert_eq!(
        parse_command(["init-config", "--config-dir", "conf", "--stdout"]).expect("parse"),
        super::ParsedArgs {
            command: super::Command::InitConfig { stdout_only: true },
            config_dir: Some(PathBuf::from("conf")),
        }
    );
}

#[test]
fn parse_command_rejects_unknown_command() {
    let err = parse_command(["bad-command"]).expect_err("unknown command");
    assert!(err.to_string().contains("unknown argument or command"));
    assert!(err.to_string().contains("--config-dir"));
}

#[test]
fn parse_command_rejects_missing_config_dir_value() {
    let err = parse_command(["--config-dir"]).expect_err("missing config dir");
    assert!(err.to_string().contains("missing value for --config-dir"));
}

#[test]
fn parse_command_rejects_config_dir_followed_by_option() {
    let err =
        parse_command(["init-config", "--config-dir", "--stdout"]).expect_err("invalid config dir");
    assert!(err.to_string().contains("missing value for --config-dir"));
}

#[test]
fn parse_command_rejects_stdout_without_init_config() {
    let err = parse_command(["--stdout"]).expect_err("stdout without init-config");
    assert!(
        err.to_string()
            .contains("--stdout is only supported with init-config")
    );
}

#[test]
fn run_from_args_init_config_creates_config_without_exec_bin() {
    let root = temp_dir("cli-init-config");

    run_from_args(root.clone(), ["init-config"]).expect("init config command");

    assert!(root.join("warp-insightd").join("insightd.toml").exists());
}

#[test]
fn run_from_args_init_config_honors_custom_config_dir() {
    let root = temp_dir("cli-init-config-custom-dir");

    run_from_args(root.clone(), ["init-config", "--config-dir", "conf"])
        .expect("init config with custom dir");

    assert!(root.join("conf").join("insightd.toml").exists());
}

#[test]
fn init_config_message_mentions_config_directory_when_created() {
    let path = Path::new("/tmp/project/warp-insightd/insightd.toml");

    let message = init_config_message(path, true);

    assert!(message.contains("initialized config directory"));
    assert!(message.contains("/tmp/project/warp-insightd"));
    assert!(message.contains("/tmp/project/warp-insightd/insightd.toml"));
}

#[test]
fn usage_message_lists_supported_commands() {
    let message = usage_message();

    assert!(message.contains("warp-insightd init-config [--stdout]"));
    assert!(message.contains("warp-insightd help"));
    assert!(message.contains("Show this help message"));
    assert!(message.contains("--config-dir <path>"));
}

#[test]
fn init_config_message_mentions_existing_config_file_and_directory() {
    let path = Path::new("/tmp/project/warp-insightd/insightd.toml");

    let message = init_config_message(path, false);

    assert!(message.contains("config file already exists"));
    assert!(message.contains("/tmp/project/warp-insightd"));
    assert!(message.contains("/tmp/project/warp-insightd/insightd.toml"));
}

#[test]
fn run_from_args_init_config_stdout_does_not_create_config_file() {
    let root = temp_dir("cli-init-config-stdout");

    run_from_args(root.clone(), ["init-config", "--stdout"]).expect("init config stdout");

    assert!(!root.join("warp-insightd").join("insightd.toml").exists());
}

#[test]
fn resolve_config_root_prefers_visible_directory() {
    let root = temp_dir("config-root-visible");
    fs::create_dir_all(root.join("warp-insightd")).expect("create visible config dir");
    fs::create_dir_all(root.join(".warp-insightd")).expect("create legacy config dir");
    fs::write(
        root.join("warp-insightd").join("insightd.toml"),
        "schema_version = \"v1\"\n",
    )
    .expect("write visible config");
    fs::write(
        root.join(".warp-insightd").join("insightd.toml"),
        "schema_version = \"v1\"\n",
    )
    .expect("write legacy config");

    assert_eq!(resolve_config_root(&root), root.join("warp-insightd"));
}

#[test]
fn resolve_config_root_falls_back_to_legacy_hidden_directory() {
    let root = temp_dir("config-root-legacy");
    fs::create_dir_all(root.join(".warp-insightd")).expect("create legacy config dir");
    fs::write(
        root.join(".warp-insightd").join("insightd.toml"),
        "schema_version = \"v1\"\n",
    )
    .expect("write legacy config");

    assert_eq!(resolve_config_root(&root), root.join(".warp-insightd"));
}

#[test]
fn resolve_config_root_accepts_legacy_agent_toml_in_visible_directory() {
    let root = temp_dir("config-root-visible-legacy-file");
    fs::create_dir_all(root.join("warp-insightd")).expect("create visible config dir");
    fs::write(
        root.join("warp-insightd").join("agent.toml"),
        "schema_version = \"v1\"\n",
    )
    .expect("write visible legacy config");

    assert_eq!(resolve_config_root(&root), root.join("warp-insightd"));
}

#[test]
fn resolve_config_root_accepts_legacy_agent_toml_in_hidden_directory() {
    let root = temp_dir("config-root-hidden-legacy-file");
    fs::create_dir_all(root.join(".warp-insightd")).expect("create hidden config dir");
    fs::write(
        root.join(".warp-insightd").join("agent.toml"),
        "schema_version = \"v1\"\n",
    )
    .expect("write hidden legacy config");

    assert_eq!(resolve_config_root(&root), root.join(".warp-insightd"));
}

#[test]
fn resolve_config_root_defaults_to_visible_directory_when_missing() {
    let root = temp_dir("config-root-default");

    assert_eq!(resolve_config_root(&root), root.join("warp-insightd"));
}

#[test]
fn resolve_config_root_prefers_legacy_when_visible_dir_has_no_config_file() {
    let root = temp_dir("config-root-visible-empty");
    fs::create_dir_all(root.join("warp-insightd")).expect("create visible config dir");
    fs::create_dir_all(root.join(".warp-insightd")).expect("create legacy config dir");
    fs::write(
        root.join(".warp-insightd").join("insightd.toml"),
        "schema_version = \"v1\"\n",
    )
    .expect("write legacy config");

    assert_eq!(resolve_config_root(&root), root.join(".warp-insightd"));
}

#[test]
fn resolve_requested_config_root_uses_relative_override_from_root() {
    let root = temp_dir("requested-config-root-relative");

    assert_eq!(
        resolve_requested_config_root(&root, Some(Path::new("conf"))),
        root.join("conf")
    );
}

#[test]
fn resolve_requested_config_root_preserves_absolute_override() {
    let root = temp_dir("requested-config-root-absolute");
    let absolute = root.join("external-conf");

    assert_eq!(
        resolve_requested_config_root(&root, Some(&absolute)),
        absolute
    );
}

#[test]
fn resolve_exec_bin_rejects_missing_candidate() {
    let root = temp_dir("missing");
    let current_exe = root.join("bin").join("warp-insightd");
    fs::create_dir_all(current_exe.parent().expect("current_exe parent"))
        .expect("create current_exe parent");

    let err =
        resolve_exec_bin_from(&current_exe, None, None).expect_err("missing exec should fail");
    assert!(err.to_string().contains("warp-insight-exec was not found"));
}

#[cfg(unix)]
#[test]
fn resolve_exec_bin_rejects_non_executable_file() {
    let root = temp_dir("not-executable");
    let current_exe = root.join("bin").join("warp-insightd");
    let candidate = root.join("bin").join("warp-insight-exec");
    fs::create_dir_all(current_exe.parent().expect("current_exe parent"))
        .expect("create current_exe parent");
    fs::write(&candidate, b"#!/bin/sh\n").expect("write candidate");
    let mut perms = fs::metadata(&candidate)
        .expect("candidate metadata")
        .permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&candidate, perms).expect("set candidate permissions");

    let err = resolve_exec_bin_from(&current_exe, None, None)
        .expect_err("non executable candidate should fail");

    assert!(err.to_string().contains("not executable"));
}

#[test]
fn sync_runtime_identity_prefers_config_identity_when_present() {
    let mut runtime = AgentRuntimeState::new(
        "local-agent".to_string(),
        "local-instance".to_string(),
        "0.1.0".to_string(),
        RuntimeMode::Normal,
        "2026-04-12T10:00:00Z".to_string(),
    );
    let config = AgentConfigContract::new(
        AgentSection {
            agent_id: Some("agent-from-config".to_string()),
            environment_id: Some("prod".to_string()),
            instance_name: Some("instance-from-config".to_string()),
        },
        ControlPlaneSection {
            enabled: false,
            endpoint: None,
            enrollment_token: None,
            credential_request: None,
            credential_id: None,
            bearer_token: None,
            credential_expires_at: None,
            tls_mode: None,
            trust_bundle: None,
            auth_mode: None,
        },
        PathsSection {
            root_dir: ".".to_string(),
            run_dir: "run".to_string(),
            state_dir: "state".to_string(),
            log_dir: "log".to_string(),
        },
        ExecutionSection {
            max_running_actions: 1,
            cancel_grace_ms: 5_000,
            default_stdout_limit_bytes: 1,
            default_stderr_limit_bytes: 1,
        },
    );

    sync_runtime_identity(&mut runtime, &config).expect("sync identity");

    assert_eq!(runtime.agent_id, "agent-from-config");
    assert_eq!(runtime.instance_id, "instance-from-config");
}

#[test]
fn sync_runtime_identity_rejects_config_agent_id_conflicting_with_enrolled_identity() {
    let mut runtime = AgentRuntimeState::new(
        "agent-a".to_string(),
        "instance-a".to_string(),
        "0.1.0".to_string(),
        RuntimeMode::Normal,
        "2026-04-12T10:00:00Z".to_string(),
    );
    let config = AgentConfigContract::new(
        AgentSection {
            agent_id: Some("agent-b".to_string()),
            environment_id: None,
            instance_name: Some("instance-from-config".to_string()),
        },
        ControlPlaneSection {
            enabled: false,
            endpoint: None,
            enrollment_token: None,
            credential_request: None,
            credential_id: None,
            bearer_token: None,
            credential_expires_at: None,
            tls_mode: None,
            trust_bundle: None,
            auth_mode: None,
        },
        PathsSection {
            root_dir: ".".to_string(),
            run_dir: "run".to_string(),
            state_dir: "state".to_string(),
            log_dir: "log".to_string(),
        },
        ExecutionSection {
            max_running_actions: 1,
            cancel_grace_ms: 5_000,
            default_stdout_limit_bytes: 1,
            default_stderr_limit_bytes: 1,
        },
    );

    let err = sync_runtime_identity(&mut runtime, &config).expect_err("conflict rejected");

    assert!(err.to_string().contains("conflicts"));
    assert_eq!(runtime.agent_id, "agent-a");
}
