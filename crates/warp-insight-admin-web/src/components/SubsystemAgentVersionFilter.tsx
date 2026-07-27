import styles from "./SubsystemAgentVersionFilter.module.css";

interface SubsystemAgentVersionFilterProps {
  children?: React.ReactNode;
}

export function SubsystemAgentVersionFilter({ children }: SubsystemAgentVersionFilterProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>Agent 版本筛选</div>
      {children}
    </div>
  );
}
