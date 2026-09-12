# telemetry 上报协议：帧格式定义

## 1. 目的与范围

本文档定义 `warp-insight` 数据面 `agentd → gateway → center 数据平台` 链路上，**接收端接入层（ingress）** 需要消费的帧格式。它是 `wist-delivery`（去重 / 缺口检测 / 过滤）的上游协议约定。

覆盖两类载体：

| 载体 | 通道 | 承载什么 |
| --- | --- | --- |
| **数据帧**（data frame） | 数据平面（TCP） | 一条原始记录（带 `seq`）；被动丢失靠接收端 `seq` 跳号检测，帧本身不表达丢失 |
| **数据报告**（data report） | 独立报告通道（控制/状态） | 主动过滤的丢弃区间 + `reason` |

只定义格式，不绑定具体 IO / 存储 / 协议实现。与「防丢失」的语义关系见 [`data-loss-prevention.md`](data-loss-prevention.md)。

---

## 2. 命名约定

数据面高频、量大，帧字段名取**单字短名**；Rust 契约结构体仍用可读的 snake_case 长名，二者通过 `#[serde(rename = "短名")]` 映射，代码可读、线上省字节。

| Rust 字段（长名） | 线上短名 |
| --- | --- |
| `schema_version` | `schema` |
| `agent_id` | `agent` |
| `observed_at` | `ts` |
| `seq` | `seq` |
| `range_start` | `start` |
| `range_end` | `end` |
| `drop_reason` | `reason` |

---

## 3. 传输与分帧

数据平面（`agentd → gateway`、`gateway → center`）在 TCP 字节流上逐帧发送，两种分帧模式（见 `warp-agentd` 的 `TcpFraming`）：

| 模式 | 编码 | 适用 |
| --- | --- | --- |
| `line` | 每帧以 `\n` 结尾 | body 不含换行的日志行 |
| `len` | `{字节长度} {帧内容}`，长度与内容间一个空格 | body 可能含换行（多行/富格式） |

> body 可能含换行时必须用 `len`；`line` 仅适用于单行、无换行的记录。

---

## 4. 数据平面帧格式

数据平面（TCP）**只承载数据帧**：`{envelope} RAW: <正文>`。接收端解析帧开头的 JSON 信封（`{...}`），信封之后是 ` RAW: <正文>`。

`RAW: ` 是帧标记，后跟一个空格；正文（原始行）不进入 JSON、不转义，便于审计核对与回放。

> 主动过滤不混进数据平面，走独立报告通道的数据报告（§6）；因此数据平面无需判别帧类型，被动丢失由接收端从 `seq` 跳号检测（见 `data-loss-prevention.md` §8）。

---

## 5. 数据帧（data frame）

```
{envelope} RAW: <正文>
```

信封字段（信号无关，通用）：

| 字段 | 类型 | 必填 | 语义 | 用途 |
| --- | --- | --- | --- | --- |
| `schema` | string | 是 | 契约版本（`v1`） | 版本演进 |
| `agent` | string | **是（新增）** | 生产者全局唯一 ID | 复合键 `(agent, seq)` |
| `ts` | string | 是 | 采集时间（RFC3339） | 时间语义 |
| `seq` | u64 | 是 | per-`agent` 单调递增序号 | seq 去重 + 缺口检测 |

> **不携带来源信息**：帧与信号来源无关——不管记录来自文件、syslog、metric 还是 trace，都只靠 `seq`（per-`agent` 全局）做去重与缺口检测。`input`/文件路径/偏移等来源细节不进帧；「丢在哪条源」靠 spool 的 `seq → 内容` 映射在恢复时归源（见 `data-loss-prevention.md` §5.2.1）。
>
> **数据帧只表达真实数据**：帧本身不带「丢失/过滤」标记；被动丢失由接收端检测 `seq` 跳号（洞），主动过滤走数据报告（§6），二者通道分离。

**示例**：

```
{"schema":"v1","agent":"agent-001","ts":"2026-04-14T00:00:00Z","seq":0} RAW: 2026-04-14 INFO request completed
```

---

## 6. 主动过滤：数据报告（独立通道）

在 `seq` 取号**之后**过滤某条记录时，**不往数据平面插 in-band 标记**，两条通道分离：

1. **数据帧**：被过滤的记录不再转发 → 下游 `seq` 出现「洞」；
2. **数据报告**：走独立报告通道（控制/状态）上报丢弃区间 `{start, end, reason}`。

接收端对账时从「洞」中扣除报告区间，剩余才是真丢失（`洞 = 主动过滤 + 被动丢失`）。

### 6.1 丢弃区间（dropped range）

把连续被过滤的 `seq` 压缩成一条区间，随 `StreamLossReport.dropped_ranges` 走独立报告通道（控制/状态）：

```json
{ "agent": "agent-001", "start": 2, "end": 2, "reason": "filter:x" }
```

| 字段 | 类型 | 必填 | 语义 |
| --- | --- | --- | --- |
| `agent` | string | 是 | 生产者 ID（定位流） |
| `start` | u64 | 是 | 区间起始 `seq`（含） |
| `end` | u64 | 是 | 区间结束 `seq`（含） |
| `reason` | string | 是 | 丢弃原因（`filter:x` / `sample` / `debug`） |

接收端 `QualityChannel::on_dropped_range(start, end, reason)` 处理：批量推进水位线跳过区间（不计丢失）、计入 `filtered_count`、保留区间明细供 center 对账按 reason 拆分。

> 高量级（如 `debug`）与低量级（单条规则）过滤**统一走数据报告**，不再区分 in-band / out-of-band——数据平面只承载真实数据。

---

## 7. 字段语义：端到端不变与去重键

| 概念 | 字段 | 说明 |
| --- | --- | --- |
| **端到端不变** | `seq` | agentd 取号后跨 hop 保留原值，gateway 转发不重新取号 |
| **去重键** | `(agent, seq)` | 命中即重复，对所有信号通用 |

> 去重只靠 `seq`（`(agent, seq)` 主键，`seq` per-`agent` 全局），不引入位置/世代辅助判据。理由：`next_seq` 与 checkpoint 同次原子写，崩溃窗口内重读会得到**相同** `seq`，`seq` 去重已能覆盖崩溃窗口重复，位置辅助去重冗余（见 `data-loss-prevention.md` §7）。

---

## 8. 与 `wist-delivery` 的映射

接收端把帧还原后喂给 [`QualityChannel`](../../crates/wist-delivery/README.md)：

| 载体 | 映射 |
| --- | --- |
| 数据帧 | `channel.on_record(seq)` |
| 数据报告（丢弃区间） | `channel.on_dropped_range(start, end, reason)` |

`QualityChannel` 按 `agent` 分组维护。

---

## 9. 兼容性与演进

1. **新增字段向后兼容**：`agent` 及报告字段在反序列化侧用 `#[serde(default)]`，旧数据缺字段可退化解析。
2. **缺失字段的降级**：无 `agent` 的旧记录无法参与复合键去重；`seq` 缺口检测不受影响。
3. **`schema`** 当前 `TelemetryRecordContract` 有 `schema_version` 字段，但 TCP 帧信封未携带——应在实现帧格式时补齐到信封。

---

## 10. 实现缺口（待办）

1. 数据报告（丢弃区间）尚无序列化/解析路径与独立通道对接（当前只有 `build_record_frame` 的普通数据帧）。

已对齐：

- `wist-delivery`：in-band 墓碑（`DropMarker` / `on_tombstone`）已移除，主动过滤统一走 `on_dropped_range`；位置辅助去重（`Deduper` / `RecordPosition`）已移除，去重收敛为 `(agent, seq)`。
- `TelemetryRecordContract`：新增 `agent_id`（`#[serde(default)]` 向后兼容），移除 `signal_kind`；`input_id`/来源字段保留在契约内部（spool/路由用）、不进帧。
- `build_record_frame`：信封收敛为 `{schema, agent, ts, seq}` 短名，不再携带 `signal_kind`/`input`/来源字段；信封已落成 `DataFrame` 结构体（`#[serde(rename)]` 映射短名，见 `wist-contracts::telemetry_record::DataFrame`），不再手写 `json!`。

下游 warp-parse 的 WPL/OML 模型（`sysrun/warp-gateway/data-plane/models/`）仍按旧帧字段（`signal_kind`/`input_id`/`source_path`/`file_offset`）解析，需随帧格式一并改造（另立任务）。
