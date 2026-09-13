import styles from "./SubsystemAgentControlUsecaseBoard.module.css";
import { SubsystemPauseAgentUsecaseCard } from "./SubsystemPauseAgentUsecaseCard";
import { SubsystemUpgradeAgentRemotelyUsecaseCard } from "./SubsystemUpgradeAgentRemotelyUsecaseCard";
import type {
  SubsystemAdminPauseAgentRequested,
  SubsystemAdminUpgradeAgentRequested,
} from "../types";
import type { DispatchReceipt } from "../api";

interface SubsystemAgentControlUsecaseBoardProps {
  onSubsystemAdminPauseAgentRequested?: (
    payload: SubsystemAdminPauseAgentRequested,
  ) => void;
  onSubsystemAdminUpgradeAgentRequested?: (
    payload: SubsystemAdminUpgradeAgentRequested,
  ) => void;
  pauseReceipt?: DispatchReceipt;
  upgradeReceipt?: DispatchReceipt;
  pauseError?: unknown;
  upgradeError?: unknown;
  pauseSubmitting?: boolean;
  upgradeSubmitting?: boolean;
  children?: React.ReactNode;
}

export function SubsystemAgentControlUsecaseBoard({
  onSubsystemAdminPauseAgentRequested,
  onSubsystemAdminUpgradeAgentRequested,
  pauseReceipt,
  upgradeReceipt,
  pauseError,
  upgradeError,
  pauseSubmitting,
  upgradeSubmitting,
}: SubsystemAgentControlUsecaseBoardProps) {
  return (
    <div className={styles.container}>
      <SubsystemPauseAgentUsecaseCard
        onSubsystemAdminPauseAgentRequested={onSubsystemAdminPauseAgentRequested}
        receipt={pauseReceipt}
        error={pauseError}
        submitting={pauseSubmitting}
      />
      <SubsystemUpgradeAgentRemotelyUsecaseCard
        onSubsystemAdminUpgradeAgentRequested={onSubsystemAdminUpgradeAgentRequested}
        receipt={upgradeReceipt}
        error={upgradeError}
        submitting={upgradeSubmitting}
      />
    </div>
  );
}
