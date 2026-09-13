import styles from "./SubsystemPauseAgentUsecaseMeta.module.css";

interface SubsystemPauseAgentUsecaseMetaProps {
  children?: React.ReactNode;
}

/**
 * 用例头部：一次说清"这个操作做什么、走什么回执通道"，
 * 具体动作与回执通道以行内标签呈现，避免三行标题堆叠。
 */
export function SubsystemPauseAgentUsecaseMeta({
  children,
}: SubsystemPauseAgentUsecaseMetaProps) {
  return (
    <div className={styles.container}>
      <div className={styles.head}>
        <h3 className={styles.title}>暂停采集</h3>
        <p className={styles.desc}>
          向单台 Agent 派发暂停命令，中断其指标与日志上报，等待人工恢复。
        </p>
      </div>
      <div className={styles.chips}>{children}</div>
    </div>
  );
}
