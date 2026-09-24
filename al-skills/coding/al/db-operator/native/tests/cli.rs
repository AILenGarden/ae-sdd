use assert_cmd::Command;
use serde_json::Value;

#[test]
fn agent_cli_exposes_only_query_status_and_schema() {
    let output = Command::cargo_bin("db-operator")
        .unwrap()
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for command in ["query", "status", "schema"] {
        assert!(
            stdout.contains(&format!("\n  {command}")),
            "help must include {command}"
        );
    }
    for command in [
        "register", "list", "use", "test", "remove", "doctor", "daemon", "serve", "stop",
    ] {
        assert!(
            !stdout.contains(&format!("\n  {command}")),
            "agent help must hide {command}"
        );
    }
}

#[test]
fn admin_cli_owns_connection_and_service_management() {
    let output = Command::cargo_bin("db-operator-admin")
        .unwrap()
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for command in [
        "register", "list", "use", "test", "remove", "doctor", "client",
    ] {
        assert!(
            stdout.contains(command),
            "admin help must include {command}"
        );
    }
    for forbidden in ["query", "daemon", "serve"] {
        assert!(
            !stdout.contains(&format!("\n  {forbidden}")),
            "admin help must not expose legacy {forbidden}"
        );
    }
}

#[test]
fn daemon_binary_is_separate_from_the_agent_client() {
    let output = Command::cargo_bin("db-operator-daemon")
        .unwrap()
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());

    let help = Command::cargo_bin("db-operator-daemon")
        .unwrap()
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let stdout = String::from_utf8(help.stdout).unwrap();
    for option in ["--home", "--endpoint", "--pipe-sddl"] {
        assert!(stdout.contains(option), "daemon help must include {option}");
    }
}

#[test]
fn admin_register_is_non_interactive_when_all_required_flags_are_present() {
    let private_dir = tempfile::tempdir().unwrap();
    let output = Command::cargo_bin("db-operator-admin")
        .unwrap()
        .env("DB_OPERATOR_HOME", private_dir.path())
        .args([
            "register",
            "--name",
            "reporting-test",
            "--engine",
            "postgresql",
            "--host",
            "invalid.example",
            "--port",
            "5432",
            "--database",
            "reporting",
            "--username",
            "read_only_user",
            "--ssl-mode",
            "require",
            "--password-stdin",
        ])
        .write_stdin("temporary-test-password\n")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn admin_issues_daemon_digest_and_agent_client_config() {
    let private_dir = tempfile::tempdir().unwrap();
    let agent_dir = tempfile::tempdir().unwrap();
    let client_path = agent_dir.path().join("client.json");
    std::fs::write(
        private_dir.path().join("connections.json"),
        r#"{
          "version": 3,
          "default": "reporting-prod",
          "connections": {
            "reporting-prod": {
              "engine": "postgresql",
              "endpoint": {"mode": "tcp", "host": "db.internal", "port": 5432},
              "database": "reporting",
              "username": "report_reader",
              "secret_ref": "vault://db-operator/reporting-prod",
              "ssl": {"mode": "require", "ca_file": null, "client_cert": null, "client_key": null},
              "environment": "test",
              "tags": [],
              "limits": {"connect_timeout_seconds": 10, "statement_timeout_seconds": 30, "max_rows": 500},
              "read_only": true
            }
          }
        }"#,
    )
    .unwrap();

    let output = Command::cargo_bin("db-operator-admin")
        .unwrap()
        .env("DB_OPERATOR_HOME", private_dir.path())
        .args([
            "client",
            "issue",
            "--connection",
            "reporting-prod",
            "--endpoint",
            r"\\.\pipe\db-operator-v2",
            "--output",
        ])
        .arg(&client_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let capability_record: Value = serde_json::from_str(
        &std::fs::read_to_string(private_dir.path().join("capability.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(capability_record["connection"], "reporting-prod");
    assert_eq!(capability_record["digest"].as_str().unwrap().len(), 64);
    let client: Value =
        serde_json::from_str(&std::fs::read_to_string(client_path).unwrap()).unwrap();
    assert_eq!(client["connection"], "reporting-prod");
    assert_eq!(client["endpoint"], r"\\.\pipe\db-operator-v2");
    assert!(client["capability"].as_str().unwrap().len() >= 32);
    for forbidden in ["host", "port", "username", "password", "url", "registry"] {
        assert!(
            client.get(forbidden).is_none(),
            "client config leaked {forbidden}"
        );
    }
    assert!(
        !capability_record["digest"]
            .as_str()
            .unwrap()
            .contains(client["capability"].as_str().unwrap())
    );
}
