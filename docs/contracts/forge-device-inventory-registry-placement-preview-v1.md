# Forge registry-backed placement preview v1

This document defines the candidate transport boundary for evaluating a task
against the owner-scoped lifecycle-registry image:

```text
POST /api/v1/device-placement/registry-preview
scope: forge:devices:placement:preview
```

The boundary is a read-only preview. It must not select a device, reserve
capacity, schedule or dispatch work, contact a Runner, execute a process, or
write a receipt.

## Request

The request body is one exact JSON object with one member:

```json
{"requirements": { ... }}
```

`requirements` has the exact field set already used by
`forge.device-inventory-placement-evaluation/v2`:

```text
os, architecture, min_cpu_cores, min_memory_bytes, min_storage_bytes,
runtime, gpu, data_residency_zones, minimum_trust_zone, sandbox_floor,
concurrency_slots
```

The body does not carry `owner`, `evaluated_at_ms`, a device list, a selected
target, authority flags, or server-owned cordon/reservation state. The owner
comes from the verified bearer principal, the observation time comes from the
server clock, and the candidate source reads the private owner-scoped
`forge.device-enrollment-heartbeat-lifecycle-file-set/v1` image. A caller
cannot choose another owner or rewrite the registry through this request.

## Response

The response reuses the value projection
`forge.device-inventory-placement-evaluation/v2` produced by
`PersistedInventoryPlacementV2Evaluation`. Its transport response has this
exact top-level field set:

```text
schema_version, evaluation_mode, source_schema_version, evaluation_owner,
evaluated_at_ms, notice, decisions, eligible_candidate_count,
selected_device_id, selected_instance_id, authority
```

The route response uses `decisions` because it is an evaluation result. The
offline fixture consumed by the Rust CLI/TUI and Flutter Console also carries
the input-side `requirements`, `observation`, and `expected` fields so those
clients can recompute and compare a complete value document. The two shapes
share the same schema version, decision fields, ordering, and authority
invariants; a future HTTP consumer must decode the response shape explicitly
instead of treating a response as a caller fixture.

The response must satisfy all of the following:

- `schema_version` is `forge.device-inventory-placement-evaluation/v2`.
- `evaluation_mode` is `offline_static_only` and
  `source_schema_version` is `forge.device-inventory-observation/v2`.
- `evaluation_owner` and `evaluated_at_ms` are the verified owner and server
  observation time used for the evaluation.
- `selected_device_id` and `selected_instance_id` are JSON `null`.
- Every authority member is `false`, including `placement_selected`,
  `reservation_created`, `execution_authorized`, and `dispatch_performed`.
- Every decision preserves revision, generation, heartbeat sequence, device
  ID, instance ID, reservation declaration, GPU count, aggregate available
  GPU memory, sorted exclusion reasons, and both unverified declaration
  markers.
- The decision set is deterministically ordered by `device_id` and then
  `instance_id`; `eligible_candidate_count` equals the number of decisions
  with no exclusion reason.

The source bridge and evaluator composition are covered by
`TestPersistedLifecycleRegistryPlacementPreviewContract`. That test also
pins the exact one-member request shape and the response field set before an
HTTP route is mounted. The Rust Runtime CLI/TUI and Flutter Console now have
explicit candidate transport consumers for this compact response. Each
consumer requires a caller-supplied requirements document (and, for the
Flutter Gate, an owner, candidate origin, and opt-in flag), rejects selected
or authority-bearing values, and keeps startup/sync request-free unless the
candidate is explicitly enabled.

The response fixture
`forge-device-inventory-registry-placement-preview-v1.json` is mirrored by
Aero-ID, Aero-IM's audit connector, Aero-Vault's governance relay, and
Snaplink Audit Governance. Their receiver tests use closed decoders and
reject duplicate/unknown/trailing fields, unsafe owner/time/counter values,
unsorted or repeated candidates/reasons, selected targets, and any authority
bit. These compatibility receivers do not publish Audit facts or grant
identity, inventory, placement, reservation, scheduling, dispatch, Runner,
or execution authority.

This is a candidate contract only. Production route construction remains
default-off/404 while ADR-0039 is planning-only and ADR-0114 is Proposed with
no acceptance recorded.
