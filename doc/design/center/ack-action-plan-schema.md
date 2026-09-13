# warp-insight ActionPlanAck Schema 草案

## 1. 文档目的

本文档定义边缘 `warp-insightd` 对 `DispatchActionPlan` 的接收确认消息。

重点回答：

- ack 只确认什么，不确认什么
- 哪些本地拒绝应在 ack 阶段返回
- 中心如何通过 ack 关联到后续 execution

相关文档：

- [`dispatch-action-plan-schema.md`](dispatch-action-plan-schema.md)
- [`agentd-state-schema.md`](../../crates/wist-agentd/docs/agentd-state-schema.md)
- [`error-codes.md`](../edge/error-codes.md)

---

## 2. 核心结论

`ActionPlanAck` 只表示边缘对投递对象的本地接收结论。

它不表示：

- 计划已经执行完成
- 计划一定会成功
- 最终结果已经产生

---

## 3. 顶层结构

```text
ActionPlanAck {
  api_version
  kind
  dispatch_id
  action_id
  plan_digest
  agent_id
  instance_id
  ack_status
  execution_id?
  reason_code?
  reason_message?
  queue_position?
  received_at
  acknowledged_at
  agent_runtime?
}
```

### 3.1 固定值

- `api_version = "v1"`
- `kind = "action_plan_ack"`

### 3.2 必选字段

- `api_version`
- `kind`
- `dispatch_id`
- `action_id`
- `plan_digest`
- `agent_id`
- `instance_id`
- `ack_status`
- `received_at`
- `acknowledged_at`

---

## 4. `ack_status`

第一版建议枚举：

- `accepted`
- `queued`
- `rejected`
- `duplicate`
- `stale`
- `busy`

### 4.1 含义说明

- `accepted`
  已通过本地校验，且已进入本地执行流程
- `queued`
  已通过本地校验，但当前只进入 `execution_queue`
- `rejected`
  本地校验失败，不会执行
- `duplicate`
  相同执行语义已处理
- `stale`
  计划已过期，或已被更高版本替代
- `busy`
  本地资源或策略暂时不允许接收

### 4.2 `execution_id`

建议规则：

- `accepted` / `queued` 时应返回
- 其他状态可为空

### 4.3 `queue_position`

建议规则：

- 仅在 `ack_status = "queued"` 时返回
- 表示当前 `execution_queue` 内的相对位置

---

## 5. `reason_code` 与 `reason_message`

### 5.1 `reason_code`

建议优先复用 [`error-codes.md`](../edge/error-codes.md) 中稳定原因码。

例如：

- `signature_invalid`
- `target_mismatch`
- `plan_expired`
- `capability_missing`
- `constraints_violation`
- `execution_queue_full`

补充约定：

- 若 `dispatch_id` 已处理，但对应的 `action_id + plan_digest` 与本地既有 execution 相同，应返回 `ack_status = "duplicate"`
- 若 `dispatch_id` 不同，但 `action_id + plan_digest` 已存在，也应返回 `ack_status = "duplicate"`，而不是重新执行

### 5.2 `reason_message`

第一版要求：

- 可选
- 仅用于辅助排障
- 不应放大 payload 或泄露敏感数据

---

## 6. `agent_runtime`

```text
AckAgentRuntimeSnapshot {
  mode
  execution_queue_size
  running_executions
  reporting_executions
  version?
}
```

### 6.1 用途

为中心提供 ack 时刻的最小本地状态快照。

### 6.2 第一版要求

- `agent_runtime` 可选
- 如返回，字段应来自本地状态快照，而不是临时拼接推测值

---

## 7. 最小示例

```json
{
  "api_version": "v1",
  "kind": "action_plan_ack",
  "dispatch_id": "dsp_01",
  "action_id": "act_01",
  "plan_digest": "sha256:abc123",
  "agent_id": "agent_prod_web_01",
  "instance_id": "inst_01",
  "ack_status": "queued",
  "execution_id": "exec_01",
  "queue_position": 1,
  "received_at": "2026-04-12T10:00:01Z",
  "acknowledged_at": "2026-04-12T10:00:01Z",
  "agent_runtime": {
    "mode": "normal",
    "execution_queue_size": 1,
    "running_executions": 0,
    "reporting_executions": 0,
    "version": "0.1.0"
  }
}
```

---

## 8. 当前决定

当前阶段固定以下结论：

- ack 只确认本地接收和校验结果
- `execution_id` 应尽早在 ack 阶段建立
- 最终成功或失败必须通过 `ActionResult` 路径单独回报
