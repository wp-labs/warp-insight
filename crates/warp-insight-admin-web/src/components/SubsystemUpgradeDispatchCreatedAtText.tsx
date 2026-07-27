import styles from "./SubsystemUpgradeDispatchCreatedAtText.module.css";

interface SubsystemUpgradeDispatchCreatedAtTextProps {
  children?: React.ReactNode;
}

export function SubsystemUpgradeDispatchCreatedAtText({ children }: SubsystemUpgradeDispatchCreatedAtTextProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>升级派发创建时间</div>
      {children}
    </div>
  );
}
