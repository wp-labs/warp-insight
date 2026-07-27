import styles from "./SubsystemAgentVersionText.module.css";

interface SubsystemAgentVersionTextProps {
  children?: React.ReactNode;
}

export function SubsystemAgentVersionText({ children }: SubsystemAgentVersionTextProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>Agent 版本</div>
      {children}
    </div>
  );
}
