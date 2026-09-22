# Forge execution reconciliation observation v1

`forge.execution-reconciliation-observation/v1` is a pure restart-boundary
value contract. It joins caller-supplied Run status, Attempt state, lease
proof, and optional Runner terminal receipt at one explicit observation time.
The Go and Rust implementations classify the same input into a stable
`next_observation` value:

- `await_terminal` means the declared Attempt is dispatchable and its lease is
  active, while no terminal receipt was supplied.
- `lease_expired_without_terminal`, `attempt_not_dispatchable`,
  `attempt_terminal_without_receipt`, and `run_terminal_without_receipt` mark
  incomplete or stale evidence for review.
- `terminal_completed`, `terminal_failed`, and `terminal_uncertain` classify a
  fenced receipt whose disposition agrees with the declared Attempt state.
- `terminal_state_conflict` preserves a receipt/state mismatch for review
  instead of silently rewriting either declaration.

A receipt must match the exact lease proof and be observed inside that lease
window. The outer observation time cannot precede the lease or a supplied
receipt. All timestamps and epochs stay within the JSON-safe integer ceiling.
`automatic_retry` is always false, and `reconciliation_required` and
`manual_review_required` are derived from the classification.

The authority object is fixed all false. The contract reads no store or clock,
does not issue or renew a lease, does not select or reserve a device, does not
schedule or dispatch work, does not contact a Runner, and does not persist a
receipt or publish Audit evidence. It is an interoperability and recovery
input boundary for a future separately authorized execution adapter.

The canonical fixture is
`docs/contracts/fixtures/forge-execution-reconciliation-observation-v1.json`.
The focused Go and Rust tests are wired into
`scripts/test-forge-contracts.sh`.
