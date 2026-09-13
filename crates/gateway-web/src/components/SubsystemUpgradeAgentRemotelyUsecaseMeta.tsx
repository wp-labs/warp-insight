import styles from "./SubsystemUpgradeAgentRemotelyUsecaseMeta.module.css";

interface SubsystemUpgradeAgentRemotelyUsecaseMetaProps {
  children?: React.ReactNode;
}

export function SubsystemUpgradeAgentRemotelyUsecaseMeta({
  children,
}: SubsystemUpgradeAgentRemotelyUsecaseMetaProps) {
  return (
    <div className={styles.container}>
      <div className={styles.head}>
        <h3 className={styles.title}>远程升级</h3>
        <p className={styles.desc}>
          向单台 Agent 派发升级命令，指定目标版本，进程重启后按新版本继续采集。
        </p>
      </div>
      <div className={styles.chips}>{children}</div>
    </div>
  );
}
