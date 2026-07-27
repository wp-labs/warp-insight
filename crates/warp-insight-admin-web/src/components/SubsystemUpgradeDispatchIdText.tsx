import styles from "./SubsystemUpgradeDispatchIdText.module.css";

interface SubsystemUpgradeDispatchIdTextProps {
  children?: React.ReactNode;
}

export function SubsystemUpgradeDispatchIdText({ children }: SubsystemUpgradeDispatchIdTextProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>升级派发 ID</div>
      {children}
    </div>
  );
}
