//! Runtime config loading and mode selection.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use orion_error::{conversion::ToStructError, prelude::*};

use crate::fs_async::write_bytes_atomic_async;
use wist_contracts::agent_config::{AgentConfigContract, LogFileInputsFile};
use wist_shared::fs::write_bytes_atomic;
use wist_shared::paths::{AGENTD_CONFIG_FILE, LEGACY_AGENT_CONFIG_FILE};
use wist_validate::config::validate_config;

#[path = "config_runtime_support.rs"]
mod support;

use support::{
    absolutize, default_file_config_text, expand_env_contract, expand_string, resolve_paths,
};

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Config")]
pub struct EnsuredConfigFile {
    pub path: PathBuf,
    pub created: bool,
}

pub use crate::error::{ConfigError, ConfigReason};

pub fn load_or_init(config_root: &Path) -> Result<AgentConfigContract, ConfigError> {
    let ensured = ensure_default_config(config_root)?;
    load_from_path(&ensured.path)
}

pub fn ensure_default_config(config_root: &Path) -> Result<EnsuredConfigFile, ConfigError> {
    fs::create_dir_all(config_root).source_err(
        ConfigReason::Io,
        format!("create config dir {}", config_root.display()),
    )?;
    let config_path = resolve_config_path(config_root);
    let created = if config_path.exists() {
        false
    } else {
        let text = default_file_config_text();
        write_bytes_atomic(&config_path, text.as_bytes()).source_err(
            ConfigReason::Io,
            format!("write config {}", config_path.display()),
        )?;
        true
    };
    Ok(EnsuredConfigFile {
        path: config_path,
        created,
    })
}

pub fn default_config_template() -> String {
    default_file_config_text()
}

pub fn load_from_path(config_path: &Path) -> Result<AgentConfigContract, ConfigError> {
    let text = fs::read_to_string(config_path).source_err(
        ConfigReason::Io,
        format!("read config {}", config_path.display()),
    )?;
    let mut parsed = toml::from_str::<AgentConfigContract>(&text)
        .source_raw_err(ConfigReason::ParseToml, "parse config")?;
    load_file_inputs_from_task_file(&mut parsed, config_path)?;
    let env_resolved = expand_env_contract(parsed)?;
    let path_resolved = resolve_paths(env_resolved, config_path);
    validate_config(&path_resolved)
        .map_err(|err| ConfigReason::Validation.to_err().with_detail(err.code))?;
    Ok(path_resolved)
}

/// 若声明了 `[telemetry.logs] file_inputs_file`，从该外置任务清单加载 `file_inputs`。
///
/// 任务清单路径相对本配置文件解析（支持 `${ENV}` 展开），与内联
/// `[[telemetry.logs.file_inputs]]` 互斥：同时出现时拒绝加载，避免来源不明。
fn load_file_inputs_from_task_file(
    config: &mut AgentConfigContract,
    config_path: &Path,
) -> Result<(), ConfigError> {
    let Some(raw_path) = config.telemetry.logs.file_inputs_file.take() else {
        return Ok(());
    };
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
    let task_path = absolutize(config_dir, &expand_string(raw_path)?);
    let text = fs::read_to_string(&task_path).source_err(
        ConfigReason::Io,
        format!("read file_inputs_file {}", task_path.display()),
    )?;
    if !config.telemetry.logs.file_inputs.is_empty() {
        return Err(ConfigReason::Validation.to_err().with_detail(format!(
            "file_inputs_file {} conflicts with inline [[telemetry.logs.file_inputs]]; declare one or the other",
            task_path.display()
        )));
    }
    let tasks = toml::from_str::<LogFileInputsFile>(&text).source_raw_err(
        ConfigReason::ParseToml,
        format!("parse file_inputs_file {}", task_path.display()),
    )?;
    config.telemetry.logs.file_inputs = tasks.file_inputs;
    Ok(())
}

pub fn resolve_config_path(config_root: &Path) -> PathBuf {
    let preferred = config_root.join(AGENTD_CONFIG_FILE);
    if preferred.is_file() {
        return preferred;
    }

    let legacy = config_root.join(LEGACY_AGENT_CONFIG_FILE);
    if legacy.is_file() {
        return legacy;
    }

    preferred
}

pub async fn load_or_init_async(config_root: &Path) -> Result<AgentConfigContract, ConfigError> {
    let ensured = ensure_default_config_async(config_root).await?;
    load_from_path_async(&ensured.path).await
}

pub async fn ensure_default_config_async(
    config_root: &Path,
) -> Result<EnsuredConfigFile, ConfigError> {
    tokio::fs::create_dir_all(config_root).await.source_err(
        ConfigReason::Io,
        format!("create config dir {}", config_root.display()),
    )?;
    let config_path = resolve_config_path(config_root);
    let created = match tokio::fs::metadata(&config_path).await {
        Ok(_) => false,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let text = default_file_config_text();
            write_bytes_atomic_async(&config_path, text.as_bytes())
                .await
                .source_err(
                    ConfigReason::Io,
                    format!("write config {}", config_path.display()),
                )?;
            true
        }
        Err(err) => {
            return Err(StructError::builder(ConfigReason::Io)
                .detail(format!("stat config {}", config_path.display()))
                .source_std(err)
                .finish());
        }
    };
    Ok(EnsuredConfigFile {
        path: config_path,
        created,
    })
}

pub async fn load_from_path_async(config_path: &Path) -> Result<AgentConfigContract, ConfigError> {
    let text = tokio::fs::read_to_string(config_path).await.source_err(
        ConfigReason::Io,
        format!("read config {}", config_path.display()),
    )?;
    let mut parsed = toml::from_str::<AgentConfigContract>(&text)
        .source_raw_err(ConfigReason::ParseToml, "parse config")?;
    load_file_inputs_from_task_file_async(&mut parsed, config_path).await?;
    let env_resolved = expand_env_contract(parsed)?;
    let path_resolved = resolve_paths(env_resolved, config_path);
    validate_config(&path_resolved)
        .map_err(|err| ConfigReason::Validation.to_err().with_detail(err.code))?;
    Ok(path_resolved)
}

async fn load_file_inputs_from_task_file_async(
    config: &mut AgentConfigContract,
    config_path: &Path,
) -> Result<(), ConfigError> {
    let Some(raw_path) = config.telemetry.logs.file_inputs_file.take() else {
        return Ok(());
    };
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
    let task_path = absolutize(config_dir, &expand_string(raw_path)?);
    let text = tokio::fs::read_to_string(&task_path).await.source_err(
        ConfigReason::Io,
        format!("read file_inputs_file {}", task_path.display()),
    )?;
    if !config.telemetry.logs.file_inputs.is_empty() {
        return Err(ConfigReason::Validation.to_err().with_detail(format!(
            "file_inputs_file {} conflicts with inline [[telemetry.logs.file_inputs]]; declare one or the other",
            task_path.display()
        )));
    }
    let tasks = toml::from_str::<LogFileInputsFile>(&text).source_raw_err(
        ConfigReason::ParseToml,
        format!("parse file_inputs_file {}", task_path.display()),
    )?;
    config.telemetry.logs.file_inputs = tasks.file_inputs;
    Ok(())
}

#[cfg(test)]
#[path = "config_runtime_tests.rs"]
mod tests;
