import styles from "./SubsystemUpgradeAgentRemotely.module.css";

interface SubsystemUpgradeAgentRemotelyProps {
  children?: React.ReactNode;
}

export function SubsystemUpgradeAgentRemotely({ children }: SubsystemUpgradeAgentRemotelyProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>远程升级 Agent</div>
      {children}
    </div>
  );
}
