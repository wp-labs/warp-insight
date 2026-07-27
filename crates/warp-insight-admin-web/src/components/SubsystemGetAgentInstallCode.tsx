import styles from "./SubsystemGetAgentInstallCode.module.css";

interface SubsystemGetAgentInstallCodeProps {
  children?: React.ReactNode;
}

export function SubsystemGetAgentInstallCode({ children }: SubsystemGetAgentInstallCodeProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>获取安装代码</div>
      {children}
    </div>
  );
}
