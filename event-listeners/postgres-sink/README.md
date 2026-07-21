# Postgres Sink

Polls Soroban RPC `getEvents` for one contract and writes each event as a row
in Postgres, deduped by event id.

## Run locally

```bash
docker compose up -d          # starts a local Postgres
export SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
export CONTRACT_ID=<your contract id>
export DATABASE_URL=postgres://sink:sink@localhost:5433/soroban_events
cargo run
```

## Known limitations (good first issues!)

- [ ] Ledger cursor is not persisted across restarts — it always starts from
      `START_LEDGER` (default 0), which will hit Soroban RPC's retention
      window limit on a real network. Persist the cursor in a
      `sync_state` table.
- [ ] No topic filtering — currently pulls all events for the contract.
      Add a `TOPIC_FILTER` env var.
- [ ] No backoff/retry policy beyond "sleep and try again next tick."
- [ ] `value` and `topic` are stored as raw JSON (still XDR-shaped in
      places) rather than decoded into native types — a schema mapper for
      common types (Address, i128, Symbol) would make this much more
      useful downstream.

## Not verified against a live RPC endpoint yet

This was written against the documented `getEvents` JSON-RPC shape but
hasn't been run against a live testnet contract in this environment.
If you hit a response-shape mismatch, that's a good first PR.
