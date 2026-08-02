import styles from "./SubsystemAgentInstallPage.module.css";
import { SubsystemAdminTopNavigation } from "./SubsystemAdminTopNavigation";
import { SubsystemAgentInstallCodeList } from "./SubsystemAgentInstallCodeList";
import { SubsystemGetAgentInstallCode } from "./SubsystemGetAgentInstallCode";
import { ApiError } from "../api";
import { useAgentInstallCode } from "../hooks";

interface SubsystemAgentInstallPageProps {
  children?: React.ReactNode;
}

export function SubsystemAgentInstallPage({}: SubsystemAgentInstallPageProps) {
  const { data, isLoading, isError, error } = useAgentInstallCode();

  const authError =
    isError && error instanceof ApiError && error.status === 401;

  return (
    <div className={styles.container}>
      <SubsystemAdminTopNavigation />
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>Agent 安装</h1>
        <p className={styles.pageSummary}>
          复制对应架构的安装命令，在目标 Linux 主机上执行。
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
      <SubsystemAgentInstallCodeList installCode={data} loading={isLoading}>
        <SubsystemGetAgentInstallCode />
      </SubsystemAgentInstallCodeList>
    </div>
  );
}
