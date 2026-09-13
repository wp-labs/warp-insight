import { useEffect, useState } from "react";
import { useIsFetching, useQueryClient } from "@tanstack/react-query";
import { matchPath, useLocation } from "react-router-dom";
import { ADMIN_AUTH_CHANGED_EVENT, getAdminApiToken } from "../api";
import styles from "./AppStatusBar.module.css";

const SECTION_LABELS: Record<string, string> = {
  monitoring: "监控",
  ops: "运维",
};

interface RouteMeta {
  section: keyof typeof SECTION_LABELS;
  crumbs: string[];
}

/** 路由 → 面包屑。detail 段由页面自身补充，这里只负责到二级。 */
function describeRoute(pathname: string): RouteMeta {
  if (matchPath("/agents/:agentId/metrics", pathname)) {
    return { section: "monitoring", crumbs: ["主机指标", "主机详情"] };
  }
  switch (pathname) {
    case "/hosts":
      return { section: "monitoring", crumbs: ["主机指标"] };
    case "/pipeline":
      return { section: "monitoring", crumbs: ["数据采集"] };
    case "/control":
      return { section: "ops", crumbs: ["控制中心"] };
    case "/install":
      return { section: "ops", crumbs: ["安装 Agent"] };
    case "/init":
      return { section: "ops", crumbs: ["初始化 Gateway"] };
    case "/":
      return { section: "monitoring", crumbs: ["Agent 总览"] };
    default:
      return { section: "monitoring", crumbs: ["未知页面"] };
  }
}

/**
 * 全局状态条：跨页面常驻，回答三个运维最常问的问题
 * —— 我在哪儿、数据是不是在自动刷新、上一次成功同步是什么时候。
 */
export function AppStatusBar() {
  const { pathname } = useLocation();
  const queryClient = useQueryClient();
  const fetching = useIsFetching();
  const [lastSyncedAt, setLastSyncedAt] = useState<number | null>(null);
  const [hasToken, setHasToken] = useState(() => Boolean(getAdminApiToken()));

  useEffect(() => {
    const unsubscribe = queryClient.getQueryCache().subscribe((event) => {
      if (event?.type === "updated" && event.query.state.dataUpdatedAt) {
        setLastSyncedAt(event.query.state.dataUpdatedAt);
      }
    });
    return unsubscribe;
  }, [queryClient]);

  useEffect(() => {
    const onAuthChanged = () => setHasToken(Boolean(getAdminApiToken()));
    window.addEventListener(ADMIN_AUTH_CHANGED_EVENT, onAuthChanged);
    return () =>
      window.removeEventListener(ADMIN_AUTH_CHANGED_EVENT, onAuthChanged);
  }, []);

  const route = describeRoute(pathname);
  const syncing = fetching > 0;

  return (
    <div className={styles.bar}>
      <nav className={styles.crumbs} aria-label="面包屑">
        <span className={styles.crumbRoot}>{SECTION_LABELS[route.section]}</span>
        {route.crumbs.map((crumb, index) => (
          <span key={crumb} className={styles.crumbGroup}>
            <span className={styles.sep} aria-hidden="true">
              /
            </span>
            <span
              className={
                index === route.crumbs.length - 1
                  ? styles.crumbCurrent
                  : styles.crumb
              }
            >
              {crumb}
            </span>
          </span>
        ))}
      </nav>

      <div className={styles.right}>
        <span
          className={
            hasToken
              ? `${styles.pill} ${styles.pillLive}`
              : `${styles.pill} ${styles.pillIdle}`
          }
          title={
            hasToken
              ? "每 5 秒自动拉取一次 Agent 与指标数据"
              : "未设置 Admin Token，数据不会自动拉取"
          }
        >
          <span
            className={syncing ? styles.dotSyncing : styles.dot}
            aria-hidden="true"
          />
          {hasToken ? (syncing ? "同步中" : "自动刷新 5s") : "未授权"}
        </span>

        <span className={styles.stamp}>
          最近同步
          <strong className={styles.stampValue}>
            {lastSyncedAt
              ? new Intl.DateTimeFormat("zh-CN", {
                  hour: "2-digit",
                  minute: "2-digit",
                  second: "2-digit",
                  hour12: false,
                }).format(new Date(lastSyncedAt))
              : "—"}
          </strong>
        </span>

        <button
          type="button"
          className={styles.refresh}
          onClick={() => void queryClient.invalidateQueries()}
          disabled={!hasToken}
        >
          <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
            <path
              d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 1.5V5H10"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          刷新
        </button>
      </div>
    </div>
  );
}
