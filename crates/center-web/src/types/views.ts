export interface GatewayStatusView {
  gatewayId: string;
  instanceId: string;
  version: string;
  status: string;
  health: string;
  lastSeenAt: string;
}

export interface GatewayListView {
  gatewayCount: number;
  onlineCount: number;
  degradedCount: number;
  offlineCount: number;
  updatedAt: string;
}

export interface GatewayInstance {
  gatewayId: string;
  instanceId: string;
  status: string;
  createdAt: string;
}

export interface GatewayCustomerBinding {
  gatewayId: string;
  customerId: string;
  status: string;
  boundAt: string;
}

export interface GatewayInitialConfig {
  controlCenterEndpoint: string;
  policyVersion: string;
  telemetryOutput: string;
}

export interface WistAgentdRelease {
  version: string;
  artifactUrl: string;
  status: string;
  publishedAt: string;
}

export interface WarpGateWayRelease {
  version: string;
  artifactUrl: string;
  status: string;
  publishedAt: string;
}

export interface UpgradePlan {
  planId: string;
  component: string;
  targetVersion: string;
  targetCount: number;
  status: string;
  createdAt: string;
}

export interface UpgradePlanApproval {
  planId: string;
  status: string;
  approvedBy: string;
  approvedAt: string;
}

export interface GlobalPolicyDispatch {
  dispatchId: string;
  policyVersion: string;
  targetCount: number;
  status: string;
  dispatchedAt: string;
}