import { MetricTile } from "./MetricTile";

interface SubsystemUnhealthyAgentMetricProps {
  value: number;
  hint?: string;
  loading?: boolean;
}

/**
 * 注意：网关当前固定返回 unhealthyAgents = 0
 * （见 crates/warp-gateway/src/api/overview.rs:110），异常判定尚未实现，
 * 所以这里不会因为 0 就断言"系统健康"。
 */
export function SubsystemUnhealthyAgentMetric({
  value,
  hint,
  loading,
}: SubsystemUnhealthyAgentMetricProps) {
  return (
    <MetricTile
      label="异常 Agent 数"
      value={String(value)}
      hint={hint ?? "由网关侧异常判定规则统计，当前规则尚未启用"}
      tone={value > 0 ? "crit" : "unknown"}
      loading={loading}
    />
  );
}
