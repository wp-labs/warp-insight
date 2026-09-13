import styles from "./SubsystemPauseDispatchReceiptResult.module.css";
import type { DispatchReceipt } from "../api";

interface SubsystemPauseDispatchReceiptResultProps {
  receipt?: DispatchReceipt;
  error?: unknown;
  children?: React.ReactNode;
}

export function SubsystemPauseDispatchReceiptResult({
  receipt,
  error,
}: SubsystemPauseDispatchReceiptResultProps) {
  if (error) {
    return (
      <div className={styles.errorBox}>
        <strong className={styles.errorTitle}>派发失败</strong>
        <span className={styles.errorText}>{describeError(error)}</span>
      </div>
    );
  }

  if (!receipt) {
    return (
      <div className={styles.empty}>尚未派发暂停命令，回执会显示在这里。</div>
    );
  }

  const accepted = receipt.status === "accepted";

  return (
    <div className={styles.container}>
      <div className={styles.statusRow}>
        <span
          className={
            accepted
              ? `${styles.status} ${styles.statusOk}`
              : `${styles.status} ${styles.statusCrit}`
          }
        >
          {accepted ? "已受理" : "已拒绝"}
        </span>
        <span className={styles.statusAgent}>{receipt.agentId}</span>
      </div>
      <dl className={styles.rows}>
        <div className={styles.row}>
          <dt>派发 ID</dt>
          <dd title={receipt.dispatchId}>{receipt.dispatchId}</dd>
        </div>
        <div className={styles.row}>
          <dt>命令 ID</dt>
          <dd title={receipt.commandId}>{receipt.commandId}</dd>
        </div>
        <div className={styles.row}>
          <dt>创建时间</dt>
          <dd>
            {new Intl.DateTimeFormat("zh-CN", {
              month: "2-digit",
              day: "2-digit",
              hour: "2-digit",
              minute: "2-digit",
              second: "2-digit",
              hour12: false,
            }).format(new Date(receipt.createdAt))}
          </dd>
        </div>
      </dl>
    </div>
  );
}

function describeError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
