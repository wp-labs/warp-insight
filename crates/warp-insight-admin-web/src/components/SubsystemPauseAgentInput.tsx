import styles from "./SubsystemPauseAgentInput.module.css";

interface SubsystemPauseAgentInputProps {
  children?: React.ReactNode;
}

export function SubsystemPauseAgentInput({ children }: SubsystemPauseAgentInputProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>暂停 Agent 输入</div>
      {children}
    </div>
  );
}
