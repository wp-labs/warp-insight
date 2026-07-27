import styles from "./SubsystemAgentOnlineStatusBadge.module.css";

interface SubsystemAgentOnlineStatusBadgeProps {
  children?: React.ReactNode;
}

export function SubsystemAgentOnlineStatusBadge({ children }: SubsystemAgentOnlineStatusBadgeProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>Agent 在线标识</div>
      {children}
    </div>
  );
}
