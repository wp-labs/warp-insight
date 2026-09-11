# warp-agentd 设计文档

warp-agentd（edge daemon）实现时最重要的设计文档，与全仓设计索引
[`doc/design`](../../../doc/design/README.md) 对应。

> **来源约定**：agentd 专属设计文档已从 `doc/design/{edge,telemetry}` **迁入本目录**
> （crate 就近阅读入口）；`doc/design` 只保留跨端共享设计，不再维护这些文档的另一份副本。
> 全仓索引见 [`doc/design/README.md`](../../../doc/design/README.md)。

## 阅读顺序建议

0. [development-plan.md](./development-plan.md) — 开发计划（当前差距 → 批次落地 → 验收标准）
1. [agentd-architecture.md](./agentd-architecture.md) — daemon 总体架构与边界
2. [agentd-state-and-boundaries.md](./agentd-state-and-boundaries.md) — 状态与边界
3. [agentd-state-schema.md](./agentd-state-schema.md) — 本地状态 schema
4. [agentd-events.md](./agentd-events.md) — 运行时事件
5. [agentd-failure-handling.md](./agentd-failure-handling.md) — 故障处理
6. [agentd-exec-protocol.md](./agentd-exec-protocol.md) — 本地执行协议（配合 `src/exec/`）
7. [agent-config-schema.md](./agent-config-schema.md) — 配置 schema（配合 `src/config/`）
8. [self-observability.md](./self-observability.md) — 自观测（配合 `src/runtime/self_observability.rs`）
9. [capability-report-schema.md](./capability-report-schema.md) — 能力上报（配合 `src/control/`）

## 日志 / telemetry 采集（配合 `src/telemetry/logs/files/`）

- [log-file-input-spec.md](./log-file-input-spec.md) — 文件日志输入规格（checkpoint/tail/rotate）
- [log-file-state-schema.md](./log-file-state-schema.md) — 文件日志 checkpoint 状态 schema
- [macos-agent-uplink-to-warp-parse.md](./macos-agent-uplink-to-warp-parse.md) — macOS P0 采集端到端方案
- [macos-security-audit-log-sources.md](./macos-security-audit-log-sources.md) — macOS 安全/审计日志源清单

## 模块对照（src 目录化后）

| 域 | 相关文档 |
|---|---|
| `src/bootstrap` | agentd-state-and-boundaries |
| `src/config` | agent-config-schema |
| `src/control` | capability-report-schema、agentd-events（gateway 侧） |
| `src/exec` | agentd-exec-protocol、agentd-failure-handling |
| `src/runtime` | agentd-architecture、self-observability |
| `src/reporting` | capability-report-schema（上报侧） |
| `src/telemetry` | log-file-input-spec、log-file-state-schema、macos-* |
