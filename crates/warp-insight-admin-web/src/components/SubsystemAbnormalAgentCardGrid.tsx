import styles from "./SubsystemAbnormalAgentCardGrid.module.css";
import { SubsystemAbnormalAgentCard } from "./SubsystemAbnormalAgentCard";
import type { AgentRuntimeStatusView } from "../api";

interface SubsystemAbnormalAgentCardGridProps {
  items?: AgentRuntimeStatusView[];
  children?: React.ReactNode;
}

export function SubsystemAbnormalAgentCardGrid({ items }: SubsystemAbnormalAgentCardGridProps) {
  return (
    <div className={styles.container}>
      {(items ?? []).map((agent) => (
        <SubsystemAbnormalAgentCard key={agent.agentId} agent={agent} />
      ))}
    </div>
  );
}
