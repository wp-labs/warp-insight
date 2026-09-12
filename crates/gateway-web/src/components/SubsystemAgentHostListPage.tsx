import { Link } from "react-router-dom";
import styles from "./SubsystemAgentHostListPage.module.css";
import { SubsystemAdminTopNavigation } from "./SubsystemAdminTopNavigation";
import { useAllAgentsHostMetrics } from "../hooks";

interface SubsystemAgentHostListPageProps {
  children?: React.ReactNode;
}

function formatKiB(kb?: number): string {
  if (kb === undefined || kb === null) return "—";
  const bytes = kb * 1024;
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / 1024).toFixed(0)} KB`;
}

function formatPercent(value?: number): string {
  return value === undefined || value === null ? "—" : `${value.toFixed(1)}%`;
}

function formatNumber(value?: number): string {
  return value === undefined || value === null ? "—" : value.toFixed(2);
}

export function SubsystemAgentHostListPage({}: SubsystemAgentHostListPageProps) {
  const { data, isLoading, isError } = useAllAgentsHostMetrics();
  const hosts = data ?? [];

  return (
    <div className={styles.container}>
      <SubsystemAdminTopNavigation />
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>主机指标</h1>
        <p className={styles.pageSummary}>
          每台主机的实时 CPU / 内存 / 磁盘占用，点击进入详情查看趋势。
        </p>
      </header>
      {isError ? (
        <div className={styles.errorBanner}>
          无法连接 warp-insight-admin，请确认管理服务已启动。
        </div>
      ) : null}
      {isLoading ? (
        <div className={styles.loading}>加载中…</div>
      ) : hosts.length === 0 ? (
        <div className={styles.empty}>
          暂无可查看的主机，请确认已接入 Agent 并在右上角应用 Admin Token。
        </div>
      ) : (
        <div className={styles.grid}>
          {hosts.map((host) => {
            const memoryUsedKb =
              host.memoryTotalKb !== undefined &&
              host.memoryAvailableKb !== undefined
                ? host.memoryTotalKb - host.memoryAvailableKb
                : undefined;
            return (
              <article key={host.agentId} className={styles.card}>
                <div className={styles.cardTop}>
                  <h2 className={styles.cardName}>{host.agentId}</h2>
                  <Link
                    className={styles.cardLink}
                    to={`/agents/${encodeURIComponent(host.agentId)}/metrics`}
                  >
                    查看详情 →
                  </Link>
                </div>
                <dl className={styles.metrics}>
                  <div className={styles.metric}>
                    <dt className={styles.metricLabel}>CPU 负载</dt>
                    <dd className={styles.metricValue}>
                      {formatNumber(host.loadAverage1m)}
                    </dd>
                  </div>
                  <div className={styles.metric}>
                    <dt className={styles.metricLabel}>内存已用</dt>
                    <dd className={styles.metricValue}>
                      {formatKiB(memoryUsedKb)}
                      <span className={styles.metricSub}>
                        {" "}
                        / {formatKiB(host.memoryTotalKb)}
                      </span>
                    </dd>
                  </div>
                  <div className={styles.metric}>
                    <dt className={styles.metricLabel}>磁盘使用率</dt>
                    <dd className={styles.metricValue}>
                      {formatPercent(host.diskUsagePercent)}
                    </dd>
                  </div>
                </dl>
              </article>
            );
          })}
        </div>
      )}
    </div>
  );
}
