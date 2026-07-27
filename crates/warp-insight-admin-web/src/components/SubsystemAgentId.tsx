import styles from "./SubsystemAgentId.module.css";

interface SubsystemAgentIdProps {
  children?: React.ReactNode;
}

export function SubsystemAgentId({ children }: SubsystemAgentIdProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>Agent 标识</div>
      {children}
    </div>
  );
}
