import styles from "./SubsystemPauseDispatchReceiptResult.module.css";
import type { DispatchReceipt } from "../api";

interface SubsystemPauseDispatchReceiptResultProps {
  receipt?: DispatchReceipt;
  children?: React.ReactNode;
}

export function SubsystemPauseDispatchReceiptResult({ receipt }: SubsystemPauseDispatchReceiptResultProps) {
  if (!receipt) {
    return <div className={styles.empty}>暂无暂停派发回执</div>;
  }

  return (
    <div className={styles.container}>
      <div className={styles.row}>
        <span>派发 ID</span>
        <strong>{receipt.dispatchId}</strong>
      </div>
      <div className={styles.row}>
        <span>命令 ID</span>
        <strong>{receipt.commandId}</strong>
      </div>
      <div className={styles.row}>
        <span>状态</span>
        <strong>{receipt.status}</strong>
      </div>
      <div className={styles.row}>
        <span>创建时间</span>
        <strong>{new Date(receipt.createdAt).toLocaleString("zh-CN")}</strong>
      </div>
    </div>
  );
}
