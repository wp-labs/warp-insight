import styles from "./SubsystemAdminPauseAgent.module.css";

interface SubsystemAdminPauseAgentProps {
  children?: React.ReactNode;
}

export function SubsystemAdminPauseAgent({ children }: SubsystemAdminPauseAgentProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>管理员暂停 Agent</div>
      {children}
    </div>
  );
}
