//! 接收端「交付完整性」纯逻辑：丢检查与报告。
//!
//! 供数据面接收端（`warp-gateway` / `wist-gateway`、`wist-center` 数据平台）共用，
//! 保证去重、缺口检测、过滤与对账的口径一致。只做纯状态机与可序列化类型，
//! 不绑定具体 IO / 存储 / 协议。
//!
//! # 模块
//!
//! - [`channel`]：接入层单一入口 `QualityChannel`，把去重/查缺/过滤串成一次调用。
//! - [`watermark`]：有界窗口 + watermark 的乱序容忍缺口检测（`seq` 丢失判定）。
//! - [`report`]：丢失/过滤/接收计数的可序列化报告类型（含带外丢弃区间）。
//! - [`reconcile`]：多跳差值定位与端到端交付账本。
//!
//! # 设计约定
//!
//! - **去重主键 `(agent, seq)`**：`seq` 为 per-`agent` 全局单调序号，`seq` 去重由
//!   [`watermark::WatermarkTracker::observe`] 的 `Duplicate` 承担，不引入位置辅助判据。
//! - **主动过滤走带外丢弃区间**：数据平面只承载真实数据；主动过滤（`seq` 取号后）不插
//!   in-band 标记，由 [`report::DroppedRange`] 随报告通道上报，接收端从缺口中扣除。
//!
//! 本 crate 不依赖 `wist-contracts`：接收端把一条记录映射成一个 `seq` 后喂入，保持解耦、
//! 可独立单测。

pub mod channel;
pub mod reconcile;
pub mod report;
pub mod watermark;

pub use channel::{Acceptance, QualityChannel};
pub use reconcile::{DeliveryLedger, HopLosses};
pub use report::{DroppedRange, StreamLossReport};
pub use watermark::{ObserveOutcome, ObserveResult, WatermarkTracker};
