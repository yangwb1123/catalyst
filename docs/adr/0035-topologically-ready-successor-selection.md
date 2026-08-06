# ADR-0035: Topologically-ready successor selection (wave-parallel base)

- Status: Accepted for the scheduled successor-selection slice
- Date: 2026-08-05
- Extends: [ADR-0031](0031-passive-successor-contract-candidate.md)

## Context

ADR-0031's successor selection is serial-prefix: the receipts must match
`schedule.Nodes[0..N]` in order and the successor is `schedule.Nodes[N]`.
That forces every node to wait for ALL earlier ordinals even when they are
topologically unrelated: in a diamond graph (frontend, backend → sso) the
backend cannot be selected until frontend's receipt exists, despite backend
having no direct predecessor. The schedule's waves already express the true
parallelism (frontend and backend share wave 0); the Project-lane protocol
already allows independent lanes to dispatch concurrently. Only the selection
rule is stricter than the topology.

Rust admission already validates topologically (`direct_predecessor_node_ids ⊆
consumed receipts` and node identity from the schedule by ordinal); only the
Go selector is serial.

## Decision

Replace the serial-prefix selector with a topologically-ready selector while
keeping every identity binding and the same candidate shape:

- `verifyReceipts` requires each receipt to bind a schedule node (graph run,
  lane, node id, attempt), with no duplicates and at most one receipt per
  node. Receipts may arrive in any order and need not be a prefix.
- `selectReadyNode` picks the first node in the schedule's serial
  (wave-then-authored) order whose direct predecessors are all consumed and
  which itself is not yet consumed. Its execution ordinal is used for the
  candidate, and its `required_predecessor_node_ids` are its direct
  predecessors (the covered set).
- The candidate's `predecessor_terminal_receipts` are exactly the receipts of
  the candidate's direct predecessors; other consumed receipts are evidence
  of progress but are not carried by this candidate's request (receipt
  metadata is never copied into the provider body beyond what the candidate
  binds, per ADR-0031).

Serial behavior is preserved for serial graphs: when every earlier ordinal is
a direct predecessor (the chain case), the selected node is exactly the next
ordinal, byte-identical to today. For diamond graphs, frontend and backend
can now each be selected after their own (empty) predecessor sets, enabling
independent Project-lane dispatches in the same wave — the wave-parallel base.

## Safety

The candidate contract shape, digests, flags, admission validation, and the
effectful lifecycle (ADR-0030/0032/0033/0034) are unchanged. Only the
selection rule relaxes from "contiguous prefix" to "topologically ready";
every binding still fails closed on drift. No node is ever selected before
its direct predecessors are provably terminal, so the no-send, no-resend,
and no-leakage invariants are untouched. A selected candidate is still
per-node/per-attempt and one-per-Run.
