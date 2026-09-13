import styles from "./SubsystemDispatchReceipt.module.css";

interface SubsystemDispatchReceiptProps {
  children?: React.ReactNode;
}

/** 回执通道标识：与动作标签同族的行内标签。 */
export function SubsystemDispatchReceipt({
  children,
}: SubsystemDispatchReceiptProps) {
  return (
    <span className={styles.chip}>
      <span className={styles.key}>回执</span>
      派发回执
      {children}
    </span>
  );
}
