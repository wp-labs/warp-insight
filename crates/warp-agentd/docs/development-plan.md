# warp-agentd 开发计划

聚焦 `crates/warp-agentd` 的落地计划。当前主线是 **5 项**（W1–W5，按优先级排列）；
架构约束见 [agentd-architecture.md](./agentd-architecture.md)，全仓里程碑/backlog 见
[`doc/design/foundation/roadmap.md`](../../../doc/design/foundation/roadmap.md)、
[`implementation-backlog.md`](../../../doc/design/foundation/implementation-backlog.md)。

## 1. 当前基线（已实现/已验证）

- 模块按职责域目录化：`bootstrap / config / control / discovery / exec / reporting / runtime / state_store / telemetry`；
- `cargo test -p warp-agentd --lib` → **242 passed**；集成测试 `tests/local_exec` → **44 passed**；`wist-validate` → **32 passed**；
- 文件日志输入已具备：tail/head、rotate（rename/copytruncate）、truncate 重读、多行折叠、
  checkpoint、spool 重放；上送帧 = JSON 信封 + `RAW:`；
- `control/enrollment` 注册/续期已跑通（本机实测 agent 在线）；
- 执行链路可用：`scheduler` + `exec/local_exec` + `process_control` + `state_store`；
- 端到端：macOS P0 采集 → 数据平面（warp-gateway data-plane）已实测。

## 2. 主线 5 项（要完成的目标）

| # | 主线 | 完成定义（一句话） | 状态 |
|---|---|---|---|
| W1 | **先收好日志** | 文件日志采集可靠：断连/崩溃/轮转/截断/权限异常下不丢行、不重复、有界积压 | 完成（截断计数/权限恢复/spool 背压/崩溃重启 e2e 已落地） |
| W2 | **可以正确上报** | 采集与状态记录能正确上送：帧/批量/重试/顺序/去重正确，失败可恢复 | 进行中 |
| W3 | **收到指标** | agent 能采集并上送指标（Batch A：host/process 等），数据面可查到 | 待收口 |
| W4 | **指标扩展机制** | 新增一类指标/目标只需“加 spec + 映射（+ 可选 provider）”，不重编核心 | 基本落地（spec/映射/provider 三层已接，capability 协商已补；脚本型 opcode 后续） |
| W5 | **完成可升级** | 升级闭环：计划接收→互斥→下载校验→拉起 `wist-upgrader`→结果/版本上报→失败回滚 | 待启动 |

```mermaid
flowchart LR
    W1[W1 日志采集可靠] --> W2[W2 正确上报]
    W2 --> W3[W3 指标采集]
    W3 --> W4[W4 扩展机制]
    W4 --> W5[W5 可升级]
```

## 3. 各主线范围与验收

### W1 先收好日志

- 范围：`telemetry/logs/files`（文件输入）、`state_store/log_checkpoint_state`、`telemetry/spool`。
- 要补的可靠性点：
  1. 崩溃恢复：checkpoint（最后提交偏移）与 spool 组合，重启后不丢不重；
  2. 轮转组合：rename 轮转 + copytruncate + 多代轮转（`.1/.2`）——旧文件排空与新文件接续；
  3. 异常输入：超长行、无换行大文件、部分写入（半行）、文件被删除后重建、权限拒绝后恢复；
  4. 有界：内存预算 + spool 上限（避免无人消费时无限增长），背压时不丢数据只降速；
  5. checkpoint 原子写（崩溃窗口不产生坏状态）。
- 验收：`logs::files` 现有 29 用例保持通过 + 新增覆盖 1–5；集成测试跑真实文件写入/轮转/重启。
- 改动点：`telemetry/logs/files/*`、`telemetry/spool`、`state_store/log_checkpoint_state`。

落地进展（本轮）：

- 3 超长行：`max_line_bytes` 截断提交 + `truncated_lines` 计数（`ReadLimits`，见 `log-file-input-spec.md` §7.3）；
- 3 权限：源不可读时失败且 checkpoint 不动，恢复后从 checkpoint 续读（`tests/permissions.rs`）；
- 4 有界：spool 上限（`spool_max_bytes`）+ `pause` 背压（`spool_over_limit` 校验仅 `pause`），
  超限时停读停 checkpoint、回放至低水位自动恢复；暂停/恢复是**工作状态通知**（work-state notification，
  非告警、非失败），本地输出与随 `AgentHello.work_state_changes` 上报、gateway 落库均已落地（进入/退出各一次，见下方待办）；
- 4 分块：单轮 `max_read_bytes_per_tick` / `max_lines_per_tick` 在行边界停读，保证下次从行首继续。
- 5 边界语义：读取预算/截断/背压的精确语义与缺陷修复已由 5 轮 review 固化并沉淀到
  [`log-file-input-spec.md`](./log-file-input-spec.md) §7.6（含缺陷→修复→用例对照表）。
- 1 崩溃恢复：真实文件崩溃重启 e2e（`daemon_restart_recovers_checkpoint_without_loss_or_duplication`）
  验证首次运行持久化 checkpoint 后，重启从 checkpoint 续读新增行，不丢不重。

待办（follow-up，不阻塞 W1 收口）：

- [x] **工作状态通知（work-state notification）与上报**：把暂停/恢复统一为工作状态通知
  （`paused` / `resumed`，非告警、非失败），进入/退出各产生并**上报**一次，对齐
  [`log-file-input-spec.md`](./log-file-input-spec.md) §7.5 第 3 条与 §7.6。
  - 现状（已实现）：暂停/恢复从 `TelemetryFailureKind` 移出为 `TelemetryTick.notifications`，
    daemon 跨 tick 差值检测进入/退出，随 `AgentHello.work_state_changes` 上报，
    gateway 落库到 `StoredAgentRegistration.work_state_changes`。
  - 通道（已定）：控制面 `/api/v1/agent/status`——在 `AgentHello` 上新增**可选**字段携带
    “自上次上报以来的工作状态变化”，由 gateway 落库、center 展示（另开）。不采用数据平面上送：
    暂停恰恰因为 spool/上报不通，用被暂停的通道报“我暂停了”自相矛盾。
    - 契约：`AgentWorkStateChange { input_id, state: paused|resumed, reason, at }`，
      挂 `Option<Vec<AgentWorkStateChange>>` 并 `#[serde(default)]`。
    - 注意 `AgentHello` 有**两处定义**，需同时改：
      `wist_contracts::gateway::AgentHello`（agentd 序列化，`deny_unknown_fields`、
      `memory_bytes: Option<u64>`）与 `insight_control::AgentHello`（gateway 反序列化、simulator 构造，
      `memory_bytes: Option<i64>`）；只改一处会导致 gateway 收不到新字段。
    - 兼容：老 agent→新 gateway 由 `#[serde(default)]` 兜住；新 agent→老 gateway 因 gateway 侧
      `insight_control::AgentHello` 无 `deny_unknown_fields`，多余字段被忽略，不会拒绝。
    - `insight-simulator/src/agentd.rs::build_agent_hello` 的 struct 字面量需同步补新字段（或派生 `Default`）。
  - 实现顺序（已按此落地）：通道无关部分（`TelemetryTick.notifications` + 进入/退出检测 + 健康快照
    `paused_inputs`）与契约/上报/gateway 落库均已接入。
  - 难点：`FileInputProcessor` 每 tick 重建，过渡检测需跨 tick（daemon 已有“上一 tick 集合 + 差值”
    模式，无需落盘；若要跨重启对齐则需持久化）。
  - 验收：跨 tick 用例断言进入/退出各一次、中间 tick 不重复；上报端到端（gateway 可见）。
- [x] **`drop_oldest` 收敛**：已将 `spool_over_limit` 校验收敛为只允许 `pause`
  （`wist-validate` 拒绝 `drop_oldest`，消除“可配置但不生效”的误导）；
  按 input 优先级丢弃最旧 spool 记录留待后续按 §7.5/§12.2 落地。

### W2 可以正确上报

- 范围：上送通道（`telemetry/warp_parse` 的 TCP sink、`telemetry/spool` 重放）、控制面状态/能力上报（`control/`、`runtime/daemon`）。
- 要补的正确性点：
  1. 断连→重连：指数退避、连接复用、批量与超时边界；
  2. **去重**：每条记录带 per-`agent` 全局 `seq`（与 checkpoint 同次原子写），下游按 `(agent, seq)` 去重；
  3. 顺序与缺口：`seq` per-`agent` 全局单调不回退；缺口 = 被动丢失，接收端 watermark 检测（见 `data-loss-prevention.md` §8）；
  4. 失败语义：部分成功（已送部分推进 checkpoint）、失败落 spool、重放优先；
  5. 状态/能力上报：心跳、失败计数、版本与模式上报可被网关正确解析。
- 验收：mock TCP sink 的断连/重连/半写用例；重启/重放后数据面**去重后条数正确**；truncate/轮转后再采不被误判为重复；
  e2e：真实采集 → 数据面 `macos-agent.json` 内容与顺序正确、无重复；心跳在网关在线可见。
- 决策点（已决）：信封引入 per-`agent` 全局 `seq`，去重键 `(agent, seq)`。

落地进展（本轮）：

- 帧格式收敛：信封 `{schema, agent, ts, seq}` 短名，落成 `DataFrame` 结构体（`#[serde(rename)]`，见
  `wist-contracts::telemetry_record::DataFrame`），`build_record_frame` 不再手写 `json!`；
- 契约 `TelemetryRecordContract`：`+agent_id`、`-signal_kind`，`input_id`/来源字段留在契约内部（spool/路由用）、不进帧；
- `seq` 语义固化：per-`agent` 全局、读入取号（非发送时）、spool 存原号、发送原样发（`data-loss-prevention.md` §5.3）；
- `next_seq` 全局化：per-`agent` 全局高水位收进独立文件 `state/logs/seq.json`（`log_seq_state`），
  跨 input 共享同一个单调计数器（`&mut u64`）；每个 input 提交 checkpoint 前先把当时的全局值原子写回该文件
  （前移一位），重启后从该文件续号；误删单个 input 的 checkpoint 不回退号源。新增用例
  `daemon_run_once_assigns_globally_monotonic_seq_across_inputs`（跨 input 不撞号）与
  `deleting_checkpoint_does_not_regress_global_seq`（误删 checkpoint 不回退）；
- 通道分离：数据帧只表达被动丢失（洞），主动过滤走独立数据报告；`wist-delivery` 对齐——删 in-band 墓碑
  （`DropMarker`/`on_tombstone`）、删位置去重（`Deduper`/`RecordPosition`），去重收敛 `(agent, seq)`，
  主动过滤统一 `on_dropped_range`；
- 断连→重连：`TcpRecordSink` 增加连接/写超时（各 5s）+ 指数退避（1s 起、翻倍、30s 封顶），
  退避窗口内返回 `WouldBlock` 快速失败交给 spool、成功重置退避；新增用例 `tcp_sink_backs_off_after_connect_failure`；
- 下游 warp-parse 的 WPL/OML 已按新帧 `{schema, agent, ts, seq}` 改造（`models/wpl/macos_agent/parse.wpl` +
  `models/oml/macos_agent_record.oml` 均解析新信封，日志/指标帧均已接）；
- 测试：`warp-agentd` 248 lib + 45 integration、`wist-contracts` 16、`wist-delivery` 26 全绿。

待办（本主线内，未完成）：

- Jumo 模型同步（`FileInputConfig.agent_id` 已入代码，`.mju` 待同步）；
- 接收端（gateway/center）接入 `wist-delivery`（缺口检测 + 去重，见 `data-loss-prevention.md` §11 待落地）。

### W3 收到指标

- 范围：`telemetry/metrics/{runtime,samples,target_view}`、`exec/planner_bridge`（discovery → 采集候选）、上送通道复用。
- 要补的点：
  1. 指标链路闭环：discovery targets → 目标视图 → 采集采样 → 规范化记录 → 上送；
  2. Batch A 指标（host/process 等）在数据面/VM 可查（`warp-*` 或约定 series）；
  3. 采集失败/缺目标的降级与计数；
  4. 与日志共用 uplink 的互不影响（日志洪峰不挤掉指标，反之亦然）。
- 验收：单测覆盖 target_view/samples 生成；集成：采集 → 上报 → 数据面查询可见；日志与指标并发压测无相互阻塞。

落地进展（本轮）：

- 指标帧序列化 + uplink 接入：`TcpRecordSink::write_metrics`（` METRICS:` 帧）已接入 daemon 主循环——
  每 tick 在 `process_metrics_tick` 产出运行时快照后，经 `samples::build_samples_snapshot` 规范化，
  与日志共用同一 TCP sink 上送（指标优先、无样本时跳过）；`write_metrics` 的 `#[allow(dead_code)]` 已移除。
- 集成覆盖：`tests/local_exec/daemon_file_input` 验证指标帧与日志帧共连、指标先于日志帧。
- 指标帧 `seq` 已接入 agent 级全局 `seq`（与日志同源、跨重启不回退）：daemon 每 tick 加载一次全局计数器，
  指标先取号（前移持久化到 `state/logs/seq.json`）再发送、日志续号，消除 `batch_seq` 占位导致的日志/指标撞号。
  新增用例 `write_metrics_uplink_draws_from_global_seq_and_persists`；
- **Batch A 采集补齐（host/process/disk，跨 Linux/macOS）**：
  - 采集库：host + disk 用 [`sysinfo`](https://crates.io/crates/sysinfo) 0.36（MSRV 1.85 约束下能用的最高版；
    0.39 需 rust 1.95，暂不升）；process 因 sysinfo 在 macOS 用 `proc_pidinfo` 读不到 root/他用户进程（`EPERM`），
    保留手写（Linux `/proc` + macOS `ps`）。
  - host：`system.target.count`、`system.load_average.{1m,5m,15m}`、`system.uptime`、
    `system.memory.{total,available}`、`system.disk.{usage,total,available}`（`System` + `Disks`）。
  - process：`process.memory.rss`、`process.state`；Linux 读 `/proc/<pid>/stat`，macOS 走 `ps -o state=,rss=,comm=`。
    **进程 CPU 的 user/system 累计 ticks 暂不实现**（sysinfo 只有总量/百分比，不做拆分）。
  - disk：`Disks`；macOS 先选 `/System/Volumes/Data`（APFS 数据卷）再回退 `/`
    （`/` 在 macOS 只反映封存系统卷，非用户数据）；`usage = (total − available) / total`。
  - 实测：macOS P0 实例（host 1 + process ~1044 目标）在 VM 可查到上述 series。

### W4 指标扩展机制（设计 + 落地）

**目标**：新增一类指标或一类目标时，只需“加 spec + 映射”，不必改核心引擎。

**三层结构**

| 层 | 职责 | 扩展点 |
|---|---|---|
| 契约层（spec） | 声明“采什么”：`MetricSpec{ name, unit, target_selector, value_type, labels, provider, interval }` | 新增 spec（契约/模型仓），不写实现细节 |
| 映射层（`exec/planner_bridge`） | discovery 快照 → 目标视图 → `CollectionPlan`（候选） | selector 规则（按 target kind/labels 匹配） |
| 采集层（provider） | 执行采集、产出样本：`MetricProvider{ id, supports(spec), collect(target) -> samples }` | 注册新 provider（v1 编译期注册；脚本型走 `wist-exec` opcode） |

**数据流**

```mermaid
flowchart LR
    D[discovery snapshot] --> M[planner_bridge: selector 映射]
    M --> P[CollectionPlan]
    P --> PR[provider.collect]
    PR --> S[规范化样本记录]
    S --> U[uplink: 信封 + 通道]
```

**协商**：`capability_report` 声明 agent 支持的 providers / discovery modes / metric 家族；
中心只下发 agent 声明支持的计划（避免下发无法执行的指标）。

**扩展步骤**（以新增“磁盘使用率”为例）

1. 契约加 `MetricSpec{ name: disk.usage_ratio, target_selector: kind=host, provider: local_runtime }`；
2. `planner_bridge` selector 命中 `kind=host` 目标 → 生成采集计划；
3. 若现有 provider 覆盖不到 → 新增 provider（如 `fs_stat`）并注册，或在 `wist-exec` 增加采集 opcode（脚本型，无需重编 agent）；
4. `capability_report` 声明 provider id；
5. 测试：provider 单测 + 计划生成测试 + 上送 e2e。

**边界纪律**：provider 只产出样本（不做上报决策）；uplink 不做采集选择；spec 不含实现细节
（采集命令/路径属于 provider 或 opcode）。

**落地顺序**：先固化 spec 契约与 `CollectionPlan`，再把现有 Batch A 采集改成“走 provider 接口”，
最后补 capability 协商与脚本型 opcode。

**落地进展**：spec（`spec.rs` `MetricSpec` + `METRIC_SPECS`）、映射（`planner_bridge::build_collection_candidates`）、
provider（`MetricProvider` + 静态注册表）三层已落地，Batch A 已走 provider 接口；capability 协商已把
provider 的 collection_kind 声明进 `collectors`（`capability_report.rs::metrics_capabilities`）。
脚本型 opcode 留待后续（§7 已记）。

**结构拆分**：采集契约（`MetricProvider` trait + outcome/sample/target-entry 类型 + spec 表）已下沉到
共享 crate `wist-metrics`，`warp-agentd` 仅再导出（`telemetry/metrics/{runtime,target_view,spec}.rs`）。
非系统指标（Postgres/MySQL/…）的 provider 与重依赖（`sqlx` 等）放 `wist-metrics`（feature 门控），
不进入 `warp-agentd` 默认依赖图；系统指标（host/process/container）provider 仍在 `warp-agentd`。

### W5 完成可升级

- 范围：`control/`（升级计划接收与结果上报）、`runtime/scheduler`（互斥与编排）、`wist-upgrader`（执行体，独立 crate）。
- 闭环步骤：
  1. 接收升级计划/指令（managed；standalone 走本地辅助）；
  2. 与远程动作互斥（升级进行中不并发执行其他动作）；
  3. 制品下载与校验（哈希/签名、版本兼容）；
  4. 拉起 `wist-upgrader` 执行升级；监控子进程（复用 `process_control`）；
  5. 版本与结果上报（成功/失败/回滚原因）；
  6. 失败回滚与 crash 恢复（重启后能从状态恢复或安全退出）。
- 验收：mock 制品仓库的升级 e2e（成功/校验失败/中断恢复/回滚）；互斥用例；版本上报在网关/中心可见。

## 4. 支撑工作（服务于主线，非独立里程碑）

| 支撑项 | 服务于 | 说明 |
|---|---|---|
| 运行模式与开关矩阵（standalone/managed） | W2/W3 | standalone 下遥测链路不运行；“配了也不生效”需校验（架构 §2.2） |
| 执行状态模型正交化（lifecycle/exec_phase/outcome/signals） | W5（+A5 取消/超时） | 架构 §7；升级/取消/超时语义依赖它 |
| Plan 校验与 `rejected` 路径 | W3/W5 | 非法计划/升级包不入队 |
| 审计事件最小集 | W5 | 升级/取消/结果归档事件 |
| 覆盖率与复杂度门禁 | 全部 | `jumo-code code-quality` |

## 5. 顺序与依赖

- W1 → W2 是硬依赖（先收好，才谈正确上报）；
- W3 在 W2 之上复用同一 uplink；W4 是 W3 的工程化前提（先把机制定下来，再扩指标）；
- W5 依赖 W2（结果上报）与支撑项（状态模型、互斥）；
- 支撑项按需插入，不阻塞 W1/W2。

## 6. 总验收

```bash
# 单元 + 集成
cargo test -p warp-agentd

# 质量报告（写入 Studio 读取的同一份报告）
jumo-code code-quality /Users/zuowenjian/devspace/rust/x-topology/warp-insight \
  --out /Users/zuowenjian/devspace/rust/x-topology/warp-insight/jumo/model/impl/code-quality.json

# 覆盖率（同口径采集后导入）
cargo llvm-cov --workspace --json --output-path coverage.json
jumo-code code-quality <repo>/warp-insight --coverage <repo>/warp-insight/coverage.json \
  --out <repo>/warp-insight/jumo/model/impl/code-quality.json
```

完成定义：该主线的验收用例入库并通过 + 端到端实测（真实日志/指标/升级）+ 文档同步。

## 7. 待决策点

1. ~~W1/W2：信封是否引入 `seq`~~ → **已决**：per-`agent` 全局 `seq`，`next_seq` 存独立文件 `state/logs/seq.json`（先于 checkpoint 前移原子写）；
   下游按 `(agent, seq)` 去重（`input_id` 留在契约内部做 spool/路由、不进帧），见 `data-loss-prevention.md` §5.3/§7；
2. ~~spool 上限与背压策略~~ → **已决**：“暂停采集 + 告警”（保完整），见 `log-file-input-spec.md` §7.5/§12；
3. ~~**W4**：provider 的扩展方式~~ → **已决**：编译期注册（`MetricProvider` trait + 静态注册表）；脚本型指标统一走 `wist-exec` opcode 后续再议；
4. ~~**W3**：指标与日志是否共用同一 uplink 通道~~ → **已决**：共用同一 TCP 连接，信封不动、靠帧标记 ` RAW:`/` METRICS:` 区分，指标优先 + 背压隔离（见 `metrics-integration-roadmap.md` §11）；
5. **W5**：升级制品来源（网关/对象存储）与验签信任根——待定；
6. **W1**：目录/新文件输入（L`/Library/Logs/DiagnosticReports/*.ips`）是否纳入 W1（建议划 Phase2）——待定；
7. **W1**：长行上限默认值（1 MiB）是否合适（大日志平台上是否有更优默认）——待定。
8. **下游水位回传（W2 后续，方案已定、落地待排期）**：本地号源（`state/logs/seq.json` 或单个 input checkpoint）误删后，
   agent 从下游对账恢复正确起点，根治 `(agent, seq)` 撞号丢数据：
   - 水位值 = `wist-delivery::QualityChannel::committed()`（已结算前缀）；agent 恢复取 `max(本地 next_seq, downstream committed)`；
   - 三段链路：数据面接收端（warp-parse 维护 `QualityChannel`）→ 控制面（`warp-gateway`）→ agent（状态响应回传）；
   - 契约：`AgentStatusAccepted` 加 `committed_seq: Option<u64>`（`#[serde(default)]`，响应类型需下沉 `wist-contracts` 共享）；
     agent 侧 `report_status_to_control_plane` 改为解析响应 body（当前只看 HTTP 状态码）；
   - 分阶段：P1 agent↔控制面打通（不含数据面）→ P2 数据面接入 `wist-delivery` 后上报水位 → P3 启动门控（可选）；
   - 待定：① 水位语义（`committed()` vs 最高已见 seq，建议前者）；② 数据面→控制面通道（代报 / 内部 RPC / 共享存储）；③ 是否做 P3。

已决项（写入 `log-file-input-spec.md`）：超长行 = 截断提交 + 计数（§7.3）；spool 超限 = 暂停采集 + 告警（§7.5/§12）；
源日志默认不清理（§15）。
