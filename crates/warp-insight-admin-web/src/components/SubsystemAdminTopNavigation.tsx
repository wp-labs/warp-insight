import { NavLink } from "react-router-dom";
import styles from "./SubsystemAdminTopNavigation.module.css";

interface SubsystemAdminTopNavigationProps {
  children?: React.ReactNode;
}

export function SubsystemAdminTopNavigation({ children }: SubsystemAdminTopNavigationProps) {
  return (
    <div className={styles.container}>
      {children ?? (
        <>
          <div className={styles.brand}>WarpInsight 管理台</div>
          <nav className={styles.links} aria-label="主导航">
            <NavLink
              className={({ isActive }) =>
                isActive ? `${styles.link} ${styles.active}` : styles.link
              }
              to="/" end
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
        </>
      )}
    </div>
  );
}
