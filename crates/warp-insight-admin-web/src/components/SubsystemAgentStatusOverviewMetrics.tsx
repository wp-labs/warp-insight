import styles from "./SubsystemAgentStatusOverviewMetrics.module.css";
import { SubsystemTotalAgentMetric } from "./SubsystemTotalAgentMetric";
import { SubsystemOnlineAgentMetric } from "./SubsystemOnlineAgentMetric";
import { SubsystemUnhealthyAgentMetric } from "./SubsystemUnhealthyAgentMetric";
import { SubsystemLastSeenLagMetric } from "./SubsystemLastSeenLagMetric";
import type { AgentOverviewMetrics } from "../api";

interface SubsystemAgentStatusOverviewMetricsProps {
  metrics?: AgentOverviewMetrics;
  loading?: boolean;
  children?: React.ReactNode;
}

function formatLag(seconds?: number): string {
  if (seconds === undefined) return "-";
  if (seconds < 60) return `${seconds} 秒`;
  return `${Math.round(seconds / 60)} 分钟`;
}

export function SubsystemAgentStatusOverviewMetrics({ metrics, loading }: SubsystemAgentStatusOverviewMetricsProps) {
  const totalAgents = loading ? "加载中" : String(metrics?.totalAgents ?? "-");
  const onlineAgents = loading ? "加载中" : String(metrics?.onlineAgents ?? "-");
  const unhealthyAgents = loading ? "加载中" : String(metrics?.unhealthyAgents ?? "-");
  const lastSeenLag = loading ? "加载中" : formatLag(metrics?.lastSeenLagSeconds);

  return (
    <div className={styles.container}>
      <SubsystemTotalAgentMetric>
        <strong className={styles.value}>{totalAgents}</strong>
      </SubsystemTotalAgentMetric>
      <SubsystemOnlineAgentMetric>
        <strong className={styles.value}>{onlineAgents}</strong>
      </SubsystemOnlineAgentMetric>
      <SubsystemUnhealthyAgentMetric>
        <strong className={styles.value}>{unhealthyAgents}</strong>
      </SubsystemUnhealthyAgentMetric>
      <SubsystemLastSeenLagMetric>
        <strong className={styles.value}>{lastSeenLag}</strong>
      </SubsystemLastSeenLagMetric>
    </div>
  );
}
