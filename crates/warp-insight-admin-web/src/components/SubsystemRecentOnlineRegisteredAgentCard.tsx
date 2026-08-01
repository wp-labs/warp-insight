import styles from "./SubsystemRecentOnlineRegisteredAgentCard.module.css";
import type { RecentOnlineRegisteredAgent } from "../api";

interface SubsystemRecentOnlineRegisteredAgentCardProps {
  agent: RecentOnlineRegisteredAgent;
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

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds} 秒`;
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0 && minutes > 0) return `${hours} 小时 ${minutes} 分钟`;
  if (hours > 0) return `${hours} 小时`;
  return `${minutes} 分钟`;
}

export function SubsystemRecentOnlineRegisteredAgentCard({
  agent,
}: SubsystemRecentOnlineRegisteredAgentCardProps) {
  const sourceLabel = agent.source === "real" ? "真实" : "示例";
  return (
    <article className={styles.container}>
      <div className={styles.top}>
        <div className={styles.identity}>
          <div className={styles.agentId}>{agent.agentId}</div>
          <div className={styles.instance}>{agent.instanceId}</div>
        </div>
        <div className={styles.badges}>
          <span className={styles.badge}>在线</span>
          <span className={agent.source === "real" ? styles.realBadge : styles.exampleBadge}>
            {sourceLabel}
          </span>
        </div>
      </div>
      <div className={styles.durationBlock}>
        <span className={styles.durationLabel}>上线时长</span>
        <strong className={styles.duration}>{formatDuration(agent.onlineDurationSeconds)}</strong>
      </div>
      <div className={styles.details}>
        <div className={styles.item}>
          <div className={styles.label}>版本</div>
          <div className={styles.value}>{agent.version}</div>
        </div>
        <div className={styles.item}>
          <div className={styles.label}>注册时间</div>
          <div className={styles.value}>{formatDateTime(agent.registeredAt)}</div>
        </div>
        <div className={styles.item}>
          <div className={styles.label}>上线时间</div>
          <div className={styles.value}>{formatDateTime(agent.onlineSince)}</div>
        </div>
      </div>
    </article>
  );
}
