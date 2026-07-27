import styles from "./SubsystemAgentStatusSearchInput.module.css";

interface SubsystemAgentStatusSearchInputProps {
  children?: React.ReactNode;
}

export function SubsystemAgentStatusSearchInput({ children }: SubsystemAgentStatusSearchInputProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>Agent 状态搜索</div>
      {children}
    </div>
  );
}
