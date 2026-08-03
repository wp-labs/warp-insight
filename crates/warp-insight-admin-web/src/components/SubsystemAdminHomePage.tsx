import styles from "./SubsystemAdminHomePage.module.css";
import { SubsystemAdminTopNavigation } from "./SubsystemAdminTopNavigation";
import { SubsystemAgentStatusOverviewMetrics } from "./SubsystemAgentStatusOverviewMetrics";
import { SubsystemRecentOnlineRegisteredAgentPanel } from "./SubsystemRecentOnlineRegisteredAgentPanel";
import { SubsystemAbnormalAgentPanel } from "./SubsystemAbnormalAgentPanel";
import { RateLimitNotice } from "./RateLimitNotice";
import { isRateLimitedError } from "../api";
import { useAgentOverview } from "../hooks";

interface SubsystemAdminHomePageProps {
  children?: React.ReactNode;
}

export function SubsystemAdminHomePage({}: SubsystemAdminHomePageProps) {
  const { data, isLoading, isError, error } = useAgentOverview();

  return (
    <div className={styles.container}>
      <SubsystemAdminTopNavigation />
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>Agent 总览</h1>
        <p className={styles.pageSummary}>
          查看已接入 Agent 的在线状态、健康状态和需要处理的异常节点。
        </p>
      </header>
      {isError ? (
        isRateLimitedError(error) ? (
          <RateLimitNotice error={error} />
        ) : (
          <div className={styles.errorBanner}>
            无法连接 warp-insight-admin，请确认管理服务已启动。
          </div>
        )
      ) : null}
      <SubsystemAgentStatusOverviewMetrics
        metrics={data?.metrics}
        loading={isLoading}
      />
      <SubsystemRecentOnlineRegisteredAgentPanel
        agents={data?.recentOnlineAgents ?? []}
        loading={isLoading}
      />
      <SubsystemAbnormalAgentPanel
        agents={data?.abnormalAgents ?? []}
        loading={isLoading}
      />
    </div>
  );
}
