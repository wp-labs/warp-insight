// @moju generated
// @moju hash=8c5f4e01d1ad3c4e

use crate::domain::messages::AdminGetAgentInstallCode;

pub struct WarpInsightAdminPublicInstallInterface;

impl WarpInsightAdminPublicInstallInterface {
    pub fn route() -> (&'static str, &'static str) {
        ("GET", "/api/v1/agent/install-code")
    }
}

pub fn handler(_input: AdminGetAgentInstallCode) -> Result<(), crate::AppError> {
    todo!()
}
