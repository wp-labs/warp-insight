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

export interface AgentMetricSample {
  at: string;
  memoryBytes?: number;
  cpuPercent?: number;
  adminLatencyMs?: number;
}

export interface RecentOnlineRegisteredAgent {
  agentId: string;
  instanceId: string;
  version: string;
  registeredAt: string;
  onlineSince: string;
  onlineDurationSeconds: number;
  source: "real" | "example";
  memoryBytes?: number;
  cpuPercent?: number;
  adminLatencyMs?: number;
  metricsHistory?: AgentMetricSample[];
}

export interface AgentOverview {
  metrics: AgentOverviewMetrics;
  recentOnlineAgents: RecentOnlineRegisteredAgent[];
  abnormalAgents: AgentRuntimeStatusView[];
}

export interface AgentHostMetrics {
  agentId: string;
  loadAverage1m?: number;
  loadAverage5m?: number;
  loadAverage15m?: number;
  uptimeSeconds?: number;
  memoryTotalKb?: number;
  memoryAvailableKb?: number;
  diskUsagePercent?: number;
  diskTotalKb?: number;
  diskAvailableKb?: number;
  history?: AgentHostMetricsHistory;
}

export interface AgentHostMetricsHistory {
  loadAverage1m: [number, number][];
  loadAverage5m: [number, number][];
  loadAverage15m: [number, number][];
  memoryTotalKb: [number, number][];
  memoryAvailableKb: [number, number][];
  diskUsagePercent: [number, number][];
}

export interface AgentHostMetricsSummary {
  agentId: string;
  loadAverage1m?: number;
  memoryTotalKb?: number;
  memoryAvailableKb?: number;
  diskUsagePercent?: number;
}

export interface AgentInstallCode {
  x86LinuxInstallCode: string;
  armLinuxInstallCode: string;
  macosInstallCode: string;
  bootstrapEnrollmentToken: string;
}

/** 控制中心返回给 Gateway 的初始连接材料，字段与 Gateway 面接口契约一致。 */
export interface ControlCenterTrustBundle {
  trust_bundle_id: string;
  control_endpoint: string;
  ca_bundle: string;
  server_name: string;
  expected_san: string;
  issued_at: string | null;
  expires_at: string | null;
}

/** GET /api/v1/gateway/initial-config 的 config 载荷。 */
export interface GatewayInitialConfig {
  gateway_id: string;
  control_center_endpoint: string;
  trust_bundle: ControlCenterTrustBundle | null;
  server_tls_required: boolean;
  protocol_version: string;
  enrollment_token_id: string;
}

export type GatewayInstanceLifecycleState =
  "Provisioned" | "Initializing" | "Running" | "Failed";

/** Center 侧实例初始化状态；initialized 是 lifecycle_state 的服务端派生值。 */
export interface GatewayInitializationStatus {
  gateway_id: string;
  instance_id: string | null;
  lifecycle_state: GatewayInstanceLifecycleState;
  initialized: boolean;
}

/** 页面完成状态守卫并取得 JSON 初始配置后的结果。 */
export interface GatewayInitializationResult {
  config: GatewayInitialConfig;
  status: GatewayInitializationStatus;
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

export const ADMIN_AUTH_CHANGED_EVENT = "warpInsightAdminAuthChanged";
const ADMIN_API_TOKEN_STORAGE_KEY = "warpInsightAdminApiToken";

// Persist the admin token for the current browser session (survives page
// reloads, but is cleared when the tab/session closes).
let adminApiToken: string | null =
  typeof window !== "undefined"
    ? window.sessionStorage.getItem(ADMIN_API_TOKEN_STORAGE_KEY)
    : null;

export class ApiError extends Error {
  readonly status: number;
  /** Seconds until the per-IP rate-limit block expires, when status is 429. */
  readonly retryAfterSeconds?: number;

  constructor(status: number, path: string, retryAfterSeconds?: number) {
    super(`HTTP ${status} ${path}`);
    this.name = "ApiError";
    this.status = status;
    this.retryAfterSeconds = retryAfterSeconds;
  }
}

/** 初始化 URL 不满足 Center 当前入口契约时抛出，错误由页面作为表单反馈展示。 */
export class GatewayInitializationInputError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "GatewayInitializationInputError";
  }
}

/** Center 已记录实例进入初始化态或运行态时抛出，阻止页面再次消费置备凭证。 */
export class GatewayAlreadyInitializedError extends Error {
  readonly status: GatewayInitializationStatus;

  constructor(status: GatewayInitializationStatus) {
    super("gateway is already initialized");
    this.name = "GatewayAlreadyInitializedError";
    this.status = status;
  }
}

export function isRateLimitedError(error: unknown): error is ApiError {
  return error instanceof ApiError && error.status === 429;
}

async function requestJson<T>(path: string, init?: RequestInit): Promise<T> {
  const adminToken = getAdminApiToken();
  const response = await fetch(path, {
    ...init,
    headers: {
      "content-type": "application/json",
      ...(adminToken ? { authorization: `Bearer ${adminToken}` } : {}),
      ...(init?.headers ?? {}),
    },
  });
  if (!response.ok) {
    if (response.status === 429) {
      const retryAfter = Number.parseInt(
        response.headers.get("Retry-After") ?? "",
        10,
      );
      throw new ApiError(
        response.status,
        path,
        Number.isFinite(retryAfter) && retryAfter > 0 ? retryAfter : 60,
      );
    }
    throw new ApiError(response.status, path);
  }
  return (await response.json()) as T;
}

export function getAdminApiToken(): string | null {
  return adminApiToken;
}

export function setAdminApiToken(token: string): void {
  const trimmed = token.trim();
  adminApiToken = trimmed || null;
  if (typeof window !== "undefined") {
    if (adminApiToken) {
      window.sessionStorage.setItem(ADMIN_API_TOKEN_STORAGE_KEY, adminApiToken);
    } else {
      window.sessionStorage.removeItem(ADMIN_API_TOKEN_STORAGE_KEY);
    }
    window.dispatchEvent(new Event(ADMIN_AUTH_CHANGED_EVENT));
  }
}

export function clearAdminApiToken(): void {
  setAdminApiToken("");
}

function requiredString(value: unknown, fieldName: string): string {
  if (typeof value === "string") return value;
  throw new Error(`Invalid API response: missing ${fieldName}`);
}

function requiredNumber(value: unknown, fieldName: string): number {
  if (typeof value === "number") return value;
  throw new Error(`Invalid API response: missing ${fieldName}`);
}

function requiredBoolean(value: unknown, fieldName: string): boolean {
  if (typeof value === "boolean") return value;
  throw new Error(`Invalid API response: missing ${fieldName}`);
}

function requiredRecord(
  value: unknown,
  fieldName: string,
): Record<string, unknown> {
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  throw new Error(`Invalid API response: missing ${fieldName}`);
}

function nullableString(value: unknown, fieldName: string): string | null {
  if (value === null) return null;
  return requiredString(value, fieldName);
}

function normalizeTrustBundle(value: unknown): ControlCenterTrustBundle | null {
  if (value === null) return null;
  const bundle = requiredRecord(value, "config.trust_bundle");
  return {
    trust_bundle_id: requiredString(
      bundle.trust_bundle_id,
      "config.trust_bundle.trust_bundle_id",
    ),
    control_endpoint: requiredString(
      bundle.control_endpoint,
      "config.trust_bundle.control_endpoint",
    ),
    ca_bundle: requiredString(
      bundle.ca_bundle,
      "config.trust_bundle.ca_bundle",
    ),
    server_name: requiredString(
      bundle.server_name,
      "config.trust_bundle.server_name",
    ),
    expected_san: requiredString(
      bundle.expected_san,
      "config.trust_bundle.expected_san",
    ),
    issued_at: nullableString(
      bundle.issued_at,
      "config.trust_bundle.issued_at",
    ),
    expires_at: nullableString(
      bundle.expires_at,
      "config.trust_bundle.expires_at",
    ),
  };
}

/** 按当前 JSON 契约收敛初始配置，避免把服务端错误静默成空字段。 */
export function normalizeGatewayInitialConfig(
  payload: unknown,
): GatewayInitialConfig {
  const root = requiredRecord(payload, "response");
  const config = requiredRecord(root.config, "config");
  return {
    gateway_id: requiredString(config.gateway_id, "config.gateway_id"),
    control_center_endpoint: requiredString(
      config.control_center_endpoint,
      "config.control_center_endpoint",
    ),
    trust_bundle: normalizeTrustBundle(config.trust_bundle),
    server_tls_required: requiredBoolean(
      config.server_tls_required,
      "config.server_tls_required",
    ),
    protocol_version: requiredString(
      config.protocol_version,
      "config.protocol_version",
    ),
    enrollment_token_id: requiredString(
      config.enrollment_token_id,
      "config.enrollment_token_id",
    ),
  };
}

function normalizeGatewayLifecycleState(
  value: unknown,
): GatewayInstanceLifecycleState {
  if (
    value === "Provisioned" ||
    value === "Initializing" ||
    value === "Running" ||
    value === "Failed"
  ) {
    return value;
  }
  throw new Error("Invalid API response: invalid lifecycle_state");
}

/** 按 QueryGatewayInitializationStatus 响应契约校验 Center 返回值。 */
export function normalizeGatewayInitializationStatus(
  payload: unknown,
): GatewayInitializationStatus {
  const status = requiredRecord(payload, "response");
  return {
    gateway_id: requiredString(status.gateway_id, "gateway_id"),
    instance_id: nullableString(status.instance_id, "instance_id"),
    lifecycle_state: normalizeGatewayLifecycleState(status.lifecycle_state),
    initialized: requiredBoolean(status.initialized, "initialized"),
  };
}

function requiredArray(value: unknown, fieldName: string): any[] {
  if (Array.isArray(value)) return value;
  throw new Error(`Invalid API response: missing ${fieldName}`);
}

function normalizeAgentStatus(
  value: unknown,
): AgentRuntimeStatusView["status"] {
  if (value === "online" || value === "offline" || value === "paused")
    return value;
  throw new Error("Invalid API response: invalid agent status");
}

function normalizeAgentHealth(
  value: unknown,
): AgentRuntimeStatusView["health"] {
  if (value === "healthy" || value === "degraded" || value === "unhealthy")
    return value;
  throw new Error("Invalid API response: invalid agent health");
}

function normalizeReceiptStatus(value: unknown): DispatchReceipt["status"] {
  if (value === "accepted" || value === "rejected") return value;
  throw new Error("Invalid API response: invalid dispatch receipt status");
}

function normalizeInstallCode(payload: any): AgentInstallCode {
  const installCode = payload.install_code ?? payload.installCode ?? payload;
  return {
    x86LinuxInstallCode: requiredString(
      installCode.x86_linux_install_code ?? installCode.x86LinuxInstallCode,
      "installCode.x86LinuxInstallCode",
    ),
    armLinuxInstallCode: requiredString(
      installCode.arm_linux_install_code ?? installCode.armLinuxInstallCode,
      "installCode.armLinuxInstallCode",
    ),
    macosInstallCode: requiredString(
      installCode.macos_install_code ?? installCode.macosInstallCode,
      "installCode.macosInstallCode",
    ),
    bootstrapEnrollmentToken: requiredString(
      installCode.bootstrap_enrollment_token ??
        installCode.bootstrapEnrollmentToken,
      "installCode.bootstrapEnrollmentToken",
    ),
  };
}

function normalizeReceipt(payload: any): DispatchReceipt {
  const receipt = payload.result ?? payload;
  return {
    dispatchId: requiredString(
      receipt.dispatch_id ?? receipt.dispatchId,
      "receipt.dispatchId",
    ),
    commandId: requiredString(
      receipt.command_id ?? receipt.commandId,
      "receipt.commandId",
    ),
    agentId: requiredString(
      receipt.agent_id ?? receipt.agentId,
      "receipt.agentId",
    ),
    status: normalizeReceiptStatus(receipt.status),
    createdAt: requiredString(
      receipt.created_at ?? receipt.createdAt,
      "receipt.createdAt",
    ),
  };
}

function normalizeRuntimeStatus(payload: any): AgentRuntimeStatusView {
  return {
    agentId: requiredString(
      payload.agent_id ?? payload.agentId,
      "agent.agentId",
    ),
    instanceId: requiredString(
      payload.instance_id ?? payload.instanceId,
      "agent.instanceId",
    ),
    version: requiredString(payload.version, "agent.version"),
    status: normalizeAgentStatus(payload.status),
    health: normalizeAgentHealth(payload.health),
    lastSeenAt: requiredString(
      payload.last_seen_at ?? payload.lastSeenAt,
      "agent.lastSeenAt",
    ),
  };
}

function normalizeRecentOnlineAgent(payload: any): RecentOnlineRegisteredAgent {
  const source = payload.source ?? "real";
  if (source !== "real" && source !== "example") {
    throw new Error("Invalid API response: invalid recent online agent source");
  }
  const rawHistory = payload.metrics_history ?? payload.metricsHistory ?? [];
  return {
    agentId: requiredString(
      payload.agent_id ?? payload.agentId,
      "recentOnlineAgent.agentId",
    ),
    instanceId: requiredString(
      payload.instance_id ?? payload.instanceId,
      "recentOnlineAgent.instanceId",
    ),
    version: requiredString(payload.version, "recentOnlineAgent.version"),
    registeredAt: requiredString(
      payload.registered_at ?? payload.registeredAt,
      "recentOnlineAgent.registeredAt",
    ),
    onlineSince: requiredString(
      payload.online_since ?? payload.onlineSince,
      "recentOnlineAgent.onlineSince",
    ),
    onlineDurationSeconds: requiredNumber(
      payload.online_duration_seconds ?? payload.onlineDurationSeconds,
      "recentOnlineAgent.onlineDurationSeconds",
    ),
    source,
    memoryBytes: payload.memory_bytes ?? payload.memoryBytes,
    cpuPercent: payload.cpu_percent ?? payload.cpuPercent,
    adminLatencyMs: payload.admin_latency_ms ?? payload.adminLatencyMs,
    metricsHistory: rawHistory.map((sample: any) => ({
      at: sample.at,
      memoryBytes: sample.memory_bytes ?? sample.memoryBytes,
      cpuPercent: sample.cpu_percent ?? sample.cpuPercent,
      adminLatencyMs: sample.admin_latency_ms ?? sample.adminLatencyMs,
    })),
  };
}

export function normalizeOverview(payload: any): AgentOverview {
  const metrics = payload.metrics;
  const recentOnlineAgents =
    payload.recent_online_agents ?? payload.recentOnlineAgents;
  const abnormalAgents = payload.abnormal_agents ?? payload.abnormalAgents;
  return {
    metrics: {
      totalAgents: requiredNumber(
        metrics?.total_agents ?? metrics?.totalAgents,
        "metrics.totalAgents",
      ),
      onlineAgents: requiredNumber(
        metrics?.online_agents ?? metrics?.onlineAgents,
        "metrics.onlineAgents",
      ),
      unhealthyAgents: requiredNumber(
        metrics?.unhealthy_agents ?? metrics?.unhealthyAgents,
        "metrics.unhealthyAgents",
      ),
      lastSeenLagSeconds: requiredNumber(
        metrics?.last_seen_lag_seconds ?? metrics?.lastSeenLagSeconds,
        "metrics.lastSeenLagSeconds",
      ),
    },
    recentOnlineAgents: requiredArray(
      recentOnlineAgents,
      "overview.recentOnlineAgents",
    ).map(normalizeRecentOnlineAgent),
    abnormalAgents: requiredArray(
      abnormalAgents,
      "overview.abnormalAgents",
    ).map(normalizeRuntimeStatus),
  };
}

function normalizeHostMetrics(payload: any): AgentHostMetrics {
  return {
    agentId: requiredString(payload.agent_id ?? payload.agentId, "host.agentId"),
    loadAverage1m: payload.load_average_1m ?? payload.loadAverage1m,
    loadAverage5m: payload.load_average_5m ?? payload.loadAverage5m,
    loadAverage15m: payload.load_average_15m ?? payload.loadAverage15m,
    uptimeSeconds: payload.uptime_seconds ?? payload.uptimeSeconds,
    memoryTotalKb: payload.memory_total_kb ?? payload.memoryTotalKb,
    memoryAvailableKb: payload.memory_available_kb ?? payload.memoryAvailableKb,
    diskUsagePercent: payload.disk_usage_percent ?? payload.diskUsagePercent,
    diskTotalKb: payload.disk_total_kb ?? payload.diskTotalKb,
    diskAvailableKb: payload.disk_available_kb ?? payload.diskAvailableKb,
    history: payload.history
      ? normalizeHostMetricsHistory(payload.history)
      : undefined,
  };
}

function normalizeHostMetricsHistory(payload: any): AgentHostMetricsHistory {
  return {
    loadAverage1m: normalizeSeries(payload.load_average_1m ?? payload.loadAverage1m),
    loadAverage5m: normalizeSeries(payload.load_average_5m ?? payload.loadAverage5m),
    loadAverage15m: normalizeSeries(payload.load_average_15m ?? payload.loadAverage15m),
    memoryTotalKb: normalizeSeries(payload.memory_total_kb ?? payload.memoryTotalKb),
    memoryAvailableKb: normalizeSeries(
      payload.memory_available_kb ?? payload.memoryAvailableKb,
    ),
    diskUsagePercent: normalizeSeries(
      payload.disk_usage_percent ?? payload.diskUsagePercent,
    ),
  };
}

function normalizeSeries(payload: any): [number, number][] {
  if (!Array.isArray(payload)) return [];
  return payload.flatMap((point: any) => {
    if (!Array.isArray(point) || point.length < 2) return [];
    const timestamp = Number(point[0]);
    const value = Number(point[1]);
    if (!Number.isFinite(timestamp) || !Number.isFinite(value)) return [];
    return [[timestamp, value] as [number, number]];
  });
}

function normalizeHostMetricsSummaries(payload: any): AgentHostMetricsSummary[] {
  if (!Array.isArray(payload)) return [];
  return payload.map(normalizeHostMetricsSummary);
}

function normalizeHostMetricsSummary(payload: any): AgentHostMetricsSummary {
  return {
    agentId: requiredString(payload.agent_id ?? payload.agentId, "host.agentId"),
    loadAverage1m: payload.load_average_1m ?? payload.loadAverage1m,
    memoryTotalKb: payload.memory_total_kb ?? payload.memoryTotalKb,
    memoryAvailableKb: payload.memory_available_kb ?? payload.memoryAvailableKb,
    diskUsagePercent: payload.disk_usage_percent ?? payload.diskUsagePercent,
  };
}

export async function fetchAgentOverview(): Promise<AgentOverview> {
  const payload = await requestJson<unknown>("/api/v1/admin/agents/overview");
  return normalizeOverview(payload);
}

export async function fetchAgentHostMetrics(
  agentId: string,
): Promise<AgentHostMetrics> {
  const payload = await requestJson<unknown>(
    `/api/v1/admin/agents/${encodeURIComponent(agentId)}/host-metrics`,
  );
  return normalizeHostMetrics(payload);
}

export async function fetchAllAgentsHostMetrics(): Promise<
  AgentHostMetricsSummary[]
> {
  const payload = await requestJson<unknown>(
    "/api/v1/admin/agents/host-metrics",
  );
  return normalizeHostMetricsSummaries(payload);
}

export async function fetchAgentInstallCode(): Promise<AgentInstallCode> {
  const payload = await requestJson<unknown>("/api/v1/agent/install-code");
  return normalizeInstallCode(payload);
}

/**
 * 从 Gateway 页面调用控制中心的网关面初始化接口。
 * initUrl 由 Center 创建实例时下发，**不携带凭证**（token 不进 URL）；
 * 网关凭证由操作者单独输入，只放入 Authorization Header。
 * 返回 Center 当前实现的 `application/json` 响应中的 `config` 对象。
 */
export async function fetchGatewayInitialConfig(
  initUrl: string,
  token?: string,
): Promise<GatewayInitialConfig> {
  // 去掉可能残留的 fragment（如手工复制带 # 的链接）。
  const path = initUrl.split("#", 1)[0];
  const response = await fetch(path, {
    headers: {
      accept: "application/json",
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
  });
  if (!response.ok) {
    throw new ApiError(response.status, path);
  }
  return normalizeGatewayInitialConfig(await response.json());
}

/** 初始化 URL 校验后得到的请求目标，供页面 Service 串联状态查询与配置请求。 */
export interface GatewayInitializationTarget {
  initUrl: string;
  instanceId: string;
  statusUrl: string;
}

/**
 * 校验 Center 交付的初始化 URL，并派生同一 Center 上的初始化状态查询地址。
 * URL 只允许 instance_id 查询参数；Bearer 凭证必须由调用方另行放入 Header。
 */
export function parseGatewayInitializationUrl(
  input: string,
): GatewayInitializationTarget {
  let url: URL;
  try {
    url = new URL(input.trim());
  } catch {
    throw new GatewayInitializationInputError(
      "请输入完整、有效的控制中心初始化 URL。",
    );
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new GatewayInitializationInputError(
      "初始化 URL 只支持 HTTP 或 HTTPS 协议。",
    );
  }
  if (url.username || url.password || url.hash) {
    throw new GatewayInitializationInputError(
      "初始化 URL 不能携带用户信息、凭证或 fragment。",
    );
  }
  if (!url.pathname.endsWith("/api/v1/gateway/initial-config")) {
    throw new GatewayInitializationInputError(
      "初始化 URL 必须指向 /api/v1/gateway/initial-config。",
    );
  }
  const queryNames = [...url.searchParams.keys()];
  if (queryNames.length !== 1 || queryNames[0] !== "instance_id") {
    throw new GatewayInitializationInputError(
      "初始化 URL 只能包含 instance_id；Bearer 凭证请填写到独立凭证输入框。",
    );
  }
  const instanceId = url.searchParams.get("instance_id")?.trim();
  if (!instanceId) {
    throw new GatewayInitializationInputError("初始化 URL 缺少 instance_id。");
  }

  const statusUrl = new URL(url);
  statusUrl.pathname = statusUrl.pathname.replace(
    /\/initial-config$/,
    "/initialization-status",
  );
  statusUrl.search = "";
  statusUrl.searchParams.set("instance_id", instanceId);
  return {
    initUrl: url.toString(),
    instanceId,
    statusUrl: statusUrl.toString(),
  };
}

/** 查询 Center 侧实例状态；可选 Bearer 仍只通过 Authorization Header 发送。 */
export async function fetchGatewayInitializationStatus(
  statusUrl: string,
  token?: string,
): Promise<GatewayInitializationStatus> {
  const response = await fetch(statusUrl, {
    headers: {
      accept: "application/json",
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
  });
  if (!response.ok) {
    throw new ApiError(response.status, statusUrl);
  }
  return normalizeGatewayInitializationStatus(await response.json());
}

/**
 * 页面初始化业务 Service：解析 URL → 查询状态 → 拦截重复初始化 → 获取 JSON 配置。
 * 状态检查不替代 Center 的服务端守卫，只用于在消费一次性凭证前提供明确反馈。
 */
export async function initializeGatewayViaUrl(
  initUrl: string,
  token?: string,
): Promise<GatewayInitializationResult> {
  const target = parseGatewayInitializationUrl(initUrl);
  const status = await fetchGatewayInitializationStatus(
    target.statusUrl,
    token,
  );
  if (status.initialized) {
    throw new GatewayAlreadyInitializedError(status);
  }
  const config = await fetchGatewayInitialConfig(target.initUrl, token);
  return { config, status };
}

export async function pauseAgent(
  command: PauseAgentCommand,
): Promise<DispatchReceipt> {
  const payload = await requestJson<unknown>(
    `/api/v1/admin/agents/${encodeURIComponent(command.agentId)}/pause`,
    {
      method: "POST",
      body: JSON.stringify({ requested_by: command.requestedBy }),
    },
  );
  return normalizeReceipt(payload);
}

export async function upgradeAgent(
  command: UpgradeAgentCommand,
): Promise<DispatchReceipt> {
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
}
