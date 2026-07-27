mod support;

use ae_sdd_context::{
    DEFAULT_CONSECUTIVE_SAMPLES, DEFAULT_COOLDOWN_MS, DEFAULT_HIGH_WATERMARK_BPS,
    DEFAULT_LOW_WATERMARK_BPS, PressureDecision, PressureError, PressurePolicy, PressureSample,
    PressureSource, PressureTracker,
};
use ae_sdd_domain::{ContextGeneration, SampleSequence};
use ae_sdd_host::HostAdapterId;

use support::session;

fn sample(seq: u64, used: u64, at: u64) -> PressureSample {
    PressureSample::new(
        HostAdapterId::new("codex").expect("valid adapter"),
        session(1),
        ContextGeneration::new(0),
        SampleSequence::new(seq),
        used,
        1_000,
        PressureSource::HostTokenCounter,
        at,
    )
    .expect("valid pressure sample")
}

#[test]
fn high_low_consecutive_and_cooldown_policy_does_not_oscillate() {
    let mut tracker = PressureTracker::new(
        HostAdapterId::new("codex").expect("valid adapter"),
        session(1),
        ContextGeneration::new(0),
        PressurePolicy::default(),
    );
    assert_eq!(
        tracker.observe(&sample(1, 800, 1_000)),
        Ok(PressureDecision::HighSample { consecutive: 1 })
    );
    assert_eq!(
        tracker.observe(&sample(2, 800, 1_001)),
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
        tracker.observe(&sample(5, 900, 1_004)),
        Ok(PressureDecision::HighSample { consecutive: 1 })
    );
    assert_eq!(
        tracker.observe(&sample(6, 900, 1_005)),
        Ok(PressureDecision::Cooldown)
    );
}

#[test]
fn default_policy_uses_the_frozen_basis_point_thresholds() {
    let policy = PressurePolicy::default();

    assert_eq!(
        policy.high_watermark_basis_points(),
        DEFAULT_HIGH_WATERMARK_BPS
    );
    assert_eq!(
        policy.low_watermark_basis_points(),
        DEFAULT_LOW_WATERMARK_BPS
    );
    assert_eq!(policy.consecutive_samples(), DEFAULT_CONSECUTIVE_SAMPLES);
    assert_eq!(policy.cooldown_ms(), DEFAULT_COOLDOWN_MS);
    assert_eq!(sample(1, 800, 1_000).basis_points(), 8_000);
}

#[test]
fn samples_and_policies_validate_inputs_and_expose_exact_contract_fields() {
    let adapter = HostAdapterId::new("native").expect("valid adapter");
    let generation = ContextGeneration::new(2);
    let value = PressureSample::new(
        adapter.clone(),
        session(9),
        generation,
        SampleSequence::new(3),
        1,
        3,
        PressureSource::HostNativeNotification,
        4_000,
    )
    .expect("valid pressure sample");

    assert_eq!(value.adapter_id(), &adapter);
    assert_eq!(value.session_id(), session(9));
    assert_eq!(value.context_generation(), generation);
    assert_eq!(value.sample_seq(), SampleSequence::new(3));
    assert_eq!(value.observed_at_unix_ms(), 4_000);
    assert_eq!(value.source(), PressureSource::HostNativeNotification);
    assert_eq!(value.permille(), 333);
    assert_eq!(value.basis_points(), 3_333);

    let build = |sequence, used, window, observed| {
        PressureSample::new(
            adapter.clone(),
            session(9),
            generation,
            SampleSequence::new(sequence),
            used,
            window,
            PressureSource::HostTokenCounter,
            observed,
        )
    };
    assert_eq!(build(1, 0, 0, 1), Err(PressureError::InvalidTokenCount));
    assert_eq!(build(1, 101, 100, 1), Err(PressureError::InvalidTokenCount));
    assert_eq!(build(0, 1, 100, 1), Err(PressureError::ZeroSampleSequence));
    assert_eq!(build(1, 1, 100, 0), Err(PressureError::InvalidTimestamp));

    let policy = PressurePolicy::new(900, 500, 3, 10_000).expect("valid policy");
    assert_eq!(policy.high_watermark_permille(), 900);
    assert_eq!(policy.high_watermark_basis_points(), 9_000);
    assert_eq!(policy.low_watermark_permille(), 500);
    assert_eq!(policy.low_watermark_basis_points(), 5_000);
    assert_eq!(policy.consecutive_samples(), 3);
    assert_eq!(policy.cooldown_ms(), 10_000);

    for invalid in [
        PressurePolicy::new(1_001, 500, 2, 1),
        PressurePolicy::new(800, 800, 2, 1),
        PressurePolicy::new(800, 600, 0, 1),
        PressurePolicy::new(800, 600, 2, 0),
    ] {
        assert_eq!(invalid, Err(PressureError::InvalidPolicy));
    }
}

#[test]
fn tracker_fences_identity_generation_replay_and_resets_on_advance() {
    let adapter = HostAdapterId::new("codex").expect("valid adapter");
    let mut tracker = PressureTracker::new(
        adapter.clone(),
        session(1),
        ContextGeneration::new(0),
        PressurePolicy::default(),
    );
    let build = |adapter_id, session_id, generation, sequence| {
        PressureSample::new(
            adapter_id,
            session_id,
            generation,
            SampleSequence::new(sequence),
            700,
            1_000,
            PressureSource::HostTokenCounter,
            1_000 + sequence,
        )
        .expect("valid pressure sample")
    };

    assert_eq!(
        tracker.observe(&build(
            HostAdapterId::new("other").expect("valid adapter"),
            session(1),
            ContextGeneration::new(0),
            1,
        )),
        Err(PressureError::IdentityMismatch)
    );
    assert_eq!(
        tracker.observe(&build(
            adapter.clone(),
            session(2),
            ContextGeneration::new(0),
            1,
        )),
        Err(PressureError::IdentityMismatch)
    );
    assert_eq!(
        tracker.observe(&build(
            adapter.clone(),
            session(1),
            ContextGeneration::new(1),
            1,
        )),
        Err(PressureError::GenerationMismatch)
    );

    let accepted = build(adapter.clone(), session(1), ContextGeneration::new(0), 1);
    assert_eq!(
        tracker.observe(&accepted),
        Ok(PressureDecision::InHysteresisBand)
    );
    assert_eq!(tracker.observe(&accepted), Err(PressureError::SampleReplay));
    assert_eq!(
        tracker.advance_generation(ContextGeneration::new(0)),
        Err(PressureError::GenerationNotMonotonic)
    );
    tracker
        .advance_generation(ContextGeneration::new(1))
        .expect("generation advances");
    assert_eq!(
        tracker.observe(&build(adapter, session(1), ContextGeneration::new(1), 1,)),
        Ok(PressureDecision::InHysteresisBand)
    );
}
