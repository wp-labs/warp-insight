import styles from "./SubsystemAgentLastSeenAtText.module.css";

interface SubsystemAgentLastSeenAtTextProps {
  children?: React.ReactNode;
}

export function SubsystemAgentLastSeenAtText({ children }: SubsystemAgentLastSeenAtTextProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>最后上报时间</div>
      {children}
    </div>
  );
}
