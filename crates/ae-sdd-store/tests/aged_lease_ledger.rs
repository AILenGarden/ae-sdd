//! Compatibility contract for lease ledgers written before Rust owned the
//! store.
//!
//! The Python→Rust migration was validated by behavioural equivalence on
//! artifacts the Rust side produced, so nothing ever asserted that Rust can
//! read what Python actually left behind. `.auto-engineering/` is gitignored,
//! which is why no such fixture existed: real aged state cannot be committed
//! from its live location. These fixtures are scrubbed copies of real files
//! (identifiers redacted, structure verbatim) and exist so this class of
//! regression fails in CI instead of at a user's first `lease.status`.

use std::path::PathBuf;

use ae_sdd_store::LeaseLedger;

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("tests/fixtures/aged-state")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {}: {error}", path.display()))
}

#[test]
fn a_released_python_ledger_is_readable_and_holds_no_lease() {
    let ledger = LeaseLedger::from_json(&fixture("python-released-lease.v1.json"))
        .expect("a released Python ledger must stay readable");
    assert!(
        ledger.active().is_none(),
        "a released ledger must not resurrect its last holder"
    );
    assert!(
        ledger.last_fencing_token().get() > 0,
        "the generation recorded by Python must be preserved"
    );
}

#[test]
fn an_active_python_ledger_is_readable_and_keeps_its_generation() {
    let ledger = LeaseLedger::from_json(&fixture("python-active-lease.v1.json"))
        .expect("an active Python ledger must stay readable");
    assert!(
        ledger.last_fencing_token().get() > 0,
        "the generation recorded by Python must be preserved"
    );
}

/// `renewed` extends a lease in place and `acquired` opens one. Only
/// `released`/`expired`/`broken` end a generation, so a ledger whose history
/// mixes all five kinds must not report a tombstone per event.
#[test]
fn non_terminal_history_events_do_not_become_tombstones() {
    for name in [
        "python-released-lease.v1.json",
        "python-active-lease.v1.json",
    ] {
        let bytes = fixture(name);
        let raw: serde_json::Value = serde_json::from_slice(&bytes).expect("fixture is valid JSON");
        let events = raw["history"].as_array().expect("fixture has history");
        let terminal = events
            .iter()
            .filter(|event| {
                matches!(
                    event["event"].as_str(),
                    Some("released" | "expired" | "broken")
                )
            })
            .count();
        assert!(
            events.len() > terminal,
            "{name} must contain non-terminal events for this test to mean anything"
        );
        let ledger = LeaseLedger::from_json(&bytes).expect("aged ledger is readable");
        assert_eq!(
            ledger.tombstones().len(),
            terminal,
            "{name}: only terminal events may become tombstones"
        );
    }
}

/// Reading an aged ledger must not let a Python-era proof validate again, so
/// the next grant has to sit above every generation the file mentions.
#[test]
fn the_generation_never_regresses_below_recorded_history() {
    for name in [
        "python-released-lease.v1.json",
        "python-active-lease.v1.json",
    ] {
        let bytes = fixture(name);
        let raw: serde_json::Value = serde_json::from_slice(&bytes).expect("fixture is valid JSON");
        let highest = raw["history"]
            .as_array()
            .expect("fixture has history")
            .iter()
            .filter_map(|event| event["fencingToken"].as_u64())
            .chain(raw["fencingToken"].as_u64())
            .max()
            .expect("fixture records at least one generation");
        let ledger = LeaseLedger::from_json(&bytes).expect("aged ledger is readable");
        assert!(
            ledger.last_fencing_token().get() >= highest,
            "{name}: generation regressed below recorded history"
        );
    }
}

/// Read compatibility is one-way: the next write emits the native shape.
#[test]
fn an_aged_ledger_is_written_back_in_the_native_shape() {
    for name in [
        "python-released-lease.v1.json",
        "python-active-lease.v1.json",
    ] {
        let ledger = LeaseLedger::from_json(&fixture(name)).expect("aged ledger is readable");
        let encoded = ledger.to_canonical_json().expect("canonical encode");
        let text = String::from_utf8(encoded).expect("canonical JSON is UTF-8");
        assert!(
            text.contains("\"lastFencingToken\":"),
            "{name}: normalized output must carry the native discriminator"
        );
        LeaseLedger::from_json(text.as_bytes()).expect("normalized output round-trips");
    }
}
