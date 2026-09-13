import type { FormEvent } from "react";
import { useApproveUpgradePlan } from "../hooks";
import {
  ErrorBanner,
  FormField,
  FormStack,
  PrimaryButton,
  ReceiptCard,
  SectionCard,
  TextInput,
  formatDateTime,
} from "./ui";

export function UpgradePlanApprovePanel() {
  const mutation = useApproveUpgradePlan();

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    mutation.mutate({
      planId: String(data.get("planId") ?? ""),
      approvedBy: String(data.get("approvedBy") ?? ""),
    });
  }

  const approval = mutation.data?.data;

  return (
    <SectionCard
      title="批准升级计划"
      subtitle="客户服务工程师批准升级计划，同意对指定范围内的 WistAgentd / WarpGateWay 执行升级。"
    >
      <form onSubmit={handleSubmit}>
        <FormStack
          actions={
            <PrimaryButton type="submit" disabled={mutation.isPending}>
              {mutation.isPending ? "批准中…" : "批准计划"}
            </PrimaryButton>
          }
        >
          <FormField label="计划 ID" hint="例如：plan-3f2a">
            <TextInput name="planId" required placeholder="请输入计划 ID" />
          </FormField>
          <FormField label="批准人（approved_by）">
            <TextInput name="approvedBy" defaultValue="admin" required />
          </FormField>
        </FormStack>
      </form>
      {mutation.error ? (
        <ErrorBanner>批准失败：{String(mutation.error)}</ErrorBanner>
      ) : null}
      {approval ? (
        <ReceiptCard
          title="批准回执"
          fields={[
            ["计划 ID", approval.planId],
            ["状态", approval.status],
            ["批准人", approval.approvedBy],
            ["批准时间", formatDateTime(approval.approvedAt)],
          ]}
        />
      ) : null}
    </SectionCard>
  );
}
