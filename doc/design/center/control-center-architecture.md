# warp-insight 控制中心架构设计

## 1. 文档目的

本文档定义 `warp-insight` 中心侧控制系统的总体架构。

这里的“控制中心”特指：

- 面向 `warp-insightd` 的中心控制节点
- 负责请求、审批、编译、签名、下发、跟踪、审计、升级编排的中心系统

本文档重点回答：

- 控制中心应该拆成哪些逻辑模块
- 哪些能力属于控制中心，哪些不属于
- 控制中心与边缘 agent 的交互主通路是什么
- 第一版应该先做成单体，还是直接拆分服务
- 控制中心需要哪些核心存储与异步通道

相关文档：

- [`architecture.md`](../foundation/architecture.md)
- [`target.md`](../foundation/target.md)
- [`control-plane.md`](control-plane.md)
- [`security-model.md`](../foundation/security-model.md)
- [`dispatch-action-plan-schema.md`](dispatch-action-plan-schema.md)
- [`ack-action-plan-schema.md`](ack-action-plan-schema.md)
- [`report-action-result-schema.md`](report-action-result-schema.md)
- [`control-center-storage-schema.md`](control-center-storage-schema.md)
- [`agent-gateway-protocol.md`](agent-gateway-protocol.md)
- [`identity-enrollment-protocol.md`](identity-enrollment-protocol.md)
- [`management-platform-bootstrap.md`](management-platform-bootstrap.md)
- [`roadmap.md`](../foundation/roadmap.md)

---

## 2. 核心结论

`warp-insight` 控制中心第一版应被定义为：

- 一个中心治理系统
- 一个控制平面执行系统
- 一个审计与风险控制系统

它不是：

- 边缘执行器
- 数据面 collector
- 边缘 metrics/logs/traces 处理节点

一句话说：

- 边缘负责执行和上报
- 控制中心负责受理、治理、编译、下发、跟踪和归档

---

## 3. 设计原则

### 3.1 控制中心优先做“控制闭环”

第一版最重要的不是做一个很大的平台 UI，而是先保证下面这条链路可稳定运行：

`ActionRequest -> Approval -> Compile -> Sign -> Dispatch -> Ack -> Execute -> Report -> Audit`

### 3.2 控制中心优先做“确定性编排”

控制中心可以结合 AI 提供建议，但真正进入边缘执行的对象必须是确定性产物：

- 已绑定策略
- 已绑定审批
- 已绑定目标
- 已签名
- 可静态校验

### 3.3 控制中心与数据中心分层

控制中心可以依赖资源目录、查询系统、AI 分析系统提供辅助信息，但不应与其强耦合成单一运行时。

第一版应明确：

- 控制中心可以没有完整查询平台也能工作
- 控制中心不能依赖 AI 在线推理才能下发计划

### 3.4 单体起步，服务化边界先设计清楚

第一版实现建议：

- 逻辑上按服务边界拆分
- 物理上允许单体部署

这样可以同时满足：

- 尽快落地
- 后续可按边界拆分

---

## 4. 控制中心边界

### 4.1 控制中心负责什么

控制中心负责：

- agent 注册与身份管理
- agent 会话与租约管理
- capability 接收与索引
- 策略管理
- 动作模板管理
- 动作请求受理
- 审批流转
- `ActionPlan` 编译与签名
- 计划分发
- ack 接收与状态更新
- 结果接收与归档
- 升级计划编排
- 审计、风险与回滚记录

### 4.2 控制中心不负责什么

控制中心不负责：

- 替代 `warp-insightd` 执行 opcode
- 直接控制 `warp-insight-exec`
- 在中心侧实时解析边缘原始 telemetry 流
- 替代资源查询平台做全量分析引擎

### 4.3 与其他中心系统的关系

建议把控制中心视为中心节点中的一个核心子系统。

它可以依赖：

- 资源目录
- 身份与 RBAC 系统
- 查询/搜索系统
- AI Copilot 系统

但这些依赖都应通过稳定接口接入，而不是把控制中心写成它们的内部模块。

---

## 5. 总体逻辑架构

建议控制中心拆成以下逻辑模块：

- Northbound API
- Install Bootstrap Endpoint
- Enrollment Service
- Agent Gateway
- Agent Registry
- Token / Certificate Service
- Capability Catalog
- Policy Service
- Action Template Registry
- Approval Service
- Plan Compiler
- Signer
- Dispatch Service
- Execution Tracker
- Result Ingestor
- Upgrade Orchestrator
- Audit & Risk Service
- Control Query API

逻辑视图如下：

```text
+------------------------------------------------------------------+
|                      warp-insight Control Center                     |
|                                                                  |
|  +------------------+    +------------------+                    |
|  | Northbound API   |    | Control Query    |                    |
|  +------------------+    +------------------+                    |
|  +------------------+    +------------------+                    |
|  | Policy Service   |    | Approval Service |                    |
|  +------------------+    +------------------+                    |
|  +------------------+    +------------------+                    |
|  | Template Reg.    |    | Plan Compiler    |                    |
|  +------------------+    +------------------+                    |
|  +------------------+    +------------------+                    |
|  | Signer           |    | Dispatch Service |                    |
|  +------------------+    +------------------+                    |
|  +------------------+    +------------------+                    |
|  | Agent Gateway    |    | Execution Tracker|                    |
|  +------------------+    +------------------+                    |
|  +------------------+    +------------------+                    |
|  | Result Ingestor  |    | Audit & Risk     |                    |
|  +------------------+    +------------------+                    |
|  +------------------+    +------------------+                    |
|  | Agent Registry   |    | Capability Cat.  |                    |
|  +------------------+    +------------------+                    |
|  +------------------+                                             |
|  | Upgrade Orch.    |                                             |
|  +------------------+                                             |
+------------------------------^-----------------------------------+
                               |
                     control stream / result stream
                               |
+------------------------------v-----------------------------------+
|                           warp-insightd                               |
+------------------------------------------------------------------+
```

---

## 6. 模块职责

### 6.1 Northbound API

面向人和自动化系统的上层入口。

第一版建议承担：

- 创建 `ActionRequest`
- 查询 request / execution / result / audit
- 创建或更新 `PolicySet`
- 创建或更新 `ActionTemplate`
- 创建升级计划

它不直接做：

- 编译
- 分发
- ack 处理

### 6.2 Install Bootstrap Endpoint

面向 operator 和目标机器安装脚本的 bootstrap 入口。

第一版建议承担：

- 根据 `advertise_url` 生成 agent 安装 URL
- 生成短期 bootstrap token
- 分发 agent 安装脚本
- 分发 trust bundle
- 分发最小 bootstrap 配置

它不直接做：

- 长期控制面认证
- agent client certificate 签发后的正式控制通道
- telemetry 数据接收

### 6.3 Enrollment Service

面向首次启动的 `warp-insightd`。

第一版建议承担：

- 校验 `enrollment_token`
- 接收 CSR 和 node facts
- 创建或更新 agent identity
- 触发 client certificate 签发
- 返回 `agent_id`、`instance_id`、CA bundle、gateway endpoint 和初始配置

它不直接做：

- action 编译
- action dispatch
- telemetry 数据解析

### 6.4 Agent Gateway

面向 `warp-insightd` 的南向入口。

第一版建议承担：

- agent 双向认证接入
- 长连接或流式控制通道维护
- 接收 register / heartbeat / capability report
- 下发 `DispatchActionPlan`
- 接收 `ActionPlanAck`
- 接收 `ReportActionResult`

它不直接做：

- 策略判断
- 编译
- 审批

### 6.5 Agent Registry

负责维护 agent 逻辑身份、实例状态和租约。

第一版建议维护：

- `agent_id`
- `instance_id`
- `tenant_id`
- `environment_id`
- `node_id`
- 当前版本
- 会话状态
- `last_seen_at`
- `lease_expires_at`

### 6.6 Token / Certificate Service

负责 bootstrap token 与 agent client certificate 生命周期。

第一版建议承担：

- bootstrap token 创建、过期、使用次数限制和撤销
- 管理平台 server trust bundle 管理
- agent CSR 校验与 client certificate 签发
- 证书续期与吊销记录

它不直接做：

- agent 会话维护
- action dispatch
- 审批判断

### 6.7 Capability Catalog

负责接收并索引边缘能力声明。

第一版建议支持：

- opcode 能力
- discovery 能力
- upgrade 能力
- 本地限制上限

它为以下模块提供读取能力：

- Plan Compiler
- Dispatch Service
- Upgrade Orchestrator

### 6.8 Policy Service

负责策略定义、版本管理和目标范围匹配。

第一版建议支持：

- 全局平台基线策略
- 租户级策略
- 环境级策略
- node selector 生效范围

### 6.9 Action Template Registry

负责维护作者侧模板与可复用动作。

第一版建议存：

- `control_ref`
- `run_ref`
- template metadata
- 默认风险等级
- 支持目标范围

### 6.10 Approval Service

负责风险校验、审批流转和审批结果落库。

第一版建议支持：

- `R0` 免审批但审计
- `R1` 受权限控制
- `R2` / `R3` 审批流
- 审批过期与撤销

### 6.11 Plan Compiler

负责把作者输入和控制约束编译成边缘唯一理解的 `ActionPlan`。

输入建议包括：

- `ActionTemplate`
- `ActionRequest`
- `PolicySet`
- `ApprovalRecord`
- `CapabilityReport`

输出：

- 单目标 `ActionPlan`

### 6.12 Signer

负责对中心侧最终控制对象签名。

第一版建议负责：

- `ActionPlan.attestation.signature`
- 升级计划签名

说明：

- 结果级签名属于 `warp-insightd` 责任，不属于中心 Signer

### 6.13 Dispatch Service

负责将已签名计划投递到目标 agent。

第一版建议承担：

- 目标 agent 选择
- `dispatch_id` 分配
- 投递重试
- `DispatchRecord` 状态更新
- ack 超时处理

它不负责：

- 重编译计划
- 解释执行结果

### 6.14 Execution Tracker

负责维护 request / action / execution 的中心状态机。

第一版建议聚合：

- `ActionPlanAck`
- `ReportActionResult`
- agent 在线状态
- dispatch 状态

### 6.15 Result Ingestor

负责接收 `ReportActionResult` 并做最小校验与入库。

第一版建议至少校验：

- `action_id`
- `execution_id`
- `plan_digest`
- `result_attestation`
- `final_status`

### 6.16 Upgrade Orchestrator

负责中心侧升级批次管理、波次控制和结果跟踪。

第一版建议支持：

- 升级批次
- 目标范围展开
- 波次发布
- 健康闸门
- 自动暂停 / 回滚触发

### 6.17 Audit & Risk Service

负责高风险动作、策略变化、审批变化和执行闭环的审计归档。

第一版建议至少归档：

- request 创建
- approval 变化
- plan 编译
- dispatch / ack
- result report
- upgrade / rollback

### 6.18 Control Query API

负责给 UI、CLI、自动化系统提供控制面查询。

第一版建议支持：

- agent 状态查询
- request / action / execution 查询
- result 摘要查询
- 审计记录查询

---

## 7. 核心数据对象

控制中心第一版建议把数据对象分成五类：

- 身份与会话对象
- 治理对象
- 执行对象
- 升级对象
- 审计对象

### 7.1 身份与会话对象

- `AgentRegistryEntry`
- `CapabilityReport`
- `AgentLease`

### 7.2 治理对象

- `PolicySet`
- `ActionTemplate`
- `ActionRequest`
- `ApprovalRecord`

### 7.3 执行对象

- `ActionPlan`
- `DispatchRecord`
- `ActionExecution`
- `ActionResult`

### 7.4 升级对象

- `UpgradePlan`
- `UpgradeDispatch`
- `UpgradeExecution`

### 7.5 审计对象

- `AuditRecord`
- `RiskEvent`

---

## 8. 中心状态主线

控制中心第一版至少要维护三条状态主线：

- agent 主线
- action 主线
- upgrade 主线

### 8.1 agent 主线

建议状态：

- `registered`
- `online`
- `offline`
- `lease_expired`
- `draining`
- `upgrading`

### 8.2 action 主线

建议关系：

`ActionRequest -> ActionPlan -> DispatchRecord -> ActionExecution -> ActionResult`

其中：

- `ActionRequest` 是意图主键
- `ActionPlan` 是执行语义主键
- `DispatchRecord` 是投递主键
- `ActionExecution` 是实际执行主键

### 8.3 upgrade 主线

建议关系：

`UpgradePlan -> UpgradeDispatch -> UpgradeExecution`

---

## 9. 存储设计

第一版建议最少引入三类存储：

- 关系型元数据存储
- 对象存储
- 异步事件总线

### 9.1 关系型元数据存储

建议存放：

- agent registry
- leases
- capabilities summary
- policies
- templates
- requests
- approvals
- dispatch records
- execution summary
- audit index

原因：

- 事务边界清晰
- 查询条件稳定
- 适合控制对象和状态机

### 9.2 对象存储

建议存放：

- `ActionPlan` 原文
- `ActionResult` 原文
- 升级计划原文
- 审计附件

原因：

- 大对象不适合全部塞进关系库
- 便于归档和回放

### 9.3 异步事件总线

建议承载：

- agent lifecycle event
- request lifecycle event
- approval event
- dispatch event
- result event
- audit event

说明：

- 第一版允许用进程内事件总线或数据库 outbox 起步
- 不要求一开始就上重型消息系统

---

## 10. 通信通路

### 10.1 北向通路

人和自动化系统通过 `Northbound API` 与控制中心交互。

建议能力：

- REST/gRPC 二选一或并存
- UI / CLI / automation client 共用统一服务层

### 10.2 南向通路

`warp-insightd` 通过 `Agent Gateway` 与控制中心交互。

第一版建议支持：

- 双向认证
- 长连接控制通道
- 心跳
- capability 上报
- ack/result 上报

当前建议是：

- 逻辑协议独立于传输实现
- 第一版优先采用 agent 主动出站的 `HTTPS long polling + mTLS`
- WebSocket over mTLS 可作为后续低延迟下发的可选实现
- gRPC 可作为后续可选实现，不作为第一版绑定前提

### 10.3 多级树拓扑下的接入模型

如果未来边缘接入演进为多级树结构，控制中心应坚持：

- 南向协议对象保持统一
- 通过 gateway 分层与分片扩容
- 不把树形扩容问题转成 broker/topic 设计问题

推荐模型是：

- 叶子节点连接本层 gateway
- 中间层只做会话承接、路由转发、状态汇总
- 控制中心始终面向统一的 southbound protocol

这样做的好处是：

- 单跳协议简单
- 调试路径清晰
- 审计链可保留 hop 信息
- 规模增长时可以优先控制单节点扇出

### 10.4 内部通路

控制中心内部模块之间建议通过：

- 服务内调用
- 事务 + outbox
- 事件订阅

不建议第一版：

- 让所有模块直接共享数据库表并互相写状态

---

## 11. 多租户与隔离

第一版控制中心必须把以下维度作为一等隔离键：

- `tenant_id`
- `environment_id`

所有核心对象都应能回溯到至少这两个维度。

必须保证：

- request 不能越租户
- dispatch 不能越环境
- 审计查询不能越权读
- 策略生效范围可解释

---

## 12. 与资源目录和 AI 的关系

### 12.1 资源目录

控制中心可读取资源目录，用于：

- target selector 展开
- node / service / workload 查找
- 风险辅助判断

但控制中心不负责全局资源归并算法本身。

### 12.2 AI Copilot

AI 在控制中心中的建议位置是：

- request 辅助生成
- 风险解释
- 审批建议
- 失败原因摘要
- 回滚建议

AI 不应直接拥有：

- 绕过审批的执行权
- 直接签发 `ActionPlan` 的权限
- 直接向 agent 下发原始命令的能力

---

## 13. 第一版部署建议

### 13.1 逻辑拆分

第一版建议至少按以下逻辑边界编码：

- `control-api`
- `install-bootstrap`
- `enrollment-service`
- `agent-gateway`
- `token-cert-service`
- `policy-engine`
- `plan-compiler`
- `dispatch-tracker`
- `audit-risk`

### 13.2 物理部署

第一版建议优先单体部署：

- 一个 all-in-one 管理平台容器
- 一个进程或少量同容器进程
- 一个关系库
- 一个对象存储
- 一个最小事件总线实现

管理平台容器必须通过 `advertise_url` 对外声明自身可访问地址，用于生成安装 URL 和 agent bootstrap 配置。

第一版推荐入口：

```text
operator runs warp-insight-platform container
  -> platform generates install URL
  -> agent installer writes enrollment_url / gateway_url / trust_bundle
  -> agent enrolls and switches to mTLS control plane
```

但这不意味着 `Agent Gateway` 永远与其他模块共进程。

当南向连接规模上升时，建议优先把它拆成独立接入层，以承接：

- 长连接会话
- 心跳与在线状态
- 下发与回报流量
- 树形分层下的下级 gateway / relay 接入

### 13.3 后续拆分顺序

当负载增长后，建议优先拆：

1. `agent-gateway`
2. `plan-compiler`
3. `audit-risk`
4. `control-query`

---

## 14. 第一版最小闭环

控制中心第一版最小可用闭环建议固定为：

1. agent 注册与租约
2. 管理平台安装 URL 生成
3. bootstrap token 与 trust bundle 分发
4. enrollment 与 client certificate 签发
5. capability report 接收
6. `ActionTemplate` 管理
7. `ActionRequest` 创建
8. `ApprovalRecord` 流转
9. `ActionPlan` 编译与签名
10. `DispatchActionPlan` 下发
11. `ActionPlanAck` 接收
12. `ReportActionResult` 接收
13. 审计归档与查询

如果这条链路未打通，不应宣称控制中心已经成立。

---

## 15. 当前决定

当前阶段固定以下结论：

- 控制中心是中心节点中的独立核心子系统
- 控制中心第一版优先做控制闭环，不优先做大而全平台
- 逻辑边界先拆清，物理部署允许单体起步
- 第一版物理部署推荐 all-in-one 管理平台容器
- 管理平台容器通过安装 URL 引导 agent 接入，不要求 agent 端口扫描
- 中心编译签名、边缘执行回报的职责分工必须保持不变
