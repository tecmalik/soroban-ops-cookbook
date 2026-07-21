# Agent Policies

Ready-to-deploy Soroban policy contracts for scoping what an AI agent or
automated workload is allowed to do with a smart account, built on top of
[OpenZeppelin's `stellar-accounts`](https://crates.io/crates/stellar-accounts)
policy framework.

These are **not** a replacement for `stellar-accounts` — they're pre-written
policy implementations for the patterns people ask for most:

| Pattern | What it enforces |
|---|---|
| [`spend-limit/`](./spend-limit) | Max amount per transaction, per token. |
| [`rate-limit/`](./rate-limit) | Max number of transactions per rolling time window. |
| [`contract-allowlist/`](./contract-allowlist) | Agent may only invoke a pre-approved set of contract addresses/functions. |

Each policy implements the `Policy` trait from `stellar-accounts` and can be
attached to a smart account alongside other signers/policies.

## Usage

```bash
cd spend-limit
cargo test
soroban contract build
```

Then attach the built `.wasm` as a policy on your smart account per the
[`stellar-accounts` docs](https://docs.openzeppelin.com/stellar-contracts).
