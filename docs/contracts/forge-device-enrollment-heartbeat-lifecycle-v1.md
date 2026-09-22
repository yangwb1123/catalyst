# Forge device enrollment and heartbeat lifecycle value contract v1

`forge.device-enrollment-heartbeat-lifecycle/v1` is a cross-language,
value-only preparation contract for the future P3b device boundary. It joins
the existing identity proof, heartbeat sequencing/CAS, persisted inventory CAS,
and fixed-time inventory projection models so Go Core, Forge Runtime, and
Snaplink Console can verify the same lifecycle without opening a device route.

The fixture is intentionally deterministic. The caller supplies the verified
owner declaration, configured device binding, challenge/proof test vector,
heartbeat, prior revision values, server observation time, lease duration, and
projection time. A successful case returns only a replacement plan and an
inventory projection. No function in this contract reads a clock, verifies a
cryptographic signature, consumes a challenge, writes storage, issues a device
credential, reserves capacity, schedules work, contacts a Runner, or grants
execution authority.

The lifecycle requires owner and key binding first. Pending approval stops the
joined transition with `approval_required`; an approved device then passes the
existing generation/sequence and lease checks. Heartbeat and inventory CAS
revisions are evaluated as one in-memory plan, so an inventory conflict never
exposes a partial heartbeat replacement. Projection is evaluated at the fixed
caller time and can report a stale declaration without making it executable.

The canonical cases cover pending approval, the first approved heartbeat,
replayed sequence, generation advance, server-clock rollback, inventory
revision conflict, skipped generation, expired capability projection,
revoked credential, and owner tuple drift. Every authority field in the
envelope is false. The production `Run` constructor does not mount an
enrollment or heartbeat route; `/api/v1/devices`, registration, heartbeat, and
inventory authority remain closed while ADR-0114 is Proposed with null
acceptance metadata.

Canonical fixture:
`docs/contracts/fixtures/forge-device-enrollment-heartbeat-lifecycle-v1.json`.

The Go value layer also exposes a composite replacement image for a future
durable adapter. `CommitPersistedEnrollmentHeartbeatLifecycle` uses one outer
revision and derives the nested heartbeat and inventory revisions from the
current image, while `RestorePersistedEnrollmentHeartbeatLifecycle` rejects
split images, owner/device binding drift, and capability aliasing. These
functions are deliberately unmounted from HTTP and do not write a file or
database; they provide the atomic restart and stale-writer boundary required
before an accepted P3b persistence implementation can be wired.
