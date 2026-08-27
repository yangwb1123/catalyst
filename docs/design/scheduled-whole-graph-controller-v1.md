# Scheduled Whole-Graph Controller v1

## Purpose

The existing scheduled execution path safely handles one Core-selected node per explicit call. This design adds a bounded durable controller around that path so a serial Graph can advance across process restarts without turning journal replay into consent or provider-send authority.

The controller is intentionally caller-driven. It may automatically materialize and prepare the next selected node, but every provider request remains behind a new explicit `step` call and fresh exact-request consent.

## Public operations

```text
forge-runtime group graph run controller start GRAPH_RUN_ID ...
forge-runtime group graph run controller show GRAPH_RUN_ID
forge-runtime group graph run controller advance GRAPH_RUN_ID ...
forge-runtime group graph run controller step GRAPH_RUN_ID ...
```

`start` binds the immutable schedule, Core digest, execution profile and durable budgets, then passively drives to the first consent boundary. `show` is a read-only journal inspection and does not claim to have revalidated Core. `advance` performs recovery and passive selected-node materialization or request preparation with no provider poll. `step` performs the same recovery and may delegate one exact request to the existing one-node step.

One `step` invocation polls at most one provider request. After a completed node it may prepare the next successor, but it returns at `AwaitingFreshConsent` before a second request.

Pure argument, identifier, profile and numeric-bound validation precedes Core construction or storage access. For an existing Hub, `start` performs a read-only schema/controller-pin inspection before invoking Core; all four required pinned reconcile, ready-release, materialization and terminal handshakes complete before the writable v29 open and any migration. A self-consistent different executable therefore cannot use migration as a side effect before the pin mismatch is reported.

## Immutable header

One Graph Run has one controller. The header binds:

- controller and protocol versions;
- Graph Run, schedule ID, schedule digest and node count;
- schedule-v1 and progress-v1 compatibility;
- exact Core executable SHA-256;
- endpoint, model, token, event, byte, timeout, pricing and result limits;
- maximum effectful steps and maximum total micro-USD reservation;
- creation time, controller ID and controller digest.

The header is separate from the Graph Run's existing seed-only journal. Exact controller recreation is replay; a changed header for the same Graph Run is conflict.

## Journal model

The append-only canonical event chain uses these payloads:

| Event | Meaning |
|---|---|
| `Started` | Controller and first atomic progress observation were accepted. |
| `MaterializePlanned` | The exact Core-selected ordinal will be materialized with a deterministic key. |
| `MaterializeObserved` | The selected candidate contract was durably observed. |
| `PreparePlanned` | The selected contract's provider request will be prepared with a deterministic key. |
| `PrepareObserved` | The exact prepared request was durably observed. |
| `AwaitingFreshConsent` | Current request, authorization, snapshot, decision and content condition are fixed for operator consent. |
| `DispatchPlanned` | Fresh current-call consent was observed and one step plus maximum-cost reservation was consumed. |
| `NodeCompleted` | The exact lifecycle has a validated completed terminal receipt. |
| `RetryablePreclaimFailure` | A closed preclaim dependency failed; the reservation remains consumed. |
| `Stopped` | Unsafe, incompatible or exhausted evidence ended this controller. |
| `Completed` | Reconcile proves every scheduled node completed. |

Every event binds the controller, Graph Run, sequence, previous digest and timestamp. The store compares the submitted sequence and previous digest to the current head in one immediate transaction. Exact event replay is idempotent; a different event at the same sequence conflicts.

The application validates the complete prospective journal before submitting an append. Production commands inject a clock and resample it for every durable event rather than reusing command-start time; the sampled time is clamped to the current head for monotonicity. In particular, a completion or stop observed after a provider call carries a post-call sample rather than the preceding `DispatchPlanned` time.

Bounds are 512 events, 64 KiB for a header or event and 4 MiB for a reconstructed journal. The journal carries identifiers, digests, dispositions, consent-observed facts and reservations only. Prompt, predecessor output, provider body and model result remain in their existing private records.

`show` returns a redacted projection of the validated header and a redacted complete controller event chain. It does not copy the private execution profile, Core pin, candidate, provider request body, predecessor content or model result into the controller output, and its trust facts state only the Runtime checks actually performed; they do not imply current Core validation or process containment.

## Passive drive

The controller obtains one atomic `{snapshot, decision}` observation. Only exact schedule-v1 `ready` can enter selected-node processing. `completed` records controller completion. Claimed, manual-recovery, failed, failed-uncertain or incompatible dispositions map to a terminal stop.

For `ready`, the controller processes only the selected ordinal:

1. Revalidate the schedule and immutable Core pin.
2. If its candidate is absent, append `MaterializePlanned`.
3. Materialize the exact initial or successor candidate outside the controller transaction using a deterministic controller-and-ordinal key.
4. Revalidate the stored candidate and append `MaterializeObserved`.
5. If its request is absent, append `PreparePlanned`.
6. Prepare and revalidate the exact request outside the controller transaction with a second deterministic key.
7. Append `PrepareObserved`, rerun ready release, and append `AwaitingFreshConsent` with current anchors.

The general wave-admit path is not reused. It can materialize multiple ready nodes and does not express this controller's single selected ordinal, immutable profile or bounded serial cursor.

Materialization and preparation are provider-poll-free but may write their existing durable candidate and request records. A planned call can therefore be re-entered only through the same deterministic key and full source validation.

Automatic successor materialization passes predecessor terminal receipts but not predecessor plaintext, so controller-created successors are content-free in v1. The controller may reuse a separately precreated, fully validated content-bearing candidate; that request still exposes the content condition and requires its independent fresh consent. Automatic plaintext dataflow needs a new materialization contract.

## Effectful step

At `AwaitingFreshConsent`, the command requires caller anchors for the current awaiting-event digest, request and authorization plus fresh off-machine consent. When predecessor content is included it independently requires fresh content consent. No consent is inferred from the journal, an earlier CLI invocation, a terminal receipt or another flag.

Before credential, provider or executor-owner construction, the controller checks remaining step and total-cost capacity and appends `DispatchPlanned`. It records the actual current-call off-machine and predecessor-content consent facts; predecessor-content consent is mandatory only when the awaiting event says content is included. One dispatch reserves the execution profile's complete per-node maximum cost. The reservation is monotonic and nonrefundable in v1.

The caller-supplied pricing artifact is read lazily, only after journal validation, unresolved-dispatch recovery, passive drive, terminal checks and current consent-anchor validation establish that this invocation may plan a dispatch. Recovery and terminal re-entry therefore do not depend on an otherwise-unused pricing file or stdin stream.

The controller then invokes the existing one-node application service exactly once outside the journal transaction. That service reruns current ready release and uses its immediate snapshot-to-lifecycle claim plus cross-family Project lane as the only send fence. The controller CAS is only journal ordering.

Successful completion appends `NodeCompleted` and resumes passive drive. A specifically classified credential, provider-construction or owner-evidence failure appends `RetryablePreclaimFailure`; only that durable safe-preclaim classification may return to consent with a later current authorization and new consent. A bare dangling `DispatchPlanned`, claim-uncertain or post-claim-uncertain return first yields to exact durable lifecycle evidence; absent or temporarily unavailable evidence is conservatively persisted as `Stopped(claimed_unknown)` when the journal remains writable. Structural lifecycle corruption remains a hard error. All other post-claim states follow the stop rules below.

If another recovery writer wins the controller head while an explicit uncertainty stop is being persisted, the uncertain call reloads and validates a strict descendant, rechecks lifecycle evidence, accepts only matching completion or uncertainty-safe terminal evidence, and otherwise rebases the stop onto the current nonterminal head. The loop is bounded by the protocol's 512-event cap and every retry requires strict chain growth. Generic budget, compatibility or reauthorization state cannot clear an unresolved dispatch.

If the delegated one-node invocation returns a structured result but a later controller append, reconcile or reload fails, the command preserves that invocation result and reports the redacted post-invocation error separately. It also reports whether the returned controller journal was observed current; it never collapses a successful external invocation into an ordinary pre-effect failure or presents a stale journal as current.

## Re-entry matrix

| Durable observation after a planned dispatch | Controller action | Provider action |
|---|---|---|
| No lifecycle for a bare `DispatchPlanned` | Append `Stopped(claimed_unknown)` and preserve the reservation. | No retry or resend. |
| Durable `RetryablePreclaimFailure` after a classified closed preclaim failure | Return to fresh consent with the reservation preserved. | No automatic poll or reuse of old consent. |
| Claimed | Append `Stopped(claimed_unknown)`. | No resend; operator lifecycle handling only. |
| Completed receipt | Append `NodeCompleted`, reconcile and passively prepare the successor. | No repeat of the completed request. |
| Quarantined | Append `Stopped(quarantined)`. | No resend or automatic recovery. |
| Adjudicated | Append `Stopped(adjudicated)`. | No resend or automatic recovery. |
| Failed receipt | Append `Stopped(failed)`. | No retry. |
| Failed-uncertain receipt | Append `Stopped(failed_uncertain)`. | No retry. |
| Valid Core `incompatible_progress` decision, or future typed and integrity-valid unsupported schedule | Append the matching stop. | No materialization, claim or poll. |
| Structurally corrupt schedule/progress/lifecycle or invalid Core decision | Return a hard redacted error; do not invent a stop disposition. | No materialization, claim or poll. |
| Step or total-cost budget exhausted | Append `Stopped(budget_exhausted)`. | No claim or poll. |

A `Stopped` or `Completed` event is terminal. Version 1 does not reopen it after later operator action.

## Concurrency and crashes

The Planned/call/Observed split ensures the intent is durable before a fallible call and the observation is never invented before it returns. Calls remain outside SQLite transactions.

Two controller writers can race, but only one exact head append wins. A CAS loser reloads and validates the winning descendant before returning the redacted `concurrent_update` classification; an invalid or non-descendant reload remains corrupt evidence. A dispatch planner may still overlap a process whose one-node call has not reached lifecycle claim. The journal does not attempt to infer that process's liveness. Every contender must enter the existing lifecycle claim transaction; only that winner receives non-clone request authority and can poll the provider.

Crash after `DispatchPlanned` consumes budget permanently. Re-entry scans the most recent unresolved dispatch and inspects any-family lifecycle evidence before generic schedule/progress classification. A late completed lifecycle may repair the unresolved dispatch without resend. Absence means only that no durable lifecycle was observed at that instant, not that the prior process was harmless; consequently a bare unresolved dispatch is durably stopped as `claimed_unknown`. Only a durable `RetryablePreclaimFailure` classification proves that the old request authority was safely released and permits a later fresh-awaiting event and explicit call.

If valid progress moves beyond an awaiting or passive node without a matching controller `DispatchPlanned`/`NodeCompleted` lineage, the controller does not appropriate that external completion. It appends `Stopped(incompatible_progress)`. The same rule binds the initial `Started` snapshot/decision crash cut: drift before the first controller-owned transition is terminal for that controller.

## Compatibility and trust

Controller v1 accepts only the existing serial schedule: contiguous completion prefix, one in-flight node, exactly one attempt and fail-fast failure. A valid Core semantic decision can stop non-contiguous evidence as `incompatible_progress`. The current schedule store and codec are strict v1 readers, so unknown versions, noncanonical bytes, invalid digests and malformed policy are structural hard errors rather than `incompatible_schedule`; that stop reason is reserved for a future compatibility reader able to prove a typed, integrity-valid but unsupported schedule. Concurrent schedule execution needs a new controller and lifecycle design.

Every Core-using operation requires the immutable header's executable digest and exact handshake. The current pinned-executable implementation copies Core into a sealed in-memory executable and is Linux-only, so `start`, `advance` and `step` are Linux-only; read-only `show` does not construct Core. Core remains an operator-trusted same-user TCB. Digest pinning and empty environment are not publisher identity, function attestation, filesystem isolation, network isolation or effect containment.

## Required validation

- Domain transition, digest, canonical encoding, identifier, ordinal, timestamp, event-count, byte and budget boundaries.
- SQLite v28-to-v29 migration, rollback, exact replay, divergent conflict, corruption and two-connection head races.
- Every crash cut before and after Planned, call and Observed for materialize, prepare and dispatch.
- Every reconcile disposition and lifecycle substate, including late completed repair, external-progress refusal and permanent unsafe stops.
- Awaiting-event/request/authorization mismatch, conditional predecessor content, stale authorization and replay refusal.
- Durable reservation behavior across restart, preclaim failure and budget exhaustion.
- Competing process tests that prove controller CAS ordering and lifecycle single-send without conflating them.
- Compiled CLI pin, handshake, interruption, privacy, honest read-only show trust and no-credential/no-provider passive-command tests; deterministic application fixtures separately prove the at-most-one-poll `step` invariant without contacting a live provider.
- Fresh-context architecture and security review followed by the complete repository acceptance harness.

All provider tests use deterministic local fixtures. No validation run requires a live paid provider.

## Deliberate limits

This version has no daemon, background execution, parallel wave, automatic retry, transport resend, lease expiry, automatic owner death decision, adjudication, quarantine recovery, budget refund, profile mutation, automatic predecessor-plaintext propagation, remote request lookup or remote exactly-once guarantee. Core-using commands are Linux-only. It does not authenticate consent or Core and does not protect the local database from same-user replacement or rewriting.
