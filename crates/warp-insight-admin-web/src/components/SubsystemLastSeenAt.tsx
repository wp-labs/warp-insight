import styles from "./SubsystemLastSeenAt.module.css";

interface SubsystemLastSeenAtProps {
  children?: React.ReactNode;
}

export function SubsystemLastSeenAt({ children }: SubsystemLastSeenAtProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>最后上报时间</div>
      {children}
    </div>
  );
}
