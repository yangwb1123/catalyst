# Forge device credential lifecycle v1

`forge.device-credential-lifecycle/v1` is the pure metadata contract for the
short-lived, device-only transport credential described by ADR-0114. It
computes issue, revoke, and key-rotation replacement plans from exact caller
values. It never creates bearer material, authenticates an owner, reads a
clock, consumes a challenge, persists a row, publishes inventory, or grants
execution authority.

An issued credential is bound to one immutable owner tuple, device ID, key ID,
public-key digest, approval state, and key generation. The credential window
must be at least one second and at most one hour; issue and rotation require
an explicit observation time inside the window. Pending and approved devices
may receive a metadata candidate. Revoked approval is rejected, and a revoked
credential is terminal. Rotation changes the credential ID and key binding and
increments the generation rather than silently mutating an existing identity.

`OwnerBindingMatched` records only structural equality. The other authority
markers remain false, including `CredentialMaterialMade`. A future Go-owned
authority must authenticate the owner, verify device proof and approval, mint
the secret through a separately reviewed credential service, and atomically
persist the replacement before enabling any enrollment or heartbeat route.

The canonical vector is
`docs/contracts/fixtures/forge-device-credential-lifecycle-v1.json`.

Forge Core also exposes an explicitly injected owner-scoped
`POST /api/v1/device-enrollment-heartbeat/credential-candidate` boundary. It
applies the value transition to one lifecycle-registry member and stores only
the metadata candidate through same-revision CAS; the ordinary lifecycle
projection omits that optional field. The route has its own
`forge:devices:lifecycle:credential` scope and remains unmounted by production
constructors. Production device routes remain closed while ADR-0114 is
Proposed/null.
