import styles from "./SubsystemUpgradeAgentRemotelyUsecaseCard.module.css";
import { SubsystemUpgradeAgentRemotelyUsecaseMeta } from "./SubsystemUpgradeAgentRemotelyUsecaseMeta";
import { SubsystemUpgradeAgent } from "./SubsystemUpgradeAgent";
import { SubsystemDispatchReceipt } from "./SubsystemDispatchReceipt";
import { SubsystemUpgradeAgentRemotelyForm } from "./SubsystemUpgradeAgentRemotelyForm";
import { SubsystemUpgradeDispatchReceiptResult } from "./SubsystemUpgradeDispatchReceiptResult";
import type { SubsystemAdminUpgradeAgentRequested } from "../types";
import type { DispatchReceipt } from "../api";

interface SubsystemUpgradeAgentRemotelyUsecaseCardProps {
  onSubsystemAdminUpgradeAgentRequested?: (payload: SubsystemAdminUpgradeAgentRequested) => void;
  receipt?: DispatchReceipt;
  submitting?: boolean;
  children?: React.ReactNode;
}

export function SubsystemUpgradeAgentRemotelyUsecaseCard({ onSubsystemAdminUpgradeAgentRequested, receipt, submitting }: SubsystemUpgradeAgentRemotelyUsecaseCardProps) {
  return (
    <div className={styles.container}>
      <SubsystemUpgradeAgentRemotelyUsecaseMeta>
        <SubsystemUpgradeAgent />
        <SubsystemDispatchReceipt />
      </SubsystemUpgradeAgentRemotelyUsecaseMeta>
      <SubsystemUpgradeAgentRemotelyForm onSubsystemAdminUpgradeAgentRequested={onSubsystemAdminUpgradeAgentRequested} submitting={submitting} />
      <SubsystemUpgradeDispatchReceiptResult receipt={receipt} />
    </div>
  );
}
