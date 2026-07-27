import styles from "./SubsystemDispatchId.module.css";

interface SubsystemDispatchIdProps {
  children?: React.ReactNode;
}

export function SubsystemDispatchId({ children }: SubsystemDispatchIdProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>派发 ID</div>
      {children}
    </div>
  );
}
