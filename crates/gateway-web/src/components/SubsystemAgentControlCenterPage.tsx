import styles from "./SubsystemAgentControlCenterPage.module.css";
import { SubsystemAdminOperatorLane } from "./SubsystemAdminOperatorLane";
import { SubsystemAgentControlUsecaseBoard } from "./SubsystemAgentControlUsecaseBoard";
import type {
  SubsystemAdminPauseAgentRequested,
  SubsystemAdminUpgradeAgentRequested,
} from "../types";
import { usePauseAgent, useUpgradeAgent } from "../hooks";

interface SubsystemAgentControlCenterPageProps {
  onSubsystemAdminPauseAgentRequested?: (
    payload: SubsystemAdminPauseAgentRequested,
  ) => void;
  onSubsystemAdminUpgradeAgentRequested?: (
    payload: SubsystemAdminUpgradeAgentRequested,
  ) => void;
  children?: React.ReactNode;
}

export function SubsystemAgentControlCenterPage({
  onSubsystemAdminPauseAgentRequested,
  onSubsystemAdminUpgradeAgentRequested,
}: SubsystemAgentControlCenterPageProps) {
  const pauseMutation = usePauseAgent();
  const upgradeMutation = useUpgradeAgent();

  return (
    <div className={styles.container}>
      <header className={styles.pageHeader}>
        <h1 className={styles.pageTitle}>Agent 控制中心</h1>
        <p className={styles.pageSummary}>
          对已注册主机派发暂停采集与远程升级命令。每次派发都会返回一条派发回执，
          失败原因保留在回执中，便于排查。
        </p>
      </header>

      <SubsystemAdminOperatorLane />

      <SubsystemAgentControlUsecaseBoard
        onSubsystemAdminPauseAgentRequested={(payload) => {
          onSubsystemAdminPauseAgentRequested?.(payload);
          pauseMutation.reset();
          pauseMutation.mutate(payload);
        }}
        onSubsystemAdminUpgradeAgentRequested={(payload) => {
          onSubsystemAdminUpgradeAgentRequested?.(payload);
          upgradeMutation.reset();
          upgradeMutation.mutate(payload);
        }}
        pauseReceipt={pauseMutation.data}
        upgradeReceipt={upgradeMutation.data}
        pauseError={pauseMutation.isError ? pauseMutation.error : undefined}
        upgradeError={upgradeMutation.isError ? upgradeMutation.error : undefined}
        pauseSubmitting={pauseMutation.isPending}
        upgradeSubmitting={upgradeMutation.isPending}
      />
    </div>
  );
}
