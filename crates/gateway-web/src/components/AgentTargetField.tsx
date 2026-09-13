import { useMemo } from "react";
import styles from "./AgentTargetField.module.css";
import { useAgentOverview } from "../hooks";

interface AgentTargetFieldProps {
  value: string;
  onChange: (value: string) => void;
  label?: string;
  /** 允许自由输入：目标主机可能还没出现在最近上线列表里。 */
  allowFreeInput?: boolean;
}

/**
 * 目标 Agent 输入：既可以从已注册 Agent 里选，也可以手工输入。
 * 手工输入是必需的——运维常常要对一台刚接入、还没上报状态的主机下发命令。
 */
export function AgentTargetField({
  value,
  onChange,
  label = "目标 Agent",
  allowFreeInput = true,
}: AgentTargetFieldProps) {
  const { data } = useAgentOverview();
  const agents = data?.recentOnlineAgents ?? [];
  const listId = useMemo(
    () => `agent-options-${Math.random().toString(36).slice(2, 8)}`,
    [],
  );

  return (
    <label className={styles.field}>
      <span className={styles.label}>{label}</span>
      <input
        className={styles.input}
        list={listId}
        placeholder="agent-mbp-p0-collector"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        required
      />
      <datalist id={listId}>
        {agents.map((agent) => (
          <option key={agent.agentId} value={agent.agentId}>
            {agent.version} · {agent.instanceId}
          </option>
        ))}
      </datalist>
      <span className={styles.hint}>
        {agents.length > 0
          ? `可选 ${agents.length} 台已注册主机，输入时会自动补全；也可直接填写未上报的主机 ID。`
          : "暂未读取到已注册主机，请直接填写主机 ID。"}
      </span>
    </label>
  );
}
