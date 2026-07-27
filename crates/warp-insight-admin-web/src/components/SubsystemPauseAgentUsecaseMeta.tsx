import styles from "./SubsystemPauseAgentUsecaseMeta.module.css";

interface SubsystemPauseAgentUsecaseMetaProps {
  children?: React.ReactNode;
}

export function SubsystemPauseAgentUsecaseMeta({ children }: SubsystemPauseAgentUsecaseMetaProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>暂停 Agent 用例信息</div>
      {children}
    </div>
  );
}
