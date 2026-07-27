import { useMutation, useQuery } from "@tanstack/react-query";
import {
  fetchAgentInstallCode,
  fetchAgentOverview,
  pauseAgent,
  upgradeAgent,
  type PauseAgentCommand,
  type UpgradeAgentCommand,
} from "../api";

export function useAgentOverview() {
  return useQuery({
    queryKey: ["agent-overview"],
    queryFn: fetchAgentOverview,
  });
}

export function useAgentInstallCode() {
  return useQuery({
    queryKey: ["agent-install-code"],
    queryFn: fetchAgentInstallCode,
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
