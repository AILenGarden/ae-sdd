use ae_sdd_domain::EventStoreId;
use ae_sdd_runtime::{
    ExecutionAuthorityCursor, ExecutionCheckpointRecord, ExecutionCheckpointRecovery,
    ExecutionCheckpointRecoveryInput, ExecutionCheckpointScope, MemoryPersistence, PersistencePort,
};
use uuid::Uuid;

fn capsule_digest() -> String {
    "1".repeat(64)
}

fn stale_capsule_digest() -> String {
    "9".repeat(64)
}

fn queue_digest() -> String {
    "b".repeat(64)
}

fn stale_queue_digest() -> String {
    "c".repeat(64)
}

fn scope() -> ExecutionCheckpointScope {
    ExecutionCheckpointScope {
        workspace_id: "workspace-1".to_owned(),
        work_item_id: "WI-EXEC-1".to_owned(),
        session_id: "session-1".to_owned(),
    }
}

fn authority(
    capsule_digest: &str,
    queue_digest: &str,
    active_ordinal: u32,
) -> ExecutionAuthorityCursor {
    ExecutionAuthorityCursor {
        capsule_ref: ".auto-engineering/WI-EXEC-1/execution/capsule.json".to_owned(),
        capsule_digest: capsule_digest.to_owned(),
        queue_digest: queue_digest.to_owned(),
        active_ordinal,
    }
}

fn persisted_checkpoint(capsule_digest: &str) -> ExecutionCheckpointRecord {
    ExecutionCheckpointRecord {
        workspace_id: "workspace-1".to_owned(),
        work_item_id: "WI-EXEC-1".to_owned(),
        session_id: "session-1".to_owned(),
        capsule_digest: capsule_digest.to_owned(),
        queue_digest: queue_digest(),
        active_ordinal: 2,
        no_progress_batches: 2,
        source_cache_hits: 7,
        source_cache_misses: 3,
        updated_event_seq: 41,
        updated_at_unix_ms: 1_785_000_000_000,
    }
}

fn recovery_input(
    capsule_digest: &str,
    queue_digest: &str,
    active_ordinal: u32,
) -> ExecutionCheckpointRecoveryInput {
    ExecutionCheckpointRecoveryInput {
        scope: scope(),
        authority: authority(capsule_digest, queue_digest, active_ordinal),
        updated_event_seq: 57,
        now_unix_ms: 1_785_000_100_000,
    }
}

#[test]
fn restart_restores_persisted_checkpoint_when_project_authority_matches() {
    let persistence = MemoryPersistence::new(EventStoreId::from_uuid(Uuid::from_u128(401)));
    let persisted = persisted_checkpoint(&capsule_digest());
    persistence
        .store_execution_checkpoint(&persisted)
        .expect("checkpoint persists before restart");

    // Daemon restart: a fresh boot holds only the metadata store and the
    // project authority cursor; the persisted row must accelerate the rebuild.
    let loaded = persistence
        .load_execution_checkpoint(&scope())
        .expect("checkpoint load")
        .expect("persisted checkpoint survives restart");
    let recovery = ExecutionCheckpointRecord::recover(
        &recovery_input(&capsule_digest(), &queue_digest(), 2),
        Some(loaded),
    );

    let ExecutionCheckpointRecovery::Restored(restored) = recovery else {
        panic!("matching authority must restore the persisted checkpoint");
    };
    assert_eq!(restored, persisted);
    assert_eq!(restored.no_progress_batches, 2);
    assert_eq!(restored.source_cache_hits, 7);
    assert_eq!(restored.source_cache_misses, 3);
    assert_eq!(restored.updated_event_seq, 41);
}

#[test]
fn restart_discards_stale_checkpoint_when_capsule_digest_mismatches_authority() {
    let persistence = MemoryPersistence::new(EventStoreId::from_uuid(Uuid::from_u128(402)));
    let stale = persisted_checkpoint(&stale_capsule_digest());
    persistence
        .store_execution_checkpoint(&stale)
        .expect("checkpoint persists before restart");

    // The project authority moved to a new approved capsule while the daemon
    // was down; the SQLite row is only an accelerator and must be discarded.
    let loaded = persistence
        .load_execution_checkpoint(&scope())
        .expect("checkpoint load")
        .expect("persisted checkpoint survives restart");
    let input = recovery_input(&capsule_digest(), &queue_digest(), 3);
    let authority_before = input.authority.clone();
    let recovery = ExecutionCheckpointRecord::recover(&input, Some(loaded));

    let ExecutionCheckpointRecovery::Rebuilt { record, discard } = recovery else {
        panic!("stale capsule digest must rebuild from the project authority");
    };
    assert_eq!(discard, Some(stale));
    assert_eq!(record.capsule_digest, capsule_digest());
    assert_eq!(record.queue_digest, queue_digest());
    assert_eq!(record.active_ordinal, 3);
    assert_eq!(record.no_progress_batches, 0);
    assert_eq!(record.source_cache_hits, 0);
    assert_eq!(record.source_cache_misses, 0);
    assert_eq!(record.updated_event_seq, 57);
    assert_eq!(record.updated_at_unix_ms, 1_785_000_100_000);
    // Recovery is a pure decision: the project authority snapshot is only
    // read, never written back.
    assert_eq!(input.authority, authority_before);

    // Applying the decision discards the stale cache and stores the rebuild;
    // no project state is mutated by this path.
    persistence
        .discard_execution_checkpoint(&scope())
        .expect("stale checkpoint discarded");
    assert!(
        persistence
            .load_execution_checkpoint(&scope())
            .expect("checkpoint load")
            .is_none(),
        "stale checkpoint must be discarded before the rebuild is stored"
    );
    persistence
        .store_execution_checkpoint(&record)
        .expect("rebuilt checkpoint persists");
    let reloaded = persistence
        .load_execution_checkpoint(&scope())
        .expect("checkpoint load")
        .expect("rebuilt checkpoint survives");
    assert_eq!(reloaded, record);
}

#[test]
fn restart_discards_checkpoint_when_queue_digest_drifts() {
    let persistence = MemoryPersistence::new(EventStoreId::from_uuid(Uuid::from_u128(403)));
    let stale = persisted_checkpoint(&capsule_digest());
    persistence
        .store_execution_checkpoint(&stale)
        .expect("checkpoint persists before restart");

    let loaded = persistence
        .load_execution_checkpoint(&scope())
        .expect("checkpoint load")
        .expect("persisted checkpoint survives restart");
    let recovery = ExecutionCheckpointRecord::recover(
        &recovery_input(&capsule_digest(), &stale_queue_digest(), 2),
        Some(loaded),
    );

    let ExecutionCheckpointRecovery::Rebuilt { record, discard } = recovery else {
        panic!("stale queue digest must rebuild from the project authority");
    };
    assert_eq!(discard, Some(stale));
    assert_eq!(record.queue_digest, stale_queue_digest());
    assert_eq!(record.no_progress_batches, 0);
}

#[test]
fn restart_rebuilds_from_authority_when_no_checkpoint_exists() {
    let persistence = MemoryPersistence::new(EventStoreId::from_uuid(Uuid::from_u128(404)));
    let loaded = persistence
        .load_execution_checkpoint(&scope())
        .expect("checkpoint load");
    assert!(loaded.is_none());

    let recovery = ExecutionCheckpointRecord::recover(
        &recovery_input(&capsule_digest(), &queue_digest(), 1),
        loaded,
    );

    let ExecutionCheckpointRecovery::Rebuilt { record, discard } = recovery else {
        panic!("missing checkpoint must rebuild from the project authority");
    };
    assert_eq!(discard, None);
    assert_eq!(record.capsule_digest, capsule_digest());
    assert_eq!(record.active_ordinal, 1);
    assert_eq!(record.no_progress_batches, 0);
    assert_eq!(record.updated_event_seq, 57);
}
