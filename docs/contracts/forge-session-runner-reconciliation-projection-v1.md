# Forge session Runner reconciliation projection v1

`forge.session-runner-reconciliation-projection/v1` is a bounded, content-free
projection of a session-bound Runner receipt history whose latest observation
is uncertain. It carries only owner and Conversation/Prompt/Run references,
latest receipt metadata, and the manual-reconciliation state needed by a
reader.

The projection is display-only. `selected_target_id` is always `null`,
`automatic_retry` is always `false`, `preview_only` is always `true`, and every
authority flag is `false`. It does not authenticate an owner, persist a
receipt, choose a target, issue a lease, reserve capacity, dispatch or execute
a Runner, or publish Audit. A reader must reject unknown, duplicate, or
trailing fields and any owner/session, source-history, latest-value, flag,
selection, or authority drift.

Forge Core derives this value only from a complete, canonical
`forge.session-runner-receipt-history/v1` value whose latest disposition is
`uncertain`. The pure projector rejects completed or failed terminal histories;
it does not infer or schedule another Attempt.

The explicit authenticated candidate route is:

`POST /api/v1/conversations/{conversation_id}/runs/{run_id}/runner-reconciliation/preview`

It requires `forge:conversations:read`, accepts the canonical receipt-history
value as its body, binds its owner and Conversation/Run references to the
authenticated principal and path, and returns the derived projection. The
ordinary production session constructor does not mount the route. The route
reads no receipt store and performs no persistence, retry, target selection,
lease, Runner transport or execution, or Audit publication.
