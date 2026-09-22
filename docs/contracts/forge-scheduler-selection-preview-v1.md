# Forge scheduler selection preview v1

`forge.scheduler-selection-preview/v1` is a deterministic comparison result
for the accepted `EXECUTE` admission surface. It consumes an owner-scoped,
lossless v2 inventory observation through the existing placement evaluator and
may declare the first eligible `(device_id, instance_id)` after stable lexical
ordering. The declaration is useful to CLI, TUI, Web, desktop App, and Mobile
clients when they render one shared scheduling preview.

The declaration never means that a target was adopted. Every response carries
`preview_only: true` and an authority object with these fields fixed to
`false`:

```json
{
  "placement_selected": false,
  "reservation_created": false,
  "lease_issued": false,
  "execution_authorized": false,
  "dispatch_performed": false,
  "audit_published": false
}
```

The preview request is a strict JSON object containing `conversation_id`,
`run_id`, `attempt_id`, and the existing `requirements` object. The server
binds all three identifiers and the requirements to the authenticated JWT
owner, samples its Coordinator clock, reads the owner-private v2 source, and
re-evaluates that source before selecting. A missing candidate returns
`selection_available: false`, `selection_reason: "no_eligible_candidate"`,
and null selected IDs. A successful declaration returns
`selection_reason: "first_sorted_eligible_candidate"` and both selected IDs.

The v2 observation intentionally carries unverified inventory declarations.
The preview therefore cannot prove device identity, liveness, capacity,
approval, reservation state, or lease ownership. It does not read or write a
Run, Attempt, registry, reservation, lease, credential, Runner, or Audit
outbox, and it has no retry or dispatch semantics. A later P4-approved
adapter must revalidate the observation, acquire a fenced durable lease, and
record an auditable Run/Attempt transition before any Runner transport is
opened; it cannot treat `selected_*` as authority.

The production path is mounted only by an explicitly accepted
`EXECUTE + P4` activation. Default, `OFF`, `INVENTORY`, and `OBSERVE` route
assembly stays closed, and `MIGRATE`/`FEDERATE` remain unassembled.
