import styles from "./SubsystemAdminOperatorIdentity.module.css";

interface SubsystemAdminOperatorIdentityProps {
  children?: React.ReactNode;
}

export function SubsystemAdminOperatorIdentity({ children }: SubsystemAdminOperatorIdentityProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>管理员身份</div>
      {children}
    </div>
  );
}
