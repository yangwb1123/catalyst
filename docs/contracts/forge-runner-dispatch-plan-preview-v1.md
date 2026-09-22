# Forge Runner dispatch-plan preview v1

`forge.runner-dispatch-plan-preview/v1` is a pure value bridge between three
already supplied observations:

- an offline device placement request and its caller-declared resource
  candidates;
- an Attempt lifecycle state; and
- a Runner execution-intent target plus an `executionlease` epoch/fencing
  grant.

The evaluator uses the placement request's fixed `evaluated_at_ms` as the
lease observation time. It validates the lease shape and exact Attempt/target
proof, then reports every candidate in deterministic device-ID order. A
candidate is marked `declarative_ready` only when its unverified placement
declaration matches, the lease target matches and is active at that fixed time,
and the Attempt state is one of `accepted`, `starting`, or `running`. This is a
comparison result; it is not an `eligible` or `schedulable` claim.

The intent target is an input identity and is never selected by this value
bridge. `selected_target_id` is always `null`. The result carries no fencing
token, command argv, workspace reference, output, reservation, or dispatch
receipt. `reservation_created`, `execution_authorized`, and
`dispatch_performed` are fixed `false`, and every authority bit is fixed
`false`. Requested, interrupted, completed, failed, uncertain, and unknown
Attempt states cannot produce a declaratively ready candidate; unknown states
are rejected as malformed input.

The Go implementation and focused tests are in
`forge-core/internal/deviceplacement/runner_dispatch_plan_preview.go` and
`runner_dispatch_plan_preview_test.go`. The tests derive an accepted state
through `executionattempt.Lifecycle`, consume the canonical `executionlease`
validation, exercise matching and mismatching candidates, terminal state,
expired lease, identity confusion, deterministic ordering, and mutation of
the returned observation. No production route, device registry, reservation,
dispatcher, Runner transport, or ADR status is changed.

## Read-only Rust consumer

The bounded Rust CLI consumes the Go observation directly with:

```text
forge-runtime --json device runner-dispatch-plan-preview --input FILE|-
```

`forge-runtime/crates/interfaces/src/device_runner_dispatch_plan_command.rs`
rejects duplicate JSON keys, unknown fields, oversized input, malformed owner
or identity values, unsorted or duplicate candidates, inconsistent lease or
Attempt gates, selected targets, and any non-false authority bit. It emits
only the validated observation or a metadata-only text rendering; it does not
re-evaluate inventory, choose a candidate, reserve a target, issue or renew a
lease, authorize execution, or contact a Runner. The cross-language fixture
is `docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json` and is
covered by the focused Rust command tests.
