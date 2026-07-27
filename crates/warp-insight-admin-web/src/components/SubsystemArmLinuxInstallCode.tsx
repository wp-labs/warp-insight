import styles from "./SubsystemArmLinuxInstallCode.module.css";

interface SubsystemArmLinuxInstallCodeProps {
  command?: string;
  loading?: boolean;
  children?: React.ReactNode;
}

export function SubsystemArmLinuxInstallCode({ command, loading, children }: SubsystemArmLinuxInstallCodeProps) {
  const displayCommand = loading ? "安装代码加载中..." : command ?? "暂无安装代码";

  return (
    <div className={styles.container}>
      <div className={styles.label}>Arm Linux 安装代码</div>
      <code className={styles.code}>{displayCommand}</code>
      <button className={styles.copyButton} type="button" disabled={!command} onClick={() => command && navigator.clipboard?.writeText(command)}>
        复制
      </button>
      {children}
    </div>
  );
}
