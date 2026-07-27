import styles from "./SubsystemInstanceId.module.css";

interface SubsystemInstanceIdProps {
  children?: React.ReactNode;
}

export function SubsystemInstanceId({ children }: SubsystemInstanceIdProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>实例 ID</div>
      {children}
    </div>
  );
}
