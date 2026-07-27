import styles from "./SubsystemX86LinuxAgentInstallCode.module.css";

interface SubsystemX86LinuxAgentInstallCodeProps {
  children?: React.ReactNode;
}

export function SubsystemX86LinuxAgentInstallCode({ children }: SubsystemX86LinuxAgentInstallCodeProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>X86 Linux 安装代码</div>
      {children}
    </div>
  );
}
