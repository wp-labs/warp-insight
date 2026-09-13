//! 指标声明（spec）已下沉到 `wist-metrics` 共享 crate，这里仅作兼容性再导出。
//!
//! 系统指标（host/process/container）与后续非系统指标共用同一张 spec 表，避免「系统/
//! 非系统」两套命名契约。

pub use wist_metrics::spec::find_metric_spec;
