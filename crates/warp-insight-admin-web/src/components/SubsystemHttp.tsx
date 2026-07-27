import styles from "./SubsystemHttp.module.css";

interface SubsystemHttpProps {
  children?: React.ReactNode;
}

export function SubsystemHttp({ children }: SubsystemHttpProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>HTTP</div>
      {children}
    </div>
  );
}
