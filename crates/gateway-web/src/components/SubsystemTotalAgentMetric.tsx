import { MetricTile } from "./MetricTile";

interface SubsystemTotalAgentMetricProps {
  value: number;
  hint?: string;
  loading?: boolean;
}

/**
 * 注意：网关返回的 totalAgents 取自最近上线列表并截断到 6 条
 * （见 crates/warp-gateway/src/api/overview.rs:97 truncate(6)），
 * 因此它不是集群真实主机总数——文案按实际口径描述。
 */
export function SubsystemTotalAgentMetric({
  value,
  hint,
  loading,
}: SubsystemTotalAgentMetricProps) {
  return (
    <MetricTile
      label="已接入 Agent"
      value={String(value)}
      hint={hint ?? "网关最近上线列表中的 Agent 数量（网关侧上限 6 条）"}
      tone="accent"
      loading={loading}
    />
  );
}
