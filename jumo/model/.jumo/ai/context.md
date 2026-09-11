# Jumo AI Context

This file is context for `.jumo/ai/ai-task.md`. Follow the selected AI task, not a generic fix task.

## Project
/Users/zuowenjian/devspace/rust/x-topology/warp-insight/jumo/model

## Active View
Code Quality

## Active Domain


## Selected Element
file `crates/warp-agentd/src/telemetry/logs/files/delivery_support.rs`

## Model Summary
代码质量报告为采集快照，重构前须核对当前源码；下方度量为派发任务时的值，任务完成后须重算并比较。

## Quality Target

目标：file `crates/warp-agentd/src/telemetry/logs/files/delivery_support.rs`

### 基线度量（派发任务时采集）
- 代码行 56 行（拆分敏感）
- 文件复杂度密度 214.3/KLOC（5/10 档）（拆分敏感）
- 最大圈复杂度 9
- 最长函数 41 行
- 超圈复杂度函数 0 个
- 超长函数 0 个
- 行覆盖率 —
- 目标告警数 0 条

### 主要问题函数
- `crates/warp-agentd/src/telemetry/logs/files/delivery_support.rs:12` `deliver_records` 圈复杂度 9 · 长度 41 行
- `crates/warp-agentd/src/telemetry/logs/files/delivery_support.rs:54` `replay_spool_if_present` 圈复杂度 3 · 长度 11 行

### 目标相关告警
- 无

### 报告与复算
- 报告：`/Users/zuowenjian/devspace/rust/x-topology/warp-insight/jumo/model/impl/code-quality.json`
- 复算命令：`jumo-code code-quality /Users/zuowenjian/devspace/rust/x-topology/warp-insight --out /Users/zuowenjian/devspace/rust/x-topology/warp-insight/jumo/model/impl/code-quality.json`

## Diagnostics
- none

## Related Files
- /Users/zuowenjian/devspace/rust/x-topology/warp-insight/jumo/model/impl/code-quality.json

## Source Snippets
No source snippets were available.

## Working Rules
- Use `.jumo/ai/ai-task.md` as the task source.
- Keep changes focused on relevant `.mju`, `layout.json`, or necessary documentation files.
- Do not introduce duplicate definitions.
- Run the relevant `jumo verify .` / `jumo readiness .`, or the project's existing validation command.
