import styles from "./SubsystemRefreshRuntimeStatusAction.module.css";

interface SubsystemRefreshRuntimeStatusActionProps {
  children?: React.ReactNode;
}

export function SubsystemRefreshRuntimeStatusAction({ children }: SubsystemRefreshRuntimeStatusActionProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>刷新运行状态</div>
      {children}
    </div>
  );
}
