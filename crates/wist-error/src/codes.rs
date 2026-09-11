//! Stable numeric error codes + tags, decoupled from reason `Display`.
//!
//! The first digit hints the HTTP mapping (2=no content, 4=client, 5=server),
//! the middle two digits reserve blocks per domain, and the last two digits are
//! scoped reasons. Keeping these constants together makes future assignments
//! deliberate and visible.

use crate::reason::{app::AppReason, config::ConfigReason, store::StoreReason};

pub mod plan {
    pub const DEFAULT_TAG: &str = "wist.err";

    pub mod store {
        pub const TAG: &str = "store";
        pub const IO: u16 = 50001;
        pub const JSON: u16 = 50002;
        pub const SQL: u16 = 50003;
        pub const CONFLICT: u16 = 40901;
        pub const NOT_FOUND: u16 = 40401;
        pub const ENROLLMENT: u16 = 40101;
        pub const UVS: u16 = 50000;
    }

    pub mod config {
        pub const TAG: &str = "config";
        pub const IO: u16 = 50011;
        pub const PARSE: u16 = 42211;
        pub const VALIDATION: u16 = 42212;
        pub const UVS: u16 = 50010;
    }

    pub mod app {
        pub const TAG: &str = "app";
        pub const STORE: u16 = 50020;
        pub const CONFIG: u16 = 50021;
        pub const INVALID_ARGS: u16 = 40001;
        pub const UVS: u16 = 50029;
    }
}

/// Maps a reason to a stable numeric code and tag for observability / API use.
pub trait SysErrorCode {
    fn sys_code(&self) -> u16;
    fn sys_tag(&self) -> &'static str {
        plan::DEFAULT_TAG
    }
}

impl SysErrorCode for StoreReason {
    fn sys_code(&self) -> u16 {
        match self {
            StoreReason::Io => plan::store::IO,
            StoreReason::Json => plan::store::JSON,
            StoreReason::Sql => plan::store::SQL,
            StoreReason::Conflict => plan::store::CONFLICT,
            StoreReason::NotFound => plan::store::NOT_FOUND,
            StoreReason::Enrollment => plan::store::ENROLLMENT,
            StoreReason::General(_) => plan::store::UVS,
        }
    }
    fn sys_tag(&self) -> &'static str {
        plan::store::TAG
    }
}

impl SysErrorCode for ConfigReason {
    fn sys_code(&self) -> u16 {
        match self {
            ConfigReason::Io => plan::config::IO,
            ConfigReason::Parse => plan::config::PARSE,
            ConfigReason::Validation => plan::config::VALIDATION,
            ConfigReason::General(_) => plan::config::UVS,
        }
    }
    fn sys_tag(&self) -> &'static str {
        plan::config::TAG
    }
}

impl SysErrorCode for AppReason {
    fn sys_code(&self) -> u16 {
        match self {
            AppReason::Store => plan::app::STORE,
            AppReason::Config => plan::app::CONFIG,
            AppReason::InvalidArgs => plan::app::INVALID_ARGS,
            AppReason::General(_) => plan::app::UVS,
        }
    }
    fn sys_tag(&self) -> &'static str {
        plan::app::TAG
    }
}
