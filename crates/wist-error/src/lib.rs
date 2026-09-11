//! Shared structured error vocabulary for the warp-insight server side.
//!
//! Centralizes the cross-crate reason enums (built on `orion-error`) and the
//! conversions between them, so `wist-center` / `warp-gateway` / `wist-gateway`
//! / `insight-control` / `wist-security` / `wist-reporting` share one error
//! language instead of each hand-rolling a `StoreError` / `ConfigError` /
//! `Box<dyn Error>`.
//!
//! Reasons are unit-shaped plus a transparent `General(UnifiedReason)`; dynamic
//! diagnostics live on [`StructError`] as detail / context / source. Lifting a
//! lower reason into [`AppReason`] uses [`orion_error::conversion::ConvErr`] via
//! the `From` impls in [`convert`].

pub mod codes;
pub mod convert;
pub mod reason;

pub use codes::SysErrorCode;
pub use reason::app::{AppError, AppReason, AppResult};
pub use reason::config::{ConfigError, ConfigReason, ConfigResult};
pub use reason::store::{StoreError, StoreReason, StoreResult};
