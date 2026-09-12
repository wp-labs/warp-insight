import { useParams } from "react-router-dom";
import styles from "./SubsystemAgentHostMetricsPage.module.css";
import { SubsystemAdminTopNavigation } from "./SubsystemAdminTopNavigation";
import { TrendChart } from "./TrendChart";
import { useAgentHostMetrics } from "../hooks";

interface SubsystemAgentHostMetricsPageProps {
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

function formatUptime(seconds?: number): string {
  if (seconds === undefined || seconds === null) return "—";
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days} 天 ${hours} 小时`;
  if (hours > 0) return `${hours} 小时 ${minutes} 分钟`;
  return `${minutes} 分钟`;
}

export function SubsystemAgentHostMetricsPage({}: SubsystemAgentHostMetricsPageProps) {
  const { agentId = "" } = useParams<{ agentId: string }>();
  const { data, isLoading, isError } = useAgentHostMetrics(agentId);

  const memoryUsedKb =
    data?.memoryTotalKb !== undefined && data?.memoryAvailableKb !== undefined
      ? data.memoryTotalKb - data.memoryAvailableKb
      : undefined;
  const memoryUsagePercent =
    data?.memoryTotalKb !== undefined && data.memoryTotalKb > 0
      ? ((data.memoryTotalKb - (data.memoryAvailableKb ?? 0)) /
          data.memoryTotalKb) *
        100
      : undefined;

  return (
    <div className={styles.container}>
      <SubsystemAdminTopNavigation />
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>主机指标</h1>
        <p className={styles.pageSummary}>
          目标主机：<strong>{agentId || "未指定"}</strong> · 5 秒自动刷新
        </p>
      </header>
      {isError ? (
        <div className={styles.errorBanner}>
          无法加载主机指标，请确认该 Agent 已接入且数据面 VictoriaMetrics 可用。
        </div>
      ) : null}
      {isLoading ? (
        <div className={styles.loading}>加载中…</div>
      ) : (
        <section className={styles.cards}>
          <article className={styles.card}>
            <h2 className={styles.cardTitle}>CPU 负载</h2>
            <dl className={styles.cardBody}>
              <div className={styles.row}>
                <dt>1 分钟</dt>
                <dd>{formatNumber(data?.loadAverage1m)}</dd>
              </div>
              <div className={styles.row}>
                <dt>5 分钟</dt>
                <dd>{formatNumber(data?.loadAverage5m)}</dd>
              </div>
              <div className={styles.row}>
                <dt>15 分钟</dt>
                <dd>{formatNumber(data?.loadAverage15m)}</dd>
              </div>
            </dl>
          </article>
          <article className={styles.card}>
            <h2 className={styles.cardTitle}>内存</h2>
            <dl className={styles.cardBody}>
              <div className={styles.row}>
                <dt>总量</dt>
                <dd>{formatKiB(data?.memoryTotalKb)}</dd>
              </div>
              <div className={styles.row}>
                <dt>可用</dt>
                <dd>{formatKiB(data?.memoryAvailableKb)}</dd>
              </div>
              <div className={styles.row}>
                <dt>已用</dt>
                <dd>{formatKiB(memoryUsedKb)}</dd>
              </div>
              <div className={styles.row}>
                <dt>使用率</dt>
                <dd>{formatPercent(memoryUsagePercent)}</dd>
              </div>
            </dl>
          </article>
          <article className={styles.card}>
            <h2 className={styles.cardTitle}>磁盘</h2>
            <dl className={styles.cardBody}>
              <div className={styles.row}>
                <dt>使用率</dt>
                <dd>{formatPercent(data?.diskUsagePercent)}</dd>
              </div>
              <div className={styles.row}>
                <dt>总量</dt>
                <dd>{formatKiB(data?.diskTotalKb)}</dd>
              </div>
              <div className={styles.row}>
                <dt>可用</dt>
                <dd>{formatKiB(data?.diskAvailableKb)}</dd>
              </div>
            </dl>
          </article>
          <article className={styles.card}>
            <h2 className={styles.cardTitle}>运行时长</h2>
            <dl className={styles.cardBody}>
              <div className={styles.row}>
                <dt>开机至今</dt>
                <dd>{formatUptime(data?.uptimeSeconds)}</dd>
              </div>
            </dl>
          </article>
        </section>
      )}
      {data?.history ? (
        <section className={styles.trends}>
          <h2 className={styles.trendsTitle}>趋势（近 1 小时）</h2>
          <article className={styles.trendCard}>
            <h3 className={styles.trendCardTitle}>CPU 负载</h3>
            <TrendChart
              series={[
                {
                  name: "1 分钟",
                  color: "#0550ae",
                  points: data.history.loadAverage1m,
                },
                {
                  name: "5 分钟",
                  color: "#cf222e",
                  points: data.history.loadAverage5m,
                },
                {
                  name: "15 分钟",
                  color: "#22863a",
                  points: data.history.loadAverage15m,
                },
              ]}
              valueFormatter={(value) => value.toFixed(2)}
            />
          </article>
          <article className={styles.trendCard}>
            <h3 className={styles.trendCardTitle}>内存（可用）</h3>
            <TrendChart
              series={[
                {
                  name: "可用",
                  color: "#0550ae",
                  points: data.history.memoryAvailableKb,
                },
              ]}
              valueFormatter={(value) => formatKiB(value)}
            />
          </article>
          <article className={styles.trendCard}>
            <h3 className={styles.trendCardTitle}>磁盘使用率</h3>
            <TrendChart
              series={[
                {
                  name: "使用率",
                  color: "#cf222e",
                  points: data.history.diskUsagePercent,
                },
              ]}
              valueFormatter={(value) => `${value.toFixed(1)}%`}
            />
          </article>
        </section>
      ) : null}
    </div>
  );
}
