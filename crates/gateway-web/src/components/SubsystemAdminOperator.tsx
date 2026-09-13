import styles from "./SubsystemAdminOperator.module.css";

interface SubsystemAdminOperatorProps {
  children?: React.ReactNode;
}

export function SubsystemAdminOperator({ children }: SubsystemAdminOperatorProps) {
  return (
    <span className={styles.value}>
      admin-operator
      {children}
    </span>
  );
}
