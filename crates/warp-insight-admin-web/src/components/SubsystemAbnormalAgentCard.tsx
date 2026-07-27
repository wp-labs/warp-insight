import styles from "./SubsystemAbnormalAgentCard.module.css";
import type { AgentRuntimeStatusView } from "../api";

interface SubsystemAbnormalAgentCardProps {
  agent: AgentRuntimeStatusView;
  children?: React.ReactNode;
}

function formatDateTime(value: string): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function statusText(status: AgentRuntimeStatusView["status"]): string {
  const labels: Record<AgentRuntimeStatusView["status"], string> = {
    online: "在线",
    offline: "离线",
    paused: "已暂停",
  };
  return labels[status] ?? status;
}

function healthText(health: AgentRuntimeStatusView["health"]): string {
  const labels: Record<AgentRuntimeStatusView["health"], string> = {
    healthy: "健康",
    degraded: "降级",
    unhealthy: "不健康",
  };
  return labels[health] ?? health;
}

export function SubsystemAbnormalAgentCard({ agent }: SubsystemAbnormalAgentCardProps) {
  const badgeClassName =
    agent.health === "unhealthy"
      ? `${styles.badge} ${styles.badgeDanger}`
      : `${styles.badge} ${styles.badgeWarn}`;

  return (
    <div className={styles.container}>
      <div className={styles.top}>
        <div>
          <div className={styles.agentId}>{agent.agentId}</div>
          <div className={styles.instance}>{agent.instanceId}</div>
        </div>
        <span className={badgeClassName}>{healthText(agent.health)}</span>
      </div>
      <div className={styles.details}>
        <div className={styles.item}>
          <div className={styles.label}>状态</div>
          <div className={styles.value}>{statusText(agent.status)}</div>
        </div>
        <div className={styles.item}>
          <div className={styles.label}>版本</div>
          <div className={styles.value}>{agent.version}</div>
        </div>
        <div className={styles.item}>
          <div className={styles.label}>最后上报</div>
          <div className={styles.value}>{formatDateTime(agent.lastSeenAt)}</div>
        </div>
      </div>
    </div>
  );
}
