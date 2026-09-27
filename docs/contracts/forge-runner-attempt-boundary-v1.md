# Forge Runner Attempt lifecycle dispatch boundary v1

`forge.runner-attempt-boundary/v1` is a pure, redacted observation after the
Runner execution-boundary preview and before any future effect adapter. It
reuses the Platform Core Attempt state graph and carries only owner,
Conversation/Run/Attempt, command, target, lease epoch, and lifecycle
metadata.

The dispatchable lifecycle edges are deliberately narrow:

- `accepted -> starting` through `begin_starting`
- `starting -> running` through `observe_running`

Other known lifecycle edges remain observable but are not dispatchable at this
boundary. Illegal edges fail closed. A valid observation is always
`preview_only=true`; Attempt persistence, reservation, execution
authorization, dispatch, and Audit publication are all false. It contains no
fencing token, argv, workspace, payload, or Runner response.

The canonical accepted observation is in
`fixtures/forge-runner-attempt-boundary-v1.json`. Forge Core and the Rust
Runtime domain receiver validate the value directly; Aero-ID, Aero-IM,
Aero-Vault, and Snaplink Audit Governance consume byte-identical mirrors with
the same strict field, lifecycle, readiness, and all-false authority checks.
The fixture is compatibility evidence only; it does not read or mutate a
lease, persist an Attempt, open transport, execute a command, or publish
Audit.

The accepted execution assembly also exposes an opt-in authenticated preview
at `POST /api/v1/conversations/{conversation_id}/runs/{run_id}/runner-attempt-boundary/preview`.
Its strict request is the Runner execution-boundary preview shape plus a
`transition` field. Core binds the declared owner and path, re-reads the
owner-private lease proof, derives the execution-boundary observation, and
then derives this lifecycle observation. The route is still a read-only
projection: it does not persist an Attempt, mutate or renew a lease, reserve
a target, authorize or dispatch a command, contact a Runner, execute argv,
persist a receipt, or publish Audit. The ordinary server constructor leaves
the route at 404 unless accepted `EXECUTE + P4` activation and independent
Runner authority configuration are supplied.
