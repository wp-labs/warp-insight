import styles from "./SubsystemAgentOnlineStatusFilter.module.css";

interface SubsystemAgentOnlineStatusFilterProps {
  children?: React.ReactNode;
}

export function SubsystemAgentOnlineStatusFilter({ children }: SubsystemAgentOnlineStatusFilterProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>在线状态筛选</div>
      {children}
    </div>
  );
}
