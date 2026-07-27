import styles from "./SubsystemCreatedAt.module.css";

interface SubsystemCreatedAtProps {
  children?: React.ReactNode;
}

export function SubsystemCreatedAt({ children }: SubsystemCreatedAtProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>创建时间</div>
      {children}
    </div>
  );
}
