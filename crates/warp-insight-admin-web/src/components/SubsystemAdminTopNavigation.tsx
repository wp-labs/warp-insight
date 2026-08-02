import { useEffect, useRef, useState, type FormEvent } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { NavLink } from "react-router-dom";
import { clearAdminApiToken, getAdminApiToken, setAdminApiToken } from "../api";
import styles from "./SubsystemAdminTopNavigation.module.css";

const ADMIN_AUTH_CHANGED_EVENT = "warpInsightAdminAuthChanged";

interface SubsystemAdminTopNavigationProps {
  children?: React.ReactNode;
}

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

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setAdminApiToken(token);
    setToken(getAdminApiToken() ?? "");
    setApplied(true);
    window.clearTimeout(appliedTimer.current);
    appliedTimer.current = window.setTimeout(() => setApplied(false), 1500);
    void queryClient.invalidateQueries();
  }

  function handleClear() {
    clearAdminApiToken();
    setToken("");
    void queryClient.invalidateQueries();
  }

  return (
    <div className={styles.container}>
      {children ?? (
        <>
          <div className={styles.primary}>
            <div className={styles.brand}>WarpInsight 管理台</div>
            <nav className={styles.links} aria-label="主导航">
              <NavLink
                className={({ isActive }) =>
                  isActive ? `${styles.link} ${styles.active}` : styles.link
                }
                to="/"
                end
              >
                总览
              </NavLink>
              <NavLink
                className={({ isActive }) =>
                  isActive ? `${styles.link} ${styles.active}` : styles.link
                }
                to="/control"
              >
                控制中心
              </NavLink>
              <NavLink
                className={({ isActive }) =>
                  isActive ? `${styles.link} ${styles.active}` : styles.link
                }
                to="/install"
              >
                安装
              </NavLink>
            </nav>
          </div>
          <form className={styles.authForm} onSubmit={handleSubmit}>
            <label
              className={styles.authLabel}
              htmlFor="warp-insight-admin-token"
            >
              Admin Token
            </label>
            <input
              id="warp-insight-admin-token"
              className={styles.authInput}
              type="password"
              autoComplete="off"
              value={token}
              onChange={(event) => setToken(event.target.value)}
            />
            <button className={styles.authButton} type="submit">
              {applied ? "已应用" : "应用"}
            </button>
            <button
              className={styles.authButton}
              type="button"
              onClick={handleClear}
              disabled={!token}
            >
              清除
            </button>
          </form>
        </>
      )}
    </div>
  );
}
