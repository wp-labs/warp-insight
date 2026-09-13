import styles from "./SubsystemAdminOperatorIdentity.module.css";

interface SubsystemAdminOperatorIdentityProps {
  children?: React.ReactNode;
}

export function SubsystemAdminOperatorIdentity({
  children,
}: SubsystemAdminOperatorIdentityProps) {
  return (
    <span className={styles.chip}>
      <span className={styles.key}>操作者</span>
      {children}
    </span>
  );
}
