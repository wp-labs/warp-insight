import styles from "./SubsystemAgentInstallPage.module.css";
import { SubsystemAdminTopNavigation } from "./SubsystemAdminTopNavigation";
import { SubsystemAgentInstallCodeList } from "./SubsystemAgentInstallCodeList";
import { SubsystemGetAgentInstallCode } from "./SubsystemGetAgentInstallCode";
import { useAgentInstallCode } from "../hooks";

interface SubsystemAgentInstallPageProps {
  children?: React.ReactNode;
}

export function SubsystemAgentInstallPage({  }: SubsystemAgentInstallPageProps) {
  const { data, isLoading } = useAgentInstallCode();

  return (
    <div className={styles.container}>
      <SubsystemAdminTopNavigation />
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>Agent 安装</h1>
        <p className={styles.pageSummary}>复制对应架构的安装命令，在目标 Linux 主机上执行。</p>
      </header>
      <SubsystemAgentInstallCodeList installCode={data} loading={isLoading}>
        <SubsystemGetAgentInstallCode />
      </SubsystemAgentInstallCodeList>
    </div>
  );
}
