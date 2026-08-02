import styles from "./SubsystemArmLinuxInstallCode.module.css";

interface SubsystemArmLinuxInstallCodeProps {
  command?: string;
  token?: string;
  loading?: boolean;
  children?: React.ReactNode;
}

export function SubsystemArmLinuxInstallCode({
  command,
  token,
  loading,
  children,
}: SubsystemArmLinuxInstallCodeProps) {
  const displayCommand = loading
    ? "安装代码加载中..."
    : (command ?? "暂无安装代码");
  const fullCommand =
    command && token
      ? `export WARP_INSIGHT_ENROLLMENT_TOKEN=${token}\n${command}`
      : command;

  return (
    <div className={styles.container}>
      <div className={styles.label}>Arm Linux 安装命令</div>
      <code className={styles.code}>{displayCommand}</code>
      <div className={styles.actions}>
        <button
          className={styles.copyButton}
          type="button"
          disabled={!command}
          onClick={() => command && navigator.clipboard?.writeText(command)}
        >
          复制命令
        </button>
        <button
          className={styles.copyButton}
          type="button"
          disabled={!fullCommand}
          onClick={() =>
            fullCommand && navigator.clipboard?.writeText(fullCommand)
          }
        >
          复制完整命令（含 Token）
        </button>
      </div>
      {children}
    </div>
  );
}
