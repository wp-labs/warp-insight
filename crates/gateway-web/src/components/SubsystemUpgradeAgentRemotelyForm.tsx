import { useEffect, useState, type FormEvent } from "react";
import styles from "./SubsystemUpgradeAgentRemotelyForm.module.css";
import { AgentTargetField } from "./AgentTargetField";
import type { SubsystemAdminUpgradeAgentRequested } from "../types";

interface SubsystemUpgradeAgentRemotelyFormProps {
  onSubsystemAdminUpgradeAgentRequested?: (
    payload: SubsystemAdminUpgradeAgentRequested,
  ) => void;
  submitting?: boolean;
  children?: React.ReactNode;
}

/** 远程升级会重启 Agent 进程，同样走两段式确认。 */
export function SubsystemUpgradeAgentRemotelyForm({
  onSubsystemAdminUpgradeAgentRequested,
  submitting,
}: SubsystemUpgradeAgentRemotelyFormProps) {
  const [agentId, setAgentId] = useState("");
  const [targetVersion, setTargetVersion] = useState("");
  const [confirming, setConfirming] = useState(false);

  useEffect(() => setConfirming(false), [agentId, targetVersion]);

  const canSubmit =
    Boolean(agentId.trim()) && Boolean(targetVersion.trim()) && !submitting;

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSubmit) return;
    if (!confirming) {
      setConfirming(true);
      return;
    }
    onSubsystemAdminUpgradeAgentRequested?.({
      agentId: agentId.trim(),
      targetVersion: targetVersion.trim(),
      requestedBy: "admin-operator",
    });
    setConfirming(false);
  }

  return (
    <form className={styles.container} onSubmit={handleSubmit}>
      <AgentTargetField value={agentId} onChange={setAgentId} />

      <label className={styles.field}>
        <span className={styles.label}>目标版本</span>
        <input
          className={styles.input}
          placeholder="v0.3.2"
          value={targetVersion}
          onChange={(event) => setTargetVersion(event.target.value)}
          required
        />
        <span className={styles.hint}>
          需与制品仓库中已发布的版本号一致，填写错误会派发失败并留回执。
        </span>
      </label>

      <div className={styles.actions}>
        <button
          className={confirming ? styles.buttonDanger : styles.button}
          type="submit"
          disabled={!canSubmit}
        >
          {submitting
            ? "提交中…"
            : confirming
              ? "确认升级到该版本"
              : "远程升级"}
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
            将向 {agentId.trim()} 派发升级到 {targetVersion.trim()} 的命令，
            进程会重启，采集短暂中断。
          </span>
        ) : (
          "升级会重启 Agent 进程，采集会短暂中断；失败时回执会保留错误原因。"
        )}
      </p>
    </form>
  );
}
