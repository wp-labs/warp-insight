use crate::reason::{app::AppReason, config::ConfigReason, store::StoreReason};

impl From<StoreReason> for AppReason {
    fn from(value: StoreReason) -> Self {
        match value {
            StoreReason::General(reason) => AppReason::General(reason),
            _ => AppReason::Store,
        }
    }
}

impl From<ConfigReason> for AppReason {
    fn from(value: ConfigReason) -> Self {
        match value {
            ConfigReason::General(reason) => AppReason::General(reason),
            _ => AppReason::Config,
        }
    }
}
