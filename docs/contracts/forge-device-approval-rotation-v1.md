# Forge device approval, revocation, and key rotation v1

`forge.device-approval-rotation/v1` is a Forge Core Go value contract for the
device enrollment lifecycle proposed by ADR-0114. It is deliberately below
authentication and persistence: the caller supplies a current device value,
an exact owner/device tuple, and one operation; the pure transition returns a
replacement plan or a stable rejection.

The only approval vocabulary is `pending`, `approved`, and `revoked`. A
pending device may be approved or revoked. An approved device may be revoked.
Revocation is terminal: approval, another revoke, and key rotation all fail
after a device reaches `revoked`. Repeating approval on an already approved
device is rejected as an invalid transition so a caller cannot confuse a
replayed request with a new lifecycle event.

Key rotation is available while a device is pending or approved. It preserves
the exact `device_id`, owner tuple, and approval state, replaces the key ID and
public-key digest, and computes `key_generation + 1`. The input does not choose
the next generation. The new key material must be valid and must differ from
the current key; generation overflow fails closed.

Every operation repeats the current state's validation and compares the
request's owner and device ID byte-for-byte with that state. Unknown approval
states, unknown operations, malformed identifiers or digests, owner drift,
device drift, terminal revocation, and invalid transitions are rejected with
stable error codes. Owner strings are structurally bounded and are not
normalized.

`Apply` performs no owner authentication, key proof, cryptography, clock read,
credential issuance, storage write, network operation, inventory publication,
reservation, scheduling, dispatch, or execution. Its `owner_binding_matched`
flag records an exact value comparison only; `owner_authenticated`,
`persisted`, `credential_issued`, `inventory_authoritative`, and
`execution_authorized` remain false. A future Go-owned authority and storage
layer must authenticate the owner and atomically persist the replacement before
exposing any live enrollment or inventory route.

The canonical fixture is
`docs/contracts/fixtures/forge-device-approval-rotation-v1.json`; its schema
is `forge-device-approval-rotation-v1.schema.json`. Production device routes
remain closed under ADR-0039, and ADR-0114 remains Proposed/null.

## Owner-scoped lifecycle candidate transport

Forge Core's explicitly injected `POST
/api/v1/device-enrollment-heartbeat/approval-candidate` accepts only
`device_id`, `action`, optional rotation key values, and an expected lifecycle
revision. The verified bearer supplies the owner. The handler reads the
owner-scoped lifecycle registry, applies this pure transition, and publishes
the resulting `approval_candidate` value through the existing exact-image
file CAS. The live `DeviceBinding`, heartbeat, inventory, credential state,
and Runner image remain unchanged; the registry revision is unchanged because
this is a candidate plan rather than a lifecycle observation.

The candidate is independently gated by
`forge:devices:lifecycle:approval`, rejects query/unknown/duplicate fields,
owner/device/revision drift, terminal transitions, and stale CAS images, and
returns `preview_only=true`, `candidate_published=true`, and an all-false
authority object. The candidate route is only wired by explicit test or
migration configuration. Production remains 404/default-off: no credential
issuance, cryptographic proof, enrollment, heartbeat acceptance, inventory
authority, selection, reservation, scheduling, dispatch, Runner execution,
receipt, or Audit publication occurs.
