import styles from "./SubsystemUnhealthyAgentMetric.module.css";

interface SubsystemUnhealthyAgentMetricProps {
  children?: React.ReactNode;
}

export function SubsystemUnhealthyAgentMetric({ children }: SubsystemUnhealthyAgentMetricProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>异常 Agent 数</div>
      {children}
    </div>
  );
}
