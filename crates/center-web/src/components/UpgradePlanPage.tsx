import { PageShell } from "./ui";
import { UpgradePlanCreatePanel } from "./UpgradePlanCreatePanel";
import styles from "./UpgradePlanPage.module.css";

export function UpgradePlanPage() {
  return (
    <PageShell
      title="升级计划"
      summary="创建多组件、多步骤的 WistAgentd / WarpGateWay 升级计划；批准入口见「批准升级计划」菜单。"
    >
      <div className={styles.grid}>
        <UpgradePlanCreatePanel />
      </div>
    </PageShell>
  );
}
