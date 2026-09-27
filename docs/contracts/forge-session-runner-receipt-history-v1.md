# Forge session Runner receipt history v1

`forge.session-runner-receipt-history/v1` is a bounded, display-only reduction
of terminal receipt observations for one owner-bound Conversation/Prompt/Run.
It is a value contract for Core, Runtime CLI/TUI, Console Web/App/Mobile, and
the ecosystem receivers; it is not a receipt store.

The `receipts` array contains one or more already validated
`forge.session-runner-receipt-observation/v1` values, in nondecreasing
`observed_at_ms` order. Equal timestamps are ordered by strictly increasing
Attempt ID, and all Attempt IDs are unique. At most 16 observations are
accepted. A failed observation may be followed by another attempt. A
`completed` or `uncertain` observation closes the history, so a later receipt
after either value is rejected. The latest observation supplies the repeated
summary fields (`latest_*`, reconciliation flags, and `follow_up`).

An uncertain latest observation always has `reconciliation_required=true`,
`manual_review_required=true`, `follow_up="reconciliation_manual"`, and
`automatic_retry=false`. Completed and failed observations use `follow_up="none"`.
`selected_target_id` must be null, `preview_only` must be true, and every
authority field is false. The value contains no argv, workspace, output,
fencing token, lease grant, transport payload, or device credential.

Consumers use strict decoding: unknown, duplicate, or trailing JSON fields,
owner/session binding drift, time/order drift, summary drift, authority
elevation, selected targets, and lifecycle violations fail closed. Reading or
importing this value performs no network request, persistence, retry, lease
issuance, target selection, dispatch, Runner transport, process execution, or
Audit publication.

The canonical fixture is
`docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json`. It is
mirrored byte-for-byte into Aero-ID, Aero-IM, Aero-Vault, Snaplink Audit
Governance, and Snaplink Console. ADR-0039 remains planning-only, ADR-0114
remains Proposed/null, and the separate P4 execution decision remains the
effect gate.

## Authenticated preview boundary

An explicitly enabled candidate may `POST` this same canonical observation
envelope to
`/api/v1/conversations/{conversation_id}/runs/{run_id}/runner-receipt-history/preview`
with `forge:conversations:read`. Core rechecks the authenticated owner, the
Conversation/Run path binding, strict JSON shape, and the pure reduction before
returning the derived history. The ordinary server constructor leaves this
route at `404`; Runtime CLI/TUI and Console Web/App/Mobile only use it when a
caller supplies an explicit candidate request and origin. This boundary adds
no receipt read or write, retry, target selection, lease, Runner, or Audit
authority.
