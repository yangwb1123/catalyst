# Platform Core Receipt and State v1

## Scope

This contract is the second pure Platform Core increment. It freezes:

- `ExecutionReceipt` declarations produced for one Runtime Attempt/Session;
- `VerificationRequest` and `VerificationReceipt` declarations exchanged with Harness;
- WorkItem, Attempt, and Action state vocabularies and allowed directed edges; and
- stable machine-readable rejection classes shared by Go, Rust, and Python.

It reuses the typed IDs, `ScopeRef`, `ActorRef`, `RecordRef`, `ArtifactRef`, exact
canonical JSON, Unicode restrictions, signed-int64 profile, depth/count limits,
and reference caveats from [Platform Core Envelope v1](platform-core-envelope-v1.md).

This is a supplied-bytes contract. It does not mint an ID, authenticate an actor,
resolve a record, verify an Artifact, evaluate a Grant or Approval, execute a
check, append an event, read current state, acquire transition authority, persist
a receipt, or complete a Change.

## Canonical wire and identity

Each family has exact integer version `1`, canonicalization
`forge.canonical-json/v1`, exact required fields, and a 256 KiB complete-document
limit. Unknown fields, duplicate keys, null required arrays, noncanonical JSON,
floats, invalid UTF-8, forbidden Unicode, values outside signed int64, depth over
twelve, arrays over 256, or objects over 64 fields fail closed.

Readers classify failures in this order: bounded JSON framing and canonical
form; complete typed document shape (exact required fields, unknown fields,
scalar/collection types, and non-null required arrays); identifier and non-state
per-field value rules; reference bindings; state vocabulary; transition edges;
then cross-field/cross-record relations. Operations without a stage skip it.
Typed-language zero values cannot bypass the complete-shape step. This order is
load-bearing for the stable rejection code even though diagnostic text is not.
`VerificationExchange` applies each stage across both supplied documents before
advancing: both shapes, both value sets, both reference sets, receipt state, and
only then request, receipt, and exchange relations. A request relation cannot
mask an earlier-stage receipt rejection.

Receipt and request observation digests are:

```text
forge.platform.execution-receipt.v1\0 || canonical ExecutionReceipt bytes
forge.platform.verification-request.v1\0 || canonical VerificationRequest bytes
forge.platform.verification-receipt.v1\0 || canonical VerificationReceipt bytes
```

They exist for conformance and request binding. They are not signatures, MACs,
authenticated record identities, persistence proofs, or authorization.

`receipt_id` uses `rcp_`; `verification_id` uses `ver_`. The producer supplies
both values. Live generation, collision handling, reservation, replay protection,
and durable uniqueness remain outside this contract.

## ExecutionReceipt

An `ExecutionReceipt` contains:

- exact `receipt_id`, canonicalization, and `execution_receipt_version`;
- a contiguous session-level `scope_ref` with ProjectSnapshot, Attempt, and
  Session present and Turn/Action absent;
- exact Attempt, Session, and ProjectSnapshot `EntityRef` copies matching scope;
- executor actor declaration restricted to `agent`, `service`, or `system`,
  namespaced adapter ID, and canonical numeric `major.minor.patch` adapter
  version;
- nullable unresolved Grant (`forge.control.capability_grant`) and Approval
  (`forge.control.approval_record`) `RecordRef` declarations, with role/type
  mismatches rejected as `pc_reference_mismatch`;
- start/end times and observed usage;
- terminal Attempt state and reason codes;
- sorted input/output `ArtifactRef` sets; and
- a nullable contiguous declared event range for the Attempt aggregate.

Observed usage contains nonnegative `cost_usd_micros`, `elapsed_ms`,
`input_tokens`, `model_calls`, `network_bytes`, `output_bytes`, `output_tokens`,
and `tool_calls`. `elapsed_ms` must equal `ended_at_unix_ms -
started_at_unix_ms`; the interval is bounded at 365 days. Counts are capped at
1,000,000,000 and quantities at 1,000,000,000,000,000. These are caller-supplied
observations, not independently metered budget authority. Each endpoint is an
independently bounded timestamp; endpoint ordering, maximum duration, and the
elapsed equality are final `pc_relation_mismatch` checks after references.

Input and output arrays are always present and contain at most 32 ArtifactRefs,
strictly sorted and unique by logical ID. Every Artifact binds the receipt's
ProjectSnapshot. An input cannot postdate execution start. An output must name
the receipt Attempt as producer and have a creation time inside the execution
interval. Validators still do not resolve content, recompute size/digest, or
authenticate provenance.

`event_range` is null or contains an Attempt aggregate, first/last Event IDs,
and positive ordered component sequences. A one-event range uses one equal ID;
a multi-event range uses distinct endpoint IDs. The range does not prove that
events exist, are contiguous in a durable journal, share correlation, or contain
the claimed terminal transition. Sequence positivity is an independent value
rule; endpoint ordering and ID/cardinality agreement are relation rules.

Only `interrupted`, `completed`, `failed`, and `uncertain` are terminal receipt
states. `completed` requires an empty reason set; every other terminal requires
one or more sorted unique lowercase reason codes. A completed Attempt receipt
does not imply successful verification, WorkItem completion, or Change completion.

## VerificationRequest

A `VerificationRequest` binds:

- canonicalization, `verification_request_version`, and `verification_id`;
- requester actor declaration and request time;
- a scope containing ProjectSnapshot and Attempt;
- one immutable-input `ArtifactRef` matching that snapshot and producing
  Attempt, whose creation time does not postdate the request; and
- 1–64 checks, strictly sorted and unique by `check_id`.

Each check has a bounded lowercase `check_id`, a namespaced `check_name`, and
boolean `declared_required`. The declaration is carried for a later completion
policy consumer; this pure validator neither registers check implementations nor
makes `declared_required` effective policy.

## VerificationReceipt

A `VerificationReceipt` binds its own `rcp_` identity, the `ver_` request
identity, the domain-separated request digest, exact request scope and input,
a Harness actor declaration, bounded start/end times, sorted results, and a
strictly derived overall status.

Each result exactly covers one requested `check_id` when request and receipt are
compared. It contains:

- `status`: `pass`, `fail`, `inconclusive`, or `not_executed`;
- `applicability`: `applicable` or `not_applicable`;
- nullable `applicability_reason`;
- sorted unique reason codes; and
- 0–16 unresolved evidence `RecordRef` declarations, strictly ascending and
  unique by the ASCII `record_id` bytes (the other `RecordRef` fields do
  not participate in ordering).

An applicable result requires a null applicability reason. A not-applicable
result must be `not_executed` and carry a nonempty bounded reason. `pass` requires
no reason codes; every non-pass status requires at least one. Evidence references
are not resolved or authenticated.

`overall_status` is derived over applicable results only with precedence:

```text
fail > inconclusive > not_executed > pass
```

If every result is not applicable, overall status is `not_executed`, never
`pass`. The request/receipt relation validator requires the exact request digest,
verification ID, ScopeRef, input ArtifactRef, nonretrograde timing, and one result
for every requested check in the same sorted order. It does not make either
record durable or authoritative.

## State vocabulary

Validation only answers whether an explicitly supplied edge belongs to this
graph. It does not read current state, enforce expected version, consume policy,
append an event, or apply a transition.

### WorkItem

```text
draft -> planned | cancelled
planned -> awaiting_approval | ready | cancelled
awaiting_approval -> ready | blocked | cancelled
ready -> dispatched | blocked | cancelled
dispatched -> running | blocked | failed | uncertain | cancelled
running -> verifying | blocked | failed | uncertain | cancelled
verifying -> completed | ready | blocked | failed | uncertain | cancelled
blocked -> ready | failed | cancelled
failed -> ready | cancelled
uncertain -> ready | failed | cancelled
completed -> (none)
cancelled -> (none)
```

The `planned -> ready` and `requested -> approved` shortcuts only make room for
a future policy decision that permits bypassing human approval. Structural edge
validity does not supply that decision.

### Attempt

```text
requested -> accepted
accepted -> starting | interrupted | failed | uncertain
starting -> running | interrupted | failed | uncertain
running -> interrupted | completed | failed | uncertain
interrupted | completed | failed | uncertain -> (none)
```

Retries create a new Attempt identity; a terminal Attempt cannot reopen.

### Action

```text
requested -> awaiting_approval | approved | rejected | cancelled
awaiting_approval -> approved | rejected | cancelled
approved -> started | cancelled
started -> finished | failed | cancelled | uncertain
finished | rejected | failed | cancelled | uncertain -> (none)
```

## Stable rejection classes

Public contract operations return one code independently from their non-stable
human diagnostic:

| Code | Meaning |
|---|---|
| `pc_document_invalid` | malformed, noncanonical, duplicate/unknown-field, type, or framing rejection |
| `pc_identifier_invalid` | malformed Platform ID or wrong typed namespace |
| `pc_value_invalid` | unsupported or out-of-bound field value/cardinality |
| `pc_reference_mismatch` | valid reference shape does not match required scope/aggregate |
| `pc_state_invalid` | known field is not an allowed state for that role |
| `pc_transition_invalid` | source and target states are valid but the edge is absent |
| `pc_relation_mismatch` | individually shaped values violate a cross-field or cross-record relation |

Callers branch only on the code, never diagnostic text. One failure may satisfy
several categories; validators return the first deterministic load-bearing
rejection in their documented validation order. A future finer taxonomy requires
a new reviewed contract version or additive subcode field, not repurposing these
meanings.

The shared golden is
[`fixtures/platform-core-receipt-v1.json`](fixtures/platform-core-receipt-v1.json).
The shared mutation corpus is
[`fixtures/platform-core-rejection-corpus-v1.json`](fixtures/platform-core-rejection-corpus-v1.json).
Its four evidence-reference boundary/order cases, three executor-role acceptance
cases, exhaustive 61-edge state-machine tables, 62 wire cases, and 9 targeted
transition cases cover all normative executor roles and all seven rejection
classes. Go, Rust, and Python independently reconstruct all three digests and
consume the same corpus.

## Bounds and compatibility

- Execution input/output Artifact arrays: 0–32 each, non-null.
- Verification checks/results: 1–64 each, non-null.
- Result evidence references: 0–16, non-null.
- Reason codes: 0–16, non-null; additional semantic minimums apply.
- Applicability reason: 1–512 UTF-8 bytes when present.
- Adapter version: canonical decimal `major.minor.patch`, each component at most
  nine digits and without leading zeroes except zero itself.
- Complete request or receipt: at most 256 KiB under the Envelope v1 canonical
  JSON depth, collection, scalar, and Unicode profile.

Writers emit exact v1. Readers accept exact v1 only. The Receipt schema references
Envelope v1 primitives but does not alter the Envelope v1 wire or its digest.
Adding optional wire fields, changing status/edge semantics, changing derivation,
or changing a digest domain requires a reviewed version. Diagnostic wording may
change while stable code meaning does not.

The JSON Schema is a structural shadow. Executable validators remain load-bearing
for canonical bytes, duplicate detection, byte/depth limits, sorted uniqueness,
typed-ID agreement, scope ancestry, time relations, Artifact relations, event
range cardinality, request digest binding, exact check coverage, state edges,
reason/applicability rules, and overall-status derivation.

## Non-authority and completion boundary

Positive validation means only that supplied declarations are internally
consistent under this version. Grant/Approval refs remain unresolved; actor type
is unauthenticated; Artifact existence and content are unproved; event ranges are
not journal observations; usage is not trusted metering; check evidence is not
truth; and state-edge membership is not transition permission.

No receipt alone may mark WorkItem, Change, Objective, Run, ADR, release, or
repository work complete. A future durable consumer must separately bind current
state/version, idempotency, event sequence, immutable bytes, reference existence,
freshness/expiry, authenticated policy/Approval/Grant, required verification,
completion rules, transactionality, and recovery.
