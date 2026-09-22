# Pure device inventory status projection v1

`forge-device-inventory-status-contract-v1.json` freezes a small, effect-free
projection that clients can use to display a caller-declared inventory row.
The projection receives approval, cordon, liveness, reservation, snapshot,
lease, and a fixed evaluation time. It never reads a clock or a registry.

The status precedence is `revoked`, `cordoned`, `offline`, `stale`,
`pending`, `reserved`, then `online`. A snapshot is fresh only when it is not
older than `stale_after_ms` and its declared lease expires after the fixed
evaluation time. `declared_eligible` is true only for `online`; it describes
the declaration comparison and is not a reservation, scheduler result, or
execution permission.

Unknown states, future snapshots, and a lease that precedes its snapshot are
rejected. The fixture keeps identity verification, heartbeat persistence,
authoritative inventory, reservation, execution, and dispatch false. Go and
Rust consume the same strict cases; no endpoint, storage, discovery,
heartbeat listener, scheduler, or Runner is introduced.

The Aero-ID, Aero-Vault, and Snaplink Audit Governance Go receivers and the
Aero-IM Rust receiver consume byte-identical mirrors with strict unknown-field
and duplicate-key rejection. Their tests recompute the same fixed-time cases
and keep every authority bit false.

This contract does not amend ADR-0039 and does not change Proposed-only
ADR-0114.
