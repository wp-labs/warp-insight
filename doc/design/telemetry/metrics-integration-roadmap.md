# warp-insight Metrics Integration 路线图

## 1. 文档目的

本文档定义 `warp-insight` 在 metrics 侧的 integration 覆盖范围、优先级和分批落地顺序。

目标不是列一个很长的支持清单，而是明确：

- 哪些 target 应作为第一优先级
- 哪些 target 应由 `warp-insightd` 直接内建采集
- 哪些 target 可以先走 exporter compatibility mode
- AI 在这条研发线上应承担什么角色

相关文档：

- [`target.md`](../foundation/target.md)
- [`architecture.md`](../foundation/architecture.md)
- [`roadmap.md`](../foundation/roadmap.md)
- [`telemetry-uplink-and-warp-parse.md`](telemetry-uplink-and-warp-parse.md)

---

## 2. 核心结论

`warp-insight` 在 metrics 侧的默认路线应是：

- `warp-insightd` 内建 collector / scraper / receiver
- 尽量覆盖大多数常见 target
- 外部 exporter 仅作为兼容 fallback

这条路线成立的前提是：

- 常见 integration 已具备高度重复性
- 标准接口和语义约定已经足够成熟
- AI 已经能够显著加速 collector 骨架、mapping、fixture 和测试样例生成

因此，“支持大多数常用 metrics 数据采集”应被视为常规工程主线，不应被视为高不可攀的特殊项目。

---

## 3. 设计原则

### 3.1 优先做高复用 target

优先选择：

- 覆盖面广
- 接口稳定
- 运维刚需强
- OTel / Prometheus 语义成熟

### 3.2 优先做低耦合采集方式

优先顺序建议：

1. 主机 / 运行时本地采集
2. 标准 pull 接口
3. 标准 push 接口
4. 常见中间件专用 collector
5. 专有系统兼容模式

### 3.3 优先做统一能力，再做个性目标

优先建设：

- discovery
- auth
- scrape scheduler
- relabel / normalize
- resource binding
- rate / timeout / budget

然后再叠加各 target integration。

### 3.4 不把远程 action 当采集主路径

metrics 采集不能通过 `wist-exec` 临时执行命令来补。

metrics integration 必须属于 `warp-insightd` 常驻数据面能力。

---

## 4. 目标分层

建议把 metrics target 分成五层。

### 4.1 L1 主机与运行时

目标：

- host
- cpu
- memory
- filesystem
- disk io
- network
- process
- systemd
- container runtime
- Kubernetes node / pod

特点：

- 覆盖面最大
- 是所有环境的基础观测底盘
- 不依赖应用配合

### 4.2 L2 标准暴露接口

目标：

- Prometheus / OpenMetrics endpoint
- OTLP metrics receiver
- StatsD receiver
- JMX bridge / scraper

特点：

- 通用性强
- 覆盖大量现代服务
- 可以快速替代大量 exporter 场景

### 4.3 L3 常见服务 / 中间件

目标：

- nginx
- mysql
- postgresql
- redis
- kafka
- elasticsearch
- clickhouse
- rabbitmq

特点：

- 业务中高频出现
- 每类都值得做成标准 collector

### 4.4 L4 平台与云原生组件

目标：

- kube-apiserver
- kubelet
- coredns
- etcd
- ingress controller
- service mesh control plane

特点：

- Kubernetes 环境价值高
- 适合跟 discovery 强结合

### 4.5 L5 专有与遗留系统

目标：

- 私有协议设备
- 老旧闭源系统
- 短期不值得自研的目标

特点：

- 先保留 exporter compatibility mode
- 不作为第一版默认主线

---

## 5. 分批落地建议

### 5.1 Batch A

第一批应优先覆盖：

- host / cpu / memory / filesystem / network
- process
- container runtime
- Kubernetes node / pod
- Prometheus / OpenMetrics scrape
- OTLP metrics receiver

原因：

- 这是平台底盘能力
- 复用面最大
- 能最快替代大量现有 exporter

### 5.2 Batch B

第二批建议覆盖：

- StatsD
- JMX
- nginx
- mysql
- postgresql
- redis

原因：

- 通用程度高
- 企业环境中非常常见
- AI 很容易加速这类 integration 的骨架和测试样例生成

### 5.3 Batch C

第三批建议覆盖：

- kafka
- elasticsearch
- rabbitmq
- clickhouse
- coredns
- kube-apiserver
- kubelet
- etcd

### 5.4 Batch D

第四批再考虑：

- 厂商私有系统
- 云厂商专有服务桥接
- 历史 exporter compatibility 强依赖目标

---

## 6. 每类 integration 的统一结构

建议所有 metrics integration 都收敛到统一模型：

- `discovery`
- `auth`
- `target_selector`
- `scrape_or_receive_config`
- `normalize`
- `resource_mapping`
- `budget`

### 6.1 `discovery`

负责发现：

- endpoint
- address
- port
- instance id
- service metadata

### 6.2 `auth`

负责：

- basic auth
- token
- tls
- cert / key
- db credentials

### 6.3 `scrape_or_receive_config`

负责：

- interval
- timeout
- protocol
- path
- query
- labels

### 6.4 `normalize`

负责：

- metric rename
- unit normalize
- type normalize
- label cleanup

### 6.5 `resource_mapping`

负责把指标绑定到：

- host
- service
- pod
- container
- deployment environment

### 6.6 `budget`

负责：

- cpu budget
- memory budget
- timeout limit
- concurrent scrape limit
- sample limit

---

## 7. AI 加速的研发环节

AI 适合加速以下研发工作：

- collector skeleton 生成
- Prometheus/OpenMetrics 字段映射整理
- OTel semantic convention 对齐草案生成
- fixture 和 sample payload 生成
- 集成测试样例生成
- 文档和配置模板生成

但必须明确：

- AI 不进入 `warp-insightd` 运行时热路径
- AI 不参与每次 scrape 决策
- AI 不参与边缘端实时指标解释

---

## 8. 第一版验收标准

如果要说“metrics integration 路线成立了”，第一版至少应满足：

- `warp-insightd` 能直接采集主机与运行时基础指标
- `warp-insightd` 能直接 scrape Prometheus / OpenMetrics endpoint
- `warp-insightd` 能直接接收 OTLP metrics
- 至少落地 3 到 5 个高频中间件 integration
- exporter compatibility mode 存在，但不是默认推荐路径

---

## 9. 与总路线图的关系

建议把 metrics integration 与总体路线图这样对齐：

- M2：
  `warp-insightd` skeleton 里预留 metrics collection framework
- M3：
  本地闭环先不追求多 integration，只验证框架可运行
- M4 之后：
  metrics integration 与远程执行、升级分线推进
- M5 之后：
  控制平面补充 discovery、配置、secret、integration policy 下发

也就是说：

- metrics integration 不是 `wist-exec` 工作
- metrics integration 是 `warp-insightd` 数据面主线

---

## 10. 当前决定

当前阶段固定以下结论：

- `warp-insightd` 应承担大多数常见 metrics 采集
- exporter 不再是默认前提，而是兼容选项
- 先做 Batch A，再做 Batch B
- AI 用于加速 integration 研发，而不是进入边缘运行时

---

## 11. W3 落地设计（uplink + 扩展机制）

W3「收到指标」的链路闭环与三项已定决策：

### 11.1 链路闭环

```text
discovery snapshot → planner_bridge（discovery → 采集候选）→ provider.collect → normalize → 指标帧 → uplink
```

现状：`target_view`（候选）/ `runtime`（采集）/ `samples`（规范化）已落地；**指标帧序列化 + uplink 接入 + provider 抽象（W4）待落地**。

### 11.2 已定决策

- **帧标记区分（A）**：信封 `{schema, agent, ts, seq}` 保持信号无关、不动；信号类型靠帧标记表达——` RAW:`（日志）/ ` METRICS:`（指标），见 `telemetry-uplink-protocol.md` §4/§5。
- **共享 uplink（B）**：指标与日志共用同一 TCP 连接，靠帧标记区分；指标优先 + 背压隔离（日志洪峰不挤掉指标，反之亦然）。
- **编译期注册 provider（C1）**：`MetricProvider` trait + 静态注册表，新增 provider = 写 `impl` + 注册一行 + 重编；脚本型指标（`wist-exec` opcode）后续再议。

### 11.3 三层结构（W4 扩展机制）

| 层 | 职责 | 扩展点 |
| --- | --- | --- |
| 契约层（spec） | `MetricSpec{ name, unit, value_type, target_selector, provider, ... }` | 新增 spec |
| 映射层（planner_bridge） | discovery snapshot → `CollectionPlan`（候选） | selector 规则 |
| 采集层（provider） | `MetricProvider{ id, supports(spec), collect(target) -> samples }` | 编译期注册新 provider |
