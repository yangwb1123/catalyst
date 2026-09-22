# Forge device-aware fabric activation gate v1

`forge.device-fabric-activation-gate/v1` is the pure, fail-closed policy
boundary for the future `OFF -> INVENTORY -> OBSERVE -> EXECUTE -> MIGRATE ->
FEDERATE` progression. It consumes explicit ADR lifecycle metadata and a
reviewable evidence declaration; it performs no file or network I/O, changes no
ADR status, and never registers a device or opens a Runner listener.

`OFF` is allowed for the zero-value request. Every other mode requires ADR-0039
to be formally accepted with non-planning metadata, plus accepted ADR-0113 and
ADR-0114 decisions. Inventory and observation additionally require evidence for
the authenticated Coordinator owner boundary, device proof of possession,
owner approval/revocation, heartbeat CAS and freshness, owner-scoped reads,
default-off route closure, and security review. `EXECUTE` adds a separate P4
decision and independent Runner isolation, lease/fencing, cancellation and
uncertain-effect, Vault artifact authorization, and Audit outbox evidence.
`MIGRATE` and `FEDERATE` remain blocked until their own decisions exist.

The result contains the schema version, requested mode, an allow bit, and
sorted stable reason codes. It is suitable for startup/configuration checks so a future
route mount cannot accidentally treat a Proposed ADR or a caller-declared
inventory value as authority. The ordinary zero-value production `Run` path
remains device-route-free. An explicitly configured Accepted `INVENTORY` or
`OBSERVE` activation may mount the reviewed read-only inventory, lifecycle,
placement-preflight, and client-instance observation routes; it cannot mount
enrollment, heartbeat writes, credential issuance, reservation, dispatch, or
Runner execution. An Accepted `EXECUTE` activation now has a separate
admission-only assembly: it mounts those same owner-scoped observations plus
the existing consent, pending Run-intent, and pure Attempt/lease, Runner-receipt,
dispatch-plan, and reconciliation preflight handlers. That assembly accepts an
explicit owner-bound intent and complete comparison packet for review, but does
not select or adopt a device, issue a lease, reserve capacity, create a Run,
dispatch a Runner, execute work, or publish Audit. It may also expose the
`forge.scheduler-selection-preview/v1` comparison, whose selected IDs are
display-only and whose placement, reservation, lease, execution, dispatch, and
Audit authority fields remain false. `MIGRATE` and `FEDERATE` remain
unassembled even if their pure policy result is later allowed; they require
separate migration/federation routes and evidence. The repository's ADR-0113/
0114 acceptance fields remain null, so no activation manifest is enabled by
default.

Forge Server can optionally read an owner-private
`forge.device-fabric-activation-manifest/v1` file before startup. The file is
strictly decoded, rejects duplicate/unknown fields and symlinks, and is passed
through the same pure gate; an absent file is equivalent to the zero-value
`OFF` request. This manifest is a configuration check only. The separate
review-only packet and threat model are documented in
[`forge-device-fabric-activation-request-v1.md`](forge-device-fabric-activation-request-v1.md);
neither format changes ADR lifecycle state. Only a separately accepted runtime
configuration can mount the read-only observation routes described above.

The review-only input packet is defined by
[`forge-device-fabric-activation-request-v1.schema.json`](./forge-device-fabric-activation-request-v1.schema.json)
and decoded by `devicefabricgate.DecodeReviewRequest`. It admits only
`INVENTORY` and `OBSERVE`, requires explicit ADR-0039/0113/0114 lifecycle
metadata, and carries seven bounded evidence declarations plus an all-false
authority boundary. Duplicate keys, unknown fields, non-canonical bytes,
unsafe references, and any production/execution bit fail closed. The proposed
repository snapshot and synthetic positive review fixtures are under
`docs/contracts/fixtures/`; the synthetic fixture is a pure test vector and
does not authorize production activation. See
[`forge-device-fabric-activation-threat-model-v1.md`](../security/forge-device-fabric-activation-threat-model-v1.md)
for the threat boundary and future acceptance evidence.
