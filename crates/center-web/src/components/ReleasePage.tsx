import { useState } from "react";
import { useGatewayAgentsForAll, useGatewayStatusView } from "../hooks";
import { PageShell } from "./ui";
import { WistAgentdReleasePanel } from "./WistAgentdReleasePanel";
import { WarpGateWayReleasePanel } from "./WarpGateWayReleasePanel";
import { GatewayOnlineStatusBadge } from "./GatewayOnlineStatusBadge";
import { GatewayVersionText } from "./GatewayVersionText";
import styles from "./ReleasePage.module.css";

/** 版本发布：发布 WistAgentd / WarpGateWay 新版本，并展示各网关/Agent 的当前与历史版本信息。 */
export function ReleasePage() {
  const [activeTarget, setActiveTarget] = useState<"gateway" | "agentd">(
    "gateway",
  );
  const { data: statusData } = useGatewayStatusView();
  const gateways = statusData?.data ?? [];
  const gatewayIds = gateways.map((gateway) => gateway.gatewayId);
  const { data: agentsData } = useGatewayAgentsForAll(gatewayIds);
  const agents = agentsData ?? [];

  return (
    <PageShell
      title="版本发布"
      summary="选择发布目标，提交版本产物并查看发布记录；当前已部署版本用于评估升级范围。"
    >
      <section className={styles.workspace}>
        <header className={styles.workspaceHeader}>
          <div>
            <div className={styles.eyebrow}>Release Workspace</div>
            <h2 className={styles.workspaceTitle}>发布新版本</h2>
            <p className={styles.workspaceSubtitle}>
              一次只处理一个发布目标，减少误发布并让历史记录保持聚焦。
            </p>
          </div>
        </header>
        <div className={styles.tabs} role="tablist" aria-label="发布目标">
          <button
            type="button"
            role="tab"
            id="release-tab-gateway"
            aria-selected={activeTarget === "gateway"}
            aria-controls="release-panel"
            className={
              activeTarget === "gateway"
                ? `${styles.tab} ${styles.tabActive}`
                : styles.tab
            }
            onClick={() => setActiveTarget("gateway")}
          >
            <strong>WarpGateWay</strong>
            <span>网关运行时</span>
          </button>
          <button
            type="button"
            role="tab"
            id="release-tab-agentd"
            aria-selected={activeTarget === "agentd"}
            aria-controls="release-panel"
            className={
              activeTarget === "agentd"
                ? `${styles.tab} ${styles.tabActive}`
                : styles.tab
            }
            onClick={() => setActiveTarget("agentd")}
          >
            <strong>WistAgentd</strong>
            <span>Agent 服务</span>
          </button>
        </div>
        <div
          id="release-panel"
          role="tabpanel"
          aria-labelledby={
            activeTarget === "gateway"
              ? "release-tab-gateway"
              : "release-tab-agentd"
          }
          className={styles.tabPanel}
        >
          {activeTarget === "gateway" ? (
            <WarpGateWayReleasePanel />
          ) : (
            <WistAgentdReleasePanel />
          )}
        </div>
      </section>

      <section className={styles.inventorySection}>
        <header className={styles.sectionHeader}>
          <div>
            <h2 className={styles.sectionTitle}>已部署版本</h2>
            <p className={styles.sectionSubtitle}>
              这些版本来自当前在线状态上报，不代表尚未部署的发布记录。
            </p>
          </div>
        </header>
        <div className={styles.inventoryGrid}>
          <section className={styles.inventoryCard}>
            <h3 className={styles.inventoryTitle}>Gateway</h3>
            {gateways.length === 0 ? (
              <div className={styles.empty}>暂无网关。</div>
            ) : (
              <div className={styles.table}>
                {gateways.map((gateway) => (
                  <div key={gateway.gatewayId} className={styles.row}>
                    <span className={styles.rowMain}>{gateway.gatewayId}</span>
                    <span className={styles.rowVersion}>
                      <GatewayVersionText value={gateway.version} />
                    </span>
                    <GatewayOnlineStatusBadge value={gateway.status} />
                  </div>
                ))}
              </div>
            )}
          </section>

          <section className={styles.inventoryCard}>
            <h3 className={styles.inventoryTitle}>Agent</h3>
            {agents.length === 0 ? (
              <div className={styles.empty}>暂无 Agent。</div>
            ) : (
              <div className={styles.table}>
                {agents.map((agent) => (
                  <div key={agent.agentId} className={styles.row}>
                    <span className={styles.rowMain}>
                      {agent.agentId}
                      <span className={styles.rowSub}>{agent.gatewayId}</span>
                    </span>
                    <span className={styles.rowVersion}>
                      <GatewayVersionText value={agent.version} />
                    </span>
                    <GatewayOnlineStatusBadge value={agent.status} />
                  </div>
                ))}
              </div>
            )}
          </section>
        </div>
      </section>
    </PageShell>
  );
}
