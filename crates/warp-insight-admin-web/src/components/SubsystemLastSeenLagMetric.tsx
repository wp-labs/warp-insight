import styles from "./SubsystemLastSeenLagMetric.module.css";

interface SubsystemLastSeenLagMetricProps {
  children?: React.ReactNode;
}

export function SubsystemLastSeenLagMetric({ children }: SubsystemLastSeenLagMetricProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>上报延迟</div>
      {children}
    </div>
  );
}
