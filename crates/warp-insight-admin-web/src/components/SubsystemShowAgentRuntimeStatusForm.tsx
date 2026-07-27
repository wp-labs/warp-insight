import styles from "./SubsystemShowAgentRuntimeStatusForm.module.css";
import { SubsystemRuntimeStatusAgentInput } from "./SubsystemRuntimeStatusAgentInput";
import { SubsystemAgentId } from "./SubsystemAgentId";
import { SubsystemRefreshRuntimeStatusAction } from "./SubsystemRefreshRuntimeStatusAction";
import { SubsystemAdminShowAgentRuntimeStatus } from "./SubsystemAdminShowAgentRuntimeStatus";
import type { SubsystemAdminShowAgentRuntimeStatusRequested } from "../types";

interface SubsystemShowAgentRuntimeStatusFormProps {
  onSubsystemAdminShowAgentRuntimeStatusRequested?: (payload: SubsystemAdminShowAgentRuntimeStatusRequested) => void;
  children?: React.ReactNode;
}

export function SubsystemShowAgentRuntimeStatusForm({ onSubsystemAdminShowAgentRuntimeStatusRequested }: SubsystemShowAgentRuntimeStatusFormProps) {
  return (
    <div className={styles.container}
        onClick={() => onSubsystemAdminShowAgentRuntimeStatusRequested?.({ agentId: "", requestedBy: "" } as unknown as SubsystemAdminShowAgentRuntimeStatusRequested)}>
      <SubsystemRuntimeStatusAgentInput>
        <SubsystemAgentId />
      </SubsystemRuntimeStatusAgentInput>
      <SubsystemRefreshRuntimeStatusAction>
        <SubsystemAdminShowAgentRuntimeStatus />
      </SubsystemRefreshRuntimeStatusAction>
    </div>
  );
}
