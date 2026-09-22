# Forge device inventory placement batch evaluation v1

`forge.device-inventory-placement-batch-evaluation/v1` is a bounded,
caller-supplied dry-run value. It compares a placement requirement against a
fixed set of persisted-inventory cases and preserves the expected decision or
stable error for each case. The canonical value is
[`forge-device-inventory-placement-batch-evaluation-v1.json`](fixtures/forge-device-inventory-placement-batch-evaluation-v1.json).

The envelope binds one declared evaluation owner and evaluation timestamp to
the `forge-device-inventory-placement-input/v1` source. It contains exactly
six named cases in the canonical fixture (`online`, `pending`, `expired`,
`stale`, `offline`, and `future`), each with a device/Runner pair and an
expected revision, match flag, and deterministic exclusion-reason list. The
`future` case carries explicit observation and capability-lease timestamps so
consumers can verify future-snapshot rejection without consulting a clock.

Receivers must reject duplicate or unknown JSON members, trailing values,
owner/time drift, duplicate device or instance identities, invalid bounds, and
authority mutations. `selected_device_id` and `selected_instance_id` are null;
all authority booleans are false. `empty_inputs_allowed` records that an empty
candidate list is a valid read-only result. The error cases are
`owner_mismatch`, `duplicate_device`, `duplicate_instance`, and
`invalid_evaluated_at` with their stable error strings.

This contract carries no authenticated device identity, heartbeat acceptance,
authoritative inventory, target selection, reservation, scheduling, dispatch,
Runner execution, receipt persistence, or Audit publication. It is suitable
for byte-parity checks across Forge Core, Runtime, Snaplink Console, Aero-ID,
Aero-IM, Aero-Vault, and Snaplink Audit Governance while ADR-0039 remains
planning-only and ADR-0114/P3b/P4 remain gated.
