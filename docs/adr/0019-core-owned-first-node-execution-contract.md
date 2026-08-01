# ADR-0019: Core-owned first-node execution contract

- Status: Accepted for the passive v10 contract-admission slice
- Date: 2026-07-30
- Extends: [ADR-0018](0018-core-owned-group-agent-graph-run-plan.md)

## Context

ADR-0018 establishes one Go-owned topology plan and a Rust-persisted passive
Graph Run. The Run deliberately stops at `awaiting_execution_contract`.
Neither a topology wave nor a profile label authorizes a model, tool, network,
workspace, result, or writeback effect.

The first executable boundary still lacks:

- exact model request bytes and provider destination;
- token, byte, event, time, and cost budgets;
- a workspace/isolation identity and same-project concurrency rule;
- tool capabilities and result provenance;
- approval, retry, cancellation, and post-claim uncertainty rules;
- a compare-and-swap cursor proving which scheduler state was consumed.

Multiple authored nodes can target one Project and share one topology wave.
Dispatching all of wave zero would therefore be unsafe before isolation or
serialization exists.

Reusing the existing Project `RunService` is also incorrect. Project Runs are
bound to a Conversation, user Prompt, workspace, and optional assistant Prompt
writeback. A Graph node has none of those semantics. The low-level Rust
`AgentRuntime` may be reusable later, but its existing adapter is not.

## Decision

Sprint 48 remains effect-free. It freezes and admits exactly one complete Node
Execution Contract, but releases no dispatch authority.

The cross-language flow is:

```text
Rust exports one fully validated canonical control snapshot
  -> Go validates it and selects plan.waves[0][0]
  -> Go emits one canonical Node Execution Contract
  -> Rust admits it by exact seq/head CAS
  -> Graph Run stops at awaiting_core_dispatch
```

Go remains the only scheduler. Rust reconstructs the same selection only as
fail-closed validation; it exposes no next-node, advance-wave, claim, retry, or
execution API.

Version 1 is deliberately global single-flight:

- only the first authored node in topology wave zero is eligible;
- the attempt is exactly 1;
- one Graph Run can contain only one admitted contract;
- workspace mode is `none`;
- tools and predecessor receipts are empty;
- model turns are limited to one;
- fresh off-machine consent is required by any future provider dispatch.

This prevents same-Project concurrency even when two independent nodes share a
wave. Later protocols may select another node only after exact predecessor
terminal receipts and isolation or database-wide project-lane exclusion exist.

## Canonical control snapshot

Rust exposes:

```text
forge-runtime group graph run control export GRAPH_RUN_ID
```

The command writes exact compact UTF-8 JSON without a trailing LF. It is a
private export: it contains manager instructions, Project/member identifiers,
node tasks, and acceptance text. It is never included in default show/list
output.

The version-1 fields, in order, are:

```text
v
scheduler_protocol_version
graph_run_version
graph_run_id
graph_id
source_snapshot_sha256
graph_manifest_sha256
core_plan_sha256
last_event_seq
last_event_sha256
execution_contract_present
dispatch_authority_released
plan
manifest
snapshot_sha256
```

Only a fully revalidated v1 Graph Run with status
`awaiting_execution_contract`, seq 1, and both authority flags false can be
exported. `plan` and `manifest` are the exact validated v1 Core Plan and Group
Agent Graph Manifest.

The snapshot digest is:

```text
SHA-256(
  "forge.group-agent-graph-control-snapshot.v1\0"
  || canonical_json(snapshot_without_snapshot_sha256)
)
```

The snapshot is bounded to 4 MiB. The seq-1 head digest is recomputed from the
stored preparation event; it is not trusted from a projection column.

## Go selection and contract command

Go exposes:

```text
forge graph-node-contract --control FILE|-
  --endpoint HTTPS_URL
  --model MODEL
  --max-output-tokens N
  --max-model-output-bytes N
  --max-model-events N
  --timeout-ms N
  --max-cost-usd-micros N
  --pricing-snapshot-sha256 SHA256
  --max-result-bytes N
```

It requires exact canonical snapshot bytes and validates all Run, Graph,
manifest, plan, node-order, edge, wave, digest, cursor, and authority
bindings. The caller cannot choose a node. Version 1 always selects
`plan.waves[0][0]`.

The Node Execution Contract fields, in canonical order, are:

```text
v
scheduler_protocol_version
node_execution_protocol_version
graph_run_id
graph_id
source_snapshot_sha256
graph_manifest_sha256
core_plan_sha256
control_snapshot_sha256
expected_last_event_seq
expected_last_event_sha256
node
workspace
provider
request
budgets
approval
result
failure
execution_contract_present
dispatch_authority_released
contract_id
contract_sha256
```

`node` contains:

```text
node_id
authored_node_index
topology_wave_index
attempt
project_id
member_role
agent_profile
project_lane_sha256
same_project_policy
```

The attempt is 1 and `same_project_policy` is
`exclusive_until_terminal`. The project lane digest is:

```text
SHA-256(
  "forge.group-agent-project-lane.v1\0"
  || UTF8(project_id)
)
```

`workspace` is fixed to:

```json
{
  "mode": "none",
  "root_identity": null,
  "isolation_id": null,
  "allowed_read_paths": []
}
```

`provider` contains the caller-pinned HTTPS endpoint and model, with:

```text
kind = openai_responses
store = false
stream = true
```

No credential is accepted, read, or stored.

The Go generator and Rust admission share one conservative byte-stable endpoint
grammar: lowercase `https://`, canonical lowercase DNS or dotted-decimal IPv4,
an optional canonical non-default port, and an empty or `/`-rooted path made
only of ASCII unreserved bytes. Userinfo, query, fragment, percent escapes, dot
segments, IPv6, Unicode/uppercase host spellings, and normalized/default ports
are rejected.

`request` contains:

```text
system_prompt
system_prompt_bytes
system_prompt_sha256
user_prompt
user_prompt_bytes
user_prompt_sha256
predecessor_result_receipts
tools
request_sha256
```

The predecessor and tool arrays are empty. Prompt byte counts use UTF-8 bytes;
their SHA-256 values are unkeyed identities over those exact bytes.

The system Prompt is exactly:

```text
Execute exactly one frozen Group Agent Graph node. Follow the manager
instruction, complete only the assigned task, and return a text result that can
be checked against the acceptance criteria. Tools, network, workspace access,
memory, and writeback are unavailable.

Manager instruction:
<exact manager instruction>
```

The line wrapping above is prose formatting only. The implementation freezes
the prefix as one line ending after `unavailable.`, followed by
`\n\nManager instruction:\n` and the exact instruction.

The user Prompt is canonical compact JSON with fields:

```text
v
node_id
task
acceptance
predecessor_result_receipts
```

The request digest is:

```text
SHA-256(
  "forge.group-agent-node-request.v1\0"
  || canonical_json(request_without_request_sha256)
)
```

`budgets` contains:

```text
max_turns = 1
max_tool_calls = 0
max_output_tokens
max_model_output_bytes
max_model_events
timeout_ms
max_cost_usd_micros
pricing_snapshot_sha256
```

These values are frozen for a future dispatcher. This sprint does not claim
that cost can already be enforced.

`approval` fixes provider dispatch to `fresh_off_machine_consent` and marks
workspace, tools, and writeback forbidden.

`result` fixes:

```text
artifact_kind = local_graph_node_artifact
max_result_bytes = caller bound
predecessor_dataflow = none
conversation_writeback = none
prompt_writeback = none
memory_writeback = none
```

`failure` fixes:

```text
automatic_retry = false
lease_retry = false
post_claim_uncertainty = dispatch_unknown
failure_propagation_owner = forge_core
```

The payload digest excludes `contract_id` and `contract_sha256`:

```text
SHA-256(
  "forge.group-agent-node-execution-contract.v1\0"
  || canonical_json(contract_payload)
)
```

`contract_id` is `node-contract-` followed by the full lowercase payload
digest. `contract_sha256` is that same digest. The complete contract must equal
its canonical re-encoding byte for byte and has no trailing LF.

## Durable v10 admission

SQLite schema v10 rebuilds the two Graph Run tables with exact v1/v2 state
combinations and adds `group_agent_graph_node_execution_contracts`.
The Hub-owned v10 catalog is fixed at 23 tables, 18 explicit indexes, and 41
implicit autoindexes. The release-pinned v1–v10 length-framed DDL SHA-256 is
`16752cf9b054b8e840a98976b06e8f2d015aca6f001191943d4ac54a237e352b`;
the independent v10 structural-contract SHA-256 is
`ce5383f44a3a982ab127608acda473d1531ff10fc4b6ca8e7036d84fdec75d8d`.

A legacy or newly prepared base Run remains:

```text
run_version = 1
status = awaiting_execution_contract
execution_contract_present = 0
dispatch_authority_released = 0
last_event_seq = 1
```

Admitting the first contract atomically transitions only that Run to:

```text
run_version = 2
status = awaiting_core_dispatch
execution_contract_present = 1
dispatch_authority_released = 0
last_event_seq = 2
```

Existing rows are not batch-upgraded. The Core Plan remains the immutable v1
artifact whose two original authority fields are false.

The contract table stores exact canonical bytes, digest, request digest,
project lane, node/attempt, expected cursor/head, idempotency key, and original
creation time. `graph_run_id` is unique in version 1. Project-lane and recency
indexes support future claim checks and metadata inventories.

The seq-2 event is `node_execution_contract_admitted`, with fields:

```text
v
graph_run_id
seq
type
previous_event_sha256
control_snapshot_sha256
contract_id
contract_sha256
contract_bytes
node_id
attempt
request_sha256
project_lane_sha256
admitted_at_ms
```

Its digest is:

```text
SHA-256(
  "forge.group-agent-graph-run-control-event.v1\0"
  || canonical_event_json
)
```

Admission runs in one `BEGIN IMMEDIATE` transaction:

1. look up the idempotency key;
2. fully revalidate stored Run, event journal, Graph, frozen Group source,
   members, manifest, and Core Plan;
3. reconstruct and verify the exact control snapshot;
4. validate the contract, request, node, budgets, prompt construction, project
   lane, and all digests;
5. compare the expected seq/head and require the exact v1 base state;
6. insert the contract and seq-2 event;
7. update the Run to the exact v2 state;
8. reread and validate the complete aggregate before commit.

Same-key, same-contract replay returns the original contract identity, bytes,
event, and time. Candidate time is not semantic. Reusing a key with divergent
input, using a second key for the same Run, submitting a stale cursor/head, or
reusing a contract identity for different bytes conflicts. Stored corruption
is reported before an idempotency conflict.

## CLI and privacy

Rust exposes:

```text
forge-runtime group graph run contract admit GRAPH_RUN_ID
  --contract FILE|-
  [--idempotency-key KEY]

forge-runtime group graph run contract show CONTRACT_ID
  [--include-contract]

forge-runtime group graph run contract list [GRAPH_RUN_ID] [--limit N]
```

Malformed, noncanonical, oversized, or invalid contract input is rejected
before opening the Hub.

Default output hides the control snapshot, contract, Prompts, manager
instruction, task, acceptance, Project/member identifiers, model, endpoint,
pricing digest, idempotency key, and file path. Explicit contract inclusion
reveals sensitive plaintext and is terminal-escaped in Human output. List is
metadata-only.

Every output distinguishes:

- the Node Execution Contract is present;
- dispatch authority is absent;
- no manager or node Agent ran;
- no credential, provider, model, network, tool, workspace, result,
  Conversation, Prompt, memory, or writeback effect occurred.

Digests are local content identities, not signatures, Go identity,
attestation, or same-user database tamper protection.

## Rejected alternatives

- **Dispatch a deterministic fake Agent.** It would not verify the real
  provider, cost, recovery, or isolation boundary and would create misleading
  completion semantics.
- **Dispatch every wave-zero node.** Topology independence does not establish
  Project or resource independence.
- **Let the caller name the node.** Selection belongs to the Go scheduler.
- **Keep the Run at `awaiting_execution_contract`.** Once a contract exists,
  that status and `execution_contract_present=0` would be false.
- **Reuse Project Run persistence.** It would invent Conversation, Prompt, and
  writeback semantics.
- **Release a timeout lease.** Lease expiry cannot prove an external effect did
  not happen and cannot authorize resend.
- **Store only a child contract without cursor/head CAS.** That permits stale
  scheduler decisions to be admitted.

## Consequences and next slice

Sprint 48 creates an auditable, cross-language, fully budgeted execution
contract and a truthful Graph Run transition without executing it.

The next effectful slice must:

- require fresh explicit off-machine consent;
- revalidate provider destination, pricing policy, contract, source, seq/head,
  and project lane before claim;
- atomically append `NodeDispatchReleased` and return one non-cloneable
  exact-byte authority;
- enter `dispatch_unknown` immediately after the claim commits;
- use a new node-specific adapter around `AgentRuntime`;
- persist an exact result artifact and terminal receipt before Go may select a
  successor;
- never retry unknown dispatch because of time, cancellation, EOF, or lease
  expiry.

Parallel nodes, workspace read/write, OS isolation, predecessor dataflow,
manager execution/replanning, memory/writeback, remote sync/ACL, and live
multi-Agent discussion remain separate capabilities.
