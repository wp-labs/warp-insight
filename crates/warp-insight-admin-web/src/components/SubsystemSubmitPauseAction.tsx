import styles from "./SubsystemSubmitPauseAction.module.css";

interface SubsystemSubmitPauseActionProps {
  children?: React.ReactNode;
}

export function SubsystemSubmitPauseAction({ children }: SubsystemSubmitPauseActionProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>提交暂停</div>
      {children}
    </div>
  );
}
