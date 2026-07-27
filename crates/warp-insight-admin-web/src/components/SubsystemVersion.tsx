import styles from "./SubsystemVersion.module.css";

interface SubsystemVersionProps {
  children?: React.ReactNode;
}

export function SubsystemVersion({ children }: SubsystemVersionProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>版本</div>
      {children}
    </div>
  );
}
