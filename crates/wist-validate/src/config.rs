//! Config validation entrypoints.

use std::collections::HashSet;

use wist_contracts::SCHEMA_VERSION_V1;
use wist_contracts::agent_config::{AgentConfigContract, LogsSection};

use crate::{ValidationError, require_non_empty};

pub fn validate_config(contract: &AgentConfigContract) -> Result<(), ValidationError> {
    if contract.schema_version != SCHEMA_VERSION_V1 {
        return Err(ValidationError::new("invalid_schema_version"));
    }
    if contract.control_plane.enabled {
        let endpoint = contract
            .control_plane
            .endpoint
            .as_deref()
            .unwrap_or_default();
        require_non_empty(endpoint, "missing_control_plane_endpoint")?;
    }

    require_non_empty(&contract.paths.root_dir, "missing_root_dir")?;
    require_non_empty(&contract.paths.run_dir, "missing_run_dir")?;
    require_non_empty(&contract.paths.state_dir, "missing_state_dir")?;
    require_non_empty(&contract.paths.log_dir, "missing_log_dir")?;

    if contract.execution.max_running_actions == 0 {
        return Err(ValidationError::new("invalid_max_running_actions"));
    }
    if contract.execution.max_running_actions != 1 {
        return Err(ValidationError::new("unsupported_max_running_actions"));
    }
    if contract.execution.cancel_grace_ms == 0 {
        return Err(ValidationError::new("invalid_cancel_grace_ms"));
    }
    if contract.execution.default_stdout_limit_bytes == 0 {
        return Err(ValidationError::new("invalid_stdout_limit"));
    }
    if contract.execution.default_stderr_limit_bytes == 0 {
        return Err(ValidationError::new("invalid_stderr_limit"));
    }
    if !contract.discovery.host_enabled
        && !contract.discovery.network_enabled
        && !contract.discovery.endpoint_enabled
        && !contract.discovery.process_enabled
        && !contract.discovery.container_enabled
    {
        return Err(ValidationError::new("missing_discovery_probe"));
    }

    validate_logs_section(&contract.telemetry.logs)?;

    Ok(())
}

fn validate_logs_section(logs: &LogsSection) -> Result<(), ValidationError> {
    if logs.in_memory_buffer_bytes == 0 {
        return Err(ValidationError::new("invalid_logs_buffer_bytes"));
    }
    require_non_empty(&logs.spool_dir, "missing_logs_spool_dir")?;
    require_non_empty(&logs.output.kind, "missing_logs_output_kind")?;
    match logs.output.kind.as_str() {
        "file" => {
            require_non_empty(&logs.output.file.path, "missing_logs_output_file_path")?;
        }
        "tcp" => {
            require_non_empty(&logs.output.tcp.addr, "missing_logs_output_tcp_addr")?;
            if logs.output.tcp.port == 0 {
                return Err(ValidationError::new("invalid_logs_output_tcp_port"));
            }
            match logs.output.tcp.framing.as_str() {
                "line" | "len" => {}
                _ => return Err(ValidationError::new("invalid_logs_output_tcp_framing")),
            }
        }
        _ => return Err(ValidationError::new("invalid_logs_output_kind")),
    }
    match logs.spool_over_limit.as_str() {
        "pause" | "drop_oldest" => {}
        _ => return Err(ValidationError::new("invalid_logs_spool_over_limit")),
    }
    for (value, code) in [
        (logs.max_line_bytes, "invalid_logs_max_line_bytes"),
        (
            logs.max_read_bytes_per_tick,
            "invalid_logs_max_read_bytes_per_tick",
        ),
        (logs.max_lines_per_tick, "invalid_logs_max_lines_per_tick"),
        (logs.spool_max_bytes, "invalid_logs_spool_max_bytes"),
    ] {
        if value == 0 {
            return Err(ValidationError::new(code));
        }
    }
    let mut input_ids = HashSet::new();
    for input in &logs.file_inputs {
        require_non_empty(&input.input_id, "missing_log_input_id")?;
        require_non_empty(&input.path, "missing_log_input_path")?;
        if !input_ids.insert(input.input_id.as_str()) {
            return Err(ValidationError::new("duplicate_log_input_id"));
        }
        match input.startup_position.as_str() {
            "head" | "tail" => {}
            _ => return Err(ValidationError::new("invalid_log_startup_position")),
        }
        match input.multiline_mode.as_str() {
            "none" | "indented" => {}
            _ => return Err(ValidationError::new("invalid_log_multiline_mode")),
        }
    }

    Ok(())
}
