import styles from "./SubsystemShowAgentRuntimeStatusUsecaseMeta.module.css";

interface SubsystemShowAgentRuntimeStatusUsecaseMetaProps {
  children?: React.ReactNode;
}

export function SubsystemShowAgentRuntimeStatusUsecaseMeta({ children }: SubsystemShowAgentRuntimeStatusUsecaseMetaProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>查看运行状态用例信息</div>
      {children}
    </div>
  );
}
