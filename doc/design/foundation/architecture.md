# warp-insight 架构设计

相关设计文档：

- [`target.md`](target.md)：目标定义、分层边界、非功能约束和阶段目标
- [`glossary.md`](glossary.md)：统一术语词典和推荐用法
- [`security-model.md`](security-model.md)：三进程信任边界、最小权限、审批链和审计链
- [`error-handling-system.md`](error-handling-system.md)：Rust 错误模型、协议错误投影、故障裁决和观测暴露
- [`action-plan-schema.md`](../execution/action-plan-schema.md)：`ActionPlan` 的字段级协议定义
- [`action-result-schema.md`](../execution/action-result-schema.md)：`ActionResult` 和 `StepActionRecord` 的字段级协议定义
- [`agentd-architecture.md`](../../crates/wist-agentd/docs/agentd-architecture.md)：`warp-insightd` 的模块边界、本地状态机和调度职责
- [`agentd-failure-handling.md`](../../crates/wist-agentd/docs/agentd-failure-handling.md)：`warp-insightd` 的故障分层、状态权威性、恢复和 health 口径
- [`agentd-exec-protocol.md`](../../crates/wist-agentd/docs/agentd-exec-protocol.md)：`warp-insightd` 与 `wist-exec` 的本地交互协议
- [`agentd-state-and-boundaries.md`](../../crates/wist-agentd/docs/agentd-state-and-boundaries.md)：`warp-insightd` 的本地状态模型、唯一写入权和模块协作边界
- [`agentd-state-schema.md`](../../crates/wist-agentd/docs/agentd-state-schema.md)：`warp-insightd` 本地状态对象的字段级 schema
- [`agentd-events.md`](../../crates/wist-agentd/docs/agentd-events.md)：`warp-insightd` 进程内事件对象和模块事件流
- [`capability-report-schema.md`](../../crates/wist-agentd/docs/capability-report-schema.md)：agent 能力声明和匹配规则
- [`agent-config-schema.md`](../../crates/wist-agentd/docs/agent-config-schema.md)：`warp-insightd` 总配置骨架
- [`error-codes.md`](../edge/error-codes.md)：统一错误码和原因码词典
- [`self-observability.md`](../../crates/wist-agentd/docs/self-observability.md)：`warp-insight` 自身可观测性和验收指标面设计
- [`non-functional-targets.md`](non-functional-targets.md)：资源预算、退化阈值、buffer/backpressure 和保底目标
- [`metrics-integration-roadmap.md`](../telemetry/metrics-integration-roadmap.md)：metrics integration 的目标分层、优先级和批次规划
- [`metrics-batch-a-plan.md`](../telemetry/metrics-batch-a-plan.md)：Batch A 的最小 target 覆盖、指标范围和统一配置骨架
- [`metrics-config-schema.md`](../telemetry/metrics-config-schema.md)：metrics integration 的统一配置结构和公共字段
- [`metrics-batch-a-specs.md`](../telemetry/metrics-batch-a-specs.md)：Batch A 核心 integration 的规格草案
- [`container-metrics-spec.md`](../telemetry/container-metrics-spec.md)：`container_metrics` 的字段、发现和资源绑定规格
- [`k8s-node-pod-metrics-spec.md`](../telemetry/k8s-node-pod-metrics-spec.md)：`k8s_node_pod_metrics` 的字段、发现和资源绑定规格
- [`metrics-discovery-and-resource-mapping.md`](../telemetry/metrics-discovery-and-resource-mapping.md)：metrics discovery 和 resource mapping 的公共规则
- [`telemetry-uplink-and-warp-parse.md`](../telemetry/telemetry-uplink-and-warp-parse.md)：数据面上报目标、`warp-parse` 角色边界和统一接收器设计
- [`action-dsl.md`](../execution/action-dsl.md)：动作作者 DSL、执行 IR、opcode 白名单和编译边界
- [`control-plane.md`](../center/control-plane.md)：控制对象、状态机、下发协议与编排闭环
- [`control-center-architecture.md`](../center/control-center-architecture.md)：中心控制系统的模块边界、存储、通路与部署形态
- [`control-center-storage-schema.md`](../center/control-center-storage-schema.md)：控制中心主存储对象、主键、索引与对象存储边界
- [`agent-gateway-protocol.md`](../center/agent-gateway-protocol.md)：中心与 `warp-insightd` 的南向会话和消息协议
- [`dispatch-action-plan-schema.md`](../center/dispatch-action-plan-schema.md)：中心向边缘投递 `ActionPlan` 的 envelope schema
- [`ack-action-plan-schema.md`](../center/ack-action-plan-schema.md)：边缘对计划投递的接收确认 schema
- [`report-action-result-schema.md`](../center/report-action-result-schema.md)：边缘向中心回报最终结果的 envelope schema

## 1. 文档目的

本文档在 [`target.md`](target.md) 的目标定义基础上，进一步回答以下问题：

- `warp-insight` 应拆成哪些核心模块
- 环境内 agent 与中心节点之间如何分工
- 数据平面与控制平面如何解耦
- 升级、远程执行、资源发现、多信号关联应落在哪一层
- 如何在满足 OpenTelemetry 基线的同时，坚持“业务优先、低资源、可退化、可治理”的非功能约束

本文档只定义第一版总体架构，不展开到接口字段级别和数据库表结构级别。

---

## 2. 架构原则

`warp-insight` 的第一版架构必须同时满足以下原则：

- 两层分离：
  环境内 agent 与中心节点必须职责清晰，不能重新耦合成一个“大一统进程”
- 数据面与控制面分离：
  数据采集、标准化、缓冲、上送是一条主通路；策略、升级、远程执行、审计是另一条主通路
- OpenTelemetry 优先：
  协议、资源语义、信号模型、字段命名优先对齐 OTel
- 业务优先：
  agent 永远不能为了观测而压过业务负载
- 资源硬封顶：
  CPU、内存、buffer、线程、连接都必须有明确上限
- 默认可退化：
  资源紧张、网络异常、中心不可达时，优先退化观测能力而不是拖垮宿主机
- 中心可选：
  中心节点用于治理、编排和全局分析，但不是边缘数据面运行的前置条件
- standalone 先验证：
  第一版实现顺序必须先验证 `standalone` 可独立成立，再叠加 `managed` 接入路径
- 中心治理：
  AI、统一策略、升级编排、远程执行、审计、权限都放到中心节点
- 边缘确定性：
  环境内 agent 不依赖 AI 推理即可稳定完成核心任务

---

## 3. 总体架构

### 3.1 分层概览

`warp-insight` 由两层组成：

- 环境内 agent：
  部署在主机、容器、Kubernetes 节点或目标环境中，负责采集、发现、标准化、缓冲、上送，以及受控执行升级和远程动作
- 中心节点：
  部署在中心端，负责控制平面、分析平面和治理平面

可以用如下逻辑视图理解：

```text
+--------------------------------------------------------------+
|                         Center Node                          |
|                                                              |
|  +------------------+   +------------------+                |
|  | Control API      |   | Policy / Orches. |                |
|  +------------------+   +------------------+                |
|  +------------------+   +------------------+                |
|  | Ingest Gateway   |   | Resource Catalog |                |
|  +------------------+   +------------------+                |
|  +------------------+   +------------------+                |
|  | Correlation      |   | Query / Search   |                |
|  +------------------+   +------------------+                |
|  +------------------+   +------------------+                |
|  | Audit / Risk     |   | AI Copilot       |                |
|  +------------------+   +------------------+                |
+-------------------------------^------------------------------+
                                |
                        Data plane + Control plane
                                |
+-------------------------------v------------------------------+
|                     Environment Agent                        |
|                                                              |
|  +------------------+   +------------------+                |
|  | Inputs           |   | Discovery        |                |
|  +------------------+   +------------------+                |
|  +------------------+   +------------------+                |
|  | Normalize / OTel |   | Event Builder    |                |
|  +------------------+   +------------------+                |
|  +------------------+   +------------------+                |
|  | Buffer / Spool   |   | Exporter         |                |
|  +------------------+   +------------------+                |
|  +------------------+   +------------------+                |
|  | Upgrade Exec     |   | Remote Action    |                |
|  +------------------+   +------------------+                |
+--------------------------------------------------------------+
```

### 3.2 两条主通路

系统内部应明确区分两条主通路。

第一条是数据平面：

- 输入接入
- 解析与标准化
- 资源引用建立
- 统一事件封装
- 本地缓冲
- 可靠上送

第二条是控制平面：

- agent 注册
- 配置 / 策略下发
- 能力协商
- 升级编排
- 远程动作编排
- 权限与审计
- 健康与状态回报

关键约束是：

- 控制面异常不应直接阻断数据面本地采集
- 数据面拥塞时不应让控制面完全失联
- 没有中心节点时，边缘数据面仍应继续独立工作
- 没有中心节点时，依赖中心编排的远程任务默认不可用
- AI 只能影响中心节点的建议和编排，不能直接成为边缘热路径依赖

---

## 4. 部署形态

### 4.1 环境内 agent 部署形态

第一阶段建议支持以下部署形态：

- 主机常驻进程
- Kubernetes DaemonSet
- 容器内 sidecar 或宿主节点代理

无论哪种形态，环境内 agent 都应保持同一套逻辑架构，只允许在输入源、发现器和权限模型上有差异。

### 4.2 中心节点部署形态

中心节点建议按逻辑服务拆分，但可以在早期以单体部署方式交付：

- 控制 API
- 数据接入网关
- 编排服务
- 资源目录与关联服务
- 查询服务
- 审计与风险控制服务
- AI 服务

早期可以先单体实现，后续再按负载和隔离需求拆分成多个服务。

### 4.3 运行模式

`warp-insight` 第一版应明确支持两种运行模式：

- `standalone`
  没有中心控制节点，边缘本地完成采集、发现、标准化、缓冲和上送
- `managed`
  接入中心控制节点，在 `standalone` 基础上增加策略、远程任务、升级编排和集中治理

两种模式的边界应固定为：

- 数据面能力两者都可用
- 本地状态、自观测、保护模式两者都可用
- 远程任务只在 `managed` 模式下可用
- 中心编排升级只在 `managed` 模式下可用

同时，运行模式不只是部署选项，还应作为实现顺序的验证门：

- `standalone` 必须先成为独立成立的运行模式，而不是 `managed` 模式删掉中心连接后的降级残留
- 第一阶段应先验证一条 `standalone` 文件日志替代切片
- 该切片至少覆盖 `file input -> parser / multiline -> checkpoint / commit point -> buffer / spool -> warp-parse/file output`
- 该切片成立后，再叠加 enrollment、gateway session、远程任务和中心治理
- 因此 `managed` 是在 `standalone` 基线之上叠加的治理模式，而不是先行定义产品价值的入口

---

## 5. 环境内 Agent 架构

### 5.1 模块组成

环境内 agent 建议拆成以下模块：

- Bootstrap Manager：
  负责首次安装、环境探测、启动顺序、工作目录初始化、版本信息暴露
- Runtime Supervisor：
  负责子模块生命周期、健康检查、故障隔离、限流、退化切换
- Config & Policy Runtime：
  负责接收中心策略、本地落盘、版本切换和最小动态重载
- Input Adapters：
  负责文件、syslog、OTLP、系统日志、容器日志、Kubernetes 事件等输入
- Metrics Collectors / Scrapers：
  负责主机、进程、容器、Kubernetes，以及常见目标的 Prometheus/OpenMetrics、OTLP、JMX、SQL、HTTP status 等 metric 采集
- Discovery Engine：
  负责 host、process、container、pod、service 等资源发现
- Normalize Pipeline：
  负责解析、字段抽取、OTel 对齐、标签附加、规则驱动转换
- Event Builder：
  负责建立统一原始事件封装、生成 `event_id`、补充 `resource_refs` 和 `correlation`
- Buffer / Spool Manager：
  负责内存队列、磁盘缓冲、重试、水位控制、背压和保护模式
- Exporter：
  负责向中心节点上送 OTLP 数据和扩展事件
- Self Observability：
  负责 agent 自身指标、状态、日志、退化信号、drop 统计
- Upgrade Executor：
  负责升级包下载、签名校验、预检查、切换、健康检查和回滚
- Remote Action Executor：
  负责执行中心下发的受控动作，并返回结果

这里还应明确一个架构方向：

- `warp-insightd` 要尽量内建大多数常见目标的 metrics 采集能力
- “先部署各种 exporter，再让 agent 去抓” 不应成为默认路线
- 对少数专有系统、历史系统或短期不值得自研的目标，才保留 exporter compatibility mode

### 5.2 模块边界

环境内 agent 内部应坚持以下边界：

- 输入模块只负责读取，不负责复杂治理决策
- metrics collector 模块只负责受控采集，不通过远程 action 临时执行命令拿指标
- 发现模块只负责本地事实发现，不负责全局资源归并
- 标准化模块只负责当前事件转换，不承担跨节点全局关联
- Buffer 模块只负责本地可靠性，不承担全局消息编排
- 升级和远程执行模块只负责执行，不负责策略制定

换句话说，环境内 agent 是执行面，不是全局决策面。

### 5.3 常见 metrics 采集与 exporter 策略

`warp-insight` 在 metrics 侧的目标，不应只是“兼容 exporter 生态”，而应是：

- 对大多数常见目标直接内建采集能力
- 尽量减少额外 exporter 的安装、升级和运维成本
- 在统一资源上下文下直接产出标准化 metrics

第一版建议优先覆盖三类目标：

- 主机与运行时：
  host、process、filesystem、network、container、Kubernetes node/pod
- 标准暴露接口：
  Prometheus/OpenMetrics endpoint、OTLP metrics、StatsD、JMX
- 常见中间件 / 服务：
  nginx、mysql、postgresql、redis、kafka、elasticsearch 等

对这类目标，推荐模型是：

- `warp-insightd` 内建 collector / scraper / receiver
- 结合 discovery 自动发现 target
- 结合 normalize pipeline 直接做 OTel 对齐和资源绑定

只有在以下场景下，才保留外部 exporter 兼容模式：

- 专有协议
- 厂商私有系统
- 历史遗留目标
- 短期不值得自研 collector 的目标

### 5.4 AI 对采集集成开发的作用边界

在当前 AI 条件下，常见数据采集 integration 的开发门槛已经显著下降。

AI 更适合用于：

- 生成常见中间件 collector 骨架
- 生成字段映射和 OTel semantic convention 对齐草案
- 生成 fixture、sample payload、测试样例和回归用例
- 辅助快速补齐常见 target 的 discovery 与 scrape 配置模板

这意味着从研发组织视角看：

- 支持大多数常见 metrics 数据采集，应被视为可持续、可规模化推进的常规工程工作

但必须明确：

- AI 加速的是研发和中心侧治理
- 不是让边缘 agent 在运行时依赖 AI 推理
- 也不是让远程 action 成为 metrics 采集主路径

### 5.5 进程模型建议

除了逻辑模块分离，环境内 agent 还应在进程模型上做最小但明确的隔离。

建议默认采用三类进程：

- `warp-insightd`：
  常驻主进程，负责采集、发现、标准化、统一事件封装、buffer、上送、心跳、策略接收和本地保护状态机
- `wist-exec`：
  按需拉起的动作执行进程，负责执行远程诊断、只读命令、服务控制类动作和其他受控动作
- `wist-upgrader`：
  按需拉起的升级辅助进程，负责升级包下载、校验、切换、健康探测和回滚

这三个进程的职责边界应明确如下：

- `warp-insightd` 不直接承载高风险远程动作执行逻辑
- `warp-insightd` 不直接在自身进程内完成自我替换式升级
- `wist-exec` 不负责采集、发现、buffer 和上送
- `wist-upgrader` 不负责常驻采集任务和远程动作编排

之所以这样拆分，是因为 `target.md` 已经把以下要求定义成硬约束：

- 业务优先
- 故障隔离
- 资源封顶
- 可审计
- 升级与远程执行属于高风险动作

如果把采集热路径、远程动作执行、自升级都塞进同一个常驻进程，会直接放大以下风险：

- 远程动作卡死或子进程泄漏，拖垮采集主进程
- 高权限执行路径和普通采集路径混在一起，权限边界不清
- 自升级过程中难以安全替换正在运行的主进程
- 审计链路、超时控制、资源限制难以独立实现

因此这里的建议不是“把所有能力都拆成微服务”，而是：

- 数据面常驻能力尽量收敛在一个轻量 daemon
- 高风险执行能力采用独立执行器进程
- 升级能力采用独立升级器进程

这是一种面向边缘环境的最小进程隔离模型。

### 5.6 进程间协作模型

第一版建议由 `warp-insightd` 作为本地唯一协调者：

- 接收中心节点策略、升级计划和远程动作计划
- 校验本地状态是否允许执行
- 拉起 `wist-exec` 或 `wist-upgrader`
- 传递最小必要参数和受限执行上下文
- 回收执行结果、状态码和审计元数据
- 对执行超时、异常退出和资源超限进行统一治理

建议协作关系如下：

```text
Center Node
    |
    v
warp-insightd
  |---- spawn ----> wist-exec
  |---- spawn ----> wist-upgrader
  |
  +---- collect / discover / buffer / export
```

关键原则：

- 中心节点不直接控制本地执行器进程
- 本地执行器不直接长连中心节点做自由编排
- 所有动作都通过 `warp-insightd` 统一受理、编号、审计和状态回报

这样可以把本地控制面收口到一个确定性入口，避免边缘侧控制面失序。

### 5.7 进程隔离的具体收益

把 daemon 与执行器分离，至少有以下直接收益：

- 故障隔离：
  执行器崩溃、阻塞、输出失控，不应影响采集 pipeline 常驻运行
- 权限隔离：
  可针对执行器和升级器分别授予更小的能力集合
- 资源隔离：
  可针对执行器单独设置 CPU、内存、并发、超时和输出上限
- 升级可实现性：
  使用独立升级器比主进程自替换更容易实现可靠切换和失败回滚
- 审计清晰：
  `warp-insightd -> executor/upgrader -> result` 的调用链更容易形成稳定审计记录

### 5.8 进程模型约束

为了避免“为了隔离而过度拆分”，还需要补充以下约束：

- 不建议把每个采集 pipeline 都拆成独立常驻进程
- 不建议把发现模块拆成多个独立守护进程
- 不建议第一版做本地多服务网格
- 常驻进程数量应尽量少，默认应控制在一个主 daemon 加少量按需子进程的规模
- 执行器和升级器应默认按需拉起、执行完退出，而不是长期常驻

也就是说，`warp-insight` 追求的是“高风险能力隔离”，不是“边缘侧服务化泛滥”。

### 5.9 单机数据流

环境内 agent 的标准数据流建议为：

```text
Input -> Parse -> Normalize -> Attach Resource Ref -> Build Event Envelope
      -> Local Queue -> Buffer / Spool -> Export -> Ack / Retry / Backpressure
```

这里有几个关键点：

- `raw` 必须尽量保留，以支撑审计和后续再解释
- `normalized` 必须尽量向 OTel 对齐
- `resource_refs` 应在第一跳尽量建立
- `event_id` 应在边缘生成，避免中心端二次分配导致主线漂移

### 5.10 运行时调度模型

环境内 agent 需要采用轻量、受控的调度模型。

第一版建议：

- 以 pipeline 为核心调度单元
- 每个 pipeline 有独立队列、水位和限流状态
- 高优先级信号和低优先级信号分开调度
- 发现任务、采集任务、上送任务相互隔离
- 升级执行和远程执行与采集热路径隔离

这样做的目的是避免任一高成本任务拖垮整机 agent。

### 5.11 本地状态

环境内 agent 本地需要保存的状态应尽量少而明确：

- agent 身份和节点身份
- 当前生效配置版本
- 本地资源快照缓存
- buffer / spool 元数据
- 升级状态
- 远程动作执行记录
- 审计日志和关键保护事件

本地状态原则：

- 只保留环境执行所必需的状态
- 优先可恢复，不追求在边缘维护复杂长期历史
- 关键状态必须可校验、可清理、可恢复

---

## 6. 中心节点架构

### 6.1 模块组成

中心节点建议拆成以下逻辑模块：

- Agent Registry：
  维护 agent 注册、身份、版本、能力和租约状态
- Control API：
  提供配置、策略、升级、远程动作、状态查询和治理接口
- Policy Engine：
  维护采集策略、发现策略、限流策略、数据优先级和退化规则
- Ingest Gateway：
  接收来自 agent 的 OTLP 数据、扩展事件和状态回报
- Resource Catalog：
  汇聚并归并全局资源对象，维护资源生命周期和关系图
- Event Correlator：
  围绕 `event_id`、`resource_refs`、`trace_id` 等键做统一关联
- Signal Derivation Engine：
  从原始事件主线派生或关联 `Logs`、`Security`、`Metrics`、`Traces`
- Query & Retrieval：
  提供按事件主轴、资源主轴、时间范围、环境范围的统一查询能力
- Upgrade Orchestrator：
  负责升级计划、批次控制、分阶段发布、失败暂停和回滚协调
- Remote Action Orchestrator：
  负责远程动作审批、下发、超时、中断、结果回收和审计
- Audit & Risk Engine：
  负责权限校验、策略审计、风险等级、异常动作检测和留痕
- OTel Governance：
  负责 schema 检查、字段映射建议、命名治理和兼容性校验
- AI Copilot：
  负责异常解释、规则建议、升级建议、诊断建议和知识增强

### 6.2 中心节点的角色定位

中心节点不是简单的接收端，而是三个角色的组合：

- 控制平面：
  下发策略、编排升级、编排远程动作、管理权限
- 分析平面：
  做全局关联、资源图构建、跨信号分析和统一查询
- 治理平面：
  做 OTel 对齐、审计、风险控制和 AI 辅助治理

### 6.3 中心节点内部数据分层

中心节点内部建议把数据分成四层：

- 接入层：
  接收 agent 上送数据和状态
- 事实层：
  保留原始事件、标准化事件、资源事实、执行事实
- 关联层：
  构建事件主线、资源关系、跨信号关联、派生结果
- 服务层：
  对外提供查询、检索、策略、编排、AI 辅助

这样可以避免“接入格式”和“查询视图”过度耦合。

---

## 7. 关键对象模型

### 7.1 Agent 对象

中心节点至少需要维护以下 agent 元数据：

- `agent_id`
- `tenant_id`
- `environment_id`
- `node_id`
- `version`
- `capabilities`
- `last_seen`
- `health_state`
- `policy_version`
- `upgrade_state`

### 7.2 Resource 对象

资源对象应遵循 [`target.md`](target.md) 中统一资源模型的方向，并优先兼容 OTel Resource 语义。

核心字段包括：

- `resource_uid`
- `resource_type`
- `attributes`
- `owner_refs`
- `runtime`
- `state`
- `valid_from`
- `valid_to`

### 7.3 Event 对象

事件对象应以统一事件封装为中心，至少包括：

- `event_id`
- `collector_id`
- `source_type`
- `observed_time`
- `ingest_time`
- `resource_refs`
- `correlation`
- `raw`
- `normalized`

### 7.4 控制对象

为了支撑控制平面，建议引入以下控制对象：

- `PolicySet`：
  描述采集、发现、优先级、限流和退化策略
- `UpgradePlan`：
  描述目标版本、适用范围、发布批次、健康门槛、回滚条件
- `ActionPlan`：
  描述目标节点、控制约束、执行程序、审批绑定、超时与执行限制
- `ActionResult`：
  描述远程动作输出、退出码、耗时、执行节点和审计元数据

---

## 8. 关键流程设计

### 8.1 Agent 首次注册流程

首次安装后，环境内 agent 应执行如下流程：

1. 生成或获取节点身份
2. 建立与中心节点的受信连接
3. 上报自身版本、能力、运行环境和最小资源事实
4. 拉取初始策略与配置
5. 启动发现器和采集 pipeline
6. 周期上报心跳、健康、版本和水位状态

### 8.2 数据采集与上送流程

标准流程建议为：

1. 输入模块接收原始数据
2. 解析模块完成基础结构化
3. 标准化模块尽量映射到 OTel 风格对象
4. 发现模块提供可复用的本地资源引用
5. Event Builder 生成统一事件封装
6. Buffer / Spool 负责本地可靠缓存
7. Exporter 发送到中心节点
8. 中心节点完成接收、关联、派生、入库和查询索引

### 8.3 资源发现与资源目录同步

资源发现建议分成两层：

- 边缘发现：
  只负责本地事实采集和本地 `resource_ref` 建立
- 中心归并：
  负责去重、合并、生命周期管理和跨节点资源关系

这样可以避免边缘 agent 为了全局一致性承担过多状态复杂度。

### 8.4 升级流程

升级必须由中心节点主导，边缘 agent 执行。

建议流程为：

1. 中心创建 `UpgradePlan`
2. 系统校验目标 agent 能力和版本兼容性
3. 按环境、批次、风险等级分阶段下发
4. agent 下载升级包并校验签名 / 摘要
5. agent 做本地预检查
6. agent 进入平滑切换或最小中断切换
7. agent 回报健康结果
8. 中心根据健康门槛决定继续、暂停或回滚

关键要求：

- 升级过程可中止
- 升级过程可审计
- 升级失败可自动回滚
- 升级期间不应默认丢失关键缓冲数据

### 8.5 远程动作执行流程

远程动作必须走中心治理，不允许边缘自由执行。

建议流程为：

1. 用户或自动化系统在中心提出动作请求
2. 权限系统校验租户、环境、节点、角色和动作白名单
3. 高风险动作进入审批流
4. 中心节点编译并签名 `ActionPlan`
5. 中心节点将动作分发到目标 agent
6. agent 在本地最小权限上下文中执行
7. agent 回传标准输出、标准错误、退出码和执行元数据
8. 中心归档结果并写入审计记录

远程动作模型建议优先支持：

- 诊断类动作
- 只读类动作
- 服务控制类动作
- 有明确定义输入输出的运维动作

不建议第一版直接支持任意 shell。

### 8.6 退化与保护模式流程

当 agent 检测到 CPU、内存、buffer 或中心连通性异常时，应自动进入保护流程：

1. 上报当前压力和水位状态
2. 关闭高成本增强能力
3. 启动局部 backpressure
4. 停止低优先级派生或主动发现任务
5. 必要时对低优先级输入执行采样或丢弃
6. 条件恢复后自动退出保护模式

中心节点需要能看到每个 agent 当前是否处于：

- 正常
- 降级
- 保护
- 丢弃
- 升级中
- 远程执行中

---

## 9. 协议与通信面

### 9.1 数据平面协议

第一阶段建议：

- 输入优先支持 OTLP
- 上送优先支持 OTLP
- 对统一事件封装和控制状态回报，可在 OTLP 基础上扩展事件类型或增加独立控制接口

设计原则：

- 优先兼容标准，而不是优先自造协议
- 确有必要时才在标准外增加扩展对象
- 扩展对象必须与 OTel 语义保持可映射关系

### 9.2 控制平面协议

控制平面至少需要支持以下语义：

- 注册 / 心跳
- 配置拉取或订阅
- 状态回报
- 升级命令
- 远程动作命令
- 结果回传
- 审计事件上报

控制平面可以是 gRPC 或 HTTP API，但必须满足：

- 双向身份认证
- 幂等重试
- 明确超时
- 断线恢复
- 版本协商

---

## 10. 安全与治理架构

### 10.1 身份与信任

边缘 agent 与中心节点之间必须建立强身份关系。

建议要求：

- agent 持有唯一身份
- 通信使用双向认证
- 升级包必须签名校验
- 控制命令必须带来源身份和审计上下文

### 10.2 最小权限原则

环境内 agent 的权限设计必须分层：

- 采集权限
- 发现权限
- 升级权限
- 远程动作权限

远程动作执行时，必须进一步按动作类型和目标范围收缩权限。

### 10.3 审计与追责

以下行为必须天然可审计：

- 策略变更
- agent 注册与版本变化
- 升级下发与回滚
- 远程动作提交、审批、执行和结果
- 退化、限流、丢弃和保护模式切换

### 10.4 AI 的位置

AI 只能部署在中心节点，并作为增强层存在。

AI 适合参与：

- 异常归因建议
- 规则生成建议
- 升级批次建议
- 远程诊断动作建议
- schema / OTel 映射建议

AI 不应直接拥有：

- 边缘节点自由执行权
- 绕过审批的高风险动作下发权
- 替代确定性规则的最终控制权

---

## 11. 非功能约束如何映射到架构

为了满足 [`target.md`](target.md) 中定义的“业务优先”和“世界第一水准”非功能目标，架构上必须做以下落地：

- 采集、发现、上送、升级、远程执行必须分模块隔离，避免单点高成本任务拖垮整机
- 所有队列和 buffer 必须有硬上限，避免无限堆积
- agent 必须能在中心不可达时继续本地有限采集，但不能无限增长本地成本
- agent 必须能在没有中心节点时长期稳定运行数据面，而不是只支持“中心短时不可达”
- 高优先级信号和低优先级信号必须支持优先级分层
- 升级和远程执行与采集热路径隔离，避免控制动作压制数据面
- 退化和保护模式必须是架构内建能力，不是事后补丁
- 中心节点必须能观测每个 agent 的资源状态、水位状态和保护状态

---

## 12. 第一阶段推荐实现顺序

### 12.1 Phase A

先打通 `standalone` 边缘运行基线：

- `warp-insightd` / `wist-exec` / `wist-upgrader` 三进程骨架
- 本地工作目录、状态目录和恢复骨架
- `standalone` 运行模式
- 最小自观测、错误码和 health 基线

### 12.2 Phase B

再打通 `standalone` 可替代切片：

- 受控单路径文件日志输入
- parser / multiline
- `checkpoint / commit point`
- 本地 buffer / spool
- `warp-parse` 或本地 file output
- rotate / truncate / restart recovery
- 一类真实 `Fluent Bit tail input` 替代场景验证

此阶段不要求先完成通用 discovery / watcher / protect 产品形态。

### 12.3 Phase C

在 `standalone` 切片成立后，再叠加 `managed` 接入基线：

- agent 注册
- 中心接收
- 基础资源目录
- enrollment / identity
- gateway session / capability report

### 12.4 Phase D

补齐治理骨架：

- 策略下发
- 降级 / backpressure 状态机
- 统一审计事件
- 升级协议
- 升级回滚机制

### 12.5 Phase E

补齐高风险控制面：

- 远程动作模型
- 权限 / 审批 / 审计
- 动作并发控制
- 风险分级

### 12.6 Phase F

补齐高阶分析平面：

- 全局资源关系图
- 多信号关联
- OTel 治理
- AI 辅助解释与建议

---

## 13. 本文档不展开的内容

以下内容应在后续子文档中继续展开：

- 控制平面协议细节
- 统一事件模型字段级设计
- 统一资源模型字段级设计
- 升级协议与包格式
- 远程动作白名单模型
- 审批与 RBAC 模型
- 多租户隔离模型
- 存储与索引设计
- AI 辅助流程与人机闭环

---

## 14. 当前架构结论

第一版 `warp-insight` 架构可以总结为：

- 边缘 agent 是轻量执行面，负责采集、发现、标准化、缓冲、上送，以及受控执行升级和远程动作
- 中心节点是控制、分析和治理中心，负责策略、资源目录、统一关联、升级编排、远程动作编排、审计和 AI 辅助
- 第一版实现顺序必须先让 `standalone` 独立成立，并先验证一条可替代部分 `Fluent Bit` 工作的最小切片，再叠加 `managed` 接入
- 数据平面与控制平面必须明确分离，但共享统一身份、统一资源模型和统一审计链路
- OpenTelemetry 是协议和语义的优先基线
- 非功能目标不是附属要求，而是第一架构约束

如果后续实现偏离以上五点，`warp-insight` 很容易重新滑回“重边缘、弱治理、强耦合”的错误方向。
