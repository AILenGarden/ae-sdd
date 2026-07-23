use std::error::Error;
use std::fmt;

use ae_sdd_protocol::RpcMethod;
use serde::{Deserialize, Serialize};

/// Evidence status copied from the compatibility routing manifest.
///
/// A resolved route can be callable while still being provisional. Callers
/// must not translate `Pending` into a parity or release-completeness claim.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImplementationStatus {
    Implemented,
    BreakingFixVerified,
    Pending,
}

/// Native Rust build kernel selected by a compatibility route.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeJobKind {
    Offline,
    Compile,
    Init,
    Install,
    Distribute,
    Harness,
    Migrate,
    Admin,
}

impl NativeJobKind {
    /// Stable wire spelling accepted by the build job request.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Compile => "compile",
            Self::Init => "init",
            Self::Install => "install",
            Self::Distribute => "distribute",
            Self::Harness => "harness",
            Self::Migrate => "migrate",
            Self::Admin => "admin",
        }
    }
}

/// Payload selector that the CLI composition root must attach to an RPC.
///
/// The router does not invent operation or job payload schemas. It only
/// supplies the frozen selector and leaves the caller-provided payload intact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyRpcAdapter {
    /// Forward the caller payload to the exact registered RPC method.
    Passthrough,
    /// Invoke `operation.execute` for this typed operation.
    TypedOperation { operation: String },
    /// Submit an admin/diagnostic job through the daemon scheduler.
    JobSubmission {
        job: NativeJobKind,
        entrypoint: String,
    },
}

/// Side-effect-free dispatch target returned to `main.rs`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyTarget {
    /// Authenticated local daemon RPC.
    Rpc {
        method: RpcMethod,
        adapter: LegacyRpcAdapter,
    },
    /// Legitimate Rust build kernel launched without a daemon policy bypass.
    NativeBuildJob {
        job: NativeJobKind,
        entrypoint: String,
    },
    /// Explicitly removed spelling. This is metadata, never a launch target.
    Rejected {
        stable_code: String,
        remediation: String,
    },
}

/// Fail-closed and evidence metadata retained from the frozen fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyRouteContract {
    pub deadline_ms: u64,
    pub fail_closed: bool,
    pub fixture: String,
    pub evidence: String,
    pub status: ImplementationStatus,
}

/// One frozen command id and its secure replacement target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyCommandRoute {
    pub command_id: String,
    pub identity_workspace: bool,
    pub identity_work_item: bool,
    pub identity_session: bool,
    pub target: LegacyTarget,
    pub contract: LegacyRouteContract,
}

impl LegacyCommandRoute {
    pub fn command_tokens(&self) -> impl Iterator<Item = &str> {
        self.command_id.split(' ')
    }

    #[must_use]
    pub fn is_provisional(&self) -> bool {
        self.contract.status == ImplementationStatus::Pending
    }
}

/// A longest-prefix legacy command match plus untouched command arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLegacyCommand {
    pub route: LegacyCommandRoute,
    pub consumed_arguments: usize,
    pub trailing_arguments: Vec<String>,
}

/// Stable fail-closed routing errors. The CLI maps every variant to non-zero.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyRouteError {
    InvalidManifest(String),
    MissingCommand,
    UnknownOrRemovedDeprecated(String),
    RemovedDeprecated {
        command_id: String,
        stable_code: String,
        remediation: String,
    },
}

impl fmt::Display for LegacyRouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManifest(reason) => {
                write!(
                    formatter,
                    "embedded legacy routing manifest is invalid: {reason}"
                )
            }
            Self::MissingCommand => formatter.write_str("a legacy command is required"),
            Self::UnknownOrRemovedDeprecated(command) => write!(
                formatter,
                "legacy command is unknown or removed-deprecated and was denied: {command}"
            ),
            Self::RemovedDeprecated {
                command_id,
                stable_code,
                remediation,
            } => write!(
                formatter,
                "{stable_code}: legacy command {command_id} was removed; {remediation}"
            ),
        }
    }
}

impl Error for LegacyRouteError {}
