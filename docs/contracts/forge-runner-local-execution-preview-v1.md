# Forge local Runner execution preview v1

`forge.runner-local-execution-preview/v1` is a deliberately explicit local
adapter seam for validating the next Forge execution boundary. It accepts an
already validated `forge.runner-execution-intent/v1` value, a caller-supplied
`forge-runner-command-terminal-receipt-v1` lease grant, and a fixed observation
time. A caller must inject the Runner-shaped executor; the adapter does not
discover a device, select a target, issue or persist a lease, or open a remote
transport.

The adapter invokes only that injected executor with the command's direct
`argv`, an empty stdin value, and the declared timeout. It counts returned
output bytes and uses a digest only as the opaque completed terminal receipt
value. Output and executor error text never enter the observation. A clean
non-zero exit becomes `failed`; an executor error becomes `uncertain` and
requires manual reconciliation; output beyond the command limit becomes
`failed` with `output_limit_exceeded`.

The returned value nests the existing Runner intent and session receipt
observations. It is metadata-only and has `preview_only: true`; every authority
bit (`device_identity_verified`, `command_persisted`, `reservation_created`,
`execution_authorized`, `dispatch_performed`, and `audit_published`) is fixed
to `false`. It does not create a durable Attempt, Run, receipt, inventory
record, reservation, or Audit event.

The Go implementation and focused tests live in
`forge-core/internal/deviceplacement/local_runner_preview.go` and
`local_runner_preview_test.go`. The direct-argv process test is test-only
evidence for the injected seam and does not enable any production route.

Forge Core also has a private test-only HTTP candidate at
`/api/v1/conversations/{conversation_id}/run-intents/{intent_id}/execution-readiness-preview`.
It requires the verified owner and `forge:conversations:read`, binds both path
identities to the submitted value, and invokes only an explicitly injected
`LocalRunnerPreviewAdapter`. The candidate returns the same metadata-only
observation and is not mounted by the production session constructor; the
normal route therefore remains `404`.

Forge Runtime exposes the candidate to authenticated CLI/TUI consumers through
`remote runner execution-readiness-preview --input FILE|-` and
`runner-execution-readiness-preview --input FILE`. The consumer validates the
request against the pure Runner intent and lease domains, posts exactly once to
the path-bound candidate, rejects response identity or authority drift, and
renders only command/attempt/target/disposition metadata. The TUI requires the
Conversation to be the selected owner-scoped session and clears that local view
after a 401/403. This remains a test-only injected execution seam; the remote
command does not retry the POST, create a Run, persist a receipt, select a
device, reserve capacity, or enable production execution.

Snaplink Console exposes the same candidate through an explicit
`ForgeConversationsApi.previewLocalRunnerExecutionReadiness` consumer. Its
typed request serializes the existing Runner intent and caller-supplied lease
grant, validates the Conversation/intent path before sending, disables the
normal bearer refresh/replay path for this potentially effectful POST, and
strictly rechecks the nested intent, session receipt, command identities,
observation time, and all-false authority in the response. The method is an
opt-in API seam; no default Gate or Sessions screen wiring calls it, and the
production route remains unmounted/404.

An opt-in Snaplink JWT integration test can now send one owner-bound request
through the same candidate to the direct HTTP probe, Rust CLI, Rust TUI, and
Flutter API process. The TUI first refreshes a minimal owner-scoped session
fixture, then selects the Conversation before issuing its single POST. The
injected executor call count is checked across clients, while private output
and fencing data remain absent from every response. This is cross-client
transport evidence only; it does not turn the candidate into a production
Runner, device, lease, receipt, or Audit authority.
