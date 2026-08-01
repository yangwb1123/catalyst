# ADR-0021: Core-owned node dispatch request preparation

- Status: Accepted for the effect-free v11 request-preparation slice
- Date: 2026-08-01
- Extends: [ADR-0019](0019-core-owned-first-node-execution-contract.md)

## Context

ADR-0019 leaves one fully validated Graph Run at version 2,
`awaiting_core_dispatch`. The Run has one immutable first-node execution
contract and a two-event journal, but dispatch authority is still false. The
contract freezes the system and user Prompt, provider destination, model,
budgets, zero workspace/tools/dataflow, fresh future consent, and the
post-claim no-retry rule.

That contract is not yet the exact byte vector a provider transport would
send. In particular, the OpenAI Responses adapter owns the canonical envelope
that maps the frozen logical request to `instructions`, `input`, `include`,
`tools`, `max_output_tokens`, `stream`, and `store`. Encoding that envelope
after a dispatch claim would be unsafe: a codec mismatch, unsupported body, or
size failure would be discovered only after the Run had irreversibly entered
an uncertain dispatch state.

Claiming and immediately connecting the existing Project `AgentRuntime` is
also the wrong next step. That adapter owns Conversation, Project Run,
multi-turn, tool, workspace, event-sink, and optional Prompt-writeback
semantics that a Graph node does not have. Result artifact and terminal receipt
rules are not yet defined, so this slice cannot honestly call a node running or
completed.

The safe intermediate boundary is therefore exact request preparation. It
must remain effect-free and must not let Rust become a second scheduler.

## Decision

Hub schema v11 adds one adapter-owned, exact Node Dispatch Request preparation
step. It deterministically encodes the already selected Node Execution
Contract and persists the exact provider body, but releases no dispatch
authority.

The flow is:

```text
validated v2 Graph Run at awaiting_core_dispatch
  -> Rust revalidates the complete source and admitted contract
  -> Rust's pure OpenAI Responses codec encodes exact request bytes
  -> SQLite admits the request by seq-2/head CAS
  -> Graph Run stops at awaiting_dispatch_authorization
```

Rust does not choose a node, attempt, lane, provider, endpoint, model, Prompt,
budget, or capability. Every semantic input was already frozen by the
Go-owned Node Execution Contract. Pure adapter serialization is not scheduling
because it cannot change those values and cannot release, send, retry, or
advance anything.

Go remains the sole scheduler and future release-decision owner. This ADR adds
no Rust next-node, release, claim, run, complete, retry, or wave-advance API.

## Exact version-3 state

Schema v11 admits these exact Graph Run states:

| Run version | Status | Contract | Dispatch request | Authority | Last seq |
|---|---|---:|---:|---:|---:|
| 1 | `awaiting_execution_contract` | false | false | false | 1 |
| 2 | `awaiting_core_dispatch` | true | false | false | 2 |
| 3 | `awaiting_dispatch_authorization` | true | true | false | 3 |

The version-3 projection has:

```text
execution_contract_present = true
dispatch_request_present = true
dispatch_authority_released = false
last_event_seq = 3
```

The immutable Core Plan and Node Execution Contract retain their original
`dispatch_authority_released=false` fields. Those fields describe the
artifacts when they were created; request preparation must not rewrite them.

Version 3 is not `ready`, `claimed`, `running`, `dispatched`, `completed`, or
`dispatch_unknown`. It says only that exact provider request bytes exist and
await a separate Go authorization decision.

## Exact provider request

Version 1 supports only the contract's `openai_responses` provider kind. The
application reconstructs one logical `ModelRequest` from the admitted
contract:

- `system_prompt` is the exact frozen `system_prompt`;
- `messages` contains exactly one user message with the exact frozen
  `user_prompt`;
- `tools` is empty;
- `max_output_tokens` equals the frozen contract budget; and
- cancellation is local runtime state and is not serialized.

The infrastructure adapter's existing pure codec encodes canonical compact
JSON equivalent to:

```json
{
  "include": ["reasoning.encrypted_content"],
  "input": [
    {
      "content": "<exact frozen user_prompt>",
      "role": "user",
      "type": "message"
    }
  ],
  "instructions": "<exact frozen system_prompt>",
  "max_output_tokens": 1,
  "model": "<exact frozen model>",
  "store": false,
  "stream": true,
  "tools": []
}
```

The numeric example above represents the contract value; it is not a default.
Object keys are recursively sorted by the existing Responses canonical codec,
strings retain their exact UTF-8 content, and the output has no trailing LF.
Preparation calls only the pure encoding and exact-validation functions. It
does not construct a provider or HTTP client.

The body must be non-empty, valid canonical UTF-8 JSON, and no larger than the
versioned request-body bound. Its exact bytes are persisted before any future
claim. Re-encoding from a logical request at send time is never a substitute
for the persisted body.

## Durable request artifact

Schema v11 adds an independent
`group_agent_graph_node_dispatch_requests` table. One Graph Run and one
admitted contract can each have at most one request. Its exact physical
columns, in schema order, are:

```text
id → graph_run_id → contract_id → request_version → codec_protocol_version
node_id → attempt → contract_sha256 → request_sha256 → project_lane_sha256
provider_kind → endpoint → model → destination_sha256 → pricing_snapshot_sha256
provider_request_blob → provider_request_bytes → provider_request_sha256
dispatch_request_sha256 → expected_last_event_seq → expected_last_event_sha256
idempotency_key → created_at_ms
```

The physical `request_sha256` is the admitted logical-request identity;
`provider_request_blob`, `provider_request_bytes`, and
`provider_request_sha256` are exposed by the protocol as `request_body`,
`request_body_bytes`, and `request_body_sha256`. `id` is the
`dispatch_request_id`. The blob is the exact canonical byte vector, not a
newly serialized copy. Endpoint, model, Prompts, and body are private plaintext
in the local Hub database.

The exact body identity is:

```text
SHA-256(
  "forge.group-agent-node-provider-request.v1\0"
  || request_body
)
```

The dispatch-request digest binds the canonical metadata payload and the exact
body identity:

```text
SHA-256(
  "forge.group-agent-node-dispatch-request.v1\0"
  || canonical_json(dispatch_request_payload)
)
```

The canonical `dispatch_request_payload` has this exact field order:

```text
v → codec_protocol_version → graph_run_id → contract_id → contract_sha256
expected_last_event_seq → expected_last_event_sha256 → node_id → attempt
project_lane_sha256 → provider_kind → endpoint → model → destination_sha256
logical_request_sha256 → pricing_snapshot_sha256 → request_body_bytes
request_body_sha256
```

`logical_request_sha256` is the admitted contract's
`request.request_sha256`. `dispatch_request_id` is
`node-dispatch-request-` followed directly by the full lowercase
dispatch-request digest. The payload excludes `dispatch_request_id`,
`dispatch_request_sha256`, idempotency key, and creation time. Candidate time
is not semantic.

The destination identity is:

```text
SHA-256(
  "forge.group-agent-node-destination.v1\0"
  || canonical_json({
       "v": 1,
       "provider_kind": provider_kind,
       "endpoint": endpoint,
       "model": model
     })
)
```

The object keys above are also the exact canonical field order. This digest
binds destination plaintext; it does not hide or attest it.

The table has source, recency, and project-lane indexes for validation and
future claim checks. A project-lane index is not a lane claim: several Runs may
prepare requests for the same Project. Exclusive lane ownership is acquired
only by a future atomic dispatch claim.

## Preparation event

Preparation appends exactly one canonical seq-3 event:

```text
v → graph_run_id → seq = 3 → type = node_dispatch_request_prepared
previous_event_sha256
contract_id
contract_sha256
dispatch_request_id
dispatch_request_sha256
request_body_sha256
request_body_bytes
logical_request_sha256
node_id
attempt
project_lane_sha256
codec_protocol_version
provider_kind
destination_sha256
pricing_snapshot_sha256
prepared_at_ms
```

`destination_sha256` is a domain-separated identity over provider kind,
endpoint, and model. It avoids copying destination plaintext into the compact
event but is not anonymization. The event's `previous_event_sha256` must equal
the recomputed seq-2 journal head.

No event field says that consent was granted, a credential was checked, a
provider was contacted, or dispatch authority was released.

## Validation and atomic admission

`prepare` performs no external preflight. Before constructing a candidate it
fully inspects and validates the Graph Run, Graph, frozen Group Run and member
bindings, canonical manifest, Core Plan, complete journal, admitted contract,
and the contract's reconstructed base control snapshot. It then:

1. requires the exact v2 `awaiting_core_dispatch` state;
2. recomputes the seq-2 event digest rather than trusting a projection;
3. revalidates first-node selection, authored index, wave zero, and attempt 1;
4. revalidates project-lane identity and `exclusive_until_terminal` policy;
5. reconstructs the exact manager/task/acceptance Prompts and logical request;
6. requires `workspace:none`, zero tools, zero predecessor receipts, one turn,
   and no Conversation/Prompt/memory writeback;
7. revalidates provider kind, canonical endpoint, model, `store:false`,
   `stream:true`, every budget, pricing-snapshot identity, and result bound;
8. requires fresh future consent and the frozen `dispatch_unknown`, no-lease,
   no-automatic-retry failure policy; and
9. encodes the exact Responses body and validates it by exact re-encoding
   through the same versioned production codec, then validates every
   domain-separated digest.

The store repeats all durable-source validation in one `BEGIN IMMEDIATE`
transaction. It looks up the idempotency key first, reconstructs the complete
source through the same SQLite connection, and reads the parent Run before the
candidate contract. A Run projection that declares a contract or request whose
child row is missing is stored corruption, not a candidate conflict or source
`NotFound`. The store compares expected seq 2 and its exact head digest,
inserts the request and seq-3 event, updates only the Run to the exact v3 state,
and rereads the complete aggregate before commit.

A seq-only check is insufficient. A locally self-consistent replacement of
event 2 must fail because its recomputed head no longer matches the candidate.
Any late insert, update, reread, or validation failure rolls back the request,
event, and Run transition together.

## Idempotency and corruption

An explicit or generated preparation idempotency key names only this local
request-preparation operation. Inside `BEGIN IMMEDIATE`, replay resolution is
ordered: look up the key first; fully validate any stored request, exact body,
contract, source, and journal before comparing the candidate; return corruption
before conflict or replay; and only then return the original stored artifact.
When no key exists, request-ID, Run, and contract uniqueness checks precede a
fresh v2 source validation and CAS create.

- Same key and same semantic input returns the original request identity,
  body, event, and time without consulting mutable latest history.
- Same key with different contract, source, body, codec, destination, pricing,
  or cursor conflicts.
- A second key for a Run that already has a request conflicts.
- Reusing a request identity for different bytes conflicts.
- A stale seq/head, divergent canonical bytes, or changed source fails closed.
- Stored corruption is reported as corruption before an input conflict or
  replay disposition.

Replay never changes authority and never creates a second event. It is safe
only because preparation has no off-machine effect.

## CLI and internal API boundary

Rust exposes only local preparation and inspection:

```text
forge-runtime group graph run dispatch prepare GRAPH_RUN_ID
forge-runtime group graph run dispatch show DISPATCH_REQUEST_ID
  [--include-request]
forge-runtime group graph run dispatch list [GRAPH_RUN_ID] [--limit N]
```

The normal global `--idempotency-key` option applies to `prepare`. There is no
`send`, `release`, `claim`, `retry`, `resume`, `complete`, or `advance`
subcommand in this slice.

The application may expose a pure request builder and a request-preparation
store port. It must not expose a dispatch authority type yet. The existing
`AgentRuntime`, `ModelProvider`, `PreparedModelProvider::stream_prepared`, and
transport collector are not invoked by this path.

## Consent, credentials, and preflight

Preparation intentionally accepts no consent flag. Consent obtained during a
local preparation command would be stale and replayable by the time a future
external effect occurs.

The path does not read `OPENAI_API_KEY` or any other credential. It does not
construct an Authorization header, provider, HTTP client, DNS query, TLS
connection, or health check. It verifies only the frozen endpoint grammar and
exact codec semantics already available locally.

Consequently, request preparation does not claim that:

- a credential exists or is accepted remotely;
- the destination is reachable or healthy;
- the model exists at that destination;
- the pricing snapshot is correct or current; or
- the maximum cost can already be enforced.

Those checks belong immediately before the future irreversible claim.

## Privacy and honesty

Default `prepare`, `show`, and `list` output hides:

- the exact provider body and logical request;
- system/user Prompts, manager instruction, task, and acceptance text;
- Project/member identifiers and project lane;
- endpoint, model, pricing digest, and idempotency key;
- raw event bytes, control snapshots, and local paths.

`list` is metadata-only and states that it did not revalidate source, contract,
event, request body, or pricing identity. Non-empty output says only that
metadata rows were returned; empty output does not infer preparation. `show`
performs complete validation. Explicit
`--include-request` reveals sensitive local plaintext and exact bytes; Human
output terminal-escapes controls, line separators, and bidi characters.

The database is private but unencrypted. Unkeyed SHA-256 values are
correlatable local content identities, not MACs, signatures, Go identity,
provider attestation, factual proof, or same-user database tamper protection.
`store:false` is a request field, not a privacy guarantee.

Fully validating `prepare` and `show` output states that exact request bytes are
present while dispatch authority is absent. Metadata-only `list` makes no such
attestation and says only that the list operation itself released no authority
or external effect. No output may say that a node, Agent, model, provider, tool,
network, workspace, or result ran or that any writeback occurred.

## Threat model

This slice fails closed against:

- stale or substituted Run, Graph, plan, contract, journal, Prompt, or request
  bytes;
- Go/Rust or logical/provider request serialization drift;
- unknown JSON fields, duplicate fields, noncanonical encodings, invalid UTF-8,
  oversized bodies, and digest-domain confusion;
- concurrent different-key preparation and partial SQLite commits;
- replay that silently follows newer Group history or membership;
- preparation being mistaken for consent, a lane claim, dispatch, execution,
  completion, or result provenance; and
- accidental disclosure through default CLI output or terminal controls.

Private-directory permissions, strict schema ownership, and complete aggregate
validation reduce accidental or structural local drift. They do not defend
against a malicious process with the same OS-user database access, a
compromised kernel, memory inspection, or an attacker who controls the future
provider. No network or provider threat is exercised because this slice makes
no external request.

## Required tests

The implementation must include at least:

1. a shared Go-contract-to-Rust-provider-body golden with exact bytes and no
   trailing LF;
2. exact Responses envelope assertions for Prompt, model, token limit, empty
   tools, `store:false`, `stream:true`, and fixed `include`;
3. domain tests for body/request/event digests, bounds, canonical JSON, and the
   three exact Run states;
4. application tests proving complete source and contract reconstruction,
   attempt/first-node/lane checks, and zero-capability policies;
5. SQLite v0-v10 to v11 migration, immutable historical DDL, complete catalog,
   and migration rollback tests;
6. seq-2/head CAS, concurrent same-key replay, concurrent different-key
   conflict, second-key conflict, identity reuse, and late-reread rollback;
7. locally self-consistent tamper cases for source, manager/task Prompt,
   contract, missing projected child rows, event head, provider body,
   destination, pricing, and stored byte counts;
8. exact replay preserving request/event identity, body, time, and digest;
9. CLI prepare/show/list, malformed arguments, oversized/non-UTF-8 input where
   applicable, terminal escaping, default redaction, explicit reveal, and
   metadata-only honesty;
10. side-effect sentinels proving no credential read, provider construction,
    network, AgentRuntime, tool, workspace, Conversation, Prompt, memory,
    result, or writeback operation; and
11. process-level tests with provider credential variables removed.

Tests may use the pure codec and deterministic in-memory fixtures. They must
not call a real LLM, provider, network, tool, or workspace.

## Rejected alternatives

- **Claim and then encode.** A local codec failure would happen after the
  irreversible uncertainty boundary.
- **Prepare and immediately use `AgentRuntime`.** It would import Project Run,
  Conversation, workspace, tool-loop, and writeback semantics that the Graph
  contract forbids.
- **Let Rust release because it encoded the body.** Serialization ownership is
  not scheduler authority; this would create a second release-decision owner.
- **Have Go duplicate the provider codec.** Go should validate the future
  release artifact, but two independent production encoders would create
  avoidable byte drift.
- **Treat preparation as a project-lane reservation.** No external effect has
  been authorized, and passive requests must not deadlock unrelated Runs.
- **Collect consent during prepare.** A replayable local consent bit is not
  fresh authorization for a later disclosure.
- **Run a provider health check.** It is an extra network effect and does not
  prove a later POST will succeed.
- **Store only a logical request or digest.** Future claim must hand out the
  exact persisted bytes, not a best-effort reserialization.

## Future claim-before-effect slice

The next effectful slice must begin from a fully validated v3 Run and preserve
the following order:

1. Rust exports a private canonical release-control snapshot containing the
   complete source, contract, seq-3/head, and exact prepared request.
2. Go Core independently revalidates the source, journal, contract, first
   node/attempt, project lane and exclusivity policy, exact request semantics,
   destination/model, budgets, pricing identity, zero capabilities, fresh
   consent requirement, and no-retry policy.
3. Go emits a canonical, content-addressed release authorization bound to the
   exact request and expected seq-3/head. Its SHA-256 is not a signature.
4. The final dispatch invocation obtains fresh, disclosure-specific
   off-machine consent and performs local credential/header, registered exact
   destination/model, exact body, pricing-policy, and budget preflight. It does
   no provider health request.
5. SQLite fully revalidates the same source in one immediate transaction,
   enforces global active project-lane exclusion, and CAS-appends
   `node_dispatch_released` from the exact seq-3/head.
6. The Run immediately becomes `dispatch_unknown` with authority true, and
   only the claim winner receives a non-`Clone`, non-serializable, consuming
   authority over the exact persisted bytes. Later callers receive redacted
   `AlreadyClaimed` state and never receive bytes.

The non-`Clone` type is an in-process ownership guard, not cryptographic
non-copyability. No timeout, cancellation, EOF, process crash, lease expiry,
provider error, or result-persistence failure may authorize an automatic
resend. A terminal result artifact, Core-validated receipt, lane release,
failure propagation, successor selection, and wave advance require later
contracts.

## Explicit non-goals

This ADR does not add:

- consent, credential access, dispatch claim, provider invocation, network I/O,
  retry, lease, cancellation recovery, or remote idempotency;
- `AgentRuntime`, manager or node execution, tools, workspace read/write,
  process execution, or OS sandboxing;
- result artifacts, terminal receipts, task status, failure propagation,
  project-lane ownership, node completion, successor selection, or wave
  advance;
- Conversation/Prompt/task/memory writeback, current-history lookup, remote
  account/sync/ACL, multi-Agent discussion, or Web UI; or
- proof of current pricing, enforceable cost, model availability, provider
  acceptance, factual correctness, or external attestation.

The version-3 terminal condition for this slice is exact request presence with
dispatch authority still absent.
