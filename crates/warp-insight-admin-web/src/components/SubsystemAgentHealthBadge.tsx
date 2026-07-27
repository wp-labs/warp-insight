import styles from "./SubsystemAgentHealthBadge.module.css";

interface SubsystemAgentHealthBadgeProps {
  children?: React.ReactNode;
}

export function SubsystemAgentHealthBadge({ children }: SubsystemAgentHealthBadgeProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>Agent 健康标识</div>
      {children}
    </div>
  );
}
