import styles from "./SubsystemPauseAgent.module.css";

interface SubsystemPauseAgentProps {
  children?: React.ReactNode;
}

/** 用例动作标识：以行内标签呈现，供用例头部聚合成一行说明。 */
export function SubsystemPauseAgent({ children }: SubsystemPauseAgentProps) {
  return (
    <span className={styles.chip}>
      <span className={styles.key}>动作</span>
      暂停 Agent
      {children}
    </span>
  );
}
