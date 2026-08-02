import styles from "./SubsystemBootstrapTokenCard.module.css";

interface SubsystemBootstrapTokenCardProps {
  token?: string;
  loading?: boolean;
}

export function SubsystemBootstrapTokenCard({
  token,
  loading,
}: SubsystemBootstrapTokenCardProps) {
  const display = loading
    ? "加载中..."
    : (token ?? "暂无（请先在上方输入 Admin Token）");

  return (
    <section className={styles.container}>
      <div className={styles.heading}>
        <h2 className={styles.title}>Bootstrap Token（新 Agent 注册凭证）</h2>
        <p className={styles.desc}>
          一次性凭证，让目标主机上的 agent 加入集群；安装完成后即作废。在目标主机上运行安装命令时，通过环境变量注入，或按提示粘贴。
        </p>
      </div>
      <div className={styles.tokenRow}>
        <code className={styles.token}>{display}</code>
        <button
          type="button"
          className={styles.copyButton}
          disabled={!token}
          onClick={() => token && navigator.clipboard?.writeText(token)}
        >
          复制 Token
        </button>
      </div>
      <div className={styles.usage}>
        <div className={styles.usageTitle}>目标主机上运行安装命令，二选一提供 Token：</div>
        <div className={styles.option}>
          <span className={styles.optionTag}>方式 A</span>
          <span>环境变量注入：</span>
          <code className={styles.inlineCode}>
            WARP_INSIGHT_ENROLLMENT_TOKEN=&lt;Token&gt; &lt;安装命令&gt;
          </code>
        </div>
        <div className={styles.option}>
          <span className={styles.optionTag}>方式 B</span>
          <span>直接运行安装命令，在提示</span>
          <code className={styles.inlineCode}>Enrollment token:</code>
          <span>时粘贴。</span>
        </div>
      </div>
    </section>
  );
}
