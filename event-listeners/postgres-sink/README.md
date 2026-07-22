# Postgres Sink

Polls Soroban RPC `getEvents` for one contract and writes each event as a row
in Postgres, deduped by event id.

## Features

- **Persistent cursor** — ledger position is saved in a `sync_state` table so
  the service resumes where it left off after restarts.
- **Topic filtering** — optionally filter events by topic prefix via the
  `TOPIC_FILTER` env var.
- **Exponential backoff** — transient RPC failures are retried with exponential
  backoff before falling through to the next poll tick.
- **Upsert deduplication** — events are inserted with `ON CONFLICT DO NOTHING`
  so re-processing the same ledger range is safe.

## Run locally

```bash
docker compose up -d          # starts a local Postgres
export SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
export CONTRACT_ID=<your contract id>
export DATABASE_URL=postgres://sink:sink@localhost:5433/soroban_events
cargo run
```

## Configuration

| Env var | Required | Default | Description |
|---|---|---|---|
| `SOROBAN_RPC_URL` | yes | — | Soroban RPC endpoint |
| `CONTRACT_ID` | yes | — | Contract to watch |
| `DATABASE_URL` | yes | — | Postgres connection string |
| `POLL_INTERVAL_SECS` | no | `10` | Seconds between poll ticks |
| `START_LEDGER` | no | `0` | Initial ledger (only used when no persisted cursor exists) |
| `TOPIC_FILTER` | no | — | Comma-separated topic prefixes to include |
| `MAX_RETRIES` | no | `3` | Max RPC retries with backoff before sleeping |

## Schema

The service auto-creates two tables on startup:

**`soroban_events`** — one row per contract event:

| Column | Type | Notes |
|---|---|---|
| `id` | `TEXT PK` | Soroban event id |
| `contract_id` | `TEXT` | Indexed |
| `ledger` | `BIGINT` | Indexed |
| `ledger_closed_at` | `TIMESTAMPTZ` | |
| `topic` | `JSONB` | |
| `value` | `JSONB` | |
| `inserted_at` | `TIMESTAMPTZ` | Auto-set |

**`sync_state`** — cursor persistence:

| Column | Type | Notes |
|---|---|---|
| `key` | `TEXT PK` | `'last_ledger'` |
| `value` | `BIGINT` | Next ledger to poll |
| `updated_at` | `TIMESTAMPTZ` | Auto-set |

## Not verified against a live RPC endpoint yet

This was written against the documented `getEvents` JSON-RPC shape but
hasn't been run against a live testnet contract in this environment.
If you hit a response-shape mismatch, that's a good first PR.
