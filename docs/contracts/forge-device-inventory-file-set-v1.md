# Persisted inventory file-set v1

`forge.device-inventory-file-set/v1` is a bounded, read-only restart image
for an explicitly injected Forge Core observation source. The envelope has
exactly `schema_version`, `owner`, and `states`; `states` contains at most 128
complete `PersistedInventoryState` values. The envelope owner and every state
owner must equal the caller's exact `(issuer, subject, tenant_id)` tuple.

The image is read only from a regular private `0600` file through `statefs`.
Symlinks, non-regular files, broad permissions, duplicate JSON member names,
unknown fields, trailing JSON values, malformed states, duplicate device IDs,
and duplicate Runner instance IDs reject the complete image. A valid image is
returned in deterministic `(device_id, instance_id)` order; no partial fleet
is exposed when any member is invalid.

This is an observation restart boundary. It has no clock, network, write,
heartbeat, enrollment, inventory authority, selection, reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit behavior. The
private appserver candidate may project it into the existing v1 or lossless v2
display envelope only when a focused test explicitly injects the source.
Production device routes remain unregistered and return 404 while
ADR-0039 is planning-only and ADR-0113/0114 remain Proposed with no
acceptance metadata.
