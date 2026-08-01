// @moju generated
// @moju hash=ac6a7c4539bc6948

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control")]
pub struct AgentInstallCode {
    pub x86_linux_install_code: String,
    pub bootstrap_enrollment_token: String,
    pub bootstrap_bundle: crate::domain::types::AgentBootstrapBundle,
    pub arm_linux_install_code: String,
}
