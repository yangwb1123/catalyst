# Forge scheduler selection preview v1

This contract carries the deterministic candidate comparison used by the
accepted `EXECUTE + P4` admission path:

```text
POST /api/v1/device-placement/scheduler-preview
scope: forge:devices:placement:preview
```

The request is an owner-bound comparison for one Conversation, Run, and
Attempt. It contains no device ID, lease token, reservation, credential,
Runner command, or authority marker supplied by the caller:

```json
{
  "conversation_id": "conversation-1",
  "run_id": "run-1",
  "attempt_id": "attempt-1",
  "requirements": { "...": "the exact placement requirements object" }
}
```

The owner is taken from the verified bearer principal. The server reads the
owner-private lossless inventory observation, evaluates it with the
Coordinator clock, and returns one value-only comparison. An explicitly
configured `EXECUTE` assembly may also read the same owner-private policy
registry used by the fenced scheduler-lease route. That registry supplies
residency, trust, sandbox, and concurrency declarations missing from the
lifecycle image; every row is bound to the inventory device/instance and its
revision, generation, and heartbeat sequence. If the policy image is absent,
the preview keeps the lifecycle-only comparison. If it is present but missing,
foreign, stale, or malformed, the route fails closed instead of showing a
candidate that a later lease claim could not use.

The route does not create a Run or Attempt, persist a lease, reserve capacity,
contact a Runner, dispatch work, execute a process, or publish Audit.

## Response

The response has this exact top-level field set:

```text
schema_version, evaluation_mode, owner, conversation_id, run_id, attempt_id,
evaluated_at_ms, candidate_count, eligible_candidate_count,
selection_available, selection_reason, selected_device_id,
selected_instance_id, preview_only, authority
```

The response must satisfy these invariants:

- `schema_version` is `forge.scheduler-selection-preview/v1` and
  `evaluation_mode` is `pure_scheduler_selection_preview`.
- The owner and Conversation/Run/Attempt IDs are the verified/request-bound
  values. IDs are bounded ASCII identifiers and the evaluation time is a
  positive JSON-safe integer.
- `candidate_count` and `eligible_candidate_count` are bounded, and the
  eligible count does not exceed the candidate count.
- When `selection_available` is true, both selected IDs are present,
  `eligible_candidate_count` is positive, and `selection_reason` is
  `first_sorted_eligible_candidate`. The selected pair is the first eligible
  `(device_id, instance_id)` after deterministic sorting.
- When `selection_available` is false, selected IDs are JSON `null` and
  `selection_reason` is `no_eligible_candidate`.
- `preview_only` is true and every member of `authority` is false:
  `placement_selected`, `reservation_created`, `lease_issued`,
  `execution_authorized`, `dispatch_performed`, and `audit_published`.

The canonical response fixture
`forge-scheduler-selection-preview-v1.json` is mirrored by Aero-ID,
Aero-IM's audit connector, Aero-Vault's governance relay, and Snaplink Audit
Governance. Their closed decoders reject duplicate, unknown, missing,
trailing, unsafe, owner/identifier, reason, selected-ID, and authority
mutations. These receivers provide interoperability evidence only; they do
not publish an Audit fact or grant identity, inventory, placement, lease,
reservation, scheduling, dispatch, Runner, or execution authority.

Production remains default-off outside the accepted `EXECUTE + P4` assembly;
`OFF`, `INVENTORY`, and `OBSERVE` keep the route closed, while `MIGRATE` and
`FEDERATE` remain unassembled.

The v2 observation intentionally carries unverified inventory declarations.
The preview therefore cannot prove device identity, liveness, capacity,
approval, reservation state, or lease ownership. The policy registry makes
the comparison policy-complete for the lease boundary; it does not turn the
declarations into device or execution authority. The route does not read or
write a Run, Attempt, registry, reservation, lease, credential, Runner, or
Audit outbox, and it has no retry or dispatch semantics. A later P4-approved
adapter must revalidate the observation, acquire a fenced durable lease, and
record an auditable Run/Attempt transition before any Runner transport is
opened; it cannot treat `selected_*` as authority.
