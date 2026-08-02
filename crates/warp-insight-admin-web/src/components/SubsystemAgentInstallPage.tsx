import styles from "./SubsystemAgentInstallPage.module.css";
import { SubsystemAdminTopNavigation } from "./SubsystemAdminTopNavigation";
import { SubsystemBootstrapTokenCard } from "./SubsystemBootstrapTokenCard";
import { SubsystemX86LinuxInstallCode } from "./SubsystemX86LinuxInstallCode";
import { SubsystemArmLinuxInstallCode } from "./SubsystemArmLinuxInstallCode";
import { ApiError } from "../api";
import { useAgentInstallCode } from "../hooks";

export function SubsystemAgentInstallPage() {
  const { data, isLoading, isError, error } = useAgentInstallCode();

  const authError = isError && error instanceof ApiError && error.status === 401;
  const token = data?.bootstrapEnrollmentToken;

  return (
    <div className={styles.container}>
      <SubsystemAdminTopNavigation />
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>Agent 安装</h1>
        <p className={styles.pageSummary}>
          获取 Bootstrap Token，然后在目标 Linux 主机上运行对应架构的安装命令，让 agent 加入集群。
        </p>
      </header>
      {isError ? (
        <div className={styles.errorBanner}>
          {authError ? (
            <>
              Admin Token 缺失或无效，无法获取安装代码。请在上方输入正确的
              Admin Token 并点击"应用"。
            </>
          ) : (
            <>无法获取安装代码，请确认 warp-insight-admin 已启动。</>
          )}
        </div>
      ) : null}
      <div className={styles.content}>
        <SubsystemBootstrapTokenCard token={token} loading={isLoading} />
        <section className={styles.commandsSection}>
          <h2 className={styles.sectionTitle}>安装命令</h2>
          <div className={styles.commandGrid}>
            <SubsystemX86LinuxInstallCode
              command={data?.x86LinuxInstallCode}
              token={token}
              loading={isLoading}
            />
            <SubsystemArmLinuxInstallCode
              command={data?.armLinuxInstallCode}
              token={token}
              loading={isLoading}
            />
          </div>
        </section>
      </div>
    </div>
  );
}
