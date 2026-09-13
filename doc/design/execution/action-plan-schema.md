# warp-insight ActionPlan Schema 草案

## 1. 文档目的

本文档把 [`action-plan-ir.md`](action-plan-ir.md) 中的 `ActionPlan IR` 收敛成字段级 schema 草案。

目标是给以下工作提供直接输入：

- `M1 IR Schema`
- `M2 Agentd Skeleton`
- `M3 Exec Skeleton`

相关文档：

- [`action-plan-ir.md`](action-plan-ir.md)
- [`control-plane.md`](../center/control-plane.md)
- [`agentd-exec-protocol.md`](../../crates/wist-agentd/docs/agentd-exec-protocol.md)

---

## 2. 核心结论

第一版 `ActionPlan` 必须满足：

- 单目标
- 可静态校验
- 可签名
- 可版本化
- 不依赖边缘解释 DSL

因此第一版建议：

- 结构固定
- 字段尽量少
- 明确枚举
- 明确哪些字段是必选

---

## 3. 顶层结构

```text
ActionPlan {
  api_version
  kind
  meta
  target
  constraints
  program
  attestation
  provenance?
}
```

### 3.1 必选字段

- `api_version`
- `kind`
- `meta`
- `target`
- `constraints`
- `program`
- `attestation`

### 3.2 可选字段

- `provenance`

### 3.3 固定值

- `api_version = "v1"`
- `kind = "action_plan"`

---

## 4. `meta`

```text
ActionPlanMeta {
  action_id
  request_id
  template_id?
  tenant_id
  environment_id
  plan_version
  compiled_at
  expires_at
}
```

### 4.1 字段约束

- `action_id`: `string`, 必选
- `request_id`: `string`, 必选
- `template_id`: `string`, 可选
- `tenant_id`: `string`, 必选
- `environment_id`: `string`, 必选
- `plan_version`: `uint64`, 必选
- `compiled_at`: RFC3339 UTC, 必选
- `expires_at`: RFC3339 UTC, 必选

---

## 5. `target`

```text
ActionPlanTarget {
  agent_id
  instance_id?
  node_id
  host_name?
  platform
  arch
  selectors?
}
```

### 5.1 字段约束

- `agent_id`: `string`, 必选
- `instance_id`: `string`, 可选
- `node_id`: `string`, 必选
- `host_name`: `string`, 可选
- `platform`: `string`, 必选
- `arch`: `string`, 必选
- `selectors`: `map<string,string>`, 可选

### 5.2 第一版限制

- 一个 `ActionPlan` 只能对应一个目标 agent

---

## 6. `constraints`

```text
ActionPlanConstraints {
  risk_level
  approval_ref?
  approval_mode
  requested_by
  reason?
  max_total_duration_ms
  step_timeout_default_ms
  limits
  allow
  required_capabilities
  execution_profile
}
```

### 6.1 `risk_level`

第一版建议枚举：

- `R0`
- `R1`
- `R2`
- `R3`

### 6.2 `approval_mode`

第一版建议枚举：

- `not_required`
- `required`

### 6.3 `limits`

```text
ActionLimits {
  max_stdout_bytes?
  max_stderr_bytes?
  max_memory_bytes?
  max_concurrent_ops?
}
```

### 6.4 `allow`

```text
ActionAllow {
  paths?
  services?
  targets?
}
```

字段建议：

- `paths`: `string[]`
- `services`: `string[]`
- `targets`: `string[]`

### 6.5 `required_capabilities`

- `string[]`, 必选

### 6.6 `execution_profile`

第一版建议枚举：

- `agent_exec_v1`

---

## 7. `program`

```text
Program {
  entry
  steps[]
  failure_policy?
}
```

### 7.1 字段约束

- `entry`: `string`, 必选
- `steps`: `Step[]`, 必选，长度 >= 1
- `failure_policy`: `FailurePolicy`, 可选

### 7.2 `Step`

第一版建议判别联合：

- `InvokeStep`
- `BranchStep`
- `GuardStep`
- `OutputStep`
- `AbortStep`

#### `InvokeStep`

```text
InvokeStep {
  step_id
  kind = "invoke"
  op
  args
  bind?
  timeout_ms?
  retry_policy?
  error_policy
  next?
}
```

#### `BranchStep`

```text
BranchStep {
  step_id
  kind = "branch"
  condition
  then
  else?
}
```

#### `GuardStep`

```text
GuardStep {
  step_id
  kind = "guard"
  condition
  reason_code
  detail?
  next?
}
```

#### `OutputStep`

```text
OutputStep {
  step_id
  kind = "output"
  items[]
  next?
}
```

#### `AbortStep`

```text
AbortStep {
  step_id
  kind = "abort"
  reason_code
  detail?
}
```

### 7.3 `retry_policy`

```text
RetryPolicy {
  max_attempts
  delay_ms?
  retry_on?
}
```

字段建议：

- `max_attempts`: `uint32`, 必选，>= 1
- `delay_ms`: `uint64`, 可选
- `retry_on`: `string[]`, 可选

### 7.4 `error_policy`

第一版建议枚举：

- `fail`
- `continue`

### 7.5 `OutputItem`

```text
OutputItem {
  name
  value
  redaction?
}
```

### 7.6 表达式和值

第一版建议：

- `ValueExpr`
  - `literal`
  - `var`
  - `field`
- `ConditionExpr`
  - `compare`
  - `logic`

---

## 8. `attestation`

```text
ActionPlanAttestation {
  policy_version
  compiler_version
  approval_digest?
  plan_digest
  signature
  issued_by
}
```

### 8.1 字段约束

- `policy_version`: `string`, 必选
- `compiler_version`: `string`, 必选
- `approval_digest`: `string`, 可选
- `plan_digest`: `string`, 必选
- `signature`: `string`, 必选
- `issued_by`: `string`, 必选

---

## 9. `provenance`

```text
ActionPlanProvenance {
  frontend_kind
  frontend_ref?
  frontend_digest?
  control_ref?
  control_digest?
}
```

### 9.1 `frontend_kind`

第一版建议枚举：

- `run.gxl`
- `run.war`
- `native_json`

---

## 10. 最小示例

```json
{
  "api_version": "v1",
  "kind": "action_plan",
  "meta": {
    "action_id": "act_01",
    "request_id": "req_01",
    "tenant_id": "t1",
    "environment_id": "prod",
    "plan_version": 1,
    "compiled_at": "2026-04-12T10:00:00Z",
    "expires_at": "2026-04-12T10:10:00Z"
  },
  "target": {
    "agent_id": "agent_prod_web_01",
    "node_id": "prod-web-01",
    "platform": "linux",
    "arch": "amd64"
  },
  "constraints": {
    "risk_level": "R0",
    "approval_mode": "not_required",
    "requested_by": "ops@example",
    "max_total_duration_ms": 5000,
    "step_timeout_default_ms": 3000,
    "limits": {},
    "allow": {
      "paths": [
        "/etc/nginx/nginx.conf"
      ]
    },
    "required_capabilities": [
      "file.read_range"
    ],
    "execution_profile": "agent_exec_v1"
  },
  "program": {
    "entry": "s1",
    "steps": [
      {
        "step_id": "s1",
        "kind": "invoke",
        "op": "file.read_range",
        "args": {
          "path": {
            "literal": "/etc/nginx/nginx.conf"
          },
          "offset": {
            "literal": 0
          },
          "length": {
            "literal": 4096
          }
        },
        "bind": "part",
        "error_policy": "fail",
        "next": "s2"
      },
      {
        "step_id": "s2",
        "kind": "output",
        "items": [
          {
            "name": "part",
            "value": {
              "var": "part"
            }
          }
        ]
      }
    ]
  },
  "attestation": {
    "policy_version": "p1",
    "compiler_version": "c1",
    "plan_digest": "sha256:xxx",
    "signature": "sig:xxx",
    "issued_by": "control-plane"
  }
}
```

---

## 11. 当前决定

当前阶段固定以下结论：

- `ActionPlan` 第一版必须是单目标、显式步骤图
- `program.steps[]` 只允许 `invoke / branch / guard / output / abort`
- `constraints`、`program`、`attestation` 是必选
