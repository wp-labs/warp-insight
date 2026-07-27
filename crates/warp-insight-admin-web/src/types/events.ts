export interface ControlAgentCredentialRevoked {
  agentId: string;
  instanceId: string;
  reasonCode: string;
  revokedAt: string;
}

export interface ControlControlMessageRejected {
  messageId: string;
  agentId: string;
  instanceId: string;
  reasonCode: string;
  rejectedAt: string;
}

export interface ControlAgentCredentialIssued {
  agentId: string;
  instanceId: string;
  credential: string;
  issuedAt: string;
}

export interface ControlAgentEnrollmentTokenAccepted {
  tokenId: string;
  tenantId: string;
  environmentId: string;
  nodeId: string;
  acceptedAt: string;
}

export interface ReportingIngestHeadError {
  reason: string;
  detail: string;
}

export interface ReportingActionPlanAck {
  apiVersion: string;
  kind: string;
  dispatchId: string;
  actionId: string;
  planDigest: string;
  agentId: string;
  instanceId: string;
  executionId: string;
  ackStatus: string;
  reasonCode: string;
  reasonMessage: string;
  queuePosition: string;
  receivedAt: string;
  acknowledgedAt: string;
}

export interface ReportingDiscoveryIngestAck {
  reportId: string;
  status: string;
  ingestedResources: string;
  ingestedTargets: string;
  receivedAt: string;
  ackAt: string;
}

export interface ControlControlLongPollTimedOut {
  agentId: string;
  instanceId: string;
  waitMs: number;
  timedOutAt: string;
}

export interface ControlControlCommandsReturned {
  agentId: string;
  instanceId: string;
  messages: string;
  nextSequence: number;
  returnedAt: string;
}

export interface ControlAgentEnrollmentAccepted {
  agentId: string;
  instanceId: string;
  tenantId: string;
  environmentId: string;
  nodeId: string;
  acceptedAt: string;
}

export interface SubsystemAdminPauseAgentRequested {
  agentId: string;
  requestedBy: string;
}

export interface ControlAgentEnrollmentRejected {
  tokenId: string;
  nodeId: string;
  reasonCode: string;
  rejectedAt: string;
}

export interface ControlAgentCredentialVerified {
  agentId: string;
  instanceId: string;
  verifiedAt: string;
}

export interface ControlAgentCredentialRejected {
  agentId: string;
  instanceId: string;
  reasonCode: string;
  rejectedAt: string;
}

export interface SubsystemAdminUpgradeAgentRequested {
  agentId: string;
  targetVersion: string;
  requestedBy: string;
}

export interface ControlAgentEnrollmentTokenRejected {
  tokenId: string;
  nodeId: string;
  reasonCode: string;
  rejectedAt: string;
}

export interface ControlControlMessageAccepted {
  messageId: string;
  agentId: string;
  instanceId: string;
  acceptedAt: string;
}

export interface SubsystemAdminShowAgentRuntimeStatusRequested {
  agentId: string;
  requestedBy: string;
}

export interface ControlDuplicateRegistrationDetected {
  nodeId: string;
  existingAgentId: string;
  candidateInstanceId: string;
  action: string;
  detectedAt: string;
}

export type AppEvent = ControlAgentCredentialRevoked | ControlControlMessageRejected | ControlAgentCredentialIssued | ControlAgentEnrollmentTokenAccepted | ReportingIngestHeadError | ReportingActionPlanAck | ReportingDiscoveryIngestAck | ControlControlLongPollTimedOut | ControlControlCommandsReturned | ControlAgentEnrollmentAccepted | SubsystemAdminPauseAgentRequested | ControlAgentEnrollmentRejected | ControlAgentCredentialVerified | ControlAgentCredentialRejected | SubsystemAdminUpgradeAgentRequested | ControlAgentEnrollmentTokenRejected | ControlControlMessageAccepted | SubsystemAdminShowAgentRuntimeStatusRequested | ControlDuplicateRegistrationDetected;
