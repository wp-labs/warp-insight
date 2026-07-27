import styles from "./SubsystemDispatchReceipt.module.css";

interface SubsystemDispatchReceiptProps {
  children?: React.ReactNode;
}

export function SubsystemDispatchReceipt({ children }: SubsystemDispatchReceiptProps) {
  return (
    <div className={styles.container}>
      <div className={styles.label}>派发回执</div>
      {children}
    </div>
  );
}
