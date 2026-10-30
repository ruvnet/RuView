# Social metaharness contributor contract

This scope inherits the repository root contract and adds stricter social
account controls.

## Authority boundary

Phase 1 is a zero credential, read only control plane. It may inspect committed
policy and research, lint drafts, plan actions, normalize user supplied metrics,
verify digest receipts, and screen optimization candidates. It must not connect
accounts, fetch authenticated data, publish, reply, react, message, moderate,
delete, spend, record audio, deploy, or promote learned policy.

All MCP tools remain read only. A future external effect may be exposed only by
a CLI adapter that is isolated from MCP and requires an exact, expiring,
device bound human approval plus a separately granted process authority.

## Invariants

1. Each platform operation is exactly `API_ALLOWED`, `ATTENDED_MANUAL`, or
   `DENY`.

2. Missing API capability never falls back to computer use.

3. Voice can create a direction but can never approve it.

4. Personal rUv, ruvnet, Agentics, and Cognitum identities remain separate.

5. Tool arguments, receipts, logs, tests, and fixtures contain no credentials,
   cookies, private messages, raw audio, or account capability links.

6. Audit receipts contain digests and bounded metadata, not content.

7. Platform response headers and current policy are authoritative for mutable
   limits. Never hardcode a historical quota as an execution allowance.

8. The optimizer may propose. Phase 1 cannot establish review eligibility or
   promote. A future separately accepted path must require a named maintainer
   and independently verified evidence, anchor, provenance, security, and
   blocked action gates.

9. Public counters are dated evidence. Followers, stars, downloads, clones,
   and views are not unique users and are never summed across platforms.

10. GCP resources are optional, restartable, least authority, and disabled
    until a human runs a reviewed plan. Cloud runtime is never described as
    perpetual.

## Validation

Run from this directory:

```bash
npm test
npm run test:security
npm run doctor -- --strict
npm run manifest:verify
npm pack --dry-run
```

Review the diff for secrets, invite links, new write tools, broader IAM roles,
mutable container tags, unsupported claims, raw content in receipts, and
unrelated files.
