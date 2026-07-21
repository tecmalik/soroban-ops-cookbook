//! Polls Soroban RPC's `getEvents` for a single contract and writes each
//! event into a Postgres table. Meant as a starting point, not a production
//! streaming pipeline — see ../../event-listeners/README.md for when to
//! reach for a hosted indexer instead.
//!
//! Required env vars:
//!   SOROBAN_RPC_URL   e.g. https://soroban-testnet.stellar.org
//!   CONTRACT_ID       the contract to watch
//!   DATABASE_URL      postgres connection string
//!   POLL_INTERVAL_SECS  optional, defaults to 10

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<GetEventsResult>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GetEventsResult {
    events: Vec<SorobanEvent>,
    #[serde(rename = "latestLedger")]
    latest_ledger: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SorobanEvent {
    #[serde(rename = "contractId")]
    contract_id: String,
    id: String,
    #[serde(rename = "ledger")]
    ledger: u64,
    #[serde(rename = "ledgerClosedAt")]
    ledger_closed_at: String,
    topic: Vec<serde_json::Value>,
    value: serde_json::Value,
}

async fn ensure_schema(pool: &sqlx::PgPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS soroban_events (
            id TEXT PRIMARY KEY,
            contract_id TEXT NOT NULL,
            ledger BIGINT NOT NULL,
            ledger_closed_at TIMESTAMPTZ NOT NULL,
            topic JSONB NOT NULL,
            value JSONB NOT NULL,
            inserted_at TIMESTAMPTZ NOT NULL DEFAULT now()
        );
        CREATE INDEX IF NOT EXISTS idx_soroban_events_contract
            ON soroban_events (contract_id);
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create schema")?;
    Ok(())
}

async fn fetch_events(
    client: &reqwest::Client,
    rpc_url: &str,
    contract_id: &str,
    start_ledger: u64,
) -> Result<GetEventsResult> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getEvents",
        "params": {
            "startLedger": start_ledger,
            "filters": [{
                "type": "contract",
                "contractIds": [contract_id]
            }]
        }
    });

    let resp: RpcResponse = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .context("RPC request failed")?
        .json()
        .await
        .context("failed to parse RPC response")?;

    if let Some(err) = resp.error {
        anyhow::bail!("RPC error: {err}");
    }

    resp.result.context("RPC response had no result")
}

async fn insert_events(pool: &sqlx::PgPool, events: &[SorobanEvent]) -> Result<()> {
    for event in events {
        sqlx::query(
            r#"
            INSERT INTO soroban_events (id, contract_id, ledger, ledger_closed_at, topic, value)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(&event.id)
        .bind(&event.contract_id)
        .bind(event.ledger as i64)
        .bind(
            chrono_from_rfc3339(&event.ledger_closed_at)
                .unwrap_or_else(|_| chrono::Utc::now()),
        )
        .bind(serde_json::to_value(&event.topic)?)
        .bind(&event.value)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn chrono_from_rfc3339(s: &str) -> Result<chrono::DateTime<chrono::Utc>, chrono::ParseError> {
    chrono::DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&chrono::Utc))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let rpc_url = env::var("SOROBAN_RPC_URL").context("SOROBAN_RPC_URL not set")?;
    let contract_id = env::var("CONTRACT_ID").context("CONTRACT_ID not set")?;
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    let poll_interval: u64 = env::var("POLL_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("failed to connect to Postgres")?;

    ensure_schema(&pool).await?;

    let client = reqwest::Client::new();
    // NOTE: in production, persist this cursor (e.g. in a `sync_state`
    // table) instead of starting from a fixed ledger on every restart.
    let mut start_ledger: u64 = env::var("START_LEDGER")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    tracing::info!(%contract_id, %rpc_url, start_ledger, "starting event poll loop");

    loop {
        match fetch_events(&client, &rpc_url, &contract_id, start_ledger).await {
            Ok(result) => {
                if !result.events.is_empty() {
                    tracing::info!(count = result.events.len(), "fetched events");
                    if let Err(e) = insert_events(&pool, &result.events).await {
                        tracing::error!(error = %e, "failed to insert events");
                    }
                }
                start_ledger = result.latest_ledger + 1;
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to fetch events, will retry");
            }
        }

        tokio::time::sleep(Duration::from_secs(poll_interval)).await;
    }
}
