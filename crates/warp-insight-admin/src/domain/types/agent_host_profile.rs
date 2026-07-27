// @moju generated
// @moju hash=da7ecaf505afe2dd

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control")]
pub struct AgentHostProfile {
    pub cloud_instance_id: String,
    #[moju(unique)]
    pub node_id: String,
    pub hostname: String,
    pub os: String,
    pub machine_id: String,
    pub arch: String,
    pub k8s_node_uid: String,
    pub ip_addresses: Vec<String>,
}
