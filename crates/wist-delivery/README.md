# wist-delivery

接收端「交付完整性」纯逻辑 crate：**丢检查 + 去重 + 过滤 + 对账**。

供数据面接收端（`warp-gateway` / `wist-gateway`、`wist-center` 数据平台）共用，保证去重、缺口检测、过滤与对账的口径一致。**只做纯状态机与可序列化类型，不绑定 IO / 存储 / 协议**。

设计背景见 [`doc/design/telemetry/data-loss-prevention.md`](../../doc/design/telemetry/data-loss-prevention.md)。

## 定位与边界

| 归谁 | 内容 |
|---|---|
| ✅ 本 crate（接收侧） | watermark 缺口检测、`(agent, seq)` 去重、带外丢弃区间（过滤）、丢失报告、差值对账 |
| ❌ 留在 agentd（生产侧） | checkpoint、spool、`seq` 取号（`next_seq` 与 checkpoint 原子写） |

本 crate **不依赖 `wist-contracts`**：接收端把一条记录映射成一个 `seq` 后喂入，保持解耦、可独立单测。

> **集成位置**：接收端的**接入层（ingress）**，在 ETL 之前——TCP 收到字节流 → 还原带 `seq` 的记录 → 本 crate 去重/查缺/处理丢弃区间 → 把干净记录流交给 `warp-parse` 的 ETL（parse/transform/route）。`warp-parse` 本身不集成本 crate。

## 依赖

- `serde`（可序列化类型：`StreamLossReport` / `DroppedRange`）。

## 快速开始

```toml
[dependencies]
wist-delivery = { path = "crates/wist-delivery" }
```

## 模块速览

| 模块 | 结构 | 职责 |
|---|---|---|
| [`channel`](src/channel.rs) | `QualityChannel` / `Acceptance` | 接入层单一入口：把去重/查缺/过滤串成一次调用 |
| [`watermark`](src/watermark.rs) | `WatermarkTracker` | 有界窗口 + watermark 乱序容忍缺口检测（含带外丢弃区间） |
| [`report`](src/report.rs) | `StreamLossReport` / `DroppedRange` | 丢失/过滤/接收计数 + 带外丢弃区间（可序列化） |
| [`reconcile`](src/reconcile.rs) | `HopLosses` / `DeliveryLedger` | 多跳差值定位 + 交付账本 |

---

## 1. 缺口检测（`WatermarkTracker`）

单个 `agent_id` 流的 `seq` 单调递增，但多连接 / 多 gateway / 异步重放下**不保证按序到达**。`WatermarkTracker` 用有界窗口（`lag`）容忍乱序：只有某 `seq` 滑出窗口下界仍未到达，才判定丢失。

```rust
use wist_delivery::{ObserveOutcome, WatermarkTracker};

// lag = 4：允许 4 个 seq 的乱序容忍窗口。
let mut tracker = WatermarkTracker::new(4);

for seq in [0, 2, 1, 3, 4] {
    let result = tracker.observe(seq);
    match result.outcome {
        ObserveOutcome::New => println!("accept seq {seq}"),
        ObserveOutcome::Duplicate => println!("drop duplicate seq {seq}"),
    }
    for lost in result.lost {
        println!("LOST seq {lost}");
    }
}
```

要点：

- **「缺 seq」不即时判丢**：乱序到达的 seq 进入窗口缓冲，等它滑出窗口下界仍缺失才判丢（延迟确认）。
- **`observe` 返回 `Duplicate`** 即承担了 seq 去重——去重主键 `(agent, seq)`，`seq` per-`agent` 全局，不引入位置辅助判据。
- **中途接入**已有流用 `with_committed(lag, committed)`：把已处理/已过滤的历史 `seq` 视为已结算前缀，避免误判丢失。

```rust
// 已结算前缀 10：seq 0..10 视为已处理，从 10 续接。
let mut tracker = WatermarkTracker::with_committed(4, 10);
```

---

## 2. 主动过滤（`DroppedRange`，带外丢弃区间）

主动过滤（`seq` 取号后，如 `debug` 内容级过滤）**不插 in-band 标记**，而是把连续被过滤的 `seq` 压缩成一条区间，走独立报告通道（控制/状态）上报。数据平面只承载真实数据；接收端对账时从「洞」中扣除丢弃区间，剩余才是真丢失（`洞 − 丢弃区间 = 真丢失`）。

```rust
use wist_delivery::DroppedRange;

// 丢弃 seq [100, 199]（100 条），原因 debug。
let range = DroppedRange::new(100, 199, "debug");
assert_eq!(range.count(), 100);

// 序列化为帧短名 { "start": 100, "end": 199, "reason": "debug" }
let json = serde_json::to_string(&range).unwrap();
```

接收端调用 `QualityChannel::on_dropped_range(100, 199, "debug")`：批量推进水位线跳过该区间（不计丢失）、计入 `filtered_count`、并保留区间明细供 center 对账按 reason 拆分。

> **通道分离是原则**：数据帧只能表达被动丢失（洞）；主动过滤一律走带外丢弃区间，二者在通道上不混。

---

## 3. 丢失报告（`StreamLossReport`）

接收端把计数快照序列化后随状态/控制面通道上报。`dropped_ranges` 字段携带带外丢弃区间明细（`#[serde(default)]`，旧版无此字段仍可反序列化）。

```rust
use wist_delivery::StreamLossReport;

let report = StreamLossReport::new("agent-001");
// ... 填充 lost_count / filtered_count / accepted_count / dropped_ranges ...
let json = serde_json::to_string(&report).unwrap();
// {"agent":"agent-001","lost_count":0,"filtered_count":0,"accepted_count":0,"dropped_ranges":[]}
```

---

## 4. 对账（`reconcile`）

利用端到端不变的 `seq`，用差值定位丢失发生在哪一跳。

```rust
use wist_delivery::HopLosses;

// gateway 第一段丢失 G=2，center 累计丢失 C=5。
let losses = HopLosses { first_segment: 2, cumulative: 5 };
assert_eq!(losses.second_segment(), 3); // gateway → center 丢失
assert_eq!(losses.total(), 5);          // 端到端总丢失
```

端到端交付账本恒等式：

```rust
use wist_delivery::DeliveryLedger;

let ledger = DeliveryLedger {
    produced: 100,       // seq 分配总数
    stored: 90,          // 成功落库
    filtered: 5,         // 主动过滤（丢弃区间）
    source_dropped: 3,   // 源头显式丢弃
    silent_loss: 2,      // watermark 判定丢失
};
assert!(ledger.balanced()); // produced == stored + filtered + source_dropped + silent_loss
```

---

## 端到端集成模式（`QualityChannel`）

接收端（gateway / center 数据平台）每个 `agent_id` 维护一个 [`QualityChannel`](src/channel.rs)，把去重/查缺/过滤串成单一入口：

```rust
use wist_delivery::{Acceptance, QualityChannel};

let mut channel = QualityChannel::new("agent-001", /* lag = */ 4);

// 普通记录：喂入 seq，返回是否交给 ETL。
match channel.on_record(0) {
    Acceptance::Accepted => { /* 交给 ETL 落库/转发 */ }
    Acceptance::Rejected => { /* 重复或已过滤，丢弃 */ }
}

// 带外丢弃区间（主动过滤，如 debug）：批量跳过、不计丢失、计入 filtered。
channel.on_dropped_range(2, 100, "debug");

// 上报：计数快照 + 带外丢弃区间明细。
let report = channel.report();
let json = serde_json::to_string(&report).unwrap();
```

处理语义（与设计一致）：

- **`on_record`**：`seq` 去重 → 缺口判定（丢失计入 `lost_count`）→ 接受计数。
- **`on_dropped_range`**：批量推进水位线跳过区间、计入 `filtered_count`、保留区间明细，**不**计丢失。

## 语义与注意事项

1. **按 agent 分组**：`QualityChannel` / `WatermarkTracker` 都是「单 agent」状态，调用方必须按 `agent_id` 分组维护。
2. **`lag` 取值**：与最大重试延迟、连接数、gateway 数挂钩。`lag` 太小会把「迟到」误判成「丢失」，太大则丢失发现延迟、内存占用上升。
3. **丢弃区间必须被水位线观测**：`on_dropped_range` 推进 `committed`，否则下游会把被过滤的 `seq` 误判成丢失。`QualityChannel` 已在内部保证。
4. **内存边界（后续）**：当前 `observed` 不主动裁剪，长流下需要配合窗口/水位线下沉（`with_committed` 的 `committed` 即水位线）；`dropped_ranges` 随结算推进逐段消费。
5. **不保证全局顺序**：本 crate 只负责「乱序容忍 + 延迟确认丢失」，不负责排序；排序语义由调用方（单连接、spool 重放等）保证。

## 关联文档

- 设计：`doc/design/telemetry/data-loss-prevention.md`
- 帧格式：`doc/design/telemetry/telemetry-uplink-protocol.md`
- 文件日志输入 spec（`seq`/去重 §11.1.1、背压 §12）：`crates/wist-agentd/docs/log-file-input-spec.md`
