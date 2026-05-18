//! Discovery snapshot to collection candidate mapping.

use warp_insight_contracts::discovery::{
    CandidateCollectionTarget, DiscoverySnapshotContract, StringKeyValue,
};

pub fn build_collection_candidates(
    snapshot: &DiscoverySnapshotContract,
) -> Vec<CandidateCollectionTarget> {
    let mut candidates = Vec::new();

    for target in &snapshot.targets {
        match target.kind.as_str() {
            "host" => candidates.push(CandidateCollectionTarget {
                candidate_id: format!("{}:host_metrics", target.target_id),
                target_ref: target.target_id.clone(),
                collection_kind: "host_metrics".to_string(),
                resource_ref: target.resource_ref.clone(),
                execution_hints: target
                    .execution_hints
                    .iter()
                    .map(|(k, v)| StringKeyValue::new(k.clone(), v.clone()))
                    .collect(),
                generated_at: snapshot.generated_at.clone(),
            }),
            "process" => {
                let mut execution_hints = Vec::new();
                for (key, value) in &target.execution_hints {
                    match key.as_str() {
                        "process.pid"
                        | "process.identity"
                        | "discovery.identity_strength"
                        | "discovery.identity_status" => {
                            execution_hints.push(StringKeyValue::new(key.clone(), value.clone()))
                        }
                        _ => {}
                    }
                }
                candidates.push(CandidateCollectionTarget {
                    candidate_id: format!("{}:process_metrics", target.target_id),
                    target_ref: target.target_id.clone(),
                    collection_kind: "process_metrics".to_string(),
                    resource_ref: target.resource_ref.clone(),
                    execution_hints,
                    generated_at: snapshot.generated_at.clone(),
                });
            }
            "container" => {
                let mut execution_hints = Vec::new();
                for (key, value) in &target.execution_hints {
                    match key.as_str() {
                        "container.runtime"
                        | "container.runtime.namespace"
                        | "pid"
                        | "cgroup.path"
                        | "k8s.namespace.name"
                        | "k8s.pod.uid"
                        | "k8s.pod.name"
                        | "k8s.container.name" => {
                            execution_hints.push(StringKeyValue::new(key.clone(), value.clone()))
                        }
                        _ => {}
                    }
                }
                candidates.push(CandidateCollectionTarget {
                    candidate_id: format!("{}:container_metrics", target.target_id),
                    target_ref: target.target_id.clone(),
                    collection_kind: "container_metrics".to_string(),
                    resource_ref: target.resource_ref.clone(),
                    execution_hints,
                    generated_at: snapshot.generated_at.clone(),
                });
            }
            _ => {}
        }
    }

    candidates
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use warp_insight_contracts::discovery::{DiscoveredTarget, DiscoverySnapshotContract};

    use super::build_collection_candidates;

    #[test]
    fn build_collection_candidates_maps_batch_a_targets() {
        let mut snapshot = DiscoverySnapshotContract::new(
            "snapshot-1".to_string(),
            1,
            "2026-04-19T00:00:00Z".to_string(),
        );
        snapshot.targets = vec![
            DiscoveredTarget {
                target_id: "host-1:host".to_string(),
                kind: "host".to_string(),
                origin_idx: 0,
                resource_ref: "host-1".to_string(),
                execution_hints: BTreeMap::from([("host.name".to_string(), "host-1".to_string())]),
                state: "active".to_string(),
            },
            DiscoveredTarget {
                target_id: "host-1:pid:42:process".to_string(),
                kind: "process".to_string(),
                origin_idx: 1,
                resource_ref: "host-1:pid:42".to_string(),
                execution_hints: BTreeMap::from([
                    ("process.pid".to_string(), "42".to_string()),
                    (
                        "process.identity".to_string(),
                        "linux_proc_start:1".to_string(),
                    ),
                ]),
                state: "active".to_string(),
            },
            DiscoveredTarget {
                target_id: "container-1".to_string(),
                kind: "container".to_string(),
                origin_idx: 2,
                resource_ref: "container-1".to_string(),
                execution_hints: BTreeMap::from([
                    ("container.runtime".to_string(), "containerd".to_string()),
                    ("k8s.pod.uid".to_string(), "pod-1".to_string()),
                    ("pid".to_string(), "1234".to_string()),
                ]),
                state: "active".to_string(),
            },
        ];

        let candidates = build_collection_candidates(&snapshot);

        assert_eq!(candidates.len(), 3);
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.collection_kind == "host_metrics"
                    && candidate.target_ref == "host-1:host")
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.collection_kind == "process_metrics"
                    && candidate.target_ref == "host-1:pid:42:process")
        );
        let container_candidate = candidates
            .iter()
            .find(|candidate| candidate.collection_kind == "container_metrics")
            .expect("container candidate");
        assert_eq!(
            container_candidate.candidate_id,
            "container-1:container_metrics"
        );
        assert_eq!(container_candidate.target_ref, "container-1");
        assert_eq!(container_candidate.resource_ref, "container-1");
        assert!(
            container_candidate
                .execution_hints
                .iter()
                .any(|hint| hint.key == "container.runtime" && hint.value == "containerd")
        );
        assert!(
            container_candidate
                .execution_hints
                .iter()
                .any(|hint| hint.key == "k8s.pod.uid" && hint.value == "pod-1")
        );
    }
}
