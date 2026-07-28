export interface AgentRuntimeStatusView {
  agentId: string;
  instanceId: string;
  version: string;
  status: "online" | "offline" | "paused";
  health: "healthy" | "degraded" | "unhealthy";
  lastSeenAt: string;
}

export interface AgentOverviewMetrics {
  totalAgents: number;
  onlineAgents: number;
  unhealthyAgents: number;
  lastSeenLagSeconds: number;
}

export interface RecentOnlineRegisteredAgent {
  agentId: string;
  instanceId: string;
  version: string;
  registeredAt: string;
  onlineSince: string;
  onlineDurationSeconds: number;
}

export interface AgentOverview {
  metrics: AgentOverviewMetrics;
  recentOnlineAgents: RecentOnlineRegisteredAgent[];
  abnormalAgents: AgentRuntimeStatusView[];
}

export interface AgentInstallCode {
  x86LinuxInstallCode: string;
  armLinuxInstallCode: string;
}

export interface DispatchReceipt {
  dispatchId: string;
  commandId: string;
  agentId: string;
  status: "accepted" | "rejected";
  createdAt: string;
}

export interface PauseAgentCommand {
  agentId: string;
  requestedBy: string;
}

export interface UpgradeAgentCommand {
  agentId: string;
  targetVersion: string;
  requestedBy: string;
}

const mockOverview: AgentOverview = {
  metrics: {
    totalAgents: 9,
    onlineAgents: 7,
    unhealthyAgents: 2,
    lastSeenLagSeconds: 42,
  },
  recentOnlineAgents: [
    {
      agentId: "agent-prod-017",
      instanceId: "i-0842c71f",
      version: "v0.3.2",
      registeredAt: "2026-07-27T09:02:12+08:00",
      onlineSince: "2026-07-27T09:08:24+08:00",
      onlineDurationSeconds: 5400,
    },
    {
      agentId: "agent-edge-021",
      instanceId: "edge-node-021",
      version: "v0.3.2",
      registeredAt: "2026-07-27T08:44:09+08:00",
      onlineSince: "2026-07-27T08:51:36+08:00",
      onlineDurationSeconds: 6420,
    },
    {
      agentId: "agent-prod-018",
      instanceId: "i-0f93b42a",
      version: "v0.3.1",
      registeredAt: "2026-07-27T08:21:45+08:00",
      onlineSince: "2026-07-27T08:29:01+08:00",
      onlineDurationSeconds: 7770,
    },
  ],
  abnormalAgents: [
    {
      agentId: "agent-prod-001",
      instanceId: "i-0a12c9f8",
      version: "v0.3.1",
      status: "online",
      health: "degraded",
      lastSeenAt: "2026-07-27T10:32:18+08:00",
    },
    {
      agentId: "agent-edge-014",
      instanceId: "edge-node-014",
      version: "v0.2.8",
      status: "offline",
      health: "unhealthy",
      lastSeenAt: "2026-07-27T09:48:03+08:00",
    },
  ],
};

const mockInstallCode: AgentInstallCode = {
  x86LinuxInstallCode:
    "curl -fsSL http://127.0.0.1:3000/api/v1/agent/install/x86/install.sh | bash",
  armLinuxInstallCode:
    "curl -fsSL http://127.0.0.1:3000/api/v1/agent/install/arm/install.sh | bash",
};

async function requestJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: {
      "content-type": "application/json",
      ...(init?.headers ?? {}),
    },
  });
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} ${path}`);
  }
  return (await response.json()) as T;
}

function normalizeInstallCode(payload: any): AgentInstallCode {
  const installCode = payload.install_code ?? payload.installCode ?? payload;
  return {
    x86LinuxInstallCode:
      installCode.x86_linux_install_code ??
      installCode.x86LinuxInstallCode ??
      mockInstallCode.x86LinuxInstallCode,
    armLinuxInstallCode:
      installCode.arm_linux_install_code ??
      installCode.armLinuxInstallCode ??
      mockInstallCode.armLinuxInstallCode,
  };
}

function normalizeReceipt(payload: any): DispatchReceipt {
  const receipt = payload.result ?? payload;
  return {
    dispatchId: receipt.dispatch_id ?? receipt.dispatchId,
    commandId: receipt.command_id ?? receipt.commandId,
    agentId: receipt.agent_id ?? receipt.agentId,
    status: receipt.status ?? "accepted",
    createdAt: receipt.created_at ?? receipt.createdAt,
  };
}

function normalizeRuntimeStatus(payload: any): AgentRuntimeStatusView {
  return {
    agentId: payload.agent_id ?? payload.agentId,
    instanceId: payload.instance_id ?? payload.instanceId,
    version: payload.version,
    status: payload.status,
    health: payload.health,
    lastSeenAt: payload.last_seen_at ?? payload.lastSeenAt,
  };
}

function normalizeRecentOnlineAgent(payload: any): RecentOnlineRegisteredAgent {
  return {
    agentId: payload.agent_id ?? payload.agentId,
    instanceId: payload.instance_id ?? payload.instanceId,
    version: payload.version,
    registeredAt: payload.registered_at ?? payload.registeredAt,
    onlineSince: payload.online_since ?? payload.onlineSince,
    onlineDurationSeconds:
      payload.online_duration_seconds ?? payload.onlineDurationSeconds ?? 0,
  };
}

function normalizeOverview(payload: any): AgentOverview {
  const metrics = payload.metrics ?? {};
  const recentOnlineAgents =
    payload.recent_online_agents ?? payload.recentOnlineAgents ?? [];
  const abnormalAgents = payload.abnormal_agents ?? payload.abnormalAgents ?? [];
  return {
    metrics: {
      totalAgents: metrics.total_agents ?? metrics.totalAgents ?? 0,
      onlineAgents: metrics.online_agents ?? metrics.onlineAgents ?? 0,
      unhealthyAgents: metrics.unhealthy_agents ?? metrics.unhealthyAgents ?? 0,
      lastSeenLagSeconds:
        metrics.last_seen_lag_seconds ?? metrics.lastSeenLagSeconds ?? 0,
    },
    recentOnlineAgents: recentOnlineAgents.map(normalizeRecentOnlineAgent),
    abnormalAgents: abnormalAgents.map(normalizeRuntimeStatus),
  };
}

function createMockReceipt(agentId: string, kind: "pause" | "upgrade"): DispatchReceipt {
  const now = new Date().toISOString();
  return {
    agentId,
    commandId: `admin-${kind}-command`,
    dispatchId: `stub-${kind}-${Date.now()}`,
    status: "accepted",
    createdAt: now,
  };
}

export async function fetchAgentOverview(): Promise<AgentOverview> {
  try {
    const payload = await requestJson<unknown>("/api/v1/admin/agents/overview");
    return normalizeOverview(payload);
  } catch {
    return mockOverview;
  }
}

export async function fetchAgentInstallCode(): Promise<AgentInstallCode> {
  try {
    const payload = await requestJson<unknown>("/api/v1/agent/install-code");
    return normalizeInstallCode(payload);
  } catch {
    return mockInstallCode;
  }
}

export async function pauseAgent(command: PauseAgentCommand): Promise<DispatchReceipt> {
  try {
    const payload = await requestJson<unknown>(
      `/api/v1/admin/agents/${encodeURIComponent(command.agentId)}/pause`,
      {
        method: "POST",
        body: JSON.stringify({ requested_by: command.requestedBy }),
      },
    );
    return normalizeReceipt(payload);
  } catch {
    return createMockReceipt(command.agentId, "pause");
  }
}

export async function upgradeAgent(command: UpgradeAgentCommand): Promise<DispatchReceipt> {
  try {
    const payload = await requestJson<unknown>(
      `/api/v1/admin/agents/${encodeURIComponent(command.agentId)}/upgrade`,
      {
        method: "POST",
        body: JSON.stringify({
          requested_by: command.requestedBy,
          target_version: command.targetVersion,
        }),
      },
    );
    return normalizeReceipt(payload);
  } catch {
    return createMockReceipt(command.agentId, "upgrade");
  }
}
