import styles from "./SubsystemOnlineAgentMetric.module.css";

interface SubsystemOnlineAgentMetricProps {
  children?: React.ReactNode;
}

export function SubsystemOnlineAgentMetric({ children }: SubsystemOnlineAgentMetricProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>在线 Agent 数</div>
      {children}
    </div>
  );
}
