mod support;

use ae_sdd_context::{
    PressureDecision, PressurePolicy, PressureSample, PressureSource, PressureTracker,
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
