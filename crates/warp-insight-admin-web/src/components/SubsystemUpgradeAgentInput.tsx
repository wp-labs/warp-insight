import styles from "./SubsystemUpgradeAgentInput.module.css";

interface SubsystemUpgradeAgentInputProps {
  children?: React.ReactNode;
}

export function SubsystemUpgradeAgentInput({ children }: SubsystemUpgradeAgentInputProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>升级 Agent 输入</div>
      {children}
    </div>
  );
}
