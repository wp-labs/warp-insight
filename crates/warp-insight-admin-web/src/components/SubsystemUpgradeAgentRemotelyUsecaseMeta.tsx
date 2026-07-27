import styles from "./SubsystemUpgradeAgentRemotelyUsecaseMeta.module.css";

interface SubsystemUpgradeAgentRemotelyUsecaseMetaProps {
  children?: React.ReactNode;
}

export function SubsystemUpgradeAgentRemotelyUsecaseMeta({ children }: SubsystemUpgradeAgentRemotelyUsecaseMetaProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>远程升级 Agent 用例信息</div>
      {children}
    </div>
  );
}
