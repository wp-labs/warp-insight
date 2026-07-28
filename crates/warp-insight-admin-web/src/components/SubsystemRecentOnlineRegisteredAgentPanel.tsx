import styles from "./SubsystemRecentOnlineRegisteredAgentPanel.module.css";
import { SubsystemRecentOnlineRegisteredAgentCardGrid } from "./SubsystemRecentOnlineRegisteredAgentCardGrid";
import type { RecentOnlineRegisteredAgent } from "../api";

interface SubsystemRecentOnlineRegisteredAgentPanelProps {
  agents?: RecentOnlineRegisteredAgent[];
  loading?: boolean;
  children?: React.ReactNode;
}

export function SubsystemRecentOnlineRegisteredAgentPanel({
  agents = [],
  loading,
}: SubsystemRecentOnlineRegisteredAgentPanelProps) {
  return (
    <section className={styles.container}>
      <div className={styles.header}>
        <div>
          <h2 className={styles.title}>最近上线注册 Agent</h2>
          <p className={styles.summary}>查看近期完成注册并保持在线的 Agent，优先确认新接入主机状态。</p>
        </div>
        <span className={styles.count}>{loading ? "加载中" : `${agents.length} 个`}</span>
      </div>
      <SubsystemRecentOnlineRegisteredAgentCardGrid items={agents} loading={loading} />
    </section>
  );
}
