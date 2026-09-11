use orion_error::prelude::*;

/// Error carrier for the persistence layer.
pub type StoreError = StructError<StoreReason>;
pub type StoreResult<T> = Result<T, StoreError>;

/// Reason namespace for storage access.
#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum StoreReason {
    #[orion_error(identity = "sys.wist.store.io")]
    Io,
    #[orion_error(identity = "sys.wist.store.json")]
    Json,
    #[orion_error(identity = "sys.wist.store.sql")]
    Sql,
    #[orion_error(identity = "biz.wist.store.conflict")]
    Conflict,
    #[orion_error(identity = "biz.wist.store.not_found")]
    NotFound,
    #[orion_error(identity = "biz.wist.store.enrollment")]
    Enrollment,
    #[orion_error(transparent)]
    General(UnifiedReason),
}
