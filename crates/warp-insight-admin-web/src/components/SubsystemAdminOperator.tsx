import styles from "./SubsystemAdminOperator.module.css";

interface SubsystemAdminOperatorProps {
  children?: React.ReactNode;
}

export function SubsystemAdminOperator({ children }: SubsystemAdminOperatorProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>管理员</div>
      {children}
    </div>
  );
}
