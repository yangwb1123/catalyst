# Project Run root-input branch v1

## Status

Delivered as the bounded, queryable Project Run branching slice. The contract
creates a fresh child from a validated terminal Run's root input and records
one immutable direct-parent relation. The architectural contract is recorded
in [ADR-0095](../adr/ADR-0095-project-run-root-input-branch-v1.md).

## Product flow

```text
validated terminal parent
        |
        | run branch PARENT_RUN_ID
        v
fresh child + direct-parent lineage + run_started(seq 1)
        |
        | run lineage CHILD_RUN_ID   (read-only metadata)
        | run resume CHILD_RUN_ID    (explicit execution)
        v
independent child execution journal
```

The public commands are:

```text
forge-runtime --idempotency-key KEY -C PATH run branch PARENT_RUN_ID
forge-runtime run lineage RUN_ID
forge-runtime -C PATH run resume CHILD_RUN_ID
```

`run branch` requires an explicit Project selector and idempotency key. The
parent must exist in that Project and pass complete same-snapshot Run
inspection with a terminal recovery state.

## Root-input inheritance

The child reuses exactly the parent's persisted:

- Project ID;
- Conversation ID;
- bound user Prompt ID and root Prompt bytes; and
- complete execution configuration, including provider/model, system Prompt,
  read allowlist, live/offline mode, and execution limits.

The child begins with exactly one fresh `run_started` event at sequence 1. It
does not copy any parent journal suffix, assistant answer, terminal result,
tool call, tool output, usage event, or external-effect claim. The branch
command itself does not execute the child; the caller releases execution only
through a later explicit `run resume CHILD_RUN_ID`. If that child has terminal
outcome `RunOutcome::Completed`, the same command performs only idempotent
assistant Conversation writeback reconciliation after Project binding and
before credential, provider, tool, history, or workspace-content setup.
Children with terminal outcome `RunOutcome::Failed`, `RunOutcome::Cancelled`,
or `RunOutcome::LimitExceeded` remain ineligible for `run resume`.

Conversation context preceding the bound user Prompt is not snapshot-bound by
this relation. Workspace bytes are also not snapshot-bound. Machine output
therefore declares `context_snapshot_bound=false` and
`workspace_snapshot_bound=false`.

## Atomic persistence

SQLite v28 stores one `run_lineages` row per branch child. One
`BEGIN IMMEDIATE` transaction creates or exactly replays all three branch
facts:

1. the child `runs` row;
2. the immutable direct-parent lineage row; and
3. the child `run_started` event at sequence 1.

The transaction commits all three facts together. A child cannot be exposed
with a missing lineage or missing seed, and a lineage cannot be committed
without its child.

The lineage record is version 1 and fixes:

```text
relation_kind     = branch
branch_mode       = root_input
source_event_seq  = 1
```

`source_event_sha256` domain-binds the exact parent Run envelope and parent
root event. `lineage_sha256` domain-binds the child ID, parent ID, mode, source
sequence, source digest, and child creation time. Reads revalidate those
digests, the parent's terminal/root-event state, and the child's inherited
Project, Conversation, Prompt, execution configuration, creation time, and
root Prompt.

## Idempotency

The child Run ID is derived in a dedicated branch domain from the parent Run
ID and caller key, and uses the reserved `run-branch-` namespace. The same key
and parent exactly replay the committed child, lineage, and seed. Reusing the
key for another parent or another Run creation operation is a conflict.

Late exact replay reports the child journal's current validated state. It does
not reset, truncate, or replace child events that were appended after the
initial branch transaction.

## Lineage query

`run lineage RUN_ID` uses the existing-current immutable SQLite reader. It is
read-only, performs no migration or logical write, and returns only direct
parent metadata:

- whether a lineage record exists;
- child and parent Run IDs;
- lineage version and `root_input` mode;
- source sequence and integrity digests; and
- creation time.

The view is content-free: it returns no Prompt, event body, answer, result,
tool output, execution secrets, workspace path content, or ancestor expansion.
Ordinary starts and independent restarts correctly return no recorded branch
lineage.

## Effect boundary

Branch preparation reads only the selected local Hub state needed to validate
the Project and parent Run. It does not read credentials or workspace content,
construct a provider/tool/transport, access the network, or claim that any
model or tool effect occurred.

`RunOutcome::Completed` terminal `run resume` is similarly effect-bounded: it
performs writeback-only recovery from the persisted answer and does not execute
provider or tool work or read workspace content. Failed, cancelled, and
limit-exceeded terminal Runs remain non-resumable.

This v1 contract is intentionally limited to the root input at source sequence
1 and one direct-parent query. Arbitrary event-prefix branching, inherited
tool-effect replay, context/workspace snapshots, transitive lineage expansion,
automatic execution, and whole-Graph branching require separate contracts.
