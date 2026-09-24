use db_operator_native::protocol::{
    MAX_SQL_BYTES, Operation, PROTOCOL_VERSION, Request, decode_request, encode_request,
};
use db_operator_native::registry::WriteLevel;

#[test]
fn protocol_round_trips_query_requests_and_rejects_oversized_frames() {
    let request = Request {
        version: PROTOCOL_VERSION,
        request_id: "request-1".to_owned(),
        capability: "agent-query-token".to_owned(),
        operation: Operation::Query {
            connection: "warehouse".to_owned(),
            sql: "SELECT 1".to_owned(),
            write_level: WriteLevel::None,
        },
    };

    let encoded = encode_request(&request).expect("request should encode");
    assert_eq!(decode_request(&encoded).unwrap(), request);

    let oversized = vec![b'x'; 1024 * 1024 + 1];
    assert!(decode_request(&oversized).is_err());
}

#[test]
fn protocol_rejects_sql_larger_than_64_kib() {
    let request = Request {
        version: PROTOCOL_VERSION,
        request_id: "request-large".to_owned(),
        capability: "agent-query-token".to_owned(),
        operation: Operation::Query {
            connection: "warehouse".to_owned(),
            sql: "x".repeat(MAX_SQL_BYTES + 1),
            write_level: WriteLevel::None,
        },
    };

    let encoded = serde_json::to_vec(&request).unwrap();
    assert!(encode_request(&request).is_err());
    assert!(decode_request(&encoded).is_err());
}

#[test]
fn protocol_rejects_requests_without_a_capability() {
    let frame = br#"{"version":2,"request_id":"request-1","op":"status"}
"#;

    assert!(decode_request(frame).is_err());
}

#[test]
fn agent_protocol_rejects_management_operations() {
    for operation in ["stop", "schema_refresh"] {
        let frame = format!(
            "{{\"version\":2,\"request_id\":\"request-1\",\"capability\":\"token\",\"op\":\"{operation}\",\"connection\":\"warehouse\"}}\n"
        );
        assert!(decode_request(frame.as_bytes()).is_err());
    }
}
