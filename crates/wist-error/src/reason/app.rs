use orion_error::prelude::*;

/// Error carrier for the top-level process / use-case boundary.
pub type AppError = StructError<AppReason>;
pub type AppResult<T> = Result<T, AppError>;

/// Reason namespace for the top-level boundary.
#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum AppReason {
    #[orion_error(identity = "biz.wist.app.store")]
    Store,
    #[orion_error(identity = "conf.wist.app.config")]
    Config,
    #[orion_error(identity = "biz.wist.app.invalid_args")]
    InvalidArgs,
    #[orion_error(transparent)]
    General(UnifiedReason),
}
