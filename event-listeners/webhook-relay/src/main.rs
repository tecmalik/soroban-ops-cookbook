//! Polls Soroban RPC's `getEvents` for a single contract and forwards each
//! event as a JSON webhook POST to a configured URL with exponential
//! backoff/retry.
//!
//! This is the webhook counterpart to `postgres-sink` — same polling loop,
//! different output sink. See ../../event-listeners/README.md for when to
//! reach for a hosted indexer instead.
//!
//! Required env vars:
//!   SOROBAN_RPC_URL      e.g. https://soroban-testnet.stellar.org
//!   CONTRACT_ID          the contract to watch
//!   WEBHOOK_URL          where to POST events
//!   POLL_INTERVAL_SECS   optional, defaults to 10
//!   START_LEDGER         optional, defaults to 0
//!   MAX_RETRIES          optional, defaults to 5
//!   TOPIC_FILTER         optional, comma-separated topic prefixes to include

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
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

/// Payload sent to the webhook URL for each event.
#[derive(Debug, Serialize)]
struct WebhookPayload {
    event_id: String,
    contract_id: String,
    ledger: u64,
    ledger_closed_at: String,
    topic: Vec<serde_json::Value>,
    value: serde_json::Value,
    delivered_at: String,
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

/// Send a single event to the webhook URL with exponential backoff.
async fn deliver_webhook(
    client: &reqwest::Client,
    webhook_url: &str,
    event: &SorobanEvent,
    max_retries: u32,
) -> Result<()> {
    let payload = WebhookPayload {
        event_id: event.id.clone(),
        contract_id: event.contract_id.clone(),
        ledger: event.ledger,
        ledger_closed_at: event.ledger_closed_at.clone(),
        topic: event.topic.clone(),
        value: event.value.clone(),
        delivered_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut attempt = 0u32;
    loop {
        match client
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .header("X-Soroban-Event-Id", &event.id)
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::debug!(event_id = %event.id, "webhook delivered");
                return Ok(());
            }
            Ok(resp) => {
                let status = resp.status();
                attempt += 1;
                if attempt > max_retries {
                    anyhow::bail!(
                        "webhook delivery failed after {max_retries} retries: HTTP {status}"
                    );
                }
                tracing::warn!(
                    event_id = %event.id,
                    %status,
                    attempt,
                    "webhook delivery failed, retrying"
                );
            }
            Err(e) => {
                attempt += 1;
                if attempt > max_retries {
                    anyhow::bail!(
                        "webhook delivery failed after {max_retries} retries: {e}"
                    );
                }
                tracing::warn!(
                    event_id = %event.id,
                    error = %e,
                    attempt,
                    "webhook request failed, retrying"
                );
            }
        }

        // Exponential backoff: 1s, 2s, 4s, 8s, 16s, ...
        let backoff = Duration::from_secs(1u64 << attempt.min(5));
        tokio::time::sleep(backoff).await;
    }
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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let rpc_url = env::var("SOROBAN_RPC_URL").context("SOROBAN_RPC_URL not set")?;
    let contract_id = env::var("CONTRACT_ID").context("CONTRACT_ID not set")?;
    let webhook_url = env::var("WEBHOOK_URL").context("WEBHOOK_URL not set")?;
    let poll_interval: u64 = env::var("POLL_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let max_retries: u32 = env::var("MAX_RETRIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    let topic_filter: Option<Vec<String>> = env::var("TOPIC_FILTER").ok().map(|v| {
        v.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")?;

    let mut start_ledger: u64 = env::var("START_LEDGER")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    tracing::info!(
        %contract_id, %rpc_url, %webhook_url, start_ledger,
        "starting webhook relay poll loop"
    );

    loop {
        match fetch_events(&client, &rpc_url, &contract_id, start_ledger).await {
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
                }

                for event in &filtered {
                    if let Err(e) =
                        deliver_webhook(&client, &webhook_url, event, max_retries).await
                    {
                        tracing::error!(
                            event_id = %event.id,
                            error = %e,
                            "failed to deliver webhook, skipping event"
                        );
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
