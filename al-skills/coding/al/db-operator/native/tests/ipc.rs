use std::path::Path;

use db_operator_native::ipc::{UNIX_SOCKET_MODE, endpoint_name_for, validate_pipe_sddl};

#[test]
fn ipc_endpoint_is_stable_and_service_scoped() {
    let first = endpoint_name_for("alice", Path::new("C:/Users/alice/AppData/db-operator"));
    let same = endpoint_name_for("alice", Path::new("C:/Users/alice/AppData/db-operator"));
    let other = endpoint_name_for("bob", Path::new("C:/Users/bob/AppData/db-operator"));

    assert_eq!(first, same);
    assert_eq!(first, other);
    #[cfg(windows)]
    assert_eq!(first, r"\\.\pipe\db-operator-v2");
}

#[test]
fn unix_socket_is_group_accessible_but_not_world_accessible() {
    assert_eq!(UNIX_SOCKET_MODE, 0o660);
}

#[test]
fn windows_pipe_sddl_rejects_broad_principals() {
    for weak in [
        "D:(A;;GA;;;WD)",
        "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;AU)",
        "D:P(A;;GA;;;SY)(A;;GA;;;BA)",
    ] {
        assert!(validate_pipe_sddl(weak).is_err(), "must reject {weak}");
    }
    assert!(
        validate_pipe_sddl("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;S-1-5-21-1-2-3-1001)").is_ok()
    );
}
