use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use db_operator_native::client_config::ClientConfig;
use db_operator_native::ipc::send_request_to;
use db_operator_native::protocol::{Operation, PROTOCOL_VERSION, Request};
use serde_json::json;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ERROR: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let samples = std::env::args()
        .nth(1)
        .map(|value| value.parse::<usize>())
        .transpose()
        .context("sample count must be an integer")?
        .unwrap_or(500);
    if samples < 20 {
        bail!("sample count must be at least 20");
    }

    let config = ClientConfig::load_default()?;
    for index in 0..20 {
        status(&config, format!("warmup-{index}")).await?;
    }

    let mut micros = Vec::with_capacity(samples);
    for index in 0..samples {
        let started = Instant::now();
        status(&config, format!("bench-{index}")).await?;
        micros.push(started.elapsed().as_secs_f64() * 1_000_000.0);
    }
    micros.sort_by(f64::total_cmp);
    let p50 = percentile(&micros, 0.50) / 1_000.0;
    let p95 = percentile(&micros, 0.95) / 1_000.0;
    println!(
        "{}",
        serde_json::to_string(&json!({
            "samples": samples,
            "warm_ipc_p50_ms": round_millis(p50),
            "warm_ipc_p95_ms": round_millis(p95),
        }))?
    );
    Ok(())
}

async fn status(config: &ClientConfig, request_id: String) -> Result<()> {
    let response = send_request_to(
        &config.endpoint,
        &Request {
            version: PROTOCOL_VERSION,
            request_id: format!("{}-{request_id}", unique_prefix()),
            capability: config.capability.clone(),
            operation: Operation::Status,
        },
        Duration::from_secs(2),
    )
    .await?;
    if !response.ok {
        bail!("daemon rejected benchmark request");
    }
    Ok(())
}

fn unique_prefix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * percentile).floor() as usize]
}

fn round_millis(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}
