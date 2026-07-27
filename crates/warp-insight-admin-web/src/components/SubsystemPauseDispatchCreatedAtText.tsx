import styles from "./SubsystemPauseDispatchCreatedAtText.module.css";

interface SubsystemPauseDispatchCreatedAtTextProps {
  children?: React.ReactNode;
}

export function SubsystemPauseDispatchCreatedAtText({ children }: SubsystemPauseDispatchCreatedAtTextProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>暂停派发创建时间</div>
      {children}
    </div>
  );
}
