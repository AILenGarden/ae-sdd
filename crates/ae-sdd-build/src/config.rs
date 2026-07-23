use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::jobs::PermissionClass;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceTarget {
    SystemdUser,
    LaunchdAgent,
    WindowsUser,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceConfigInput {
    pub service_name: String,
    pub description: String,
    pub executable: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub allowed_roots: Vec<String>,
    pub working_directory: String,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    pub restart_delay_seconds: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    UserPrompt,
    PreTool,
    PostTool,
    Stop,
}

impl HookEvent {
    const fn as_str(self) -> &'static str {
        match self {
            Self::UserPrompt => "user_prompt",
            Self::PreTool => "pre_tool",
            Self::PostTool => "post_tool",
            Self::Stop => "stop",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookHost {
    ClaudeCode,
    Codex,
    Hermes,
    Mavis,
    Zcode,
}

impl HookHost {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::Hermes => "hermes",
            Self::Mavis => "mavis",
            Self::Zcode => "zcode",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookConfigInput {
    pub host: HookHost,
    pub executable: String,
    #[serde(default)]
    pub common_arguments: Vec<String>,
    pub events: Vec<HookEvent>,
    pub deadline_ms: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedConfig {
    pub schema_version: String,
    pub relative_path: String,
    pub contents: String,
    pub permission: PermissionClass,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("configuration field {0} is empty or contains a line/NUL delimiter")]
    InvalidField(&'static str),
    #[error("service name must contain only ASCII letters, digits, '.', '_' or '-'")]
    InvalidServiceName,
    #[error("environment key is not a portable identifier: {0}")]
    InvalidEnvironmentKey(String),
    #[error("hook event list must be non-empty and contain no duplicates")]
    InvalidHookEvents,
    #[error("hook deadline must be between 1 and 250 milliseconds")]
    InvalidHookDeadline,
    #[error("failed to encode generated configuration: {0}")]
    Encode(#[from] serde_json::Error),
}

pub fn generate_service_config(
    target: ServiceTarget,
    input: &ServiceConfigInput,
) -> Result<GeneratedConfig, ConfigError> {
    validate_service(input)?;
    match target {
        ServiceTarget::SystemdUser => Ok(GeneratedConfig {
            schema_version: "ae-sdd-service-config/v1".to_owned(),
            relative_path: format!("systemd/user/{}.service", input.service_name),
            contents: render_systemd(input),
            permission: PermissionClass::PrivateFile,
        }),
        ServiceTarget::LaunchdAgent => Ok(GeneratedConfig {
            schema_version: "ae-sdd-service-config/v1".to_owned(),
            relative_path: format!("launchd/{}.plist", input.service_name),
            contents: render_launchd(input),
            permission: PermissionClass::PrivateFile,
        }),
        ServiceTarget::WindowsUser => Ok(GeneratedConfig {
            schema_version: "ae-sdd-service-config/v1".to_owned(),
            relative_path: format!("windows/{}.service.json", input.service_name),
            contents: serde_json::to_string_pretty(&WindowsServiceManifest::from(input))? + "\n",
            permission: PermissionClass::PrivateFile,
        }),
    }
}

pub fn generate_hook_config(input: &HookConfigInput) -> Result<GeneratedConfig, ConfigError> {
    validate_text("executable", &input.executable)?;
    if input.deadline_ms == 0 || input.deadline_ms > 250 {
        return Err(ConfigError::InvalidHookDeadline);
    }
    let mut events = input.events.clone();
    events.sort_unstable();
    events.dedup();
    if events.is_empty() || events.len() != input.events.len() {
        return Err(ConfigError::InvalidHookEvents);
    }
    for argument in &input.common_arguments {
        validate_text("commonArguments", argument)?;
    }

    let hooks = events
        .into_iter()
        .map(|event| HookEntry {
            event: event.as_str(),
            executable: input.executable.clone(),
            arguments: input
                .common_arguments
                .iter()
                .cloned()
                .chain([
                    "hook".to_owned(),
                    "--method".to_owned(),
                    format!("hook.{}", event.as_str()),
                    "--request-json".to_owned(),
                    "-".to_owned(),
                ])
                .collect(),
            deadline_ms: input.deadline_ms,
            fail_closed: true,
        })
        .collect();
    let manifest = HookManifest {
        schema_version: "ae-sdd-hook-config/v1",
        host: input.host.as_str(),
        hooks,
    };
    Ok(GeneratedConfig {
        schema_version: "ae-sdd-hook-config/v1".to_owned(),
        relative_path: format!("hooks/{}.json", input.host.as_str()),
        contents: serde_json::to_string_pretty(&manifest)? + "\n",
        permission: PermissionClass::PrivateFile,
    })
}

fn validate_service(input: &ServiceConfigInput) -> Result<(), ConfigError> {
    if input.service_name.is_empty()
        || !input
            .service_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ConfigError::InvalidServiceName);
    }
    validate_text("description", &input.description)?;
    validate_text("executable", &input.executable)?;
    validate_text("workingDirectory", &input.working_directory)?;
    for argument in &input.arguments {
        validate_text("arguments", argument)?;
    }
    if input.allowed_roots.is_empty() || input.allowed_roots.len() > 64 {
        return Err(ConfigError::InvalidField("allowedRoots"));
    }
    for root in &input.allowed_roots {
        validate_text("allowedRoots", root)?;
    }
    for (key, value) in &input.environment {
        if key.is_empty()
            || !key.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            })
        {
            return Err(ConfigError::InvalidEnvironmentKey(key.clone()));
        }
        validate_text("environment", value)?;
    }
    Ok(())
}

fn validate_text(field: &'static str, value: &str) -> Result<(), ConfigError> {
    if value.is_empty() || value.contains(['\0', '\r', '\n']) {
        return Err(ConfigError::InvalidField(field));
    }
    Ok(())
}

fn render_systemd(input: &ServiceConfigInput) -> String {
    let mut output = String::new();
    output.push_str("[Unit]\nDescription=");
    output.push_str(&escape_ini(&input.description));
    output.push_str("\n\n[Service]\nType=simple\nExecStart=");
    output.push(' ');
    output.push_str(&quote_systemd(&input.executable));
    for argument in service_arguments(input) {
        output.push(' ');
        output.push_str(&quote_systemd(&argument));
    }
    output.push_str("\nWorkingDirectory=");
    output.push_str(&quote_systemd(&input.working_directory));
    for (key, value) in &input.environment {
        output.push_str("\nEnvironment=");
        output.push_str(&quote_systemd(&format!("{key}={value}")));
    }
    output.push_str("\nRestart=on-failure\nRestartSec=");
    output.push_str(&input.restart_delay_seconds.to_string());
    output.push_str("\n\n[Install]\nWantedBy=default.target\n");
    output
}

fn quote_systemd(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
        .replace('$', "$$");
    format!("\"{escaped}\"")
}

fn escape_ini(value: &str) -> String {
    value.replace('%', "%%")
}

fn render_launchd(input: &ServiceConfigInput) -> String {
    let mut output = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n",
    );
    plist_pair(&mut output, "Label", &input.service_name);
    output.push_str("  <key>ProgramArguments</key>\n  <array>\n");
    for value in std::iter::once(input.executable.clone()).chain(service_arguments(input)) {
        output.push_str("    <string>");
        output.push_str(&escape_xml(&value));
        output.push_str("</string>\n");
    }
    output.push_str("  </array>\n");
    plist_pair(&mut output, "WorkingDirectory", &input.working_directory);
    output.push_str("  <key>EnvironmentVariables</key>\n  <dict>\n");
    for (key, value) in &input.environment {
        plist_pair(&mut output, key, value);
    }
    output.push_str("  </dict>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>KeepAlive</key>\n  <dict>\n    <key>SuccessfulExit</key>\n    <false/>\n  </dict>\n</dict>\n</plist>\n");
    output
}

fn plist_pair(output: &mut String, key: &str, value: &str) {
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

fn service_arguments(input: &ServiceConfigInput) -> impl Iterator<Item = String> + '_ {
    std::iter::once("serve".to_owned())
        .chain(input.arguments.iter().cloned())
        .chain(
            input
                .allowed_roots
                .iter()
                .flat_map(|root| ["--allowed-root".to_owned(), root.clone()]),
        )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HookManifest {
    schema_version: &'static str,
    host: &'static str,
    hooks: Vec<HookEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HookEntry {
    event: &'static str,
    executable: String,
    arguments: Vec<String>,
    deadline_ms: u32,
    fail_closed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowsServiceManifest<'a> {
    schema_version: &'static str,
    service_name: &'a str,
    description: &'a str,
    executable: &'a str,
    arguments: Vec<String>,
    working_directory: &'a str,
    environment: &'a BTreeMap<String, String>,
    restart_delay_seconds: u32,
    user_scope_only: bool,
}

impl<'a> From<&'a ServiceConfigInput> for WindowsServiceManifest<'a> {
    fn from(input: &'a ServiceConfigInput) -> Self {
        Self {
            schema_version: "ae-sdd-windows-service/v1",
            service_name: &input.service_name,
            description: &input.description,
            executable: &input.executable,
            arguments: service_arguments(input).collect(),
            working_directory: &input.working_directory,
            environment: &input.environment,
            restart_delay_seconds: input.restart_delay_seconds,
            user_scope_only: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> ServiceConfigInput {
        ServiceConfigInput {
            service_name: "io.ae-sdd.daemon".to_owned(),
            description: "ae-sdd daemon".to_owned(),
            executable: "/opt/ae sdd/ae-sddd".to_owned(),
            arguments: vec!["--user".to_owned()],
            allowed_roots: vec!["/workspace/root".to_owned()],
            working_directory: "/tmp/ae-sdd".to_owned(),
            environment: BTreeMap::from([("RUST_LOG".to_owned(), "info".to_owned())]),
            restart_delay_seconds: 2,
        }
    }

    #[test]
    fn service_renderers_preserve_argument_boundaries() {
        let systemd = generate_service_config(ServiceTarget::SystemdUser, &service())
            .expect("systemd config");
        assert!(
            systemd
                .contents
                .contains("ExecStart= \"/opt/ae sdd/ae-sddd\"")
        );
        assert!(
            systemd
                .contents
                .contains("\"serve\" \"--user\" \"--allowed-root\" \"/workspace/root\"")
        );

        let launchd = generate_service_config(ServiceTarget::LaunchdAgent, &service())
            .expect("launchd config");
        assert!(
            launchd
                .contents
                .contains("<string>/opt/ae sdd/ae-sddd</string>")
        );
        assert!(launchd.contents.contains("<string>--allowed-root</string>"));
        assert!(
            launchd
                .contents
                .contains("<string>/workspace/root</string>")
        );

        let windows = generate_service_config(ServiceTarget::WindowsUser, &service())
            .expect("windows config");
        assert!(windows.contents.contains("\"userScopeOnly\": true"));
        assert!(windows.contents.contains("\"--allowed-root\""));
        assert!(windows.contents.contains("\"/workspace/root\""));
    }

    #[test]
    fn hook_config_is_direct_argv_and_fail_closed() {
        let generated = generate_hook_config(&HookConfigInput {
            host: HookHost::Codex,
            executable: "C:\\Program Files\\ae-sdd\\ae-sdd.exe".to_owned(),
            common_arguments: vec!["--json".to_owned()],
            events: vec![HookEvent::PreTool, HookEvent::Stop],
            deadline_ms: 250,
        })
        .expect("hook config");
        assert!(generated.contents.contains("\"failClosed\": true"));
        assert!(generated.contents.contains("\"pre_tool\""));
        assert!(generated.contents.contains("\"hook.pre_tool\""));
        assert!(generated.contents.contains("\"--request-json\""));
        assert!(!generated.contents.contains("shell"));
    }

    #[test]
    fn line_injection_is_rejected() {
        let mut input = service();
        input.executable = "/bin/true\nEnvironment=BAD=1".to_owned();
        assert!(matches!(
            generate_service_config(ServiceTarget::SystemdUser, &input),
            Err(ConfigError::InvalidField("executable"))
        ));
    }
}
