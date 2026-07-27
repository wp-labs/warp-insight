import styles from "./SubsystemAdminShowAgentRuntimeStatus.module.css";

interface SubsystemAdminShowAgentRuntimeStatusProps {
  children?: React.ReactNode;
}

export function SubsystemAdminShowAgentRuntimeStatus({ children }: SubsystemAdminShowAgentRuntimeStatusProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>管理员查看 Agent 运行状态</div>
      {children}
    </div>
  );
}
