use std::io::{self, Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use db_operator_native::cache::SchemaCache;
use db_operator_native::database::{PoolHandle, profile_fingerprint, read_password};
use db_operator_native::database::{delete_password, execute_direct, store_password};
use db_operator_native::registry::{
    ConnectionProfile, Endpoint, Engine, Limits, Registry, SslProfile, WriteLevel, registry_path,
    validate_alias,
};
use db_operator_native::security::issue_client_capability;
use serde_json::{Value, json};
use url::Url;
use zeroize::Zeroizing;

#[derive(Debug, Parser)]
#[command(
    name = "db-operator-admin",
    version,
    about = "Strictly read-only database CLI and local daemon"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Trusted local registrar JSON protocol (stdin only).
    Manage,
    /// Register or update a local connection profile.
    Register(Box<RegisterArgs>),
    /// List redacted connection metadata.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Select the default connection.
    Use { name: String },
    /// Test a registered connection with SELECT 1.
    Test { name: Option<String> },
    /// Remove a connection profile and its credential.
    Remove {
        name: String,
        #[arg(long)]
        yes: bool,
    },
    /// Inspect local configuration and native runtime status.
    Doctor,
    /// Issue an Agent query capability and redacted client configuration.
    Client {
        #[command(subcommand)]
        command: ClientCommand,
    },
    /// Refresh cached database metadata through the trusted administration path.
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ClientCommand {
    /// Replace the daemon query capability and write an Agent client configuration.
    Issue {
        #[arg(long)]
        connection: String,
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value = "none")]
        write_level: WriteLevelArg,
    },
}

#[derive(Debug, Subcommand)]
enum SchemaCommand {
    /// Refresh schema metadata from the selected database.
    Refresh,
}

#[derive(Debug, Args)]
struct RegisterArgs {
    #[arg(long)]
    name: Option<String>,
    #[arg(long, value_enum)]
    engine: Option<EngineArg>,
    #[arg(long, conflicts_with = "url")]
    host: Option<String>,
    #[arg(long, conflicts_with = "host")]
    url: Option<String>,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long, conflicts_with = "no_default_database")]
    database: Option<String>,
    #[arg(long)]
    no_default_database: bool,
    #[arg(long)]
    username: Option<String>,
    #[arg(long, value_enum)]
    ssl_mode: Option<SslModeArg>,
    #[arg(long)]
    ssl_ca: Option<String>,
    #[arg(long)]
    ssl_cert: Option<String>,
    #[arg(long)]
    ssl_key: Option<String>,
    #[arg(long, default_value = "unknown")]
    environment: String,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long)]
    connect_timeout: Option<u64>,
    #[arg(long, default_value_t = 30)]
    statement_timeout: u64,
    #[arg(long, default_value_t = 500)]
    max_rows: usize,
    #[arg(long)]
    password_stdin: bool,
    #[arg(long)]
    overwrite: bool,
    #[arg(long)]
    set_default: bool,
    #[arg(long)]
    test: bool,
    #[arg(long, value_enum, default_value = "none")]
    write_level: WriteLevelArg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum WriteLevelArg {
    None,
    Dml,
    Ddl,
}

impl From<WriteLevelArg> for WriteLevel {
    fn from(value: WriteLevelArg) -> Self {
        match value {
            WriteLevelArg::None => Self::None,
            WriteLevelArg::Dml => Self::Dml,
            WriteLevelArg::Ddl => Self::Ddl,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EngineArg {
    Mysql,
    Postgresql,
    Postgres,
}

impl From<EngineArg> for Engine {
    fn from(value: EngineArg) -> Self {
        match value {
            EngineArg::Mysql => Self::Mysql,
            EngineArg::Postgresql | EngineArg::Postgres => Self::Postgresql,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SslModeArg {
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl SslModeArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("ERROR: {error:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Register(args) => register_command(*args).await,
        Command::Manage => {
            let mut body = Zeroizing::new(String::new());
            io::stdin().take(65537).read_to_string(&mut body)?;
            if body.len() > 65536 { bail!("request too large"); }
            let input = serde_json::from_str(&body).context("invalid management request")?;
            print_json(&db_operator_native::management::execute(&db_operator_native::registry::config_dir(), input).await?)
        }
        Command::List { json } => list_command(json),
        Command::Use { name } => use_command(&name),
        Command::Test { name } => test_command(name.as_deref()).await,
        Command::Remove { name, yes } => remove_command(&name, yes),
        Command::Doctor => doctor_command().await,
        Command::Client { command } => client_command(command),
        Command::Schema { command } => schema_command(command).await,
    }
}

fn client_command(command: ClientCommand) -> Result<()> {
    match command {
        ClientCommand::Issue {
            connection,
            endpoint,
            output,
            write_level,
        } => {
            let registry = Registry::load_default()?;
            let (connection, _) = registry.resolve(Some(&connection))?;
            issue_client_capability(
                &db_operator_native::registry::config_dir(),
                &output,
                &endpoint,
                connection,
                write_level.into(),
            )?;
            print_json(&json!({
                "connection": connection,
                "endpoint": endpoint,
                "client_config": output,
                "issued": true
            }))
        }
    }
}

async fn register_command(args: RegisterArgs) -> Result<()> {
    let mut url_fields = args
        .url
        .as_deref()
        .map(parse_registration_url)
        .transpose()?;
    let alias = required_or_prompt(args.name, "Connection name")?;
    validate_alias(&alias)?;
    let engine = match args
        .engine
        .map(Into::into)
        .or_else(|| url_fields.as_ref().map(|fields| fields.engine))
    {
        Some(engine) => engine,
        None => prompt_engine()?,
    };
    let host = match args
        .host
        .or_else(|| url_fields.as_mut().and_then(|fields| fields.host.take()))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        Some(host) => host,
        None => prompt("Host")?,
    };
    let port = args
        .port
        .or_else(|| url_fields.as_ref().map(|fields| fields.port))
        .unwrap_or(match engine {
            Engine::Mysql => 3306,
            Engine::Postgresql => 5432,
        });
    let username = match args
        .username
        .or_else(|| {
            url_fields
                .as_mut()
                .and_then(|fields| fields.username.take())
        })
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        Some(username) => username,
        None => prompt("Username")?,
    };
    let mut database = if args.no_default_database {
        None
    } else {
        args.database.or_else(|| {
            url_fields
                .as_mut()
                .and_then(|fields| fields.database.take())
        })
    };
    if database.is_none() && engine == Engine::Postgresql {
        database = Some(prompt("Database")?);
    } else if database.is_none()
        && engine == Engine::Mysql
        && args.url.is_none()
        && !args.no_default_database
    {
        database = prompt_optional("Database (optional for MySQL)")?;
    }
    let ssl_mode = args
        .ssl_mode
        .map(SslModeArg::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            url_fields
                .as_ref()
                .and_then(|fields| fields.ssl_mode.clone())
        })
        .unwrap_or_else(|| "prefer".to_owned())
        .to_owned();
    let connect_timeout = args
        .connect_timeout
        .or_else(|| {
            url_fields
                .as_ref()
                .and_then(|fields| fields.connect_timeout)
        })
        .unwrap_or(10);
    let profile = ConnectionProfile {
        engine,
        endpoint: Endpoint {
            mode: "tcp".to_owned(),
            host,
            port,
        },
        database,
        username,
        secret_ref: format!("vault://db-operator/{alias}"),
        ssl: SslProfile {
            mode: ssl_mode,
            ca_file: args.ssl_ca,
            client_cert: args.ssl_cert,
            client_key: args.ssl_key,
        },
        environment: args.environment,
        tags: args.tags,
        limits: Limits {
            connect_timeout_seconds: connect_timeout,
            statement_timeout_seconds: args.statement_timeout,
            max_rows: args.max_rows,
        },
        write_level: args.write_level.into(),
    };
    let password = Zeroizing::new(if args.password_stdin {
        read_password_stdin()?
    } else {
        rpassword::prompt_password("Password: ")?
    });
    if password.is_empty() {
        bail!("password must not be empty");
    }

    let mut registry = Registry::load_default_or_empty()?;
    if registry.connections.contains_key(&alias) && !args.overwrite {
        bail!("connection {alias:?} is already registered; use --overwrite to replace it");
    }
    if args.test {
        let pool =
            db_operator_native::database::PoolHandle::connect(&profile, password.as_str()).await?;
        let test_result = pool
            .execute(
                &alias,
                &profile,
                "SELECT 1 AS db_operator_test",
                password.as_str(),
                WriteLevel::None,
            )
            .await;
        pool.close().await;
        test_result?;
    }

    store_password(&alias, password.as_str())?;
    let configured_write_level = profile.write_level;
    registry.connections.insert(alias.clone(), profile);
    if args.set_default || registry.default.is_none() {
        registry.default = Some(alias.clone());
    }
    if let Err(error) = registry.save_default() {
        let _ = delete_password(&alias);
        return Err(error.into());
    }
    print_json(&json!({
        "connection": alias,
        "registered": true,
        "default": registry.default,
        "write_level": configured_write_level
    }))
}

fn list_command(as_json: bool) -> Result<()> {
    let registry = Registry::load_default_or_empty()?;
    let connections: Vec<Value> = registry
        .connections
        .iter()
        .map(|(alias, profile)| {
            json!({
                "name": alias,
                "engine": profile.engine.as_str(),
                "database": profile.database,
                "environment": profile.environment,
                "tags": profile.tags,
                "default": registry.default.as_deref() == Some(alias),
                "write_level": profile.write_level
            })
        })
        .collect();
    if as_json {
        print_json(&json!({"connections": connections}))
    } else {
        for connection in connections {
            println!(
                "{}\t{}\t{}{}",
                connection["name"].as_str().unwrap_or_default(),
                connection["engine"].as_str().unwrap_or_default(),
                connection["database"].as_str().unwrap_or("<server>"),
                if connection["default"] == true {
                    "\t(default)"
                } else {
                    ""
                }
            );
        }
        Ok(())
    }
}

fn use_command(name: &str) -> Result<()> {
    let mut registry = Registry::load_default()?;
    let (alias, _) = registry.resolve(Some(name))?;
    let alias = alias.to_owned();
    registry.default = Some(alias.clone());
    registry.save_default()?;
    println!("Selected default connection: {alias}");
    Ok(())
}

async fn test_command(name: Option<&str>) -> Result<()> {
    let registry = Registry::load_default()?;
    let (alias, profile) = registry.resolve(name)?;
    let result = execute_direct(alias, profile, "SELECT 1 AS db_operator_test").await?;
    print_json(&json!({
        "connection": alias,
        "database": profile.database,
        "ok": result.row_count == 1,
        "read_only": true,
        "runtime": {"mode": "direct"}
    }))
}

fn remove_command(name: &str, yes: bool) -> Result<()> {
    let mut registry = Registry::load_default()?;
    let (alias, _) = registry.resolve(Some(name))?;
    let alias = alias.to_owned();
    if !yes && !confirm(&format!("Remove connection {alias:?}?"))? {
        bail!("removal cancelled");
    }
    registry.connections.remove(&alias);
    if registry.default.as_deref() == Some(alias.as_str()) {
        registry.default = registry.connections.keys().next().cloned();
    }
    registry.save_default()?;
    delete_password(&alias)?;
    print_json(&json!({"connection": alias, "removed": true}))
}

async fn doctor_command() -> Result<()> {
    let registry = Registry::load_default_or_empty()?;
    print_json(&json!({
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "registry": registry_path(),
        "connection_count": registry.connections.len(),
        "default": registry.default,
        "native": true,
        "python_required": false,
        "ok": true
    }))
}

async fn schema_command(command: SchemaCommand) -> Result<()> {
    match command {
        SchemaCommand::Refresh => {
            let registry = Registry::load_default()?;
            let (alias, profile) = registry.resolve(None)?;
            let password = read_password(alias)?;
            let pool = PoolHandle::connect(profile, password.as_str()).await?;
            let snapshot = pool.schema_snapshot(profile, password.as_str()).await;
            pool.close().await;
            let objects = snapshot?;
            let fingerprint = profile_fingerprint(profile)?;
            let cache = SchemaCache::open(
                &db_operator_native::registry::config_dir().join("schema-cache.sqlite3"),
            )?;
            cache.replace_snapshot(alias, &fingerprint, &objects)?;
            print_json(&json!({
                "connection": alias,
                "objects": objects.len(),
                "refreshed": true
            }))
        }
    }
}

fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn required_or_prompt(value: Option<String>, label: &str) -> Result<String> {
    match value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        Some(value) => Ok(value),
        None => prompt(label),
    }
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    let value = value.trim().to_owned();
    if value.is_empty() {
        bail!("{label} must not be empty");
    }
    Ok(value)
}

fn prompt_optional(label: &str) -> Result<Option<String>> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    let value = value.trim().to_owned();
    Ok((!value.is_empty()).then_some(value))
}

fn prompt_engine() -> Result<Engine> {
    let value = prompt("Engine (mysql/postgresql)")?;
    match value.to_ascii_lowercase().as_str() {
        "mysql" => Ok(Engine::Mysql),
        "postgres" | "postgresql" => Ok(Engine::Postgresql),
        _ => bail!("engine must be mysql or postgresql"),
    }
}

fn confirm(message: &str) -> Result<bool> {
    print!("{message} [y/N]: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn read_password_stdin() -> Result<String> {
    let mut bytes = Vec::new();
    io::stdin().read_to_end(&mut bytes)?;
    let decoded = if bytes.starts_with(&[0xff, 0xfe]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16(&units)?
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16(&units)?
    } else {
        String::from_utf8(
            bytes
                .strip_prefix(&[0xef, 0xbb, 0xbf])
                .unwrap_or(&bytes)
                .to_vec(),
        )?
    };
    Ok(decoded.trim_end_matches(['\r', '\n']).to_owned())
}

struct UrlFields {
    engine: Engine,
    host: Option<String>,
    port: u16,
    database: Option<String>,
    username: Option<String>,
    ssl_mode: Option<String>,
    connect_timeout: Option<u64>,
}

fn parse_registration_url(input: &str) -> Result<UrlFields> {
    let url = Url::parse(input).context("invalid connection URL")?;
    if url.password().is_some() {
        bail!("do not include a password in the connection URL");
    }
    let mut ssl_mode = None;
    let mut connect_timeout = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "sslmode" => {
                if !matches!(
                    value.as_ref(),
                    "disable" | "prefer" | "require" | "verify-ca" | "verify-full"
                ) {
                    bail!("connection URL contains unsupported sslmode {value:?}");
                }
                ssl_mode = Some(value.into_owned());
            }
            "connect_timeout" => {
                connect_timeout = Some(
                    value
                        .parse::<u64>()
                        .context("connection URL connect_timeout must be an integer")?,
                );
            }
            _ => bail!("connection URL contains unsupported parameter {key:?}"),
        }
    }
    let engine = match url.scheme().split('+').next().unwrap_or_default() {
        "mysql" => Engine::Mysql,
        "postgres" | "postgresql" => Engine::Postgresql,
        _ => bail!("connection URL scheme must be mysql or postgresql"),
    };
    let host = url.host_str().map(ToOwned::to_owned);
    if host.is_none() {
        bail!("connection URL must include a host");
    }
    let database = url
        .path_segments()
        .and_then(|mut segments| segments.next())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if engine == Engine::Postgresql && database.is_none() {
        bail!("PostgreSQL connection URL must include a database");
    }
    Ok(UrlFields {
        engine,
        host,
        port: url.port().unwrap_or(match engine {
            Engine::Mysql => 3306,
            Engine::Postgresql => 5432,
        }),
        database,
        username: (!url.username().is_empty()).then(|| url.username().to_owned()),
        ssl_mode,
        connect_timeout,
    })
}
