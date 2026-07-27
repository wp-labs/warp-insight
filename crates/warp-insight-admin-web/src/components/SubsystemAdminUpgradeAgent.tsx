import styles from "./SubsystemAdminUpgradeAgent.module.css";

interface SubsystemAdminUpgradeAgentProps {
  children?: React.ReactNode;
}

export function SubsystemAdminUpgradeAgent({ children }: SubsystemAdminUpgradeAgentProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>管理员升级 Agent</div>
      {children}
    </div>
  );
}
