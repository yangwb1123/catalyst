# Forge device fabric activation review request v1

`forge.device-fabric-activation-request/v1` is a review-only, canonical JSON
packet for the future `INVENTORY` or `OBSERVE` activation decision. It is
validated by `internal/devicefabricgate` and contains lifecycle metadata,
bounded evidence references, and explicit all-false authority markers.

The packet does not contain device identifiers, credentials, inventory rows,
routes, listener settings, placement, or task payloads. Its evidence and
acceptance fields are declarations for an external architecture/security
review; the validator does not read referenced artifacts, change ADR files, or
open any listener.

The current repository snapshot intentionally fails the staged gate: ADR-0039
is accepted but planning-only, while ADR-0113 and ADR-0114 are Proposed with
null acceptance metadata. The repository fixture
`forge-device-fabric-activation-review-proposed-v1.json` records that result.
The synthetic accepted fixture exists only to exercise the pure positive
decoder/evaluator and is marked `synthetic_review` with
`production_authorization:false` and every authority bit false.

The JSON Schema is published at
[`forge-device-fabric-activation-request-v1.schema.json`](forge-device-fabric-activation-request-v1.schema.json).
The threat model is
[`forge-device-fabric-activation-threat-model-v1.md`](../security/forge-device-fabric-activation-threat-model-v1.md).

Passing review validation never enables enrollment, heartbeat persistence,
inventory authority, placement, reservation, scheduling, dispatch, Runner
execution, migration, federation, or Audit publication. Those capabilities
remain separately governed and production routes remain default-off.
