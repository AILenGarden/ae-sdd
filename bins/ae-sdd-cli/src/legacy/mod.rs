//! Frozen legacy CLI leaf routing.
//!
//! This module is deliberately side-effect free. The binary composition root
//! resolves a legacy argv prefix here, then chooses the concrete async daemon
//! client or native build launcher itself.

mod argv;
mod job;
mod job_poll;
mod manifest;
mod model;
mod native;
mod router;
mod rpc_adapter;
mod temp;
mod tokens;
mod typed;

pub use argv::{
    LegacyRequestSource, LegacyRpcInvocation, parse_rpc_invocation, validate_request_params,
};
pub use job::adapt_job_submission;
pub use job_poll::{LegacyJobPollContext, validate_job_terminal_status};
pub use manifest::embedded_routes;
pub use model::{
    ImplementationStatus, LegacyCommandRoute, LegacyRouteContract, LegacyRouteError,
    LegacyRpcAdapter, LegacyTarget, NativeJobKind, ResolvedLegacyCommand,
};
pub use native::{
    LegacyNativeInvocation, LegacyNativeRequestSource, parse_native_invocation,
    verify_offline_request,
};
pub use router::{resolve_command_id, resolve_legacy_argv};
pub use rpc_adapter::{adapt_passthrough_request, validate_passthrough_result};
pub use temp::TemporaryJsonRequest;
pub use tokens::LegacyArgumentError;
pub use typed::adapt_typed_operation_request;

/// Frozen number of legacy leaf commands in protocol compatibility v1.
pub const LEGACY_COMMAND_COUNT: usize = 113;
