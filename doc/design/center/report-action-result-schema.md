# warp-insight ReportActionResult Schema 草案

## 1. 文档目的

本文档定义边缘 `warp-insightd` 向中心回报最终执行结果时使用的消息 envelope。

重点回答：

- `ActionResult` 如何包装成控制面回报消息
- 回报重试需要哪些字段
- 中心如何区分“消息投递失败”和“执行失败”

相关文档：

- [`action-result-schema.md`](../execution/action-result-schema.md)
- [`ack-action-plan-schema.md`](ack-action-plan-schema.md)
- [`agentd-state-schema.md`](../../crates/wist-agentd/docs/agentd-state-schema.md)

---

## 2. 核心结论

`ActionResult` 是执行语义对象，`ReportActionResult` 是控制面传输对象。

两者必须分开：

- `ActionResult` 描述执行发生了什么
- `ReportActionResult` 描述边缘怎样把结果回报给中心

---

## 3. 顶层结构

```text
ReportActionResult {
  api_version
  kind
  report_id
  dispatch_id?
  action_id
  plan_digest
  execution_id
  agent_id
  instance_id
  report_attempt
  final_status
  result
  result_attestation
  reported_at
}
```

### 3.1 固定值

- `api_version = "v1"`
- `kind = "report_action_result"`

### 3.2 必选字段

- `api_version`
- `kind`
- `report_id`
- `action_id`
- `plan_digest`
- `execution_id`
- `agent_id`
- `instance_id`
- `report_attempt`
- `final_status`
- `result`
- `result_attestation`
- `reported_at`

---

## 4. 字段说明

### 4.1 `report_id`

建议类型：

- `string`

用途：

- 标识一次结果回报尝试对象

### 4.2 `dispatch_id`

建议类型：

- `string`, 可选

说明：

- 如果当前 execution 明确来自某个 `DispatchActionPlan`，建议回填

### 4.3 `plan_digest`

建议类型：

- `string`

说明：

- 必须与已执行 `ActionPlan.attestation.plan_digest` 一致
- 便于中心在重试、回放和验签场景中稳定关联到同一执行语义

### 4.4 `report_attempt`

建议类型：

- `uint32`

说明：

- 第几次向中心回报，从 `1` 开始

### 4.5 `final_status`

建议与 `result.final_status` 保持一致。

这样中心即使不完整展开 `result`，也能直接做索引和聚合。

### 4.6 `result`

```text
result: ActionResult
```

说明：

- 完整复用 [`action-result-schema.md`](../execution/action-result-schema.md)

### 4.7 `result_attestation`

```text
ResultAttestation {
  result_digest
  signature
  issued_by
  attested_at
}
```

字段建议：

- `result_digest`: `string`, 必选
- `signature`: `string`, 必选
- `issued_by`: `string`, 必选
- `attested_at`: RFC3339 UTC, 必选

用途：

- 对 `ActionResult` 做结果级完整性保护
- 支撑幂等、去重、本地文件校验和中心验签

---

## 5. 回报状态要求

第一版建议：

- `warp-insightd` 在生成 `ActionResult` 后，先写本地 `result.json`
- `warp-insightd` 读取 `result.json` 后计算 `result_digest` 并生成本地签名
- 再生成 `reporting/<execution_id>.json`
- 成功收到中心确认前，不删除本地 reporting 状态

中心处理 `ReportActionResult` 时要能区分：

- 执行本身失败
- 结果消息重复
- 结果消息验签或摘要不匹配

---

## 6. 最小示例

```json
{
  "api_version": "v1",
  "kind": "report_action_result",
  "report_id": "rep_01",
  "dispatch_id": "dsp_01",
  "action_id": "act_01",
  "plan_digest": "sha256:abc123",
  "execution_id": "exec_01",
  "agent_id": "agent_prod_web_01",
  "instance_id": "inst_01",
  "report_attempt": 1,
  "final_status": "succeeded",
  "result_attestation": {
    "result_digest": "sha256:abc123",
    "signature": "sig_result_01",
    "issued_by": "agent_prod_web_01",
    "attested_at": "2026-04-12T10:00:03Z"
  },
  "reported_at": "2026-04-12T10:00:03Z",
  "result": {
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
        "started_at": "2026-04-12T10:00:02Z",
        "finished_at": "2026-04-12T10:00:02Z",
        "duration_ms": 4
      }
    ],
    "outputs": {
      "items": [
        {
          "name": "part",
          "value": {
            "length": 4096
          }
        }
      ]
    },
    "started_at": "2026-04-12T10:00:02Z",
    "finished_at": "2026-04-12T10:00:02Z"
  }
}
```

---

## 7. 当前决定

当前阶段固定以下结论：

- `ActionResult` 与 `ReportActionResult` 必须分离
- 回报重试由 `report_attempt` 和 `report_id` 标识
- `result_attestation` 是结果完整性保护的必需字段
- `final_status` 应在 envelope 顶层重复一份，便于中心索引
