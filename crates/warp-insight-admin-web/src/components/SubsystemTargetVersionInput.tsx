import styles from "./SubsystemTargetVersionInput.module.css";

interface SubsystemTargetVersionInputProps {
  children?: React.ReactNode;
}

export function SubsystemTargetVersionInput({ children }: SubsystemTargetVersionInputProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>目标版本输入</div>
      {children}
    </div>
  );
}
