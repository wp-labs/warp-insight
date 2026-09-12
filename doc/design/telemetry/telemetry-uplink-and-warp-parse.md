# warp-insight Telemetry 上报与 Warp Parse 接入设计

## 1. 文档目的

本文档定义 `warp-insight` 数据面采集结果的上报方向，并明确 `warp-parse` 在整体架构中的角色。

重点回答：

- `warp-insightd` 采集的数据是否可以直接提交到 `warp-parse`
- `warp-parse` 是否可以作为统一数据接收器
- `logs / metrics / traces / security` 是否都应该走同一种输入形式
- `standalone` 和 `managed` 模式下的数据上报路径应如何设计

相关文档：

- [`target.md`](../foundation/target.md)
- [`architecture.md`](../foundation/architecture.md)
- [`roadmap.md`](../foundation/roadmap.md)
- [`metrics-integration-roadmap.md`](metrics-integration-roadmap.md)
- [`metrics-config-schema.md`](metrics-config-schema.md)

---

## 2. 核心结论

当前阶段固定以下结论：

- `warp-parse` 可以作为 `warp-insightd` 的数据面上报目标之一
- `warp-parse` 可以被设计成统一数据接收器
- 这里的“统一”指统一接入层，而不是把所有信号都降级成文本日志
- 第一阶段默认路线可以采用“统一结构化文本记录 -> `warp-parse`”
- `warp-parse` 只承担数据面接入、解析、转换、路由职责
- `warp-parse` 不能替代 `warp-insight` 控制中心、`Agent Gateway` 或远程任务治理链路

一句话说：

- 数据可以进 `warp-parse`
- 控制不能进 `warp-parse`

---

## 3. 角色边界

### 3.1 `warp-parse` 的合适角色

`warp-parse` 在 `warp-insight` 体系里最适合承担：

- 数据接入
- 数据解析
- 数据转换
- 数据路由
- 下游分发

它适合做：

- 统一数据接收器
- 统一 ETL 节点
- 日志 / 事件 / 安全事件的高性能接入节点

### 3.2 `warp-parse` 不应承担的角色

`warp-parse` 不应承担：

- `Agent Gateway`
- agent 注册与租约管理
- 远程任务下发
- 审批、签名和审计主链路
- 升级编排控制面

这几个能力仍然属于：

- `warp-insight` 控制中心

---

## 4. 统一接收器的正确含义

“统一接收器”不应理解为：

- 所有信号都先变成一行文本
- 再统一按日志规则解析

正确理解应是：

- 一个统一 ingress runtime
- 支持多种 receiver
- 进入系统后再统一做 normalize / route / enrich / export

也就是说，统一的是：

- 接入层
- 运行时
- 路由与治理框架

不是统一成：

- 单一文本格式

但对第一阶段，还需要加一个工程性判断：

- 如果 `warp-insightd` 先把多信号转成统一的结构化文本记录
- `warp-parse` 就可以先作为统一接收器成立

因此第一阶段可以先收敛为：

- 统一文本化 record
- 统一接入到 `warp-parse`

后续再决定是否对部分信号增加原生 receiver。

---

## 5. 各信号的接入方式

### 5.1 Logs

`Logs` 最适合的输入形式包括：

- 原始文本行
- syslog
- JSON log
- NDJSON

这类数据非常适合进入 `warp-parse`，再由：

- WPL 解析
- OML 转换
- route 分发

### 5.2 Security

`Security` 数据通常也适合进入 `warp-parse`，尤其是：

- audit log
- auth log
- network/security appliance event
- 主机安全事件
- JSON security event

这里的推荐路径与 `Logs` 类似，但应注意：

- `Security` 可以与 `Logs` 共用原始输入主线
- 标准化后应保留独立的 signal 语义

### 5.3 Metrics

`Metrics` 不建议被降级成普通日志文本。

更合适的输入形式是：

- OTLP metrics
- Prometheus/OpenMetrics scrape 结果
- StatsD
- JMX bridge 后的结构化结果

因此，如果 `warp-parse` 要作为 metrics 接收器，推荐方式是：

- 增加 metrics-aware receiver
- 直接接收结构化 metrics 数据

而不是：

- 把每个 sample 先伪装成一行文本日志

但第一阶段允许采用折中方案：

- `warp-insightd` 把 metric sample 转成结构化文本 record
- `warp-parse` 先统一接收和路由

这里的关键前提是：

- record 中必须保留 metric 名称、数值、时间戳、resource、attributes 和类型语义

### 5.4 Traces

`Traces` 更不应被降级成普通文本日志。

原因是：

- `trace` 本质是多 `span` 结构关系
- 需要保留 `trace_id / span_id / parent_span_id`
- 还要保留 attributes、events、links、status 等结构字段

因此推荐的接入方式是：

- OTLP traces receiver
- 或标准化的 structured span JSON receiver

不推荐：

- 把 span 打平成普通文本日志再解析

但第一阶段允许采用折中方案：

- `warp-insightd` 把每个 span 编码成结构化文本 record
- `warp-parse` 对其进行统一接收、解析和路由

这里必须保留：

- `trace_id`
- `span_id`
- `parent_span_id`
- resource attributes
- span attributes
- timing 与 status

---

## 6. 为什么 traces 不能简单走文本日志路径

虽然 `trace` 也可以被序列化成：

- JSON
- NDJSON
- 其他文本编码

但这不意味着它应该按“普通日志文本”处理。

关键区别在于：

- 文本只是编码形式
- `trace/span` 的本质仍然是结构化对象

所以判断标准不是：

- 它是不是文本

而是：

- 接收器能不能保留 trace 语义

只要接收器能保留：

- `trace_id`
- `span_id`
- `parent_span_id`
- resource attributes
- span attributes
- timing 结构

那么文本 JSON 也可以。

但如果把它变成：

- 一行不可恢复关系的日志文本

那就不对。

---

## 7. 推荐的接入模型

### 7.1 Receiver 分类

如果 `warp-parse` 承担统一数据接收器角色，建议至少有四类 receiver：

- `log_receiver`
- `security_receiver`
- `metrics_receiver`
- `trace_receiver`

其中：

- `log_receiver`
  支持 text / json / syslog
- `security_receiver`
  支持 text / json / audit/security event
- `metrics_receiver`
  支持 OTLP metrics / scrape / push metrics
- `trace_receiver`
  支持 OTLP traces / structured span JSON

### 7.2 统一运行时

上述 receiver 进入统一运行时后，走共同主线：

- validate
- normalize
- resource binding
- routing
- buffering / backpressure
- export

这才是“统一接收器”的真正价值。

### 7.3 第一阶段默认路线

第一阶段默认建议不要求 `warp-parse` 先补齐所有原生 receiver。

默认路线可以先定为：

- `warp-insightd` 将 `logs / metrics / traces / security` 统一编码成结构化文本 record
- `warp-parse` 先作为统一文本接入器、解析器和路由器

这种做法的优势是：

- 能最快成立统一数据接入器
- 不需要第一天就把所有原生协议 receiver 做完
- 可以先复用 `warp-parse` 的文本/结构化记录处理能力

它的代价也必须明确：

- `warp-insightd` 侧编码责任更重
- metrics / traces 的标准原生协议优势会被部分折叠
- 长期看未必是最终最优性能路径

因此应把它视为：

- `V1` 默认路线

而不是：

- 长期唯一形态

---

## 8. `warp-insightd` 的上报目标建议

`warp-insightd` 第一版建议支持以下数据面上报目标类型：

- `warp_parse`
- `otlp`
- `file`
- `object_store`

### 8.1 `warp_parse`

适合：

- 需要统一 ETL 接入层
- 需要高性能解析 / 转换 / 路由
- 希望 `Logs / Security / 部分 Metrics / 部分 Traces` 统一落到一个数据入口

第一阶段默认可采用：

- 统一结构化文本 record -> `warp-parse`

### 8.2 `otlp`

适合：

- 直接对接 OTel backend
- metrics / traces 走标准最短路径
- 不需要中间解析转换节点

### 8.3 `file`

适合：

- `standalone` 模式
- 本地落盘
- 调试与回放

### 8.4 `object_store`

适合：

- 批量归档
- 冷数据保留
- 异步后处理

---

## 9. 运行模式下的推荐路径

### 9.1 `standalone`

`standalone` 模式下推荐：

- `warp-insightd -> warp-parse`
  或
- `warp-insightd -> local file / object store`

此时：

- 不要求中心控制节点存在
- 数据面仍能继续工作
- 不提供远程任务能力

### 9.2 `managed`

`managed` 模式下推荐：

- 数据面：
  `warp-insightd -> warp-parse` 或 `OTLP backend`
- 控制面：
  `warp-insightd <-> warp-insight control center`

这里必须明确：

- 数据面和控制面是两条不同链路
- 即使数据面提交到 `warp-parse`，远程任务和升级编排仍走控制中心

---

## 10. 对 `warp-insight` 的直接设计影响

### 10.1 架构影响

`warp-insight` 需要把“数据上报目标”视为一等配置项，而不是写死成“只能上中心节点”。

### 10.2 协议影响

后续需要单独定义：

- telemetry uplink envelope —— 已定稿，见 [`telemetry-uplink-protocol.md`](telemetry-uplink-protocol.md)
- `warp_parse` target 的连接协议
- `otlp` target 的直连规则
- failover / retry / idempotency 规则

### 10.3 产品影响

这条设计允许：

- `warp-insightd` 在没有控制中心时独立工作
- `warp-parse` 先成为统一数据接收器
- `warp-insight` 控制中心后续再叠加治理能力

这对当前阶段是有利的，因为它把：

- 数据面成立
- 控制面成立

拆成了两个可以分阶段落地的目标。

同时也把数据面接入再拆成两步：

1. `V1`
   统一结构化文本 record，直接提交到 `warp-parse`
2. `V2`
   对 metrics / traces 视需要补充原生 receiver

---

## 11. 当前建议

当前阶段建议固定以下结论：

- `warp-parse` 可以作为 `warp-insightd` 的统一数据接收器
- 但它只负责数据面，不负责控制面
- `Logs / Security` 最适合先直接进入 `warp-parse`
- 第一阶段允许 `Metrics / Traces` 先由 `warp-insightd` 编码成结构化文本 record，再进入 `warp-parse`
- 这里说的“文本”必须是保留完整语义的结构化 record，而不是普通 message line
- 后续仍可按需要为 `Metrics / Traces` 增加原生 receiver
- 后续应继续补一份专门的 `telemetry uplink protocol` 文档，把上报 envelope 与目标协议继续定稿 —— 已落地为 [`telemetry-uplink-protocol.md`](telemetry-uplink-protocol.md)
