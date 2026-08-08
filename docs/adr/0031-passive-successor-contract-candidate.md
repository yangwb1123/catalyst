# ADR-0031: Passive successor contract candidate consuming verified predecessor receipts

- Status: Accepted for the effect-free schema-v17 successor-candidate slice
- Date: 2026-08-05
- Extends: [ADR-0026](0026-passive-schedule-bound-initial-node-contract.md)
- Follows: [ADR-0030](0030-scheduled-node-effectful-dispatch-lifecycle.md)

## Context

Sprint 59 delivered the scheduled ordinal-zero effectful dispatch lifecycle:
a real provider terminal produces a result/uncertainty artifact and a pinned
Go Core emits one intermediate terminal receipt, atomically persisted in the
v16 scheduled-lifecycle sidecar. ADR-0026 deliberately froze the contract
candidate at ordinal zero with an empty predecessor set, and ADR-0030 records
that "successor/wave advancement is a later protocol that must consume a
verified per-node, per-attempt receipt". ADR-0025 additionally fixed
authored-order direct-predecessor receipt slots in the schedule, and its
`receipt_handling` policy requires predecessor receipts before a successor
contract can exist. No successor selection, receipt consumption, or
content-addressed successor contract exists today.

## Decision

> **Current v24 clarification (2026-08-08):** the schema-v17 serial-prefix and
> one-candidate-per-Run details below are historical. ADR-0035/0036 supersede
> them with topology-ready, per-node slots: caller receipt files may arrive in
> any order, but Core emits the complete direct-predecessor set in canonical
> schedule order and Rust matches the supplied full receipts as an exact set.
> Only strict durable `completed`/result receipts can satisfy a non-empty
> predecessor; an explicit ordinal>0 target with no direct predecessors may be
> selected with zero receipts. ADR-0033 separately permits ≤1 MiB content bound
> to the canonical first direct receipt. None of these passive paths grants
> dispatch or successor-advance authority.

Add one passive, content-addressed successor contract candidate v2 for the
next serial node after a contiguous prefix of verified terminal receipts.
The predecessor receipt is evidence, never a provider-Prompt input:
`predecessor_content_included` stays false, and receipt identities are bound
only as contract metadata (the ADR-0026 disclosure rule).

### Predecessor receipt export (Rust)

```text
forge-runtime group graph run scheduled-contract \
  predecessor-receipt export PROVIDER_REQUEST_ID
```

The command opens the current v16 Hub read-only (existing-current
schema-v16 preflight), requires the scheduled lifecycle sidecar for that
provider request to be `terminalized` with a persisted Core receipt, and
returns the exact canonical receipt JSON without a trailing LF plus its
domain-separated digest. It verifies the receipt binds the persisted
artifact/control identities before emitting. The command creates no state,
reads no credential, constructs no provider, and does not access the network,
workspace, or tools. One receipt is exported per invocation; the caller
orders multiple exports by execution ordinal.

### Successor selection and candidate build (Go Core)

```text
forge graph-scheduled-node-contract \
  --control control.json --schedule-sha256 SHA256 \
  [--predecessor-receipt FILE|-]... \
  --endpoint ... --model ... --max-output-tokens N ... > candidate.json
```

Core rebuilds the unique schedule from the exact private control and requires
its digest to equal `--schedule-sha256` (unchanged). When at least one
`--predecessor-receipt` is supplied, Core requires the receipts to form the
exact contiguous prefix of the serial schedule: receipt *i* must correspond to
`schedule.nodes[i-1]` (execution ordinal *i-1*, same `node_id` and `attempt`),
and the successor candidate is `schedule.nodes[N]` where N is the number of
receipts. Each receipt must:

- strictly decode as one canonical `GroupAgentScheduledNodeTerminalReceipt`;
- re-derive `receipt_sha256` under the fixed receipt digest domain and match;
- bind `graph_run_id` to the control snapshot, `project_lane_sha256` to the
  scheduled node, `node_id`/`attempt` to `schedule.nodes[N-1]`, and a
  non-empty `dispatch_id`;
- carry `retry_authorized = false` and `successor_advance_authorized = false`
  (the receipt is evidence that a terminal occurred, not an authority grant);
- present a valid `artifact_kind`/`node_outcome` pair and self-consistent
  `artifact_id`/`artifact_sha256`.

Core then freezes the successor candidate:

```text
contract_version = 2
contract_scope    = schedule_successor_only
execution_ordinal = N
required_predecessor_node_ids  = schedule.nodes[N].direct_predecessor_node_ids
predecessor_terminal_receipts  = [receipt metadata for each direct predecessor]
predecessor_content_included   = false
expected_last_event_seq        = 1          (scheduled Run stays v1/seq-1)
```

All effect flags stay false: `lifecycle_contract_admitted`,
`provider_request_present`, `execution_authority_released`,
`dispatch_authority_released`, `progress_observed`,
`successor_advance_authorized`. The candidate is not the Run's lifecycle
contract, cannot be dispatched, and does not advance a wave or successor.

### Admission (Rust, SQLite v17)

```text
forge-runtime group graph run scheduled-contract \
  successor admit GRAPH_RUN_ID --contract candidate.json \
  [--predecessor-receipt FILE|-]... --idempotency-key KEY
forge-runtime group graph run scheduled-contract \
  successor show SUCCESSOR_ID [--include-contract]
forge-runtime group graph run scheduled-contract \
  successor list [GRAPH_RUN_ID] [--limit N]
```

SQLite v17 adds one immutable `group_agent_graph_scheduled_node_successor_candidates`
table (one per Graph Run, mirroring the v14 one-per-Run convention, with its
own idempotency key). Admission validates in one key-first `BEGIN IMMEDIATE`:
the current Run is pristine v1/seq-1, the schedule matches, the successor
candidate is canonical, and every supplied predecessor receipt byte-for-byte
matches the durable v16 scheduled-lifecycle receipt for the same
`provider_request_id` whose lifecycle is `terminalized`. Any drift, missing
durable receipt, nonterminal state, stale head, or stored corruption fails
closed; exact same-key replay preserves identity/time/bytes. The Run, main
journal, v12 lifecycle table, v14 candidate, v16 sidecar, and WAL state are
not changed. Default views redact the candidate and receipts; only explicit
`--include-contract` reveals them.

### Safety and verification

The slice prepares a successor candidate only. It does not dispatch, claim a
lane, read a credential, construct a provider, access the network/workspace/
tools, produce a result, write back, or authorize wave/successor advancement.
Shared Go/Rust golden tests lock the canonical successor-candidate bytes,
request identity, digest domains, and a no-LF contract. Real Go→Rust CLI
tests prove byte-identical export/admit/show, replay/conflict semantics,
removed member workspaces, a zero-connection endpoint, and unchanged Hub
tables outside the new sidecar. Predecessor content disclosure and consent
for a future effectful successor dispatch remain a later protocol.
