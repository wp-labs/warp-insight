# warp-insightd 资源发现运行时设计

## 1. 文档目的

本文档定义 `warp-insightd` 在边缘节点上的资源发现运行时设计。

这里的“资源发现”特指：

- `warp-insightd` 作为常驻 agent 在本机持续发现资源与采集目标
- 产出可复用的本地事实视图
- 为数据面和控制面提供共享的本地资源引用与 target 基础

本文档不讨论：

- 中心侧资源目录归并
- 任意 shell 或远程动作式发现
- 具体某一类 metrics collector 的字段细节
- `wist-exec` 的 opcode 设计

相关文档：

- [`discovery-runtime-current-state.md`](discovery-runtime-current-state.md)
- [`agentd-architecture.md`](../../crates/wist-agentd/docs/agentd-architecture.md)
- [`agentd-state-and-boundaries.md`](../../crates/wist-agentd/docs/agentd-state-and-boundaries.md)
- [`agentd-failure-handling.md`](../../crates/wist-agentd/docs/agentd-failure-handling.md)
- [`capability-report-schema.md`](../../crates/wist-agentd/docs/capability-report-schema.md)
- [`../center/report-discovery-snapshot-schema.md`](../center/report-discovery-snapshot-schema.md)
- [`../foundation/architecture.md`](../foundation/architecture.md)
- [`../foundation/roadmap.md`](../foundation/roadmap.md)
- [`../telemetry/metrics-discovery-and-resource-mapping.md`](../telemetry/metrics-discovery-and-resource-mapping.md)
- [`self-observability.md`](../../crates/wist-agentd/docs/self-observability.md)

---

## 2. 核心结论

第一版固定以下结论：

- `discovery` 是 `warp-insightd` 的常驻运行时能力，不是 `wist-exec` 的任务
- `discovery` 负责产出本地事实，不直接决定“现在要采哪些 target”
- `discovery` 必须可持续 refresh，不是只在启动时做一次同步大扫描
- `discovery` 结果必须可同时服务数据面和控制面
- `discovery` 状态应独立于 `execution_queue` / `running` / `reporting`

一句话说：

- `discovery` 负责回答“本机有什么”
- `planning` 负责回答“这些里面哪些要采、如何采”

因此应明确：

`discovery result + config / policy + capability + budget = collection plan`

而不是：

`discovery = collection plan`

还应固定一条对象边界：

- `DiscoveredTarget` 只表达“发现到了什么 target 对象”
- `CandidateCollectionTarget` 才表达“后续准备如何采这个 target”

也就是说：

- discovery 产出的是候选事实对象
- planner 才把这些候选事实对象编译成可执行采集目标

---

## 3. 目标与边界

### 3.1 它负责什么

资源发现运行时负责：

- 发现本机 `host`
- 发现本机 `process`
- 发现本机 `container`
- 发现本机 `k8s_node` / `k8s_pod`
- 发现可采集的本地或邻接 target
- 建立稳定的本地 `resource_id` / `target_id`
- 持久化 discovery cache
- 计算 refresh diff
- 对外提供最新 `DiscoverySnapshot`

### 3.2 它不负责什么

资源发现运行时不负责：

- 直接启停 collector / scraper / receiver
- 直接生成中心下发的 `ActionPlan`
- 做中心侧多节点资源归并
- 用临时 shell 命令执行发现逻辑
- 阻塞 daemon 主启动流程直到全量扫描结束

### 3.3 与 `wist-exec` 的关系

第一版应明确：

- `wist-exec` 不承载 discovery 本体
- `wist-exec` 最多只承载只读导出类 opcode
- 不允许把常驻 discovery 建模成“反复执行的 action task”

原因是：

- `exec` 是一次性、受控、短生命周期执行器
- `discovery` 是常驻、周期刷新、带 cache 的公共底盘能力

---

## 4. 分层模型

资源发现建议拆成两层：

- 边缘发现
- 中心归并

### 4.1 边缘发现

边缘发现只负责：

- 本地事实采集
- 本地稳定 identity 建立
- 本地 target 候选产出
- 本地资源引用建立

### 4.2 中心归并

中心归并负责：

- 跨节点去重
- 生命周期合并
- 全局资源目录建模
- 环境级 / 租户级关联

这条边界必须固定，避免边缘节点为了全局一致性背负过重状态复杂度。

---

## 5. 对象模型

### 5.1 `DiscoveredResource`

建议统一成：

```text
DiscoveredResource {
  resource_id
  kind
  attributes
  runtime_facts
  discovered_at
  last_seen_at
  health
  source
}
```

字段说明：

- `resource_id`
  本机稳定资源标识
- `kind`
  资源类型
- `attributes`
  对齐 OTel 风格的资源属性
- `runtime_facts`
  本机运行时事实，不一定都进入统一 resource attrs
- `discovered_at`
  首次发现时间
- `last_seen_at`
  最近一次确认存在时间
- `health`
  本地视角资源健康状态
- `source`
  由哪类 discovery probe 产出

### 5.2 `DiscoveredTarget`

建议统一成：

```text
DiscoveredTarget {
  target_id
  kind
  resource_refs[]
  endpoint?
  labels
  runtime_facts
  discovered_at
  last_seen_at
  state
  source
}
```

字段说明：

- `target_id`
  target 的稳定标识
- `kind`
  target 类型
- `resource_refs[]`
  指向相关 `resource_id`
- `endpoint`
  如适用，记录 scrape / receive endpoint
- `labels`
  discovery 原始标签
- `runtime_facts`
  本机运行时事实
- `state`
  target 当前是否可用
- `source`
  target 来源

关键约束：

- `DiscoveredTarget.kind` 只描述发现对象本身
- 不直接表达后续采用哪类 collector / scraper / receiver

### 5.3 `CandidateCollectionTarget`

建议统一成：

```text
CandidateCollectionTarget {
  candidate_id
  target_ref
  collection_kind
  resource_refs[]
  execution_hints
  generated_at
}
```

字段说明：

- `candidate_id`
  planner 侧候选采集目标 id
- `target_ref`
  指向 `DiscoveredTarget.target_id`
- `collection_kind`
  后续计划使用的采集方式
- `resource_refs[]`
  关联资源
- `execution_hints`
  调度 / 认证 / 限流 / scrape 参数提示
- `generated_at`
  候选目标生成时间

关键约束：

- `CandidateCollectionTarget` 不是 discovery cache 的一部分
- 它由 planner 根据 discovery 结果和配置 / 策略动态生成
- planner 可在不同模式下对同一 `DiscoveredTarget` 生成不同候选采集目标

### 5.4 `DiscoverySnapshot`

建议统一成：

```text
DiscoverySnapshot {
  snapshot_id
  revision
  generated_at
  resources[]
  targets[]
}
```

它表示某一轮 discovery 的完整本地快照。

---

## 6. 枚举建议

### 6.1 `ResourceKind`

第一版建议：

- `host`
- `process`
- `container`
- `k8s_node`
- `k8s_pod`
- `service`

### 6.2 `TargetKind`

第一版建议：

- `host`
- `process`
- `container`
- `k8s_node`
- `k8s_pod`
- `service_endpoint`
- `log_file`

这些 `kind` 只描述“发现对象本身是什么”，不描述“后续用哪种 collector / scraper / receiver 去采”。

如需表达采集方式，建议在 planner 阶段引入单独枚举，例如：

- `host_metrics`
- `process_metrics`
- `container_metrics`
- `prom_scrape`
- `otlp_metrics_receiver`
- `file_tail`

### 6.3 `DiscoverySource`

第一版建议：

- `local_runtime`
- `static`
- `file`
- `k8s`

### 6.4 `TargetState`

第一版建议：

- `active`
- `inactive`
- `degraded`

### 6.5 `ResourceHealth`

第一版建议：

- `unknown`
- `healthy`
- `degraded`
- `unreachable`

---

## 7. identity 规则

### 7.1 基本原则

第一版必须固定：

- `resource_id` / `target_id` 必须稳定
- 优先使用 runtime stable id
- 不允许直接用高基数字段做主 identity
- 允许保留调试信息，但不把它升格为统一主键

### 7.2 建议规则

- `host`
  - `resource_id = host.id`
- `process`
  - 首选 `resource_id = host.id + pid + start_time`
  - 若无法获取 `start_time`，才降级为 `host.id + pid`
  - 降级时应显式标记为 weak identity，用于提醒可能存在 pid 复用误合并风险
- `container`
  - `resource_id = container.id`
- `k8s_node`
  - `resource_id = k8s.node.name`
- `k8s_pod`
  - `resource_id = pod.uid`
- `service`
  - `resource_id = service.name + namespace + endpoint signature`

### 7.3 `target_id`

建议规则：

- host target
  - `target_id = host.id + target kind`
- process target
  - 首选 `target_id = host.id + pid + start_time + target kind`
  - 无 `start_time` 时才降级为 `host.id + pid + target kind`
- container target
  - `target_id = container.id + target kind`
- `prom endpoint`
  - `target_id = scheme + host + port + path`
- `log file`
  - `target_id = input id + file identity`

---

## 8. 模块设计

建议在 `warp-insightd` 内部拆成以下模块：

- `discovery/runtime`
- `discovery/host`
- `discovery/process`
- `discovery/container`
- `discovery/k8s`
- `discovery/cache`
- `discovery/planner_bridge`

### 8.1 `runtime`

负责：

- 启动各 probe
- 调度 refresh 周期
- 汇总 `ProbeOutput`
- 计算 diff
- 发布新 snapshot

### 8.2 `host`

负责：

- 发现 host 资源
- 产出 host 类 target

### 8.3 `process`

负责：

- 发现 process 资源
- 产出 process 类 target

### 8.4 `container`

负责：

- 发现 container 资源
- 产出 container 类 target

### 8.5 `k8s`

负责：

- 发现 node / pod / service endpoint
- 产出 k8s 相关 target

### 8.6 `cache`

负责：

- 唯一写入 discovery 状态文件
- 恢复最近一次成功 snapshot
- 原子更新 cache

### 8.7 `planner_bridge`

负责：

- 把 discovery snapshot 暴露给数据面规划与调度模块
- 基于 discovery snapshot 生成 `CandidateCollectionTarget`
- 不直接下最终启停决策

---

## 9. 运行模型

### 9.1 启动流程

建议 `warp-insightd` 采用如下启动顺序：

1. `bootstrap`
2. `config_runtime`
3. 加载 discovery cache
4. 启动 telemetry / control 主循环
5. 后台启动 discovery 首轮 refresh
6. 发布首轮成功 snapshot

关键约束：

- daemon 不等待全量 discovery 完成后再进入常驻主循环
- 若有历史 cache，应优先恢复并对外提供

### 9.2 readiness 语义

必须明确 discovery 对下游的 readiness 口径：

- 有历史 cache 且成功加载：
  - discovery 视为 `ready_with_stale_snapshot`
- 无历史 cache 且首轮 refresh 未完成：
  - discovery 视为 `not_ready`
- 首轮 refresh 成功：
  - discovery 视为 `ready`

下游约束：

- `not_ready` 不能被解释为“空资源集”
- 数据面 target planner 在 `not_ready` 时不得把 discovery 结果当成明确空集合做裁决
- 控制面只读查询在 `not_ready` 时应明确返回“discovery 未就绪”，而不是“没有对象”

### 9.3 refresh 模型

第一版建议：

- 每类 probe 独立 refresh interval
- 周期刷新而非一次性扫描
- 每轮 refresh 产出完整 `ProbeOutput`
- `runtime` 统一合并并生成新 revision

### 9.4 建议 refresh 周期

第一版建议：

- `host`
  - 启动即刷新
  - 低频周期刷新
- `process`
  - 中频刷新
- `container`
  - 中频刷新
- `k8s`
  - watch 优先
  - poll fallback

### 9.5 事件与 diff

每轮 refresh 后应至少得到：

- `added resources`
- `removed resources`
- `updated resources`
- `added targets`
- `removed targets`
- `updated targets`

并转成结构化 discovery 事件。

---

## 10. Probe 输出与协作边界

建议每个 probe 只返回结构化输出：

```text
ProbeOutput {
  source
  refreshed_at
  resources[]
  targets[]
}
```

必须明确：

- probe 不直接写状态文件
- probe 不直接操作 telemetry 调度器
- probe 不直接回写 capability report

模块协作统一通过对象或事件，不通过多模块抢写状态文件协作。

---

## 11. 本地状态与目录

### 11.1 目录建议

```text
<agent_root>/
  state/
    discovery/
      resources.json
      targets.json
      meta.json
```

### 11.2 `resources.json`

保存最近一次成功快照中的 `DiscoveredResource[]`。

### 11.3 `targets.json`

保存最近一次成功快照中的 `DiscoveredTarget[]`。

### 11.4 `meta.json`

建议字段：

- `schema_version`
- `snapshot_id`
- `revision`
- `generated_at`
- `last_success_at`
- `last_error?`

### 11.5 状态边界

这些状态：

- 不属于 `execution_queue`
- 不属于 `running`
- 不属于 `reporting`
- 不属于 logs checkpoint state

它们应作为独立状态树存在。

---

## 12. 唯一写入权

第一版建议固定如下：

| 状态对象 | 唯一写入模块 | 其他模块权限 |
|---|---|---|
| `state/discovery/resources.json` | `discovery/cache` | 只读 |
| `state/discovery/targets.json` | `discovery/cache` | 只读 |
| `state/discovery/meta.json` | `discovery/cache` | 只读 |

核心原则：

- probe 不直接写文件
- 数据面运行时不直接写 discovery cache
- control 相关模块不直接改 discovery 状态

---

## 13. 与数据面的接线

discovery 结果应同时服务：

- `resource_mapping`
- metrics target planning
- logs resource enrichment
- 统一 telemetry record 的资源引用

但 discovery 本身不等于数据面调度器。

建议接线为：

```text
discovery runtime
  -> discovery snapshot
  -> planner bridge
  -> candidate collection targets
  -> data-plane target planner
  -> collector / scraper / receiver runtime
```

### 13.1 对 metrics 的作用

- 为 `host_metrics` 提供 host resource
- 为 `process_metrics` 提供 process resource
- 为 `container_metrics` 提供 container resource
- 为 `prom_scrape` 提供 candidate endpoint

### 13.2 对 logs 的作用

- 为 log record 提供资源引用
- 为路径到资源的本地映射提供补充事实

### 13.3 下游消费契约

第一版建议固定如下：

- discovery runtime 负责发布最新 `DiscoverySnapshot`
- planner bridge 负责基于最新 snapshot 生成 `CandidateCollectionTarget[]`
- 数据面运行时不直接写回 discovery cache
- downstream 模块如果只需要读取事实，可直接读取 snapshot
- downstream 模块如果需要决定“如何采”，必须读取 planner 产物，而不是直接解释 `DiscoveredTarget.kind`

---

## 14. 与控制面的接线

第一版 discovery 结果可服务控制面，但不由控制面驱动其主运行时。

可服务内容：

- capability 的 discovery mode 声明
- 只读查询类 action 的目标选择基础
- 向中心同步本地资源事实的输入

关键约束：

- discovery 不依赖 `managed` 模式才存在
- `standalone` 模式下 discovery 仍然成立

---

## 15. 自观测与事件

### 15.1 最小指标

第一版至少建议暴露：

- `agent_discovery_refresh_total{source,status}`
- `agent_discovery_refresh_failed_total{source}`
- `agent_discovery_resources{kind}`
- `agent_discovery_targets{kind}`
- `agent_discovery_last_success_unixtime{source}`

### 15.2 最小事件

第一版至少建议进入事件流：

- `DiscoveryRefreshed`
- `TargetAdded`
- `TargetRemoved`
- `DiscoveryRefreshFailed`

### 15.3 日志约束

应避免：

- 高频全量打印全部资源明细
- 把高基数标签默认打入常规日志
- 把完整 `cmdline`、annotations、secret 打入 discovery 日志

---

## 16. 故障处理

### 16.1 故障分层

第一版建议：

- 单个 probe 刷新失败：
  - `runtime-degraded`
  - 不打停整个 daemon
- discovery cache 写入失败：
  - 当前轮 refresh 失败
  - daemon 可继续运行，但 health 应反映退化
- discovery 状态损坏：
  - 允许丢弃并重建
  - 不升级成 execution 级故障

### 16.2 恢复规则

若 discovery cache 损坏：

- 不应打停 `warp-insightd`
- 应丢弃损坏 cache
- 继续使用下一轮 refresh 重建

若某个 probe 连续失败：

- 保留最近一次成功 snapshot
- 将该 probe 标记为 `degraded`
- 通过 health / metrics / event 反映

### 16.3 与保护模式的关系

资源压力过高时，discovery 应属于优先可退化能力。

保护流程建议：

1. 降低 refresh 频率
2. 暂停低优先级 discovery
3. 保留最基础 host / process refresh
4. 条件恢复后自动恢复正常频率

---

## 17. 安全约束

第一版固定以下约束：

- discovery 必须作为 `warp-insightd` 常驻能力运行
- 不允许通过远程 action 把 discovery 变成常规主路径
- 不允许 discovery 依赖任意 shell 或脚本执行
- 发现逻辑只读取本机必要事实，不默认扩大 OS 权限面

---

## 18. 分阶段落地建议

### 18.1 Phase 1

- host discovery
- process discovery
- discovery cache
- snapshot / diff / 事件

### 18.2 Phase 2

- container discovery

### 18.3 Phase 3

- k8s discovery

### 18.4 Phase 4

- planner bridge 接数据面运行时

### 18.5 Phase 5

- 如有需要，再补只读导出 discovery snapshot 的 `exec` opcode

---

## 19. 当前实现状态

截至当前实现，`warp-insightd` 已经具备一条可运行的本地 discovery -> planner -> metrics runtime 最小闭环。

### 19.1 当前可扫描资源

当前版本已经能扫描并持久化以下资源与 target：

- `host`
  - 识别本机 `host.id`
  - 识别本机 `host.name`
  - 产出单一 `host` target
- `process`
  - Linux 下扫描 `/proc`
  - 其他 Unix 下回退 `ps`
  - 当前稳定保留：
    - `process.pid`
    - `process.identity?`
    - `discovery.identity_strength?`
    - `discovery.identity_status?`
- `container`
  - 扫描本地常见 runtime task root
  - 当前覆盖：
    - `containerd`
    - `docker runtime-runc`
  - 当前稳定保留：
    - `container.id`
    - `container.name`
    - `container.runtime`
    - `container.runtime.namespace?`
    - `pid?`
    - `cgroup.path?`
    - `k8s.namespace.name?`
    - `k8s.pod.uid?`
    - `k8s.pod.name?`
    - `k8s.container.name?`

当前版本还没有实现：

- `k8s_node`
- `k8s_pod`
- 远程或邻接 endpoint 类 discovery
- runtime API client 驱动的 container enrich

### 19.2 当前数据面接线

当前 daemon tick 中，discovery 结果已经按下面顺序接入数据面：

```text
discovery refresh
  -> state/discovery/*.json
  -> planner bridge
  -> state/planner/*_metrics_candidates.json
  -> metrics target view
  -> state/telemetry/metrics_target_view.json
  -> metrics runtime
  -> state/telemetry/metrics_runtime_snapshot.json
  -> metrics samples
  -> state/telemetry/metrics_samples.json
```

当前 `metrics runtime` 的覆盖范围如下：

- `host_metrics`
  - 已有真实本地 probe
  - 当前可产出：
    - loadavg
    - uptime
    - memory total / available
    - host target count
- `process_metrics`
  - 已有真实本地 probe
  - 当前可产出：
    - process state
    - cpu user/system ticks
    - rss pages 或 rss kb
- `container_metrics`
  - 已有基于 discovery hint 的轻量 probe
  - 当前优先复用：
    - `pid`
    - `cgroup.path`
    - `container.runtime`
    - `k8s.*`
  - 若存在 `pid`，会复用 process probe 补进程态事实

### 19.3 当前本地状态输出

#### 19.3.1 discovery cache

```text
state/discovery/resources.json
state/discovery/targets.json
state/discovery/meta.json
```

用途：

- 保存最近一次成功的 discovery snapshot
- 作为 daemon 重启后的冷启动事实视图
- 作为 planner bridge 的只读输入

#### 19.3.2 planner candidate state

```text
state/planner/host_metrics_candidates.json
state/planner/process_metrics_candidates.json
state/planner/container_metrics_candidates.json
```

用途：

- 保存 `CandidateCollectionTarget[]`
- 显式区分“发现到了什么”与“准备怎么采”

#### 19.3.3 metrics target view

```text
state/telemetry/metrics_target_view.json
```

用途：

- 保存 metrics runtime 实际读取的统一 target 视图
- 对 planner candidate 做只读聚合，不带执行结果

结构示意：

```json
{
  "generated_at": "2026-04-19T00:00:00Z",
  "targets": [
    {
      "candidate_id": "host-1:host:host_metrics",
      "collection_kind": "host_metrics",
      "target_ref": "host-1:host",
      "resource_refs": ["host-1"],
      "execution_hints": [
        { "key": "discovery.source", "value": "local_runtime" }
      ]
    }
  ]
}
```

#### 19.3.4 metrics runtime snapshot

```text
state/telemetry/metrics_runtime_snapshot.json
```

用途：

- 保存当前 metrics tick 的执行摘要
- 反映每类 collection kind 的执行状态，而不是只反映 target 数量

结构示意：

```json
{
  "generated_at": "2026-04-19T00:00:00Z",
  "total_targets": 3,
  "host_targets": 1,
  "process_targets": 1,
  "container_targets": 1,
  "outcomes": [
    {
      "collection_kind": "host_metrics",
      "status": "succeeded",
      "attempted_targets": 1,
      "succeeded_targets": 1,
      "failed_targets": 0,
      "last_error": null,
      "runtime_facts": [
        { "key": "host.loadavg.1m", "value": "0.25" },
        { "key": "host.target.count", "value": "1" }
      ],
      "sample_targets": [
        {
          "candidate_id": "host-1:host:host_metrics",
          "target_ref": "host-1:host",
          "status": "succeeded",
          "last_error": null,
          "resource_refs": ["host-1"],
          "execution_hints": [
            { "key": "discovery.source", "value": "local_runtime" }
          ],
          "runtime_facts": [
            { "key": "host.loadavg.1m", "value": "0.25" }
          ]
        }
      ]
    }
  ]
}
```

当前 `status` 语义：

- `idle`
- `succeeded`
- `partial`
- `failed`

#### 19.3.5 metrics samples

```text
state/telemetry/metrics_samples.json
```

用途：

- 保存第一版可消费的 sample 视图
- 作为后续 Prometheus text / OTel exporter 的中间层输入

结构示意：

```json
{
  "generated_at": "2026-04-19T00:00:00Z",
  "samples": [
    {
      "metric_name": "system.load_average.1m",
      "value": { "kind": "f64", "value": "0.25" },
      "value_type": "gauge_f64",
      "unit": "1",
      "collection_kind": "host_metrics",
      "target_ref": "host-1:host",
      "resource_ref": "host-1",
      "metric_attributes": [
        { "key": "sample.status", "value": "succeeded" },
        { "key": "discovery.source", "value": "local_runtime" }
      ],
      "resource_attributes": [
        { "key": "resource.id", "value": "host-1" },
        { "key": "collection.kind", "value": "host_metrics" }
      ]
    }
  ]
}
```

当前 `value_type` 示例：

- `gauge_i64`
- `gauge_f64`
- `gauge_string`

### 19.4 当前实现边界

当前实现已经具备可运行闭环，但仍然属于 Batch A 的早期形态：

- 还没有真正的 exporter
- 还没有统一 interval / timeout / budget 调度
- 还没有 metrics uplink
- 还没有完整 filesystem / disk io / network / fd / thread / restart count 指标
- `container_metrics` 仍以 discovery hint 驱动为主，不依赖 runtime API
- `metrics_samples.json` 仍是本地中间态，不是最终对外协议

因此当前实现更适合定位为：

- 已可验证 discovery 能否支撑数据面
- 已可验证 planner -> runtime -> sample 的状态边界
- 已可作为后续 exporter / uplink / 调度器的稳定输入

---

## 20. 当前决定

当前阶段固定以下结论：

1. `discovery` 归属 `warp-insightd`，不归属 `wist-exec`
2. `discovery` 是常驻运行时，不是一次性任务
3. `discovery` 只产出本地事实与 target 候选，不直接做采集决策
4. `discovery` 状态独立于 execution 状态树
5. `discovery` 结果必须可同时服务数据面和控制面
