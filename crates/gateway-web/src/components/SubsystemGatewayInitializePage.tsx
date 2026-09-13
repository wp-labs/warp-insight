import { useState, type FormEvent } from "react";
import {
  ApiError,
  GatewayAlreadyInitializedError,
  GatewayInitializationInputError,
  type GatewayInitialConfig,
} from "../api";
import { useGatewayInitialConfig } from "../hooks";
import { CopyButton } from "./CopyButton";
import styles from "./SubsystemGatewayInitializePage.module.css";

function errorMessage(error: unknown): string {
  if (error instanceof GatewayInitializationInputError) return error.message;
  if (error instanceof GatewayAlreadyInitializedError) {
    return `该 Gateway 已进入 ${error.status.lifecycle_state} 状态，不能重复初始化。`;
  }
  if (error instanceof ApiError) {
    if (error.status === 401)
      return "Bearer 凭证缺失、无效或已失效：请填写 Center 交付的初始化凭证，或重新生成初始化材料。";
    if (error.status === 429) return "认证失败次数过多，请稍后再试。";
    if (error.status === 409)
      return "该 Gateway 已初始化，不能重复消费初始化材料。";
    return `控制中心返回 HTTP ${error.status}，请核对初始化 URL、instance_id 和服务状态。`;
  }
  if (error instanceof TypeError) {
    return "无法访问控制中心。若 Gateway 页面与 Center 不同源，请检查网络和 CORS 配置。";
  }
  return "初始化配置响应不符合当前契约，请检查 Center 与 Gateway 版本。";
}

const STEPS = [
  "粘贴 Center 交付的初始化 URL（必须带 instance_id）",
  "如 Center 要求鉴权，填入随附的 Bearer 凭证",
  "获取初始配置 JSON，交给 Gateway 落地",
];

/** 展示 Center 返回的 GatewayInitialConfig JSON，并保留完整对象供复制核对。 */
function ConfigResult({ config }: { config: GatewayInitialConfig }) {
  const serializedConfig = JSON.stringify(config, null, 2);
  return (
    <section
      className={styles.result}
      aria-live="polite"
      aria-labelledby="gateway-initial-config-result"
    >
      <header className={styles.resultHeader}>
        <div>
          <div className={styles.resultEyebrow}>网关初始配置</div>
          <h2 id="gateway-initial-config-result" className={styles.sectionTitle}>
            已获取配置内容
          </h2>
        </div>
        <span className={styles.successBadge}>获取成功</span>
      </header>
      <div className={styles.jsonBlock}>
        <pre>{serializedConfig}</pre>
      </div>
      <CopyButton
        text={serializedConfig}
        label="复制 JSON 配置"
        className={styles.secondaryButton}
      />
    </section>
  );
}

/**
 * Gateway 自身初始化页：消费 Center 创建实例时交付的 init_url，
 * 提交时先查询 Center 侧实例状态，未初始化才获取并展示 JSON Gateway 初始配置；
 * Bearer 只由请求层放入 Authorization Header，不处理 Agent 安装配置。
 */
export function SubsystemGatewayInitializePage() {
  const [initUrl, setInitUrl] = useState("");
  const [token, setToken] = useState("");
  const initialConfig = useGatewayInitialConfig();
  const canSubmit = initUrl.trim().length > 0;

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSubmit) return;
    initialConfig.mutate({ initUrl, token: token.trim() || undefined });
  }

  return (
    <div className={styles.container}>
      <main className={styles.main}>
        <header className={styles.pageHeader}>
          <h1 className={styles.pageTitle}>初始化 Gateway</h1>
          <p className={styles.pageSummary}>
            这是新装 Gateway 接入控制中心的唯一步骤。初始化材料由 Center
            在创建实例时交付，且只能消费一次。
          </p>
          <ol className={styles.steps}>
            {STEPS.map((step, index) => (
              <li key={step} className={styles.step}>
                <span className={styles.stepIndex} aria-hidden="true">
                  {index + 1}
                </span>
                <span className={styles.stepText}>{step}</span>
              </li>
            ))}
          </ol>
        </header>

        <section
          className={styles.usecaseCard}
          aria-labelledby="gateway-initialize-usecase"
        >
          <header className={styles.usecaseMeta}>
            <div className={styles.usecaseMetaCopy}>
              <span className={styles.usecaseTag}>一次性操作</span>
              <h2 id="gateway-initialize-usecase">填入初始化材料</h2>
              <p>
                提交后先查询 Center 侧的实例状态；只有状态为未初始化时才会返回配置，
                重复提交会被拒绝。
              </p>
            </div>
          </header>

          <form className={styles.form} onSubmit={handleSubmit}>
            <label className={`${styles.field} ${styles.urlField}`}>
              <span>控制中心初始化 URL</span>
              <input
                type="url"
                value={initUrl}
                onChange={(event) => setInitUrl(event.target.value)}
                placeholder="https://center.example/api/v1/gateway/initial-config?instance_id=gw-demo"
                autoComplete="url"
                required
              />
              <small>
                使用 Center 创建实例时交付的 init_url，必须包含 instance_id。
              </small>
            </label>
            <label className={styles.field}>
              <span>网关注册凭证（可选）</span>
              <input
                type="password"
                value={token}
                onChange={(event) => setToken(event.target.value)}
                placeholder="Center 交付的 Bearer 初始化凭证"
                autoComplete="off"
              />
              <small>
                凭证不进 URL，仅用于 Authorization Header 调 init_url；Center
                未要求鉴权时可留空。
              </small>
            </label>
            {initialConfig.isError ? (
              <div className={styles.errorBanner} role="alert">
                {errorMessage(initialConfig.error)}
              </div>
            ) : null}
            <div className={styles.formActions}>
              <button
                type="submit"
                className={styles.primaryButton}
                disabled={!canSubmit || initialConfig.isPending}
              >
                {initialConfig.isPending ? "正在获取…" : "获取初始配置"}
              </button>
              <span className={styles.actionHint}>
                初始化材料只能消费一次，请确认 URL 与实例一致后再提交。
              </span>
            </div>
          </form>
        </section>

        {initialConfig.data ? (
          <>
            <div className={styles.statusBanner} role="status">
              状态检查通过：实例原状态为{" "}
              {initialConfig.data.status.lifecycle_state}，已取回网关初始配置。
            </div>
            <ConfigResult config={initialConfig.data.config} />
          </>
        ) : null}
      </main>
    </div>
  );
}
