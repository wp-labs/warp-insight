import styles from "./SubsystemNoAbnormalAgentPlaceholder.module.css";

interface SubsystemNoAbnormalAgentPlaceholderProps {
  children?: React.ReactNode;
}

export function SubsystemNoAbnormalAgentPlaceholder({ children }: SubsystemNoAbnormalAgentPlaceholderProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>当前没有异常 Agent</div>
      {children}
    </div>
  );
}
