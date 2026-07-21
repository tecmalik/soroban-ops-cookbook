# Webhook Relay

**Status: not built yet — this is issue #2 in the repo, not a finished tool.**

Planned: same polling loop as [`postgres-sink`](../postgres-sink), but instead
of writing to Postgres, POSTs each new event as JSON to a configured webhook
URL, with basic retry/backoff.

Rather than ship a copy-pasted, unverified version of this, it's left as an
open `good-first-issue` — see the repo issues. If you want to build it,
the `postgres-sink` polling loop is the reference to fork; swap the
`insert_events` step for an HTTP POST with retry.
