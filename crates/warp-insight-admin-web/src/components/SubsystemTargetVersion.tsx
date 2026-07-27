import styles from "./SubsystemTargetVersion.module.css";

interface SubsystemTargetVersionProps {
  children?: React.ReactNode;
}

export function SubsystemTargetVersion({ children }: SubsystemTargetVersionProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>目标版本</div>
      {children}
    </div>
  );
}
