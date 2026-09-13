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

export function SubsystemAgentStatusOverviewMetrics({
  metrics,
  loading,
}: SubsystemAgentStatusOverviewMetricsProps) {
  const total = metrics?.totalAgents ?? 0;

  return (
    <div className={styles.container}>
      <SubsystemTotalAgentMetric value={total} loading={loading} />
      <SubsystemOnlineAgentMetric
        value={metrics?.onlineAgents ?? 0}
        total={total}
        loading={loading}
      />
      <SubsystemUnhealthyAgentMetric
        value={metrics?.unhealthyAgents ?? 0}
        loading={loading}
      />
      <SubsystemLastSeenLagMetric
        seconds={metrics?.lastSeenLagSeconds}
        loading={loading}
      />
    </div>
  );
}
