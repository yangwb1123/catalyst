# Forge device enrollment and heartbeat registry file set v1

`forge.device-enrollment-heartbeat-lifecycle-file-set/v1` is a private
restart image for a future Go-owned device registry. It stores up to 128
complete `PersistedEnrollmentHeartbeatLifecycleState` values under one exact
owner tuple. Each value carries the immutable device binding together with its
current heartbeat and inventory observations, so a reader cannot assemble a
device from split restart images.

The Go adapter validates a regular `0600` file in a private parent directory,
rejects symlinks and hard links, bounds the image, rejects unknown or duplicate
JSON fields and trailing values, requires the envelope and every member to
match the configured `(issuer, subject, tenant_id)`, rejects duplicate device
or Runner instance IDs, restores nested revisions and capabilities, and
returns deterministic `(device_id, instance_id)` ordering. A missing file is
reported separately from a malformed or foreign image.

The adapter exposes a candidate-only write boundary when an explicit private
file-set adapter is injected by a test or migration tool. A writer must read a
snapshot first and replace the complete validated image with an exact
byte-and-mode compare-and-swap. It preserves the owner tuple, validates every
member, sorts the image deterministically, writes mode `0600`, and reports a
stable conflict when the tracked file changed. A missing leaf is a valid empty
snapshot only when its already-existing parent is private; the adapter never
creates or repairs parent directories. The read-only adapter remains the
restart-image boundary for callers that do not need candidate writes.

Neither adapter performs clock, proof-of-possession verification, challenge
consumption, credential issuance, enrollment listening, heartbeat ingestion,
HTTP routing, inventory authority, reservation, placement, scheduling,
dispatch, Runner execution, receipt, or Audit publication. Identity proof and
heartbeat values remain pure contracts that a separately authorized
transaction must evaluate before any future persistence write.

The adapter is not wired into production routes. ADR-0039 remains
planning-only and ADR-0114 remains Proposed with null acceptance metadata;
device routes therefore stay default-off/404.
