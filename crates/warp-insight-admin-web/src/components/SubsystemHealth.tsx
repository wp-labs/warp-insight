import styles from "./SubsystemHealth.module.css";

interface SubsystemHealthProps {
  children?: React.ReactNode;
}

export function SubsystemHealth({ children }: SubsystemHealthProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>健康状态</div>
      {children}
    </div>
  );
}
