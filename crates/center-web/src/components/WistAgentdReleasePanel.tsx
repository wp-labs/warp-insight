import type { FormEvent } from "react";
import { usePublishWistAgentd, useReleases } from "../hooks";
import {
  Badge,
  ErrorBanner,
  FormField,
  PrimaryButton,
  ReceiptCard,
  SectionCard,
  TextInput,
  formatDateTime,
} from "./ui";
import styles from "./ReleasePanel.module.css";

/** 发布 WistAgentd，并在同一工作区查看该组件最近的发布记录。 */
export function WistAgentdReleasePanel() {
  const mutation = usePublishWistAgentd();
  const { data: releasesData } = useReleases("wist-agentd");
  const releases = releasesData?.data ?? [];

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    mutation.mutate({
      version: String(data.get("version") ?? ""),
      artifactUrl: String(data.get("artifactUrl") ?? ""),
      // 当前管理端没有独立发布者字段，沿用默认管理身份满足后端审计契约。
      requestedBy: "admin",
    });
  }

  const release = mutation.data?.data;

  return (
    <SectionCard
      title="发布 WistAgentd"
      subtitle="发布 Agent 服务新版本，供各 WarpGateWay 实例拉取升级。"
    >
      <form className={styles.form} onSubmit={handleSubmit}>
        <FormField label="版本号" hint="例如：v2.4.1">
          <TextInput name="version" required placeholder="请输入版本号" />
        </FormField>
        <FormField label="产物地址（artifact_url）">
          <TextInput
            name="artifactUrl"
            required
            placeholder="https://artifacts.example.com/wist-agentd/v2.4.1"
          />
        </FormField>
        <div className={styles.formAction}>
          <span className={styles.actionHint}>发布后将进入发布记录。</span>
          <PrimaryButton type="submit" disabled={mutation.isPending}>
            {mutation.isPending ? "发布中…" : "发布版本"}
          </PrimaryButton>
        </div>
      </form>
      {mutation.error ? (
        <ErrorBanner>发布失败：{String(mutation.error)}</ErrorBanner>
      ) : null}
      {release ? (
        <ReceiptCard
          title="本次发布回执"
          fields={[
            ["版本", release.version],
            ["状态", release.status],
            ["产物地址", release.artifactUrl],
            ["发布时间", formatDateTime(release.publishedAt)],
          ]}
        />
      ) : null}
      <div className={styles.history}>
        <div className={styles.historyHeader}>
          <h3 className={styles.historyTitle}>最近发布</h3>
          <span className={styles.historyHint}>WistAgentd</span>
        </div>
        {releases.length === 0 ? (
          <div className={styles.historyEmpty}>暂无发布记录。</div>
        ) : (
          <div className={styles.historyList}>
            {releases.map((record) => (
              <div key={record.version} className={styles.historyItem}>
                <strong className={styles.historyVersion}>
                  {record.version}
                </strong>
                <a
                  className={styles.artifactLink}
                  href={record.artifactUrl}
                  target="_blank"
                  rel="noreferrer"
                >
                  {record.artifactUrl}
                </a>
                <span className={styles.historyMeta}>
                  <Badge
                    tone={record.status === "published" ? "green" : "gray"}
                  >
                    {record.status}
                  </Badge>
                  {formatDateTime(record.publishedAt)}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </SectionCard>
  );
}
