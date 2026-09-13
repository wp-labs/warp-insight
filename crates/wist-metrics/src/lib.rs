//! Metrics collection contract shared by system collectors (in `warp-agentd`) and
//! non-system collectors (database / external services, kept here to isolate heavy drivers).
//!
//! This crate owns the W4「契约层 + 采集层抽象」：
//! - [`provider`]：`MetricProvider` trait 与采集结果类型（collect → samples 的中转结构）；
//! - [`target`]：采集目标视图条目（provider 的输入）；
//! - [`spec`]：指标声明表（fact_key → 规范化指标名/单位/类型）。
//!
//! 系统指标（host/process/container）的 provider 仍在 `warp-agentd` 里实现；非系统指标
//! （Postgres / MySQL / …）的 provider 放本 crate，其重依赖（如 `sqlx`）以 feature 门控，
//! 不进入 `warp-agentd` 的默认依赖图。

pub mod provider;
pub mod spec;
pub mod target;
