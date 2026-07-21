# Contributing

This repo is structured as small, independent snippets rather than one
monolithic project. Each subfolder is meant to be copy-pasteable into a real
project with minimal changes.

## Adding a new pattern

1. Pick a folder (`agent-policies/`, `anchor-configs/`, or `event-listeners/`)
   or propose a new top-level category if it's a genuinely new class of
   problem (open an issue first).
2. Each pattern gets its own subfolder with:
   - A short `README.md`: what problem it solves, what it depends on, how to
     run it.
   - Working code — it must build/run, not just illustrate an idea.
   - At least one example config or test showing it actually works.
3. Keep dependencies minimal. If you're wrapping an existing library
   (OpenZeppelin's `stellar-accounts`, Anchor Platform, Mercury), depend on
   it — don't reimplement it.

## Style

- Rust: `cargo fmt` + `cargo clippy` clean.
- Node/TS: `prettier` defaults.
- Every snippet needs a README explaining *why* it exists in one paragraph,
  not just usage instructions.

## Issue labels

- `good-first-issue` — small, well-scoped, good for first-time contributors.
- `pattern-request` — a new snippet someone wants but hasn't built.
- `bug` — something in an existing snippet is broken or outdated.
