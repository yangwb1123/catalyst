# ADR-0026: Passive schedule-bound initial-node contract candidate

- Status: Accepted for the effect-free schema-v14 initial-candidate slice
- Date: 2026-08-01
- Extends: [ADR-0025](0025-passive-multi-node-execution-schedule.md)

## Context

ADR-0025 freezes one Core-owned serial schedule for a multi-node Graph, but it
deliberately creates no contract and observes no progress. The existing node
contract and terminal lifecycle cannot simply consume that schedule. Contract
v1 is one-per-Run and occupies main-journal sequence 2, while terminal receipt
v1 proves that its only node, wave, and Graph become terminal together. Reusing
either artifact for the first node of a multi-node Graph would manufacture
Graph completion and leave no honest successor state.

The first schedule-bound contract also has a special property: Core's selected
ordinal is zero, so the schedule proves that its direct-predecessor set is
empty. It can establish the empty-receipt base case without inventing a
predecessor receipt. A noninitial contract cannot be built until a later
terminal protocol persists real per-node evidence and Core verifies completed
contiguous-prefix progress at the current durable head.

## Decision

Add a passive, content-addressed `ScheduledNodeContractCandidate` v2 for only
the initial node of an admitted multi-node schedule. It is a complete frozen
logical request and policy candidate, but it is not the Graph Run's admitted
lifecycle contract and cannot be dispatched.

Creating or persisting the candidate:

- does not change the Graph Run, main journal, status, version, or head;
- does not create the legacy contract-v1 event or a provider dispatch request;
- does not release execution, dispatch, Project-lane, workspace, tool, network,
  writeback, successor, or retry authority;
- does not read a credential, construct a provider, or access a workspace;
- observes no node result, terminal receipt, progress, or successor decision;
  and
- does not remove the ADR-0024 multi-node dispatch fence.

The public offline flow is:

```text
forge-runtime group graph run control export GRAPH_RUN_ID > control.json
forge graph-scheduled-node-contract --control control.json \
  --schedule-sha256 SHA256 \
  --endpoint HTTPS_URL --model MODEL \
  --max-output-tokens N --max-model-output-bytes N --max-model-events N \
  --timeout-ms N --max-cost-usd-micros N \
  --pricing-snapshot-sha256 SHA256 --max-result-bytes N > candidate.json
forge-runtime group graph run scheduled-contract admit GRAPH_RUN_ID \
  --contract candidate.json --idempotency-key KEY
forge-runtime group graph run scheduled-contract show CONTRACT_ID \
  [--include-contract]
forge-runtime group graph run scheduled-contract list [GRAPH_RUN_ID] [--limit N]
```

## Core ownership and exact input

Core accepts the existing exact private control-v1 snapshot, one required
lowercase schedule SHA-256, and the same explicit bounded execution options as
contract v1. There is no caller-supplied node, ordinal, attempt, predecessor,
receipt, schedule body, workspace, tool, or policy flag.

Core independently rebuilds the unique ADR-0025 schedule from the control and
requires its digest to equal `--schedule-sha256`. It then selects exactly
`schedule.nodes[0]` and requires:

```text
execution_ordinal = 0
attempt = 1
node_id = schedule.initial_node
direct_predecessor_node_ids = []
expected_last_event_seq = 1
expected_last_event_sha256 = control.last_event_sha256
```

This avoids treating a caller-provided schedule as scheduler authority. Rust
later requires the same schedule identity to be durably admitted and fully
validates its stored canonical bytes.

## Candidate and request v2

The canonical candidate binds:

- artifact v2, scheduler v1, schedule v1, and node-execution protocol v2;
- `contract_scope = schedule_initial_node_only`;
- Graph Run, Graph, source snapshot, manifest, plan, and control identities;
- schedule ID and digest;
- the exact current sequence-1 durable head;
- ordinal zero and the exact scheduled node identity, authored index, wave,
  attempt, Project/member/profile labels, Project lane, and same-Project policy;
- fixed no-workspace, provider destination, budgets, approval, bounded-result,
  no-dataflow, no-writeback, and no-retry policies; and
- a logical request-v2 identity and the final contract identity.

The logical request-v2 binds the Graph Run, schedule, ordinal, node, attempt,
exact system and user Prompt bytes and digests, empty required-predecessor
nodes, empty predecessor terminal receipts, `predecessor_content_included=false`,
and an empty tool list. Receipt identities are contract evidence, not provider
Prompt content. Even a future opaque receipt ID is metadata disclosure and
must not be copied into the provider body merely because an edge exists.

The candidate fixes these honesty flags to false:

```text
lifecycle_contract_admitted
provider_request_present
execution_authority_released
dispatch_authority_released
progress_observed
successor_advance_authorized
```

Request and contract digests use domain-separated v2 domains:

```text
forge.group-agent-scheduled-node-request.v2\0
forge.group-agent-scheduled-node-contract.v2\0
```

The complete candidate is compact canonical UTF-8 without a trailing line
feed. Its content ID is derived from the contract digest. Digests are local
content identity, not a signature, attestation, factual verification, or
same-user tamper protection.

## Predecessor receipt fence

Schema v14 and the Core command accept only the empty-predecessor initial case.
Any nonzero ordinal, nonempty required-predecessor list, nonempty terminal
receipt array, or claimed progress is invalid.

Terminal receipt v1 is never accepted as a predecessor receipt: it proves a
single-node Graph terminal state, not intermediate multi-node progress. A
later protocol must first define and persist an intermediate terminal receipt
and Core successor decision. Each future direct-predecessor slot must bind at
least predecessor node and attempt, terminal event sequence and digest, receipt
ID and digest, and a completed outcome, all verified against durable evidence
and the schedule's contiguous-prefix rule. Result content remains dataflow
`none`; transmitting content requires a separate byte-bound disclosure and
consent contract.

## Durable sidecar and mutual exclusion

Schema v14 adds an immutable scheduled-contract-candidate table and leaves the
legacy contract-v1 table and all lifecycle tables unchanged. The row stores the
exact canonical candidate plus queryable Graph Run, schedule, control, head,
ordinal, node, attempt, lane, request, contract, empty-predecessor count, false
authority/progress flags, idempotency key, and local admission time.

Version 14 supports one initial candidate per Graph Run and schedule. It also
enforces the explicit `(Graph Run, node, attempt)` and
`(schedule, ordinal, attempt)` identities. The Project lane is not unique,
because a candidate is not a lane claim. Supporting more nodes later requires
a new migration and protocol; v14 does not claim that an extensible table is an
implemented successor lifecycle.

The candidate family and legacy v1 contract family are mutually exclusive.
Candidate admission rejects any legacy contract/request/lifecycle advancement,
and legacy contract admission checks the v14 sidecar and rejects a candidate
for the same Run. Both decisions occur under `BEGIN IMMEDIATE`, so a v1/v2 race
has exactly one family winner. This cross-table fence is mandatory because the
candidate deliberately leaves the Run at sequence 1.

Within its write transaction, candidate admission:

1. validates any matching stored key, ID, slot, or Run row before deciding
   replay or conflict;
2. loads and completely validates the current Graph Run, source Graph, and
   exact stored schedule in the same snapshot;
3. independently reconstructs the canonical control and schedule;
4. requires pristine v1/sequence-1 state and no legacy lifecycle child;
5. validates every candidate byte, identity, policy, bound, digest, and false
   flag;
6. inserts and rereads the exact sidecar; and
7. commits only after full source-bound validation succeeds.

An exact same-key replay returns the original bytes and admission time only
while the source is still eligible. A stale head, changed input, another key,
or another contract family conflicts. Stored corruption is never downgraded to
replay or conflict. Application code validates the transaction result against
its candidate but does not use a post-commit reread as an atomicity witness.

## Inspection and compatibility

Default show/list output is metadata-only and hides Prompt, node/member/profile,
Project lane, provider settings, budgets, predecessor fields, and idempotency
key. `--include-contract` explicitly reveals the fully validated artifact.
Every view states that it is a passive candidate and
`current_run_lifecycle_included=false`; its false flags never describe a later
current Run.

Migration v13 to v14 only creates the empty sidecar structures and mutual-
exclusion lookup support. It preserves schedules, legacy contracts, requests,
active v4 claims and lanes, artifacts, receipts, journal bytes, and WAL state.
Dispatch preflight explicitly continues to accept exact schemas 11, 12, 13,
and 14. Hot-WAL no-send reentry continues to accept exact schemas 12, 13, and
14. Version-dependent child queries do not reference the new table before v14.

Existing dispatch preparation, authorization, readiness, and execution remain
contract-v1 only. They reject candidate v2 and multi-node topology before
consent, credential access, provider construction, network, or database writes.

## Required verification

1. shared Go/Rust canonical candidate, request and digest fixture for the
   frontend/backend/SSO diamond;
2. Core rebuild of the schedule, deterministic ordinal-zero selection, empty
   predecessor sets, and no caller node/receipt option;
3. strict malformed, duplicate, unknown, missing, null, reordered, trailing,
   invalid UTF-8, oversized, Unicode, option, domain, digest and identity
   rejection;
4. cross-Run, cross-schedule, stale-head, source/plan/control, ordinal, node,
   attempt, lane, false-flag, and any nonempty predecessor substitution
   rejection;
5. exact replay, divergent-key/input conflict, concurrent v2 candidates,
   concurrent v1-v2 admission with exactly one family, and corruption-first
   behavior;
6. v13-to-v14 migration, rollback, structural schema ownership, active-v4/lane
   preservation, and exact v11-v14 read-only compatibility;
7. default-redacted and explicit-reveal CLI behavior; and
8. byte-exact proof that admission leaves the Graph Run and main events
   unchanged and touches no credential, provider, network, workspace, tool,
   result, receipt, successor, Prompt, Conversation, task, or memory state.

## Rejected alternatives

- **Reuse the main sequence-2 contract event now.** The current Run readers,
  request path, and terminal-v1 semantics encode a one-node Graph lifecycle;
  consuming the head would imply an executable transition before its terminal
  and successor states exist.
- **Persist a generic contract-v2 sidecar.** Version 14 can only prove the
  empty-predecessor initial case; a generic name would overstate capability.
- **Accept the caller's schedule body or selected node.** Core owns both the
  deterministic schedule and initial selection.
- **Allow both candidate v2 and contract v1.** They are competing initial-node
  contracts with different safety semantics.
- **Use a terminal-v1 receipt as a predecessor.** It terminalizes the Graph and
  cannot prove intermediate progress.
- **Put receipt IDs into the provider Prompt.** Ordering evidence is not
  content or metadata disclosure consent.
- **Make the Project lane unique.** No lane has been claimed and no effect
  authority exists.

## Consequences

Multi-node Graphs gain one durable, fully budgeted initial-node contract-v2
candidate that is cryptographically bound to the Rust-admitted Core schedule
and the real pristine durable head. Cross-family races cannot create competing
v1 and v2 initial contracts, and no synthetic predecessor evidence is
introduced.

The Graph still cannot execute multiple nodes, dispatch this candidate,
terminalize an intermediate node, select or admit a successor, transmit
predecessor content, run a manager discussion, or adjudicate a hard-crashed v4
claim. Those remain separate protocols and must not be inferred from candidate
presence.
