# Persisted inventory file read adapter v1

The Forge Core Go value contract now has an explicitly injected, read-only
filesystem adapter for restart evidence. It reads one complete
`forge.device-inventory-file/v1` envelope from a private regular file, rejects
symlinks and oversized or ambiguous JSON, restores the existing persisted
inventory value contract, checks the exact owner tuple, and returns only the
fixed-time display projection. `statefs.ReadRegularUnmodified` preserves the
file image and permissions during the read.

The adapter is a storage-boundary candidate, not an enrollment or heartbeat
store. It does not write state, obtain a clock, authenticate a device, accept a
heartbeat, or publish an inventory route. The private appserver candidate can
receive this adapter through explicit test injection and render the existing
v1/v2 owner-scoped observation envelopes; production route constructors do not
mount it and `/api/v1/devices` remains unregistered.

Focused tests create a 0600 preview image, restore it from a separate test
process, preserve the revision after a pure compare-and-swap replacement,
reject a replayed revision, reject foreign owner tuples, and reject duplicate
JSON members, missing files, and symlink aliases. All observation, reservation,
selection, scheduling, dispatch, Runner, execution, receipt, and Audit
authority bits remain false. This adapter does not amend ADR-0039 or
Proposed-only ADR-0114.

An opt-in `FORGE_INVENTORY_FILE_E2E=1` test mounts the file-backed candidate
only on an explicit test mux, authenticates a Snaplink JWT, and reads a
two-instance v2 observation from separate Rust CLI and TUI processes. When
`FORGE_CONSOLE_E2E=1` is also set, the same owner-bound response is decoded by
the Flutter API E2E. The test also checks the normal production route
constructor remains 404. With the opt-in variable unset, it skips and no
external process or device route is contacted. The multi-instance envelope
shape and atomic replacement boundary are specified separately in
`forge-device-inventory-file-set-v1.md`.
