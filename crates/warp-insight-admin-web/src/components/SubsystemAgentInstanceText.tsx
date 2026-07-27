import styles from "./SubsystemAgentInstanceText.module.css";

interface SubsystemAgentInstanceTextProps {
  children?: React.ReactNode;
}

export function SubsystemAgentInstanceText({ children }: SubsystemAgentInstanceTextProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>Agent 实例</div>
      {children}
    </div>
  );
}
