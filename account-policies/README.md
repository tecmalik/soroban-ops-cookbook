# Account Policies

Ready-to-deploy Soroban policy contracts for scoping what a delegated signer or
automated workload is allowed to do with a smart account, built on top of
[OpenZeppelin's `stellar-accounts`](https://crates.io/crates/stellar-accounts)
policy framework.

These are **not** a replacement for `stellar-accounts` — they're pre-written
policy implementations for the patterns people ask for most:

| Pattern | What it enforces |
|---|---|
| [`spend-limit/`](./spend-limit) | Max amount per transaction, per token. |
| [`rate-limit/`](./rate-limit) | Max number of transactions per rolling time window. |
| [`contract-allowlist/`](./contract-allowlist) | Signer may only invoke a pre-approved set of contract addresses/functions. |

Each policy implements the `Policy` trait from `stellar-accounts` and can be
attached to a smart account alongside other signers/policies.

## Usage

```bash
cd spend-limit       # or rate-limit / contract-allowlist
cargo test
soroban contract build
```

Then attach the built `.wasm` as a policy on your smart account per the
[`stellar-accounts` docs](https://docs.openzeppelin.com/stellar-contracts).

## Policy summaries

### spend-limit

Sets a per-token cap on the maximum amount that can move in a single
transaction. Useful for scoping a delegated signer to small transfers without
giving it access to the account's full balance.

### rate-limit

Limits the number of transactions a signer can execute within a rolling
window of ledger sequences. For example, "at most 5 calls per ~1 hour
(720 ledgers)." Uses ledger sequence numbers as the clock since they're
the only monotonic timestamp available inside a Soroban contract.

### contract-allowlist

Restricts a signer to only calling specific contracts, and optionally
specific functions on those contracts. For example, "this signer may
only call `swap` on the DEX contract and `deposit` on the vault."
