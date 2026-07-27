import styles from "./SubsystemShowAgentRuntimeStatusUsecaseCard.module.css";
import { SubsystemShowAgentRuntimeStatusUsecaseMeta } from "./SubsystemShowAgentRuntimeStatusUsecaseMeta";
import { SubsystemShowAgentRuntimeStatus } from "./SubsystemShowAgentRuntimeStatus";
import { SubsystemAgentRuntimeStatusView } from "./SubsystemAgentRuntimeStatusView";
import { SubsystemShowAgentRuntimeStatusForm } from "./SubsystemShowAgentRuntimeStatusForm";
import { SubsystemAgentRuntimeStatusResult } from "./SubsystemAgentRuntimeStatusResult";
import type { SubsystemAdminShowAgentRuntimeStatusRequested } from "../types";

interface SubsystemShowAgentRuntimeStatusUsecaseCardProps {
  onSubsystemAdminShowAgentRuntimeStatusRequested?: (payload: SubsystemAdminShowAgentRuntimeStatusRequested) => void;
  children?: React.ReactNode;
}

export function SubsystemShowAgentRuntimeStatusUsecaseCard({ onSubsystemAdminShowAgentRuntimeStatusRequested }: SubsystemShowAgentRuntimeStatusUsecaseCardProps) {
  return (
    <div className={styles.container}>
      <SubsystemShowAgentRuntimeStatusUsecaseMeta>
        <SubsystemShowAgentRuntimeStatus />
        <SubsystemAgentRuntimeStatusView />
      </SubsystemShowAgentRuntimeStatusUsecaseMeta>
      <SubsystemShowAgentRuntimeStatusForm />
      <SubsystemAgentRuntimeStatusResult />
    </div>
  );
}
