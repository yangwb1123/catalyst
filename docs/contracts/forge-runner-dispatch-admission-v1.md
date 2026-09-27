# Forge Runner dispatch admission preview v1

`forge.runner-dispatch-admission/v1` is the metadata-only recheck between a
durable fenced scheduler lease and a future Runner transport. The caller
supplies the owner, Conversation/Run/Attempt binding, Attempt state, direct
argv command declaration, lease proof, and an explicit evaluation time. Core
re-reads the owner-private lease registry under its file lock, compares the
target and epoch, recomputes the command digest, and reports the current,
active, binding, and Attempt-state predicates.

`admission_ready` is true only when the command binding, current lease proof,
active lease, and dispatchable Attempt state all agree. Rejection reasons are
sorted and deterministic. The response contains no fencing token, argv,
workspace, output, Runner response, or execution receipt. `preview_only` and
every authority bit are fixed to `true`/`false` respectively; a ready result
does not authorize a command or contact a Runner.

The authenticated candidate is
`POST /api/v1/conversations/{conversation_id}/runs/{run_id}/runner-dispatch-admission/preview`.
It is mounted only by the accepted `EXECUTE + P4` assembly when an explicit
private lease registry is configured. Runtime CLI/TUI and the Console
Web/App/Mobile Gate perform one strict POST and clear owner state after an
authorization failure. The default Console Gate remains request-free.

The Core value contract and route tests are in
`forge-core/internal/deviceplacement/runner_dispatch_admission.go` and
`forge-core/internal/appserver/runner_dispatch_admission_routes_test.go`.
The canonical receiver image is
`docs/contracts/fixtures/forge-runner-dispatch-admission-v1.json`.

The accepted `EXECUTE + P4` production harness also rechecks one live fenced
lease through the authenticated Forge `Run` boundary, consumes the same
request with Runtime CLI/TUI when a Runtime binary is configured, and runs the
opt-in Flutter Web/App/Mobile API reader. Releasing the proof is then read
again as an inactive admission; this verifies fencing without opening a
Runner transport or command execution path.

This slice does not create a Run or Attempt, authorize a command, reserve
capacity, renew or release a lease, contact or dispatch a Runner, execute
work, or publish Audit. ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.
