import { useState, type FormEvent } from "react";
import type { UpgradeStep, UpgradeTarget } from "../api";
import {
  useCreateUpgradePlan,
  useGatewayStatusView,
  useReleases,
} from "../hooks";
import {
  ErrorBanner,
  PrimaryButton,
  ReceiptCard,
  SectionCard,
  formatDateTime,
} from "./ui";
import styles from "./UpgradePlanCreatePanel.module.css";

const COMPONENTS = ["wist-agentd", "warp-gateway"] as const;

/** 创建升级计划：多组件目标版本 + Gateway 范围多选 + 分批执行步骤（滚动升级）。 */
export function UpgradePlanCreatePanel() {
  const mutation = useCreateUpgradePlan();
  const { data: statusData } = useGatewayStatusView();
  const gateways = statusData?.data ?? [];
  const { data: agentdReleases } = useReleases("wist-agentd");
  const { data: gatewayReleases } = useReleases("warp-gateway");

  // 目标版本从已发布版本中选取（下拉）。
  function versionsFor(component: string): string[] {
    const releases =
      component === "warp-gateway" ? gatewayReleases : agentdReleases;
    return releases?.data?.map((release) => release.version) ?? [];
  }

  const [targets, setTargets] = useState<UpgradeTarget[]>([
    { component: "wist-agentd", targetVersion: "" },
  ]);
  const [selected, setSelected] = useState<string[]>([]);
  const [steps, setSteps] = useState<UpgradeStep[]>([]);

  function updateTarget(index: number, patch: Partial<UpgradeTarget>) {
    setTargets((prev) =>
      prev.map((target, i) => (i === index ? { ...target, ...patch } : target)),
    );
  }
  function addTarget() {
    setTargets((prev) => [
      ...prev,
      { component: "wist-agentd", targetVersion: "" },
    ]);
  }
  function removeTarget(index: number) {
    setTargets((prev) => prev.filter((_, i) => i !== index));
  }
  function toggleGateway(id: string) {
    setSelected((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );
  }
  function toggleStepGateway(stepIndex: number, id: string) {
    setSteps((prev) =>
      prev.map((step, i) =>
        i === stepIndex
          ? {
              ...step,
              gatewayIds: step.gatewayIds.includes(id)
                ? step.gatewayIds.filter((x) => x !== id)
                : [...step.gatewayIds, id],
            }
          : step,
      ),
    );
  }
  function addStep() {
    setSteps((prev) => [
      ...prev,
      { stepIndex: prev.length, gatewayIds: [], status: "pending" },
    ]);
  }
  function removeStep(index: number) {
    setSteps((prev) =>
      prev
        .filter((_, i) => i !== index)
        .map((step, i) => ({ ...step, stepIndex: i })),
    );
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    mutation.mutate({
      targets,
      gatewayIds: selected,
      steps: steps.map((step, index) => ({ ...step, stepIndex: index })),
      requestedBy: "admin",
    });
  }

  const plan = mutation.data?.data;

  return (
    <SectionCard
      title="创建升级计划"
      subtitle="选择多组件目标版本、网关范围与分批执行步骤，用于滚动升级。"
    >
      <form onSubmit={handleSubmit}>
        <div className={styles.block}>
          <div className={styles.blockHeader}>
            <span className={styles.blockTitle}>升级目标（可多选）</span>
            <button type="button" className={styles.addButton} onClick={addTarget}>
              + 添加目标
            </button>
          </div>
          {targets.map((target, index) => (
            <div key={index} className={styles.targetRow}>
              <select
                className={styles.input}
                value={target.component}
                onChange={(e) =>
                  updateTarget(index, { component: e.target.value })
                }
              >
                {COMPONENTS.map((component) => (
                  <option key={component} value={component}>
                    {component}
                  </option>
                ))}
              </select>
              <select
                className={styles.input}
                value={target.targetVersion}
                onChange={(e) =>
                  updateTarget(index, { targetVersion: e.target.value })
                }
              >
                <option value="">{versionsFor(target.component).length === 0 ? "该组件暂无已发布版本" : "请选择已发布版本"}</option>
                {versionsFor(target.component).map((version) => (
                  <option key={version} value={version}>
                    {version}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className={styles.removeButton}
                onClick={() => removeTarget(index)}
                disabled={targets.length <= 1}
              >
                删除
              </button>
            </div>
          ))}
        </div>

        <div className={styles.block}>
          <div className={styles.blockHeader}>
            <span className={styles.blockTitle}>
              Gateway 范围（{selected.length} 已选）
            </span>
          </div>
          {gateways.length === 0 ? (
            <div className={styles.empty}>暂无网关。</div>
          ) : (
            <div className={styles.gatewayGrid}>
              {gateways.map((gateway) => (
                <label key={gateway.gatewayId} className={styles.checkbox}>
                  <input
                    type="checkbox"
                    checked={selected.includes(gateway.gatewayId)}
                    onChange={() => toggleGateway(gateway.gatewayId)}
                  />
                  <span>
                    {gateway.gatewayId}
                    <span className={styles.checkboxSub}>{gateway.version}</span>
                  </span>
                </label>
              ))}
            </div>
          )}
        </div>

        <div className={styles.block}>
          <div className={styles.blockHeader}>
            <span className={styles.blockTitle}>执行步骤（分批滚动）</span>
            <button type="button" className={styles.addButton} onClick={addStep}>
              + 添加步骤
            </button>
          </div>
          {steps.length === 0 ? (
            <div className={styles.empty}>
              尚未配置执行步骤；添加步骤并勾选本批升级的网关。
            </div>
          ) : (
            steps.map((step, index) => (
              <div key={index} className={styles.stepBlock}>
                <div className={styles.stepHeader}>
                  <strong>步骤 {index + 1}</strong>
                  <button
                    type="button"
                    className={styles.removeButton}
                    onClick={() => removeStep(index)}
                  >
                    删除
                  </button>
                </div>
                <div className={styles.gatewayGrid}>
                  {selected.map((id) => (
                    <label key={id} className={styles.checkbox}>
                      <input
                        type="checkbox"
                        checked={step.gatewayIds.includes(id)}
                        onChange={() => toggleStepGateway(index, id)}
                      />
                      <span>{id}</span>
                    </label>
                  ))}
                </div>
              </div>
            ))
          )}
        </div>

        <div className={styles.formAction}>
          <PrimaryButton
            type="submit"
            disabled={mutation.isPending || selected.length === 0}
          >
            {mutation.isPending ? "创建中…" : "创建升级计划"}
          </PrimaryButton>
        </div>
      </form>
      {mutation.error ? (
        <ErrorBanner>创建失败：{String(mutation.error)}</ErrorBanner>
      ) : null}
      {plan ? (
        <ReceiptCard
          title="升级计划回执"
          fields={[
            ["计划 ID", plan.planId],
            [
              "目标",
              plan.targets
                .map((t) => `${t.component} ${t.targetVersion}`)
                .join("，"),
            ],
            ["升级范围", `${plan.targetCount} 个网关`],
            ["执行步骤", `${plan.steps.length} 步`],
            ["状态", plan.status],
            ["创建时间", formatDateTime(plan.createdAt)],
          ]}
        />
      ) : null}
    </SectionCard>
  );
}
