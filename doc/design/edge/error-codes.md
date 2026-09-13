# warp-insight 错误码与原因码词典

## 1. 文档目的

本文档统一以下字段里出现的稳定错误码和原因码：

- `ActionResult.exit_reason`
- `StepActionRecord.error_code`
- `agentd-events.reason_code`
- `agentd-state.reason_code`

相关文档：

- [`error-handling-system.md`](../foundation/error-handling-system.md)
- [`action-result-schema.md`](../execution/action-result-schema.md)
- [`agentd-events.md`](../../crates/wist-agentd/docs/agentd-events.md)
- [`agentd-state-schema.md`](../../crates/wist-agentd/docs/agentd-state-schema.md)

---

## 2. 设计原则

- 错误码稳定、短小、可审计
- 不把长文本错误直接当错误码
- 同一个原因在不同模块尽量复用同一个码

---

## 3. 第一版建议枚举

### 3.1 校验拒绝类

- `signature_invalid`
- `plan_expired`
- `target_mismatch`
- `capability_missing`
- `constraints_violation`
- `schema_invalid`

### 3.2 调度与本地控制类

- `queue_timeout`
- `execution_queue_full`
- `spawn_failed`
- `cancel_requested`
- `kill_forced`

### 3.3 执行类

- `step_failed`
- `guard_failed`
- `abort_triggered`
- `step_timeout`
- `process_exit_nonzero`

### 3.4 上报类

- `report_failed`
- `report_timeout`
- `result_signature_invalid`

---

## 4. 使用建议

- `exit_reason`
  用较高层的最终原因码
- `error_code`
  用单 step 或单模块细粒度原因码
- `detail`
  放补充文本，不放稳定标识

---

## 5. 当前决定

当前阶段固定以下结论：

- 第一版先用稳定字符串错误码
- 长文本只放 `detail`
- 新错误码先复用已有类别，再新增
