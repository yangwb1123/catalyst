# ADR-0054: Local Governance Semantic View v1

- Status: Accepted
- Date: 2026-08-10
- Owners: Governance / Architecture / Runtime Engineering
- Extends: ADR-0045, ADR-0046

## Context

ADR-0046 durably preserves exact canonical `EvidenceRecord` and `KnowledgeClaim` v1 bytes and a structural sequence head. It deliberately cannot answer which
declared record is the semantic aggregate tail, whether that declaration is not-yet-valid or overdue at a particular time, which Claims are contradictory
candidates, or which Assumption/Hypothesis validation plans are due. Consumers otherwise have to reinterpret immutable records independently, risk hidden wall-clock
dependence, or mistakenly treat the structural head as current truth.

The complete Wave 1 authority kernel is still unavailable. There is no authenticated principal, CapabilityGrant, ApprovalRecord, PDP, authoritative transition,
knowledge apply, conflict adjudicator, hard-gate admission, or completion receipt. A useful next slice must therefore materialize deterministic declared semantics
without claiming any of those capabilities.

## Decision

1. Add `GovernanceSemanticView v1` as a separately versioned, rebuildable projection over the exact local journal. Its fixed public interpretation is
   `semantic_projection_only_no_truth_or_authority`. It does not rename or reinterpret `GovernanceStructuralHead` and does not alter Evidence/Claim v1 canonical
   bytes, record digests, append request identity, receipts, or replay rules.
2. Project every structural aggregate tail into a semantic head containing exact record identity/digest, project/scope, declared state, validity interval, sequence,
   and the append time that updated the structural head. Claim tails additionally project type, subject, predicate, a domain-separated object digest, a
   domain-separated conflict-key digest, review time, and any complete validation plan. Projection and conflict-group validation recompute the conflict key from
   Claim type/project/scope/subject/predicate instead of trusting its stored value. Projection digest is SHA-256 over
   `"forgeos.governance.semantic-projection.v1\0"` plus canonical projection JSON with its digest field empty.
3. Enforce lifecycle continuity atomically on every append. For v26 compatibility, sequence one may use any state already admitted for its Claim type by the
   ADR-0045 authority-free shadow contract: Fact `candidate|contested`; Constraint/Inference/Lesson `candidate`; Decision `proposed`;
   Assumption/Hypothesis `open|testing`; Proposal `draft|submitted`; Unknown `open|investigating`. Successors must preserve record kind,
   aggregate/project/scope, Claim type, subject, predicate, object type/value, and owner; creation time cannot move backwards. Only states already admissible in the
   ADR-0045 shadow contract may transition: Fact `candidate↔contested`; Assumption/Hypothesis `open→testing`; Proposal `draft→submitted`; Unknown
   `open→investigating`; same-state replay successors are allowed where defined. Constraint/Inference/Lesson remain `candidate`, Decision remains `proposed`, and no
   authority-requiring state is admitted. This is durable declared lifecycle consistency, not authoritative promotion.
4. Require every public semantic read to supply `as_of_unix_ms` explicitly. No API consults the process wall clock. Evaluation precedence is
   `not_yet_valid`, then `validity_expired`, then Assumption/Hypothesis `validation_overdue`, then `review_overdue`, otherwise `fresh`. These labels compare declared
   times only; even `fresh` does not establish real-world correctness, source freshness, trust, approval, or hard-gate eligibility.
5. Form conflict groups only from active-at-`as_of` Claim tails that share exact Claim type/project/scope/subject/predicate but have at least two distinct canonical
   object digests. Groups and members are byte-order deterministic and report candidates only. The projection never chooses a winner, merges Claims, retracts a
   member, or changes a Claim state.
6. Deterministically materialize one validation job for each current Assumption/Hypothesis with its required plan. Job identity binds the exact current record ID and
   validation-plan digest under `"forgeos.governance.validation-job.v1\0"`; validation recomputes that identity and rejects any other Claim type or authority-bearing
   declared state. Public evaluation reports whether caller time has reached the declared due time, but does
   not execute the method, collect Evidence, authenticate the owner, or issue a validation verdict.
7. Store immutable journal rows, structural heads, semantic heads, Claim fields, and validation-job materialization in one SQLite append transaction. Existing
   structural/semantic projections are revalidated from exact immutable records before successor append or exact replay. A missing, stale, extra, or divergent
   materialized row is corruption rather than a reason to trust or silently repair it.
8. Add SQLite v27 as an additive projection migration from v26. Migration revalidates all durable batches and atomically backfills semantic rows from exact journal
   records; any lifecycle, digest, relation, cardinality, schema, or final-validation failure rolls the complete migration back to v26. No Memory, ADR, file, pre-v25
   table, or external source is imported. Older binaries must reject v27.
9. Bound public lists and each conflict group to 100 results and the v1 local Claim-head integrity scan to 10,000 aggregates. Each inspected aggregate has one shared
   1,024-record/16-MiB unique-canonical-blob budget covering its complete history, transitive reference closure, and every complete owning batch pulled in by either.
   Multi-head scans additionally share independent 65,536-record/256-MiB unique-canonical-blob and 1,000,000-work-unit budgets across decoded owning batches and
   verification work. Crossing any resource bound is unavailable, not a partial/empty result, corruption label, or “no conflicts.” Internal rebuild is atomic and not
   exposed as a public CLI mutation in this slice.
10. Keep completion and authority unchanged. `forge accept` remains the sole completion authority. Semantic output cannot satisfy a hard gate, issue permission,
    approve a decision, apply knowledge, authenticate identity, or authorize an external effect.

## Public contract

`docs/contracts/governance-semantic-view-v1.schema.json` freezes three strict read envelopes under
`forgeos.governance-semantic-view/v1`:

- `GovernanceSemanticAssessment` for one Evidence/Claim aggregate projection at explicit caller time;
- `GovernanceClaimConflictList` for bounded conflict candidates at explicit caller time;
- `GovernanceValidationJobList` for bounded scheduled validation work, optionally due-only, at explicit caller time.

The CLI adapters are:

```text
forge-runtime governance journal view KIND AGGREGATE_ID --as-of-unix-ms N
forge-runtime governance journal conflicts --as-of-unix-ms N [--limit N]
forge-runtime governance journal validation-jobs --as-of-unix-ms N [--due-only] [--limit N]
```

All always select the exact current structural aggregate tail; `as_of_unix_ms` evaluates that tail's declared interval and never selects a historical tail. Public
adapters open an existing exact-v27 live database with SQLite `mode=ro`, enforce `query_only`, and perform selection, integrity verification, and evaluation inside one
Deferred snapshot. They create, migrate, or logically write no Hub rows. They are not described as filesystem-effect-free: on a clean WAL database SQLite may
create and later remove empty WAL/SHM sidecars, and may coordinate or change SHM read-lock bytes. A fully read-only filesystem may therefore yield `Unavailable`.
Append remains the only journal CLI mutation and may
migrate a supported database to v27 before writing. The fixture
`docs/contracts/fixtures/governance-semantic-view-v1.json` pins the ADR-0045 Claim projection at update time 77 and evaluation time 1700000002000, including object,
conflict-key, and projection digests.

## Integrity and recovery

- Materialized rows are never the sole source of meaning. Reads load and validate the complete aggregate history, transitive reference closure, complete owning
  batches, exact structural tail record, canonical digest, projection digest, every projected column, and global structural/semantic cardinality within the frozen
  unique-record/byte/work budgets.
- Conflict and job queries revalidate every scanned Claim projection. Missing or extra job rows and semantic/structural cardinality drift fail closed.
- Append validates the prior semantic projection and all candidate transitions before any insert, then refreshes only resulting aggregate tails in the same
  immediate transaction. A semantic conflict writes no batch, record, structural head, semantic row, or receipt.
- Rebuild first validates every immutable batch and every lifecycle transition, clears only rebuildable semantic rows, then reconstructs all tails/jobs in one
  transaction. A failed rebuild preserves the previous projection.
- Exact replay returns the original ADR-0046 receipt only after the current semantic projection for every replayed aggregate is independently consistent.

## Consequences

- Local callers can share one deterministic interpretation of declared tail state, explicit-time validity, conflict candidates, and validation scheduling.
- Lifecycle continuity becomes durable for the authority-free state subset, reducing ambiguous histories before an authority kernel is introduced.
- Query integrity is intentionally more expensive than trusting indexes: v1 scans and revalidates bounded local Claim tails. A future scale change requires a new
  versioned integrity design rather than weakening checks silently.
- The words `current`, `fresh`, `conflict`, and `due` remain projection terms. Callers must not rewrite them as confirmed fact, approved choice, trusted source,
  completed validation, or authorization.

## Rejected alternatives

- Reinterpret the structural head as semantic currentness: rejected because sequence alone does not evaluate declared state, time, or Claim identity.
- Use the process clock implicitly: rejected because results would be non-replayable and difficult to audit.
- Store only computed status/conflict/job rows without exact revalidation: rejected because materialized corruption could become an epistemic claim.
- Select a conflict winner by confidence, append time, or sequence: rejected because none supplies authenticated authority or adjudication policy.
- Admit confirmed/accepted/active/validated terminal states now: rejected because the required Grant/PDP/Approval/Transition trust root remains unimplemented.
- Combine ContextPackage, CapabilityGrant, PDP, knowledge apply, and this projection in one migration: rejected because their authority and compatibility contracts
  require independent design and review.
