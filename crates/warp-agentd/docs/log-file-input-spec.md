# warp-insight 文件日志输入设计

## 1. 文档目的

本文档定义 `warp-agentd` 对日志文件的常驻读取能力。

这里的“文件日志输入”特指：

- `warp-agentd` 作为数据面常驻组件持续监控和读取文本日志文件
- 将文件新增内容转换为统一 telemetry record
- 进入统一 `parse -> normalize -> resource binding -> buffer/spool -> export` 主线

本文档不讨论：

- 远程动作里的 `file.tail` / `file.read_range`
- `syslog` / `journald` / `winlog` 等非文件输入
- `warp-parse` 内部 receiver 的实现细节

相关文档：

- [`../../../doc/design/foundation/architecture.md`](../../../doc/design/foundation/architecture.md)
- [`../../../doc/design/foundation/target.md`](../../../doc/design/foundation/target.md)
- [`../../../doc/design/foundation/non-functional-targets.md`](../../../doc/design/foundation/non-functional-targets.md)
- [`telemetry-uplink-and-warp-parse.md`](../../../doc/design/telemetry/telemetry-uplink-and-warp-parse.md)
- [`./.md`](./.md)
- [`./.md`](./.md)
- [`./.md`](./.md)

---

## 2. 核心结论

第一版固定以下结论：

- 文件日志读取是 `warp-agentd` 的一等数据面能力，不是远程动作替代方案
- 第一版必须明确对标 `Fluent Bit tail input`
- 对标对象是能力边界和工程行为，不是把 `warp-insight` 定义成 Fluent Bit 封装层
- 第一版必须覆盖：
  - 路径 glob 匹配与排除
  - 启动读头/读尾策略
  - 新发现文件的起始位置策略
  - `commit point`、本地 checkpoint 持久化与 crash 恢复
  - 文件 rotate / truncate 处理
  - `file watcher` 与轮询 fallback
  - 长行保护、多行拼装、buffer/backpressure
  - `source.path` / `source.offset` / 来源 metadata 注入
- 第一版不要求完全复刻 Fluent Bit 的全部历史兼容行为

一句话说：

- `warp-agentd` 需要做一个可对标 `Fluent Bit tail` 的文件日志输入器
- 但输出目标不是 Fluent Bit tag/chunk 模型，而是 `warp-insight` 的统一 record / resource / buffer 模型

实施阶段边界同时固定为：

- 本文档描述的是通用 `file input` 的完整基线目标
- `M4` 只实现其受控子集，用于先验证 `standalone` 替代切片
- `M4` 的实现边界收敛为：
  - 显式单路径输入，而不是通用 `path_patterns[]` 发现模型
  - 最小 `parser / multiline / checkpoint / rotate / truncate / restart recovery` 链路
  - 最小 `buffer / spool -> warp-parse / file output` 主线
- `M8` 再把该受控子集扩展为通用文件日志 runtime，包括 discovery、watcher 策略、完整调度、保护模式与自观测

---

## 3. 对标基线

### 3.1 参考对象

当前对标基线采用 Fluent Bit 官方 `Tail` 输入文档：

- `https://docs.fluentbit.io/manual/pipeline/inputs/tail`

按当前官方文档，Fluent Bit `tail` 已覆盖以下关键能力：

- `path` / `exclude_path`
- `read_from_head`
- `read_newly_discovered_files_from_head`
- `db` / `db.sync` / `db.compare_filename`
- `rotate_wait`
- `inotify_watcher`
- `buffer_chunk_size` / `buffer_max_size`
- `mem_buf_limit`
- `skip_long_lines` / `skip_empty_lines`
- `parser`
- `multiline.parser`
- `path_key` / `offset_key`
- `ignore_older`

### 3.2 对标口径

对标时要明确：

- 目标是达到同类成熟日志文件输入器应有的能力下限
- 不是要求配置名、内部状态文件格式、输出结构与 Fluent Bit 完全一致
- `warp-insight` 可以用更适合自身架构的对象模型替代 Fluent Bit 的 plugin/tag/chunk 习惯

### 3.3 我们应优于 Fluent Bit 的地方

第一版设计应明确争取在以下方面优于 Fluent Bit：

- `commit point`、checkpoint 与本地 spool / buffer 的关系更清晰
- resource binding 是一等语义，不依赖后置 filter 拼装
- 保护模式、退化模式和 drop 原因有统一状态机
- `standalone` / `managed` 模式下行为一致
- 自观测指标和审计事件更完整

---

## 4. 边界定义

### 4.1 它负责什么

文件日志输入负责：

- 发现匹配路径的目标文件
- 持续读取新增内容
- 进行按行切分
- 执行 parser / multiline 预处理
- 补充基础源信息和资源引用
- 把结果写入统一 telemetry pipeline
- 管理本地 checkpoint

### 4.2 它不负责什么

文件日志输入不负责：

- 远程文件内容读取
- 控制平面计划下发
- 复杂语义解析与 AI 推理
- 中心侧审计归档
- 取代 `warp-parse` 进行大规模规则解析

### 4.3 与 `file.tail` 的关系

必须明确区分：

- `logs.file_inputs[]`：
  常驻数据面输入
- `file.tail`：
  远程动作 opcode，用于临时诊断读取

两者都可能读取同一路径，但职责完全不同。

---

## 5. 第一版模块拆分

建议把文件日志输入拆成以下子模块：

- `file_target_discovery`
  负责 glob 展开、排除规则、文件初筛、目标变化检测
- `file_watcher`
  负责 `file watcher` 事件监听和轮询 fallback
- `tail_reader`
  负责按 `read offset` 增量读取、按行切分、长行处理
- `multiline_assembler`
  负责多行日志拼装与 flush
- `line_parser`
  负责 `raw/json/ndjson/cri/docker-json` 等轻量预解析
- `checkpoint_store`
  负责本地状态持久化与恢复
- `log_record_builder`
  负责补充源元数据、resource refs、统一 envelope

第一版不建议把这些模块暴露成独立进程。

它们应属于 `warp-agentd` 数据面内部模块，由统一 runtime supervisor 管理。

---

## 6. 运行模型

### 6.1 总体流水线

文件日志输入的推荐流水线应固定为：

```text
discover -> watch -> read delta -> split lines -> multiline
-> parse -> normalize -> attach resource refs
-> enqueue local telemetry buffer/spool -> reach commit point -> advance checkpoint
```

实施阶段约束：

- 对完整通用 `file input` 而言，上述流水线成立
- `M4` 只要求验证其受控子链路：

```text
explicit file target -> read delta -> split lines -> multiline
-> parse -> normalize -> attach resource refs
-> enqueue local telemetry buffer/spool -> reach commit point -> advance checkpoint
```

- `discover`、通用 `watch` 策略与更完整扫描调度属于 `M8` 扩展项

### 6.2 发现模型

第一版至少支持：

- `path_patterns[]`
- `exclude_path_patterns[]`
- 周期性 refresh
- 运行时新增文件发现

第一版建议支持的匹配语义：

- shell-style glob
- 多 pattern 并列
- exclude 在 include 之后生效

### 6.3 `file watcher` 模型

第一版建议支持两种 `file watcher` 模式：

- `native_notify`
  Linux 上优先使用 `inotify`
- `poll`
  基于 `stat` / `readdir` 的轮询 fallback

运行时建议：

- 默认 `auto`
- `auto` 优先选择 `native_notify`
- 原生 `file watcher` 不可用、配额不足或目标目录不适配时自动退回 `poll`

### 6.4 读取模型

每个被跟踪文件应维护独立 reader 状态：

- 当前 `file identity`
- 当前 `read offset`
- 最近读取时间
- 当前行缓冲
- multiline 暂存状态

读取语义固定为：

- 只读取追加内容
- 默认按 `\n` 切分
- 对未完成尾行可短暂缓存，直到补齐或 flush 超时

---

## 7. 配置骨架

第一版建议在 `AgentConfig.logs.file_inputs[]` 下固定如下结构：

```text
LogsSection {
  file_inputs[]?
}
```

```text
FileLogInput {
  id
  enabled
  path_patterns[]
  exclude_path_patterns[]?
  startup_position?
  discovered_file_position?
  ignore_older_ms?
  watcher_mode?
  refresh_interval_ms?
  rotate_wait_ms?
  parser?
  multiline?
  include_path_key?
  include_offset_key?
  include_file_id_key?
  line_buffer?
  checkpoint?
  buffering?
  resource_mapping?
  labels?
}
```

实施阶段约束：

- 配置骨架保留完整通用 `file input` 目标形态，避免后续扩展时再次改写总 schema
- `M4` 实现只要求其中的受控子集：
  - `id`
  - `enabled`
  - 单路径目标字段
  - 最小 `startup_position`
  - 最小 `parser`
  - 最小 `multiline`
  - 最小 `checkpoint`
  - 最小 `buffering`
  - 最小 `resource_mapping`
- `path_patterns[]` / `exclude_path_patterns[]`、`watcher_mode`、`refresh_interval_ms` 等通用运行时字段在 `M8` 补齐完整实现

字段说明：

- `startup_position`
  - `tail`
  - `head`
- `discovered_file_position`
  表示启动完成后新发现文件在没有 checkpoint 时从哪里开始读
  - `tail`
  - `head`
- `watcher_mode`
  - `auto`
  - `native_notify`
  - `poll`

### 7.1 `parser`

```text
FileLogParser {
  mode
  time_key?
  time_format?
  body_key?
}
```

第一版建议：

- `mode`
  - `raw`
  - `json`
  - `ndjson`
  - `cri`
  - `docker_json`

### 7.2 `multiline`

```text
MultilineConfig {
  mode
  flush_timeout_ms?
  firstline_regex?
  continue_regex?
  max_lines?
  max_bytes?
}
```

第一版建议：

- `mode`
  - `off`
  - `docker`
  - `cri`
  - `java_stacktrace`
  - `python_traceback`
  - `go_panic`
  - `custom_regex`

### 7.3 `line_buffer`

```text
LineBufferConfig {
  initial_buffer_bytes?
  max_buffer_bytes?
  skip_long_lines?
  truncate_long_lines?
  skip_empty_lines?
}
```

这里的设计意图直接对标 Fluent Bit 的：

- `buffer_chunk_size`
- `buffer_max_size`
- `skip_long_lines`
- `skip_empty_lines`

**v1 决策（长行保护）**：超长行采用「**截断提交 + 计数**」——

- `max_line_bytes`（默认 1 MiB）：单行超过上限时按上限截断后作为一条记录提交，不阻塞读取、不丢后续行；
- 固定 `truncate_long_lines = true`、`skip_long_lines = false`；
- 每次截断累加计数 `agent_log_lines_truncated_total`，并在状态变化时告警一次；
- 目的：避免“无换行大文件”把内存拖垮或让读取永久不推进。

> 精确边界语义（行边界结算、恰等于上限不截断、EOF 截断提交、分块回放等）见 §7.6。

### 7.4 `checkpoint`

```text
CheckpointConfig {
  enabled
  sync_mode?
  compare_filename?
  flush_interval_ms?
}
```

第一版建议：

- `enabled = true`
- `sync_mode`
  - `full`
  - `normal`
  - `off`
- `compare_filename = true`

这里的语义直接对标 Fluent Bit 的：

- `db`
- `db.sync`
- `db.compare_filename`

### 7.5 `buffering`

```text
FileLogBuffering {
  mem_buf_limit_bytes?
  static_batch_size_bytes?
  event_batch_size_bytes?
}
```

第一版建议保留这几个字段，用于对标 Fluent Bit 的：

- `mem_buf_limit`
- `static_batch_size`
- `event_batch_size`

**v1 决策（spool 超限）**：spool（落盘待发队列，见 [`telemetry/spool`]）必须有上限；
超限时「**暂停采集 + 告警**」——保完整、不丢数据：

```text
FileLogBuffering {
  mem_buf_limit_bytes?
  static_batch_size_bytes?
  event_batch_size_bytes?
  spool_max_bytes?             # 全局 spool 上限
  spool_over_limit = "pause"   # pause（默认）| drop_oldest（显式备选，非默认）
}
```

超限（`pause`）语义：

1. 停止读取新行与 checkpoint 推进（已读未发数据留在 spool，源文件继续增长，恢复后从 checkpoint 续读）；
2. spool 保持不动，继续按 tick 回放；回放到低水位后**自动恢复**采集；
3. 进入/退出该状态各产生一次**工作状态通知**（work-state notification，非告警/非失败）并**上报**；
   状态与原因可被自观测读取（见 §7.6）；
4. 若期间源文件被系统轮转/清理掉，属于系统侧行为，agent 不再保证该部分（见 §12）。

### 7.6 边界语义与实现一致性（v1 实现，已由 5 轮 review 固化）

本节把 §7.3 / §7.5 的意图收敛成**可测试的精确语义**，与
`telemetry/logs/files/{file_reader,file}.rs`、`telemetry/spool.rs` 的实现一致。

**读取预算（`max_read_bytes_per_tick` / `max_lines_per_tick`）**

- 预算与行数只在**行边界**结算：`line_consumed == 0` 时才可能因预算停读；
- **行内不因预算中断**——单行可能超过 `max_read_bytes_per_tick`，但绝不会被切成两半；
  否则「行长 > 预算且跨多个缓冲块」会让每轮从行首重读、`committed_end_offset` 永不前进（已修，见下表）；
- 停读点保证下次从**行首**续读（`committed_end_offset` 落在行边界）。

**截断（`max_line_bytes`）**

- 只提交**完整行**；文件尾部的半行不提交（下次继续）；
- 单行**恰等于** `max_line_bytes`：不截断；**超出**即截断提交（保留前 `max_line_bytes`），
  跳过该行剩余到行尾后原子提交，并计入 `truncated_lines`；
- 截断行的 `end_offset` 指向**真实行尾**（checkpoint 不会卡住）；到 EOF 无换行时也提交截断行；
- `ReadLimits` 对上限做 `max(1)` 夹取，避免 0 预算导致零推进。

**背压 / 暂停（`spool_max_bytes` / `spool_over_limit`）**

- 触发：回放（replay）失败**且** `spool_bytes >= spool_max_bytes`（含恰好相等）；
- **回放优先**：只要 sink 能回放成功，即使 spool 已超限也先回放清空，不进入暂停；
- 暂停/恢复是**工作状态变化（work-state notification）**，不是告警、不是失败；
  命名不能用 “告警”。进入与退出（恢复）是同一类通知的两个事件：`paused` / `resumed`；
- 暂停期间：不读源、不推进 checkpoint、`spool` 保持不变；回放至清空后**自动恢复**；
- **应上报**：进入/退出各产生一次工作状态通知并上报（不能只落本地日志）；
  接收方与通道见 [`development-plan.md`](./development-plan.md) §W1「待办（follow-up）」。
- 当前 v1 实现：仅经 `ProcessOutcomeKind::SpoolPaused` 暴露，作为 `SpoolPaused` 事件走
  既有失败缓存（`filter_new_failures`）去重后 `eprintln`——**仅本地输出、尚未上报，且仅覆盖“进入”**；
  这属于待补，不是最终形态。
- `drop_oldest` 可配置，但 **v1 仅实现 `pause` 语义**（保完整优先），未实现按优先级丢弃。

**配置校验**

- `spool_over_limit ∈ {pause, drop_oldest}`，否则 `invalid_logs_spool_over_limit`；
- `max_line_bytes` / `max_read_bytes_per_tick` / `max_lines_per_tick` / `spool_max_bytes`
  必须 > 0，否则分别返回 `invalid_logs_max_line_bytes` / `invalid_logs_max_read_bytes_per_tick` /
  `invalid_logs_max_lines_per_tick` / `invalid_logs_spool_max_bytes`（避免 0 值造成永久暂停或零预算）。

**review 固化的缺陷与修复（与用例对应）**

| # | 问题 | 类型 | 修复 | 测试 |
|---|---|---|---|---|
| 1 | 行长 > 预算且跨缓冲块时每轮从行首重读、永不推进 | 缺陷 | 预算只在行边界结算 | `long_line_larger_than_read_budget_still_completes`、`line_over_read_budget_is_delivered_without_loss` |
| 2 | （无新缺陷）分块/预算在 Processor 层行为 | 验证 | — | `chunked_read_by_max_lines_advances_checkpoint_each_tick_without_loss`、`resumes_from_line_start_after_byte_budget_stop` |
| 3 | （无新缺陷）截断边界语义 | 验证 | — | `line_exactly_at_max_line_bytes_is_not_truncated`、`line_one_byte_over_max_line_bytes_is_truncated_and_counted`、`truncated_line_without_trailing_newline_is_committed_at_eof`、`multiple_truncated_lines_are_each_counted`、`truncated_long_line_with_small_budget_still_completes`、`read_limits_clamp_zero_to_one_and_still_make_progress` |
| 4 | （无新缺陷）背压边界（相等即暂停、健康即回放） | 验证 | — | `spool_exactly_at_limit_pauses`、`spool_over_limit_with_healthy_sink_replays_without_pausing` |
| 5 | 上限为 `0` 被接受 → 永久暂停 / 零预算 | 健壮性 | 上限非零校验 | `config_with_zero_{spool_max_bytes,max_line_bytes,max_read_bytes_per_tick,max_lines_per_tick}_is_rejected`、`config_with_drop_oldest_spool_over_limit_is_accepted` |

> 表中「工作状态通知的上报」「`drop_oldest`」两项已登记为待办，见
> [`development-plan.md`](./development-plan.md) §W1「待办（follow-up）」。

---

## 8. 本地状态与 checkpoint

字段级 schema 独立定义在：

- [`./.md`](./.md)

本节只保留与运行语义直接相关的结论。

### 8.1 状态文件位置

第一版建议每个文件日志输入在本地维护：

- `state/logs/file_inputs/<id>/checkpoints.json`

### 8.2 `commit point` 与 checkpoint 推进规则

checkpoint 不能在“刚读到文件内容”时立即推进。

建议固定为：

- 读取内容并形成 record
- record 已成功进入本地 telemetry buffer
- 若启用了 spool，则以 durable spool 接纳成功为 `commit point`
- 若未启用 spool，则以 input 认可的本地 buffer 安全接纳点作为 `commit point`
- 之后再推进 checkpoint

**序号（`seq`）持久化**：每个 input 维护单调递增的 `next_seq`，**与 checkpoint 同一次原子写提交**（见 §11.1.1）；
重启后从 state 续号，同 input 内不回退。

这样可以保证：

- 正常运行与优雅退出时尽量不丢数据
- 异常崩溃时提供 at-least-once
- 允许小范围重复，不允许静默跳过

### 8.3 crash 恢复语义

第一版建议固定：

- 恢复时优先使用 checkpoint 中的最近已提交 `checkpoint offset`
- 如果 crash 发生在“已读取但未提交 checkpoint”窗口，允许重复少量记录
- 不允许因为 crash 把未持久化确认的数据视为已成功消费

---

## 9. rotate / truncate 语义

### 9.1 rotate

第一版必须支持最常见的 rename-rotate 场景：

1. 原路径文件被 rename 到新路径
2. 新文件在原路径重新创建
3. reader 继续读取旧文件剩余尾部
4. 同时开始跟踪新文件

建议提供：

- `rotate_wait_ms`

其语义与 Fluent Bit `rotate_wait` 对齐：

- 文件被 rotate 后，reader 继续保留一段时间，吸收尾部残留写入

### 9.2 truncate

第一版必须识别 truncate / copytruncate 这类场景。

建议规则：

- 同一 `file_id` 下，如果当前文件大小小于已提交 `checkpoint offset`
- 视为发生 truncate
- 记录一次 truncate 事件
- 将 `read offset` 重置到 `0`

### 9.3 inode 复用

inode 复用是 `file reader` / `tail reader` 的高风险边界。

第一版建议：

- 默认开启 `compare_filename`
- 必要时结合 `fingerprint`
- 当身份判断不可靠时，宁可保守重读少量内容，也不要静默跳过

---

## 10. 多行日志

第一版必须把 multiline 作为一等能力，而不是后期补丁。

原因很直接：

- Java / Python / Go stacktrace 很常见
- Docker / CRI 容器日志天然存在拆分与重组需求
- 没有 multiline，文件日志输入很难达到 Fluent Bit 同等级可用性

### 10.1 第一版最小模式

建议第一版至少支持：

- `docker`
- `cri`
- `java_stacktrace`
- `python_traceback`
- `go_panic`
- `custom_regex`

### 10.2 flush 规则

multiline 组装必须受以下限制：

- `flush_timeout_ms`
- `max_lines`
- `max_bytes`

任何一个限制触发时都必须：

- 立即结束当前组装
- 产生日志或指标
- 保留 `truncated` / `multiline_flush_reason` 等诊断字段

### 10.3 与 parser 的顺序

第一版建议固定顺序：

- 先按输入模式做必要的拆分或重组
- 再执行结构化 parser
- 最后进入 normalize / resource binding

不要让 parser 和 multiline 形成循环依赖。

---

## 11. 统一 record 与 resource 绑定

### 11.1 最小源字段

第一版建议每条日志 record 至少附带：

- `source_type = "file"`
- `source.path`
- `source.offset`
- `source.input_id`
- `observed_at`
- `body`

当启用对应开关时，还可补充：

- `source.file_id`
- `source.device_id`
- `source.inode`
- **`seq`（序号，v1 必须）**：per-input 单调递增 `u64`，用于下游去重与缺口检测

### 11.1.1 `seq` 与去重规则（v1 已决：方案 B）

**为什么不用 offset 单键**：truncate 后 offset 会复用、文件被替换但路径不变，旧键会把新数据误判为重复。

**`seq` 定义**：

- 粒度：`per input_id`；形态：`u64` 单调递增；
- 分配：记录生成时取号；**`next_seq` 与 checkpoint 同文件、同一次原子写**（checkpoint 推进时一并提交）；
- 重启：从 state 续号；同 input 内**只要求不回退**（不要求连续）。

**上送帧**：在信封中新增 `seq`（与 `input_id`/`source_path`/`file_offset`/`file_offset_end` 并列），原文仍在 `RAW:` 之后。

**下游去重（数据面规则，按优先级）**：

1. 主键 `(agent_id, input_id, seq)` → 命中即丢弃；
2. 辅助判据 `(agent_id, input_id, file_id, offset_start, offset_end)` → `seq` 不同但位置完全相同判为重复；
   > 原因：崩溃窗口内“已 spool、未提交 `next_seq`”的记录重读时会重新取号：同一行会出现 **`seq` 不同、位置相同**的重复。
3. **世代隔离**：`file_id`（`dev:ino`，不可得时用 fingerprint）变化（truncate / 轮转 / 文件替换）后，
   位置判据只在同一 `file_id` 内有效；跨世代一律以 `seq` 为准，避免 offset 复用导致的误丢弃。

**缺口检测**：同 `(input_id, file_id)` 内 `seq` 不连续即为可疑丢行，可上报告警（与 `agent_log_records_dropped_total` 关联）。

### 11.2 resource binding

文件日志输入不能只把路径当字符串吐出去。

第一版应尽量在边缘建立：

- `host` 资源绑定
- 容器日志路径到 `container` / `k8s_pod` 的绑定
- 常见服务日志路径到 `service` 的绑定

必要时可结合：

- discovery cache
- 文件名 regex 提取
- 目录约定
- 运行时元数据

### 11.3 与 Fluent Bit 的差异

这里不要求照搬 Fluent Bit 的 `tag` / `tag_regex`。

`warp-insight` 更适合的做法是：

- 用显式 `resource_refs`
- 用结构化 `source.*`
- 把 filename 提取得到的字段放到 labels / attrs

---

## 12. 资源预算、backpressure 与保护模式

文件日志输入属于“不可反馈输入”。

这意味着：

- 无法像 HTTP / OTLP push 一样把背压直接传回上游
- 只能靠本地 queue、spool、限额和保护模式来吸收

### 12.1 第一版必须具备的保护手段

- 每文件独立读取 buffer 上限
- 每 input 级 `mem_buf_limit_bytes`
- 全局 telemetry queue / spool 上限（超限**暂停采集 + 告警**，不丢数据）
- 长行**截断提交 + 计数**（见 §7.3 `max_line_bytes`）
- 当进入 `degraded` / `protect` 时降低扫描和读取强度

### 12.2 退化顺序

建议退化顺序：

1. 降低目录 refresh 频率
2. 暂停低优先级 input 的新文件发现
3. 减少单轮静态文件批处理量
4. 限制 multiline 暂存
5. 达到 spool 硬上限时**暂停该 input 采集并告警**（保完整，不丢数据）；
   仅当显式配置 `spool_over_limit = "drop_oldest"` 时才按 input 优先级丢弃，并记录原因

### 12.3 与 Fluent Bit 对标

这里至少要覆盖与 Fluent Bit 类似的两个层面：

- 读文件缓冲保护
- 输出拥塞时的内存保护

但 `warp-insight` 还应补充：

- 统一保护模式状态
- drop reason
- 控制面可见性

---

## 13. 自观测

第一版建议至少暴露以下指标：

- `agent_log_files_discovered`
- `agent_log_files_watched`
- `agent_log_lines_read_total`
- `agent_log_records_emitted_total`
- `agent_log_records_dropped_total`
- `agent_log_bytes_read_total`
- `agent_log_multiline_flush_total`
- `agent_log_checkpoint_commits_total`
- `agent_log_checkpoint_lag_bytes`
- `agent_log_rotate_events_total`
- `agent_log_truncate_events_total`
- `agent_log_reader_paused`

同时建议输出关键事件：

- `FileLogTargetDiscovered`
- `FileLogTargetDropped`
- `FileLogCheckpointRecovered`
- `FileLogRotated`
- `FileLogTruncated`
- `FileLogLongLineSkipped`
- `FileLogInputPaused`

---

## 14. 验收标准

### 14.1 完整 `file input` 基线验收

完整通用 `file input` 建议至少满足以下验收：

1. 能稳定读取单文件和 glob 多文件输入。
2. 能在 restart 后基于 checkpoint 恢复，并满足 at-least-once。
3. 能正确处理 rename-rotate 和 truncate。
4. 能提供 `native_notify` 与 `poll` 两种 watcher 行为。
5. 能处理 `docker` / `cri` / `java_stacktrace` 三类常见 multiline。
6. 能在 backpressure 下维持资源硬边界，不因日志洪峰拖垮宿主机。
7. 能把 `source.path`、`source.offset`、`resource_refs` 稳定挂入统一 record。
8. 能用自观测指标证明与 Fluent Bit `tail` 同等级的关键能力已具备。

### 14.2 `M4` 受控子集验收

`M4` 只按受控子集验收，不以完整通用 `file input` 为交付门：

1. 在 `control_plane.enabled = false` 时，能稳定读取一个显式配置的文件路径。
2. 能在 restart 后基于已提交 checkpoint 恢复，并满足 at-least-once。
3. 能正确处理 append、rename-rotate、truncate 与最小 multiline 基线。
4. 能把 `source.path`、`source.offset`、`resource_refs` 稳定挂入统一 record。
5. 能通过 `warp-parse` 或本地 fallback 输出，验证至少一类 `standalone` 替代链路。
6. 不要求在 `M4` 提供通用 glob 发现、完整 watcher 策略、完整保护模式与完整自观测。

---

## 15. 当前决定

当前阶段固定以下结论：

- 文件日志输入必须进入 `warp-agentd` 第一版 logs 设计范围
- 其目标是对标 Fluent Bit `tail`，不是依赖 Fluent Bit
- 配置、checkpoint、rotate、multiline、budget 必须一起设计，不能拆成零散补丁
- `file.tail` 不能替代常驻文件日志采集
- `M4` 先落受控单路径替代切片，`M8` 再扩展为通用 `file input` runtime
- **长行策略**固定为“截断提交 + 计数”（`max_line_bytes`，默认 1 MiB，见 §7.3）
- **spool 有上限，超限策略**固定为“暂停采集 + 告警”（保完整，见 §7.5/§12）；`drop_oldest` 仅为显式备选
- **交付语义**为 at-least-once（可能重复、不丢）：spool 接纳成功即推进 checkpoint；去重采用**方案 B**：
  per-input `seq`（`next_seq` 与 checkpoint 同次原子写）+ 下游组合键去重（见 §11.1.1）
- **源日志默认不清理**（只读采集）：轮转/清理交给系统或中心策略；agent 自身的 spool 与本地输出必须有界并轮转
