import styles from "./SubsystemPauseAgent.module.css";

interface SubsystemPauseAgentProps {
  children?: React.ReactNode;
}

export function SubsystemPauseAgent({ children }: SubsystemPauseAgentProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>暂停 Agent</div>
      {children}
    </div>
  );
}
