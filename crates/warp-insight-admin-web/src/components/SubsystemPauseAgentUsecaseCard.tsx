import styles from "./SubsystemPauseAgentUsecaseCard.module.css";
import { SubsystemPauseAgentUsecaseMeta } from "./SubsystemPauseAgentUsecaseMeta";
import { SubsystemPauseAgent } from "./SubsystemPauseAgent";
import { SubsystemDispatchReceipt } from "./SubsystemDispatchReceipt";
import { SubsystemPauseAgentForm } from "./SubsystemPauseAgentForm";
import { SubsystemPauseDispatchReceiptResult } from "./SubsystemPauseDispatchReceiptResult";
import type { SubsystemAdminPauseAgentRequested } from "../types";
import type { DispatchReceipt } from "../api";

interface SubsystemPauseAgentUsecaseCardProps {
  onSubsystemAdminPauseAgentRequested?: (payload: SubsystemAdminPauseAgentRequested) => void;
  receipt?: DispatchReceipt;
  submitting?: boolean;
  children?: React.ReactNode;
}

export function SubsystemPauseAgentUsecaseCard({ onSubsystemAdminPauseAgentRequested, receipt, submitting }: SubsystemPauseAgentUsecaseCardProps) {
  return (
    <div className={styles.container}>
      <SubsystemPauseAgentUsecaseMeta>
        <SubsystemPauseAgent />
        <SubsystemDispatchReceipt />
      </SubsystemPauseAgentUsecaseMeta>
      <SubsystemPauseAgentForm onSubsystemAdminPauseAgentRequested={onSubsystemAdminPauseAgentRequested} submitting={submitting} />
      <SubsystemPauseDispatchReceiptResult receipt={receipt} />
    </div>
  );
}
