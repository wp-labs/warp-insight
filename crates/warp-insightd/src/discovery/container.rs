//! Container discovery probe.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::Deserialize;

use warp_insight_contracts::discovery::{DiscoveredResource, DiscoveredTarget, DiscoveryOrigin};
use warp_insight_shared::time::now_rfc3339;

use super::{DiscoveryProbe, DiscoveryProbeError, DiscoverySourceKind, ProbeOutput};

pub struct ContainerDiscoveryProbe;

impl DiscoveryProbe for ContainerDiscoveryProbe {
    fn name(&self) -> &'static str {
        "container"
    }

    fn source(&self) -> DiscoverySourceKind {
        DiscoverySourceKind::LocalRuntime
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(30)
    }

    fn refresh(&self, _now: std::time::SystemTime) -> Result<ProbeOutput, DiscoveryProbeError> {
        let discovered_at = now_rfc3339();
        let source = self.source().as_str().to_string();
        let observed_at = discovered_at.clone();
        let origin_id = format!("{}:{}:{}", source, self.name(), observed_at);
        let containers =
            discover_containers_in_roots(&default_container_runtime_roots()).map_err(|err| {
                DiscoveryProbeError::new(
                    self.name(),
                    self.source(),
                    format!("container discovery failed: {err}"),
                )
            })?;

        let mut resources = Vec::with_capacity(containers.len());
        let mut targets = Vec::with_capacity(containers.len());
        for container in containers {
            let resource_id = container.container_id.clone();
            let mut attributes = BTreeMap::new();
            attributes.insert("container.id".to_string(), container.container_id.clone());
            attributes.insert("container.name".to_string(), container.name.clone());
            attributes.insert(
                "container.runtime".to_string(),
                container.runtime.to_string(),
            );
            let mut execution_hints = BTreeMap::new();
            execution_hints.insert(
                "container.runtime".to_string(),
                container.runtime.to_string(),
            );
            if let Some(namespace) = &container.runtime_namespace {
                attributes.insert("container.runtime.namespace".to_string(), namespace.clone());
                execution_hints
                    .insert("container.runtime.namespace".to_string(), namespace.clone());
            }
            if let Some(pid) = container.pid {
                execution_hints.insert("pid".to_string(), pid.to_string());
            }
            if let Some(cgroup_path) = &container.cgroup_path {
                execution_hints.insert("cgroup.path".to_string(), cgroup_path.clone());
            }
            if let Some(namespace) = &container.k8s_namespace_name {
                attributes.insert("k8s.namespace.name".to_string(), namespace.clone());
                execution_hints.insert("k8s.namespace.name".to_string(), namespace.clone());
            }
            if let Some(pod_uid) = &container.k8s_pod_uid {
                attributes.insert("k8s.pod.uid".to_string(), pod_uid.clone());
                execution_hints.insert("k8s.pod.uid".to_string(), pod_uid.clone());
            }
            if let Some(pod_name) = &container.k8s_pod_name {
                attributes.insert("k8s.pod.name".to_string(), pod_name.clone());
                execution_hints.insert("k8s.pod.name".to_string(), pod_name.clone());
            }
            if let Some(container_name) = &container.k8s_container_name {
                attributes.insert("k8s.container.name".to_string(), container_name.clone());
                execution_hints.insert("k8s.container.name".to_string(), container_name.clone());
            }

            resources.push(DiscoveredResource {
                resource_id: resource_id.clone(),
                kind: "container".to_string(),
                origin_idx: 0,
                attributes,
                discovered_at: discovered_at.clone(),
                last_seen_at: discovered_at.clone(),
                health: "healthy".to_string(),
                source: self.name().to_string(),
            });
            targets.push(DiscoveredTarget {
                target_id: container.container_id,
                kind: "container".to_string(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContainerRuntimeRoot {
    runtime: &'static str,
    runtime_namespace: Option<&'static str>,
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedContainer {
    container_id: String,
    name: String,
    runtime: &'static str,
    runtime_namespace: Option<String>,
    pid: Option<u32>,
    cgroup_path: Option<String>,
    k8s_namespace_name: Option<String>,
    k8s_pod_uid: Option<String>,
    k8s_pod_name: Option<String>,
    k8s_container_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OciRuntimeSpec {
    #[serde(default)]
    linux: Option<OciLinuxSpec>,
    #[serde(default)]
    annotations: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct OciLinuxSpec {
    #[serde(default)]
    cgroups_path: Option<String>,
}

fn default_container_runtime_roots() -> Vec<ContainerRuntimeRoot> {
    vec![
        ContainerRuntimeRoot {
            runtime: "containerd",
            runtime_namespace: Some("k8s.io"),
            path: PathBuf::from("/run/containerd/io.containerd.runtime.v2.task/k8s.io"),
        },
        ContainerRuntimeRoot {
            runtime: "containerd",
            runtime_namespace: Some("default"),
            path: PathBuf::from("/run/containerd/io.containerd.runtime.v2.task/default"),
        },
        ContainerRuntimeRoot {
            runtime: "containerd",
            runtime_namespace: Some("k8s.io"),
            path: PathBuf::from("/var/run/containerd/io.containerd.runtime.v2.task/k8s.io"),
        },
        ContainerRuntimeRoot {
            runtime: "containerd",
            runtime_namespace: Some("default"),
            path: PathBuf::from("/var/run/containerd/io.containerd.runtime.v2.task/default"),
        },
        ContainerRuntimeRoot {
            runtime: "docker",
            runtime_namespace: None,
            path: PathBuf::from("/run/docker/runtime-runc/moby"),
        },
        ContainerRuntimeRoot {
            runtime: "docker",
            runtime_namespace: None,
            path: PathBuf::from("/var/run/docker/runtime-runc/moby"),
        },
    ]
}

fn discover_containers_in_roots(
    roots: &[ContainerRuntimeRoot],
) -> io::Result<Vec<ObservedContainer>> {
    let mut containers = BTreeMap::new();

    for root in roots {
        if !root.path.exists() {
            continue;
        }
        for container in read_runtime_root(root)? {
            containers
                .entry(container.container_id.clone())
                .or_insert(container);
        }
    }

    Ok(containers.into_values().collect())
}

fn read_runtime_root(root: &ContainerRuntimeRoot) -> io::Result<Vec<ObservedContainer>> {
    let mut containers = Vec::new();
    let entries = match fs::read_dir(&root.path) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(containers),
        Err(err) => return Err(err),
    };

    for entry in entries {
        let entry = entry?;
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        if !file_type.is_dir() {
            continue;
        }

        let container_id = entry.file_name().to_string_lossy().trim().to_string();
        if container_id.is_empty() {
            continue;
        }
        let task_dir = entry.path();
        let runtime_spec = read_runtime_spec(&task_dir);

        containers.push(ObservedContainer {
            name: discover_container_name(runtime_spec.as_ref(), &container_id)
                .unwrap_or_else(|| container_id.to_string()),
            container_id,
            runtime: root.runtime,
            runtime_namespace: root.runtime_namespace.map(str::to_string),
            pid: discover_container_pid(&task_dir),
            cgroup_path: discover_container_cgroup_path(runtime_spec.as_ref()),
            k8s_namespace_name: discover_annotation(
                runtime_spec.as_ref(),
                "io.kubernetes.cri.sandbox-namespace",
            ),
            k8s_pod_uid: discover_annotation(
                runtime_spec.as_ref(),
                "io.kubernetes.cri.sandbox-uid",
            ),
            k8s_pod_name: discover_annotation(
                runtime_spec.as_ref(),
                "io.kubernetes.cri.sandbox-name",
            ),
            k8s_container_name: discover_annotation(
                runtime_spec.as_ref(),
                "io.kubernetes.cri.container-name",
            ),
        });
    }

    Ok(containers)
}

fn discover_container_name(
    runtime_spec: Option<&OciRuntimeSpec>,
    container_id: &str,
) -> Option<String> {
    runtime_spec
        .and_then(|spec| {
            [
                "io.kubernetes.cri.container-name",
                "io.containerd.runc.v2.container.metadata.name",
                "org.opencontainers.container.name",
            ]
            .into_iter()
            .find_map(|key| spec.annotations.get(key).cloned())
        })
        .filter(|value| !value.trim().is_empty())
        .or_else(|| Some(container_id.to_string()))
}

fn discover_annotation(runtime_spec: Option<&OciRuntimeSpec>, key: &str) -> Option<String> {
    runtime_spec
        .and_then(|spec| spec.annotations.get(key).cloned())
        .filter(|value| !value.trim().is_empty())
}

fn discover_container_pid(task_dir: &std::path::Path) -> Option<u32> {
    let init_pid_path = task_dir.join("init.pid");
    let pid_text = fs::read_to_string(init_pid_path).ok()?;
    pid_text.trim().parse::<u32>().ok()
}

fn discover_container_cgroup_path(runtime_spec: Option<&OciRuntimeSpec>) -> Option<String> {
    runtime_spec
        .and_then(|spec| {
            spec.linux
                .as_ref()
                .and_then(|linux| linux.cgroups_path.clone())
        })
        .filter(|value| !value.trim().is_empty())
}

fn read_runtime_spec(task_dir: &std::path::Path) -> Option<OciRuntimeSpec> {
    let config_path = task_dir.join("config.json");
    let text = fs::read_to_string(config_path).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ContainerRuntimeRoot, discover_containers_in_roots};

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("warp-insight-container-discovery-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn discover_containers_returns_empty_when_roots_missing() {
        let containers = discover_containers_in_roots(&[ContainerRuntimeRoot {
            runtime: "containerd",
            runtime_namespace: Some("k8s.io"),
            path: PathBuf::from("/path/that/does/not/exist"),
        }])
        .expect("discover containers");

        assert!(containers.is_empty());
    }

    #[test]
    fn discover_containers_reads_known_runtime_roots() {
        let root = temp_dir("runtime-root");
        let task_root = root.join("k8s.io");
        fs::create_dir_all(task_root.join("container-a")).expect("container a");
        fs::create_dir_all(task_root.join("container-b")).expect("container b");
        fs::write(
            task_root.join("container-a").join("config.json"),
            r#"{
              "annotations": {
                "io.kubernetes.cri.container-name": "nginx",
                "io.kubernetes.cri.sandbox-namespace": "default",
                "io.kubernetes.cri.sandbox-uid": "pod-uid-1",
                "io.kubernetes.cri.sandbox-name": "nginx-pod"
              },
              "linux": {
                "cgroups_path": "/kubepods/test-a"
              }
            }"#,
        )
        .expect("config json");
        fs::write(task_root.join("container-a").join("init.pid"), "1234\n").expect("init pid");

        let containers = discover_containers_in_roots(&[ContainerRuntimeRoot {
            runtime: "containerd",
            runtime_namespace: Some("k8s.io"),
            path: task_root,
        }])
        .expect("discover containers");

        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0].container_id, "container-a");
        assert_eq!(containers[0].name, "nginx");
        assert_eq!(containers[0].runtime, "containerd");
        assert_eq!(containers[0].runtime_namespace.as_deref(), Some("k8s.io"));
        assert_eq!(containers[0].pid, Some(1234));
        assert_eq!(
            containers[0].cgroup_path.as_deref(),
            Some("/kubepods/test-a")
        );
        assert_eq!(containers[0].k8s_namespace_name.as_deref(), Some("default"));
        assert_eq!(containers[0].k8s_pod_uid.as_deref(), Some("pod-uid-1"));
        assert_eq!(containers[0].k8s_pod_name.as_deref(), Some("nginx-pod"));
        assert_eq!(containers[0].k8s_container_name.as_deref(), Some("nginx"));
        assert_eq!(containers[1].container_id, "container-b");
        assert_eq!(containers[1].name, "container-b");
    }

    #[test]
    fn discover_containers_deduplicates_same_container_id() {
        let root = temp_dir("dedupe");
        let containerd_root = root.join("containerd");
        let docker_root = root.join("docker");
        fs::create_dir_all(containerd_root.join("same-id")).expect("containerd task");
        fs::create_dir_all(docker_root.join("same-id")).expect("docker task");

        let containers = discover_containers_in_roots(&[
            ContainerRuntimeRoot {
                runtime: "containerd",
                runtime_namespace: Some("k8s.io"),
                path: containerd_root,
            },
            ContainerRuntimeRoot {
                runtime: "docker",
                runtime_namespace: None,
                path: docker_root,
            },
        ])
        .expect("discover containers");

        assert_eq!(containers.len(), 1);
        assert_eq!(containers[0].container_id, "same-id");
        assert_eq!(containers[0].runtime, "containerd");
    }
}
