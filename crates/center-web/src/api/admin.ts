// WarpInsightCenter 全局控制中心 Admin API client.
//
// 面向 AdminFacingInterface 的 HTTP 管理接口（Control.AdminFacingInterface）：
// 网关态势 / 状态视图 / 网关管理 / 版本发布 / 升级计划。
// 后端接口尚未实现时，请求失败自动回退到 example 数据（source: "example"），
// 保证前端独立可渲染；接入真实后端后自动切换为 "real"。

export type GatewayStatus = "online" | "offline" | (string & {});
export type GatewayHealth = "healthy" | "degraded" | "unhealthy" | "unknown";

export interface GatewayStatusView {
  gatewayId: string;
  instanceId: string;
  version: string;
  status: GatewayStatus;
  health: GatewayHealth;
  memoryBytes: number | null;
  cpuPercent: number | null;
  lastSeenAt: string;
}

export interface GatewayListView {
  gatewayCount: number;
  onlineCount: number;
  degradedCount: number;
  offlineCount: number;
  updatedAt: string;
}

export interface GatewayUptime {
  gatewayId: string;
  window: string;
  /** 在线率 0..1；无历史数据 / VM 不可达 → null。 */
  uptime: number | null;
}

/** 网关在一个 Unix 秒时间戳上的历史指标采样。 */
export interface GatewayHistorySample {
  at: number;
  online: number | null;
  memoryBytes: number | null;
  cpuPercent: number | null;
}

/** 网关历史趋势；当前详情页请求最近 1 小时、每分钟一个采样点。 */
export interface GatewayHistory {
  gatewayId: string;
  window: string;
  stepSeconds: number;
  samples: GatewayHistorySample[];
}

/** 单个 Agent 的历史采样，额外包含管理接口时延。 */
export interface AgentHistorySample extends GatewayHistorySample {
  adminLatencyMs: number | null;
}

export interface AgentHistory {
  agentId: string;
  gatewayId: string;
  window: string;
  stepSeconds: number;
  samples: AgentHistorySample[];
}

export interface AgentStatusView {
  agentId: string;
  instanceId: string;
  version: string;
  status: GatewayStatus;
  health: GatewayHealth;
  memoryBytes: number | null;
  cpuPercent: number | null;
  adminLatencyMs: number | null;
  lastSeenAt: string;
}

export type GatewayInstanceLifecycleState =
  "Provisioned" | "Initializing" | "Running" | "Failed";

export interface GatewayInstance {
  gatewayId: string;
  instanceId: string;
  lifecycleState: GatewayInstanceLifecycleState;
  createdAt: string;
  initializedAt: string | null;
  /** 控制中心生成的不含凭证初始化入口；旧版本接口可能不返回。 */
  initUrl?: string;
}

/** 网关实例安装指引（创建后交付给操作者）。 */
export interface GatewayInstallInfo {
  installCommand: string;
  cloudImage: string;
  initUrl: string;
  /** Center 创建实例时签发的一次性置备引导 Token，仅在创建回执中交付。 */
  setupToken: string;
  /** 服务端生成的 curl 验证命令（Bearer 使用一次性置备引导 Token）。 */
  initCurl: string;
  /** 控制中心 CA 信任证书；未启用 TLS 时为空。 */
  trustBundlePem: string | null;
}

/** 创建网关实例返回：实例视图 + 安装指引（Gateway 启动后基于 initUrl 初始化）。 */
export interface AdminCreateGatewayInstanceReturned {
  instance: GatewayInstance;
  install: GatewayInstallInfo;
}

/** 网关生命周期一次状态转变记录（过程历史）。 */
export interface LifecycleEvent {
  gatewayId: string;
  fromState: GatewayInstanceLifecycleState | null;
  toState: GatewayInstanceLifecycleState;
  at: string;
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

export interface UpgradeTarget {
  component: string;
  targetVersion: string;
}

export interface UpgradeStep {
  stepIndex: number;
  gatewayIds: string[];
  status: string;
}

export interface UpgradePlan {
  planId: string;
  targets: UpgradeTarget[];
  targetCount: number;
  status: string;
  createdAt: string;
  steps: UpgradeStep[];
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

// ── 北向命令 ──

export interface CreateGatewayInstanceCommand {
  gatewayName: string;
  requestedBy: string;
}

export interface BindGatewayCustomerCommand {
  gatewayId: string;
  customerId: string;
  requestedBy: string;
}

export interface GetGatewayInitialConfigCommand {
  instanceId: string;
  requestedBy: string;
}

export interface PublishReleaseCommand {
  version: string;
  artifactUrl: string;
  requestedBy: string;
}

export interface CreateUpgradePlanCommand {
  targets: UpgradeTarget[];
  gatewayIds: string[];
  steps: UpgradeStep[];
  requestedBy: string;
}

export interface ApproveUpgradePlanCommand {
  planId: string;
  approvedBy: string;
}

// ── 结果信封：数据 + 来源标记 ──

export interface ExampleResult<T> {
  data: T;
  source: "real" | "example";
}

// ── 认证与请求包装 ──

export const ADMIN_AUTH_CHANGED_EVENT = "warpInsightCenterAuthChanged";
const ADMIN_API_TOKEN_STORAGE_KEY = "warpInsightCenterApiToken";
const GATEWAY_INIT_CURL_STORAGE_KEY = "warpInsightGatewayInitCurls";

let adminApiToken: string | null =
  typeof window !== "undefined"
    ? window.sessionStorage.getItem(ADMIN_API_TOKEN_STORAGE_KEY)
    : null;

export class ApiError extends Error {
  readonly status: number;
  readonly retryAfterSeconds?: number;

  constructor(status: number, path: string, retryAfterSeconds?: number) {
    super(`HTTP ${status} ${path}`);
    this.name = "ApiError";
    this.status = status;
    this.retryAfterSeconds = retryAfterSeconds;
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

/** 将创建回执中的 init curl 临时保存在当前浏览器会话，供实例详情页复用。 */
export function storeGatewayInitCurl(
  gatewayId: string,
  initCurl: string,
): void {
  if (typeof window === "undefined") return;
  try {
    const raw = window.sessionStorage.getItem(GATEWAY_INIT_CURL_STORAGE_KEY);
    const values: Record<string, string> = raw ? JSON.parse(raw) : {};
    values[gatewayId] = initCurl;
    window.sessionStorage.setItem(
      GATEWAY_INIT_CURL_STORAGE_KEY,
      JSON.stringify(values),
    );
  } catch {
    // 会话存储不可用时，创建回执仍会在当前页面直接展示命令。
  }
}

/** 读取当前会话中保存的 init curl；历史实例没有回执时返回 null。 */
export function readGatewayInitCurl(gatewayId: string): string | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.sessionStorage.getItem(GATEWAY_INIT_CURL_STORAGE_KEY);
    if (!raw) return null;
    const values: unknown = JSON.parse(raw);
    if (!values || typeof values !== "object") return null;
    const value = (values as Record<string, unknown>)[gatewayId];
    return typeof value === "string" && value.length > 0 ? value : null;
  } catch {
    return null;
  }
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

// ── 请求失败回退 example 数据 ──

async function fetchOrFallback<T>(
  path: string,
  fallback: () => T,
  init?: RequestInit,
): Promise<ExampleResult<T>> {
  try {
    const data = await requestJson<T>(path, init);
    return { data, source: "real" };
  } catch (error) {
    if (isRateLimitedError(error)) throw error;
    return { data: fallback(), source: "example" };
  }
}

// ── 类型归一化（snake_case / camelCase 容错） ──

function requiredString(value: unknown, fieldName: string): string {
  if (typeof value === "string") return value;
  throw new Error(`Invalid API response: missing ${fieldName}`);
}

function requiredNumber(value: unknown, fieldName: string): number {
  if (typeof value === "number") return value;
  throw new Error(`Invalid API response: missing ${fieldName}`);
}

function pick(obj: any, ...names: string[]): unknown {
  for (const name of names) {
    if (obj?.[name] !== undefined) return obj[name];
  }
  return undefined;
}

function nullableNumber(payload: any, ...names: string[]): number | null {
  const value = pick(payload, ...names);
  return typeof value === "number" ? value : null;
}

function normalizeGatewayStatus(value: unknown): GatewayStatus {
  // 模型 status 为 String，取值域未收紧；异常上报值不击穿整个列表，原样透传（徽标兜底显示离线）。
  if (typeof value === "string" && value.length > 0)
    return value as GatewayStatus;
  throw new Error("Invalid API response: invalid gateway status");
}

function normalizeGatewayHealth(value: unknown): GatewayHealth {
  if (value === "healthy" || value === "degraded" || value === "unhealthy")
    return value;
  // 未上报/异常健康值 → "unknown"，徽标显示「未知」。
  return "unknown";
}

function normalizeGatewayStatusView(payload: any): GatewayStatusView {
  return {
    gatewayId: requiredString(
      pick(payload, "gateway_id", "gatewayId"),
      "gateway.gatewayId",
    ),
    instanceId: requiredString(
      pick(payload, "instance_id", "instanceId"),
      "gateway.instanceId",
    ),
    version: requiredString(payload.version, "gateway.version"),
    status: normalizeGatewayStatus(payload.status),
    health: normalizeGatewayHealth(payload.health),
    memoryBytes: nullableNumber(payload, "memory_bytes", "memoryBytes"),
    cpuPercent: nullableNumber(payload, "cpu_percent", "cpuPercent"),
    lastSeenAt: requiredString(
      pick(payload, "last_seen_at", "lastSeenAt"),
      "gateway.lastSeenAt",
    ),
  };
}

function normalizeGatewayListView(payload: any): GatewayListView {
  return {
    gatewayCount: requiredNumber(
      pick(payload, "gateway_count", "gatewayCount"),
      "list.gatewayCount",
    ),
    onlineCount: requiredNumber(
      pick(payload, "online_count", "onlineCount"),
      "list.onlineCount",
    ),
    degradedCount: requiredNumber(
      pick(payload, "degraded_count", "degradedCount"),
      "list.degradedCount",
    ),
    offlineCount: requiredNumber(
      pick(payload, "offline_count", "offlineCount"),
      "list.offlineCount",
    ),
    updatedAt: requiredString(
      pick(payload, "updated_at", "updatedAt"),
      "list.updatedAt",
    ),
  };
}

function normalizeGatewayInstallInfo(payload: any): GatewayInstallInfo {
  return {
    installCommand: requiredString(
      pick(payload, "install_command", "installCommand"),
      "install.installCommand",
    ),
    cloudImage: requiredString(
      pick(payload, "cloud_image", "cloudImage"),
      "install.cloudImage",
    ),
    initUrl: requiredString(
      pick(payload, "init_url", "initUrl"),
      "install.initUrl",
    ),
    setupToken: requiredString(
      pick(payload, "setup_token", "setupToken"),
      "install.setupToken",
    ),
    initCurl: requiredString(
      pick(payload, "init_curl", "initCurl"),
      "install.initCurl",
    ),
    trustBundlePem:
      pick(payload, "trust_bundle_pem", "trustBundlePem") == null
        ? null
        : String(pick(payload, "trust_bundle_pem", "trustBundlePem")),
  };
}

function normalizeGatewayInstance(payload: any): GatewayInstance {
  const rawInitUrl = pick(payload, "init_url", "initUrl");
  return {
    gatewayId: requiredString(
      pick(payload, "gateway_id", "gatewayId"),
      "instance.gatewayId",
    ),
    instanceId: requiredString(
      pick(payload, "instance_id", "instanceId"),
      "instance.instanceId",
    ),
    lifecycleState:
      (requiredString(
        pick(payload, "lifecycle_state", "lifecycleState"),
        "instance.lifecycleState",
      ) as GatewayInstanceLifecycleState) ?? "Provisioned",
    initializedAt:
      pick(payload, "initialized_at", "initializedAt") === null ||
      pick(payload, "initialized_at", "initializedAt") === undefined
        ? null
        : String(pick(payload, "initialized_at", "initializedAt")),
    createdAt: requiredString(
      pick(payload, "created_at", "createdAt"),
      "instance.createdAt",
    ),
    initUrl:
      rawInitUrl === null || rawInitUrl === undefined
        ? undefined
        : String(rawInitUrl),
  };
}

function normalizeGatewayCustomerBinding(payload: any): GatewayCustomerBinding {
  return {
    gatewayId: requiredString(
      pick(payload, "gateway_id", "gatewayId"),
      "binding.gatewayId",
    ),
    customerId: requiredString(
      pick(payload, "customer_id", "customerId"),
      "binding.customerId",
    ),
    status: requiredString(payload.status, "binding.status"),
    boundAt: requiredString(
      pick(payload, "bound_at", "boundAt"),
      "binding.boundAt",
    ),
  };
}

function normalizeGatewayInitialConfig(payload: any): GatewayInitialConfig {
  return {
    controlCenterEndpoint: requiredString(
      pick(payload, "control_center_endpoint", "controlCenterEndpoint"),
      "config.controlCenterEndpoint",
    ),
    policyVersion: requiredString(
      pick(payload, "policy_version", "policyVersion"),
      "config.policyVersion",
    ),
    telemetryOutput: requiredString(
      pick(payload, "telemetry_output", "telemetryOutput"),
      "config.telemetryOutput",
    ),
  };
}

function normalizeRelease(payload: any): WistAgentdRelease {
  return {
    version: requiredString(payload.version, "release.version"),
    artifactUrl: requiredString(
      pick(payload, "artifact_url", "artifactUrl"),
      "release.artifactUrl",
    ),
    status: requiredString(payload.status, "release.status"),
    publishedAt: requiredString(
      pick(payload, "published_at", "publishedAt"),
      "release.publishedAt",
    ),
  };
}

function normalizeUpgradeTarget(payload: any): UpgradeTarget {
  return {
    component: requiredString(payload.component, "target.component"),
    targetVersion: requiredString(
      pick(payload, "target_version", "targetVersion"),
      "target.targetVersion",
    ),
  };
}

function normalizeUpgradeStep(payload: any): UpgradeStep {
  return {
    stepIndex: requiredNumber(
      pick(payload, "step_index", "stepIndex"),
      "step.stepIndex",
    ),
    gatewayIds: Array.isArray(pick(payload, "gateway_ids", "gatewayIds"))
      ? (pick(payload, "gateway_ids", "gatewayIds") as string[])
      : [],
    status: requiredString(payload.status, "step.status"),
  };
}

function normalizeUpgradePlan(payload: any): UpgradePlan {
  const rawTargets = Array.isArray(pick(payload, "targets"))
    ? (pick(payload, "targets") as any[])
    : [];
  const rawSteps = Array.isArray(pick(payload, "steps"))
    ? (pick(payload, "steps") as any[])
    : [];
  return {
    planId: requiredString(pick(payload, "plan_id", "planId"), "plan.planId"),
    targets: rawTargets.map(normalizeUpgradeTarget),
    targetCount: requiredNumber(
      pick(payload, "target_count", "targetCount"),
      "plan.targetCount",
    ),
    status: requiredString(payload.status, "plan.status"),
    createdAt: requiredString(
      pick(payload, "created_at", "createdAt"),
      "plan.createdAt",
    ),
    steps: rawSteps.map(normalizeUpgradeStep),
  };
}

function normalizeUpgradePlanApproval(payload: any): UpgradePlanApproval {
  return {
    planId: requiredString(
      pick(payload, "plan_id", "planId"),
      "approval.planId",
    ),
    status: requiredString(payload.status, "approval.status"),
    approvedBy: requiredString(
      pick(payload, "approved_by", "approvedBy"),
      "approval.approvedBy",
    ),
    approvedAt: requiredString(
      pick(payload, "approved_at", "approvedAt"),
      "approval.approvedAt",
    ),
  };
}

// ── example 数据 ──

function isoMinutesAgo(minutes: number): string {
  return new Date(Date.now() - minutes * 60_000).toISOString();
}

function exampleGatewayStatusView(): GatewayStatusView[] {
  return [
    {
      gatewayId: "gw-001",
      instanceId: "inst-7f2a",
      version: "v2.4.1",
      status: "online",
      health: "healthy",
      memoryBytes: 2 * 1024 ** 3,
      cpuPercent: 35,
      lastSeenAt: isoMinutesAgo(1),
    },
    {
      gatewayId: "gw-002",
      instanceId: "inst-9c31",
      version: "v2.4.1",
      status: "online",
      health: "degraded",
      memoryBytes: 1536 * 1024 ** 2,
      cpuPercent: 68,
      lastSeenAt: isoMinutesAgo(4),
    },
    {
      gatewayId: "gw-003",
      instanceId: "inst-1d8b",
      version: "v2.3.0",
      status: "offline",
      health: "unhealthy",
      memoryBytes: 768 * 1024 ** 2,
      cpuPercent: 12,
      lastSeenAt: isoMinutesAgo(138),
    },
    {
      gatewayId: "gw-004",
      instanceId: "inst-4e77",
      version: "v2.4.0",
      status: "online",
      health: "healthy",
      memoryBytes: 2 * 1024 ** 3,
      cpuPercent: 41,
      lastSeenAt: isoMinutesAgo(2),
    },
    {
      gatewayId: "gw-005",
      instanceId: "inst-aa21",
      version: "v2.2.2",
      status: "offline",
      health: "unhealthy",
      memoryBytes: 512 * 1024 ** 2,
      cpuPercent: 5,
      lastSeenAt: isoMinutesAgo(420),
    },
    {
      gatewayId: "gw-006",
      instanceId: "inst-38c4",
      version: "v2.4.1",
      status: "online",
      health: "healthy",
      memoryBytes: 2 * 1024 ** 3,
      cpuPercent: 28,
      lastSeenAt: isoMinutesAgo(0),
    },
  ];
}

function exampleGatewayListView(): GatewayListView {
  return {
    gatewayCount: 6,
    onlineCount: 4,
    degradedCount: 1,
    offlineCount: 2,
    updatedAt: new Date().toISOString(),
  };
}

// 每个网关稳定的示例在线率（0.5~0.99，由 gateway_id 派生）。
function exampleGatewayUptime(
  gatewayId: string,
  window: string,
): GatewayUptime {
  let hash = 0;
  for (const ch of gatewayId) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  const uptime = 0.5 + (hash % 50) / 100;
  return { gatewayId, window, uptime };
}

function normalizeGatewayUptime(
  payload: any,
  fallbackGatewayId: string,
  fallbackWindow: string,
): GatewayUptime {
  return {
    gatewayId:
      requiredString(
        pick(payload, "gateway_id", "gatewayId"),
        "uptime.gatewayId",
      ) || fallbackGatewayId,
    window:
      requiredString(pick(payload, "window"), "uptime.window") ||
      fallbackWindow,
    uptime: typeof payload.uptime === "number" ? payload.uptime : null,
  };
}

function exampleGatewayHistory(
  gatewayId: string,
  window: string,
): GatewayHistory {
  let hash = 0;
  for (const ch of gatewayId) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  const stepSeconds = window === "24h" ? 900 : window === "6h" ? 300 : 60;
  const pointCount = Math.floor(
    (window === "24h" ? 86_400 : window === "6h" ? 21_600 : 3_600) /
      stepSeconds,
  );
  const end = Math.floor(Date.now() / stepSeconds / 1000) * stepSeconds;
  const baseMemory = (1.4 + (hash % 8) / 10) * 1024 ** 3;
  const samples = Array.from({ length: pointCount + 1 }, (_, index) => {
    const phase = (index + (hash % 17)) / 6;
    return {
      at: end - (pointCount - index) * stepSeconds,
      online: index === Math.floor(pointCount * 0.28) ? 0 : 1,
      memoryBytes: baseMemory + Math.sin(phase * 0.7) * 110 * 1024 ** 2,
      cpuPercent: 34 + Math.sin(phase) * 11 + Math.cos(phase * 0.35) * 5,
    };
  });
  return { gatewayId, window, stepSeconds, samples };
}

function normalizeGatewayHistory(
  payload: unknown,
  fallbackGatewayId: string,
  fallbackWindow: string,
): GatewayHistory {
  const record =
    typeof payload === "object" && payload !== null
      ? (payload as Record<string, unknown>)
      : {};
  const rawSamples = pick(record, "samples");
  const samples = Array.isArray(rawSamples)
    ? rawSamples.map((sample, index): GatewayHistorySample => {
        const item =
          typeof sample === "object" && sample !== null
            ? (sample as Record<string, unknown>)
            : {};
        return {
          at: requiredNumber(pick(item, "at"), `history.samples[${index}].at`),
          online: nullableNumber(item, "online"),
          memoryBytes: nullableNumber(item, "memory_bytes", "memoryBytes"),
          cpuPercent: nullableNumber(item, "cpu_percent", "cpuPercent"),
        };
      })
    : [];
  return {
    gatewayId:
      requiredString(
        pick(record, "gateway_id", "gatewayId"),
        "history.gatewayId",
      ) || fallbackGatewayId,
    window:
      requiredString(pick(record, "window"), "history.window") ||
      fallbackWindow,
    stepSeconds: requiredNumber(
      pick(record, "step_seconds", "stepSeconds"),
      "history.stepSeconds",
    ),
    samples,
  };
}

function exampleAgentHistory(
  gatewayId: string,
  agentId: string,
  window: string,
): AgentHistory {
  let hash = 0;
  for (const ch of agentId) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  const stepSeconds = window === "24h" ? 900 : window === "6h" ? 300 : 60;
  const pointCount = Math.floor(
    (window === "24h" ? 86_400 : window === "6h" ? 21_600 : 3_600) /
      stepSeconds,
  );
  const end = Math.floor(Date.now() / stepSeconds / 1000) * stepSeconds;
  const samples = Array.from({ length: pointCount + 1 }, (_, index) => {
    const phase = (index + (hash % 19)) / 5;
    return {
      at: end - (pointCount - index) * stepSeconds,
      online: index === Math.floor(pointCount * 0.42) && hash % 3 === 0 ? 0 : 1,
      memoryBytes:
        (0.3 + (hash % 5) / 10) * 1024 ** 3 +
        Math.sin(phase * 0.65) * 32 * 1024 ** 2,
      cpuPercent: 24 + Math.sin(phase) * 15 + Math.cos(phase * 0.32) * 6,
      adminLatencyMs: 8 + Math.round(Math.abs(Math.sin(phase * 0.8)) * 9),
    };
  });
  return { agentId, gatewayId, window, stepSeconds, samples };
}

function normalizeAgentHistory(
  payload: unknown,
  fallbackGatewayId: string,
  fallbackAgentId: string,
  fallbackWindow: string,
): AgentHistory {
  const record =
    typeof payload === "object" && payload !== null
      ? (payload as Record<string, unknown>)
      : {};
  const rawSamples = pick(record, "samples");
  const samples = Array.isArray(rawSamples)
    ? rawSamples.map((sample, index): AgentHistorySample => {
        const item =
          typeof sample === "object" && sample !== null
            ? (sample as Record<string, unknown>)
            : {};
        return {
          at: requiredNumber(pick(item, "at"), `history.samples[${index}].at`),
          online: nullableNumber(item, "online"),
          memoryBytes: nullableNumber(item, "memory_bytes", "memoryBytes"),
          cpuPercent: nullableNumber(item, "cpu_percent", "cpuPercent"),
          adminLatencyMs: nullableNumber(
            item,
            "admin_latency_ms",
            "adminLatencyMs",
          ),
        };
      })
    : [];
  return {
    gatewayId:
      requiredString(
        pick(record, "gateway_id", "gatewayId"),
        "history.gatewayId",
      ) || fallbackGatewayId,
    agentId:
      requiredString(pick(record, "agent_id", "agentId"), "history.agentId") ||
      fallbackAgentId,
    window:
      requiredString(pick(record, "window"), "history.window") ||
      fallbackWindow,
    stepSeconds: requiredNumber(
      pick(record, "step_seconds", "stepSeconds"),
      "history.stepSeconds",
    ),
    samples,
  };
}

function normalizeLifecycleEvent(payload: any): LifecycleEvent {
  return {
    gatewayId: requiredString(
      pick(payload, "gateway_id", "gatewayId"),
      "lifecycle.gatewayId",
    ),
    fromState:
      (payload.from_state as GatewayInstanceLifecycleState) ??
      (payload.fromState as GatewayInstanceLifecycleState) ??
      null,
    toState: requiredString(
      pick(payload, "to_state", "toState"),
      "lifecycle.toState",
    ) as GatewayInstanceLifecycleState,
    at: requiredString(pick(payload, "at"), "lifecycle.at"),
  };
}

function exampleGatewayLifecycle(gatewayId: string): LifecycleEvent[] {
  return [
    {
      gatewayId,
      fromState: null,
      toState: "Provisioned",
      at: isoMinutesAgo(60 * 24 * 2),
    },
    {
      gatewayId,
      fromState: "Provisioned",
      toState: "Initializing",
      at: isoMinutesAgo(30),
    },
    {
      gatewayId,
      fromState: "Initializing",
      toState: "Running",
      at: isoMinutesAgo(5),
    },
  ];
}

function normalizeAgentStatusView(payload: any): AgentStatusView {
  return {
    agentId: requiredString(
      pick(payload, "agent_id", "agentId"),
      "agent.agentId",
    ),
    instanceId: requiredString(
      pick(payload, "instance_id", "instanceId"),
      "agent.instanceId",
    ),
    version: requiredString(payload.version, "agent.version"),
    status: requiredString(payload.status, "agent.status") as GatewayStatus,
    health: requiredString(payload.health, "agent.health") as GatewayHealth,
    memoryBytes: nullableNumber(payload, "memory_bytes", "memoryBytes"),
    cpuPercent: nullableNumber(payload, "cpu_percent", "cpuPercent"),
    adminLatencyMs: nullableNumber(
      payload,
      "admin_latency_ms",
      "adminLatencyMs",
    ),
    lastSeenAt: requiredString(
      pick(payload, "last_seen_at", "lastSeenAt"),
      "agent.lastSeenAt",
    ),
  };
}

function exampleAgentStatus(gatewayId: string): AgentStatusView[] {
  return [
    {
      agentId: `${gatewayId}-agent-1`,
      instanceId: `inst-${gatewayId}-a1`,
      version: "v0.3.2",
      status: "online",
      health: "healthy",
      memoryBytes: 512 * 1024 * 1024,
      cpuPercent: 20,
      adminLatencyMs: 8,
      lastSeenAt: isoMinutesAgo(0),
    },
    {
      agentId: `${gatewayId}-agent-2`,
      instanceId: `inst-${gatewayId}-a2`,
      version: "v0.3.0",
      status: "online",
      health: "degraded",
      memoryBytes: 384 * 1024 * 1024,
      cpuPercent: 45,
      adminLatencyMs: 15,
      lastSeenAt: isoMinutesAgo(1),
    },
  ];
}

function exampleGatewayInstance(
  command: CreateGatewayInstanceCommand,
): AdminCreateGatewayInstanceReturned {
  const gatewayId = command.gatewayName.trim();
  const initEndpoint = `http://127.0.0.1:3100/api/v1/gateway/initial-config?instance_id=${gatewayId}`;
  // 示例数据也模拟 Center 自动签发，避免前端重新承担凭据输入职责。
  const bootstrapToken = `boot_${Math.random().toString(36).slice(2, 14)}`;
  // init_url 不携带凭证（token 不进 URL），凭证走 config.toml / Authorization Header。
  const initUrl = initEndpoint;
  return {
    instance: {
      gatewayId,
      instanceId: `inst-${Math.random().toString(36).slice(2, 6)}`,
      lifecycleState: "Provisioned",
      createdAt: new Date().toISOString(),
      initializedAt: null,
    },
    install: {
      installCommand: `docker run -d --name warp-gateway-${gatewayId} -e WARP_GATEWAY_INIT_URL="${initUrl}" -e WARP_GATEWAY_BOOTSTRAP_TOKEN="${bootstrapToken}" warp-gateway:latest`,
      cloudImage: "warp-gateway:latest",
      initUrl,
      setupToken: bootstrapToken,
      initCurl: `curl -H "Authorization: Bearer ${bootstrapToken}" "${initEndpoint}"`,
      trustBundlePem: null,
    },
  };
}

function exampleBinding(
  command: BindGatewayCustomerCommand,
): GatewayCustomerBinding {
  return {
    gatewayId: command.gatewayId,
    customerId: command.customerId,
    status: "bound",
    boundAt: new Date().toISOString(),
  };
}

function exampleInitialConfig(
  command: GetGatewayInitialConfigCommand,
): GatewayInitialConfig {
  return {
    controlCenterEndpoint: "https://center.example.com",
    policyVersion: "policy-v12",
    telemetryOutput: "otlp://telemetry.example.com:4317",
  };
}

function exampleRelease(command: PublishReleaseCommand): WistAgentdRelease {
  return {
    version: command.version,
    artifactUrl: command.artifactUrl,
    status: "published",
    publishedAt: new Date().toISOString(),
  };
}

function exampleUpgradePlan(command: CreateUpgradePlanCommand): UpgradePlan {
  return {
    planId: `plan-${Math.random().toString(36).slice(2, 8)}`,
    targets: command.targets,
    targetCount: command.gatewayIds.length,
    status: "pending",
    createdAt: new Date().toISOString(),
    steps: command.steps,
  };
}

function exampleApproval(
  command: ApproveUpgradePlanCommand,
): UpgradePlanApproval {
  return {
    planId: command.planId,
    status: "approved",
    approvedBy: command.approvedBy,
    approvedAt: new Date().toISOString(),
  };
}

// ── 接口调用 ──

export async function fetchGatewayStatusView(): Promise<
  ExampleResult<GatewayStatusView[]>
> {
  return fetchOrFallback(
    "/api/v1/admin/gateways/status",
    exampleGatewayStatusView,
  ).then(async (result) => {
    if (result.source !== "real") return result;
    const raw = result.data as any;
    const items = Array.isArray(raw)
      ? raw
      : Array.isArray(raw?.statuses)
        ? raw.statuses
        : Array.isArray(raw?.gateways)
          ? raw.gateways
          : raw?.status
            ? [raw.status]
            : [];
    return { ...result, data: items.map(normalizeGatewayStatusView) };
  });
}

export async function fetchGatewayList(): Promise<
  ExampleResult<GatewayListView>
> {
  return fetchOrFallback("/api/v1/admin/gateways", exampleGatewayListView).then(
    async (result) => {
      if (result.source !== "real") return result;
      const raw = result.data as any;
      return {
        ...result,
        data: normalizeGatewayListView(raw?.list ?? raw),
      };
    },
  );
}

export async function fetchGatewayInstances(): Promise<
  ExampleResult<GatewayInstance[]>
> {
  return fetchOrFallback(
    "/api/v1/admin/gateways/instances",
    exampleGatewayInstances,
  ).then(async (result) => {
    if (result.source !== "real") return result;
    const raw = result.data as any;
    const items = Array.isArray(raw) ? raw : [];
    return { ...result, data: items.map(normalizeGatewayInstance) };
  });
}

function exampleGatewayInstances(): GatewayInstance[] {
  const now = new Date().toISOString();
  return [
    {
      gatewayId: "gw-001",
      instanceId: "inst-7f2a",
      lifecycleState: "Running",
      createdAt: now,
      initializedAt: now,
      initUrl:
        "http://127.0.0.1:3100/api/v1/gateway/initial-config?instance_id=gw-001",
    },
    {
      gatewayId: "gw-002",
      instanceId: "inst-9c31",
      lifecycleState: "Initializing",
      createdAt: now,
      initializedAt: null,
      initUrl:
        "http://127.0.0.1:3100/api/v1/gateway/initial-config?instance_id=gw-002",
    },
    {
      gatewayId: "gw-003",
      instanceId: "inst-1d8b",
      lifecycleState: "Provisioned",
      createdAt: now,
      initializedAt: null,
      initUrl:
        "http://127.0.0.1:3100/api/v1/gateway/initial-config?instance_id=gw-003",
    },
    {
      gatewayId: "gw-004",
      instanceId: "inst-4e77",
      lifecycleState: "Failed",
      createdAt: now,
      initializedAt: null,
      initUrl:
        "http://127.0.0.1:3100/api/v1/gateway/initial-config?instance_id=gw-004",
    },
  ];
}

export async function fetchGatewayUptime(
  gatewayId: string,
  window = "1h",
): Promise<ExampleResult<GatewayUptime>> {
  const path = `/api/v1/admin/gateways/${encodeURIComponent(
    gatewayId,
  )}/status/uptime?window=${encodeURIComponent(window)}`;
  return fetchOrFallback(path, () =>
    exampleGatewayUptime(gatewayId, window),
  ).then(async (result) => {
    if (result.source !== "real") return result;
    return {
      ...result,
      data: normalizeGatewayUptime(result.data as any, gatewayId, window),
    };
  });
}

/** 获取网关历史趋势；请求失败时返回稳定的示例序列供独立前端演示。 */
export async function fetchGatewayHistory(
  gatewayId: string,
  window = "1h",
): Promise<ExampleResult<GatewayHistory>> {
  const path = `/api/v1/admin/gateways/${encodeURIComponent(
    gatewayId,
  )}/status/history?window=${encodeURIComponent(window)}`;
  return fetchOrFallback(path, () =>
    exampleGatewayHistory(gatewayId, window),
  ).then(async (result) => {
    if (result.source !== "real") return result;
    return {
      ...result,
      data: normalizeGatewayHistory(result.data, gatewayId, window),
    };
  });
}

/** 获取单个 Agent 的历史趋势；没有真实数据时回退为稳定示例序列。 */
export async function fetchAgentHistory(
  gatewayId: string,
  agentId: string,
  window = "1h",
): Promise<ExampleResult<AgentHistory>> {
  const path = `/api/v1/admin/gateways/${encodeURIComponent(
    gatewayId,
  )}/agents/${encodeURIComponent(agentId)}/history?window=${encodeURIComponent(
    window,
  )}`;
  return fetchOrFallback(path, () =>
    exampleAgentHistory(gatewayId, agentId, window),
  ).then(async (result) => {
    if (result.source !== "real") return result;
    return {
      ...result,
      data: normalizeAgentHistory(result.data, gatewayId, agentId, window),
    };
  });
}

export async function fetchGatewayAgents(
  gatewayId: string,
): Promise<ExampleResult<AgentStatusView[]>> {
  const path = `/api/v1/admin/gateways/${encodeURIComponent(gatewayId)}/agents`;
  return fetchOrFallback(path, () => exampleAgentStatus(gatewayId)).then(
    async (result) => {
      if (result.source !== "real") return result;
      const raw = result.data as any;
      const items = Array.isArray(raw)
        ? raw
        : Array.isArray(raw?.agents)
          ? raw.agents
          : [];
      return { ...result, data: items.map(normalizeAgentStatusView) };
    },
  );
}

export async function fetchGatewayLifecycle(
  gatewayId: string,
): Promise<ExampleResult<LifecycleEvent[]>> {
  const path = `/api/v1/admin/gateways/${encodeURIComponent(gatewayId)}/lifecycle`;
  return fetchOrFallback(path, () => exampleGatewayLifecycle(gatewayId)).then(
    async (result) => {
      if (result.source !== "real") return result;
      const raw = result.data as any;
      const items = Array.isArray(raw) ? raw : [];
      return { ...result, data: items.map(normalizeLifecycleEvent) };
    },
  );
}

export async function fetchGatewayStatus(
  gatewayId: string,
): Promise<ExampleResult<GatewayStatusView | null>> {
  const path = `/api/v1/admin/gateways/${encodeURIComponent(gatewayId)}/status`;
  return fetchOrFallback(path, () => null).then(async (result) => {
    if (result.source !== "real") return result;
    const raw = result.data as any;
    const item = raw?.status ?? raw;
    return { ...result, data: item ? normalizeGatewayStatusView(item) : null };
  });
}

export async function createGatewayInstance(
  command: CreateGatewayInstanceCommand,
): Promise<ExampleResult<AdminCreateGatewayInstanceReturned>> {
  return fetchOrFallback(
    "/api/v1/admin/gateways/instances",
    () => exampleGatewayInstance(command),
    {
      method: "POST",
      body: JSON.stringify({
        gateway_name: command.gatewayName,
        requested_by: command.requestedBy,
      }),
    },
  ).then(async (result) => {
    if (result.source === "real") {
      const raw = result.data as any;
      return {
        ...result,
        data: {
          instance: normalizeGatewayInstance(raw.instance ?? raw),
          install: normalizeGatewayInstallInfo(raw.install ?? {}),
        },
      };
    }
    return result;
  });
}

export async function bindGatewayCustomer(
  command: BindGatewayCustomerCommand,
): Promise<ExampleResult<GatewayCustomerBinding>> {
  return fetchOrFallback(
    "/api/v1/admin/gateways/bind",
    () => exampleBinding(command),
    {
      method: "POST",
      body: JSON.stringify({
        gateway_id: command.gatewayId,
        customer_id: command.customerId,
        requested_by: command.requestedBy,
      }),
    },
  ).then(async (result) => {
    if (result.source === "real") {
      return { ...result, data: normalizeGatewayCustomerBinding(result.data) };
    }
    return result;
  });
}

export async function fetchGatewayInitialConfig(
  command: GetGatewayInitialConfigCommand,
): Promise<ExampleResult<GatewayInitialConfig>> {
  const path = `/api/v1/admin/gateways/instances/${encodeURIComponent(command.instanceId)}/config`;
  return fetchOrFallback(path, () => exampleInitialConfig(command)).then(
    async (result) => {
      if (result.source === "real") {
        return { ...result, data: normalizeGatewayInitialConfig(result.data) };
      }
      return result;
    },
  );
}

export async function fetchReleases(
  component: string,
): Promise<ExampleResult<WistAgentdRelease[]>> {
  const path = `/api/v1/admin/releases/${encodeURIComponent(component)}`;
  return fetchOrFallback(path, () => exampleReleases(component)).then(
    async (result) => {
      if (result.source !== "real") return result;
      const raw = result.data as any;
      const items = Array.isArray(raw) ? raw : [];
      return { ...result, data: items.map(normalizeRelease) };
    },
  );
}

function exampleReleases(component: string): WistAgentdRelease[] {
  return [
    {
      version: "v2.4.1",
      artifactUrl: `http://127.0.0.1:3100/api/v1/releases/artifact/${component}/v2.4.1/${component}-v2.4.1.bin`,
      status: "published",
      publishedAt: new Date().toISOString(),
    },
  ];
}

export async function publishWistAgentd(
  command: PublishReleaseCommand,
): Promise<ExampleResult<WistAgentdRelease>> {
  return fetchOrFallback(
    "/api/v1/admin/releases/wist-agentd",
    () => exampleRelease(command),
    {
      method: "POST",
      body: JSON.stringify({
        version: command.version,
        artifact_url: command.artifactUrl,
        requested_by: command.requestedBy,
      }),
    },
  ).then(async (result) => {
    if (result.source === "real") {
      return { ...result, data: normalizeRelease(result.data) };
    }
    return result;
  });
}

export async function publishWarpGateWay(
  command: PublishReleaseCommand,
): Promise<ExampleResult<WarpGateWayRelease>> {
  return fetchOrFallback(
    "/api/v1/admin/releases/warp-gateway",
    () => exampleRelease(command),
    {
      method: "POST",
      body: JSON.stringify({
        version: command.version,
        artifact_url: command.artifactUrl,
        requested_by: command.requestedBy,
      }),
    },
  ).then(async (result) => {
    if (result.source === "real") {
      return { ...result, data: normalizeRelease(result.data) };
    }
    return result;
  });
}

export async function fetchUpgradePlans(): Promise<
  ExampleResult<UpgradePlan[]>
> {
  return fetchOrFallback(
    "/api/v1/admin/upgrade-plans",
    exampleUpgradePlans,
  ).then(async (result) => {
    if (result.source !== "real") return result;
    const raw = result.data as any;
    const items = Array.isArray(raw) ? raw : [];
    return { ...result, data: items.map(normalizeUpgradePlan) };
  });
}

function exampleUpgradePlans(): UpgradePlan[] {
  return [
    {
      planId: "plan-example-1",
      targets: [
        { component: "wist-agentd", targetVersion: "v2.5.0" },
        { component: "warp-gateway", targetVersion: "v3.1.0" },
      ],
      targetCount: 2,
      status: "pending",
      createdAt: new Date().toISOString(),
      steps: [
        { stepIndex: 0, gatewayIds: ["gw-001"], status: "pending" },
        { stepIndex: 1, gatewayIds: ["gw-002"], status: "pending" },
      ],
    },
  ];
}

export async function createUpgradePlan(
  command: CreateUpgradePlanCommand,
): Promise<ExampleResult<UpgradePlan>> {
  return fetchOrFallback(
    "/api/v1/admin/upgrade-plans",
    () => exampleUpgradePlan(command),
    {
      method: "POST",
      body: JSON.stringify({
        targets: command.targets,
        gateway_ids: command.gatewayIds,
        steps: command.steps,
        requested_by: command.requestedBy,
      }),
    },
  ).then(async (result) => {
    if (result.source === "real") {
      return { ...result, data: normalizeUpgradePlan(result.data) };
    }
    return result;
  });
}

export async function approveUpgradePlan(
  command: ApproveUpgradePlanCommand,
): Promise<ExampleResult<UpgradePlanApproval>> {
  return fetchOrFallback(
    "/api/v1/admin/upgrade-plans/approve",
    () => exampleApproval(command),
    {
      method: "POST",
      body: JSON.stringify({
        plan_id: command.planId,
        approved_by: command.approvedBy,
      }),
    },
  ).then(async (result) => {
    if (result.source === "real") {
      return { ...result, data: normalizeUpgradePlanApproval(result.data) };
    }
    return result;
  });
}
