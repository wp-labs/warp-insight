# warp-agentd 开发计划

聚焦 `crates/warp-agentd` 的落地计划。当前主线是 **5 项**（W1–W5，按优先级排列）；
架构约束见 [agentd-architecture.md](./agentd-architecture.md)，全仓里程碑/backlog 见
[`doc/design/foundation/roadmap.md`](../../../doc/design/foundation/roadmap.md)、
[`implementation-backlog.md`](../../../doc/design/foundation/implementation-backlog.md)。

## 1. 当前基线（已实现/已验证）

- 模块按职责域目录化：`bootstrap / config / control / discovery / exec / reporting / runtime / state_store / telemetry`；
- `cargo test -p warp-agentd --lib` → **201 passed**；集成测试 `tests/local_exec` → **42 passed**；`wist-validate` → **32 passed**；
- 文件日志输入已具备：tail/head、rotate（rename/copytruncate）、truncate 重读、多行折叠、
  checkpoint、spool 重放；上送帧 = JSON 信封 + `RAW:`；
- `control/enrollment` 注册/续期已跑通（本机实测 agent 在线）；
- 执行链路可用：`scheduler` + `exec/local_exec` + `process_control` + `state_store`；
- 端到端：macOS P0 采集 → 数据平面（warp-gateway data-plane）已实测。

## 2. 主线 5 项（要完成的目标）

| # | 主线 | 完成定义（一句话） | 状态 |
|---|---|---|---|
| W1 | **先收好日志** | 文件日志采集可靠：断连/崩溃/轮转/截断/权限异常下不丢行、不重复、有界积压 | 接近完成（截断计数/权限恢复/spool 背压已落地，待真实文件崩溃重启 e2e） |
| W2 | **可以正确上报** | 采集与状态记录能正确上送：帧/批量/重试/顺序/去重正确，失败可恢复 | 进行中 |
| W3 | **收到指标** | agent 能采集并上送指标（Batch A：host/process 等），数据面可查到 | 待收口 |
| W4 | **指标扩展机制** | 新增一类指标/目标只需“加 spec + 映射（+ 可选 provider）”，不重编核心 | 待设计落地 |
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
- 4 有界：spool 上限（`spool_max_bytes`）+ `pause` 背压（`spool_over_limit` 校验 `pause|drop_oldest`），
  超限时停读停 checkpoint、回放至低水位自动恢复；暂停/恢复是**工作状态通知**（work-state notification，
  非告警、非失败），当前仅本地输出且只覆盖“进入”，**上报待补**（见下方待办）；
- 4 分块：单轮 `max_read_bytes_per_tick` / `max_lines_per_tick` 在行边界停读，保证下次从行首继续。
- 5 边界语义：读取预算/截断/背压的精确语义与缺陷修复已由 5 轮 review 固化并沉淀到
  [`log-file-input-spec.md`](./log-file-input-spec.md) §7.6（含缺陷→修复→用例对照表）。

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
- [ ] **`drop_oldest` 落地或收敛**：按 input 优先级丢弃最旧 spool 记录并记录 drop reason
  （§7.5/§12.2）。若暂不做，则将 `spool_over_limit` 校验收敛为只允许 `pause`
  （避免“可配置但不生效”的误导）；否则补齐 `drop_oldest` 的丢弃/计数/告警用例。

### W2 可以正确上报

- 范围：上送通道（`telemetry/warp_parse` 的 TCP sink、`telemetry/spool` 重放）、控制面状态/能力上报（`control/`、`runtime/daemon`）。
- 要补的正确性点：
  1. 断连→重连：指数退避、连接复用、批量与超时边界；
  2. **去重（方案 B）**：每条记录带 per-input `seq`（与 checkpoint 同次原子写），下游按 §11.1.1 规则去重；
  3. 顺序与缺口：同 `(input_id, file_id)` 内 `seq` 不回退；缺口作为丢行告警信号；
  4. 失败语义：部分成功（已送部分推进 checkpoint）、失败落 spool、重放优先；
  5. 状态/能力上报：心跳、失败计数、版本与模式上报可被网关正确解析。
- 验收：mock TCP sink 的断连/重连/半写用例；重启/重放后数据面**去重后条数正确**；truncate/轮转后再采不被误判为重复；
  e2e：真实采集 → 数据面 `macos-agent.json` 内容与顺序正确、无重复；心跳在网关在线可见。
- 决策点：信封是否引入 `seq`（影响去重与顺序的最终形态）。

### W3 收到指标

- 范围：`telemetry/metrics/{runtime,samples,target_view}`、`exec/planner_bridge`（discovery → 采集候选）、上送通道复用。
- 要补的点：
  1. 指标链路闭环：discovery targets → 目标视图 → 采集采样 → 规范化记录 → 上送；
  2. Batch A 指标（host/process 等）在数据面/VM 可查（`warp-*` 或约定 series）；
  3. 采集失败/缺目标的降级与计数；
  4. 与日志共用 uplink 的互不影响（日志洪峰不挤掉指标，反之亦然）。
- 验收：单测覆盖 target_view/samples 生成；集成：采集 → 上报 → 数据面查询可见；日志与指标并发压测无相互阻塞。

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

1. ~~W1/W2：信封是否引入 `seq`~~ → **已决（方案 B）**：per-input `seq`，`next_seq` 与 checkpoint 同次原子写；
   下游按 `(agent_id, input_id, seq)` + 位置辅助判据去重，见 `log-file-input-spec.md` §11.1.1；
2. ~~spool 上限与背压策略~~ → **已决**：“暂停采集 + 告警”（保完整），见 `log-file-input-spec.md` §7.5/§12；
3. **W4**：provider 的扩展方式——编译期注册（v1 建议）还是允许运行期加载；脚本型指标是否统一走 `wist-exec` opcode——待定；
4. **W3**：指标与日志是否共用同一 uplink 通道（建议共用但分优先级）——待定；
5. **W5**：升级制品来源（网关/对象存储）与验签信任根——待定；
6. **W1**：目录/新文件输入（L`/Library/Logs/DiagnosticReports/*.ips`）是否纳入 W1（建议划 Phase2）——待定；
7. **W1**：长行上限默认值（1 MiB）是否合适（大日志平台上是否有更优默认）——待定。

已决项（写入 `log-file-input-spec.md`）：超长行 = 截断提交 + 计数（§7.3）；spool 超限 = 暂停采集 + 告警（§7.5/§12）；
源日志默认不清理（§15）。
