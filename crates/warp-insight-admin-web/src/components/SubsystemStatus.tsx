import styles from "./SubsystemStatus.module.css";

interface SubsystemStatusProps {
  children?: React.ReactNode;
}

export function SubsystemStatus({ children }: SubsystemStatusProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>状态</div>
      {children}
    </div>
  );
}
