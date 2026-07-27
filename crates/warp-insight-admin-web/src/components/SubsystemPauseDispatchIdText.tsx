import styles from "./SubsystemPauseDispatchIdText.module.css";

interface SubsystemPauseDispatchIdTextProps {
  children?: React.ReactNode;
}

export function SubsystemPauseDispatchIdText({ children }: SubsystemPauseDispatchIdTextProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>暂停派发 ID</div>
      {children}
    </div>
  );
}
