import styles from "./SubsystemAgentRuntimeStatusView.module.css";

interface SubsystemAgentRuntimeStatusViewProps {
  children?: React.ReactNode;
}

export function SubsystemAgentRuntimeStatusView({ children }: SubsystemAgentRuntimeStatusViewProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>Agent 运行状态</div>
      {children}
    </div>
  );
}
