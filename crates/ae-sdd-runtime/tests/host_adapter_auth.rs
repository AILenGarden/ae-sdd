mod support;

use std::sync::Arc;

use ae_sdd_domain::EventStoreId;
use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::{MemoryPersistence, RuntimeConfig};
use serde_json::json;
use uuid::Uuid;

use support::{Harness, params, result, stable_error};

fn registration(credential: &str) -> ae_sdd_protocol::RequestParams<serde_json::Value> {
    let mut request = params(
        json!({"adapterId":"host-a","capabilities":["compact","create","attest"]}),
        1_000,
    );
    request.capability_token = Some(credential.to_owned());
    request.idempotency_key = Some("host-register-a".to_owned());
    request
}

#[test]
fn host_registration_requires_the_boot_scoped_credential() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::HostAdapter);
    let rejected = harness.call(
        &mut connection,
        RpcMethod::HostRegister,
        registration("forged"),
    );
    assert_eq!(stable_error(&rejected), "ENDPOINT_AUTH_FAILED");

    let registered = result(&harness.call(
        &mut connection,
        RpcMethod::HostRegister,
        registration(&harness.host_credential()),
    ));
    assert_eq!(registered["adapterId"], "host-a");
}

#[test]
fn durable_host_registration_recovers_and_rebinds_after_restart() {
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(91),
    )));
    let first = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        92,
        "first-credential".to_owned(),
    );
    let mut first_connection = first.connection(ClientKind::HostAdapter);
    let _ = result(&first.call(
        &mut first_connection,
        RpcMethod::HostRegister,
        registration(&first.host_credential()),
    ));

    let second = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence,
        93,
        "second-credential".to_owned(),
    );
    second.runtime.recover().expect("runtime recovers");
    let mut second_connection = second.connection(ClientKind::HostAdapter);
    let rebound = result(&second.call(
        &mut second_connection,
        RpcMethod::HostRegister,
        registration(&second.host_credential()),
    ));
    assert_eq!(rebound["adapterId"], "host-a");

    let capabilities = result(&second.call(
        &mut second_connection,
        RpcMethod::HostCapabilities,
        params(json!({"adapterId":"host-a"}), 1_000),
    ));
    assert_eq!(capabilities["capabilities"][0], "compact");
}
