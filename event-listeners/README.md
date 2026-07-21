# Event Listeners

Minimal, self-hosted services for getting Soroban contract events into your
own database — for teams who don't want (or can't yet justify) a hosted
indexer like Mercury or BlockEden.

| Pattern | What it does |
|---|---|
| [`postgres-sink/`](./postgres-sink) | Polls Soroban RPC for events from a given contract and writes typed rows into Postgres. |
| [`webhook-relay/`](./webhook-relay) | Same polling loop, but forwards each event as a JSON webhook instead of writing to a DB. |

These are intentionally simple polling loops, not a distributed
streaming pipeline — they're meant to be the thing you run before you need
Kafka, not instead of it forever.

## When to use these vs. a hosted indexer

Use these if: single contract (or a handful), you already run Postgres,
you want full control of the schema, or you're prototyping and don't want
to sign up for a hosted service yet.

Use a hosted indexer (Mercury, BlockEden, SubQuery) if: you need
multi-contract, historical backfill at scale, or a GraphQL API out of the
box.
