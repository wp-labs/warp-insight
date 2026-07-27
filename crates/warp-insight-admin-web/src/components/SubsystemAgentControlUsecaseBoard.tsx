import styles from "./SubsystemAgentControlUsecaseBoard.module.css";
import { SubsystemPauseAgentUsecaseCard } from "./SubsystemPauseAgentUsecaseCard";
import { SubsystemPauseAgent } from "./SubsystemPauseAgent";
import { SubsystemUpgradeAgentRemotelyUsecaseCard } from "./SubsystemUpgradeAgentRemotelyUsecaseCard";
import { SubsystemUpgradeAgentRemotely } from "./SubsystemUpgradeAgentRemotely";
import type { SubsystemAdminPauseAgentRequested, SubsystemAdminUpgradeAgentRequested } from "../types";
import type { DispatchReceipt } from "../api";

interface SubsystemAgentControlUsecaseBoardProps {
  onSubsystemAdminPauseAgentRequested?: (payload: SubsystemAdminPauseAgentRequested) => void;
  onSubsystemAdminUpgradeAgentRequested?: (payload: SubsystemAdminUpgradeAgentRequested) => void;
  pauseReceipt?: DispatchReceipt;
  upgradeReceipt?: DispatchReceipt;
  pauseSubmitting?: boolean;
  upgradeSubmitting?: boolean;
  children?: React.ReactNode;
}

export function SubsystemAgentControlUsecaseBoard({
  onSubsystemAdminPauseAgentRequested,
  onSubsystemAdminUpgradeAgentRequested,
  pauseReceipt,
  upgradeReceipt,
  pauseSubmitting,
  upgradeSubmitting,
}: SubsystemAgentControlUsecaseBoardProps) {
  return (
    <div className={styles.container}>
      <SubsystemPauseAgentUsecaseCard
        onSubsystemAdminPauseAgentRequested={onSubsystemAdminPauseAgentRequested}
        receipt={pauseReceipt}
        submitting={pauseSubmitting}
      >
        <SubsystemPauseAgent />
      </SubsystemPauseAgentUsecaseCard>
      <SubsystemUpgradeAgentRemotelyUsecaseCard
        onSubsystemAdminUpgradeAgentRequested={onSubsystemAdminUpgradeAgentRequested}
        receipt={upgradeReceipt}
        submitting={upgradeSubmitting}
      >
        <SubsystemUpgradeAgentRemotely />
      </SubsystemUpgradeAgentRemotelyUsecaseCard>
    </div>
  );
}
