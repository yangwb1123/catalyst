# ADR-0017: Durable local Group Agent Graph

- Status: Accepted for the graph-definition slice
- Date: 2026-07-29
- Upstream reference: Pi Coding Agent
  [overview](https://pi.dev/docs/latest),
  [session format](https://pi.dev/docs/latest/session-format),
  [RPC](https://pi.dev/docs/latest/rpc), and
  [security](https://pi.dev/docs/latest/security), inspected 2026-07-29

## Context

The Hub can already freeze one Group's Prompt history, produce independent
single-model analyses, assemble a panel, and persist one single-model
synthesis. None of those artifacts defines which Agent owns which project task
or how dependencies permit independent work to proceed.

Forge already has one orchestration owner. `forge-core` validates workflow
dependencies and schedules dependency waves. `forge-runtime` owns one task's
model/tool loop. Adding another autonomous scheduler inside the Rust Hub would
create two owners for retries, cancellation, budgets, and completion.

Pi remains a clean-room architectural reference. Its current public
documentation keeps a small Agent core behind explicit session, tool, event,
and RPC boundaries. It also distinguishes project trust from real operating
system isolation. Forge adopts those separation principles; no Pi source,
Prompt, UI asset, or product text is copied.

Before connecting `forge-core` to live per-node execution, Forge needs an
immutable and inspectable graph contract. The graph must bind an exact Group
Run, manager instructions, member-project assignments, tasks, dependency
edges, and deterministic waves without claiming any Agent has run.

## Decision

Hub schema v8 adds a local-only `GroupAgentGraph` artifact:

```text
forge-runtime group graph prepare GROUP_RUN_ID
              --spec FILE|-
              [--idempotency-key KEY]

forge-runtime group graph show GRAPH_ID [--include-spec]
forge-runtime group graph list [GROUP_RUN_ID] [--limit N]
```

`--spec -` reads bounded UTF-8 JSON from standard input. A file path is read
only because the caller explicitly named it; graph preparation never scans a
project workspace or Group member path.

### Ownership boundary

The graph records a manager profile and instruction, but this slice does not
execute that profile:

- `forge-core` remains the sole future dependency-wave scheduler and Group
  management control plane;
- `forge-runtime` remains the sole future per-node model/tool loop;
- the Hub owns the immutable graph artifact and provenance;
- a future TypeScript client presents and transports state, but does not run a
  second autonomous loop.

Machine and human output therefore state:

- graph preparation occurred;
- manager execution did not occur;
- node Agent execution did not occur;
- no model/provider/network/tool execution, workspace scan, writeback, task
  result, or derived memory occurred; an explicitly named spec file may have
  been read.

### Exact source

Every graph binds one fully validated prepared Group Run:

- Group Run version and ID;
- Group ID;
- context version and context-slice digest;
- snapshot digest and byte count.

Each graph node names one exact `project_id` and its frozen Group member role.
Preparation rejects a project that was not present in the frozen Group Run or
a role that differs from that snapshot. Multiple node IDs may target the same
project because a node identifies one task, not one project. Later changes to
Group membership do not change the graph.

### Graph specification

The caller supplies versioned JSON with:

```json
{
  "v": 1,
  "manager": {
    "agent_profile": "integration-manager",
    "instruction": "Coordinate the frontend, backend, and SSO contract."
  },
  "nodes": [
    {
      "node_id": "sso-contract",
      "project_id": "project-sso",
      "member_role": "identity",
      "agent_profile": "implementer",
      "task": "Define the issuer and audience contract.",
      "acceptance": "The contract is explicit and testable."
    }
  ],
  "edges": [
    {
      "from_node_id": "sso-contract",
      "to_node_id": "frontend-integration"
    }
  ]
}
```

Manager and Agent profile names are labels only. This slice does not resolve
them to `.agent/agents/*`, grant capabilities, select a model, or prove an
Agent identity.

The application builds a canonical manifest containing the exact source,
manager, nodes, canonical edge order, and deterministic dependency waves.
Node order is semantic and breaks ready-node ties. Edge input order is not
semantic; edges are sorted by `(from_node_id, to_node_id)` before persistence,
and stored edges must already be in that strict order.

Edges express ordering constraints only. They do not carry predecessor output,
task results, files, or context between nodes. Likewise, persisted waves are a
deterministic readiness plan, not evidence that any node was scheduled or ran.

Version 1 permits 1–32 nodes and at most 512 directed edges under a bounded
two-MiB manifest. Identifiers, profiles, instructions, tasks, acceptance text,
aggregate bytes, and list sizes are independently bounded.

The graph fails closed on:

- duplicate node IDs or edges;
- unknown edge endpoints;
- self-dependencies;
- cycles;
- a source project or member role mismatch;
- empty or oversized fields;
- malformed or non-canonical durable bytes;
- inconsistent node, edge, or wave counts.

Kahn's algorithm computes waves while preserving authored node order within
each wave. Every node appears exactly once. A wave may run in parallel only
after every predecessor wave has completed; actual parallel execution remains
the responsibility of `forge-core`.

### Persistence and replay

Schema v8 appends one immutable table and two explicit indexes:

- `group_agent_graphs`;
- `group_agent_graphs_group_run`;
- `group_agent_graphs_created`.

The complete v8 owned catalog contains 20 tables, 14 named explicit indexes,
and 35 implicit autoindexes. Its release-pinned v1–v8 length-framed DDL
SHA-256 is
`5e2108cca17e10f12566abcabe69d8c1a0c965856344c4463f6992ddd30edcce`;
the independent v8 structural-contract SHA-256 is
`1edda54070b62bf9777a62166222f5f62c33d6a48484be5e525cc9f42b3304ed`.

The row stores source and manifest digests, canonical manifest bytes, node,
edge, and wave counts, an idempotency key, and creation time. It references
`group_runs` with `ON DELETE RESTRICT`.

The application first validates the candidate Group Run before constructing the
canonical manifest. The SQLite persistence step then runs in one independent
`BEGIN IMMEDIATE` transaction and repeats source/member validation:

1. look up the idempotency key;
2. for a new key, load and validate the exact Group Run through the same
   connection;
3. validate source membership, manifest bytes, digest, and counts;
4. insert the row;
5. read and fully validate the durable row before commit.

An exact same-key replay returns the original graph ID, creation time, bytes,
and manifest. Candidate ID and time are ignored. Reordering only the input
edges therefore replays exactly, while reordering authored nodes conflicts.
Reusing a key for another source or semantic graph conflicts. Reusing a graph
ID under another key also conflicts. Stored corruption remains corruption
instead of becoming an idempotency conflict. Prepare, replay, and show all
revalidate the full Group Run source and every project-role binding. SQLite
does so through its transaction connection; the public prepare path's earlier
candidate-source read is validation, not part of the key-first transaction.
Current mutable Group membership is irrelevant.

`show` reads the graph and source from one deferred SQLite snapshot and fully
revalidates both. `list` is deliberately metadata-only; it validates bounded
row metadata but does not load the source or manifest and says so.

### Privacy and honesty

The local Hub database stores manager instructions, task text, acceptance
criteria, member project IDs and roles, edges, and waves in plaintext.

Default `prepare` and `show` output includes content-free graph metadata and
honesty flags. `--include-spec` explicitly reveals the fully validated source
and manifest. Human output terminal-escapes all revealed text. List never
includes the manifest and never claims source validation or Agent execution.
Default output also omits the spec source path, idempotency key, member project
IDs, roles, manager instruction, tasks, and acceptance text.

Graph commands read no provider credential, perform no network request,
discover or traverse no member workspace, authorize no tool, and create no
Conversation, Prompt, Project Run, task result, or memory writeback. Prepare
may read the one bounded spec path explicitly named by the caller; it performs
no other workspace discovery or traversal. Output separately reports whether
that explicit file read occurred. SHA-256 values are unkeyed local content
identities, not signatures, same-user tamper protection, Agent attestation, or
evidence that a task was executed.

## Rejected alternatives

- **Run the manager in Rust now.** This would duplicate `forge-core`'s
  orchestration ownership and create unresolved retry, budget, and approval
  semantics.
- **Reuse workflow YAML directly.** A workflow phase is governance
  configuration, not an immutable Group/member/source artifact.
- **Select current Group membership on execution.** That would make a reviewed
  graph mutable and destroy reproducible replay.
- **Normalize nodes and edges into independently editable rows.** Version 1 is
  immutable; canonical manifest bytes are a smaller and safer first contract.
- **Infer tasks from synthesis prose.** Free-form model text is not a typed
  execution graph. A future model-produced graph requires structured output,
  its own disclosure/consent contract, and validation.
- **Expose task text by default.** Manager and node instructions may repeat
  sensitive Group context.
- **Call preparation "manager execution".** It only validates and freezes a
  graph.

## Consequences and next slice

Sprint 46 establishes a durable interchange artifact between the Group Hub,
`forge-core`, and the self-developed Rust Agent Runtime. It makes assignments
and dependency waves inspectable without spending model budget or granting
workspace capabilities.

The next execution slice must separately define:

- a versioned `forge-core` graph-run state and per-wave checkpoint;
- exact per-node task input and predecessor-result provenance;
- per-node model/provider/tool/workspace capabilities and budgets;
- claim-before-effect, cancellation, timeout, and unknown-outcome recovery;
- human approval for mutating tools and explicit result/writeback targets;
- manager termination/re-plan rules and bounded failure propagation;
- OS-level isolation for unattended or mutating work.

Remote account binding, synchronization, shared ACL, TypeScript UI, derived
memory, live multi-Agent discussion, and model-generated graph planning remain
separate capabilities.
