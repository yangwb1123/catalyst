# ADR-0030: Independent scheduled-node effectful dispatch lifecycle

- Status: Accepted for the scheduled ordinal-zero v1 effectful slice
- Date: 2026-08-04
- Extends: [ADR-0029](0029-effect-free-scheduled-node-dispatch-readiness.md)
- Keeps separate from: [ADR-0024](0024-single-node-dispatch-terminal-lifecycle.md)

## Context

Sprint 58 completed the last effect-free checks for an ordinal-zero scheduled
provider request. The remaining operation is materially different from the
legacy single-node lifecycle: a scheduled Graph Run remains v1/seq-1, its
passive request sidecar must not be discovered by legacy dispatch, and a
crash after a durable claim cannot be repaired by a lease or a later resend.

## Decision

Implement one independent scheduled lifecycle protocol. Its durable sidecar is
not a legacy claim, lane, terminal artifact, receipt, or Graph Run journal
row. A single SQLite transaction performs the pristine-head check, fresh
authorization/credential preflight result hand-off, immutable claim, and
global Project-lane acquisition. The only returned execution authority owns
the exact persisted provider body and is consumed once.

The claim keeps two distinct content identities: `provider_request_sha256` is
the immutable prepared-request envelope identity, while `request_body_sha256`
is the SHA-256 of the exact persisted provider bytes. Both identities are
bound through the authorization, sidecar record, terminal artifact, control,
and receipt; they are never treated as interchangeable.

After the provider stream ends, the application creates bounded terminal
evidence and sends a scheduled-specific terminal control to a pinned Core
command. Core independently validates the control identity and emits one
intermediate terminal receipt. A second SQLite transaction atomically stores
the evidence and receipt and releases the scheduled lane. A Core failure or an
uncertain provider outcome is quarantined and never retried or resent.

The sidecar records only scheduled lifecycle evidence. It never changes the
scheduled Run's v1/seq-1 journal, does not infer predecessor content from an
ordering edge, and does not authorize successor execution. Successor/wave
advancement is a later protocol that must consume a verified per-node,
per-attempt receipt.

## Public command

```text
forge-runtime group graph run scheduled-contract provider-request \
  dispatch execute PROVIDER_REQUEST_ID \
  --authorization FILE|- \
  --pricing FILE|- \
  --core-bin ABSOLUTE_FILE \
  --core-bin-sha256 SHA256 \
  --confirm-off-machine
```

Authorization, pricing, and Core identity are bounded before Hub construction;
at most one artifact may use stdin. The command has no retry, resume, lease,
health-check, provider-send, or successor-advance subcommand.

## Safety and verification

The command repeats all readiness checks after fresh consent, requires an
explicit credential whose header is safe, and constructs a provider without a
health request. It opens writable schema-v16 state only after that effect-free
preflight. The scheduled sidecar has a unique active Project-lane index, exact
content bindings, corruption-first replay checks, and a no-resend terminal or
quarantine state. Existing legacy dispatch commands reject scheduled sidecars.

All tests use deterministic local providers, a pinned local Core executable,
and Go/Rust subprocesses. No live provider, paid model, public network,
workspace, tool, or writeback effect is authorized by this ADR.
