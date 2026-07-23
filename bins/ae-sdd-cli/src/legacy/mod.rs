//! Frozen legacy CLI leaf routing.
//!
//! This module is deliberately side-effect free. The binary composition root
//! resolves a legacy argv prefix here, then chooses the concrete async daemon
//! client or native build launcher itself.

mod manifest;
mod model;
mod native;
mod argv;
mod router;

pub use argv::{
    LegacyArgumentError, LegacyRequestSource, LegacyRpcInvocation, parse_rpc_invocation,
    validate_request_params,
};
pub use manifest::embedded_routes;
pub use model::{
    ImplementationStatus, LegacyCommandRoute, LegacyRouteContract, LegacyRouteError,
    LegacyRpcAdapter, LegacyTarget, NativeJobKind, ResolvedLegacyCommand,
};
pub use native::{
    LegacyNativeInvocation, LegacyNativeRequestSource, TemporaryJsonRequest,
    parse_native_invocation, verify_offline_request,
};
pub use router::{resolve_command_id, resolve_legacy_argv};

/// Frozen number of legacy leaf commands in protocol compatibility v1.
pub const LEGACY_COMMAND_COUNT: usize = 113;
