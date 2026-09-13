import styles from "./SubsystemAdminOperatorAccess.module.css";

interface SubsystemAdminOperatorAccessProps {
  children?: React.ReactNode;
}

export function SubsystemAdminOperatorAccess({
  children,
}: SubsystemAdminOperatorAccessProps) {
  return (
    <span className={styles.chip}>
      <span className={styles.key}>通道</span>
      {children}
    </span>
  );
}
