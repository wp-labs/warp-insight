// @moju generated
// @moju hash=91638372490551d3

use crate::domain::messages::SubmitEnrollmentRequest;

pub struct WpAgentOnlineRegistrationInterface;

impl WpAgentOnlineRegistrationInterface {
    pub fn route() -> (&'static str, &'static str) {
        ("POST", "/api/v1/agent/enroll")
    }
}

pub fn handler(_input: SubmitEnrollmentRequest) -> Result<(), crate::AppError> {
    todo!()
}
