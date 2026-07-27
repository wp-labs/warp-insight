import styles from "./SubsystemAdminOperatorAccess.module.css";

interface SubsystemAdminOperatorAccessProps {
  children?: React.ReactNode;
}

export function SubsystemAdminOperatorAccess({ children }: SubsystemAdminOperatorAccessProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>操作权限</div>
      {children}
    </div>
  );
}
