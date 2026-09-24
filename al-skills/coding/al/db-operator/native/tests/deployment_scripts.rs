use std::fs;
use std::path::Path;

fn script(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("scripts")
            .join(name),
    )
    .unwrap()
}

#[test]
fn agent_installers_build_only_the_query_client() {
    for name in ["install.ps1", "install.sh"] {
        let content = script(name);
        assert!(content.contains("--no-default-features"));
        assert!(content.contains("--features client"));
        assert!(content.contains("--bin db-operator"));
        assert!(!content.contains("db-operator-admin"));
        assert!(!content.contains("db-operator-daemon"));
        assert!(!content.contains(" doctor"));
    }
}

#[test]
fn service_installers_enforce_separate_identity_and_ipc_permissions() {
    let windows = script("install-service.ps1");
    for required in [
        "NT SERVICE\\db-operator",
        "function Get-VirtualServiceSid",
        "$serviceSid =",
        "(A;;GA;;;$serviceSid)",
        "DB_OPERATOR_PIPE_SDDL",
        "icacls",
        "client issue",
        "--pipe-sddl",
        "--service",
    ] {
        assert!(
            windows.contains(required),
            "Windows installer missing {required}"
        );
    }
    assert!(
        windows.find("New-Service -Name 'db-operator'").unwrap()
            < windows.find("icacls $InstallDir").unwrap(),
        "the service identity must exist before ACLs reference it"
    );
    for required in [
        "New-Service -Name 'db-operator'",
        "-BinaryPathName $binaryPath",
        "-DisplayName 'DB Operator Security Daemon'",
        "-StartupType Automatic",
        "$scConfigCommand =",
        "sc.exe config db-operator obj= \"",
        "& cmd.exe /d /s /c $scConfigCommand",
        "$scFailureCommand =",
        "sc.exe failure db-operator reset= 86400 actions=",
        "& cmd.exe /d /s /c $scFailureCommand",
    ] {
        assert!(
            windows.contains(required),
            "Windows installer missing {required}"
        );
    }

    let unix = script("install-service.sh");
    for required in [
        "db-operator-agents",
        "chmod 2770",
        "client issue",
        "systemd",
        "LaunchDaemon",
        "--endpoint",
        // The Agent must own its client config; the daemon-private staging copy stays service-owned.
        "install -d -o \"$agent_user\"",
        "install -o \"$agent_user\"",
        "0700 \"$(dirname \"$agent_config\")\"",
    ] {
        assert!(unix.contains(required), "Unix installer missing {required}");
    }
}

#[test]
fn unix_management_ui_launcher_requires_root_and_reuses_a_running_server() {
    let launcher = script("open-registry.sh");
    for required in [
        "id -u",
        "exec sudo",
        "api/session",
        "DB_OPERATOR_HOME",
        "registry-server.mjs",
        "--no-browser",
    ] {
        assert!(
            launcher.contains(required),
            "open-registry.sh missing {required}"
        );
    }
    assert!(
        !launcher.contains("--pipe-sddl"),
        "the unix launcher must not carry Windows-only options"
    );
}

#[test]
fn unix_package_ships_lf_scripts_and_the_management_ui() {
    let packaging = script("package-unix.sh");
    for required in [
        "macos",
        "linux",
        "aarch64",
        "open-registry.sh",
        "registry-server.mjs",
        "ui/index.html",
        "tar -czf",
        "release-manifest.json",
    ] {
        assert!(
            packaging.contains(required),
            "package-unix.sh missing {required}"
        );
    }
    for line in packaging.lines() {
        assert!(
            !line.contains('\r'),
            "packaging scripts must be stored with LF endings"
        );
    }
}

#[test]
fn shipped_posix_scripts_use_lf_endings() {
    for name in [
        "install.sh",
        "install-service.sh",
        "open-registry.sh",
        "package-unix.sh",
    ] {
        let content = script(name);
        assert!(
            !content.contains('\r'),
            "{name} would break on macOS and Linux with CRLF endings"
        );
    }
}

#[test]
fn flow_check_reuses_the_package_instead_of_touching_the_host() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let flow = fs::read_to_string(root.join("tests").join("e2e-flow-windows.ps1")).unwrap();
    for required in [
        "[Parameter(Mandatory = $true)] [string]$Package",
        "release-manifest.json",
        r"scripts\install.ps1",
        "'client', 'issue'",
        "H1 client reports unavailability when the daemon stops",
    ] {
        assert!(flow.contains(required), "flow check missing {required}");
    }
    // The check must never install the Windows service or write outside its temp tree.
    for forbidden in ["install-service.ps1", "New-Service", "sc.exe config", "ProgramData"] {
        assert!(
            !flow.contains(forbidden),
            "flow check must not touch the host: found {forbidden}"
        );
    }
    let fixture = fs::read_to_string(root.join("tests").join("e2e-mysql-fixture.ps1")).unwrap();
    for required in [
        "dbo_reader",
        "dbo_writer",
        "appdb.orders",
        "--initialize-insecure",
    ] {
        assert!(fixture.contains(required), "fixture missing {required}");
    }
}

#[test]
fn daemon_does_not_gate_queries_on_database_privilege_audits() {
    let daemon = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("daemon.rs"),
    )
    .unwrap();
    assert!(daemon.contains("PoolHandle::connect(profile, &password)"));
    assert!(!daemon.contains("PoolHandle::connect_verified"));
    assert!(!daemon.contains("verify_read_only_privileges"));
}
