import styles from "./SubsystemArmLinuxAgentInstallCode.module.css";

interface SubsystemArmLinuxAgentInstallCodeProps {
  children?: React.ReactNode;
}

export function SubsystemArmLinuxAgentInstallCode({ children }: SubsystemArmLinuxAgentInstallCodeProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>Arm Linux 安装代码</div>
      {children}
    </div>
  );
}
