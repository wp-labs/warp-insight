import { useEffect, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import {
  ADMIN_AUTH_CHANGED_EVENT,
  fetchAgentHostMetrics,
  fetchAgentInstallCode,
  fetchAgentOverview,
  fetchAllAgentsHostMetrics,
  getAdminApiToken,
  initializeGatewayViaUrl,
  pauseAgent,
  upgradeAgent,
  type PauseAgentCommand,
  type UpgradeAgentCommand,
} from "../api";

export function useAgentOverview() {
  // Re-render when the admin token changes so the query can transition from
  // disabled (no token) to enabled (token entered) and fetch immediately.
  const [, setAuthVersion] = useState(0);
  useEffect(() => {
    const onAuthChanged = () => setAuthVersion((version) => version + 1);
    window.addEventListener(ADMIN_AUTH_CHANGED_EVENT, onAuthChanged);
    return () =>
      window.removeEventListener(ADMIN_AUTH_CHANGED_EVENT, onAuthChanged);
  }, []);
  // Do not poll before a token is entered: every unauth'ed request would
  // otherwise hit the admin's per-IP rate limiter (5 failures -> 60s block).
  const enabled = Boolean(getAdminApiToken());
  return useQuery({
    queryKey: ["agent-overview"],
    queryFn: fetchAgentOverview,
    enabled,
    refetchInterval: enabled ? 5_000 : false,
  });
}

export function useAgentHostMetrics(agentId: string) {
  const [, setAuthVersion] = useState(0);
  useEffect(() => {
    const onAuthChanged = () => setAuthVersion((version) => version + 1);
    window.addEventListener(ADMIN_AUTH_CHANGED_EVENT, onAuthChanged);
    return () =>
      window.removeEventListener(ADMIN_AUTH_CHANGED_EVENT, onAuthChanged);
  }, []);
  const enabled = Boolean(getAdminApiToken()) && Boolean(agentId);
  return useQuery({
    queryKey: ["agent-host-metrics", agentId],
    queryFn: () => fetchAgentHostMetrics(agentId),
    enabled,
    refetchInterval: enabled ? 5_000 : false,
  });
}

export function useAllAgentsHostMetrics() {
  const [, setAuthVersion] = useState(0);
  useEffect(() => {
    const onAuthChanged = () => setAuthVersion((version) => version + 1);
    window.addEventListener(ADMIN_AUTH_CHANGED_EVENT, onAuthChanged);
    return () =>
      window.removeEventListener(ADMIN_AUTH_CHANGED_EVENT, onAuthChanged);
  }, []);
  const enabled = Boolean(getAdminApiToken());
  return useQuery({
    queryKey: ["all-agents-host-metrics"],
    queryFn: fetchAllAgentsHostMetrics,
    enabled,
    refetchInterval: enabled ? 5_000 : false,
  });
}

export function useAgentInstallCode() {
  return useQuery({
    queryKey: ["agent-install-code"],
    queryFn: fetchAgentInstallCode,
  });
}

/** Gateway 初始化页面的显式提交动作；先检查状态，不自动轮询或重复消费一次性凭证。 */
export function useGatewayInitialConfig() {
  return useMutation({
    mutationFn: ({ initUrl, token }: { initUrl: string; token?: string }) =>
      initializeGatewayViaUrl(initUrl, token),
  });
}

export function usePauseAgent() {
  return useMutation({
    mutationFn: (command: PauseAgentCommand) => pauseAgent(command),
  });
}

export function useUpgradeAgent() {
  return useMutation({
    mutationFn: (command: UpgradeAgentCommand) => upgradeAgent(command),
  });
}
