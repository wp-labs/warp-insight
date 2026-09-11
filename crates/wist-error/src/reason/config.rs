use orion_error::prelude::*;

/// Error carrier for config loading and validation.
pub type ConfigError = StructError<ConfigReason>;
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Reason namespace for config loading and validation.
#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum ConfigReason {
    #[orion_error(identity = "conf.wist.config.io")]
    Io,
    #[orion_error(identity = "conf.wist.config.parse")]
    Parse,
    #[orion_error(identity = "conf.wist.config.validation")]
    Validation,
    #[orion_error(transparent)]
    General(UnifiedReason),
}
