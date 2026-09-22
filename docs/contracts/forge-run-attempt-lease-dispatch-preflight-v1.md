# Forge Run → Attempt → lease → dispatch preflight v1

`forge.run-attempt-lease-dispatch-preflight/v1` is a pure value contract that
joins caller-supplied Run status with the existing Attempt, Runner intent,
lease proof, and offline placement preview. It is a read-only comparison at
the placement declaration's fixed `evaluated_at_ms`; it does not read a Run or
Attempt store, read a clock, issue or renew a lease, select a target, reserve
capacity, schedule, dispatch, contact a Runner, persist a receipt, or publish
Audit evidence.

The outer owner, Conversation ID, and Run ID must exactly match the nested
Runner intent. A `nonterminal` Run, an Attempt in `accepted`, `starting`, or
`running`, an active lease at the fixed observation time, and at least one
declaratively ready candidate produce `declarative_preflight_ready: true`.
Terminal Run or Attempt state, an inactive lease, and an empty ready set are
reported as sorted `rejection_reasons`; they never authorize work.

The result is metadata-only. `selected_target_id` is always `null`,
`preview_only` is always `true`, and every authority field is always `false`.
Lease fencing material, command argv, workspace references, process output,
and error text are not represented. Consumers must validate the complete
observation before displaying it.

The canonical caller declaration is
`docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-request-v1.json`;
it contains only synthetic owner, placement, intent, and lease metadata and
is suitable for offline and test-mux consumers. The corresponding response
fixture is
`docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json`.
Forge Core evaluates the request fixture and requires the indented JSON result
to match that response fixture byte for byte. This closes the request/response
pair over owner, Run, Attempt, target, lease epoch, fixed observation time,
candidate counts, readiness, null selection, and all-false authority instead
of validating the two fixtures independently.

Forge Core's implementation and focused tests are
`forge-core/internal/deviceplacement/run_attempt_lease_dispatch_preflight.go`
and
`run_attempt_lease_dispatch_preflight_test.go`. The canonical cross-language
fixture is
`docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json`.
Forge Runtime's domain test and Snaplink Console's strict Flutter fixture
model consume the same envelope. Its authenticated Rust remote CLI/TUI and
Flutter typed API accept the caller declaration only through explicit
candidate seams; POSTs are one-shot and response bindings are rechecked. The
shared Sessions Gate can receive the fixture explicitly and render the
read-only card on Web/App/Mobile while its default value remains unset and
request-free.
This slice intentionally stops at preflight metadata; production device,
inventory, lease, scheduler, dispatch, and Runner routes remain closed under
ADR-0039 and the separate ADR-0113/0114 acceptance gates.
