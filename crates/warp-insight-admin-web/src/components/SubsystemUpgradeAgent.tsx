import styles from "./SubsystemUpgradeAgent.module.css";

interface SubsystemUpgradeAgentProps {
  children?: React.ReactNode;
}

export function SubsystemUpgradeAgent({ children }: SubsystemUpgradeAgentProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>升级 Agent</div>
      {children}
    </div>
  );
}
