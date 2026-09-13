import styles from "./SubsystemHttp.module.css";

interface SubsystemHttpProps {
  children?: React.ReactNode;
}

export function SubsystemHttp({ children }: SubsystemHttpProps) {
  return (
    <span className={styles.value}>
      HTTPS Admin API
      {children}
    </span>
  );
}
