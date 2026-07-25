//! `SessionBootstrapPort`: the SPI-1 contract future daemon session services
//! call through. Defined here (inward) so `ae-sdd-session` remains the sole
//! owner of the bootstrap decision; callers depend on this trait, never on
//! `service::bootstrap` directly.

use ae_sdd_contracts::session::SessionBootstrapRequest;

use crate::error::SessionBootstrapError;
use crate::model::{BootstrapPlan, BootstrapSnapshot};

/// Pure bootstrap decision port: given a frozen request and an explicit,
/// already-resolved snapshot, decides the ordered bootstrap plan. Performs
/// no I/O; every fact it needs must be in `request` or `snapshot`.
pub trait SessionBootstrapPort {
    /// Decides the ordered bootstrap plan for `request` under `snapshot`.
    fn bootstrap(
        &self,
        request: &SessionBootstrapRequest,
        snapshot: &BootstrapSnapshot,
    ) -> Result<BootstrapPlan, SessionBootstrapError>;
}

/// Zero-sized default implementation of [`SessionBootstrapPort`], delegating
/// to the pure `service::bootstrap` decision function. Callers (future
/// daemon session services) inject this behind the trait rather than calling
/// `service::bootstrap` directly, keeping composition roots swappable.
#[derive(Clone, Copy, Debug, Default)]
pub struct PureSessionBootstrap;

impl SessionBootstrapPort for PureSessionBootstrap {
    fn bootstrap(
        &self,
        request: &SessionBootstrapRequest,
        snapshot: &BootstrapSnapshot,
    ) -> Result<BootstrapPlan, SessionBootstrapError> {
        crate::service::bootstrap(request, snapshot)
    }
}
