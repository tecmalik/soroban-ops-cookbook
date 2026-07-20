# Soroban Ops Cookbook

Small, reusable patterns for the *operational glue* around Soroban apps —
the stuff every team ends up writing once, badly, before finding a better way.

This is **not** a general "learn Soroban" cookbook (see
[`stellar/soroban-examples`](https://github.com/stellar/soroban-examples) or
[Soroban-Cookbook](https://github.com/Soroban-Cookbook/Soroban-Cookbook-) for that).
It's a set of narrow, composable snippets for three recurring integration problems:

| Folder | Problem it solves |
|---|---|
| [`agent-policies/`](./agent-policies) | Ready-to-deploy Soroban smart-account policy contracts (spend limits, rate limits, contract allowlists) for AI agents and automated workloads, built on [OpenZeppelin's `stellar-accounts`](https://crates.io/crates/stellar-accounts) framework. |
| [`anchor-configs/`](./anchor-configs) | A config generator that produces valid `docker-compose.yml` / `.env` files for SDF's [Anchor Platform](https://developers.stellar.org/docs/platforms/anchor-platform) from a short interview, instead of hand-writing SEP-24/SEP-31 config from docs. |
| [`event-listeners/`](./event-listeners) | Minimal, self-hosted services that subscribe to Soroban contract events and sink them into Postgres or forward them as webhooks — for teams that don't want a hosted indexer. |

## Why this exists

Each of these problems already has a "real" solution (OpenZeppelin's policy
framework, SDF's Anchor Platform, Mercury/BlockEden's hosted indexers). This
repo doesn't replace any of them — it packages the small, repetitive
integration work *around* them that nobody's written down yet.

## Status

Early / actively growing. Contributions welcome — see [`CONTRIBUTING.md`](./CONTRIBUTING.md)
and the [open issues](../../issues).

## License

MIT — see [`LICENSE`](./LICENSE).
