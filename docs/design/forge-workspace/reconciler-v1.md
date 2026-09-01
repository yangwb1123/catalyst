# Pure Pre-Effect Reconciler v1

> Status: R0-C4 design freeze for the FC-06 pure-decision subset. This document does not close the broader F6 Runtime, Verification, persistence, or completion work.

## 1. Purpose

`forge-core/internal/reconcile/application` is the sole Go owner of top-level pre-effect WorkItem selection. One call consumes a complete caller-supplied `ControlSnapshot` and returns exactly one deterministic `Decision`, including `NoOp` when nothing can advance.

R0-C4 admits no production consumer of that Decision. A bounded lexical and Linux/Darwin/Windows package-metadata proof rejects every importer; a future application consumer must first trigger ADR-0106's effectful-consumer revisit.

The package is a pure application policy over the existing Delivery Domain. It performs no filesystem, Git, database, clock, random, process, network, Runtime, Harness, provider, journal, outbox, identity, or state mutation operation.

## 2. Authority boundary

All input values are caller declarations. The caller must keep the complete reachable value graph race-free and stable for the duration of `Decide`. The Reconciler validates relations at the call boundary but does not prove that the supplied projection is atomic, current, authentic, durable, or complete.

`AssessmentStatusSatisfiedDeclared` is not effective Policy, Approval, Grant, budget reservation, or authorization. `RecordRef` values are structurally validated and never resolved. Equal supplied ProjectSnapshot identifiers do not prove repository freshness.

Every output is passive:

- `ReadyWorkItem` identifies one pre-effect candidate only;
- `AwaitApproval`, `BlockWorkItem`, `ReplanChange`, and `EscalateUncertain` classify why advancement must stop;
- no Decision requests a Platform Core state edge, creates an Attempt, releases an effect, or completes a WorkItem or Change.

An effectful consumer must later reload a durable current projection, authenticate Policy and Approval, reserve budget, perform expected-version/CAS checks, journal the decision and outbox entry atomically, and preserve uncertain-effect no-resend behavior.

## 3. Input contract

The public Go input is:

```go
type ControlSnapshot struct {
    Objective               domain.Objective
    Change                  domain.Change
    WorkGraph               domain.WorkGraph
    CurrentSnapshotBindings []domain.SnapshotBinding
    Assessments             []WorkItemAssessment
}

type WorkItemAssessment struct {
    WorkItemID         string
    ChangeVersion      int64
    WorkGraphVersion   int64
    PolicyProfile      string
    ProjectSnapshotID  string
    Risk               string
    RequestedEffects   []string
    Policy             AssessmentStatus
    Approval           AssessmentStatus
    Budget             AssessmentStatus
    EvidenceRefs       []platformcorecontract.RecordRef
}
```

`AssessmentStatus` is one closed vocabulary shared by the three declared preconditions:

- `unknown`;
- `satisfied_declared`;
- `unsatisfied_declared`;
- `uncertain`.

An assessment must bind the exact Change version, WorkGraph version, policy profile, ProjectSnapshot, risk, requested-effect set, and a known WorkItem. At most one assessment exists per WorkItem; omissions are permitted and fail closed when that WorkItem becomes the selected frontier candidate. Each assessment contains at most 32 requested-effect tokens of at most 64 bytes each and carries 1-16 unique, structurally valid evidence references.

`CurrentSnapshotBindings` are supplied identifiers only. `domain.CompareSnapshotBindings` validates their bounded shape and compares them with the Change binding set.

## 4. Output contract

The public Go output is:

```go
type Decision struct {
    Kind              DecisionKind
    ReasonCode        ReasonCode
    ObjectiveID       string
    ObjectiveVersion  int64
    ChangeID          string
    ChangeVersion     int64
    WorkGraphID       string
    WorkGraphVersion  int64
    WorkItemID        string
    EvidenceRefs      []platformcorecontract.RecordRef
}
```

The v1 Decision vocabulary is deliberately smaller than the target architecture:

- `NoOp`;
- `AwaitApproval`;
- `ReadyWorkItem`;
- `BlockWorkItem`;
- `ReplanChange`;
- `EscalateUncertain`.

Every valid input returns exactly one Decision with aggregate identifiers and versions. Aggregate-level decisions have an empty `WorkItemID`. Evidence is copied, deduplicated by validation, and sorted by record type, ID, and digest before return. Invalid or internally mixed inputs return the zero Decision and an error matching `ErrInvalidControlSnapshot`.

`DispatchAttempt`, `RequestVerification`, `CompleteWorkItem`, and `CompleteChange` remain absent because FC-07/FC-08 transport, effective authority, receipt freshness, completion policy, and the F7 full-Change join do not exist in this slice.

## 5. Validation order

Structural validation always precedes business classification:

1. Fail closed unless the Reconciler's exhaustive state classifier exactly covers the current Platform Core `WorkItemStateValues()` vocabulary.
2. Re-run `domain.ValidateWorkGraph` over Objective, Change, and WorkGraph.
3. Re-run `domain.CompareSnapshotBindings` over bound and supplied-current identifiers.
4. Before any assessment-key lookup, effect copy, or sort, validate assessment count, exact typed WorkItem ID, requested-effect count, per-effect byte bound, and token grammar; then validate unique WorkItem binding, versions, declarations, statuses, requested-effect set, and evidence references.
5. Build `domain.TopologicalOrder` and reject impossible progress: a WorkItem in `ready`, `dispatched`, `running`, `verifying`, `completed`, `failed`, or `uncertain` cannot have a direct predecessor that is not `completed`.

Malformed values, duplicate or unknown assessments, mixed versions, declaration substitution, and impossible progress are input errors. They are not converted to a permissive `NoOp`.

## 6. Decision precedence

After validation, v1 applies this exact priority:

1. Change observation `uncertain`, then the first topologically ordered uncertain WorkItem, then the first topologically ordered uncertain assessment: `EscalateUncertain`.
2. Any missing, changed, or unexpected supplied snapshot binding: `ReplanChange`.
3. Inactive Objective, non-active Change, non-accepted WorkGraph, or non-runnable Change observation: `NoOp`; an exact Change desired state of `awaiting_approval` returns `AwaitApproval`.
4. The first `dispatched`, `running`, or `verifying` WorkItem: `NoOp`. FC-06 never selects a second item while work is in flight.
5. The first `blocked`, `failed`, or `cancelled` WorkItem: `BlockWorkItem`. FC-06 never retries or bypasses it.
6. Visit the deterministic topological order and choose the first non-completed WorkItem whose direct predecessors are all completed. A `draft` item returns `NoOp`; `planned`, `awaiting_approval`, and `ready` proceed to precondition evaluation.
7. A missing assessment returns `AwaitApproval`. Policy is evaluated before Approval, and Approval before Budget. Unknown Policy/Budget or any unsatisfied status returns `BlockWorkItem`; unknown Approval returns `AwaitApproval`; all three `satisfied_declared` statuses return `ReadyWorkItem`.
8. If all WorkItems are completed, return `NoOp` with an unjoined-completion reason. A validated active DAG with incomplete work necessarily has a dependency-ready minimal item; if that internal invariant is ever violated, return `ErrInvalidControlSnapshot` instead of inventing a waiting business decision.

The topological order uses the Delivery Domain's lexicographic Kahn ordering. WorkItem, dependency, assessment, current-binding, requested-effect, and evidence input permutations cannot change a valid decision. Precondition status never causes v1 to skip the earlier candidate and select a later one.

## 7. Bounds and determinism

The Delivery Domain retains its existing limits: 1-128 WorkItems, at most 512 dependency edges, and 1-16 bound Projects. The Reconciler accepts at most 128 assessments, at most 32 requested-effect tokens of 1-64 bytes per assessment, and 1-16 evidence references for each present assessment. It checks caller-controlled assessment ID and effect bounds before map lookup, copying, or sorting.

`Decide` uses no ambient time, randomness, iteration-order decision, I/O, or mutable package state. For a caller-owned stable input, repeated and parallel calls return deeply equal values or equivalent stable errors. It never mutates input and never aliases an input slice in its output. The same bounded source and package-metadata proof that allows this package as Delivery's sole consumer also confirms that no production package consumes its Decisions.

## 8. Explicitly deferred work

R0-C4 does not implement:

- a current-state fold, repository, projection, controller loop, worker, journal, outbox, or crash recovery;
- authenticated Policy/Approval/Grant evaluation, budget consumption or reservation, manual override, or transition authority;
- Attempt identity, claim, idempotency key, RuntimePort, provider dispatch, retry, resume, or uncertain-effect adjudication;
- VerificationRequest/Receipt production, Artifact resolution, required-check applicability, completion join, Outcome, or Change completion;
- automatic multi-task waves, concurrency, conflict analysis, or successor Artifact disclosure;
- canonical JSON, cross-language wire, HTTP, CLI, TUI, App, or Web UI surfaces.

Those capabilities require separate ADRs and effectful consumers. Passing R0-C4 tests closes only the FC-06 pure pre-effect selection subset, not F6, F7, F3/F4, the R0 vertical journey, or Objective-to-Outcome delivery.
