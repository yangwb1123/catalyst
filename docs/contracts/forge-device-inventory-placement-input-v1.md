# Forge device inventory placement input v1

`forge.device-inventory-placement-input/v1` defines the pure conversion of a
single persisted inventory value into a candidate/input for a future placement
evaluator. It is a value contract only. It has no route, database, clock,
network, Runner, selection, reservation, dispatch, or execution behavior.

The input copies only persisted inventory fields: revision, exact declared
owner tuple, device and Runner identities, state declarations, observation and
lease timestamps, and the canonical capability snapshot. The owner tuple and
capabilities remain unverified declarations.

The converter requires exact equality between the supplied evaluation owner
and the persisted owner. A malformed persisted state, including a Runner whose
device differs from the persisted device, fails with the persisted boundary's
stable error (`runner_device_mismatch`). A foreign evaluation owner fails with
`owner_mismatch`.

Observation and capability-lease timestamps must also remain at or below the
JSON-safe integer ceiling (`9007199254740991`). Go and Rust reject a persisted
value that exceeds that ceiling with `invalid_persisted_inventory_placement_input`
before constructing the placement candidate. This keeps the heartbeat-to-
placement boundary stable for Web, Flutter, Go, and Rust consumers.

Persisted inventory does not contain data-residency, trust-zone, sandbox, or
concurrency information. Every successful output must carry exactly these
closed unknown values:

| Field | Required value |
| --- | --- |
| `data_residency_zones` | `[]` |
| `trust_zone` | `"unknown"` |
| `sandbox_levels` | `[]` |
| `concurrency_limit` | `0` |
| `active_concurrency` | `0` |
| `owner_declaration_unverified` | `true` |
| `policy_attributes_unverified` | `true` |

Consequently, a placement policy that requires any of those attributes must
exclude the input. It must not infer residency, trust, sandbox, or capacity
from an owner, capability snapshot, state string, timestamp, or resource
value. The conversion itself does not evaluate placement policy or choose a
device.

The strict shared fixture is
[`forge-device-inventory-placement-input-v1.json`](fixtures/forge-device-inventory-placement-input-v1.json).
It covers online, stale, expired, pending, cordoned, revoked, and offline
values, exact-owner rejection, Runner/device binding rejection, and absent
policy attributes that fail closed. All authority booleans are false.

The same fixture is strictly consumed by Aero-ID, Aero-IM, Aero-Vault, and
Snaplink Audit Governance. Their receivers reject unknown, duplicate, and
trailing members plus owner, Runner/device, timestamp, and authority drift;
they keep this value as offline interoperability evidence and do not publish
inventory or placement authority.
