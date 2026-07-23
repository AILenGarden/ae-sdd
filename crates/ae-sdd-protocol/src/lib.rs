//! Transport-independent wire types for the ae-sdd local daemon protocol.
//!
//! This crate deliberately contains no socket, filesystem, policy, domain, or
//! application dependencies. Consumers perform I/O and authorization outside
//! this boundary.

#![warn(missing_docs)]

mod capability;
mod error;
mod frame;
mod handshake;
mod method;
mod rpc;
mod secret;
mod wire;

pub use capability::{CAPABILITY_TOKEN_SCHEMA_V1, CapabilityTokenWire};
pub use error::{ERROR_DATA_SCHEMA_V1, RpcErrorData, RpcErrorObject, StableErrorCode};
pub use frame::{FrameError, MAX_FRAME_BYTES, decode_frame, encode_frame};
pub use handshake::{
    ENDPOINT_MANIFEST_SCHEMA_V1, EndpointManifest, HandshakeLimits, HandshakeRequest,
    HandshakeResponse, PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1,
};
pub use method::{
    METHOD_COUNT, METHOD_REGISTRY, MethodRequirements, MethodSpec, RequirementSource, RpcMethod,
    UnknownRpcMethod,
};
pub use rpc::{
    ConfirmationRef, JSON_RPC_VERSION, JsonRpcErrorResponse, JsonRpcNotification, JsonRpcRequest,
    JsonRpcResponse, JsonRpcVersion, RequestParams,
};
pub use secret::SecretString;
pub use wire::{
    ClientKind, CompactStatus, ContextPayloadKind, GateOutcomeKind, HookDecision, HostAckOutcome,
    HostActionKind, JobStatus, OperationScope, WorkspaceMode,
};
