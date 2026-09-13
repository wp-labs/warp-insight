import styles from "./SubsystemUpgradeAgentRemotely.module.css";

interface SubsystemUpgradeAgentRemotelyProps {
  children?: React.ReactNode;
}

/** 用例动作标识：以行内标签呈现，供用例头部聚合成一行说明。 */
export function SubsystemUpgradeAgentRemotely({
  children,
}: SubsystemUpgradeAgentRemotelyProps) {
  return (
    <span className={styles.chip}>
      <span className={styles.key}>动作</span>
      远程升级 Agent
      {children}
    </span>
  );
}
