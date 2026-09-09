# Runtime Attempt Admission v1

> R0-C7 / FR-04a; completion is conditional on Sprint 151 same-tree acceptance.
> This is an internal storage adapter, not a StartAttempt wire,
> authorization decision, runnable Attempt, complete FR-04, or accepted ADR.

## Scope and ownership

Add `forge-runtime/crates/infrastructure/src/sqlite_execution/` as an isolated SQLite
adapter. It consumes ADR-0107's immutable request through getters; its private storage
codec restores only by rebuilding `AttemptRequestInput` and calling the frozen validator.
ADR-0107's four production files, ADR-0109's lifecycle source/API, Platform Core and Cargo
dependencies remain unchanged. There is no lifecycle consumer in this slice.

Only persist initial `requested` admission: immutable request, version-1/sequence-1
creation event and retained pending outbox entry in one `BEGIN IMMEDIATE` transaction.
No transition, observation, effect, sender, acknowledgement, pruning, receipt producer,
application service, protocol, CLI or existing Hub migration is added. In particular,
`requested` is not authorization or permission to execute. FR-04b must separately define
observation evidence before durable lifecycle transitions are admitted.

## Explicit connection boundary

`SqliteAttemptJournal::from_connection(Connection)` takes ownership of an explicitly
caller-opened on-disk main database. It rejects in-memory databases, attached databases,
active transactions and foreign/unknown schemas. An empty database is initialized with
this profile only; an existing profile is checked, never repaired or migrated. The store
retains its connection privately; it exposes no raw connection or arbitrary SQL method.

The caller owns filesystem selection, permissions, pathname/descriptor safety, SQLite
library/VFS trust and connection creation. This adapter does not provide a safe filesystem
opener, same-user isolation, publisher authentication, sandbox, daemon readiness or a
production deployment boundary. Existing Run/Group/Graph Hub databases are rejected.
Only test-owned temporary databases are opened by this implementation's tests.

Set and verify WAL, synchronous FULL, foreign keys and a bounded busy timeout; reject a
connection that cannot satisfy the profile. Verify the exact schema and full relational
record set within the same transaction used for every read or write. Concurrent writers
are serialized by SQLite, not an in-memory cache. External same-user mutation, rollback,
hardware/VFS dishonesty and database replacement are outside the integrity guarantee.

## Closed storage contract

Three STRICT tables hold admissions, canonical events and pending outbox references.
The profile has fixed `application_id` and `user_version=1`; exact table/index definitions
are validated. Admission identity and idempotency key are globally unique within this
database. Event/message identities are unique; a store-assigned positive contiguous
cursor orders all admissions. Each Attempt has exactly one event and outbox reference.
Foreign keys plus complete replay validation enforce both directions of these relations.

The private request record has version 1, a fixed format tag, and all fifteen frozen input
fields. It is exact compact, recursively key-sorted JSON with explicit optional nulls,
no unknown/duplicate/missing fields, no trailing bytes and no alternate spelling. Sets
use the request constructor's normalization. SHA-256 is domain-separated from Platform
Core and every other storage record. Re-encoding a decoded/validated request must reproduce
the exact stored bytes; stored digests and all redundant columns must match.
Expose only the pure `request_sha256(&AttemptRequest)` preparation helper so callers can
bind the creation payload without duplicating this private codec. It does not open storage,
produce an event, authenticate a request or reserve an identity.

Reuse Platform Core `EventEnvelope` canonical validation/codec/digest. Admission additionally
requires `source_component=Runtime`, exact request Attempt/scope/snapshot, aggregate version
and Attempt sequence both 1, `schema_name=forge.runtime.attempt_requested`, schema version 1,
no causation, no extensions, no payload artifact, and exactly the inline payload
`{"request_sha256": <storage request digest>, "state": "requested"}`. Event/message IDs,
actor, correlation and positive bounded occurrence time are caller declarations validated
by Platform Core, not generated, authenticated or treated as trusted observation here.
Do not copy the context Artifact into the event's payload Artifact slot.

## Admission and replay

`admit(&AttemptRequest, &EventEnvelope)` derives and validates canonical input before
mutation. Creation means expected aggregate version 0; an existing different request may
never replace that Attempt. Exact same-key request bytes AND event bytes return the original
stored result and `Replayed`, without appending an event, incrementing a cursor or creating
another outbox entry. Any difference is `Conflict`, even if only event metadata changed.
The first successful transaction returns `Created`; concurrent exact duplicates converge
on that result. Existing replay is checked before capacity rejection.

`get(attempt_id)` returns a validated owned admission or absence. `pending(after_cursor,
limit)` returns canonical creation events, their stable digests/cursors and a snapshot head;
the cursor is local to this connection's database, not a cross-store or transport token.
Repeated pages return the same facts. There is no ack/delete API, so pending delivery state
cannot be reclaimed before FR-06 defines authenticated delivery and cursor identity.

Use at most 1,024 admissions, 64 KiB per request, 16 KiB per event and 32 MiB total stored
request/event bytes. Pages accept 1..64 records with at most 256 KiB returned event bytes;
cursor 0 starts a page and a cursor beyond the snapshot head is invalid. Reject malformed
bounds, unknown profiles, noncanonical records, bad digests, missing rows, gaps, duplicate
identities and relational drift. These are bounded prototype limits, not retention or
constant-work guarantees; each operation audits the complete bounded journal.

Storage errors remain storage errors: `Invalid`, `Conflict`, `Capacity`, `Corrupt` or
`Unavailable`. An uncertain commit acknowledgement is `Unavailable`; explicit exact replay
after reopening resolves whether it committed. Missing history or a failed SQLite call
never changes lifecycle to `uncertain`, `failed` or retryable and never triggers an effect.

## Verification and completion

Exercise full request round trips and strict decoding mutations; event/request bindings;
same-key exact replay and conflicts; cross-Attempt cursor order; count/byte pages and limits;
reopen; concurrent same/different-key writers; foreign/schema/corrupt-row rejection; missing
event/outbox and gap detection; transaction rollback faults; and subprocess interruption
before commit versus after commit/before reply. No real model/provider is called.

Admit only exact source-path + whole-source-digest consumers in the existing boundary
tests, after Serde/codegen/path checks. Do not exempt a directory or permit lifecycle access;
reject `#[path]` reuse of the newly reviewed files. Frozen request/lifecycle inventories
remain intact. Fresh-context design and implementation/security reviews, Rust fmt/strict
Clippy/tests/build, architecture/governance and same-tree formal acceptance are required
before R0-C7 is complete. ADR-0110 remains Proposed/null regardless of test success.
