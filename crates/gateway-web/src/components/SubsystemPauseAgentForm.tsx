import { useEffect, useState, type FormEvent } from "react";
import styles from "./SubsystemPauseAgentForm.module.css";
import { AgentTargetField } from "./AgentTargetField";
import type { SubsystemAdminPauseAgentRequested } from "../types";

interface SubsystemPauseAgentFormProps {
  onSubsystemAdminPauseAgentRequested?: (
    payload: SubsystemAdminPauseAgentRequested,
  ) => void;
  submitting?: boolean;
  children?: React.ReactNode;
}

/**
 * 暂停是不可逆的运维动作，因此走两段式确认：
 * 第一次点击进入待确认态，第二次才真正派发，避免误触。
 */
export function SubsystemPauseAgentForm({
  onSubsystemAdminPauseAgentRequested,
  submitting,
}: SubsystemPauseAgentFormProps) {
  const [agentId, setAgentId] = useState("");
  const [confirming, setConfirming] = useState(false);

  // 目标变化后作废待确认态，避免"确认"落到另一台主机上。
  useEffect(() => setConfirming(false), [agentId]);

  const canSubmit = Boolean(agentId.trim()) && !submitting;

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSubmit) return;
    if (!confirming) {
      setConfirming(true);
      return;
    }
    onSubsystemAdminPauseAgentRequested?.({
      agentId: agentId.trim(),
      requestedBy: "admin-operator",
    });
    setConfirming(false);
  }

  return (
    <form className={styles.container} onSubmit={handleSubmit}>
      <AgentTargetField value={agentId} onChange={setAgentId} />

      <div className={styles.actions}>
        <button
          className={confirming ? styles.buttonDanger : styles.button}
          type="submit"
          disabled={!canSubmit}
        >
          {submitting ? "提交中…" : confirming ? "确认暂停该 Agent" : "暂停 Agent"}
        </button>
        {confirming ? (
          <button
            className={styles.buttonGhost}
            type="button"
            onClick={() => setConfirming(false)}
          >
            取消
          </button>
        ) : null}
      </div>

      <p className={styles.note}>
        {confirming ? (
          <span className={styles.noteWarn}>
            Agent 将停止上报指标与日志，需重新下发启动命令才能恢复。
          </span>
        ) : (
          "暂停后该主机的指标与日志采集会中断，请确认不是生产关键节点。"
        )}
      </p>
    </form>
  );
}
