# Lossless offline device inventory observation v2

`forge-device-inventory-observation-v2.json` is a bounded, caller-supplied
observation envelope. It preserves the values that the v1 display contract
cannot represent without loss: a persisted revision, Runner generation and
heartbeat sequence, a declared reservation state, and the complete declared
GPU list. The values remain unverified input; they are not an authenticated
device identity, a heartbeat freshness proof, or a reservation record.

The envelope uses `schema_version: "forge.device-inventory-observation/v2"`
and `evaluation_mode: "offline_static_only"`. It carries a JSON-safe
`evaluated_at_ms`, one declared owner tuple, and device rows sorted by
`(device.device_id, instance_id)`. The owner and inventory flags must both be
`true`. `execution_authorized`, `reservation_created`, and
`dispatch_performed` must all be `false`.

Each row requires non-zero, JSON-safe `revision`, `generation`, and
`heartbeat_sequence` values. Device owners must equal the envelope owner.
Approval is `pending`, `approved`, or `revoked`; cordon is `clear` or
`cordoned`; reservation is the declaration `none` or `reserved`; and liveness
is `online` or `offline`. Snapshot and lease times must be JSON-safe and the
lease interval must remain within the capability lease boundary of 1,000 to
600,000 milliseconds. Operating system, architecture, and runtime labels use
the canonical lowercase tag grammar. CPU, memory, and storage values retain
the bounded resource limits; v2 leaves data-residency zones, sandbox levels,
and concurrency declarations empty or zero because those values are not
available in the persisted capability image.

GPU rows are sorted by unique GPU ID. Each GPU has a bounded vendor label,
non-zero total memory, available memory no greater than total memory, and
JSON-safe values; the aggregate available GPU memory must also remain
JSON-safe. Unknown fields, duplicate JSON keys, duplicate device or Runner
rows, owner drift, unsorted rows, invalid identifiers, unsafe counters or
resources, invalid lease windows, GPU ordering/memory changes, and any
authority mutation fail closed. The receiver does not accept or produce a
selected target or schedulable claim.

The canonical fixture is used for offline interoperability tests only. A
receiver may decode and validate the value or render it as metadata, but it
does not read or write inventory storage, ingest heartbeats, discover or
enroll devices, authenticate a Runner, reserve capacity, schedule or dispatch
a Run, contact a Runner, execute work, persist a receipt, or publish Audit
evidence. `reservation_state: "reserved"`, revisions, generations, and
heartbeat sequences remain caller declarations. This contract does not amend
ADR-0039, ADR-0113, or ADR-0114; production device and execution routes stay
closed until their separate acceptance decisions.

The Aero-ID, Aero-Vault, and Snaplink Audit Governance Go receivers and the
Aero-IM Rust receiver consume byte-identical fixture mirrors with strict
unknown-field and duplicate-key rejection. Their parity tests preserve the
unverified owner/resource boundary and do not emit a selected target or
schedulable claim.
