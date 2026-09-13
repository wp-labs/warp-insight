import { useMemo } from "react";
import { Link, useParams } from "react-router-dom";
import styles from "./SubsystemAgentHostMetricsPage.module.css";
import { TrendChart } from "./TrendChart";
import { useAgentHostMetrics, useAgentOverview } from "../hooks";
import {
  formatKiB,
  formatNumber,
  formatPercent,
  formatUptime,

  severityForLoad,
  severityForUsage,
  SEVERITY_LABEL,
  type Severity,
} from "../lib/format";

interface SubsystemAgentHostMetricsPageProps {
  children?: React.ReactNode;
}

function severityClass(severity: Severity): string {
  return {
    ok: styles.text_ok,
    warn: styles.text_warn,
    crit: styles.text_crit,
    unknown: styles.text_unknown,
  }[severity];
}

function barClass(severity: Severity): string {
  return {
    ok: styles.bar_ok,
    warn: styles.bar_warn,
    crit: styles.bar_crit,
    unknown: styles.bar_unknown,
  }[severity];
}

/** 用真实点位跨度描述趋势时间窗，避免标题写死"近 1 小时"却只有 10 分钟数据。 */
function describeSpan(points?: [number, number][]): string {
  if (!points || points.length < 2) return "暂无历史";
  const span = points[points.length - 1][0] - points[0][0];
  const minutes = Math.round(span / 60_000);
  if (minutes < 60) return `近 ${Math.max(minutes, 1)} 分钟`;
  const hours = minutes / 60;
  if (hours < 24) return `近 ${hours.toFixed(hours < 10 ? 1 : 0)} 小时`;
  return `近 ${(hours / 24).toFixed(1)} 天`;
}

export function SubsystemAgentHostMetricsPage(
  {}: SubsystemAgentHostMetricsPageProps,
) {
  const { agentId = "" } = useParams<{ agentId: string }>();
  const { data, isLoading, isError } = useAgentHostMetrics(agentId);
  const { data: overview } = useAgentOverview();

  const agent = useMemo(
    () => overview?.recentOnlineAgents.find((item) => item.agentId === agentId),
    [agentId, overview],
  );

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

  const currentLoad = data?.loadAverage1m;
  const diskSeverity = severityForUsage(data?.diskUsagePercent);
  const memorySeverity = severityForUsage(memoryUsagePercent);
  const loadSeverity = severityForLoad(currentLoad);
  const span = describeSpan(data?.history?.loadAverage1m);

  return (
    <div className={styles.container}>
      <header className={styles.pageHeader}>
        <Link className={styles.back} to="/hosts">
          <span aria-hidden="true">←</span> 返回机队视图
        </Link>
        <div className={styles.titleRow}>
          <h1 className={styles.pageTitle}>{agentId || "未指定主机"}</h1>
          <span className={styles.liveBadge}>
            <span className={styles.liveDot} aria-hidden="true" />
            5 秒自动刷新
          </span>
        </div>
        <div className={styles.metaRow}>
          {agent ? (
            <>
              <span className={styles.metaItem}>
                版本 <strong>{agent.version}</strong>
              </span>
              <span className={styles.metaItem}>
                实例 <strong>{agent.instanceId}</strong>
              </span>
              <span className={styles.metaItem}>
                Agent 在线时长{" "}
                <strong>{formatUptime(agent.onlineDurationSeconds)}</strong>
              </span>
              <span className={styles.metaItem}>
                Admin 延迟 <strong>{formatNumber(agent.adminLatencyMs, 0)} ms</strong>
              </span>
            </>
          ) : (
            <span className={styles.metaItemMuted}>
              该主机不在最近上线列表中，仅展示指标数据。
            </span>
          )}
        </div>
      </header>

      {isError ? (
        <div className={styles.errorBanner}>
          无法加载主机指标，请确认该 Agent 已接入且数据面 VictoriaMetrics 可用。
        </div>
      ) : null}

      {isLoading ? (
        <div className={styles.skeletonWrap}>
          {[0, 1, 2, 3].map((index) => (
            <div key={index} className={styles.skeletonTile} />
          ))}
        </div>
      ) : (
        <section className={styles.tiles}>
          <article className={styles.tile}>
            <h2 className={styles.tileTitle}>CPU 负载</h2>
            <p className={`${styles.tileValue} ${severityClass(loadSeverity)}`}>
              {formatNumber(currentLoad)}
            </p>
            <span className={styles.tileCaption}>
              当前分级：{SEVERITY_LABEL[loadSeverity]}（1 分钟）
            </span>
            <dl className={styles.subList}>
              <div className={styles.subRow}>
                <dt>5 分钟</dt>
                <dd>{formatNumber(data?.loadAverage5m)}</dd>
              </div>
              <div className={styles.subRow}>
                <dt>15 分钟</dt>
                <dd>{formatNumber(data?.loadAverage15m)}</dd>
              </div>
            </dl>
          </article>

          <article className={styles.tile}>
            <h2 className={styles.tileTitle}>内存</h2>
            <p className={`${styles.tileValue} ${severityClass(memorySeverity)}`}>
              {formatPercent(memoryUsagePercent)}
            </p>
            <span className={styles.tileCaption}>
              {formatKiB(memoryUsedKb)} / {formatKiB(data?.memoryTotalKb)}
            </span>
            <div className={styles.barTrack}>
              <div
                className={`${styles.barFill} ${barClass(memorySeverity)}`}
                style={{
                  width: `${Math.min(Math.max(memoryUsagePercent ?? 0, 0), 100)}%`,
                }}
              />
            </div>
            <dl className={styles.subList}>
              <div className={styles.subRow}>
                <dt>总量</dt>
                <dd>{formatKiB(data?.memoryTotalKb)}</dd>
              </div>
              <div className={styles.subRow}>
                <dt>可用</dt>
                <dd>{formatKiB(data?.memoryAvailableKb)}</dd>
              </div>
            </dl>
          </article>

          <article className={styles.tile}>
            <h2 className={styles.tileTitle}>磁盘</h2>
            <p className={`${styles.tileValue} ${severityClass(diskSeverity)}`}>
              {formatPercent(data?.diskUsagePercent)}
            </p>
            <span className={styles.tileCaption}>
              当前分级：{SEVERITY_LABEL[diskSeverity]}
            </span>
            <div className={styles.barTrack}>
              <div
                className={`${styles.barFill} ${barClass(diskSeverity)}`}
                style={{
                  width: `${Math.min(Math.max(data?.diskUsagePercent ?? 0, 0), 100)}%`,
                }}
              />
            </div>
            <dl className={styles.subList}>
              <div className={styles.subRow}>
                <dt>总量</dt>
                <dd>{formatKiB(data?.diskTotalKb)}</dd>
              </div>
              <div className={styles.subRow}>
                <dt>可用</dt>
                <dd>{formatKiB(data?.diskAvailableKb)}</dd>
              </div>
            </dl>
          </article>

          <article className={styles.tile}>
            <h2 className={styles.tileTitle}>主机开机时长</h2>
            <p className={styles.tileValue}>{formatUptime(data?.uptimeSeconds)}</p>
            <span className={styles.tileCaption}>操作系统开机至今，与 Agent 在线时长不同</span>
            <dl className={styles.subList}>
              <div className={styles.subRow}>
                <dt>指标采集窗口</dt>
                <dd>{span}</dd>
              </div>
              {agent ? (
                <div className={styles.subRow}>
                  <dt>注册时间</dt>
                  <dd>
                    {new Intl.DateTimeFormat("zh-CN", {
                      month: "2-digit",
                      day: "2-digit",
                      hour: "2-digit",
                      minute: "2-digit",
                      hour12: false,
                    }).format(new Date(agent.registeredAt))}
                  </dd>
                </div>
              ) : null}
            </dl>
          </article>
        </section>
      )}

      {data?.history ? (
        <section className={styles.trends}>
          <div className={styles.trendsHead}>
            <h2 className={styles.trendsTitle}>指标趋势</h2>
            <span className={styles.trendsHint}>
              {span} · 悬停折线可查看任意时刻取值
            </span>
          </div>

          <article className={styles.trendCard}>
            <h3 className={styles.trendCardTitle}>CPU 负载</h3>
            <TrendChart
              series={[
                {
                  name: "1 分钟",
                  color: "var(--series-1)",
                  points: data.history.loadAverage1m,
                },
                {
                  name: "5 分钟",
                  color: "var(--series-2)",
                  points: data.history.loadAverage5m,
                },
                {
                  name: "15 分钟",
                  color: "var(--series-3)",
                  points: data.history.loadAverage15m,
                },
              ]}
              valueFormatter={(value) => value.toFixed(2)}
            />
          </article>

          <div className={styles.trendGrid}>
            <article className={styles.trendCard}>
              <h3 className={styles.trendCardTitle}>内存可用量</h3>
              <TrendChart
                height={168}
                series={[
                  {
                    name: "可用",
                    color: "var(--series-1)",
                    points: data.history.memoryAvailableKb,
                  },
                ]}
                valueFormatter={(value) => formatKiB(value)}
              />
            </article>

            <article className={styles.trendCard}>
              <h3 className={styles.trendCardTitle}>磁盘使用率</h3>
              <TrendChart
                height={168}
                series={[
                  {
                    name: "使用率",
                    color: "var(--series-5)",
                    points: data.history.diskUsagePercent,
                  },
                ]}
                valueFormatter={(value) => `${value.toFixed(1)}%`}
              />
            </article>
          </div>
        </section>
      ) : null}
    </div>
  );
}
