import { useState } from "react";
import styles from "./SubsystemUpgradeAgentRemotelyForm.module.css";
import type { SubsystemAdminUpgradeAgentRequested } from "../types";

interface SubsystemUpgradeAgentRemotelyFormProps {
  onSubsystemAdminUpgradeAgentRequested?: (payload: SubsystemAdminUpgradeAgentRequested) => void;
  submitting?: boolean;
  children?: React.ReactNode;
}

export function SubsystemUpgradeAgentRemotelyForm({
  onSubsystemAdminUpgradeAgentRequested,
  submitting,
}: SubsystemUpgradeAgentRemotelyFormProps) {
  const [agentId, setAgentId] = useState("agent-edge-014");
  const [targetVersion, setTargetVersion] = useState("v0.3.2");

  return (
    <form
      className={styles.container}
      onSubmit={(event) => {
        event.preventDefault();
        onSubsystemAdminUpgradeAgentRequested?.({
          agentId: agentId.trim(),
          targetVersion: targetVersion.trim(),
          requestedBy: "admin-operator",
        });
      }}
    >
      <label className={styles.field}>
        <span className={styles.label}>Agent ID</span>
        <input
          className={styles.input}
          value={agentId}
          onChange={(event) => setAgentId(event.target.value)}
        />
      </label>
      <label className={styles.field}>
        <span className={styles.label}>目标版本</span>
        <input
          className={styles.input}
          value={targetVersion}
          onChange={(event) => setTargetVersion(event.target.value)}
        />
      </label>
      <button
        className={styles.button}
        type="submit"
        disabled={!agentId.trim() || !targetVersion.trim() || submitting}
      >
        {submitting ? "提交中" : "提交升级"}
      </button>
    </form>
  );
}
