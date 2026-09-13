import styles from "./SubsystemRecentOnlineRegisteredAgentCard.module.css";
import { Sparkline } from "./Sparkline";
import { Link } from "react-router-dom";
import type { RecentOnlineRegisteredAgent } from "../api";

interface SubsystemRecentOnlineRegisteredAgentCardProps {
  agent: RecentOnlineRegisteredAgent;
  children?: React.ReactNode;
}

/** 与 crates/warp-gateway/src/api/overview.rs 的 ONLINE_WINDOW_SECONDS 保持一致。 */
const ONLINE_WINDOW_SECONDS = 300;

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

function formatMemory(bytes?: number): string {
  if (bytes === undefined || bytes === null) return "—";
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatCpu(percent?: number): string {
  return percent === undefined || percent === null ? "—" : `${percent.toFixed(1)}%`;
}

function formatLatency(ms?: number): string {
  return ms === undefined || ms === null ? "—" : `${ms} ms`;
}

export function SubsystemRecentOnlineRegisteredAgentCard({
  agent,
}: SubsystemRecentOnlineRegisteredAgentCardProps) {
  const sourceLabel = agent.source === "real" ? "真实" : "示例";
  // 与网关侧判定保持一致：online_since 距今 ≤ 5 分钟才算在线
  // （见 crates/warp-gateway/src/api/overview.rs 的 ONLINE_WINDOW_SECONDS）。
  const secondsSinceOnline = Math.max(
    0,
    (Date.now() - new Date(agent.onlineSince).getTime()) / 1000,
  );
  const isOnline = secondsSinceOnline <= ONLINE_WINDOW_SECONDS;
  return (
    <article className={styles.container}>
      <div className={styles.top}>
        <div className={styles.identity}>
          <div className={styles.agentId}>{agent.agentId}</div>
          <div className={styles.instance}>{agent.instanceId}</div>
        </div>
        <div className={styles.badges}>
          <span className={isOnline ? styles.badge : styles.offlineBadge}>
            {isOnline
              ? "在线"
              : `离线 ${formatDuration(Math.round(secondsSinceOnline))}`}
          </span>
          <span className={agent.source === "real" ? styles.realBadge : styles.exampleBadge}>
            {sourceLabel}
          </span>
        </div>
      </div>
      <div className={styles.durationBlock}>
        <span className={styles.durationLabel}>上线时长</span>
        <strong className={styles.duration}>{formatDuration(agent.onlineDurationSeconds)}</strong>
      </div>
      <div className={styles.metrics}>
        <div className={styles.metric}>
          <span className={styles.metricLabel}>内存</span>
          <span className={styles.metricValue}>{formatMemory(agent.memoryBytes)}</span>
        </div>
        <div className={styles.metric}>
          <span className={styles.metricLabel}>CPU</span>
          <span className={styles.metricValue}>{formatCpu(agent.cpuPercent)}</span>
        </div>
        <div className={styles.metric}>
          <span className={styles.metricLabel}>Admin 延时</span>
          <span className={styles.metricValue}>{formatLatency(agent.adminLatencyMs)}</span>
        </div>
      </div>
      <div className={styles.trend}>
        <div className={styles.trendItem}>
          <span className={styles.trendLabel}>内存趋势</span>
          <Sparkline
            values={(agent.metricsHistory ?? []).map((s) => s.memoryBytes)}
            color="var(--series-1)"
          />
        </div>
        <div className={styles.trendItem}>
          <span className={styles.trendLabel}>CPU 趋势</span>
          <Sparkline
            values={(agent.metricsHistory ?? []).map((s) => s.cpuPercent)}
            color="var(--series-5)"
          />
        </div>
        <div className={styles.trendItem}>
          <span className={styles.trendLabel}>延时趋势</span>
          <Sparkline
            values={(agent.metricsHistory ?? []).map((s) => s.adminLatencyMs)}
            color="var(--series-3)"
          />
        </div>
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
      <div className={styles.footer}>
        <Link
          className={styles.metricsLink}
          to={`/agents/${encodeURIComponent(agent.agentId)}/metrics`}
        >
          查看主机指标 →
        </Link>
      </div>
    </article>
  );
}
