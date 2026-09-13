import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ADMIN_AUTH_CHANGED_EVENT,
  approveUpgradePlan,
  bindGatewayCustomer,
  createGatewayInstance,
  createUpgradePlan,
  fetchGatewayAgents,
  fetchAgentHistory,
  fetchGatewayHistory,
  fetchGatewayInstances,
  fetchGatewayLifecycle,
  fetchGatewayList,
  fetchGatewayInitialConfig,
  fetchGatewayStatus,
  fetchGatewayStatusView,
  fetchGatewayUptime,
  fetchReleases,
  fetchUpgradePlans,
  getAdminApiToken,
  publishWistAgentd,
  publishWarpGateWay,
  type ApproveUpgradePlanCommand,
  type BindGatewayCustomerCommand,
  type CreateGatewayInstanceCommand,
  type CreateUpgradePlanCommand,
  type GetGatewayInitialConfigCommand,
  type PublishReleaseCommand,
} from "../api";

// 当 Admin Token 变化时触发重渲染，使查询能立即从禁用切到启用。
function useAuthVersion() {
  const [, setAuthVersion] = useState(0);
  useEffect(() => {
    const onAuthChanged = () => setAuthVersion((version) => version + 1);
    window.addEventListener(ADMIN_AUTH_CHANGED_EVENT, onAuthChanged);
    return () =>
      window.removeEventListener(ADMIN_AUTH_CHANGED_EVENT, onAuthChanged);
  }, []);
}

export function useGatewayStatusView() {
  useAuthVersion();
  const enabled = Boolean(getAdminApiToken());
  return useQuery({
    queryKey: ["gateway-status-view"],
    queryFn: fetchGatewayStatusView,
    // 有真实后端时 5s 轮询刷新；未配置 token 时也能以 example 数据渲染。
    refetchInterval: enabled ? 5_000 : 30_000,
  });
}

export function useGatewayStatus(gatewayId: string) {
  useAuthVersion();
  const enabled = Boolean(getAdminApiToken());
  return useQuery({
    queryKey: ["gateway-status", gatewayId],
    queryFn: () => fetchGatewayStatus(gatewayId),
    refetchInterval: enabled ? 5_000 : 30_000,
    enabled: Boolean(gatewayId),
  });
}

/** 轮询网关历史序列；Query key 包含窗口，避免不同时间范围共享缓存。 */
export function useGatewayHistory(gatewayId: string, window = "1h") {
  useAuthVersion();
  const enabled = Boolean(gatewayId);
  return useQuery({
    queryKey: ["gateway-history", gatewayId, window],
    queryFn: () => fetchGatewayHistory(gatewayId, window),
    refetchInterval: getAdminApiToken() ? 15_000 : 30_000,
    enabled,
  });
}

/** 批量查询列表页网关历史，避免每张卡片各自创建一套 Query 生命周期。 */
export function useGatewayHistories(gatewayIds: string[], window = "1h") {
  useAuthVersion();
  const hasAdminToken = Boolean(getAdminApiToken());
  const enabled = gatewayIds.length > 0;
  return useQuery({
    queryKey: ["gateway-histories", gatewayIds, window],
    queryFn: async () => {
      const results = await Promise.all(
        gatewayIds.map((gatewayId) => fetchGatewayHistory(gatewayId, window)),
      );
      return Object.fromEntries(
        results.map((result) => [result.data.gatewayId, result]),
      );
    },
    refetchInterval: hasAdminToken ? 15_000 : 30_000,
    enabled,
  });
}

/** 批量查询当前网关下 Agent 的历史，保持每个 Agent 独立缓存和错误回退。 */
export function useAgentHistories(gatewayId: string, agentIds: string[]) {
  useAuthVersion();
  const hasAdminToken = Boolean(getAdminApiToken());
  const enabled = Boolean(gatewayId) && agentIds.length > 0;
  return useQuery({
    queryKey: ["agent-histories", gatewayId, agentIds],
    queryFn: async () => {
      const results = await Promise.all(
        agentIds.map((agentId) => fetchAgentHistory(gatewayId, agentId)),
      );
      return Object.fromEntries(
        results.map((result) => [result.data.agentId, result]),
      );
    },
    refetchInterval: hasAdminToken ? 15_000 : 30_000,
    enabled,
  });
}

export function useGatewayLifecycle(gatewayId: string) {
  useAuthVersion();
  const enabled = Boolean(getAdminApiToken());
  return useQuery({
    queryKey: ["gateway-lifecycle", gatewayId],
    queryFn: () => fetchGatewayLifecycle(gatewayId),
    refetchInterval: enabled ? 10_000 : 30_000,
    enabled: Boolean(gatewayId),
  });
}

/** 批量查询多网关下的 Agent 状态（版本发布页展示各 Agent 当前版本）。 */
export function useGatewayAgentsForAll(gatewayIds: string[]) {
  useAuthVersion();
  const enabled = gatewayIds.length > 0;
  return useQuery({
    queryKey: ["all-gateway-agents", gatewayIds],
    queryFn: async () => {
      const results = await Promise.all(
        gatewayIds.map((gatewayId) => fetchGatewayAgents(gatewayId)),
      );
      return results.flatMap((result, index) =>
        (result.data ?? []).map((agent) => ({
          ...agent,
          gatewayId: gatewayIds[index],
        })),
      );
    },
    refetchInterval: getAdminApiToken() ? 15_000 : 30_000,
    enabled,
  });
}

export function useGatewayAgents(gatewayId: string) {
  useAuthVersion();
  const enabled = Boolean(getAdminApiToken());
  return useQuery({
    queryKey: ["gateway-agents", gatewayId],
    queryFn: () => fetchGatewayAgents(gatewayId),
    refetchInterval: enabled ? 5_000 : 30_000,
    enabled: Boolean(gatewayId),
  });
}

export function useGatewayList() {
  useAuthVersion();
  const enabled = Boolean(getAdminApiToken());
  return useQuery({
    queryKey: ["gateway-list"],
    queryFn: fetchGatewayList,
    refetchInterval: enabled ? 5_000 : 30_000,
  });
}

/** 一次拉取多个网关的在线率，返回 { gateway_id: uptime|null } 映射（列表页用）。 */
export function useReleases(component: string) {
  useAuthVersion();
  const enabled = Boolean(getAdminApiToken());
  return useQuery({
    queryKey: ["releases", component],
    queryFn: () => fetchReleases(component),
    refetchInterval: enabled ? 15_000 : 30_000,
  });
}

export function useGatewayInstances() {
  useAuthVersion();
  const enabled = Boolean(getAdminApiToken());
  return useQuery({
    queryKey: ["gateway-instances"],
    queryFn: fetchGatewayInstances,
    refetchInterval: enabled ? 10_000 : 30_000,
  });
}

export function useGatewayUptimes(gatewayIds: string[]) {
  useAuthVersion();
  const hasAdminToken = Boolean(getAdminApiToken());
  // 无 Token 时仍请求一次，让 fetchOrFallback 提供与历史趋势一致的示例在线率。
  const enabled = gatewayIds.length > 0;
  return useQuery({
    queryKey: ["gateway-uptimes", gatewayIds],
    queryFn: async () => {
      const results = await Promise.all(
        gatewayIds.map((id) => fetchGatewayUptime(id)),
      );
      const map: Record<string, number | null> = {};
      for (const result of results) {
        map[result.data.gatewayId] = result.data.uptime;
      }
      return map;
    },
    enabled,
    refetchInterval: hasAdminToken ? 5_000 : 30_000,
  });
}

export function useCreateGatewayInstance() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (command: CreateGatewayInstanceCommand) =>
      createGatewayInstance(command),
    // 创建成功后立即刷新总览，避免等待轮询周期才能看到新实例。
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["gateway-instances"] }),
  });
}

export function useBindGatewayCustomer() {
  return useMutation({
    mutationFn: (command: BindGatewayCustomerCommand) =>
      bindGatewayCustomer(command),
  });
}

export function useGatewayInitialConfig() {
  return useMutation({
    mutationFn: (command: GetGatewayInitialConfigCommand) =>
      fetchGatewayInitialConfig(command),
  });
}

export function usePublishWistAgentd() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (command: PublishReleaseCommand) => publishWistAgentd(command),
    // 发布成功后刷新当前组件的历史，立即反馈新版本已进入发布记录。
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["releases", "wist-agentd"] }),
  });
}

export function usePublishWarpGateWay() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (command: PublishReleaseCommand) => publishWarpGateWay(command),
    // 发布成功后刷新当前组件的历史，立即反馈新版本已进入发布记录。
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["releases", "warp-gateway"] }),
  });
}

export function useUpgradePlans() {
  useAuthVersion();
  const enabled = Boolean(getAdminApiToken());
  return useQuery({
    queryKey: ["upgrade-plans"],
    queryFn: fetchUpgradePlans,
    refetchInterval: enabled ? 15_000 : 30_000,
  });
}

export function useCreateUpgradePlan() {
  return useMutation({
    mutationFn: (command: CreateUpgradePlanCommand) =>
      createUpgradePlan(command),
  });
}

export function useApproveUpgradePlan() {
  return useMutation({
    mutationFn: (command: ApproveUpgradePlanCommand) =>
      approveUpgradePlan(command),
  });
}
