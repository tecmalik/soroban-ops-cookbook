//! Polls Soroban RPC's `getEvents` for a single contract and writes each
//! event into a Postgres table. Meant as a starting point, not a production
//! streaming pipeline — see ../../event-listeners/README.md for when to
//! reach for a hosted indexer instead.
//!
//! Required env vars:
//!   SOROBAN_RPC_URL       e.g. https://soroban-testnet.stellar.org
//!   CONTRACT_ID           the contract to watch
//!   DATABASE_URL          postgres connection string
//!   POLL_INTERVAL_SECS    optional, defaults to 10
//!   START_LEDGER          optional, defaults to 0 (only used on first run)
//!   TOPIC_FILTER          optional, comma-separated topic prefixes to include
//!   MAX_RETRIES           optional, max RPC fetch retries before sleeping, defaults to 3

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
        CREATE INDEX IF NOT EXISTS idx_soroban_events_ledger
            ON soroban_events (ledger);

        CREATE TABLE IF NOT EXISTS sync_state (
            key TEXT PRIMARY KEY,
            value BIGINT NOT NULL,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
        );
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create schema")?;
    Ok(())
}

/// Load the persisted ledger cursor from the sync_state table.
/// Returns None if no cursor has been saved yet.
async fn load_cursor(pool: &sqlx::PgPool) -> Result<Option<u64>> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT value FROM sync_state WHERE key = 'last_ledger'",
    )
    .fetch_optional(pool)
    .await
    .context("failed to load cursor")?;

    Ok(row.map(|(v,)| v as u64))
}

/// Persist the ledger cursor so we resume from this point on restart.
async fn save_cursor(pool: &sqlx::PgPool, ledger: u64) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO sync_state (key, value, updated_at)
        VALUES ('last_ledger', $1, now())
        ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = now()
        "#,
    )
    .bind(ledger as i64)
    .execute(pool)
    .await
    .context("failed to save cursor")?;
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

/// Check if an event matches the topic filter (if configured).
fn matches_topic_filter(event: &SorobanEvent, topic_filter: &Option<Vec<String>>) -> bool {
    match topic_filter {
        None => true,
        Some(filters) => {
            if filters.is_empty() {
                return true;
            }
            for topic_val in &event.topic {
                let topic_str = topic_val.to_string();
                for filter in filters {
                    if topic_str.contains(filter) {
                        return true;
                    }
                }
            }
            false
        }
    }
}

/// Fetch events with exponential backoff on transient failures.
async fn fetch_events_with_retry(
    client: &reqwest::Client,
    rpc_url: &str,
    contract_id: &str,
    start_ledger: u64,
    max_retries: u32,
) -> Result<GetEventsResult> {
    let mut attempt = 0u32;
    loop {
        match fetch_events(client, rpc_url, contract_id, start_ledger).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempt += 1;
                if attempt > max_retries {
                    return Err(e);
                }
                let backoff = Duration::from_secs(1u64 << attempt.min(5));
                tracing::warn!(
                    error = %e,
                    attempt,
                    backoff_secs = backoff.as_secs(),
                    "RPC fetch failed, retrying with backoff"
                );
                tokio::time::sleep(backoff).await;
            }
        }
    }
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
    let max_retries: u32 = env::var("MAX_RETRIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    let topic_filter: Option<Vec<String>> = env::var("TOPIC_FILTER").ok().map(|v| {
        v.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("failed to connect to Postgres")?;

    ensure_schema(&pool).await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")?;

    // Resume from persisted cursor, fall back to START_LEDGER env var,
    // then to 0.
    let mut start_ledger: u64 = match load_cursor(&pool).await? {
        Some(cursor) => {
            tracing::info!(cursor, "resuming from persisted cursor");
            cursor
        }
        None => {
            let initial: u64 = env::var("START_LEDGER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            tracing::info!(initial, "no persisted cursor, starting from START_LEDGER");
            initial
        }
    };

    tracing::info!(%contract_id, %rpc_url, start_ledger, "starting event poll loop");

    loop {
        match fetch_events_with_retry(
            &client,
            &rpc_url,
            &contract_id,
            start_ledger,
            max_retries,
        )
        .await
        {
            Ok(result) => {
                let filtered: Vec<&SorobanEvent> = result
                    .events
                    .iter()
                    .filter(|e| matches_topic_filter(e, &topic_filter))
                    .collect();

                if !filtered.is_empty() {
                    tracing::info!(
                        total = result.events.len(),
                        matched = filtered.len(),
                        "fetched events"
                    );
                    let owned: Vec<SorobanEvent> =
                        filtered.into_iter().cloned().collect();
                    if let Err(e) = insert_events(&pool, &owned).await {
                        tracing::error!(error = %e, "failed to insert events");
                    }
                }

                start_ledger = result.latest_ledger + 1;

                // Persist cursor after successful processing.
                if let Err(e) = save_cursor(&pool, start_ledger).await {
                    tracing::error!(error = %e, "failed to persist cursor");
                }
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "failed to fetch events after retries, will retry next tick"
                );
            }
        }

        tokio::time::sleep(Duration::from_secs(poll_interval)).await;
    }
}
