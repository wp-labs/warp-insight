use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use crate::daemon;
use crate::enrollment::is_registered_agent_id;
use crate::self_observability;
use crate::state_store;
use warp_insight_shared::paths::{INSIGHTD_CONFIG_FILE, LEGACY_AGENT_CONFIG_FILE};

const CONFIG_DIR: &str = "warp-insightd";
const LEGACY_CONFIG_DIR: &str = ".warp-insightd";

pub(crate) async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::current_dir()?;
    run_from_args_async(root, std::env::args_os().skip(1)).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Help,
    Run,
    InitConfig { stdout_only: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedArgs {
    command: Command,
    config_dir: Option<PathBuf>,
}

async fn run_from_args_async<I, S>(root: PathBuf, args: I) -> Result<(), Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let parsed = parse_command(args)?;
    match parsed.command {
        Command::Help => {
            print!("{}", usage_message());
            Ok(())
        }
        Command::Run => run_daemon(root, parsed.config_dir.as_deref()).await,
        Command::InitConfig { stdout_only: true } => {
            print!("{}", crate::config_runtime::default_config_template());
            Ok(())
        }
        Command::InitConfig { stdout_only: false } => {
            let config_root = resolve_requested_config_root(&root, parsed.config_dir.as_deref());
            let ensured = crate::config_runtime::ensure_default_config(&config_root)?;
            println!("{}", init_config_message(&ensured.path, ensured.created));
            Ok(())
        }
    }
}

#[cfg(test)]
fn run_from_args<I, S>(root: PathBuf, args: I) -> Result<(), Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run_from_args_async(root, args))
}

fn parse_command<I, S>(args: I) -> io::Result<ParsedArgs>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let mut command = Command::Run;
    let mut command_explicit = false;
    let mut config_dir = None;
    let mut index = 0usize;

    while index < args.len() {
        let arg = &args[index];
        if arg == "help" || arg == "--help" || arg == "-h" {
            if command_explicit {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "help cannot be combined with another command",
                ));
            }
            command = Command::Help;
            command_explicit = true;
            index += 1;
            continue;
        }
        if arg == "--config-dir" {
            let value = config_dir_value(&args, index + 1)?;
            config_dir = Some(PathBuf::from(value));
            index += 2;
            continue;
        }
        if arg == "--stdout" {
            match command {
                Command::InitConfig { .. } => {
                    command = Command::InitConfig { stdout_only: true };
                    index += 1;
                    continue;
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--stdout is only supported with init-config",
                    ));
                }
            }
        }
        if arg == "init-config" {
            if command_explicit {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "multiple commands are not supported",
                ));
            }
            command = Command::InitConfig { stdout_only: false };
            command_explicit = true;
            index += 1;
            continue;
        }

        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unknown argument or command: {} (supported: help | init-config [--stdout] | --config-dir <path>)",
                PathBuf::from(arg).display()
            ),
        ));
    }

    Ok(ParsedArgs {
        command,
        config_dir,
    })
}

fn config_dir_value(args: &[OsString], value_index: usize) -> io::Result<&OsString> {
    let Some(value) = args.get(value_index) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing value for --config-dir",
        ));
    };
    if looks_like_option(value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing value for --config-dir",
        ));
    }
    Ok(value)
}

fn looks_like_option(value: &OsString) -> bool {
    value
        .to_str()
        .is_some_and(|text| matches!(text, "--help" | "-h" | "--stdout" | "--config-dir"))
}

async fn run_daemon(
    root: PathBuf,
    config_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_root = match config_dir {
        Some(path) => resolve_requested_config_root(&root, Some(path)),
        None => resolve_config_root(&root),
    };
    let mut config = crate::config_runtime::load_or_init(&config_root)?;
    let root_dir = PathBuf::from(&config.paths.root_dir);
    let run_dir = PathBuf::from(&config.paths.run_dir);
    let state_dir = PathBuf::from(&config.paths.state_dir);
    let log_dir = PathBuf::from(&config.paths.log_dir);

    crate::bootstrap::initialize(&root_dir, &run_dir, &state_dir, &log_dir)?;
    let config_path = crate::config_runtime::resolve_config_path(&config_root);
    crate::enrollment::ensure_enrolled_with_config_path(&mut config, &state_dir, &config_path)
        .await?;
    initialize_runtime_state(&state_dir, &config)?;
    self_observability::register();
    let exec_bin = resolve_exec_bin()?;
    let loop_ctx = daemon::DaemonLoop {
        config: &config,
        exec_bin: &exec_bin,
    };

    if std::env::var("WARP_INSIGHTD_RUN_ONCE").ok().as_deref() == Some("1") {
        let snapshot = daemon::run_once_async(&loop_ctx).await?;
        self_observability::emit(&snapshot);
        return Ok(());
    }

    daemon::run_forever_async(loop_ctx).await?;
    Ok(())
}

fn init_config_message(path: &Path, created: bool) -> String {
    let config_dir = path
        .parent()
        .map(|value| value.display().to_string())
        .unwrap_or_else(|| CONFIG_DIR.to_string());
    if created {
        format!(
            "initialized config directory {} and wrote config file {}",
            config_dir,
            path.display()
        )
    } else {
        format!(
            "config file already exists at {} (config directory: {})",
            path.display(),
            config_dir
        )
    }
}

fn usage_message() -> &'static str {
    concat!(
        "Usage:\n",
        "  warp-insightd [--config-dir <path>]\n",
        "  warp-insightd help\n",
        "  warp-insightd init-config [--stdout] [--config-dir <path>]\n",
        "\n",
        "Commands:\n",
        "  help                 Show this help message.\n",
        "  init-config          Initialize config directory warp-insightd/ and write insightd.toml.\n",
        "  init-config --stdout Print the default config template to stdout.\n",
        "\n",
        "Options:\n",
        "  --config-dir <path>  Use the specified config directory. Relative paths are resolved from the current working directory.\n",
    )
}

fn resolve_requested_config_root(root: &Path, config_dir: Option<&Path>) -> PathBuf {
    match config_dir {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => root.join(path),
        None => root.join(CONFIG_DIR),
    }
}

fn resolve_config_root(root: &Path) -> PathBuf {
    let preferred = root.join(CONFIG_DIR);
    if config_file_exists(&preferred) {
        return preferred;
    }

    let legacy = root.join(LEGACY_CONFIG_DIR);
    if config_file_exists(&legacy) {
        return legacy;
    }

    preferred
}

fn config_file_exists(config_root: &Path) -> bool {
    config_root.join(INSIGHTD_CONFIG_FILE).is_file()
        || config_root.join(LEGACY_AGENT_CONFIG_FILE).is_file()
}

fn initialize_runtime_state(
    state_dir: &Path,
    config: &warp_insight_contracts::agent_config::AgentConfigContract,
) -> io::Result<()> {
    let runtime_path = state_store::agent_runtime::path_for(state_dir);
    let mut runtime_state = state_store::agent_runtime::load_or_default(&runtime_path)?;
    sync_runtime_identity(&mut runtime_state, config)?;
    state_store::agent_runtime::store(&runtime_path, &runtime_state)?;

    let queue_path = state_store::execution_queue::path_for(state_dir);
    let queue_state = state_store::execution_queue::load_or_default(&queue_path)?;
    state_store::execution_queue::store(&queue_path, &queue_state)?;
    Ok(())
}

fn resolve_exec_bin() -> io::Result<PathBuf> {
    let env_override = std::env::var_os("WARP_INSIGHT_EXEC_BIN").map(PathBuf::from);
    let path_env = std::env::var_os("PATH");
    let current_exe = std::env::current_exe()?;
    resolve_exec_bin_from(&current_exe, env_override.as_deref(), path_env.as_deref())
}

fn resolve_exec_bin_from(
    current_exe: &Path,
    env_override: Option<&Path>,
    path_env: Option<&std::ffi::OsStr>,
) -> io::Result<PathBuf> {
    if let Some(path) = env_override {
        return validate_exec_bin(path.to_path_buf(), "WARP_INSIGHT_EXEC_BIN");
    }

    let sibling = current_exe.with_file_name("warp-insight-exec");
    if sibling.exists() {
        return validate_exec_bin(sibling, "current executable sibling");
    }

    if let Some(path_env) = path_env {
        for dir in std::env::split_paths(path_env) {
            let candidate = dir.join("warp-insight-exec");
            if candidate.exists() {
                return validate_exec_bin(candidate, "PATH");
            }
        }
    }

    validate_exec_bin(sibling, "current executable sibling")
}

fn validate_exec_bin(path: PathBuf, origin: &str) -> io::Result<PathBuf> {
    let metadata = std::fs::metadata(&path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "warp-insight-exec was not found via {origin}: {} ({err})",
                path.display()
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(io::Error::other(format!(
            "warp-insight-exec path is not a file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("warp-insight-exec is not executable: {}", path.display()),
            ));
        }
    }
    Ok(path)
}

fn sync_runtime_identity(
    runtime_state: &mut warp_insight_contracts::state_exec::AgentRuntimeState,
    config: &warp_insight_contracts::agent_config::AgentConfigContract,
) -> io::Result<()> {
    if let Some(agent_id) = config
        .agent
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        // Never silently rebind an already-enrolled identity to a different
        // config id while keeping its credentials — that would let state carry
        // a cross-agent credential. Fail loudly and ask the operator to resolve.
        if is_registered_agent_id(&runtime_state.agent_id) && runtime_state.agent_id != agent_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "config agent.agent_id ({agent_id}) conflicts with enrolled runtime identity {}; remove agent.agent_id or clear the agent state to re-enroll",
                    runtime_state.agent_id
                ),
            ));
        }
        runtime_state.agent_id = agent_id.to_string();
    }
    if let Some(instance_id) = config
        .agent
        .instance_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        runtime_state.instance_id = instance_id.to_string();
    }
    Ok(())
}

#[cfg(test)]
#[path = "runtime_entry_tests.rs"]
mod tests;
