use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{
    SERVICE_PLAN_SCHEMA, SERVICE_REQUEST_SCHEMA, ServiceError, ServiceLifecycleContract,
    ServiceLifecyclePlan, ServiceLifecycleRequest, ServiceManagerCommand, ServiceOperation,
    ServicePermissionPolicy, ServicePlatform,
};

const DESCRIPTION: &str = "ae-sdd per-user daemon";

pub fn generate_service_lifecycle_plan(
    request: &ServiceLifecycleRequest,
) -> Result<ServiceLifecyclePlan, ServiceError> {
    validate_request(request)?;
    let descriptor_path = descriptor_path(request);
    let daemon_argv = daemon_argv(request);
    let descriptor_contents = match request.platform {
        ServicePlatform::Windows => render_windows(request, &daemon_argv),
        ServicePlatform::Macos => render_macos(request, &daemon_argv),
        ServicePlatform::Linux => render_linux(request, &daemon_argv),
    };
    if contains_forbidden_runtime(&descriptor_contents) {
        return Err(ServiceError::SecretInDescriptor);
    }
    let descriptor_digest = digest(descriptor_contents.as_bytes());
    let manager_commands = manager_commands(request, &descriptor_path);
    Ok(ServiceLifecyclePlan {
        schema_version: SERVICE_PLAN_SCHEMA,
        platform: request.platform,
        operation: request.operation,
        manager: request.platform.manager(),
        service_name: request.platform.service_name(),
        user_home: request.user_home.clone(),
        state_dir: request.state_dir.clone(),
        descriptor_path,
        descriptor_digest,
        descriptor_contents,
        manager_commands,
        permission_policy: permission_policy(request),
        lifecycle_contract: ServiceLifecycleContract {
            daemon_argv,
            secrets_embedded: false,
            shell_wrapper: false,
            current_user_identity: request.user_identity.clone(),
            state_retained_on_uninstall: true,
        },
    })
}

fn validate_request(request: &ServiceLifecycleRequest) -> Result<(), ServiceError> {
    if request.schema_version != SERVICE_REQUEST_SCHEMA {
        return Err(ServiceError::Schema(request.schema_version.clone()));
    }
    for (field, path) in [
        ("executable", &request.executable),
        ("stateDir", &request.state_dir),
        ("workingDirectory", &request.working_directory),
        ("userHome", &request.user_home),
    ] {
        validate_absolute_path(field, path)?;
    }
    if request.allowed_roots.is_empty() || request.allowed_roots.len() > 64 {
        return Err(ServiceError::InvalidField("allowedRoots"));
    }
    for root in &request.allowed_roots {
        validate_absolute_path("allowedRoots", root)?;
    }
    validate_text("userIdentity", &request.user_identity)?;
    if request.restart_delay_seconds == 0 || request.restart_delay_seconds > 300 {
        return Err(ServiceError::InvalidRestartDelay);
    }
    for argument in &request.extra_arguments {
        validate_text("extraArguments", argument)?;
        if looks_sensitive(argument) {
            return Err(ServiceError::SecretInDescriptor);
        }
    }
    for (key, value) in &request.environment {
        if key.is_empty()
            || !key.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            })
        {
            return Err(ServiceError::InvalidEnvironmentKey(key.clone()));
        }
        validate_text("environment", value)?;
        if looks_sensitive(key) || looks_sensitive(value) {
            return Err(ServiceError::SecretInDescriptor);
        }
    }
    Ok(())
}

fn validate_absolute_path(field: &'static str, path: &Path) -> Result<(), ServiceError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(ServiceError::InvalidPath(field));
    }
    validate_text(field, &path.to_string_lossy())
}

fn validate_text(field: &'static str, value: &str) -> Result<(), ServiceError> {
    if value.trim().is_empty() || value.len() > 32 * 1024 || value.contains(['\0', '\r', '\n']) {
        return Err(ServiceError::InvalidField(field));
    }
    Ok(())
}

fn daemon_argv(request: &ServiceLifecycleRequest) -> Vec<String> {
    let mut arguments = vec![
        request.executable.to_string_lossy().into_owned(),
        "serve".to_owned(),
        "--state-dir".to_owned(),
        request.state_dir.to_string_lossy().into_owned(),
    ];
    arguments.extend(request.extra_arguments.iter().cloned());
    for root in &request.allowed_roots {
        arguments.push("--allowed-root".to_owned());
        arguments.push(root.to_string_lossy().into_owned());
    }
    arguments
}

fn descriptor_path(request: &ServiceLifecycleRequest) -> PathBuf {
    match request.platform {
        ServicePlatform::Windows => request
            .user_home
            .join("AppData/Local/ae-sdd/service/ae-sdd-daemon.xml"),
        ServicePlatform::Macos => request
            .user_home
            .join("Library/LaunchAgents/com.ae-sdd.daemon.plist"),
        ServicePlatform::Linux => request
            .user_home
            .join(".config/systemd/user/ae-sdd.service"),
    }
}

fn permission_policy(request: &ServiceLifecycleRequest) -> ServicePermissionPolicy {
    match request.platform {
        ServicePlatform::Windows => ServicePermissionPolicy {
            user_scope_only: true,
            elevation_required: false,
            runtime_directory_mode: None,
            descriptor_mode: None,
            endpoint_manifest_mode: None,
            windows_dacl_principal: Some(request.user_identity.clone()),
            windows_inheritance_removed: true,
        },
        ServicePlatform::Macos | ServicePlatform::Linux => ServicePermissionPolicy {
            user_scope_only: true,
            elevation_required: false,
            runtime_directory_mode: Some("0700"),
            descriptor_mode: Some("0600"),
            endpoint_manifest_mode: Some("0600"),
            windows_dacl_principal: None,
            windows_inheritance_removed: false,
        },
    }
}

fn render_linux(request: &ServiceLifecycleRequest, daemon_argv: &[String]) -> String {
    let mut output =
        String::from("[Unit]\nDescription=ae-sdd per-user daemon\n\n[Service]\nType=simple\n");
    output.push_str("ExecStart=");
    output.push_str(
        &daemon_argv
            .iter()
            .map(|value| quote_systemd(value))
            .collect::<Vec<_>>()
            .join(" "),
    );
    output.push_str("\nWorkingDirectory=");
    output.push_str(&quote_systemd(&request.working_directory.to_string_lossy()));
    for (key, value) in &request.environment {
        output.push_str("\nEnvironment=");
        output.push_str(&quote_systemd(&format!("{key}={value}")));
    }
    output.push_str("\nUMask=0077\nNoNewPrivileges=true\nRestart=on-failure\nRestartSec=");
    output.push_str(&request.restart_delay_seconds.to_string());
    output.push_str("\n\n[Install]\nWantedBy=default.target\n");
    output
}

fn render_macos(request: &ServiceLifecycleRequest, daemon_argv: &[String]) -> String {
    let mut output = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n",
    );
    plist_string(&mut output, "Label", request.platform.service_name());
    output.push_str("  <key>ProgramArguments</key>\n  <array>\n");
    for argument in daemon_argv {
        output.push_str("    <string>");
        output.push_str(&escape_xml(argument));
        output.push_str("</string>\n");
    }
    output.push_str("  </array>\n");
    plist_string(
        &mut output,
        "WorkingDirectory",
        &request.working_directory.to_string_lossy(),
    );
    output.push_str("  <key>EnvironmentVariables</key>\n  <dict>\n");
    for (key, value) in &request.environment {
        plist_string(&mut output, key, value);
    }
    output.push_str(
        "  </dict>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>KeepAlive</key>\n  <dict>\n    <key>SuccessfulExit</key>\n    <false/>\n  </dict>\n  <key>ThrottleInterval</key>\n  <integer>",
    );
    output.push_str(&request.restart_delay_seconds.to_string());
    output.push_str(
        "</integer>\n  <key>Umask</key>\n  <integer>63</integer>\n  <key>ProcessType</key>\n  <string>Background</string>\n</dict>\n</plist>\n",
    );
    output
}

fn render_windows(request: &ServiceLifecycleRequest, daemon_argv: &[String]) -> String {
    let arguments = daemon_argv
        .iter()
        .skip(1)
        .map(|value| quote_windows_argument(value))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Task version=\"1.4\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n  <RegistrationInfo><Description>{}</Description></RegistrationInfo>\n  <Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers>\n  <Principals><Principal id=\"Author\"><UserId>{}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>\n  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><AllowHardTerminate>true</AllowHardTerminate><StartWhenAvailable>true</StartWhenAvailable><RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable><IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings><AllowStartOnDemand>true</AllowStartOnDemand><Enabled>true</Enabled><Hidden>false</Hidden><RunOnlyIfIdle>false</RunOnlyIfIdle><WakeToRun>false</WakeToRun><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><Priority>7</Priority><RestartOnFailure><Interval>PT{}S</Interval><Count>3</Count></RestartOnFailure></Settings>\n  <Actions Context=\"Author\"><Exec><Command>{}</Command><Arguments>{}</Arguments><WorkingDirectory>{}</WorkingDirectory></Exec></Actions>\n</Task>\n",
        escape_xml(DESCRIPTION),
        escape_xml(&request.user_identity),
        request.restart_delay_seconds,
        escape_xml(&request.executable.to_string_lossy()),
        escape_xml(&arguments),
        escape_xml(&request.working_directory.to_string_lossy()),
    )
}

fn manager_commands(
    request: &ServiceLifecycleRequest,
    descriptor_path: &Path,
) -> Vec<ServiceManagerCommand> {
    match (request.platform, request.operation) {
        (ServicePlatform::Windows, ServiceOperation::Install) => vec![
            command(
                "register",
                "schtasks.exe",
                [
                    "/Create".to_owned(),
                    "/F".to_owned(),
                    "/TN".to_owned(),
                    request.platform.service_name().to_owned(),
                    "/XML".to_owned(),
                    display(descriptor_path),
                ],
            ),
            command(
                "start",
                "schtasks.exe",
                [
                    "/Run".to_owned(),
                    "/TN".to_owned(),
                    request.platform.service_name().to_owned(),
                ],
            ),
        ],
        (ServicePlatform::Windows, ServiceOperation::Uninstall) => vec![command(
            "unregister",
            "schtasks.exe",
            [
                "/Delete".to_owned(),
                "/F".to_owned(),
                "/TN".to_owned(),
                request.platform.service_name().to_owned(),
            ],
        )],
        (ServicePlatform::Windows, ServiceOperation::Status) => vec![command(
            "status",
            "schtasks.exe",
            [
                "/Query".to_owned(),
                "/TN".to_owned(),
                request.platform.service_name().to_owned(),
                "/FO".to_owned(),
                "CSV".to_owned(),
                "/V".to_owned(),
            ],
        )],
        (ServicePlatform::Macos, ServiceOperation::Install) => vec![
            command(
                "register",
                "launchctl",
                [
                    "bootstrap".to_owned(),
                    format!("gui/{}", request.user_identity),
                    display(descriptor_path),
                ],
            ),
            command(
                "start",
                "launchctl",
                [
                    "kickstart".to_owned(),
                    "-k".to_owned(),
                    format!(
                        "gui/{}/{}",
                        request.user_identity,
                        request.platform.service_name()
                    ),
                ],
            ),
        ],
        (ServicePlatform::Macos, ServiceOperation::Uninstall) => vec![command(
            "unregister",
            "launchctl",
            [
                "bootout".to_owned(),
                format!(
                    "gui/{}/{}",
                    request.user_identity,
                    request.platform.service_name()
                ),
            ],
        )],
        (ServicePlatform::Macos, ServiceOperation::Status) => vec![command(
            "status",
            "launchctl",
            [
                "print".to_owned(),
                format!(
                    "gui/{}/{}",
                    request.user_identity,
                    request.platform.service_name()
                ),
            ],
        )],
        (ServicePlatform::Linux, ServiceOperation::Install) => vec![
            command(
                "reload",
                "systemctl",
                ["--user".to_owned(), "daemon-reload".to_owned()],
            ),
            command(
                "register-and-start",
                "systemctl",
                [
                    "--user".to_owned(),
                    "enable".to_owned(),
                    "--now".to_owned(),
                    request.platform.service_name().to_owned(),
                ],
            ),
        ],
        (ServicePlatform::Linux, ServiceOperation::Uninstall) => vec![
            command(
                "unregister",
                "systemctl",
                [
                    "--user".to_owned(),
                    "disable".to_owned(),
                    "--now".to_owned(),
                    request.platform.service_name().to_owned(),
                ],
            ),
            command(
                "reload",
                "systemctl",
                ["--user".to_owned(), "daemon-reload".to_owned()],
            ),
        ],
        (ServicePlatform::Linux, ServiceOperation::Status) => vec![command(
            "status",
            "systemctl",
            [
                "--user".to_owned(),
                "show".to_owned(),
                "--property=ActiveState,SubState,MainPID,FragmentPath".to_owned(),
                request.platform.service_name().to_owned(),
            ],
        )],
    }
}

fn command<const N: usize>(
    purpose: &'static str,
    program: &'static str,
    arguments: [String; N],
) -> ServiceManagerCommand {
    ServiceManagerCommand {
        purpose,
        program,
        arguments: arguments.into(),
    }
}

fn quote_systemd(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
        .replace('$', "$$");
    format!("\"{escaped}\"")
}

fn quote_windows_argument(value: &str) -> String {
    if !value.is_empty()
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character == '"')
    {
        return value.to_owned();
    }
    let mut output = String::from("\"");
    let mut slashes = 0_usize;
    for character in value.chars() {
        match character {
            '\\' => slashes += 1,
            '"' => {
                output.push_str(&"\\".repeat(slashes.saturating_mul(2).saturating_add(1)));
                output.push('"');
                slashes = 0;
            }
            _ => {
                output.push_str(&"\\".repeat(slashes));
                output.push(character);
                slashes = 0;
            }
        }
    }
    output.push_str(&"\\".repeat(slashes.saturating_mul(2)));
    output.push('"');
    output
}

fn plist_string(output: &mut String, key: &str, value: &str) {
    output.push_str("  <key>");
    output.push_str(&escape_xml(key));
    output.push_str("</key>\n  <string>");
    output.push_str(&escape_xml(value));
    output.push_str("</string>\n");
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn looks_sensitive(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase().replace('-', "_");
    [
        "endpoint_token",
        "capability_private",
        "private_key",
        "claim_token",
        "password",
        "secret",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn contains_forbidden_runtime(contents: &str) -> bool {
    let normalized = contents.to_ascii_lowercase();
    normalized.contains("endpoint_token")
        || normalized.contains("capability_private")
        || normalized.contains("claim_token")
        || normalized.contains("python.exe")
        || normalized.contains("/usr/bin/python")
        || normalized.contains("tools/bin/ae-sdd")
        || normalized.contains("sh -c")
        || normalized.contains("cmd /c")
        || normalized.contains("powershell -command")
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(super) fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_quoting_preserves_empty_spaces_quotes_and_trailing_slashes() {
        assert_eq!(quote_windows_argument("plain"), "plain");
        assert_eq!(quote_windows_argument(""), "\"\"");
        assert_eq!(quote_windows_argument("two words"), "\"two words\"");
        assert_eq!(quote_windows_argument("a\\\"b"), "\"a\\\\\\\"b\"");
        assert_eq!(
            quote_windows_argument("C:\\path with space\\"),
            "\"C:\\path with space\\\\\""
        );
    }
}
