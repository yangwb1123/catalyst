# Forge pending-write recovery contract v1

`forge-pending-write-recovery-v1.json` is a value-only metadata contract for
an interrupted owner-scoped Conversation write. Go, Rust, and Flutter consume
the same fixture and derive the same retry guard.

The metadata identifies only the operation (`append_prompt` or
`create_conversation`), the Conversation when one already exists, the
expected aggregate version, an idempotency key, state (`pending` or
`unconfirmed`), and caller-supplied observation times. `unconfirmed` requires
reconciliation before a retry; every retry must reuse the same idempotency
key. The projection does not read a clock or decide whether a retry should be
performed.

Prompt content, title, scope, token, owner credentials, and Run payloads are
deliberately absent. The fixture fixes `body_included`, `run_created`,
`network_contacted`, and `persistence_written` to `false`. A caller may keep
the original body in its own transient or protected write path, but this
contract does not prescribe cross-process body persistence or automatically
replay a write.

The contract validates operation-specific identifiers, bounded uint64 values,
key syntax, and non-regressing caller observation times. It is a local
projection only: no endpoint, Hub/Runner contact, device discovery,
registration, inventory authority, reservation, scheduling, dispatch, or
process execution is added. ADR-0039 remains planning-only, ADR-0114 remains
Proposed/null, and P4 still requires a separately Accepted execution/security
decision.
