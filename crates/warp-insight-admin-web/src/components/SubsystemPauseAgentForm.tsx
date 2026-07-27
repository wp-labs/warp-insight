import { useState } from "react";
import styles from "./SubsystemPauseAgentForm.module.css";
import type { SubsystemAdminPauseAgentRequested } from "../types";

interface SubsystemPauseAgentFormProps {
  onSubsystemAdminPauseAgentRequested?: (payload: SubsystemAdminPauseAgentRequested) => void;
  submitting?: boolean;
  children?: React.ReactNode;
}

export function SubsystemPauseAgentForm({
  onSubsystemAdminPauseAgentRequested,
  submitting,
}: SubsystemPauseAgentFormProps) {
  const [agentId, setAgentId] = useState("agent-prod-001");

  return (
    <form
      className={styles.container}
      onSubmit={(event) => {
        event.preventDefault();
        onSubsystemAdminPauseAgentRequested?.({
          agentId: agentId.trim(),
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
      <button className={styles.button} type="submit" disabled={!agentId.trim() || submitting}>
        {submitting ? "提交中" : "提交暂停"}
      </button>
    </form>
  );
}
