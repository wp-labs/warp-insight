import styles from "./SubsystemSubmitUpgradeAction.module.css";

interface SubsystemSubmitUpgradeActionProps {
  children?: React.ReactNode;
}

export function SubsystemSubmitUpgradeAction({ children }: SubsystemSubmitUpgradeActionProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>提交升级</div>
      {children}
    </div>
  );
}
