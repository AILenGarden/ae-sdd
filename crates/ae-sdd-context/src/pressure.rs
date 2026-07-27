use ae_sdd_domain::{ContextGeneration, SampleSequence, SessionId};
use ae_sdd_host::HostAdapterId;
use thiserror::Error;

pub const DEFAULT_HIGH_WATERMARK_PERMILLE: u16 = 800;
pub const DEFAULT_LOW_WATERMARK_PERMILLE: u16 = 600;
/// Frozen high watermark in basis points (80%).
pub const DEFAULT_HIGH_WATERMARK_BPS: u16 = DEFAULT_HIGH_WATERMARK_PERMILLE * 10;
/// Frozen recovery watermark in basis points (60%).
pub const DEFAULT_LOW_WATERMARK_BPS: u16 = DEFAULT_LOW_WATERMARK_PERMILLE * 10;
pub const DEFAULT_CONSECUTIVE_SAMPLES: u16 = 2;
pub const DEFAULT_COOLDOWN_MS: u64 = 300_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressureSource {
    HostTokenCounter,
    HostNativeNotification,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PressureSample {
    adapter_id: HostAdapterId,
    session_id: SessionId,
    context_generation: ContextGeneration,
    sample_seq: SampleSequence,
    used_tokens: u64,
    context_window_tokens: u64,
    source: PressureSource,
    observed_at_unix_ms: u64,
}

impl PressureSample {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        adapter_id: HostAdapterId,
        session_id: SessionId,
        context_generation: ContextGeneration,
        sample_seq: SampleSequence,
        used_tokens: u64,
        context_window_tokens: u64,
        source: PressureSource,
        observed_at_unix_ms: u64,
    ) -> Result<Self, PressureError> {
        if context_window_tokens == 0 || used_tokens > context_window_tokens {
            return Err(PressureError::InvalidTokenCount);
        }
        if sample_seq == SampleSequence::ZERO {
            return Err(PressureError::ZeroSampleSequence);
        }
        if observed_at_unix_ms == 0 {
            return Err(PressureError::InvalidTimestamp);
        }
        Ok(Self {
            adapter_id,
            session_id,
            context_generation,
            sample_seq,
            used_tokens,
            context_window_tokens,
            source,
            observed_at_unix_ms,
        })
    }

    #[must_use]
    pub fn adapter_id(&self) -> &HostAdapterId {
        &self.adapter_id
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    #[must_use]
    pub const fn context_generation(&self) -> ContextGeneration {
        self.context_generation
    }

    #[must_use]
    pub const fn sample_seq(&self) -> SampleSequence {
        self.sample_seq
    }

    #[must_use]
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }

    #[must_use]
    pub const fn source(&self) -> PressureSource {
        self.source
    }

    #[must_use]
    pub fn permille(&self) -> u16 {
        let value = u128::from(self.used_tokens) * 1_000 / u128::from(self.context_window_tokens);
        u16::try_from(value).unwrap_or(u16::MAX)
    }

    /// Returns utilization in basis points (10000 = 100%).
    #[must_use]
    pub fn basis_points(&self) -> u16 {
        let value = u128::from(self.used_tokens) * 10_000 / u128::from(self.context_window_tokens);
        u16::try_from(value).unwrap_or(u16::MAX)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PressurePolicy {
    high_watermark_permille: u16,
    low_watermark_permille: u16,
    consecutive_samples: u16,
    cooldown_ms: u64,
}

impl PressurePolicy {
    pub fn new(
        high_watermark_permille: u16,
        low_watermark_permille: u16,
        consecutive_samples: u16,
        cooldown_ms: u64,
    ) -> Result<Self, PressureError> {
        if high_watermark_permille > 1_000
            || low_watermark_permille >= high_watermark_permille
            || consecutive_samples == 0
            || cooldown_ms == 0
        {
            return Err(PressureError::InvalidPolicy);
        }
        Ok(Self {
            high_watermark_permille,
            low_watermark_permille,
            consecutive_samples,
            cooldown_ms,
        })
    }

    #[must_use]
    pub const fn high_watermark_permille(self) -> u16 {
        self.high_watermark_permille
    }

    /// Returns the high watermark in basis points.
    #[must_use]
    pub const fn high_watermark_basis_points(self) -> u16 {
        self.high_watermark_permille * 10
    }

    #[must_use]
    pub const fn low_watermark_permille(self) -> u16 {
        self.low_watermark_permille
    }

    /// Returns the recovery watermark in basis points.
    #[must_use]
    pub const fn low_watermark_basis_points(self) -> u16 {
        self.low_watermark_permille * 10
    }

    #[must_use]
    pub const fn consecutive_samples(self) -> u16 {
        self.consecutive_samples
    }

    #[must_use]
    pub const fn cooldown_ms(self) -> u64 {
        self.cooldown_ms
    }
}

impl Default for PressurePolicy {
    fn default() -> Self {
        Self {
            high_watermark_permille: DEFAULT_HIGH_WATERMARK_PERMILLE,
            low_watermark_permille: DEFAULT_LOW_WATERMARK_PERMILLE,
            consecutive_samples: DEFAULT_CONSECUTIVE_SAMPLES,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressureDecision {
    BelowLowWatermark,
    InHysteresisBand,
    HighSample { consecutive: u16 },
    Cooldown,
    TriggerCompact,
}

#[derive(Clone, Debug)]
pub struct PressureTracker {
    adapter_id: HostAdapterId,
    session_id: SessionId,
    generation: ContextGeneration,
    policy: PressurePolicy,
    last_sample_seq: SampleSequence,
    consecutive_high: u16,
    armed: bool,
    last_triggered_at_unix_ms: Option<u64>,
}

impl PressureTracker {
    #[must_use]
    pub fn new(
        adapter_id: HostAdapterId,
        session_id: SessionId,
        generation: ContextGeneration,
        policy: PressurePolicy,
    ) -> Self {
        Self {
            adapter_id,
            session_id,
            generation,
            policy,
            last_sample_seq: SampleSequence::ZERO,
            consecutive_high: 0,
            armed: true,
            last_triggered_at_unix_ms: None,
        }
    }

    pub fn observe(&mut self, sample: &PressureSample) -> Result<PressureDecision, PressureError> {
        if sample.adapter_id != self.adapter_id || sample.session_id != self.session_id {
            return Err(PressureError::IdentityMismatch);
        }
        if sample.context_generation != self.generation {
            return Err(PressureError::GenerationMismatch);
        }
        if sample.sample_seq <= self.last_sample_seq {
            return Err(PressureError::SampleReplay);
        }
        self.last_sample_seq = sample.sample_seq;

        let pressure = sample.permille();
        if pressure <= self.policy.low_watermark_permille {
            self.consecutive_high = 0;
            self.armed = true;
            return Ok(PressureDecision::BelowLowWatermark);
        }
        if pressure < self.policy.high_watermark_permille {
            self.consecutive_high = 0;
            return Ok(PressureDecision::InHysteresisBand);
        }
        if !self.armed {
            return Ok(PressureDecision::Cooldown);
        }

        self.consecutive_high = self.consecutive_high.saturating_add(1);
        if self.consecutive_high < self.policy.consecutive_samples {
            return Ok(PressureDecision::HighSample {
                consecutive: self.consecutive_high,
            });
        }
        if self.last_triggered_at_unix_ms.is_some_and(|last| {
            sample.observed_at_unix_ms.saturating_sub(last) < self.policy.cooldown_ms
        }) {
            return Ok(PressureDecision::Cooldown);
        }

        self.consecutive_high = 0;
        self.armed = false;
        self.last_triggered_at_unix_ms = Some(sample.observed_at_unix_ms);
        Ok(PressureDecision::TriggerCompact)
    }

    pub fn advance_generation(
        &mut self,
        generation: ContextGeneration,
    ) -> Result<(), PressureError> {
        self.generation
            .advance_to(generation)
            .map_err(|_| PressureError::GenerationNotMonotonic)?;
        self.generation = generation;
        self.last_sample_seq = SampleSequence::ZERO;
        self.consecutive_high = 0;
        self.armed = true;
        Ok(())
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PressureError {
    #[error("used tokens must be within a non-zero context window")]
    InvalidTokenCount,
    #[error("pressure sample sequence must be greater than zero")]
    ZeroSampleSequence,
    #[error("pressure sample timestamp must be greater than zero")]
    InvalidTimestamp,
    #[error("pressure policy thresholds, count, or cooldown are invalid")]
    InvalidPolicy,
    #[error("pressure sample adapter/session does not match tracker")]
    IdentityMismatch,
    #[error("pressure sample context generation does not match tracker")]
    GenerationMismatch,
    #[error("pressure sample sequence is duplicate or out of order")]
    SampleReplay,
    #[error("context generation must advance monotonically")]
    GenerationNotMonotonic,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn sample(seq: u64, used: u64, observed: u64) -> PressureSample {
        PressureSample::new(
            HostAdapterId::new("codex").expect("valid adapter"),
            SessionId::from_uuid(Uuid::from_u128(1)),
            ContextGeneration::new(0),
            SampleSequence::new(seq),
            used,
            1_000,
            PressureSource::HostTokenCounter,
            observed,
        )
        .expect("valid sample")
    }

    #[test]
    fn two_high_samples_trigger_once_until_low_rearms() {
        let mut tracker = PressureTracker::new(
            HostAdapterId::new("codex").expect("valid adapter"),
            SessionId::from_uuid(Uuid::from_u128(1)),
            ContextGeneration::new(0),
            PressurePolicy::default(),
        );

        assert_eq!(
            tracker.observe(&sample(1, 800, 1_000)),
            Ok(PressureDecision::HighSample { consecutive: 1 })
        );
        assert_eq!(
            tracker.observe(&sample(2, 801, 1_001)),
            Ok(PressureDecision::TriggerCompact)
        );
        assert_eq!(
            tracker.observe(&sample(3, 900, 1_002)),
            Ok(PressureDecision::Cooldown)
        );
        assert_eq!(
            tracker.observe(&sample(4, 600, 1_003)),
            Ok(PressureDecision::BelowLowWatermark)
        );
        assert_eq!(
            tracker.observe(&sample(5, 900, 301_100)),
            Ok(PressureDecision::HighSample { consecutive: 1 })
        );
        assert_eq!(
            tracker.observe(&sample(6, 900, 301_101)),
            Ok(PressureDecision::TriggerCompact)
        );
    }

    #[test]
    fn duplicate_sample_is_rejected() {
        let mut tracker = PressureTracker::new(
            HostAdapterId::new("codex").expect("valid adapter"),
            SessionId::from_uuid(Uuid::from_u128(1)),
            ContextGeneration::new(0),
            PressurePolicy::default(),
        );
        tracker
            .observe(&sample(1, 500, 1_000))
            .expect("first sample");
        assert_eq!(
            tracker.observe(&sample(1, 500, 1_001)),
            Err(PressureError::SampleReplay)
        );
    }
}
