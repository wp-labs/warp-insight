import { useEffect, useRef, useState, type FormEvent } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { NavLink } from "react-router-dom";
import {
  ADMIN_AUTH_CHANGED_EVENT,
  clearAdminApiToken,
  getAdminApiToken,
  setAdminApiToken,
} from "../api";
import styles from "./SubsystemAdminTopNavigation.module.css";

interface SubsystemAdminTopNavigationProps {
  children?: React.ReactNode;
}

interface NavItem {
  to: string;
  label: string;
  icon: React.ReactNode;
  end?: boolean;
}

function IconOverview() {
  return (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <rect x="1.6" y="1.6" width="5.2" height="5.2" rx="1.4" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <rect x="9.2" y="1.6" width="5.2" height="5.2" rx="1.4" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <rect x="1.6" y="9.2" width="5.2" height="5.2" rx="1.4" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <rect x="9.2" y="9.2" width="5.2" height="5.2" rx="1.4" fill="none" stroke="currentColor" strokeWidth="1.4" />
    </svg>
  );
}

function IconHost() {
  return (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <rect x="1.8" y="2.4" width="12.4" height="4.6" rx="1.4" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <rect x="1.8" y="9" width="12.4" height="4.6" rx="1.4" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <circle cx="4.6" cy="4.7" r="0.9" fill="currentColor" />
      <circle cx="4.6" cy="11.3" r="0.9" fill="currentColor" />
    </svg>
  );
}

function IconControl() {
  return (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path d="M2 4.4h12M2 11.6h12" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      <circle cx="6.2" cy="4.4" r="1.9" fill="var(--sidebar-bg)" stroke="currentColor" strokeWidth="1.4" />
      <circle cx="10.4" cy="11.6" r="1.9" fill="var(--sidebar-bg)" stroke="currentColor" strokeWidth="1.4" />
    </svg>
  );
}

function IconInit() {
  return (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path d="M8 1.6v4.2" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      <path d="M4.6 3.4a5.4 5.4 0 1 0 6.8 0" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      <path d="M8 9.6v4.8" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

function IconInstall() {
  return (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path d="M8 1.8v7.4" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      <path d="M4.9 6.4 8 9.5l3.1-3.1" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M2.6 12.6h10.8" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

const NAV_GROUPS: { label: string; items: NavItem[] }[] = [
  {
    label: "监控",
    items: [
      { to: "/", label: "Agent 总览", icon: <IconOverview />, end: true },
      { to: "/hosts", label: "主机指标", icon: <IconHost /> },
    ],
  },
  {
    label: "运维",
    items: [
      { to: "/control", label: "控制中心", icon: <IconControl /> },
      { to: "/init", label: "初始化 Gateway", icon: <IconInit /> },
      { to: "/install", label: "安装 Agent", icon: <IconInstall /> },
    ],
  },
];

export function SubsystemAdminTopNavigation({
  children,
}: SubsystemAdminTopNavigationProps) {
  const queryClient = useQueryClient();
  const [token, setToken] = useState(() => getAdminApiToken() ?? "");
  const [applied, setApplied] = useState(false);
  const appliedTimer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(appliedTimer.current), []);

  useEffect(() => {
    function refreshToken() {
      setToken(getAdminApiToken() ?? "");
    }

    window.addEventListener(ADMIN_AUTH_CHANGED_EVENT, refreshToken);
    return () => {
      window.removeEventListener(ADMIN_AUTH_CHANGED_EVENT, refreshToken);
    };
  }, []);

  const hasToken = Boolean(token);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setAdminApiToken(token);
    setToken(getAdminApiToken() ?? "");
    setApplied(true);
    window.clearTimeout(appliedTimer.current);
    appliedTimer.current = window.setTimeout(() => setApplied(false), 1600);
    void queryClient.invalidateQueries();
  }

  function handleClear() {
    clearAdminApiToken();
    setToken("");
    void queryClient.invalidateQueries();
  }

  return (
    <aside className={styles.container}>
      {children ?? (
        <>
          <div className={styles.brand}>
            <span className={styles.brandMark} aria-hidden="true">
              <svg viewBox="0 0 20 20" width="17" height="17">
                <path
                  d="M10 2.4 17 6.2v7.6L10 17.6 3 13.8V6.2Z"
                  fill="none"
                  stroke="#fff"
                  strokeWidth="1.6"
                  strokeLinejoin="round"
                />
                <circle cx="10" cy="10" r="2.1" fill="#fff" />
              </svg>
            </span>
            <span className={styles.brandCopy}>
              <span className={styles.brandText}>WarpGateway</span>
              <span className={styles.brandSub}>主机与 Agent 管理台</span>
            </span>
          </div>

          <nav className={styles.links} aria-label="主导航">
            {NAV_GROUPS.map((group) => (
              <div key={group.label} className={styles.navGroup}>
                <span className={styles.navLabel}>{group.label}</span>
                {group.items.map((item) => (
                  <NavLink
                    key={item.to}
                    to={item.to}
                    end={item.end}
                    className={({ isActive }) =>
                      isActive ? `${styles.link} ${styles.active}` : styles.link
                    }
                  >
                    <span className={styles.linkIcon}>{item.icon}</span>
                    {item.label}
                  </NavLink>
                ))}
              </div>
            ))}
          </nav>

          <form className={styles.authForm} onSubmit={handleSubmit}>
            <div className={styles.authHead}>
              <label
                className={styles.authLabel}
                htmlFor="warp-insight-admin-token"
              >
                Admin Token
              </label>
              <span
                className={
                  hasToken
                    ? `${styles.authState} ${styles.authStateOn}`
                    : styles.authState
                }
              >
                {hasToken ? "已设置" : "未设置"}
              </span>
            </div>
            <input
              id="warp-insight-admin-token"
              className={styles.authInput}
              type="password"
              autoComplete="off"
              placeholder="粘贴管理令牌"
              value={token}
              onChange={(event) => setToken(event.target.value)}
            />
            <p className={styles.authHint}>
              仅存于本会话标签页，用于调用 Gateway 管理接口。
            </p>
            <div className={styles.authActions}>
              <button
                className={`${styles.authButton} ${styles.authPrimary}`}
                type="submit"
                disabled={!token || token === getAdminApiToken()}
              >
                {applied ? "已应用" : "应用"}
              </button>
              <button
                className={styles.authButton}
                type="button"
                onClick={handleClear}
                disabled={!hasToken}
              >
                清除
              </button>
            </div>
          </form>
        </>
      )}
    </aside>
  );
}
