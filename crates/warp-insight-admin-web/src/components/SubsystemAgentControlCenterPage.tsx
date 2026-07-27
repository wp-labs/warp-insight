import styles from "./SubsystemAgentControlCenterPage.module.css";
import { SubsystemAdminTopNavigation } from "./SubsystemAdminTopNavigation";
import { SubsystemAdminOperatorLane } from "./SubsystemAdminOperatorLane";
import { SubsystemAdminOperator } from "./SubsystemAdminOperator";
import { SubsystemAgentControlUsecaseBoard } from "./SubsystemAgentControlUsecaseBoard";
import type { SubsystemAdminPauseAgentRequested, SubsystemAdminUpgradeAgentRequested } from "../types";
import { usePauseAgent, useUpgradeAgent } from "../hooks";

interface SubsystemAgentControlCenterPageProps {
  onSubsystemAdminPauseAgentRequested?: (payload: SubsystemAdminPauseAgentRequested) => void;
  onSubsystemAdminUpgradeAgentRequested?: (payload: SubsystemAdminUpgradeAgentRequested) => void;
  children?: React.ReactNode;
}

export function SubsystemAgentControlCenterPage({ onSubsystemAdminPauseAgentRequested, onSubsystemAdminUpgradeAgentRequested }: SubsystemAgentControlCenterPageProps) {
  const pauseMutation = usePauseAgent();
  const upgradeMutation = useUpgradeAgent();

  return (
    <div className={styles.container}>
      <SubsystemAdminTopNavigation />
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>Agent 控制中心</h1>
        <p className={styles.pageSummary}>面向已注册 Agent 派发常用运维控制操作。</p>
      </header>
      <SubsystemAdminOperatorLane>
        <SubsystemAdminOperator />
      </SubsystemAdminOperatorLane>
      <SubsystemAgentControlUsecaseBoard
        onSubsystemAdminPauseAgentRequested={(payload) => {
          onSubsystemAdminPauseAgentRequested?.(payload);
          pauseMutation.mutate(payload);
        }}
        onSubsystemAdminUpgradeAgentRequested={(payload) => {
          onSubsystemAdminUpgradeAgentRequested?.(payload);
          upgradeMutation.mutate(payload);
        }}
        pauseReceipt={pauseMutation.data}
        upgradeReceipt={upgradeMutation.data}
        pauseSubmitting={pauseMutation.isPending}
        upgradeSubmitting={upgradeMutation.isPending}
      />
    </div>
  );
}
