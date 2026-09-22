# Forge Run execution evidence v1

`forge.run.execution-evidence.v1` is a pure binding of two existing metadata
observations: an owner-scoped `forge.run.observed.v1` Run summary and a
preview-only `forge.session-runner-receipt-observation/v1` terminal receipt.
It gives clients one bounded value for displaying the relationship between a
Run and a Runner receipt.

The value carries an opaque owner reference, Conversation/Run/Prompt IDs,
bounded Attempt/target/command identities, the command digest, Run status,
receipt disposition and receipt observation time. It contains no argv,
workspace, output, prompt, message, provider, token, lease token, credential,
artifact, or raw owner fields. The Run status is an observation and need not
be terminal when a separately observed receipt is present.

`uncertain=true` and `reconciliation_required=true` are retained together with
the receipt's manual follow-up semantics; this value never implies automatic
retry. All authority fields are closed and always `false`. Unknown fields,
foreign owner references, mismatched Conversation/Run/Prompt IDs, selected
targets, and authoritative nested observations must be rejected.

Projection has no clock, persistence, outbox, network, selection, reservation,
lease issuance, Runner transport, process execution, artifact transfer, or
Audit publication behavior. The canonical example is
[`forge-run-execution-evidence-v1.json`](fixtures/forge-run-execution-evidence-v1.json).
