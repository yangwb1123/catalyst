# ADR-0018: Core-owned Group Agent Graph Run plan

- Status: Accepted for the passive run-plan slice
- Date: 2026-07-30
- Extends: [ADR-0017](0017-durable-group-agent-graph.md)
- Upstream reference: Pi Coding Agent
  [overview](https://pi.dev/docs/latest),
  [session format](https://pi.dev/docs/latest/session-format),
  [RPC](https://pi.dev/docs/latest/rpc), and
  [security](https://pi.dev/docs/latest/security), inspected 2026-07-29

## Context

ADR-0017 freezes an exact Group, authored task nodes, dependency edges, and
deterministic topology waves. It deliberately does not define an executable
Agent assignment. In particular, an `agent_profile` is only a label, an edge
only constrains order, and a wave is not evidence that any node is safe to
dispatch.

The next boundary must preserve one orchestration owner:

- `forge-core` owns dependency planning and every future release, completion,
  failure-propagation, and stop decision;
- the Rust Hub validates and durably records exact artifacts but never chooses
  the next node or wave;
- the Rust Agent Runtime will eventually execute one fully authorized node
  envelope;
- a TypeScript client may compose the commands and present their state without
  adding another autonomous loop.

This is not merely a layering preference. Graph version 1 does not bind a
provider, model, system Prompt, budget, tool capability, workspace consent,
result schema, predecessor result, isolation identity, approval, or retry
policy. Multiple nodes may also target the same project and appear in the same
dependency wave. Topological independence therefore does not prove resource or
workspace independence.

Starting an Agent now would either invent those missing contracts or reuse a
Project Run that requires an unrelated Conversation and user Prompt. Calling a
topology projection `running`, `ready to dispatch`, or `completed` would be
false.

## Decision

Sprint 47 adds a passive, cross-language Core Plan and a durable Graph Run plan
receipt. It does not add execution authority.

The Go control plane exposes:

```text
forge graph-plan --graph-id GRAPH_ID
                 --manifest-sha256 SHA256
                 [--input FILE|-]
```

The input is the bounded version-1 Group Agent Graph authoring specification.
Go uses the same dependency-wave implementation that backs the existing
workflow orchestrator. It emits one strict canonical JSON plan.

The Rust Hub consumes and freezes that plan:

```text
forge-runtime group graph run prepare GRAPH_ID
              --plan FILE|-
              [--idempotency-key KEY]

forge-runtime group graph run show GRAPH_RUN_ID [--include-plan]
forge-runtime group graph run list [GRAPH_ID] [--limit N]
```

The explicit file or standard input is the only additional content read.
Neither command discovers a workspace, reads a provider credential, or invokes
an Agent.

### Core Plan contract

The version-1 plan contains, in canonical field order:

```json
{
  "v": 1,
  "scheduler_protocol_version": 1,
  "graph_version": 1,
  "graph_id": "graph-id",
  "graph_manifest_sha256": "lowercase-sha256",
  "authored_node_ids": ["frontend", "backend", "sso"],
  "edges": [
    {"from_node_id": "backend", "to_node_id": "sso"},
    {"from_node_id": "frontend", "to_node_id": "sso"}
  ],
  "waves": [["frontend", "backend"], ["sso"]],
  "execution_contract_present": false,
  "dispatch_authority_released": false,
  "plan_sha256": "lowercase-sha256"
}
```

Authored node order is semantic. Edge input order is not; Go sorts edges by
`(from_node_id, to_node_id)` before planning. Waves preserve authored node
order within each dependency layer.

The digest is:

```text
SHA-256(
  "forge.group-agent-graph-core-plan.v1\0"
  || canonical_json(plan_without_plan_sha256)
)
```

Canonical JSON is UTF-8, compact, field ordered, and does not HTML-escape
ordinary Unicode text. The final stored plan must equal its own canonical
re-encoding byte for byte. A shared Go/Rust golden fixture prevents the two
implementations from silently defining different bytes or digests.

The graph manifest digest binds all fields intentionally omitted from the Core
Plan, including manager instruction, project assignments, roles, Agent profile
labels, tasks, acceptance text, and exact Group Run source. Rust accepts a plan
only when its graph ID, graph version, manifest digest, authored node IDs,
canonical edges, and waves exactly match a fully revalidated stored Graph.

The two `false` fields are protocol invariants, not advisory UI labels. Version
1 cannot represent an execution contract or dispatch authority.

### One dependency algorithm

The authored-order Kahn implementation is extracted into an inward Go
dependency package. The workflow orchestrator adapts `asset.Phase` values to
that package; the Group plan builder uses the same package. It is forbidden to
copy the loop into a second Graph-specific scheduler.

Rust still recomputes waves while validating the immutable manifest and Core
Plan. That is fail-closed contract validation, not scheduling. Rust exposes no
`next_ready_wave`, `claim_ready_node`, `advance_wave`, lease-reclaim, or
automatic retry API.

### Durable passive Graph Run

SQLite schema v9 adds:

- `group_agent_graph_runs`;
- `group_agent_graph_run_events`;
- indexes by source Graph and creation time.

The complete v9 owned catalog contains 22 tables, 16 named explicit indexes,
and 38 implicit autoindexes. Its release-pinned v1-v9 length-framed DDL
SHA-256 is
`4d56c12494001f4584ce021a02c3729afc6c97dc292dfff2edaa91716aa16eab`;
the independent v9 structural-contract SHA-256 is
`c9bd523268ade499fe446673a3baa25543f6268c963d502112fd14a607300607`.

A Graph Run has the single status `awaiting_execution_contract`. It binds:

- the exact Graph ID, source snapshot digest, and manifest digest;
- scheduler protocol version;
- exact canonical Core Plan bytes, byte count, and digest;
- node and wave counts;
- a one-event append-only journal cursor;
- `execution_contract_present = 0`;
- `dispatch_authority_released = 0`;
- idempotency key and creation time.

The sole version-1 event is `graph_run_prepared`. It binds the Run, Graph,
manifest, plan, protocol, and creation time. The event is canonical,
content-digested, and committed with the Run. It is a recoverable receipt that
an exact plan was admitted; it is not a node or wave transition.

The event-table digest is
`SHA-256("forge.group-agent-graph-run-event.v1\0" ||
canonical_event_json)`. It is stored as raw 32-byte SQLite data and is not
embedded back into the event JSON.

No mutable node/wave projection is stored in this version. `waves[0]` means
only that its nodes have no graph predecessors. It is not named
`dispatch_ready`, and it grants no authority.

### Atomicity and replay

Preparation uses one `BEGIN IMMEDIATE` transaction:

1. look up the idempotency key;
2. for both replay and creation, load and fully validate the exact Graph,
   referenced Group Run, frozen member bindings, manifest bytes, and digests;
3. validate the canonical Core Plan against that Graph;
4. for a new key, insert the Run and its initial event;
5. reread and completely validate the Run, plan, event, cursor, and Graph;
6. commit only after the complete reread succeeds.

An exact same-key replay returns the original Run identity, time, plan, and
event. Candidate identity and time are not semantic. Reusing a key for another
Graph or plan conflicts. Stored corruption remains corruption and is never
downgraded to an idempotency conflict.

`show` validates the Run, event, plan, and Graph in one deferred SQLite
snapshot. `list` is metadata-only and says that it did not validate the plan,
journal, or source Graph.

### Privacy and honesty

The Core Plan contains graph and node identifiers, edges, waves, and
correlatable unkeyed digests. It omits tasks, instructions, roles, project IDs,
paths, and result content, but it is not anonymous or safe to publish by
default.

Default prepare/show output hides plan content and idempotency keys.
`--include-plan` deliberately reveals the fully validated Core Plan. Human
output terminal-escapes revealed text. List never includes the plan.

Every output states that:

- the execution contract is absent;
- dispatch authority was not released;
- manager and node Agents did not run;
- no model, provider, network, tool, workspace, task result, Conversation,
  Prompt, memory, or writeback operation occurred.

The plan digest and event receipt are local content identities, not signatures,
Agent identity, remote attestation, or evidence of task completion.

## Rejected alternatives

- **Let Rust create and advance node readiness.** That gives the Hub a second
  scheduler and allows its projection to drift from the Go control plane.
- **Persist `ready`, `running`, or `completed` node rows now.** None of those
  states is authorized by Graph version 1.
- **Begin a Project Run for every node.** Project Runs require a Project
  Conversation and existing user Prompt and would invent writeback semantics.
- **Release a no-tool model call.** Provider selection, exact Prompt bytes,
  budgets, workspace behavior, and result ownership are still undefined.
- **Use a timeout lease.** Once an external effect is released, expiry cannot
  prove it did not happen and must never authorize an automatic resend.
- **Maintain separate Go workflow and Group Graph algorithms.** Divergent wave
  order would make recovery and cross-language digest validation ambiguous.
- **Store only `graph_id`.** An exact manifest digest and plan digest are needed
  to detect same-ID or same-user database drift.

## Consequences and next slice

Sprint 47 creates a real Go-to-Rust planning seam and durable admission receipt
without pretending the system can execute the plan.

The first effectful slice must add a separately versioned Node Execution
Contract that freezes at least:

- Run, node, attempt, Graph, manifest, Core Plan, and request identities;
- exact system/user input bytes and predecessor receipt provenance;
- provider, endpoint, model, tool, token, turn, byte, time, and cost budgets;
- explicit `none`, read-only, or isolated-mutable workspace authority;
- same-project serialization or an exact isolated worktree/sandbox identity;
- approval, result artifact, writeback, cancellation, and failure rules.

Only `forge-core` may then append decisions such as
`NodeDispatchReleased` and `WaveCompleted` through a passive compare-and-swap
journal port. A committed dispatch without an exact terminal receipt derives
`dispatch_unknown`; it never expires into a retry. Rust may reject an invalid
transition but may not select it. Graph edges remain ordering-only until a new
dataflow contract explicitly carries predecessor results.

Remote accounts, synchronization, shared ACL, TypeScript UI, live
multi-Agent discussion, manager re-planning, derived memory, mutating tools,
and operating-system isolation remain separate capabilities.
