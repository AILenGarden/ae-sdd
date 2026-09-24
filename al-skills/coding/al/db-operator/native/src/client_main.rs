use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use db_operator_native::client_config::ClientConfig;
use db_operator_native::config::CLIENT_REQUEST_TIMEOUT_SECONDS;
use db_operator_native::ipc::send_request_to;
use db_operator_native::protocol::{Operation, PROTOCOL_VERSION, Request, Response};
use serde_json::Value;

#[derive(Debug, Parser)]
#[command(
    name = "db-operator",
    version,
    about = "Query-only client for the DB Operator security daemon"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Execute exactly one read-only SQL statement through the daemon.
    Query {
        sql: String,
        #[arg(long)]
        connection: Option<String>,
    },
    /// Show daemon status.
    Status,
    /// Read cached schema metadata.
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SchemaCommand {
    /// Return cached schema metadata.
    Get {
        #[arg(long)]
        connection: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("ERROR: {error:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let config = ClientConfig::load_default()?;
    let operation = match cli.command {
        Command::Query { sql, connection } => Operation::Query {
            connection: connection.unwrap_or_else(|| config.connection.clone()),
            sql,
            write_level: config.write_level,
        },
        Command::Status => Operation::Status,
        Command::Schema {
            command: SchemaCommand::Get { connection },
        } => Operation::SchemaGet {
            connection: connection.unwrap_or_else(|| config.connection.clone()),
        },
    };
    let response = send_request_to(
        &config.endpoint,
        &request(&config.capability, operation),
        Duration::from_secs(CLIENT_REQUEST_TIMEOUT_SECONDS),
    )
    .await?;
    print_response(response)
}

fn request(capability: &str, operation: Operation) -> Request {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Request {
        version: PROTOCOL_VERSION,
        request_id: format!("{}-{timestamp}", std::process::id()),
        capability: capability.to_owned(),
        operation,
    }
}

fn print_response(response: Response) -> Result<()> {
    if !response.ok {
        let error = response
            .error
            .map(|error| format!("{}: {}", error.code, error.message))
            .unwrap_or_else(|| "daemon returned an unknown error".to_owned());
        bail!(error);
    }
    let value = response.result.unwrap_or(Value::Null);
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
