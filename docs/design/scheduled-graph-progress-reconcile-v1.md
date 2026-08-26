# Scheduled Graph Progress Snapshot + Core Reconcile v1

## Purpose

The scheduled Graph path already persists a passive schedule, per-node
candidates, prepared provider requests, dispatch lifecycles, and terminal
receipts. Those records are individually inspectable, but no public operation
can atomically answer whether a whole scheduled Graph is ready to advance,
stopped on uncertain work, failed, or complete.

Version 1 adds one read-only observation and decision boundary. Rust validates
all durable inputs in one SQLite read snapshot, Go Core owns the serial
progress decision, and Rust revalidates the returned decision. Forge Runtime
does not materialize a candidate, prepare or send a provider request, claim a
Project lane, recover a dispatch, or logically mutate the Hub. The combined
claim assumes the operator trusts the exact official Core bytes.

## Public boundary

```text
forge-runtime group graph run reconcile GRAPH_RUN_ID \
  --core-bin ABSOLUTE_PATH \
  --core-bin-sha256 LOWERCASE_SHA256
```

Forge Runtime opens an existing exact-current Hub through the live read-only
path. That reader may participate in SQLite coordination, including SHM locks
or transient empty sidecars, but Runtime performs no logical Hub write or
migration. The Core executable must be canonical, regular, executable,
size-bounded, digest-pinned, copied into a sealed Linux executable, and pass a
version-1 handshake before it sees the snapshot.

Core is an operator-trusted same-user TCB component. The pin proves copied
byte identity and the handshake proves protocol compatibility only; neither is
publisher or functional attestation. Runtime launches Core with an empty
environment and bounded I/O, but supplies no syscall, filesystem, namespace,
mount or network/egress sandbox. A hostile or substituted-but-operator-pinned
binary can exercise ambient same-user capabilities.

## Snapshot contract

`ScheduledGraphProgressSnapshot` is compact canonical JSON bounded to 64 KiB.
It binds:

- the exact Graph Run, Graph, schedule identity, and schedule digest;
- the fixed schedule-v1 serial, one-in-flight, contiguous-prefix,
  exactly-one-attempt, fail-fast policies;
- exactly `node_count` entries in execution-ordinal order;
- each admitted candidate identity and contract digest;
- each prepared request identity and `prepared_request_sha256` semantic
  envelope digest, never the provider request body or its body digest;
- each lifecycle status and, only for a Core-terminalized lifecycle, its
  terminal outcome and receipt digest.

Candidate, provider-request, lifecycle, and terminal fields form a strict
presence chain. Every identity and stored artifact is revalidated against its
source records before projection. Prompt, request body, model output,
terminal artifact, authorization, pricing, credential, and workspace content
are excluded from the snapshot.

The snapshot digest is SHA-256 over the domain
`forge.scheduled-graph-progress-snapshot.v1\0` followed by the canonical
payload with `snapshot_sha256` omitted from the digest value.

## Core decision

Core strictly decodes and revalidates the snapshot, then emits one canonical
`ScheduledGraphReconcileDecision` bound to the snapshot digest:

| Disposition | Meaning | Next-node fields |
|---|---|---|
| `ready` | The first non-completed ordinal has not been claimed and can be advanced only by a later explicit operation. | Exact ordinal and node ID |
| `claimed_unknown` | The first non-completed ordinal is claimed; automatic recovery or resend is forbidden. | `null` |
| `manual_recovery_required` | The lifecycle is quarantined or operator-adjudicated; policy requires an explicit later recovery design. | `null` |
| `failed` | A trusted terminal receipt reports deterministic failure. | `null` |
| `failed_uncertain` | A trusted terminal receipt reports uncertain failure. | `null` |
| `completed` | Every scheduled node has a completed terminal receipt. | `null` |
| `incompatible_progress` | Durable evidence exists beyond the first non-completed ordinal, so it cannot be interpreted as schedule-v1 contiguous serial progress. | `null` |

Structural corruption, unknown fields, invalid identities or digests, missing
source bindings, and impossible evidence shapes fail the command. They are not
converted into `incompatible_progress`. Rust validates the decision's exact
encoding, digest, snapshot bindings, and disposition field shape without
reimplementing Core's scheduling choice.

The decision digest is SHA-256 over the domain
`forge.scheduled-graph-reconcile-decision.v1\0` followed by the canonical
payload with `decision_sha256` omitted from the digest value.

## Atomic read algorithm

Within one deferred SQLite transaction, the store:

1. validates the Graph Run and frozen Graph;
2. loads and validates its unique exact schedule;
3. walks the schedule's complete ordinal set once;
4. validates an initial or successor candidate when present;
5. validates its prepared request when present;
6. validates its lifecycle and exact terminal receipt when present;
7. rejects rows outside the scheduled ordinal set or any count disagreement;
8. constructs, hashes, and validates the content-free snapshot;
9. commits the read transaction before invoking Core.

The loaders reuse the already-validated Run, Graph, schedule, candidate, and
request objects. They do not recursively reconstruct the whole Run once per
node, so work remains linear in the bounded node and stored-body totals.

## Safety and concurrency

- The projection runs inside one deferred SQLite transaction. A deterministic
  two-connection regression pins that read snapshot after source validation,
  commits a legal scheduled lifecycle terminalization through the real
  claim/terminalize stores, and proves that the pinned reader returns the
  complete claimed pre-state while a fresh reader returns the complete
  terminalized/completed post-state with its receipt.
- Core invocation has an empty environment, bounded stdin/stdout/stderr, a
  deadline, and process-group termination on timeout. These resource controls
  do not confine filesystem or network effects.
- Forge Runtime and the tested official Core path succeed without provider
  credentials, workspace files, network, consent flags, or an idempotency key.
- The operation never grants dispatch or successor authority. `ready` is an
  observation, not authorization.
- Claimed, quarantined, adjudicated, failed, uncertain, or incompatible state
  always stops. There is no implicit lease expiry, adjudication, retry, resend,
  or cleanup.

The CLI makes this boundary explicit. JSON sets
`effect_facts_scope="forge_runtime"`; its separate `runtime_effect_facts`
object reports `credential_read=false`, `network_accessed=false`,
`workspace_accessed=false`, `logical_hub_mutated=false` and the other Runtime
effects as false. The `core_trust_boundary` object reports
`same_user_code=true`, `operator_trust_required=true`,
`binary_identity_validated=true`, `protocol_handshake_validated=true` and
`empty_environment=true`, while `filesystem_isolation_enforced`,
`network_isolation_enforced`, `effect_containment_enforced` and
`effect_attestation_present` are false. Human output repeats that the pinned
Core is trusted same-user code and that its byte pin is not effect containment
or attestation. These fields disclose the conditional contract and its trust
gap; they are not syscall-derived observations about an arbitrary pinned
executable.

## Deliberate limits and next slices

This slice is not `run-all`, a controller journal, or an effectful `step`.
Schedule v1 remains serial even though the separate wave-ready surface can
materialize topology-parallel candidates; such non-contiguous durable state is
reported as incompatible rather than silently reinterpreted.

Before an effectful slice can execute a successor selected by reconcile, the
separate Core release path needs a zero-effect successor-capable protocol. Its
current v1 validator is initial-candidate/ordinal-zero only. That prerequisite
must bind the exact progress snapshot and reconcile decision, rebuild the exact
initial or successor source, and retain a one-node maximum. The later effectful
slice must then require fresh consent for that exact provider request and may
execute at most one Core-selected node. Only after that boundary is independently
validated should a durable controller be added. Concurrent wave execution
requires a new schedule/progression version plus precise per-task crash
evidence; it is not an extension of this v1 contract.
An untrusted Core binary additionally requires a separately reviewed
filesystem/network/syscall confinement and attestation contract; SHA-256 pinning
alone cannot open that boundary.

## Implemented validation scope

Go tests currently cover every disposition, canonical decision bytes and
digest mutation. Rust domain/application tests cover strict snapshot and
decision validation, source binding and error mapping. One integration test
builds the real Go Core and checks an exact Rust snapshot/decision golden.

SQLite-focused tests cover schedule-only projection, candidate plus
prepared-request identity, preservation of non-contiguous evidence, a missing
schedule and an out-of-schedule candidate row. They now also cover the complete
32-node storage projection in ordinal order, exact canonical round-trip under
64 KiB, a legal claimed-to-terminalized transaction interleave, two valid
lifecycle baselines, and 42 independently injected corrupt states across
initial/successor candidates, provider requests, claimed lifecycles,
orphan/extra/count disagreements, and terminal artifact/control/receipt
evidence. Core tests cover two 32-node decision-boundary exemplars: a 31-node
completed prefix selects ordinal 31, while 32 completed nodes stop as complete;
both signed canonical inputs remain under 64 KiB. This proves the node-count
boundary and those exemplar sizes, not a byte-maximal snapshot using the longest
identifiers or every optional ready-node field.

Process-level CLI tests run the repository-built official Core and wrong-pin
failure path, compare logical Hub table snapshots, preserve workspace sentinels,
poison credential/endpoint inputs, reject private-output leakage and observe no
loopback connection. These tests do not prove confinement of an arbitrary
operator-pinned Core binary.
