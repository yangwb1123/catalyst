# Forge device enrollment and heartbeat lifecycle persistence image v1

`forge.device-enrollment-heartbeat-lifecycle-persistence/v1` is the
restart-boundary value contract for the future P3b durable lifecycle adapter.
It describes one complete owner/device binding, heartbeat observation, and
inventory observation image with one outer revision. The outer revision must
match both nested revisions, and the Runner fields carried by inventory must
match the heartbeat fields byte-for-byte at the value boundary.

The fixture is consumed as a pure value by Forge Core's
`PersistedEnrollmentHeartbeatLifecycleState` seam and by Forge Runtime's
domain contract test. It covers accepted initial/next images, split
revisions, Runner/device drift, generation drift, owner drift, and revision
zero. Capability collections are copied when an image is restored so a
caller-owned slice cannot mutate the restored value.

This contract does not open a route or perform file/database I/O. It does not
read a clock, verify cryptography, consume a challenge, issue credentials,
register a device, persist a heartbeat, make inventory authoritative, reserve
capacity, schedule or dispatch work, contact a Runner, execute a task, write a
receipt, or publish Audit evidence. Production enrollment, heartbeat, and
inventory paths remain exact 404 while ADR-0114 is Proposed with null
acceptance metadata.

Canonical fixture:
`docs/contracts/fixtures/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json`.
