//! Wall-clock instants shared by deadline-bearing contracts.
//!
//! This module exists because several frozen contracts need to express "not
//! valid after" without agreeing on a clock implementation: an
//! `InstructionEnvelope` deadline (`ae-sdd-daemon-design.md` §10.2), a Series
//! input deadline, a lease expiry, and Gate staleness all compare instants.
//! Carrying a bare `u64` at each site would let two of them disagree about
//! units, so the unit is fixed here once.

use std::fmt;

/// An instant expressed as milliseconds since the Unix epoch, UTC.
///
/// Milliseconds rather than seconds because Series deadlines and lease expiries
/// are routinely sub-second apart; `u64` rather than `i64` because these
/// contracts never express instants before 1970, and rejecting negatives at the
/// type level is cheaper than validating them at every use.
///
/// This type deliberately has no `now()`. A contract must be constructible and
/// verifiable without reading a clock, so the caller supplies the instant and
/// the daemon remains the only component deciding what "now" means.
/// Serialization lives in `ae-sdd-contracts::serde_domain`, keeping this crate
/// free of wire concerns like every other domain value type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EpochMillis(u64);

impl EpochMillis {
    /// Wraps a millisecond count.
    pub const fn new(millis: u64) -> Self {
        Self(millis)
    }

    /// Returns the raw millisecond count.
    ///
    /// Named `get` to match the other numeric newtypes in this crate
    /// (`StateRevision`, `EventSequence`), which lets the shared
    /// `serde_domain::counter_adapter` serialize this type without a bespoke
    /// adapter. `constraints/api.md` already fixes the unit as Unix
    /// milliseconds via `deadlineUnixMs`, so the wire form is a bare `u64`.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns whether `self` is strictly later than `other`.
    ///
    /// Named rather than left to `>` at call sites so a deadline check reads as
    /// a claim about time instead of an integer comparison.
    pub const fn is_after(self, other: Self) -> bool {
        self.0 > other.0
    }

    /// Returns whether an instruction stamped `self` has expired at `now`.
    ///
    /// Expiry is inclusive of the deadline instant: an envelope whose
    /// `expiresAt` equals `now` is expired. §10.2 uses the deadline to stop a
    /// stale instruction replaying, so the boundary fails closed.
    pub const fn is_expired_at(self, now: Self) -> bool {
        now.0 >= self.0
    }
}

impl fmt::Display for EpochMillis {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}ms", self.0)
    }
}
