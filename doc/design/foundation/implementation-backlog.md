# warp-insight 首批实现 Backlog

## 1. 文档目的

本文档把 [`roadmap.md`](roadmap.md) 中已经拆分好的里程碑，进一步落成可执行的首批实现 backlog。

这里的目标不是定义所有长期工作，而是回答三件事：

- 现在可以立刻开哪些任务
- 这些任务建议落到哪些模块 / 文件
- 每项任务完成后，以什么结果算“做完”

当前仓库还没有代码目录，因此本文中的模块和文件布局属于建议实现骨架。

相关文档：

- [`roadmap.md`](roadmap.md)
- [`glossary.md`](glossary.md)
- [`../../crates/wist-agentd/docs/agentd-architecture.md`](../../crates/wist-agentd/docs/agentd-architecture.md)
- [`../../crates/wist-agentd/docs/agentd-state-schema.md`](../../crates/wist-agentd/docs/agentd-state-schema.md)
- [`../../crates/wist-agentd/docs/log-file-state-schema.md`](../../crates/wist-agentd/docs/log-file-state-schema.md)
- [`../execution/action-plan-schema.md`](../execution/action-plan-schema.md)
- [`../center/agent-gateway-protocol.md`](../center/agent-gateway-protocol.md)

---

## 2. 当前判断

基于当前设计文档，已经足够启动首批开发。

建议启动顺序：

1. `M1` 契约与校验器
2. `M3` `standalone` 三进程骨架
3. `M4` `standalone` 可替代切片
4. `M2` 身份与 enrollment 基线
5. `M5` gateway session
6. `M6` controlled action MVP
7. `M7/M8/M9` discovery + telemetry core + Batch A metrics
8. `M10/M11` control center core + dispatch 闭环

还不建议优先启动：

- `M16+` 的 scale-out gateway / tree topology
- `M19` 的 Batch B/C integrations
- `M20` 的 AI / authoring
- 在 `M4` 成立之前，不建议把 `managed` 接入当成首个产品验证目标

---

## 3. 建议代码布局

当前仓库没有代码，可以按下面的最小布局起步：

```text
warp-insight/
  doc/
  crates/
    wist-contracts/
    wist-validate/
    wist-shared/
    warp-insightd/
    wist-exec/
    wist-upgrader/
    wist-gateway/
    warp-insight-control/
  fixtures/
    contracts/
    action-plans/
    action-results/
    telemetry/
  tests/
    e2e/
```

模块角色：

- `wist-contracts/`
  所有 schema 对象、枚举、serde 类型、版本字段
- `wist-validate/`
  独立校验器、约束检查器、负例测试
- `wist-shared/`
  错误码、通用 paths、ids、时间工具、配置加载
- `warp-insightd/`
  边缘常驻 daemon、state store、scheduler、telemetry runtime
- `wist-exec/`
  `ActionPlan` runtime、opcode dispatch、`ActionResult`
- `wist-upgrader/`
  prepare / switch / health-check / rollback
- `wist-gateway/`
  南向 session、hello、heartbeat、dispatch、ack/result 通道
- `warp-insight-control/`
  request / approval / compile / sign / dispatch / tracker

---

## 4. P0 Backlog

### 4.1 B001 合同类型定义

对应里程碑：

- `M1`

建议模块：

- `crates/wist-contracts/src/action_plan.rs`
- `crates/wist-contracts/src/action_result.rs`
- `crates/wist-contracts/src/capability_report.rs`
- `crates/wist-contracts/src/gateway.rs`
- `crates/wist-contracts/src/agent_config.rs`
- `crates/wist-contracts/src/state_exec.rs`

完成定义：

- 所有核心对象有可序列化类型
- `schema_version` / `api_version` / `kind` 固定值可表达
- 字段命名与设计文档一致

### 4.2 B002 合同校验器

对应里程碑：

- `M1`

建议模块：

- `crates/wist-validate/src/action_plan.rs`
- `crates/wist-validate/src/action_result.rs`
- `crates/wist-validate/src/config.rs`
- `crates/wist-validate/src/state.rs`

完成定义：

- 非法 `ActionPlan` 能被静态拒绝
- 非法目标、非法约束、过期时间、graph 结构错误都有明确错误码
- 校验器可被 CLI、`agentd`、control center 复用

### 4.3 B003 正反样例集

对应里程碑：

- `M1`

建议目录：

- `fixtures/contracts/action-plan/valid/`
- `fixtures/contracts/action-plan/invalid/`
- `fixtures/contracts/action-result/valid/`
- `fixtures/contracts/config/`

完成定义：

- 每类核心对象至少有一组 valid/invalid fixtures
- CI 可以批量回放校验

### 4.4 B004 `warp-insightd` skeleton

对应里程碑：

- `M3`

建议模块：

- `crates/warp-insightd/src/main.rs`
- `crates/warp-insightd/src/bootstrap.rs`
- `crates/warp-insightd/src/config_runtime.rs`
- `crates/warp-insightd/src/state_store/mod.rs`
- `crates/warp-insightd/src/self_observability.rs`

完成定义：

- `warp-insightd` 可独立启动
- 能初始化 `run/`、`state/`、`log/` 目录
- 能加载配置并进入 `standalone` 常驻主循环

### 4.5 B005 `wist-exec` skeleton

对应里程碑：

- `M3`

建议模块：

- `crates/wist-exec/src/main.rs`
- `crates/wist-exec/src/runtime.rs`
- `crates/wist-exec/src/workdir.rs`
- `crates/wist-exec/src/result_writer.rs`

完成定义：

- 能读取 `plan.json` / `runtime.json`
- 能写出 `state.json` / `result.json`
- 暂无真实 opcode 也可完成最小空执行

### 4.6 B006 `wist-upgrader` skeleton

对应里程碑：

- `M3`

建议模块：

- `crates/wist-upgrader/src/main.rs`
- `crates/wist-upgrader/src/prepare.rs`
- `crates/wist-upgrader/src/switch.rs`
- `crates/wist-upgrader/src/rollback.rs`

完成定义：

- 二进制可启动
- 预留 prepare / switch / rollback 命令面
- 与 `warp-insightd` 的工作目录和状态边界清晰

### 4.7 B007 execution state store

对应里程碑：

- `M1`
- `M3`

建议模块：

- `crates/warp-insightd/src/state_store/agent_runtime.rs`
- `crates/warp-insightd/src/state_store/execution_queue.rs`
- `crates/warp-insightd/src/state_store/running.rs`
- `crates/warp-insightd/src/state_store/reporting.rs`
- `crates/warp-insightd/src/state_store/history.rs`

完成定义：

- 对应 schema 的状态文件能原子落盘
- 唯一写入权边界在代码里明确

### 4.8 B008 logs checkpoint store

对应里程碑：

- `M1`
- `M3`
- `M4`
- `M8`

建议模块：

- `crates/warp-insightd/src/state_store/log_checkpoints.rs`

完成定义：

- 能读写 `state/logs/file_inputs/<input_id>/checkpoints.json`
- `checkpoint_offset` 与 `commit point` 语义被清晰编码

### 4.9 B009 `agentd <-> exec` local protocol

对应里程碑：

- `M3`

建议模块：

- `crates/warp-insightd/src/executor_protocol.rs`
- `crates/wist-exec/src/protocol.rs`
- `crates/wist-shared/src/paths.rs`

完成定义：

- `plan.json` / `runtime.json` / `result.json` / `state.json` 的最小交互协议固定
- `warp-insightd` 可稳定发现执行开始、执行结束、异常退出和取消结果
- 本地协议边界不依赖中心长连接是否存在

### 4.10 B010 runtime lifecycle / recovery baseline

对应里程碑：

- `M3`

建议模块：

- `crates/warp-insightd/src/supervisor.rs`
- `crates/warp-insightd/src/recovery.rs`
- `crates/warp-insightd/src/self_observability.rs`
- `crates/wist-shared/src/error_codes.rs`

完成定义：

- `warp-insightd` 具备最小健康检查与生命周期管理骨架
- 基础错误码、panic 归因和 restart/recover 路径可落到统一口径
- 本地恢复不会破坏已有 `state/` 与 workdir 边界

### 4.11 B011 standalone mode + test harness

对应里程碑：

- `M3`

建议模块：

- `crates/warp-insightd/src/bootstrap.rs`
- `crates/warp-insightd/src/config_runtime.rs`
- `tests/e2e/standalone_smoke.rs`
- `fixtures/runtime/standalone/`

完成定义：

- `control_plane.enabled = false` 的启动与运行路径固定
- 最小 smoke / integration harness 可覆盖 standalone 启动、状态目录初始化和空执行闭环
- `M3` 基线能力可以在 CI 中回归验证

### 4.12 B012 standalone replacement slice

对应里程碑：

- `M4`

建议模块：

- `crates/warp-insightd/src/telemetry/logs/file_input.rs`
- `crates/warp-insightd/src/telemetry/logs/file_watcher.rs`
- `crates/warp-insightd/src/telemetry/logs/file_reader.rs`
- `crates/warp-insightd/src/telemetry/logs/multiline.rs`
- `crates/warp-insightd/src/telemetry/logs/parser.rs`
- `crates/warp-insightd/src/telemetry/buffer.rs`
- `crates/warp-insightd/src/telemetry/spool.rs`
- `crates/warp-insightd/src/telemetry/warp_parse.rs`

完成定义：

- `standalone` 下可用受控单路径 `file input -> parser / multiline -> checkpoint -> buffer / spool -> warp-parse/file output`
- `checkpoint_offset` 只在越过 `commit point` 后推进
- 正常 append、rotate / truncate / restart recovery 正确性基线成立
- 至少覆盖一类明确的 `Fluent Bit tail input` 替代场景
- 验证的是能力与运行时行为，不要求兼容 `Fluent Bit` 配置
- 不要求在 `M4` 阶段完成通用 discovery / watcher / protect / 完整自观测

---

## 5. P1 Backlog

### 5.1 B101 identity runtime

对应里程碑：

- `M2`

建议模块：

- `crates/wist-shared/src/identity.rs`
- `crates/warp-insightd/src/identity/mod.rs`

完成定义：

- `agent_id / instance_id / boot_id` 生成与持久化成立
- 首次启动与重启语义区分清楚

### 5.2 B102 enrollment client

对应里程碑：

- `M2`
- `M5`

建议模块：

- `crates/warp-insightd/src/identity/enroll.rs`
- `crates/wist-gateway/src/enroll_api.rs`

完成定义：

- 新节点可完成首次 enrollment
- 重复实例可被识别

### 5.3 B103 gateway session client/server

对应里程碑：

- `M5`

建议模块：

- `crates/warp-insightd/src/gateway_client.rs`
- `crates/wist-gateway/src/session.rs`
- `crates/wist-gateway/src/lease.rs`

完成定义：

- `hello`、heartbeat、reconnect 跑通
- capability 上报可见

### 5.4 B104 capability report implementation

对应里程碑：

- `M5`

建议模块：

- `crates/warp-insightd/src/capability_report.rs`

完成定义：

- 边缘可上报 `exec` / `metrics` / `logs` / `upgrade`

### 5.5 B105 controlled action scheduler

对应里程碑：

- `M6`

建议模块：

- `crates/warp-insightd/src/control_receiver.rs`
- `crates/warp-insightd/src/plan_validator.rs`
- `crates/warp-insightd/src/execution_scheduler.rs`
- `crates/warp-insightd/src/executor_manager.rs`
- `crates/warp-insightd/src/result_aggregator.rs`

完成定义：

- `DispatchActionPlan -> queue -> exec -> ack/result` 跑通
- 成功、失败、取消、超时路径齐全

### 5.6 B106 opcode runtime v1

对应里程碑：

- `M6`

建议模块：

- `crates/wist-exec/src/opcodes/process.rs`
- `crates/wist-exec/src/opcodes/socket.rs`
- `crates/wist-exec/src/opcodes/service.rs`
- `crates/wist-exec/src/opcodes/file.rs`
- `crates/wist-exec/src/opcodes/config.rs`
- `crates/wist-exec/src/opcodes/agent.rs`

完成定义：

- 首批只读 opcode 都有实现和测试

### 5.7 B107 resource discovery foundation

对应里程碑：

- `M7`

建议模块：

- `crates/warp-insightd/src/discovery/host.rs`
- `crates/warp-insightd/src/discovery/process.rs`
- `crates/warp-insightd/src/discovery/container.rs`
- `crates/warp-insightd/src/discovery/k8s.rs`
- `crates/warp-insightd/src/discovery/cache.rs`

完成定义：

- 可稳定产出 host/process/container 基础资源视图
- resource identity 去重成立

### 5.8 B108 telemetry record envelope

对应里程碑：

- `M4`
- `M8`

建议模块：

- `crates/wist-contracts/src/telemetry_record.rs`
- `crates/warp-insightd/src/telemetry/envelope.rs`
- `crates/warp-insightd/src/telemetry/normalize.rs`

完成定义：

- `M4` 阶段至少能把 logs 编码到统一 record 骨架
- `M8` 阶段再扩展到 `metrics / traces / security`

### 5.9 B109 telemetry runtime core

对应里程碑：

- `M4`
- `M8`

建议模块：

- `crates/warp-insightd/src/telemetry/input_router.rs`
- `crates/warp-insightd/src/telemetry/buffer.rs`
- `crates/warp-insightd/src/telemetry/spool.rs`
- `crates/warp-insightd/src/telemetry/exporter.rs`
- `crates/warp-insightd/src/telemetry/warp_parse.rs`

完成定义：

- `M4` 阶段 record 可进入本地 buffer/spool 并输出到 `warp-parse/file`
- `M8` 阶段再补通用 telemetry budget、backpressure 与保护状态

### 5.10 B110 file input runtime

对应里程碑：

- `M4`
- `M8`
- `M18`

建议模块：

- `crates/warp-insightd/src/telemetry/logs/file_input.rs`
- `crates/warp-insightd/src/telemetry/logs/file_watcher.rs`
- `crates/warp-insightd/src/telemetry/logs/file_reader.rs`
- `crates/warp-insightd/src/telemetry/logs/multiline.rs`
- `crates/warp-insightd/src/telemetry/logs/parser.rs`

完成定义：

- `M4` 阶段 `file input` 可对显式配置的单路径按 `read offset` 持续读取
- `commit point` 与 `checkpoint_offset` 推进成立
- rotate / truncate / multiline 基线成立
- `M8` 阶段再扩展为通用 file input runtime：
  - `path_patterns[]` / `exclude_path_patterns[]`
  - refresh / 新文件发现
  - `auto / native_notify / poll`
  - 完整自观测与 `degraded / protect`

### 5.11 B111 Batch A metrics integrations

对应里程碑：

- `M9`

建议模块：

- `crates/warp-insightd/src/telemetry/metrics/host_metrics.rs`
- `crates/warp-insightd/src/telemetry/metrics/process_metrics.rs`
- `crates/warp-insightd/src/telemetry/metrics/container_metrics.rs`
- `crates/warp-insightd/src/telemetry/metrics/k8s_node_pod_metrics.rs`
- `crates/warp-insightd/src/telemetry/metrics/prom_scrape.rs`
- `crates/warp-insightd/src/telemetry/metrics/otlp_metrics_receiver.rs`

完成定义：

- Batch A 指标可稳定采集并绑定 resource

### 5.12 B112 control center core

对应里程碑：

- `M10`

建议模块：

- `crates/warp-insight-control/src/api.rs`
- `crates/warp-insight-control/src/registry.rs`
- `crates/warp-insight-control/src/request_store.rs`
- `crates/warp-insight-control/src/query.rs`
- `crates/warp-insight-control/src/result_ingest.rs`

完成定义：

- request / tracker / result query 成立

### 5.13 B113 approval / signing / dispatch

对应里程碑：

- `M11`

建议模块：

- `crates/warp-insight-control/src/approval.rs`
- `crates/warp-insight-control/src/compiler.rs`
- `crates/warp-insight-control/src/signer.rs`
- `crates/warp-insight-control/src/dispatch.rs`

完成定义：

- approval -> compile -> sign -> dispatch -> ack/result 闭环成立

### 5.14 B114 install bootstrap

对应里程碑：

- `M12`

建议目录：

- `packaging/`
- `scripts/install/`

完成定义：

- 新节点可安装并拉起 `warp-insightd`

### 5.15 B115 upgrade / rollback

对应里程碑：

- `M13`

建议模块：

- `crates/wist-upgrader/src/download.rs`
- `crates/wist-upgrader/src/verify.rs`
- `crates/wist-upgrader/src/health_check.rs`

完成定义：

- 单节点升级回滚成立

### 5.16 B116 security baseline

对应里程碑：

- `M14`

建议模块：

- `crates/wist-validate/src/attestation.rs`
- `crates/wist-exec/src/allow.rs`
- `crates/wist-shared/src/error_codes.rs`

完成定义：

- 签名校验、allow 控制、最小权限约束成立

### 5.17 B117 audit hardening

对应里程碑：

- `M15`

建议模块：

- `crates/warp-insight-control/src/audit.rs`
- `crates/warp-insightd/src/audit_logger.rs`

完成定义：

- `request_id -> action_id -> dispatch_id -> execution_id` 全链路审计成立

---

## 6. 首批交付包建议

如果按最少风险切成三个交付包，建议：

### 6.1 Package A

- B001-B012

结果：

- 可启动 `standalone` agent
- 可完成一条 `standalone` 文件日志替代验证切片
- 可直接验证“是否能替代部分 `Fluent Bit` 工作”

### 6.2 Package B

- B101-B110

结果：

- 可完成 enrollment 和 gateway session
- 可执行只读 action
- discovery / telemetry core 逐步通用化
- 为后续 control center 接入做好边缘侧准备

### 6.3 Package C

- B111-B117

结果：

- Batch A metrics、control center、安装、升级、安全、审计形成上线前基线

---

## 7. 当前最适合立刻创建的 issue

建议立刻建 16 个 issue：

1. contracts: 定义 `ActionPlan` / `ActionResult` / gateway envelopes
2. validate: 建立 schema validator 与 fixtures
3. agentd: 建立 daemon skeleton 与 state root
4. agent-exec: 建立 workdir 协议与空 runtime
5. upgrader: 建立 skeleton
6. standalone logs: `file input` + watcher + reader
7. standalone logs: parser + multiline + rotate / truncate
8. standalone telemetry: checkpoint / commit point + buffer/spool + `warp-parse` uplink
9. harness: standalone replacement scenario replay / soak baseline
10. identity: `agent_id / instance_id / boot_id` 与 enrollment
11. gateway: hello / heartbeat / reconnect
12. action: scheduler + first read-only opcodes
13. discovery: host/process/container foundation
14. metrics: Batch A integrations
15. control center: request/query/dispatch core
16. local protocol / runtime: `agentd <-> exec` workdir + health / error codes / panic-restart / recovery baseline

---

## 8. 当前决定

当前阶段固定以下结论：

- 可以启动开发，不需要等待更多抽象设计
- backlog 应按模块与交付包组织，而不是只按大里程碑名组织
- 当前第一验证目标应是 `M4` 的 standalone 替代切片，而不是 `managed` 接入
- `M1-M9` 与 `M10-M15` 仍是主线，但优先级应先满足 `standalone -> 可替代 -> 再 managed`
