import styles from "./SubsystemAgentHealthStatusFilter.module.css";

interface SubsystemAgentHealthStatusFilterProps {
  children?: React.ReactNode;
}

export function SubsystemAgentHealthStatusFilter({ children }: SubsystemAgentHealthStatusFilterProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>健康状态筛选</div>
      {children}
    </div>
  );
}
