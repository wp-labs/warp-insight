# warp-insight 错误处理体系设计

> 状态：**Target** — 定义 `warp-insight` 各 Rust crate、跨进程协议和对外暴露的统一错误处理目标态。

## 1. 文档目的

本文档定义 `warp-insight` 的错误处理体系，重点固定以下问题：

- Rust 代码中如何表达领域错误、上下文、source chain 和跨层转换
- 稳定错误码、协议字段和内部错误类型之间如何对应
- 哪些错误只影响单次 execution，哪些错误会导致 daemon 退化或失败
- 错误如何进入 action result、agent event、local state、gateway response、日志和指标
- 哪些信息可以对外暴露，哪些信息只能留在本地诊断或审计链路

相关文档：

- [`../edge/error-codes.md`](../edge/error-codes.md)：稳定错误码和原因码词典
- [`../../crates/wist-agentd/docs/agentd-failure-handling.md`](../../crates/wist-agentd/docs/agentd-failure-handling.md)：`warp-insightd` 故障分层、恢复和 health 口径
- [`../../crates/wist-agentd/docs/agentd-events.md`](../../crates/wist-agentd/docs/agentd-events.md)：daemon 进程内事件对象
- [`../../crates/wist-agentd/docs/agentd-state-schema.md`](../../crates/wist-agentd/docs/agentd-state-schema.md)：本地状态里的错误字段
- [`../execution/action-result-schema.md`](../execution/action-result-schema.md)：`ActionResult.exit_reason` 与 `StepActionRecord.error_code`
- [`../center/agent-gateway-protocol.md`](../center/agent-gateway-protocol.md)：中心与 agent 的南向协议
- [`security-model.md`](security-model.md)：权限、审批、审计和敏感信息边界

---

## 2. 核心结论

目标态固定以下结论：

1. Rust 内部错误统一使用 `orion-error` 的 `StructError<R>`。
2. 每个 crate 定义自己的领域 reason enum，并通过 `#[derive(OrionError)]` 暴露稳定 identity。
3. 领域 reason 默认使用 unit variant，动态诊断信息放入 `StructError` 的 detail、context、fields、metadata 或 source chain。
4. 热路径不得扩散 `anyhow::Error`、裸 `std::io::Error`、`Box<dyn Error>` 或字符串错误；这些错误必须在边界立即转换为 `StructError<R>`。
5. 跨 crate 的错误转换分两类：仅改变 reason 类型用 `conv_err()`；建立新的语义边界用 `source_err(...)` 或 `source_raw_err(...)`。
6. 协议字段继续使用稳定字符串码，例如 `exit_reason`、`error_code`、`reason_code`。这些字段不是 Rust enum 的 Debug 文本。
7. 稳定 external identity 使用 `ErrorIdentity.code`，并映射到协议错误码词典。
8. 对外暴露必须经过 protocol / exposure 层，不能直接把内部 detail、source error 或 backtrace 写入中心协议。
9. 错误处理必须服务于故障裁决：execution-local、runtime-degraded、fatal daemon failure 三层必须能从错误 reason 和 context 中判断。

---

## 3. 分层模型

错误体系分四层：

```text
Rust Domain Error
  StructError<Reason> + context + source chain
  负责本进程内传播、诊断、转换

Error Identity / Code
  ErrorIdentity.code + stable reason code
  负责稳定分类、幂等统计、协议投影

Protocol Projection
  ActionResult / AgentEvent / GatewayError / StateError
  负责跨进程与跨节点传输

Failure Decision
  execution-local / runtime-degraded / fatal daemon failure
  负责恢复、隔离、health 和告警
```

边界约束：

- Rust 层可以保留完整 source chain。
- protocol 层只能携带稳定码、简短 message、可暴露字段和 correlation id。
- failure decision 层不能依赖不稳定 detail 文本。
- 日志和审计可以记录更多诊断信息，但必须按安全模型脱敏。

---

## 4. Crate 级 reason 划分

每个 crate 拥有自己的 reason enum。跨 crate 不共享一个巨大错误枚举。

| crate | 建议 reason | 职责范围 |
|---|---|---|
| `wist-contracts` | `ContractReason` | schema 对象、serde 兼容、协议字段约束 |
| `wist-validate` | `ValidateReason` | ActionPlan、ActionResult、config、state 校验 |
| `wist-shared` | `SharedReason` | path、time、id、integrity、公共文件操作 |
| `warp-insightd` | `AgentdReason` | daemon bootstrap、config、state store、scheduler、reporting、telemetry runtime |
| `wist-exec` | `ExecReason` | workdir、runtime、opcode、result writer、子进程执行 |
| `wist-upgrader` | `UpgradeReason` | prepare、switch、rollback、health check |
| `wist-gateway` | `GatewayReason` | session、hello、heartbeat、dispatch、ack/result channel |
| `warp-insight-control` | `ControlReason` | request、approval、compile、sign、dispatch、tracking |

每个 reason enum 必须包含一个透明 `General(UnifiedReason)` variant，用于复用配置、IO、权限、系统、数据和校验等通用类别。
调用通用类别时优先使用 `#[derive(OrionError)]` 生成的 delegate constructor，例如 `AgentdReason::core_conf()` 或 `AgentdReason::system_error()`。

示例：

```rust
use derive_more::From;
use orion_error::prelude::*;

#[derive(Debug, Clone, PartialEq, From, OrionError)]
pub enum AgentdReason {
    #[orion_error(identity = "wi.agentd.config_invalid")]
    ConfigInvalid,
    #[orion_error(identity = "wi.agentd.state_unavailable")]
    StateUnavailable,
    #[orion_error(identity = "wi.agentd.execution_quarantined")]
    ExecutionQuarantined,
    #[orion_error(identity = "wi.agentd.reporting_backlog")]
    ReportingBacklog,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

pub type AgentdError = StructError<AgentdReason>;
```

实现约束：

- 新代码优先 `use orion_error::prelude::*;`。
- 单个 reason 变成错误时使用 `to_err()`。
- IO、serde、toml 等源错误进入结构化体系时优先用 `source_err(reason, detail)`。
- 第三方 `StdError` 没有 `UnstructuredSource` bridge 时用 `source_raw_err(reason, detail)`。
- 低层 `StructError<R1>` 只改 reason 类型时用 `conv_err()`。
- 热路径函数签名不得返回 `anyhow::Result<T>`、`Result<T, std::io::Error>`、`Result<T, Box<dyn std::error::Error>>` 或 `Result<T, String>`。
- 热路径中的 `map_err(|e| e.to_string())`、`format!("{e}")` 后直接作为错误返回属于违规；文本只能进入 `with_detail(...)` 或 protocol projection 的可暴露 message。

热路径包括：

- config loader
- state store
- scheduler / recovery
- local exec / result writer
- reporting pipeline
- gateway session / dispatch / ack / result channel
- control request compile / sign / dispatch

---

## 5. Reason 建模规则

### 5.1 允许的 reason 内容

Reason variant 表达稳定类别：

- `ConfigInvalid`
- `PlanRejected`
- `StateUnavailable`
- `ExecutionQuarantined`
- `ReportFailed`
- `SignatureInvalid`
- `GatewaySessionExpired`
- `UpgradeRollbackRequired`

Reason variant 不携带动态诊断 payload。

### 5.2 动态信息位置

动态信息放入 `StructError`：

| 信息 | 位置 |
|---|---|
| 人类可读补充 | `with_detail(...)` |
| 当前操作 | `OperationContext::doing(...)` |
| 稳定字段 | `with_field("execution_id", ...)` |
| 诊断 metadata | `with_meta("component.name", ...)` |
| 源错误 | `source_err(...)` / `source_raw_err(...)` / `with_source(...)` |

示例：

```rust
use orion_error::{prelude::*, runtime::OperationContext};

fn load_runtime_state(path: &std::path::Path) -> Result<String, AgentdError> {
    let ctx = OperationContext::doing("load agent runtime state")
        .with_field("state_path", path.display().to_string())
        .with_meta("component.name", "agentd.state_store");

    std::fs::read_to_string(path)
        .source_err(AgentdReason::StateUnavailable, "read agent_runtime.json")
        .with_context(&ctx)
}
```

---

## 6. 协议错误投影

Rust 内部错误不能直接序列化到协议。所有对外协议使用稳定投影对象。

### 6.1 最小投影字段

```text
ProtocolError {
  code
  message?
  detail?
  fields?
  correlation_id?
  retryable?
  severity?
}
```

字段说明：

| 字段 | 说明 |
|---|---|
| `code` | 稳定错误码，来自 `ErrorIdentity.code` 到错误码词典的映射 |
| `message` | 可暴露短文本，不包含路径、token、命令原文、完整 stderr |
| `detail` | 只在允许 debug 暴露的本地模式或审计通道出现 |
| `fields` | 可暴露结构化字段，例如 `execution_id`、`action_id`、`step_id` |
| `correlation_id` | request、execution、report 或 session 的关联 ID |
| `retryable` | 接收方是否可重试 |
| `severity` | `info` / `warning` / `error` / `fatal` |

### 6.2 协议落点

| 协议对象 | 字段 | 来源 |
|---|---|---|
| `ActionResult` | `exit_reason` | execution 级最终错误码 |
| `StepActionRecord` | `error_code` | 单 step 错误码 |
| `AgentEvent` | `reason_code` | 事件级原因码 |
| `AgentState` | `reason_code` / `last_error` | 本地状态摘要 |
| Gateway response | `error.code` | 南向 session / dispatch / ack 错误 |
| Control API response | `error.code` | 中心 API 错误 |

### 6.3 错误码映射

`../edge/error-codes.md` 继续维护协议稳定码。Rust reason identity 到协议码的映射由 `wist-shared` 维护。

示例映射：

| `ErrorIdentity.code` | 协议码 |
|---|---|
| `wi.validate.schema_invalid` | `schema_invalid` |
| `wi.validate.signature_invalid` | `signature_invalid` |
| `wi.agentd.queue_timeout` | `queue_timeout` |
| `wi.exec.step_timeout` | `step_timeout` |
| `wi.exec.process_exit_nonzero` | `process_exit_nonzero` |
| `wi.gateway.report_timeout` | `report_timeout` |

协议码必须稳定、短小、可审计；不得使用 Rust enum variant 名、`Display` 文本或 Debug 文本作为协议码。

---

## 7. 故障裁决

错误 reason 必须能映射到故障层级。

| 故障层级 | 典型 reason | 默认动作 |
|---|---|---|
| `execution-local` | `PlanRejected`、`StepFailed`、`ResultCorrupted`、`ExecutionQuarantined` | 隔离当前 execution，生成失败结果或 quarantine |
| `runtime-degraded` | `ReportingBacklog`、`CenterUnavailable`、`CapabilityDegraded`、`TelemetrySinkUnavailable` | daemon 继续运行，health 标记 degraded，限制相关能力 |
| `fatal-daemon` | `ConfigInvalid`、`StateUnavailable`、`QueueStoreUnavailable`、`RuntimeStoreUnavailable` | `run_once()` / `run_forever()` 返回错误，由外层拉起或人工处理 |

裁决规则：

- 不允许通过 detail 字符串判断故障层级。
- 同一个底层 IO 错误在不同边界可以有不同裁决。例如读单个 `result.json` 失败是 execution-local；读 `agent_runtime.json` 失败是 fatal-daemon。
- 裁决结果应写入 context metadata，例如 `failure.scope = execution-local`。
- health 与指标只消费稳定码和裁决结果，不解析源错误文本。

---

## 8. 跨层转换规则

### 8.1 只改变 reason 类型

低层错误已经表达了正确语义，上层只是换成自己的 reason 类型时使用 `conv_err()`。

```rust
use orion_error::{conversion::ConvErr, prelude::*};

fn bootstrap() -> Result<(), AgentdError> {
    validate_config().conv_err()
}
```

使用 `conv_err()` 的前提是上层 reason 已实现从低层 reason 的转换，例如 `impl From<ValidateReason> for AgentdReason`。

### 8.2 建立新的语义边界

上层要表达新的业务语义时使用 `source_err(...)`，把低层结构化错误作为 source 保留。

```rust
use orion_error::prelude::*;

fn start_daemon() -> Result<(), AgentdError> {
    load_config()
        .source_err(AgentdReason::ConfigInvalid, "load daemon config")
}
```

### 8.3 第三方错误

第三方错误没有 `UnstructuredSource` bridge 时使用 `source_raw_err(...)`。

```rust
use orion_error::prelude::*;

fn call_gateway() -> Result<(), GatewayError> {
    send_request()
        .source_raw_err(GatewayReason::SessionUnavailable, "send gateway request")
}
```

### 8.4 保留 `map_err` 的场景

只有在需要业务分支、改写用户可见提示、补充多个结构化字段或执行特殊裁决时保留 `map_err`。

---

## 9. 可观测性与审计

每个结构化错误进入观测链路时至少产生以下维度：

| 维度 | 示例 |
|---|---|
| `error.code` | `step_timeout` |
| `error.identity` | `wi.exec.step_timeout` |
| `error.scope` | `execution-local` |
| `component.name` | `agentd.scheduler` |
| `execution_id` | `exec_01` |
| `action_id` | `act_01` |
| `step_id` | `s1` |
| `retryable` | `true` |

日志策略：

- 本地 debug 日志可以记录 redacted report。
- 中心协议默认只携带 protocol projection。
- 审计日志记录稳定码、操作者、审批链、request id 和裁决结果。
- 不记录 token、私钥、完整命令输出、完整环境变量、未脱敏路径中的敏感片段。

指标策略：

- `*_error_total{code,scope,component}` 统计错误数量。
- `*_failure_decision_total{scope,action}` 统计裁决动作。
- reporting、gateway、exporter 类错误必须能区分 retryable 与 permanent。

---

## 10. 实施计划

### P0：错误基线

- 在 workspace 增加 `orion-error` 依赖，启用 `derive`，按需要启用 `serde_json` / `toml`。
- 在 `wist-shared` 增加错误投影和 code mapping 基础类型。
- 为 `wist-validate`、`warp-insightd`、`wist-exec` 定义首批 reason enum。
- 将 `error_codes.rs` 从 placeholder 扩展为稳定协议码词典。
- 增加热路径错误签名检查，禁止新增 `anyhow::Result`、裸 `io::Error`、`Box<dyn Error>` 和 `String` 错误返回。

### P1：热路径接入

- config loader、state store、scheduler、result writer、reporting pipeline 使用 `StructError<R>`。
- `ActionResult.exit_reason`、`StepActionRecord.error_code` 从结构化错误投影得到。
- `AgentEvent.reason_code` 和 local state `last_error` 使用同一套投影。

### P2：协议与中心

- gateway / control API 定义统一 `ProtocolError`。
- 增加 redacted debug rendering。
- 将错误码、故障裁决和 retryable 语义接入 self-observability。

---

## 11. 当前决定

当前阶段固定以下结论：

1. `orion-error` 是 Rust 内部错误处理的唯一主路径。
2. `StructError<R>` 不直接作为跨进程协议对象传输。
3. `ErrorIdentity.code` 是内部到协议映射的稳定来源。
4. `../edge/error-codes.md` 继续作为稳定字符串码词典，但需要由实现中的 code mapping 驱动。
5. reason enum 不携带动态 payload；动态信息进入 detail、context、fields、metadata 或 source。
6. 故障分层和恢复动作必须由稳定 reason / code / scope 决定。
7. 热路径不得让 `anyhow::Error`、裸 `io::Error`、`Box<dyn Error>` 或字符串错误继续向上传播。
8. 先实现 `validate`、`agentd`、`exec` 三条热路径，再扩展到 gateway、control 和 upgrader。
