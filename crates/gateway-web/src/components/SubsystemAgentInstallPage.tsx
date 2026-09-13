import styles from "./SubsystemAgentInstallPage.module.css";
import { SubsystemBootstrapTokenCard } from "./SubsystemBootstrapTokenCard";
import { SubsystemX86LinuxInstallCode } from "./SubsystemX86LinuxInstallCode";
import { SubsystemArmLinuxInstallCode } from "./SubsystemArmLinuxInstallCode";
import { SubsystemMacOSInstallCode } from "./SubsystemMacOSInstallCode";
import { ApiError, isRateLimitedError } from "../api";
import { useAgentInstallCode } from "../hooks";
import { RateLimitNotice } from "./RateLimitNotice";

export function SubsystemAgentInstallPage() {
  const { data, isLoading, isError, error } = useAgentInstallCode();

  const authError = isError && error instanceof ApiError && error.status === 401;
  const token = data?.bootstrapEnrollmentToken;

  return (
    <div className={styles.container}>
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>安装 Agent</h1>
        <p className={styles.pageSummary}>
          在目标主机上装一次采集 Agent，它就会把该主机的指标与日志持续上报到本网关。
        </p>
        <ol className={styles.steps}>
          <li>复制 Bootstrap Token（一次性注册凭证）</li>
          <li>在目标主机上按架构执行对应安装命令</li>
          <li>回到「主机指标」确认该主机已开始上报</li>
        </ol>
      </header>
      {isError ? (
        isRateLimitedError(error) ? (
          <RateLimitNotice error={error} />
        ) : (
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
        )
      ) : null}
      <div className={styles.content}>
        <SubsystemBootstrapTokenCard token={token} loading={isLoading} />
        <section className={styles.commandsSection}>
          <h2 className={styles.sectionTitle}>按目标主机架构选择安装命令</h2>
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
            <SubsystemMacOSInstallCode
              command={data?.macosInstallCode}
              token={token}
              loading={isLoading}
            />
          </div>
        </section>
      </div>
    </div>
  );
}
