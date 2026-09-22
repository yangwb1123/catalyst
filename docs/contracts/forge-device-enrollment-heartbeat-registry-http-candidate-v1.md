# Forge device enrollment and heartbeat lifecycle registry HTTP candidate v1

This document describes a test-injected HTTP projection of the private
`forge.device-enrollment-heartbeat-lifecycle-file-set/v1` restart image. It
is a candidate transport for exercising owner binding and the file-set CAS
adapter; it is not an enrollment, heartbeat, inventory-authority, scheduling,
dispatch, or Runner execution API.

## Boundary

The candidate path is:

`/api/v1/device-enrollment-heartbeat/lifecycle-registry`

The handler is constructed only through the private candidate constructor and
receives an owner-bound store. The authenticated `(issuer, subject, tenant_id)`
tuple comes from the verified bearer token. No owner may be selected from the
request body.

`GET` requires `forge:devices:lifecycle:read`, reads one complete validated
image, and returns:

```json
{
  "schema_version": "forge.device-enrollment-heartbeat-lifecycle-file-set/v1",
  "owner": {"issuer": "…", "subject": "…", "tenant_id": "…"},
  "states": []
}
```

`PUT` requires `forge:devices:lifecycle:write` and accepts exactly one JSON
field, `states`. The handler reads the current image and asks the injected
store to replace the complete state set with exact-image CAS. A stale image
returns `409 lifecycle_registry_conflict`; malformed state returns a client
error; filesystem or source failures remain server-side errors. The response
uses the same envelope as `GET`.

An absent leaf is a valid create expectation for `PUT` but produces
`404 lifecycle_registry_missing` for `GET`. The private parent directory must
already exist and be private. The candidate never creates or repairs it.

The same owner-bound file source can be injected into the existing device
observation candidates for migration fixtures. In that composition,
`GET /api/v1/devices` returns the bounded v1 inventory observation and
`GET /api/v1/devices/observations/v2` returns the lossless v2 observation,
including registry revision, Runner generation, and heartbeat sequence. Both
paths remain observation-only and preserve false execution, reservation, and
dispatch markers.

## Availability

The production session constructor does not mount this path, and the outer
production route table does not forward it. Production therefore remains
default-off/404 while ADR-0039 is planning-only and ADR-0114 is Proposed with
null acceptance metadata. Mounting the handler in a focused test or migration
fixture does not change those lifecycle gates.
