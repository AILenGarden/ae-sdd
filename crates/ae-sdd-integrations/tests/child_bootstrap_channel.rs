//! One-shot child bootstrap channel contract (Task 14 spike).
//!
//! Proves the bridge can hand a delegation claim to a physical child process
//! without the raw claim entering argv, environment variables, files, logs, or
//! any persisted record. The channel is single-consumer, challenge-bound, and
//! deadline-bound.

use std::time::Duration;

use ae_sdd_integrations::host_bridge::{
    ChildBootstrapChallenge, ChildBootstrapError, OneShotBootstrapChannel,
};
use uuid::Uuid;

const CLAIM: &str = "claim-secret-do-not-leak-6f2a91c4";

fn challenge(session: Uuid, action: Uuid) -> ChildBootstrapChallenge {
    ChildBootstrapChallenge {
        child_session_id: session,
        host_action_id: action,
    }
}

/// Publishes a claim, then a matching challenge consumes it exactly once.
#[tokio::test]
async fn matching_challenge_receives_envelope_once() {
    let session = Uuid::new_v4();
    let action = Uuid::new_v4();
    let channel = OneShotBootstrapChannel::publish(
        challenge(session, action),
        CLAIM.to_owned(),
        Duration::from_secs(5),
    )
    .expect("publish one-shot bootstrap endpoint");

    let endpoint = channel.endpoint_name().to_owned();
    let consumer = tokio::spawn(async move {
        OneShotBootstrapChannel::consume(&endpoint, challenge(session, action)).await
    });

    channel
        .serve_once()
        .await
        .expect("serve the single consumer");
    let envelope = consumer.await.expect("join consumer").expect("envelope");

    assert_eq!(envelope.claim, CLAIM);
    assert_eq!(envelope.child_session_id, session);
    assert_eq!(envelope.host_action_id, action);
}

/// A wrong challenge never receives the claim.
#[tokio::test]
async fn wrong_challenge_is_rejected() {
    let session = Uuid::new_v4();
    let action = Uuid::new_v4();
    let channel = OneShotBootstrapChannel::publish(
        challenge(session, action),
        CLAIM.to_owned(),
        Duration::from_secs(5),
    )
    .expect("publish one-shot bootstrap endpoint");

    let endpoint = channel.endpoint_name().to_owned();
    let consumer = tokio::spawn(async move {
        // Correct action, forged session id.
        OneShotBootstrapChannel::consume(&endpoint, challenge(Uuid::new_v4(), action)).await
    });

    let served = channel.serve_once().await;
    let outcome = consumer.await.expect("join consumer");

    assert!(matches!(
        served,
        Err(ChildBootstrapError::ChallengeMismatch)
    ));
    assert!(
        outcome.is_err(),
        "forged challenge must not receive a claim"
    );
}

/// The endpoint is single-consumer: a second connect finds nothing to take.
#[tokio::test]
async fn second_consumer_cannot_replay() {
    let session = Uuid::new_v4();
    let action = Uuid::new_v4();
    let channel = OneShotBootstrapChannel::publish(
        challenge(session, action),
        CLAIM.to_owned(),
        Duration::from_secs(5),
    )
    .expect("publish one-shot bootstrap endpoint");

    let endpoint = channel.endpoint_name().to_owned();
    let consumer_endpoint = endpoint.clone();
    let first = tokio::spawn(async move {
        OneShotBootstrapChannel::consume(&consumer_endpoint, challenge(session, action)).await
    });
    channel.serve_once().await.expect("serve first consumer");
    first.await.expect("join").expect("first envelope");

    // The listener is consumed by `serve_once`; the endpoint must be gone.
    let replay = OneShotBootstrapChannel::consume(&endpoint, challenge(session, action)).await;
    assert!(replay.is_err(), "claim must not be deliverable twice");
}

/// An expired deadline releases the claim without delivering it.
#[tokio::test]
async fn expired_channel_refuses_delivery() {
    let session = Uuid::new_v4();
    let action = Uuid::new_v4();
    let channel = OneShotBootstrapChannel::publish(
        challenge(session, action),
        CLAIM.to_owned(),
        Duration::from_millis(20),
    )
    .expect("publish one-shot bootstrap endpoint");

    tokio::time::sleep(Duration::from_millis(60)).await;

    let served = channel.serve_once().await;
    assert!(
        matches!(served, Err(ChildBootstrapError::Expired)),
        "expired channel must fail closed, got {served:?}"
    );
}

/// The claim is never observable through the channel's own surfaces.
#[tokio::test]
async fn claim_is_absent_from_observable_surfaces() {
    let session = Uuid::new_v4();
    let action = Uuid::new_v4();
    let channel = OneShotBootstrapChannel::publish(
        challenge(session, action),
        CLAIM.to_owned(),
        Duration::from_secs(5),
    )
    .expect("publish one-shot bootstrap endpoint");

    assert!(
        !channel.endpoint_name().contains(CLAIM),
        "endpoint name must not embed the claim"
    );
    assert!(
        !format!("{channel:?}").contains(CLAIM),
        "Debug output must redact the claim"
    );

    // The three correlation variables handed to the child are the allowlist;
    // none of them may carry the claim.
    let exported = channel.child_environment();
    assert_eq!(exported.len(), 3, "exactly three correlation variables");
    for (key, value) in &exported {
        assert!(!value.contains(CLAIM), "{key} must not carry the raw claim");
    }
    let keys: Vec<&str> = exported.iter().map(|(key, _)| key.as_str()).collect();
    assert!(keys.contains(&"AE_SDD_CHILD_BOOTSTRAP_ENDPOINT"));
    assert!(keys.contains(&"AE_SDD_CHILD_SESSION_ID"));
    assert!(keys.contains(&"AE_SDD_HOST_ACTION_ID"));
}

/// Dropping the channel closes the endpoint and zeroizes the claim.
#[tokio::test]
async fn drop_closes_endpoint() {
    let session = Uuid::new_v4();
    let action = Uuid::new_v4();
    let channel = OneShotBootstrapChannel::publish(
        challenge(session, action),
        CLAIM.to_owned(),
        Duration::from_secs(5),
    )
    .expect("publish one-shot bootstrap endpoint");
    let endpoint = channel.endpoint_name().to_owned();
    drop(channel);

    let outcome = OneShotBootstrapChannel::consume(&endpoint, challenge(session, action)).await;
    assert!(outcome.is_err(), "dropped endpoint must be unreachable");
}
