# warp-insight ActionResult Schema 草案

## 1. 文档目的

本文档把 [`action-plan-ir.md`](action-plan-ir.md) 中的 `ActionResult` 和 `StepActionRecord` 收敛成字段级 schema 草案。

目标是给以下工作提供直接输入：

- `M1 IR Schema`
- `M2 Agentd Skeleton`
- `M3 Exec Skeleton`
- `M4 Local Edge Loop`

相关文档：

- [`action-plan-ir.md`](action-plan-ir.md)
- [`agentd-exec-protocol.md`](../../crates/wist-agentd/docs/agentd-exec-protocol.md)
- [`agentd-state-schema.md`](../../crates/wist-agentd/docs/agentd-state-schema.md)

---

## 2. 核心结论

`ActionResult` 必须明确区分两类信息：

- 审计与排障信息
- 对上返回的业务结果

因此第一版建议：

- `step_records[]` 用于审计与排障
- `outputs` 用于控制平面和调用方消费

---

## 3. 顶层结构

```text
ActionResult {
  api_version
  kind
  action_id
  execution_id
  request_id?
  final_status
  exit_reason?
  step_records[]
  outputs?
  resource_usage?
  started_at?
  finished_at?
}
```

### 3.1 固定值

- `api_version = "v1"`
- `kind = "action_result"`

### 3.2 必选字段

- `api_version`
- `kind`
- `action_id`
- `execution_id`
- `final_status`
- `step_records`

---

## 4. `final_status`

第一版建议枚举：

- `rejected`
- `succeeded`
- `failed`
- `cancelled`
- `timed_out`

### 4.1 `exit_reason`

建议类型：

- `string`, 可选

用于表达稳定原因码，例如：

- `signature_invalid`
- `target_mismatch`
- `guard_failed`
- `cancel_requested`
- `step_timeout`

---

## 5. `step_records[]`

```text
StepActionRecord {
  step_id
  attempt
  op?
  status
  started_at
  finished_at?
  duration_ms?
  error_code?
  stdout_summary?
  stderr_summary?
  resource_usage?
}
```

### 5.1 `status`

第一版建议枚举：

- `started`
- `succeeded`
- `failed`
- `cancelled`
- `timed_out`
- `skipped`

### 5.2 字段说明

- `op`
  仅对 `invoke` step 强烈建议填充
- `attempt`
  从 1 开始
- `stdout_summary` / `stderr_summary`
  仅保留摘要，不应回传完整大文本

---

## 6. `outputs`

```text
ActionOutputs {
  items[]
}
```

```text
ActionOutputItem {
  name
  value
  redacted?
}
```

### 6.1 第一版要求

- `outputs` 对应 `program.output` 选择出的结果
- 不应默认把所有 step 输出都自动暴露

---

## 7. `resource_usage`

```text
ExecutionResourceUsage {
  max_rss_bytes?
  cpu_time_ms?
  stdout_bytes?
  stderr_bytes?
}
```

第一版可以部分实现，但 schema 位置先固定。

---

## 8. 最小示例

```json
{
  "api_version": "v1",
  "kind": "action_result",
  "action_id": "act_01",
  "execution_id": "exec_01",
  "final_status": "succeeded",
  "step_records": [
    {
      "step_id": "s1",
      "attempt": 1,
      "op": "file.read_range",
      "status": "succeeded",
      "started_at": "2026-04-12T10:00:00Z",
      "finished_at": "2026-04-12T10:00:00Z",
      "duration_ms": 4
    },
    {
      "step_id": "s2",
      "attempt": 1,
      "status": "succeeded",
      "started_at": "2026-04-12T10:00:00Z",
      "finished_at": "2026-04-12T10:00:00Z",
      "duration_ms": 1
    }
  ],
  "outputs": {
    "items": [
      {
        "name": "part",
        "value": {
          "content": "user nginx;\n...",
          "length": 4096
        }
      }
    ]
  },
  "started_at": "2026-04-12T10:00:00Z",
  "finished_at": "2026-04-12T10:00:00Z"
}
```

---

## 9. 与本地状态文件的关系

建议关系如下：

- `workdir/result.json`
  内容就是 `ActionResult`
- `state/reporting/<execution_id>.json`
  只保存结果回传过程状态，不重复保存完整结果

---

## 10. 当前决定

当前阶段固定以下结论：

- `ActionResult` 必须显式区分 `step_records[]` 和 `outputs`
- `step_records[]` 服务审计和排障
- `outputs` 服务控制平面和上层调用方
