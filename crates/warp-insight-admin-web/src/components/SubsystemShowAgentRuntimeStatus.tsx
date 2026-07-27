import styles from "./SubsystemShowAgentRuntimeStatus.module.css";

interface SubsystemShowAgentRuntimeStatusProps {
  children?: React.ReactNode;
}

export function SubsystemShowAgentRuntimeStatus({ children }: SubsystemShowAgentRuntimeStatusProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>查看 Agent 运行状态</div>
      {children}
    </div>
  );
}
