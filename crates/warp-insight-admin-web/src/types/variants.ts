export type ControlAgentEnrollmentTokenStatus =
  | { kind: "Meta" }
  | { kind: "LabelZhAgentToken" }
  | { kind: "LabelEnAgentEnrollmentTokenStatus" }
  | { kind: "SummaryZhToken" }
  | { kind: "TagEnrollment" }
  | { kind: "TagStatus" }
  | { kind: "Value" }
  | { kind: "Active" }
  | { kind: "Expired" }
  | { kind: "Revoked" }
  | { kind: "Exhausted" };

export type ReportingHealthState =
  | { kind: "Healthy" }
  | { kind: "Degraded" }
  | { kind: "Unhealthy" };

export type ControlAgentEnrollmentTokenValidationStatus =
  | { kind: "Valid" }
  | { kind: "HashMismatch" }
  | { kind: "Expired" }
  | { kind: "Revoked" }
  | { kind: "Exhausted" }
  | { kind: "EnvironmentMismatch" }
  | { kind: "HostNotAllowed" };

export type ControlAgentEnrollmentResultStatus =
  | { kind: "Meta" }
  | { kind: "LabelZhAgent" }
  | { kind: "LabelEnAgentEnrollmentResultStatus" }
  | { kind: "SummaryZhAgentd" }
  | { kind: "TagEnrollment" }
  | { kind: "TagStatus" }
  | { kind: "Value" }
  | { kind: "Accepted" }
  | { kind: "Rejected" }
  | { kind: "PendingReview" };

export type ControlAgentIdentityStatus =
  | { kind: "Meta" }
  | { kind: "LabelZhAgent" }
  | { kind: "LabelEnAgentIdentityStatus" }
  | { kind: "SummaryZhAgentd" }
  | { kind: "TagAgentd" }
  | { kind: "TagIdentity" }
  | { kind: "Value" }
  | { kind: "Active" }
  | { kind: "Revoked" }
  | { kind: "Expired" }
  | { kind: "RenewalRequired" };

export type ControlAgentDownstreamMessageType =
  | { kind: "Meta" }
  | { kind: "LabelZhAgent" }
  | { kind: "LabelEnAgentDownstreamMessageType" }
  | { kind: "SummaryZhAgentd" }
  | { kind: "TagControl" }
  | { kind: "TagProtocol" }
  | { kind: "Value" }
  | { kind: "EnrollmentResult" }
  | { kind: "ControlCommands" }
  | { kind: "PolicyRefreshHint" }
  | { kind: "IdentityRotationHint" };

export type ControlAgentCredentialVerificationStatus =
  | { kind: "Verified" }
  | { kind: "UnknownAgent" }
  | { kind: "CredentialMissing" }
  | { kind: "CredentialExpired" }
  | { kind: "CredentialRevoked" }
  | { kind: "CredentialMismatch" };

export type ControlAgentUpstreamMessageType =
  | { kind: "Meta" }
  | { kind: "LabelZhAgent" }
  | { kind: "LabelEnAgentUpstreamMessageType" }
  | { kind: "SummaryZhAgentd" }
  | { kind: "TagControl" }
  | { kind: "TagProtocol" }
  | { kind: "Value" }
  | { kind: "EnrollmentRequest" }
  | { kind: "StatusReport" }
  | { kind: "CommandPoll" }
  | { kind: "ActionResult" };