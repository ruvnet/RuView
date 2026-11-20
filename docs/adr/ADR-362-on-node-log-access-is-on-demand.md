# ADR-362: Reading the on-node log is on-demand, never streamed

**Status:** Accepted
**Date:** 2026-09-09
**Follows:** the on-node log engine (persistent 128 B-record ring + coredump-to-flash)

## Context

The firmware now keeps a persistent log on the node's `storage` volume and can
write a coredump to flash. Nothing reads either one. The log is a new concept
in this tree, so there is no existing consumer to extend and the access
pattern is genuinely open.

Two shapes were considered:

1. **Ingest** — the server polls every node on a schedule, drains new records,
   and keeps its own copy that the UI reads.
2. **On-demand** — the server fetches a node's log only when a human asks for
   it, and keeps nothing.

## Decision

**On-demand only. No streaming, no periodic polling, no server-side copy, no
alerting.**

The log is pulled after a board hangs or crashes, or while troubleshooting a
specific node. That is the whole access pattern.

## Why

- **The persistence already lives on the node.** The ring holds ~208 days at
  128 B per 300 s, and surviving a power cycle is the entire reason the module
  exists — a power cycle is how a wedged node gets recovered. A second copy on
  the server duplicates a durability property that is already satisfied.
- **Airtime is this fleet's binding constraint, and it is the thing being
  diagnosed.** Two nodes uplinking at 1 Mbps were measured consuming 91% of
  fleet airtime. A periodic log drain across nine nodes would spend exactly the
  resource whose exhaustion is the most common thing you would open the log to
  investigate. An instrument must not perturb what it measures.
- **Coredumps are 64 KB.** They are only ever worth moving when something has
  actually crashed.
- **The use is diagnostic, not observational.** Nobody watches nine logs. You
  open one node's history when that node misbehaves.

## What this implies gets built

- A server route that proxies to the node on request — `GET
  /api/v1/nodes/{id}/log`, and the same for `/coredump` — authenticating with
  the OTA PSK the firmware endpoints already sit behind.
- A page under the existing Tools menu to view one node's records.
- **No** scheduler, **no** new storage, **no** retention policy on the server.

## Open question, deliberately not settled here

Whether the server decodes the 128 B records into JSON, or proxies raw bytes
and lets the decoder at the edge (browser, or `node_log_read.py`) interpret
them.

Decoding server-side means the record layout is defined in two places — C and
Rust — and can drift silently. The pilot already paid for a version of this:
a field whose sentinel the host had to learn to render as `n/a`, where a
plausible wrong value would have been worse than an obvious gap. That argues
for the raw proxy and a single definition of the format.

## Non-goals

Real-time monitoring, log streaming, fleet-wide log aggregation, and alerting
on log contents are all explicitly out of scope. If a future need genuinely
requires one of them, it should revisit this ADR rather than grow into it.
