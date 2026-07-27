# warp-insight Agent Gateway 协议设计

## 1. 文档目的

本文档定义控制中心 `Agent Gateway` 与边缘 `warp-insightd` 之间的南向协议。

重点回答：

- agent 与控制中心如何建立会话
- 控制中心如何向 agent 下发计划
- agent 如何 ack 和回报结果
- Gateway 应该承担哪些职责，不承担哪些职责

相关文档：

- [`control-center-architecture.md`](control-center-architecture.md)
- [`control-plane.md`](control-plane.md)
- [`capability-report-schema.md`](../edge/capability-report-schema.md)
- [`dispatch-action-plan-schema.md`](dispatch-action-plan-schema.md)
- [`ack-action-plan-schema.md`](ack-action-plan-schema.md)
- [`report-action-result-schema.md`](report-action-result-schema.md)
- [`security-model.md`](../foundation/security-model.md)

---

## 2. 核心结论

`Agent Gateway` 第一版建议提供一条中心到边缘的受信控制通路。

它至少要承载：

- register / resume
- heartbeat
- capability report
- `DispatchActionPlan`
- `ActionPlanAck`
- `ReportActionResult`

它不应承载：

- 作者 DSL
- 任意 shell
- 边缘本地调试 RPC

---

## 3. 设计原则

### 3.1 Gateway 是协议入口，不是编排器

Gateway 负责：

- 认证
- 会话
- 收发协议对象
- 投递状态更新

Gateway 不负责：

- 审批
- 编译
- 风险判断

### 3.2 一条受信控制流

第一版建议把下面这些消息都收敛到同一南向协议族：

- agent online messages
- plan dispatch messages
- ack/result messages

### 3.3 会话与对象分离

需要区分：

- 连接是否在线
- 计划是否已投递
- 计划是否已 ack
- 计划是否已执行完成

不能把“连接在线”误当成“动作已执行”。

### 3.4 逻辑协议与传输实现解耦

本协议文档定义的是：

- 会话语义
- 消息对象
- 状态与幂等要求

它不应把这些语义强绑定到某一种传输实现。

第一版应先固定：

- `AgentHello`
- `AgentUpstreamMessage`
- `AgentDownstreamMessage`
- `DispatchActionPlan / ActionPlanAck / ReportActionResult`

至于底层是：

- `WebSocket over mTLS`
- gRPC stream
- 其他长连接传输

应视部署复杂度、调试成本和规模化要求决定。

---

## 4. 逻辑通道

第一版建议抽象成三类逻辑消息：

- `AgentHello`
- `AgentUpstreamMessage`
- `AgentDownstreamMessage`

### 4.1 `AgentHello`

用于建立或恢复 agent 会话。

### 4.2 `AgentUpstreamMessage`

边缘到中心的消息。

建议枚举：

- `heartbeat`
- `capability_report`
- `action_plan_ack`
- `report_action_result`
- `upgrade_ack`
- `upgrade_result`

### 4.3 `AgentDownstreamMessage`

中心到边缘的消息。

建议枚举：

- `dispatch_action_plan`
- `dispatch_upgrade_plan`
- `cancel_execution`
- `refresh_policy_hint`

说明：

- 第一版 `cancel_execution` 可以先保留占位
- 但不应阻塞 `DispatchActionPlan` 主链路

---

## 5. 会话建立

### 5.1 建连前提

`warp-insightd` 连接 Gateway 前必须具备：

- `agent_id`
- `instance_id`
- 双向身份认证材料

### 5.2 `AgentHello`

建议至少包含：

- `agent_id`
- `instance_id`
- `tenant_id`
- `environment_id`
- `node_id`
- `agent_version`
- `boot_id`
- `hello_time`
- `resume_token?`

### 5.3 Gateway 处理

Gateway 收到 `AgentHello` 后建议执行：

1. 完成双向认证
2. 校验 `agent_id / instance_id`
3. 创建或恢复 `agent_sessions`
4. 返回 `session_id` 或等价会话确认

### 5.4 会话恢复

第一版建议允许：

- 同一 `agent_id`
- 新 `instance_id`

被视为新实例上线，并使旧会话失效。

---

## 6. 心跳协议

### 6.1 用途

用于：

- 保持租约
- 刷新在线状态
- 返回最小运行摘要

### 6.2 建议字段

- `agent_id`
- `instance_id`
- `sent_at`
- `mode`
- `execution_queue_size`
- `running_executions`
- `reporting_executions`
- `agent_version`

### 6.3 处理原则

Gateway 不应因偶发一次心跳抖动就立即把 agent 标记永久离线。

建议区分：

- `online`
- `suspect`
- `offline`

---

## 7. CapabilityReport 上报

### 7.1 上报时机

第一版建议：

- 首次建连后立即上报
- agent 版本变化后上报
- 本地能力集变化后重上报

### 7.2 消息体

直接复用：

- [`capability-report-schema.md`](../edge/capability-report-schema.md)

### 7.3 Gateway 职责

Gateway 负责：

- 验证 envelope 基础字段
- 交给 Capability Catalog 落库与索引

Gateway 不负责：

- capability 语义决策

---

## 8. 计划下发协议

### 8.1 消息体

直接复用：

- [`dispatch-action-plan-schema.md`](dispatch-action-plan-schema.md)

### 8.2 下发前提

中心下发前应至少确认：

- agent 当前在线
- capability 预筛选通过
- `ActionPlan` 已签名
- 目标 `instance_id` 若绑定，则仍与当前会话一致

### 8.3 Gateway 职责

Gateway 负责：

- 从 Dispatch Service 接收待投递对象
- 投递到对应 agent 会话
- 更新投递尝试信息

Gateway 不负责：

- 重编译计划
- 修改计划内容

---

## 9. Ack 协议

### 9.1 消息体

直接复用：

- [`ack-action-plan-schema.md`](ack-action-plan-schema.md)

### 9.2 处理原则

Gateway 收到 ack 后建议：

1. 校验 `dispatch_id`
2. 校验 `action_id`
3. 校验 `plan_digest`
4. 更新 `dispatch_records`
5. 通知 Execution Tracker

### 9.3 关键区分

必须区分：

- `accepted`
- `queued`
- `duplicate`
- `rejected`

其中：

- `duplicate` 不等于执行失败
- `queued` 不等于已经开始执行

---

## 10. 结果回报协议

### 10.1 消息体

直接复用：

- [`report-action-result-schema.md`](report-action-result-schema.md)

### 10.2 Gateway 处理原则

Gateway 收到结果后建议：

1. 校验会话身份
2. 校验 `execution_id`
3. 校验 `action_id + plan_digest`
4. 校验 `result_attestation`
5. 交给 Result Ingestor 入库

### 10.3 幂等

Gateway 或 Result Ingestor 至少要支持以下幂等判断：

- `report_id`
- `execution_id`
- `action_id + plan_digest + result_attestation.result_digest`

---

## 11. 错误处理

第一版建议把 Gateway 错误分成四类：

- 认证错误
- 协议错误
- 路由错误
- 后端临时错误

### 11.1 认证错误

例如：

- mTLS 失败
- agent 身份不匹配

处理：

- 立即拒绝会话

### 11.2 协议错误

例如：

- 缺字段
- schema 不合法
- `dispatch_id` / `action_id` 不匹配

处理：

- 拒绝该消息
- 写审计

### 11.3 路由错误

例如：

- 会话不存在
- 目标 agent 不在线

处理：

- 交回 Dispatch Service 做重试或失败更新

### 11.4 后端临时错误

例如：

- 数据库短时不可写
- 对象存储抖动

处理：

- 优先返回可重试错误
- 不伪造成功 ack

---

## 12. 第一版传输建议

第一版建议满足以下要求：

- 双向认证
- 一条长连接控制通道
- 支持服务端主动下发
- 支持客户端主动上报

### 12.1 传输选择原则

选择传输时，优先看：

- 是否容易调试和现场排障
- 是否能稳定支撑长连接会话
- 是否容易表达双向消息
- 是否容易做断线重连和会话恢复
- 是否能支撑实例级身份认证与审计

第一版不应为了“协议看起来更先进”而引入过高的实现复杂度。

### 12.2 当前推荐

当前第一版设计决定为：

- `HTTPS long polling + mTLS`
- 消息 envelope 使用 JSON
- agent 只建立出站连接，控制平台不反连 agent

原因：

- HTTPS 是企业网络、防火墙、代理、LB 最容易接受的默认协议
- agent 主动出站连接，不要求边缘节点暴露入站端口
- long polling 足以承载第一版低频控制消息和中心侧下发
- mTLS 可以把注册后的 agent 身份绑定到 client certificate 与本地私钥
- 抓包、回放、现场诊断、审计和故障排查都比流式 RPC 更直接
- 不会把南向协议过早绑死在特定 RPC 框架上

第一版控制通道建议拆成以下 HTTPS API 语义：

- `POST /v1/agent/hello`
- `POST /v1/agent/heartbeat`
- `POST /v1/agent/capabilities`
- `GET /v1/agent/commands?wait=30s`
- `POST /v1/dispatch/{dispatch_id}/ack`
- `POST /v1/executions/{execution_id}/result`

其中：

- `GET /v1/agent/commands?wait=30s` 是控制平台向 agent 下发消息的 long polling 通道
- agent 在请求超时、收到消息或连接异常后立即重新发起下一次 long poll
- Gateway 仍必须区分连接是否在线、计划是否已投递、计划是否已 ack、结果是否已回报

身份建立分两段：

- 首次 enrollment 使用 `HTTPS server TLS + enrollment_token`
- 注册成功后的控制面通信使用 `HTTPS long polling + mTLS`

首次 enrollment 不要求 mTLS，因为 agent 当时还没有正式 client certificate。

传输实现上可以是：

- HTTPS long polling over mTLS
- WebSocket over mTLS
- gRPC bidirectional stream

其中：

- `HTTPS long polling + mTLS` 是第一版主实现
- WebSocket over mTLS 可以作为后续低延迟下发的可选实现
- gRPC stream 可以保留为后续服务化和强 IDL 治理场景的可选实现

### 12.3 为什么第一版不优先 gRPC stream

gRPC stream 的优点是：

- schema 化更自然
- 流式控制比较清晰
- 后续扩展 cancel / upgrade / policy hint 较方便

但第一版不优先它，原因是：

- 开发和调试门槛更高
- 现场排障不如文本化协议直观
- 长连接中断、重连、流恢复问题更依赖具体框架行为

### 12.4 HTTP 与 MQTT 的位置

普通 `HTTPS long polling + mTLS` 是第一版南向主协议。

它适合：

- 注册后 agent 到 Gateway 的控制会话
- heartbeat / capability 上报
- action plan 下发
- ack / result 回报
- policy hint / identity rotation hint 等低频控制消息

需要明确的是：

- 这里的 HTTP 不是短间隔 busy polling
- agent 使用有超时时间的 long poll 等待中心消息
- Gateway 通过租约、sequence、dispatch 状态机表达控制语义
- 控制消息对象仍使用 `AgentUpstreamMessage` / `AgentDownstreamMessage` 逻辑协议，不绑定 HTTP 路由

`MQTT over TLS/mTLS` 可以作为未来备选，但第一版不建议作为南向主协议，因为：

- 它更偏 broker / topic 模型
- 会把定向控制协议问题转成 topic 路由和 ACL 问题
- 审批、签名、实例级会话和审计链条会更绕

---

## 13. 多级树拓扑建议

如果未来 `warp-insightd` 演进为多级树结构，本协议仍建议保持同一逻辑协议族，不为每一级单独发明新协议。

### 13.1 基本原则

- 每个节点只与自己的父节点建立一条受信控制通道
- 子节点不直接感知更高层中心内部拓扑
- 上下游都使用统一的 `AgentHello / Upstream / Downstream` 语义
- 树形扩容优先通过分层和分片解决，不优先通过改协议解决

### 13.2 推荐模型

推荐模型是：

- 叶子 `agentd` 连接本层 `gateway`
- 中间层 `gateway` 或 relay 节点向上连接父层 gateway
- 每一跳都维护本跳的会话、心跳、ack 和重试

这意味着：

- 全局上看是树
- 单跳上看仍是点对点长连接控制协议

### 13.3 路由与身份要求

多级树下至少要保证：

- `agent_id` 仍表示最终边缘主体
- `instance_id` 仍表示当前边缘实例
- 转发链可附加 `gateway_path` 或等价 hop 元数据
- 审计系统能区分“最终执行节点”和“中间转发节点”

### 13.4 容量设计原则

当叶子节点规模达到万级时，优先关注的是：

- 单父节点扇出数
- 心跳峰值
- 重连风暴规模
- 未确认 dispatch 数量

架构原则应是：

- 先限制单节点扇出上限
- 再做 gateway 水平分片
- 再做树形分层

而不是先把协议替换成更复杂的消息系统。

### 13.5 当前建议

当前阶段建议固定：

- 万级叶子规模优先采用 gateway 水平分片和树形分层
- 南向单跳仍优先保持 `HTTPS long polling + mTLS`
- 中心内部服务间调用可使用 gRPC
- 南向逻辑协议不与具体传输强绑定

---

## 14. Gateway 本地状态建议

Gateway 自身第一版建议维护：

- 当前在线会话表
- `agent_id -> session_id` 映射
- 最近心跳时间
- 未确认 dispatch 摘要

说明：

- 会话态允许主要在内存
- 关键投递状态必须落回主库

---

## 15. 第一版最小闭环

第一版 Gateway 至少要打通：

1. `AgentHello`
2. heartbeat
3. `CapabilityReport`
4. `DispatchActionPlan`
5. `ActionPlanAck`
6. `ReportActionResult`

如果这六步未打通，控制中心就还没有真正连上边缘。

---

## 16. 当前决定

当前阶段固定以下结论：

- `Agent Gateway` 是控制中心南向协议入口
- Gateway 负责认证、会话和消息转发，不负责编译和审批
- `DispatchActionPlan / ActionPlanAck / ReportActionResult` 是第一版核心协议对象
- 第一版优先采用 agent 主动出站的 HTTPS long polling 模型
- 逻辑协议与传输实现解耦
- 第一版南向主实现优先 `HTTPS long polling + mTLS`
- 首次 enrollment 使用 `HTTPS server TLS + enrollment_token`，注册成功后正式控制面 API 使用 mTLS
