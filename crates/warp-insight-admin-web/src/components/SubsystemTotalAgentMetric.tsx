import styles from "./SubsystemTotalAgentMetric.module.css";

interface SubsystemTotalAgentMetricProps {
  children?: React.ReactNode;
}

export function SubsystemTotalAgentMetric({ children }: SubsystemTotalAgentMetricProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>Agent 总数</div>
      {children}
    </div>
  );
}
