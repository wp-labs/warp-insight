//! Process discovery probe skeleton.

#[cfg(target_os = "linux")]
use std::fs;
use std::io;
#[cfg(unix)]
use std::process::Command;

use std::collections::BTreeMap;

use warp_insight_contracts::discovery::{DiscoveredResource, DiscoveredTarget, DiscoveryOrigin};
use warp_insight_shared::time::now_rfc3339;

use crate::process_control::process_identity;

use super::host::default_host_id;
use super::{DiscoveryProbe, DiscoveryProbeError, DiscoverySourceKind, ProbeOutput};

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
pub struct ProcessDiscoveryProbe;

impl DiscoveryProbe for ProcessDiscoveryProbe {
    fn name(&self) -> &'static str {
        "process"
    }

    fn source(&self) -> DiscoverySourceKind {
        DiscoverySourceKind::LocalRuntime
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(30)
    }

    fn refresh(&self, _now: std::time::SystemTime) -> Result<ProbeOutput, DiscoveryProbeError> {
        let discovered_at = now_rfc3339();
        let host_id = default_host_id();
        let source = self.source().as_str().to_string();
        let observed_at = discovered_at.clone();
        let origin_id = format!("{}:{}:{}", source, self.name(), observed_at);
        let processes = list_processes().map_err(|err| {
            DiscoveryProbeError::new(
                self.name(),
                self.source(),
                format!("process discovery failed: {err}"),
            )
        })?;

        let mut resources = Vec::with_capacity(processes.len());
        let mut targets = Vec::with_capacity(processes.len());
        for process in processes {
            let resource_id = process.resource_id(&host_id);
            let target_id = process.target_id(&host_id);
            let mut attributes = BTreeMap::new();
            attributes.insert("process.pid".to_string(), process.pid.to_string());
            if let Some(name) = &process.name {
                attributes.insert("process.executable.name".to_string(), name.clone());
            }
            let mut execution_hints = BTreeMap::new();
            execution_hints.insert("process.pid".to_string(), process.pid.to_string());
            if let Some(identity) = &process.identity {
                execution_hints.insert("process.identity".to_string(), identity.clone());
            }
            if process.identity.is_none() {
                execution_hints.insert(
                    "discovery.identity_strength".to_string(),
                    "weak".to_string(),
                );
            }
            if process.identity_unavailable {
                execution_hints.insert(
                    "discovery.identity_status".to_string(),
                    "unavailable".to_string(),
                );
            }

            resources.push(DiscoveredResource {
                resource_id: resource_id.clone(),
                kind: "process".to_string(),
                origin_idx: 0,
                attributes,
                discovered_at: discovered_at.clone(),
                last_seen_at: discovered_at.clone(),
                health: "healthy".to_string(),
                source: self.name().to_string(),
            });
            targets.push(DiscoveredTarget {
                target_id,
                kind: "process".to_string(),
                origin_idx: 0,
                resource_ref: resource_id,
                execution_hints,
                state: "active".to_string(),
            });
        }

        Ok(ProbeOutput {
            probe: self.name().to_string(),
            source: self.source(),
            refreshed_at: discovered_at,
            origin: DiscoveryOrigin {
                origin_id,
                probe: self.name().to_string(),
                source,
                observed_at,
            },
            resources,
            targets,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Observed", module = "Observed.Entity")]
struct ObservedProcess {
    pid: u32,
    identity: Option<String>,
    name: Option<String>,
    identity_unavailable: bool,
}

impl ObservedProcess {
    fn resource_id(&self, host_id: &str) -> String {
        match &self.identity {
            Some(identity) => format!("{host_id}:pid:{}:{identity}", self.pid),
            None => format!("{host_id}:pid:{}", self.pid),
        }
    }

    fn target_id(&self, host_id: &str) -> String {
        match &self.identity {
            Some(identity) => format!("{host_id}:pid:{}:{identity}:process", self.pid),
            None => format!("{host_id}:pid:{}:process", self.pid),
        }
    }
}

#[cfg(target_os = "linux")]
fn list_processes() -> io::Result<Vec<ObservedProcess>> {
    let mut processes = Vec::new();
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Ok(pid) = file_name.parse::<u32>() else {
            continue;
        };
        let name = fs::read_to_string(format!("/proc/{pid}/comm"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let (identity, identity_unavailable) = match process_identity(pid) {
            Ok(identity) => (identity, false),
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                ) =>
            {
                (None, true)
            }
            Err(err) => return Err(err),
        };
        processes.push(ObservedProcess {
            pid,
            identity,
            name,
            identity_unavailable,
        });
    }
    Ok(processes)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn list_processes() -> io::Result<Vec<ObservedProcess>> {
    let output = Command::new("ps").args(["-axo", "pid=,comm="]).output()?;
    if !output.status.success() {
        return Err(io::Error::other("ps did not exit successfully"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut processes = Vec::new();
    for line in stdout.lines() {
        let Some((pid, name)) = parse_unix_ps_process_line(line) else {
            continue;
        };
        let (identity, identity_unavailable) = match process_identity(pid) {
            Ok(identity) => (identity, false),
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                ) =>
            {
                (None, true)
            }
            Err(err) => return Err(err),
        };
        processes.push(ObservedProcess {
            pid,
            identity,
            name,
            identity_unavailable,
        });
    }
    Ok(processes)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn parse_unix_ps_process_line(line: &str) -> Option<(u32, Option<String>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let first_non_pid = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let pid_text = &trimmed[..first_non_pid];
    let pid = pid_text.parse::<u32>().ok()?;
    let name = trimmed[first_non_pid..].trim();
    let name = if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    };
    Some((pid, name))
}

#[cfg(not(unix))]
fn list_processes() -> io::Result<Vec<ObservedProcess>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::ObservedProcess;
    #[cfg(all(unix, not(target_os = "linux")))]
    use super::parse_unix_ps_process_line;

    #[test]
    fn process_identity_uses_identity_when_present() {
        let observed = ObservedProcess {
            pid: 42,
            identity: Some("linux_proc_start:123".to_string()),
            name: Some("demo".to_string()),
            identity_unavailable: false,
        };

        assert_eq!(
            observed.resource_id("host-1"),
            "host-1:pid:42:linux_proc_start:123"
        );
        assert_eq!(
            observed.target_id("host-1"),
            "host-1:pid:42:linux_proc_start:123:process"
        );
    }

    #[test]
    fn process_identity_falls_back_to_pid_when_missing() {
        let observed = ObservedProcess {
            pid: 42,
            identity: None,
            name: Some("demo".to_string()),
            identity_unavailable: false,
        };

        assert_eq!(observed.resource_id("host-1"), "host-1:pid:42");
        assert_eq!(observed.target_id("host-1"), "host-1:pid:42:process");
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    #[test]
    fn parse_unix_ps_process_line_keeps_command_with_spaces() {
        let parsed = parse_unix_ps_process_line(
            "123 /Applications/Microsoft Outlook.app/Contents/MacOS/Microsoft Outlook",
        );

        assert_eq!(
            parsed,
            Some((
                123,
                Some(
                    "/Applications/Microsoft Outlook.app/Contents/MacOS/Microsoft Outlook"
                        .to_string(),
                ),
            ))
        );
    }
}
