import styles from "./SubsystemX86LinuxInstallCode.module.css";

interface SubsystemX86LinuxInstallCodeProps {
  command?: string;
  loading?: boolean;
  label?: string;
  children?: React.ReactNode;
}

export function SubsystemX86LinuxInstallCode({
  command,
  loading,
  label = "X86 Linux 安装代码",
  children,
}: SubsystemX86LinuxInstallCodeProps) {
  const displayCommand = loading
    ? "安装代码加载中..."
    : (command ?? "暂无安装代码");

  return (
    <div className={styles.container}>
      <div className={styles.label}>{label}</div>
      <code className={styles.code}>{displayCommand}</code>
      <button
        className={styles.copyButton}
        type="button"
        disabled={!command}
        onClick={() => command && navigator.clipboard?.writeText(command)}
      >
        复制
      </button>
      {children}
    </div>
  );
}
