// @moju generated
// @moju hash=aab56236cb440d6c

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct DaemonLoop {
    pub exec_bin: String,
    pub config: String,
}
