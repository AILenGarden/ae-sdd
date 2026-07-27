#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Pure lifecycle planning and mutation-intent generation.

mod canonical;
mod engine;
mod projection;
mod validation;

pub use engine::{
    LifecycleEngine, MAX_CONFIRMATION_APPROVED_AT_BYTES, MAX_CONFIRMATION_APPROVED_BY_BYTES,
    MAX_CONFIRMATION_ID_BYTES, MAX_FILE_LOCK_TTL_MS,
};
