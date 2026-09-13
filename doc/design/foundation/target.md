# warp-insight 目标定义

相关设计文档：

- [`architecture.md`](architecture.md)：总体架构、模块边界、数据面 / 控制面分层、升级与远程执行流程
- [`glossary.md`](glossary.md)：统一术语词典、推荐用法、禁止混用词和作者侧/执行侧术语边界
- [`security-model.md`](security-model.md)：身份、权限、审批、审计、升级与远程执行安全边界
- [`action-plan-ir.md`](../execution/action-plan-ir.md)：边缘唯一执行契约、IR 对象模型、步骤语义、结果模型与校验规则
- [`action-plan-schema.md`](../execution/action-plan-schema.md)：`ActionPlan v1` 的字段级 schema、步骤类型与约束定义
- [`action-result-schema.md`](../execution/action-result-schema.md)：`ActionResult v1` 和 `StepActionRecord` 的字段级 schema
- [`agentd-architecture.md`](../../crates/wist-agentd/docs/agentd-architecture.md)：`warp-insightd` 模块拆分、本地状态机、调度职责与进程边界
- [`agentd-exec-protocol.md`](../../crates/wist-agentd/docs/agentd-exec-protocol.md)：`warp-insightd` 与 `wist-exec` 的本地协议、工作目录、状态文件与取消机制
- [`agentd-state-and-boundaries.md`](../../crates/wist-agentd/docs/agentd-state-and-boundaries.md)：`warp-insightd` 的本地状态分层、唯一写入权、状态机拥有者与模块协作边界
- [`agentd-state-schema.md`](../../crates/wist-agentd/docs/agentd-state-schema.md)：`warp-insightd` 本地状态文件的字段级 schema 草案
- [`agentd-events.md`](../../crates/wist-agentd/docs/agentd-events.md)：`warp-insightd` 模块协作事件对象、最小事件流与审计事件边界
- [`capability-report-schema.md`](../../crates/wist-agentd/docs/capability-report-schema.md)：agent 能力声明、能力匹配和限制字段的 schema 草案
- [`agent-config-schema.md`](../../crates/wist-agentd/docs/agent-config-schema.md)：`warp-insightd` 本地总配置骨架和各配置段结构
- [`error-codes.md`](../edge/error-codes.md)：统一错误码、原因码和使用建议
- [`self-observability.md`](../../crates/wist-agentd/docs/self-observability.md)：`warp-insight` 自身的 metrics、logs、events 与验收观测面设计
- [`metrics-integration-roadmap.md`](../telemetry/metrics-integration-roadmap.md)：metrics integration 覆盖范围、优先级、分批落地与 exporter 策略
- [`metrics-batch-a-plan.md`](../telemetry/metrics-batch-a-plan.md)：Batch A 的 target 范围、最小指标集、resource 绑定与验收标准
- [`metrics-config-schema.md`](../telemetry/metrics-config-schema.md)：metrics integration 的统一配置骨架、通用字段与 budget 结构
- [`metrics-batch-a-specs.md`](../telemetry/metrics-batch-a-specs.md)：`host_metrics`、`process_metrics`、`prom_scrape`、`otlp_metrics_receiver` 的规格草案
- [`metrics-discovery-and-resource-mapping.md`](../telemetry/metrics-discovery-and-resource-mapping.md)：metrics target discovery、去重、resource binding 与高基数控制规则
- [`action-dsl.md`](../execution/action-dsl.md)：中心侧动作 DSL、边缘执行 IR、opcode 白名单和审批绑定
- [`control-plane.md`](../center/control-plane.md)：控制对象、生命周期状态机、中心到边缘协议与编排闭环
- [`control-center-architecture.md`](../center/control-center-architecture.md)：控制中心的逻辑模块、状态主线、存储和部署建议
- [`control-center-storage-schema.md`](../center/control-center-storage-schema.md)：控制中心的关系库、对象存储和幂等键设计
- [`agent-gateway-protocol.md`](../center/agent-gateway-protocol.md)：控制中心与边缘 agent 的会话、心跳、下发、ack、结果上报协议
- [`roadmap.md`](roadmap.md)：多里程碑开发路线图、阶段目标、验收标准与依赖关系
- [`run-gxl-subset.md`](../execution/run-gxl-subset.md)：`run.gxl` 的受限语法子集、AST、opcode 映射与编译边界
- [`run-gxl-construct-mapping.md`](../execution/run-gxl-construct-mapping.md)：`step/when/expect/emit/fail/retry` 与当前 GXL 的映射和实现策略
- [`action-schema.md`](../execution/action-schema.md)：各 opcode 的参数 schema、返回 schema、风险、限制与 capability
- [`references.md`](references.md)：可借鉴的业界产品与设计模式参考

## 1. 当前目标

`warp-insight` 当前阶段的目标，不是做一个“基于 Fluent Bit 的封装层”，也不是把 AI 直接塞进部署在环境内的采集代理里。

当前目标应明确为：

构建一套由“环境内轻量代理 + 中心节点控制平面”组成的统一观测系统。

其中：

- 环境内代理负责统一安装、资产/资源发现、标准化采集、可靠缓冲和可靠上送
- 环境内代理负责执行中心节点下发的升级与远程执行任务
- 中心节点负责全局资源建模、多信号关联、OpenTelemetry 对齐治理，以及结合 AI 进行事件理解、异常解释、规则建议与操作辅助
- 中心节点负责统一编排升级、远程执行、权限控制、审计与回滚策略

这套系统需要同时满足以下几个方向：

- 自研实现与现有成熟采集代理相近的核心能力
- 提供内置安装、升级、服务托管与回滚能力
- 支持环境内 agent 的自我升级
- 支持受控的远程指令执行能力
- `warp-insightd` 在没有中心控制节点时也能独立工作，至少应继续完成本地采集、发现、标准化、缓冲与上送
- 自动发现目标环境中的资产与资源
- 在常见目标上由 `warp-insightd` 直接完成 metric 采集，而不是默认要求额外 exporter 部署
- 以同一份采集、同一份原始事件为主线
- 在统一资源上下文下组织 `Metrics`、`Logs`、`Traces`、`Security`
- 以 OpenTelemetry 作为协议、资源模型、信号模型和字段语义的优先标准基线
- 将 AI 能力放在中心节点，而不是放在环境内代理的热路径里
- 在环境内 agent 上保持低 CPU 与低内存占用

一句话概括：

`warp-insight` 要做的是一个“边缘轻代理 + 中心智能控制平面”的统一环境观测系统，并在边缘提供受控的自我升级与远程执行能力。

还要补充一个明确约束：

- 中心节点是增强治理能力，不是边缘数据面存活的前提
- 没有中心节点时，`warp-insightd` 仍应作为独立 agent 正常运行
- 没有中心节点时，不提供依赖中心编排的远程任务能力

当前阶段还要固定一个产品验证顺序：

- 第一验证目标不是先证明 `managed` 接入完整成立
- 第一验证目标是先证明 `standalone` 模式下，`warp-insightd` 可以完成一条可替代部分 `Fluent Bit` 工作的最小纵向切片
- 这条切片应至少覆盖一个显式配置文件路径上的 `file input -> parser / multiline -> checkpoint / commit point -> buffer / spool -> warp-parse/file output`
- 对标的是能力与运行时行为，不要求兼容 `Fluent Bit` 配置格式
- `path_patterns[]`、通用 discovery / watcher、完整 `degraded / protect` 与更完整自观测属于后续 telemetry core 阶段
- 只有在这条切片成立后，`managed` 接入、中心治理和统一多信号能力才应继续前推

---

## 2. 总体分层

### 2.1 环境内 Agent

部署在主机、容器、Kubernetes 节点或目标环境内部。

主要职责：

- 安装
- 启动
- 采集
- 基础发现
- 标准化
- 缓冲
- 上送
- 自我升级
- 受控执行远程指令

目标特征：

- 轻量
- 确定性
- 低资源占用
- 可本地稳定运行
- 可脱离中心独立运行数据面主路径
- 不依赖 AI 推理才能完成核心任务
- 升级与远程执行都必须可审计、可限权、可回滚

这里的低资源占用应明确理解为一项硬约束，而不是附带优化项：

- CPU 占用要尽量低
- 内存占用要尽量低
- 资源波动要尽量小
- 在高流量下也不能因资源失控影响业务节点稳定性

同时要明确一个产品目标：

- 常见 metrics 采集应优先通过 `warp-insightd` 内建 collector / scraper / receiver 完成
- “先安装很多 exporter” 不能成为默认使用前提
- 外部 exporter 只应保留为少数兼容场景下的 fallback

### 2.2 中心节点

部署在中心端，承担控制平面与分析平面的职责。

主要职责：

- 全局资源目录
- 全局事件关联
- 多信号统一查询
- OTel 对齐治理
- AI 驱动的理解、解释、建议和辅助决策
- 统一升级编排
- 统一远程执行编排
- 权限、审计与风险控制

目标特征：

- 有全局视角
- 可接入模型与知识库
- 可做跨环境、跨资源、跨信号分析
- 可统一纳管审计与风险控制

---

## 3. 为什么要拆成两层

如果把环境内代理和中心节点混成一个目标，会出现明显问题：

- 环境内代理会被塞入过多复杂职责
- 采集热路径会受到 AI 不确定性的干扰
- 资源占用和故障面会扩大
- 需要全局数据才能完成的分析任务无法在边缘可靠落地
- 升级与远程执行缺乏集中治理，会放大安全风险

因此必须明确：

- 环境内 agent 负责“采”“送”“受控执行”
- 中心节点负责“关联”“理解”“解释”“建议”“编排”

这个边界是 `warp-insight` 是否能工程化落地的关键。

同时，这个边界还有两个直接收益：

- 把复杂分析和 AI 放到中心节点，本质上也是为了把 CPU / 内存消耗从环境内 agent 剥离出去
- 把升级和远程执行放到中心节点统一编排，本质上也是为了把高风险动作纳入统一治理

还要补充一个当前时代下的现实判断：

- 在当前 AI 条件下，常见中间件和基础设施的采集集成开发、字段映射、fixture 生成和测试样例生产，已经可以被明显加速
- 这意味着 `warp-insight` 应优先内建大多数常见目标的数据采集能力，而不是继续把“部署 exporter”作为默认路线

但这里的 AI 作用必须被限定为：

- 加速研发侧的 integration 开发
- 加速 schema、mapping、sample 和测试夹具生成

而不是：

- 让边缘 agent 在运行时依赖 AI 推理才能完成采集

---

## 4. OpenTelemetry 标准约束

`warp-insight` 必须把 OpenTelemetry 作为标准基线，而不是事后兼容项。

这里所说的“符合 OpenTelemetry 标准”，重点不是一句口号，而是四个层面的约束：

- 资源模型尽量对齐 OTel Resource 语义
- Logs / Metrics / Traces 的信号组织尽量对齐 OTel 的信号模型
- 输入输出协议优先支持 OTLP
- 字段命名、关联键和语义尽量遵循 OTel semantic conventions

这意味着：

- `host.name`、`service.name`、`service.namespace`、`deployment.environment.name` 这类属性应优先采用 OTel 风格
- trace 关联键应优先采用 `trace_id`、`span_id`
- 资源对象、事件对象和信号对象之间的关系，应优先复用 OTel 已经稳定定义的概念

同时也要明确：

- `Security` 不是 OTel 当前最成熟的标准信号类型之一，因此安全语义可以在 OTel 基线之上做扩展
- “同一份原始事件派生多信号”也是本系统的增强目标，不能被 OTel 现有抽象完全限制

因此正确原则应是：

优先对齐 OpenTelemetry，必要时在不破坏兼容性的前提下扩展。

---

## 5. 为什么不能把目标定义成“基于 Fluent Bit”

如果目标定义成“基于 Fluent Bit”，会出现几个问题：

### 5.1 核心能力被外部产品绑定

一旦把采集核心完全建立在第三方代理之上，后续很多关键能力都会受限：

- 资源发现模型受限
- 事件主线建模受限
- 多信号派生受限
- 行为一致性受限
- 安装、升级、回滚边界受限

### 5.2 很难把资源发现和统一事件主线做成一等能力

外部采集代理通常擅长：

- 数据接入
- 基础过滤
- 转发

但不一定把这些能力作为系统内核：

- 资产 / 资源自动发现
- 统一资源目录
- 事件主线 ID
- 事件与资源的强关联
- 多信号统一派生语义

而这些恰恰是 `warp-insight` 真正要做的差异化能力。

### 5.3 未来演进会被插件边界锁住

如果我们只是包装第三方代理，那么很多未来想做的事情都会退化为：

- 能不能塞进插件
- 能不能挂一个 filter
- 能不能复用某个内部结构

这会导致系统长期停留在“拼接式产品”阶段，而不是形成自己的内核。

---

## 6. 为什么 AI 不应放在环境内 Agent

AI 能力不是不要，而是不应放在环境内代理的核心热路径里。

### 6.1 环境内代理需要保持轻量和确定性

环境内 agent 常常部署在业务主机、边缘节点或 Kubernetes 节点，必须优先保证：

- CPU 占用可控
- 内存占用可控
- 网络依赖最小
- 热路径稳定

而 AI 推理天然引入：

- 更高资源消耗
- 更强外部依赖
- 更高波动性
- 更差的可复现性

### 6.2 环境内代理缺乏全局视角

很多 AI 真正高价值的任务都需要：

- 全局 logs
- 全局 metrics
- 全局 traces
- 全局 security events
- 跨资源关系
- 历史样本和知识库

这些天然更适合放到中心节点完成。

### 6.3 环境内代理权限高，AI 行为应更谨慎

环境内 agent 往往具备较高本地权限，例如：

- 读取日志
- 查看进程
- 枚举资源
- 访问本地系统信息
- 执行升级
- 执行远程指令

因此越靠近环境，越应该强调：

- 确定性
- 审计性
- 最小功能集

这决定了 AI 不应成为环境内代理的基本前提。

---

## 7. 环境内 Agent 的目标

环境内 agent 需要聚焦在以下职责。

### 7.0 非功能目标

环境内 agent 必须满足明确的非功能要求：

- 业务优先
- 低 CPU 占用
- 低内存占用
- 长时间运行下资源曲线稳定
- 高峰流量下具备可控退化行为

这里的原则是：

- 任何时候都不能为了观测而压过业务
- 默认情况下不能把业务主机当成“大资源机器”来假设
- 资源消耗必须可预算、可观测、可限制
- 不能为了增加功能不断抬高环境内 agent 的常驻成本
- agent 自身故障不能放大为业务故障
- 遇到资源紧张时必须优先退化观测能力，而不是挤占业务资源

因此设计上应优先考虑：

- 流式处理优先，避免大对象堆积
- bounded queue / bounded buffer
- 零拷贝或低拷贝路径优先
- 解析与转换尽量增量化
- 非关键增强能力可异步、可关闭、可降级

### 7.0.1 “世界第一水准”在非功能上的含义

这里的“世界第一水准”，不应理解为营销表述，而应理解为对环境内 agent 提出顶级工程要求：

- 在业务机器上长期运行，但默认不干扰业务
- 在流量暴涨、中心端异常、网络抖动、配置错误等情况下，优先自我限流和自我退化，而不是拖垮宿主机
- 在同类产品对比中，把资源效率、稳定性、确定性和可治理性做到第一梯队
- 非功能指标不是“能跑就行”，而是要作为核心竞争力设计、实现和验收

具体应落成以下硬约束：

- 业务优先：
  当业务负载与 agent 争抢 CPU、内存、磁盘 IO、网络带宽时，业务优先
- 故障隔离：
  agent 崩溃、卡死、阻塞、升级失败、中心端不可达，不应导致业务进程异常或节点失稳
- 资源封顶：
  agent 的 CPU、内存、磁盘 buffer、线程数、连接数都必须有明确上限，不能无界增长
- 可退化优先：
  允许牺牲部分增强观测能力，但不允许因为坚持“全量采集”而破坏业务稳定性
- 可恢复优先：
  退化、限流、断连、重试、升级失败后必须可以自动恢复到稳定态，而不是需要人工逐台抢修
- 可审计优先：
  每次退化、限流、丢弃、回滚、远程执行都必须有可观测记录，便于事后解释和治理

如果做不到以上几点，就不能称为“世界第一”的非功能水准。

### 7.0.2 可量化指标草案

为了避免“低 CPU / 低内存”停留在口号层面，当前先定义一版可压测、可验收的指标草案。

这组指标先基于如下假设：

- 单个环境内 agent 进程
- Linux 主机或 Kubernetes 节点场景
- 最小目标环境按 `2 vCPU / 4 GiB RAM` 估算
- 开启基础资源发现、基础解析、标准化和 OTLP 上送
- 平均原始事件大小按 `1 KiB` 估算
- CPU 以单逻辑核 `100%` 为口径
- 内存以 agent 进程稳定态 `RSS` 为口径，不把内核 page cache 计入 agent 预算

第一版建议按三档工作负载定义：

- 空闲态：
  `<= 10 EPS`，以心跳、状态上报、少量资源发现和极低流量日志为主
- 中等流量：
  `1000 EPS` 持续流量，包含解析、标准化、资源引用建立和上送
- 峰值流量：
  `5000 EPS` 持续 `5` 分钟，或中心端短时不可用、导致本地缓冲快速上升

建议目标如下：

- 空闲态目标：
  平均 CPU `<= 1%`，短时峰值 CPU `<= 3%`
- 空闲态内存：
  稳定态 RSS `<= 80 MiB`，告警阈值 `120 MiB`
- 中等流量 CPU：
  平均 CPU `<= 15%`，`P95 <= 25%`
- 中等流量内存：
  稳定态 RSS `<= 180 MiB`，上界 `256 MiB`
- 中等流量稳定性：
  预热完成后 `30` 分钟观测窗口内，RSS 漂移不应超过 `10%`
- 峰值流量 CPU：
  平均 CPU `<= 50%`，硬上限不应长期超过 `1` 个逻辑核
- 峰值流量内存：
  RSS 硬上限 `<= 384 MiB`
- 峰值流量稳定性：
  在进入退化模式后，不允许出现无界内存增长、无界任务/线程增长或因 agent 自身资源失控触发 OOM

这些数字当前应被视为第一版工程指标草案，后续需要通过压测和不同部署形态分层校准，而不是长期固定不变。

### 7.0.3 峰值流量下的允许退化策略

峰值流量下允许退化，但退化必须有明确顺序，不能退化成“资源打满直到节点失控”。

建议按以下顺序退化：

- 先关闭可选增强能力，例如高成本 enrich、低优先级标签补全、低价值派生计算
- 再降低主动发现频率，例如把高频扫描降到低频轮询
- 再扩大批量、延长 flush 间隔上限，以减少 CPU 抖动和网络放大
- 再对可再生、低优先级派生数据执行 sampling 或 drop
- 最后才对无法 backpressure 的低优先级原始输入执行丢弃策略

退化时必须遵守以下原则：

- 原始事件优先于派生结果
- 安全审计类事件优先于普通观测增强数据
- 能 backpressure 的输入优先 backpressure，而不是优先丢数据
- 所有退化动作都必须产生日志、指标和状态上报，便于中心节点审计

### 7.0.4 Buffer 上限与 Backpressure 策略草案

buffer 和 backpressure 需要从一开始就有硬边界，不能把“可靠上送”实现成“无限堆积”。

建议先定义如下上限：

- 单条 pipeline 的内存 buffer 硬上限：
  `64 MiB` 或 `60` 秒待发送数据量，两者取较小值
- 单节点 agent 的总内存 buffer 硬上限：
  `128 MiB`
- 本地磁盘 buffer 默认上限：
  `1 GiB` 或 `30` 分钟待发送数据量，两者取较小值
- 单个租户 / 数据源 / pipeline 不应独占超过总 buffer 的 `50%`

建议按水位触发分级策略：

- `70%` 水位：
  提前告警，提升 batch，暂停非关键增强任务
- `85%` 水位：
  对支持反馈控制的输入启用 backpressure，主动降低发现频率，停止低优先级派生数据生成
- `95%` 水位：
  进入保护模式，只保留高优先级原始事件和关键审计事件；低优先级派生数据允许采样或丢弃
- `100%` 水位：
  不再继续无界申请内存或磁盘；必须按优先级执行拒收、丢弃或上游限流，并产生明确告警

backpressure 策略建议明确分层：

- 对可反馈协议输入，例如 OTLP/gRPC、HTTP push：
  返回限流、忙碌或重试语义
- 对可控本地采集输入，例如轮询式发现、主动扫描：
  主动降频、延期或暂停
- 对不可反馈输入，例如文件追加、部分系统日志源：
  使用本地 buffer 吸收；buffer 到达硬上限后按优先级丢弃，同时保留 drop 统计和原因

工程上需要保证：

- buffer 使用量必须可观测
- backpressure 状态必须可观测
- drop 必须带计数、原因和优先级标签
- 任何可靠性增强都不能突破 CPU、内存和磁盘预算硬边界
- 中心节点必须能看到 agent 当前处于正常、降级、保护或丢弃状态

### 7.1 自研统一采集能力

环境内 agent 需要自己具备统一采集代理的核心能力，而不是完全依赖外部代理。

至少要覆盖：

- 多来源输入
- 多协议接入
- 基础解析
- 标签与元数据附加
- 中间处理 pipeline
- 输出与路由
- 配置驱动运行
- 资源受控与高可用运行

同时需要明确：

- 输入协议优先支持 OTLP
- 输出协议优先支持 OTLP
- 内部对象模型尽量不要脱离 OTel 的基本信号抽象另起一套完全不兼容的命名

### 7.2 内置安装能力

环境内 agent 必须具备内置安装能力。

需要支持：

- 自动识别 OS / 架构
- 自动部署代理程序
- 自动生成最小配置
- 自动注册服务
- 自动健康检查
- 自动升级与回滚

### 7.3 资产 / 资源自动发现能力

环境内 agent 需要负责最接近数据源的一层资源发现与本地标识建立。

优先发现对象包括：

- host
- process
- container
- kubernetes pod / node / namespace / workload
- service
- ip / port

目标不是做完整 CMDB，而是为后续事件关联建立本地资源视角。

### 7.4 统一原始事件主线

环境内 agent 要坚持“只采一次、统一建模”的原则。

即：

- 原始数据只进入系统一次
- 第一跳保留原始事件
- 原始事件在边缘侧就建立统一事件主线
- 后续所有 Logs / Security / Metrics / Traces 的组织尽量围绕同一原始事件展开

### 7.5 可靠上送

环境内 agent 不只负责采，还负责可靠地把数据送到中心端。

至少要支持：

- buffer
- retry
- backpressure
- 网络波动下的可恢复行为
- 健康检查与状态上报

这里同样要受非功能约束限制：

- 缓冲不能无限增长
- retry 不能导致 CPU 空转
- backpressure 不能演化成内存膨胀
- 所有保护机制都要优先服务于“低资源消耗 + 稳定运行”

### 7.6 自我升级能力

环境内 agent 需要具备自我升级能力，而不是依赖人工逐台替换。

目标包括：

- 版本检查
- 升级包获取
- 完整性校验
- 平滑切换或最小中断切换
- 升级后健康探测
- 失败自动回滚

设计约束：

- 升级过程必须可中止
- 升级过程必须可审计
- 升级过程必须有版本兼容约束
- 升级过程不能默认丢失本地缓冲中的关键数据

### 7.7 远程指令执行能力

环境内 agent 可以具备远程指令执行能力，但该能力必须是受控能力，而不是默认无限 shell。

建议目标定义为：

- 支持中心节点下发远程执行任务
- 支持有限的命令或动作模型
- 支持返回标准输出、标准错误、退出码与执行元数据
- 支持超时、中断、并发限制和结果回传

设计原则：

- 默认最小权限
- 默认白名单动作优先
- 高风险动作必须二次确认
- 所有执行过程必须审计
- 必须支持按租户、环境、节点、角色做权限隔离

也就是说，目标不是“给每个 agent 一个任意远程 shell”，而是“给系统一个可治理、可审计、可限权的远程执行面”。

---

## 8. 中心节点的目标

中心节点需要承担以下职责。

### 8.1 全局资源目录

把各环境、各节点上送的资源信息汇聚成全局资源视图。

需要解决：

- 资源去重
- 资源归并
- 资源身份稳定化
- 资源生命周期维护
- 资源关系图维护

### 8.2 全局多信号关联

中心节点负责围绕统一原始事件和统一资源模型组织四类信号：

- Logs
- Security
- Metrics
- Traces

这里必须明确现实边界：

- `Logs` 和 `Security` 最适合从同一原始事件直接派生
- `Metrics` 通常是事件聚合结果，不是原始事件本体
- `Traces` 只有在具备上下文时才适合关联或生成，不应强行从任意日志反推

所以正确目标不是“四信号强制同构”，而是：

在统一原始事件和统一资源上下文下，按条件派生和关联多类信号。

### 8.3 统一关联查询

中心节点必须支持围绕以下两个主轴查询：

- 事件主轴：围绕同一 `event_id` 查看原始事件及其派生产物
- 资源主轴：围绕同一资源查看其 Logs / Security / Metrics / Traces

如果做不到这一点，就仍然只是采集系统，不是统一观测系统。

### 8.4 OTel 对齐治理

中心节点要承担标准治理职责，包括：

- 资源属性对齐
- 字段命名治理
- schema 漂移检查
- OTel semantic mapping 建议
- OTLP 输入输出一致性检查

这类能力天然更适合集中式治理，而不是分散在边缘节点上。

### 8.5 升级与远程执行编排

中心节点需要统一管理：

- 哪些 agent 需要升级
- 升级批次、升级窗口和回滚策略
- 哪些节点允许执行远程指令
- 哪些命令或动作可以执行
- 哪些执行必须人工确认
- 执行结果如何回收与审计

这意味着升级和远程执行不是边缘 agent 的自主策略能力，而是中心节点主导、边缘 agent 执行的能力。

---

## 9. AI 与中心节点的结合目标

AI 不是不要，而是明确放在中心节点处理。

中心节点上的 AI 能力适合承担：

- 事件语义增强
- 资源语义补全
- 多信号关联解释
- 异常摘要
- 根因假设
- OTel 对齐建议
- 规则生成与规则修正建议
- 排障建议
- 自动化行动建议
- 知识沉淀与经验复用

这些能力的共同特点是：

- 需要全局视角
- 需要知识库
- 需要历史样本
- 需要跨资源、跨信号分析
- 需要人工确认、审计与统一风险控制

因此 AI 应该是中心节点的增强能力，而不是环境内代理的基础能力。

AI 在这里更适合做的，是：

- 帮助生成升级计划
- 帮助生成远程诊断执行计划
- 帮助判断某类远程动作的风险等级

而不是直接在边缘节点上自由决策执行。

---

## 10. 统一事件模型目标

为了支撑“同一份采集、同一份原始事件、多信号关联”，系统必须定义统一事件模型。

统一事件模型至少需要：

- `event_id`
- `collector_id`
- `source_type`
- `observed_time`
- `ingest_time`
- `resource_refs`
- `correlation`
- `raw`
- `normalized`

在字段设计上，建议遵循以下约束：

- 资源相关字段优先映射到 OTel Resource 属性
- trace 关联字段优先使用 OTel 既有命名
- 自定义扩展字段尽量收敛在扩展命名空间中，不污染标准字段

可参考如下结构：

```json
{
  "event_id": "uuid",
  "collector_id": "agent-01",
  "source_type": "file|syslog|otlp|ebpf|winlog",
  "observed_time": "...",
  "ingest_time": "...",
  "resource_refs": [
    { "type": "host", "uid": "host-123" },
    { "type": "container", "uid": "container-456" },
    { "type": "service", "uid": "service-789" }
  ],
  "correlation": {
    "trace_id": "...",
    "span_id": "...",
    "session_id": "...",
    "process_uid": "proc-001"
  },
  "raw": {
    "format": "json",
    "body": "..."
  },
  "normalized": null
}
```

这里最关键的是三条原则：

- 原始事件必须保留
- 资源引用和关联主键必须在第一跳尽量建立
- 当统一事件模型与 OTel 标准对象可以一一映射时，应优先保证可映射性

---

## 11. 统一资源模型目标

资源被发现后，必须进入统一资源模型，而不是只保留临时探测结果。

统一资源模型至少需要：

- `resource_uid`
- `resource_type`
- `name`
- `labels`
- `state`
- `owner_refs`
- `runtime`
- `valid_from`
- `valid_to`

这个模型虽然是系统内部资源目录模型，但应尽量与 OTel Resource 语义对齐。

例如：

- `name` / `labels` 中应优先保留可映射到 `service.*`、`host.*`、`k8s.*`、`cloud.*` 的字段
- 不要为了内部方便创造与 OTel 完全断裂的基础资源命名体系

可参考如下结构：

```json
{
  "resource_uid": "pod-abc",
  "resource_type": "k8s_pod",
  "name": "checkout-7d8f9",
  "labels": {
    "cluster": "prod-cn",
    "namespace": "payment",
    "app": "checkout"
  },
  "state": "running",
  "owner_refs": [
    { "type": "deployment", "uid": "deploy-001" }
  ],
  "runtime": {
    "node": "node-01",
    "ip": "10.0.0.12"
  },
  "valid_from": "...",
  "valid_to": null
}
```

---

## 12. 自动发现方式目标

资源自动发现建议按三类能力建设。

### 12.1 被动发现

通过已有观测数据反推资源：

- 日志中的 host/container/pod/service 字段
- traces 中的 resource attributes
- metrics labels
- security events 中的 process / user / ip / file path

### 12.2 主动发现

通过系统探测和平台 API 获取资源：

- 本机进程扫描
- 端口与监听枚举
- Kubernetes API watch
- 容器 runtime API
- cloud metadata / cloud API
- systemd / Windows Service 枚举

### 12.3 旁路发现

通过 eBPF、网络或审计信号持续感知资源变化：

- process / socket / DNS / connection 观察
- 审计事件识别用户、文件、进程活动
- 网络行为识别服务依赖

第一阶段建议从“被动发现 + 主动发现”起步，不把复杂旁路发现作为硬门槛。

同时需要保证：

- 发现出的资源字段尽量可映射到 OTel Resource attributes
- Kubernetes、Cloud、Host、Process 等对象命名尽量不偏离 OTel 常见语义

---

## 13. 目标边界

为了避免范围发散，必须明确当前不追求什么。

### 13.1 不是封装 Fluent Bit

`warp-insight` 可以参考 Fluent Bit，但不应把自己定义成其托管壳或安装器。

### 13.2 不是完整 CMDB

`warp-insight` 维护的是服务于观测与关联的动态资源目录，不是全功能 CMDB。

### 13.3 不是全网安全扫描器

安全信号以统一采集和事件派生为核心，不以漏洞扫描为核心。

### 13.4 不是一次性复制全部现有代理插件生态

当前目标是实现核心主干能力，而不是完整复制所有历史插件和边缘能力。

### 13.5 不是要求所有事件都生成 Trace

trace 只有在具备上下文时才应关联或生成。

### 13.6 不是把 AI 下沉到环境内热路径

AI 可以增强系统，但不应成为环境内 agent 采集、发现、缓冲和上送的基础前提。

### 13.7 不是脱离 OpenTelemetry 另起一套基础标准

允许在 OTel 之上扩展，但不应在资源模型、信号模型和协议层面无必要地另造一套完全不兼容标准。

### 13.8 不是默认开放的任意远程执行器

远程指令执行必须是受控、可限权、可审计的能力，不能默认演化成一个无限制远程 shell。

### 13.9 不是不可回滚的自动升级器

自我升级必须与健康检查、失败回滚和版本约束绑定，不能做成“一次升级失败就节点失控”的模式。

### 13.10 不是用资源消耗换功能堆叠

环境内 agent 不能因为不断叠加能力而演化成高 CPU、高内存、高波动的常驻进程。

---

## 14. 首批高价值落地方向

### 14.1 MVP-1

目标：

- 建立没有中心节点也能独立运行的 `standalone` 边缘基线
- 打通一条可替代部分 `Fluent Bit` 工作的最小日志链路
- 支持单路径 `file input -> parser / multiline -> checkpoint / commit point -> buffer / spool -> warp-parse/file output`
- 建立 rotate / truncate / restart recovery 基线
- 建立统一结构化 record 和最小本地输出 / 上送路径
- 建立最小 CPU / 内存占用观测与压测基线
- 用一类真实文件日志场景验证“能力可替代、配置不兼容”的产品假设

说明：

- `MVP-1` 不要求先把 file input 做成通用产品形态
- `path_patterns[]` / `exclude_path_patterns[]`、完整 watcher 策略、通用保护模式和完整自观测留给后续 telemetry core 阶段

### 14.2 MVP-2

目标：

- 建立 `managed` 接入基线，而不是把它当前置验证门
- 打通 enrollment、agent identity、gateway session 与 capability 上报
- 建立受控远程执行框架和首批只读动作
- 建立基础资源发现与通用 telemetry core
- 构建中心端资源目录、状态查询和最小治理闭环
- 加入升级、健康检查、回滚

### 14.3 MVP-3

目标：

- 接入原生 traces / OTLP traces
- 建立 trace 与事件、资源的关联
- 支持跨信号链路查看
- 中心节点开始承担 OTel 对齐治理能力
- 建立升级批次控制、远程执行审计与权限模型

### 14.4 MVP-4

目标：

- 在中心节点上引入 AI 事件解释、规则建议和操作辅助
- 强化资源拓扑
- 强化云环境发现
- 扩大对标代理的核心能力覆盖面

---

## 15. 成功标准

判断 `warp-insight` 是否达标，建议看以下工程结果：

- 是否可以独立安装和运行，不依赖外部采集代理作为前提
- 是否已在 `standalone` 模式下跑通至少一类可替代部分 `Fluent Bit tail input` 的真实链路
- 是否已经具备一类成熟采集代理的核心主干能力
- 是否在核心对象模型和协议层面尽量符合 OpenTelemetry
- 是否能自动发现关键资源对象
- 是否能在边缘建立稳定 `event_id`
- 是否能把事件与资源稳定关联到中心节点
- 是否能从同一采集主线组织 Logs / Security / Metrics
- 是否能在有上下文时关联 Traces
- 是否能围绕事件主轴和资源主轴完成统一查询
- 是否能把 AI 能力稳定收敛在中心节点，而不破坏环境内 agent 的确定性
- 是否具备可审计、可回滚的自我升级能力
- 是否具备可限权、可审计的远程指令执行能力
- 是否能在环境内维持低 CPU、低内存、低波动的常驻运行

如果这些能力成立，`warp-insight` 才算真正达成当前目标。

---

## 16. 当前结论

当前阶段对 `warp-insight` 的目标结论如下：

1. `warp-insight` 的目标不是“基于 Fluent Bit”，而是自研实现现有成熟采集代理的相近核心能力。
2. `Fluent Bit` 等现有产品应作为能力参考和对标对象，而不是系统依赖。
3. `warp-insight` 必须拆成“环境内轻量代理 + 中心节点控制平面”两层。
4. OpenTelemetry 应作为协议、资源模型、信号模型和字段语义的优先标准基线。
5. 环境内 agent 负责安装、发现、采集、缓冲、上送，不承担复杂 AI 理解任务。
6. 环境内 agent 还应具备受控的自我升级和远程指令执行能力。
7. 环境内 agent 必须把低 CPU、低内存、低波动作为硬性非功能目标。
8. AI 能力应放在中心节点，承担事件理解、异常解释、规则建议与操作辅助。
9. 升级与远程执行应由中心节点统一编排，由边缘 agent 受控执行。
10. 当前第一验证目标必须先打通 `standalone` 的可替代切片，而不是先追求 `managed` 接入闭环。
11. 该切片应至少覆盖 `file input -> parser / multiline -> checkpoint / commit point -> buffer / spool -> warp-parse/file output`。
12. 在 `standalone` 可替代切片成立后，再继续推进 enrollment、gateway、远程执行与中心治理闭环。
13. 长期目标是形成更适合目标环境、且尽量符合 OpenTelemetry 标准的一体化环境观测系统。

最终目标可以用一句话描述：

构建一个由环境内轻量代理和中心节点组成的统一环境观测系统，自研实现与现有成熟采集代理相近的核心能力，并以 OpenTelemetry 作为优先标准基线，在统一资源上下文下组织 `Metrics`、`Logs`、`Security`，并在具备上下文时关联 `Traces`，同时将 AI 能力集中放在中心节点完成解释、建议与辅助决策，并由边缘 agent 提供受控的自我升级与远程执行能力。
