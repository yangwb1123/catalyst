# ADR-0025: Passive multi-node execution schedule sidecar

- Status: Accepted for the effect-free schema-v13 schedule slice
- Date: 2026-08-01
- Extends: [ADR-0024](0024-single-node-dispatch-terminal-lifecycle.md)

## Context

ADR-0024 deliberately permits provider execution only for one-node, one-wave,
zero-edge Graphs. Its contract selects `waves[0][0]`, predecessor receipts are
empty, predecessor dataflow is `none`, and the terminal receipt makes the node,
wave, and Graph terminal together. SQLite v12 likewise permits only one
contract, request, claim, artifact, and receipt per Graph Run.

Those constraints cannot be relaxed independently. Treating a v1 node result
as multi-node progress would falsely complete the whole Graph. Re-running the
topology planner after each result would also let Rust or a caller replace Core
as scheduler owner. Passing result text merely because two nodes have an edge
would turn an ordering declaration into undeclared off-machine disclosure.

Before a successor contract can exist, the system therefore needs one exact,
Core-owned policy that freezes how a multi-node Graph would be traversed and
which predecessor receipt identities a later contract must bind. That policy
must add no execution fact and must not consume the main Graph Run journal
sequence already assigned to the first contract.

## Decision

Add a passive, content-addressed `GraphExecutionSchedule` v1 sidecar for Graphs
with at least two nodes. It is built from the existing exact private v1 Graph
control snapshot and persisted by Rust only after independently rebuilding the
same current Graph, Run, plan, manifest, seq-1 head, and control digest.

The schedule is policy, not progress. Creating it:

- does not change the Graph Run version, status, flags, journal, or head;
- does not create a contract, request, authorization, claim, lane, result,
  terminal receipt, successor decision, Conversation, Prompt, task, or memory;
- does not read a credential, construct a provider, access a workspace or tool,
  or use the network;
- does not authorize a later node or reuse consent; and
- does not make the existing contract-v1 path schedule-bound or multi-node
  executable.

The public offline flow is:

```text
forge-runtime group graph run control export GRAPH_RUN_ID > control.json
forge graph-execution-schedule --control control.json > schedule.json
forge-runtime group graph run schedule admit GRAPH_RUN_ID \
  --schedule schedule.json --idempotency-key KEY
forge-runtime group graph run schedule show SCHEDULE_ID [--include-schedule]
forge-runtime group graph run schedule list [GRAPH_RUN_ID] [--limit N]
```

The existing control export remains the single source of private Graph prose.
The Core schedule output deliberately omits manager instruction, task,
acceptance, Project ID, member role, agent profile, provider, model, credential,
and result text.

## Exact Core input

Core accepts only the existing canonical `GroupAgentGraphControlSnapshot` v1.
That snapshot already binds:

- Graph Run and Graph identity;
- frozen Group source, manifest, and Core Plan digests;
- exact plan and private manifest;
- Run v1, seq 1, and the exact event head;
- `execution_contract_present=false`; and
- `dispatch_authority_released=false`.

The normal strict decoder rejects duplicate, unknown, missing, null, reordered,
trailing, invalid UTF-8, oversized, noncanonical, or digest-drifting input. The
schedule builder additionally rejects a one-node Graph; ADR-0024 remains the
only effectful protocol for that topology.

## Schedule policy

Core flattens `plan.waves` in wave order while preserving authored order inside
each wave. The result fixes:

```text
execution_mode = serial
max_in_flight_nodes = 1
selection_policy = topology_wave_then_authored_order
progression_policy = completed_contiguous_prefix
attempt_policy = exactly_one
```

Every scheduled node contains:

- zero-based execution ordinal;
- node identity and zero-based authored index;
- topology wave index;
- attempt 1;
- the domain-separated Project lane digest; and
- only its direct predecessor node identities, ordered by authored-node order.

Transitive predecessors are not inserted. Canonical edge ordering is not used
as predecessor ordering. The schedule also records the complete initial
frontier in authored order and Core's deterministic initial selection. That
selection is a static policy fact, not evidence that the node is ready at some
later durable head.

Edges retain `ordering_only` semantics. Schedule v1 fixes predecessor dataflow
to `none`, partial-output dataflow to false, and receipt handling to
`future_verified_identity_slots`. The direct predecessor list identifies the
receipt slots a later contract-v2 protocol must fill, but contains no synthetic
receipt ID and copies no result or partial-result text.

The fixed fail-fast outcome policy is:

- a completed non-final node permits a later Core protocol to consider the next
  serial ordinal;
- a completed final node permits a later Core protocol to complete the Graph;
- `length` fails the Graph with no successor and no partial-output flow;
- uncertainty fails the Graph uncertain with no successor and no retry; and
- `dispatch_unknown` remains quarantined.

These are static rules only. Schedule v1 has no terminal input and observes no
outcome. Provider `completed` would still prove only protocol completion, not
that the authored acceptance criterion is true.

The artifact always states:

```text
progress_observed = false
successor_advanced = false
execution_contract_present = false
dispatch_authority_released = false
```

Its SHA-256 is computed over the canonical payload without the final ID and
digest, using the domain:

```text
forge.group-agent-graph-execution-schedule.v1\0
```

The content-addressed ID is
`graph-execution-schedule-<lowercase-sha256>`. Core emits compact UTF-8 without
a trailing line feed.

## Durable sidecar and replay

Schema v13 adds an application-owned immutable schedule table. It references
one Graph Run, stores the exact canonical Core artifact and its byte/digest
identity, repeats the expected seq-1/head and false authority flags as queryable
constraints, and records a caller idempotency key plus local admission time.
The Graph Run and its main event table are not rebuilt or modified.

Admission uses `BEGIN IMMEDIATE`. Within the transaction Rust:

1. loads and completely validates the current v1 Graph Run and frozen Graph;
2. reconstructs the exact control snapshot and compares its canonical bytes;
3. validates the Core artifact, per-node topology, lanes, predecessor order,
   policies, false flags, byte count, digest, and ID;
4. resolves the key and unique Graph-Run ownership;
5. inserts the exact sidecar;
6. rereads and fully validates the stored schedule; and
7. commits only after all checks pass.

The application layer validates the returned sidecar against its pre-admission
candidate snapshot, but does not use a second read after commit as an atomicity
witness. A legitimate concurrent contract admission may advance the Run after
the schedule transaction commits; only the storage transaction can prove that
the schedule write itself was sidecar-only and used the pristine head.

The same key and exact semantic schedule replays the original identity, bytes,
and time. The same key with drift, another key for the Run, a stale head, or a
different schedule conflicts. Stored corruption is not downgraded to replay or
conflict. Concurrent candidates yield one creation and only an exact replay or
conflict for the other contender.

Migration from v12 only creates the new empty table and index. It does not
rewrite any existing table, active v4 claim, Project lane, artifact, receipt,
or WAL state. Existing one-per-Run uniqueness remains intact.

## Inspection and privacy

`show` performs the full source, control, canonical-artifact, and stored-row
validation. Default output is metadata-only and hides the schedule body, node
identities, predecessor identities, lane digests, and idempotency key.
`--include-schedule` explicitly reveals the validated artifact. `list` remains
metadata-only and states that it does not revalidate source or artifact bytes.
CLI false flags are scoped as `artifact_*`; schedule views explicitly state
`current_run_lifecycle_included=false`, because a later legacy contract may
legitimately coexist with the historical artifact.
Human output terminal-escapes every displayed identifier.

The schedule and its SHA-256 are local consistency evidence, not a signature,
MAC, remote attestation, factual verification, or protection from the same OS
user modifying SQLite.

## Compatibility and protocol fences

Single-node Graphs continue through ADR-0024 and reject schedule creation.
Existing passive contract-v1 artifacts for multi-node Graphs remain readable,
but do not bind this schedule and remain rejected by effectful dispatch. A later
contract-v2 protocol must explicitly consume the schedule digest and real
predecessor receipt identities before the execution fence can change.

Terminal receipt v1, the five-event main journal, and every v12 lifecycle table
remain unchanged. In particular, this ADR does not reinterpret a terminal
receipt, remove a unique constraint, release a stranded v4 lane, or adjudicate
a hard crash.

## Required verification

1. shared Go/Rust canonical bytes and digest for a frontend/backend/SSO diamond;
2. authored-order serial ordinals, initial frontier, direct predecessor slots,
   and Project lane digests;
3. input edge reordering invariance and authored-node reordering sensitivity;
4. single-node, malformed, noncanonical, null-array, Unicode, bound, policy,
   false-flag, digest, and identity rejection;
5. v12-to-v13 migration, active-v4/lane preservation, fault rollback, exact
   replay, divergent-key conflict, concurrent admission, and corruption-first
   behavior;
6. default-redacted and explicit-reveal CLI behavior; and
7. byte-exact proof that schedule admission leaves the Graph Run main record,
   events, credential/provider/network/workspace/tool state unchanged.

## Rejected alternatives

- **Relax terminal receipt v1.** It equates one node terminal with Graph
  terminal and would manufacture completion.
- **Write a schedule event at Graph Run seq 2.** Seq 2 is the published first
  contract event; consuming it would break old journals.
- **Copy predecessor result text now.** Edges are ordering-only, no predecessor
  receipt exists, and no disclosure consent has been granted.
- **Let the caller choose the initial node.** Scheduler selection remains owned
  by Go Core.
- **Parallelize a topology wave.** A wave is dependency-ready, not proof of
  Project, workspace, budget, or provider safety.
- **Adjudicate old v4 claims while adding the schedule.** Old claims lack a
  claim-scoped executor lock and durable send-phase evidence; time cannot prove
  no-send.

## Consequences

Multi-node Graphs gain a durable, inspectable, Core-owned execution order,
per-node lane identity, direct predecessor receipt slots, and fail-fast policy
without any external effect. This closes the policy gap that must precede a
real successor contract while preserving every v1 terminal and v12 quarantine
guarantee.

The system still does not execute frontend/backend/SSO as a group, advance a
successor, transmit predecessor content, admit a schedule-bound contract,
perform manager discussion, or recover an abandoned dispatch. Those remain
separate protocols and must not be inferred from schedule presence.
