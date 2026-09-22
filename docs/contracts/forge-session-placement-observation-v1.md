# Forge session placement observation contract v1

`forgeos.session-placement-observation-contract/v1` binds one caller-declared
owner tuple, Conversation ID, and Run ID to a deterministic placement
comparison. The fixture embeds the existing
`forge.device-placement-policy-parity-test/v1` request so Go, Rust, and
Flutter exercise the same resource and policy declarations.

The observer consumes a caller-supplied evaluation timestamp. It validates the
session binding, pairs each declared device with one declared Runner instance,
reuses the offline placement evaluator, and sorts decisions by `device_id`
(with the existing evaluator's stable exclusion-reason order). Each decision
retains both `device_id` and `instance_id`, and carries only
`matches_requirements` plus exclusion reasons. A mismatched owner declaration
is reported by the placement comparison; it is not silently rebound.

The observation always marks owner and device declarations as unverified. The
selected device and instance are always `null`. Every field in `authority` is
fixed to `false`:

- `identity_verified`
- `heartbeat_persisted`
- `inventory_authoritative`
- `reservation_created`
- `execution_authorized`
- `dispatch_performed`

This is a value-only contract. It does not read a clock, contact a Hub or
Runner, discover or register devices, persist inventory, reserve capacity,
select a target, schedule work, dispatch a command, or execute a process. The
fixture is consumed by Go's `deviceplacement` package, Rust Runtime's domain
placement model, and Flutter Console's `forge_session_placement` model. The
contract test script covers all three consumers and a duplicate-instance
failure case.

ADR-0039 remains planning-only, ADR-0114 remains Proposed with no acceptance
fields, and the separate P4 execution/security decision remains required
before any authority-bearing placement or Runner path is exposed.
