import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import styles from "./SubsystemAgentHostListPage.module.css";
import { useAllAgentsHostMetrics } from "../hooks";
import {
  formatKiB,
  formatNumber,
  formatPercent,
  severityForLoad,
  severityForUsage,
  SEVERITY_LABEL,
  type Severity,
} from "../lib/format";

interface SubsystemAgentHostListPageProps {
  children?: React.ReactNode;
}

type SortKey = "severity" | "agentId" | "cpu" | "memory" | "disk";

interface HostRow {
  agentId: string;
  loadAverage1m?: number;
  memoryTotalKb?: number;
  memoryAvailableKb?: number;
  diskUsagePercent?: number;
  memoryUsedKb?: number;
  memoryUsagePercent?: number;
  severity: Severity;
}

const SEVERITY_ORDER: Record<Severity, number> = {
  crit: 0,
  warn: 1,
  unknown: 2,
  ok: 3,
};

function severityRank(left: Severity, right: Severity): number {
  return SEVERITY_ORDER[left] - SEVERITY_ORDER[right];
}

function worst(...values: Severity[]): Severity {
  return values.reduce((acc, cur) => (severityRank(cur, acc) < 0 ? cur : acc));
}

function severityClass(kind: "badge" | "text" | "bar", severity: Severity) {
  const map = {
    badge: {
      ok: styles.badge_ok,
      warn: styles.badge_warn,
      crit: styles.badge_crit,
      unknown: styles.badge_unknown,
    },
    text: {
      ok: styles.text_ok,
      warn: styles.text_warn,
      crit: styles.text_crit,
      unknown: styles.text_unknown,
    },
    bar: {
      ok: styles.bar_ok,
      warn: styles.bar_warn,
      crit: styles.bar_crit,
      unknown: styles.bar_unknown,
    },
  } as const;
  return map[kind][severity];
}

export function SubsystemAgentHostListPage({}: SubsystemAgentHostListPageProps) {
  const { data, isLoading, isError } = useAllAgentsHostMetrics();
  const [query, setQuery] = useState("");
  const [severityFilter, setSeverityFilter] = useState<Severity | "all">("all");
  const [sortKey, setSortKey] = useState<SortKey>("severity");
  const [sortAsc, setSortAsc] = useState(true);

  const rows: HostRow[] = useMemo(() => {
    return (data ?? []).map((host) => {
      const memoryUsagePercent =
        host.memoryTotalKb && host.memoryTotalKb > 0
          ? ((host.memoryTotalKb - (host.memoryAvailableKb ?? 0)) /
              host.memoryTotalKb) *
            100
          : undefined;
      return {
        ...host,
        memoryUsagePercent,
        memoryUsedKb:
          host.memoryTotalKb !== undefined &&
          host.memoryAvailableKb !== undefined
            ? host.memoryTotalKb - host.memoryAvailableKb
            : undefined,
        severity: worst(
          severityForUsage(memoryUsagePercent),
          severityForUsage(host.diskUsagePercent),
          severityForLoad(host.loadAverage1m),
        ),
      };
    });
  }, [data]);

  const counts = useMemo(() => {
    const base: Record<Severity, number> = {
      ok: 0,
      warn: 0,
      crit: 0,
      unknown: 0,
    };
    for (const row of rows) base[row.severity] += 1;
    return base;
  }, [rows]);

  const visible = useMemo(() => {
    const keyword = query.trim().toLowerCase();
    const filtered = rows.filter((row) => {
      if (severityFilter !== "all" && row.severity !== severityFilter) {
        return false;
      }
      if (keyword && !row.agentId.toLowerCase().includes(keyword)) return false;
      return true;
    });

    return [...filtered].sort((a, b) => {
      // 默认视图按严重度排序：需要处理的主机永远排在前面，
      // 同级再按磁盘使用率从高到低。
      if (sortKey === "severity") {
        const bySeverity = severityRank(a.severity, b.severity);
        if (bySeverity !== 0) return bySeverity;
        return (b.diskUsagePercent ?? -1) - (a.diskUsagePercent ?? -1);
      }

      const pick = (row: HostRow): number | string => {
        switch (sortKey) {
          case "cpu":
            return row.loadAverage1m ?? -1;
          case "memory":
            return row.memoryUsagePercent ?? -1;
          case "disk":
            return row.diskUsagePercent ?? -1;
          default:
            return row.agentId;
        }
      };

      const left = pick(a);
      const right = pick(b);
      const result =
        typeof left === "string" && typeof right === "string"
          ? left.localeCompare(right)
          : Number(left) - Number(right);
      return sortAsc ? result : -result;
    });
  }, [query, rows, severityFilter, sortAsc, sortKey]);

  function toggleSort(key: SortKey) {
    if (key === sortKey) {
      setSortAsc((prev) => !prev);
    } else {
      setSortKey(key);
      setSortAsc(key === "agentId" || key === "severity");
    }
  }

  const filters: { key: Severity | "all"; label: string; count: number }[] = [
    { key: "all", label: "全部", count: rows.length },
    { key: "crit", label: SEVERITY_LABEL.crit, count: counts.crit },
    { key: "warn", label: SEVERITY_LABEL.warn, count: counts.warn },
    { key: "ok", label: SEVERITY_LABEL.ok, count: counts.ok },
    { key: "unknown", label: SEVERITY_LABEL.unknown, count: counts.unknown },
  ];

  return (
    <div className={styles.container}>
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>主机指标</h1>
        <p className={styles.pageSummary}>
          机队实时负载视图：一行一台已接入主机，按严重度排序，优先暴露需要处理的主机。
          数据每 5 秒刷新。
        </p>
        <p className={styles.pageThresholds}>
          分级口径：内存 / 磁盘使用率 ≥90% 为危急、≥75% 为偏高；1 分钟负载按 8 核基准折算。
        </p>
      </header>

      {isError ? (
        <div className={styles.errorBanner}>
          无法连接 warp-insight-admin，请确认管理服务已启动并在左下角设置 Admin Token。
        </div>
      ) : null}

      {isLoading ? (
        <div className={styles.skeletonWrap}>
          {[0, 1, 2].map((index) => (
            <div key={index} className={styles.skeletonRow} />
          ))}
        </div>
      ) : rows.length === 0 ? (
        <div className={styles.empty}>
          <strong className={styles.emptyTitle}>暂无主机数据</strong>
          <span className={styles.emptyText}>
            确认 Agent 已启动并完成注册；也可以在「安装 Agent」页获取安装命令。
          </span>
        </div>
      ) : (
        <section className={styles.fleet}>
          <div className={styles.toolbar}>
            <div className={styles.search}>
              <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
                <circle
                  cx="7"
                  cy="7"
                  r="4.6"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                />
                <path
                  d="M10.6 10.6 14 14"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                />
              </svg>
              <input
                type="search"
                className={styles.searchInput}
                placeholder="搜索主机 ID"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                aria-label="搜索主机 ID"
              />
            </div>

            <div className={styles.filters} role="group" aria-label="按状态筛选">
              {filters.map((filter) => {
                const active = filter.key === severityFilter;
                const tone =
                  filter.key === "all"
                    ? undefined
                    : severityClass("badge", filter.key);
                return (
                  <button
                    key={filter.key}
                    type="button"
                    className={
                      active ? `${styles.chip} ${styles.chipActive}` : styles.chip
                    }
                    onClick={() => setSeverityFilter(filter.key)}
                    aria-pressed={active}
                  >
                    {tone ? (
                      <span className={`${styles.chipDot} ${tone}`} aria-hidden="true" />
                    ) : null}
                    {filter.label}
                    <span className={styles.chipCount}>{filter.count}</span>
                  </button>
                );
              })}
            </div>

            <span className={styles.resultCount}>
              显示 <strong>{visible.length}</strong> / {rows.length} 台
            </span>
          </div>

          <div className={styles.tableWrap}>
            <table className={styles.table}>
              <thead>
                <tr>
                  <th scope="col" className={styles.thHost}>
                    <button
                      type="button"
                      className={styles.sortHead}
                      onClick={() => toggleSort("agentId")}
                    >
                      主机
                      <SortMark active={sortKey === "agentId"} asc={sortAsc} />
                    </button>
                  </th>
                  <th scope="col">
                    <button
                      type="button"
                      className={styles.sortHead}
                      onClick={() => toggleSort("severity")}
                    >
                      状态
                      <SortMark active={sortKey === "severity"} asc={sortAsc} />
                    </button>
                  </th>
                  <th scope="col">
                    <button
                      type="button"
                      className={styles.sortHead}
                      onClick={() => toggleSort("cpu")}
                    >
                      CPU 负载 1m
                      <SortMark active={sortKey === "cpu"} asc={sortAsc} />
                    </button>
                  </th>
                  <th scope="col">
                    <button
                      type="button"
                      className={styles.sortHead}
                      onClick={() => toggleSort("memory")}
                    >
                      内存
                      <SortMark active={sortKey === "memory"} asc={sortAsc} />
                    </button>
                  </th>
                  <th scope="col">
                    <button
                      type="button"
                      className={styles.sortHead}
                      onClick={() => toggleSort("disk")}
                    >
                      磁盘
                      <SortMark active={sortKey === "disk"} asc={sortAsc} />
                    </button>
                  </th>
                  <th scope="col" className={styles.thAction}>
                    <span className={styles.srOnly}>操作</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {visible.map((row) => (
                  <tr key={row.agentId}>
                    <td className={styles.tdHost}>
                      <Link
                        className={styles.hostLink}
                        to={`/agents/${encodeURIComponent(row.agentId)}/metrics`}
                      >
                        {row.agentId}
                      </Link>
                    </td>
                    <td>
                      <span
                        className={`${styles.badge} ${severityClass("badge", row.severity)}`}
                      >
                        {SEVERITY_LABEL[row.severity]}
                      </span>
                    </td>
                    <td className={styles.tdMetric}>
                      <MetricBar
                        percent={undefined}
                        value={formatNumber(row.loadAverage1m)}
                        severity={severityForLoad(row.loadAverage1m)}
                      />
                    </td>
                    <td className={styles.tdMetric}>
                      <MetricBar
                        percent={row.memoryUsagePercent}
                        value={formatPercent(row.memoryUsagePercent)}
                        detail={`${formatKiB(row.memoryUsedKb)} / ${formatKiB(row.memoryTotalKb)}`}
                        severity={severityForUsage(row.memoryUsagePercent)}
                      />
                    </td>
                    <td className={styles.tdMetric}>
                      <MetricBar
                        percent={row.diskUsagePercent}
                        value={formatPercent(row.diskUsagePercent)}
                        severity={severityForUsage(row.diskUsagePercent)}
                      />
                    </td>
                    <td className={styles.tdAction}>
                      <Link
                        className={styles.detailLink}
                        to={`/agents/${encodeURIComponent(row.agentId)}/metrics`}
                      >
                        详情
                        <span aria-hidden="true">→</span>
                      </Link>
                    </td>
                  </tr>
                ))}
                {visible.length === 0 ? (
                  <tr>
                    <td className={styles.tdEmpty} colSpan={6}>
                      没有符合条件的主机，试试换个关键词或状态筛选。
                    </td>
                  </tr>
                ) : null}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </div>
  );
}

function SortMark({ active, asc }: { active: boolean; asc: boolean }) {
  return (
    <span
      className={active ? styles.sortMarkActive : styles.sortMark}
      aria-hidden="true"
    >
      {active ? (asc ? "↑" : "↓") : "↕"}
    </span>
  );
}

function MetricBar({
  percent,
  value,
  detail,
  severity,
}: {
  percent?: number;
  value: string;
  detail?: string;
  severity: Severity;
}) {
  return (
    <div className={styles.metricCell}>
      <div className={styles.metricTop}>
        <span className={`${styles.metricValue} ${severityClass("text", severity)}`}>
          {value}
        </span>
        {detail ? <span className={styles.metricDetail}>{detail}</span> : null}
      </div>
      {percent !== undefined ? (
        <div className={styles.barTrack}>
          <div
            className={`${styles.barFill} ${severityClass("bar", severity)}`}
            style={{ width: `${Math.min(Math.max(percent, 0), 100)}%` }}
          />
        </div>
      ) : null}
    </div>
  );
}
