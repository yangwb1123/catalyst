# Forge Runner execution boundary v1

`forge.runner-execution-boundary/v1` is the pure, metadata-only join before a
future live Runner effect adapter. It revalidates the current dispatch and
transport admission observations, the accepted device-fabric `EXECUTE + P4`
gate, and a separate deployment-owned Runner authority decision.

The authority decision is independent from P4 and the zero-value configuration
is disabled. The activation gate also retains the required lease-fencing,
cancellation/uncertain-work, Vault artifact authorization, and Audit outbox
evidence. A request with an active cancellation or an `uncertain` effect is
rejected; only `not_started` or explicitly `reconciled` effects are startable.

`execution_boundary_ready` means that the declarations are complete for a
future reviewed adapter. It never grants command, reservation, transport,
dispatch, execution, or Audit authority. `preview_only` remains true and all
authority fields remain false. The observation contains no fencing token,
argv, workspace reference, payload body, or Runner response.

`rejection_reasons` is always a JSON array. A ready observation uses `[]`,
which keeps strict Rust, Dart, Go, and downstream receiver decoders on one
wire shape instead of allowing a language-specific `null` slice.

This contract performs no I/O, does not read or mutate a lease, and is not a
production Runner route. ADR-0039 remains planning-only, ADR-0114 remains
Proposed/null, and the independent Runner authority decision is not accepted
by the current repository configuration.

## Authenticated preview projection

The optional Core route is:

`POST /api/v1/conversations/{conversation_id}/runs/{run_id}/runner-execution-boundary/preview`

The caller body has exactly `owner`, `conversation_id`, `run_id`, `attempt_id`,
`attempt_state`, `command`, `transport`, `expected_payload_sha256`, and
`controls`. `controls` has exactly `effect_state` and
`cancellation_requested`. The body never accepts activation, Runner authority,
lease status, or `evaluated_at_ms`; the authenticated server supplies those
values, reads the owner-private fenced lease, and samples the Coordinator
clock. The response has exactly the observation fields defined by the Core
type, including `mode`, the four gate/effect predicates, sorted
`rejection_reasons`, `preview_only`, and an all-false `authority` object.

Runtime CLI/TUI and Snaplink Console Web/App/Mobile are strict value-only
consumers of this projection. They pin the path and owner bindings, perform
one authenticated POST without unauthorized replay, and render no fencing
material, argv, workspace, transport payload, or Runner output. A ready
observation remains a preview and is not a permission to open a connection or
send a command.
