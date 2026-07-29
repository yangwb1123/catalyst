# ADR-0012: Strict Hub schema ownership validation

- Status: Accepted
- Date: 2026-07-29

## Context

SQLite schema versions 1 through 5 were delivered incrementally by ADRs
0007–0011. Opening a version-5 Hub originally validated only that the older
objects existed, then later tightened the three version-5 analysis tables.
That left an asymmetric failure mode: a weakened older `CHECK`, key, foreign
key, or index could survive reopen even though the application assumes the
original contract.

The Hub database is private application state, not a user-extensible SQLite
container. Unknown tables, views, triggers, or virtual-table shadows can also
change introspection behavior or introduce effects outside the Store APIs.
Existence checks therefore do not establish that the database implements the
schema the current binary was built to use.

## Decision

The version remains 5 because this decision changes validation, not stored
layout. The `main` catalog is exclusively Hub-owned and must contain exactly:

- 14 tables introduced by schema versions 1 through 5;
- 8 named explicit indexes;
- the implicit primary-key and unique indexes derived from those tables; and
- no non-internal views, triggers, virtual tables, or shadow tables.

Every open first validates the exact contract for its declared version before
executing any migration DDL; version 0 requires an empty application-owned
catalog. A direct version-5 open therefore validates the complete contract
before any Store operation. A valid version 0–4 open performs its normal
migration chain inside the existing `BEGIN IMMEDIATE`, runs the complete
version-5 validation, and commits only if validation succeeds. Failure rolls
back the schema version, new objects, and data changes together. A declared
layout mismatch is reported as corruption and is never repaired or normalized
automatically. SQLite corruption/not-a-database introspection failures retain
that classification; environmental SQLite failures such as exhausted locks or
I/O remain unavailable and are rolled back by the same transaction.

Expected state is generated once per process by executing the exact published
version-1 creation SQL and version-2 through version-5 migration SQL in an
isolated in-memory database. The on-disk database is then checked at two
independent levels:

1. `main.sqlite_schema` must have the exact object inventory and exact table
   and explicit-index SQL definitions.
2. `PRAGMA main.table_xinfo`, `foreign_key_list`, `index_list`, and
   `index_xinfo` must produce the same column, default, hidden/generated,
   primary/unique key, foreign-key, index origin/uniqueness/partial flag, key
   order, direction, and collation signatures.

Explicit indexes additionally retain exact names. SQLite-generated autoindex
names are not an API and are ignored; their semantic signatures are sorted and
compared instead. The catalog still pins the sorted owning-table multiset for
all 25 implicit indexes, so raw catalog owner drift cannot hide behind an
unstable name. Schema introspection uses schema-qualified PRAGMAs rather than
shadowable table-valued PRAGMA names.

The published version-1 through version-5 DDL literals are durable
serialization contracts. A test pins their length-framed SHA-256. They must
not be reformatted or weakened in place. A genuine future layout change
requires a new schema version. If a pinned SQLite upgrade legitimately
produces another stored representation, compatibility must be explicit through
a reviewed historical-definition set or migration; silently relaxing exact
validation is not acceptable.

The release goldens are:

- length-framed v1–v5 DDL SHA-256:
  `cb3b65a96f9d4434995ecc409acd7da256332f800142bc661e25f9ab7296ebf8`;
- domain-separated full structural-contract SHA-256:
  `790b05cb9b2727755829f42fae47e3d0193170acdba41415a2005444e797bbf9`.

## Security and compatibility boundary

This is fail-closed corruption detection for the application-owned schema. It
does not encrypt the database, authenticate its owner, provide a MAC or
signature, stop the same OS user from synthesizing a fully self-consistent
replacement database, or protect against filesystem changes after validation.
The existing private-directory and file-permission boundary remains unchanged.

SQLite and `rusqlite` stay lockfile-pinned and are tested offline. A dependency
upgrade must reopen legacy fixtures and pass the exact/structural contract
before release. Internal SQLite objects whose names begin with `sqlite_` are
not treated as Hub-owned catalog extensions, but every index reported for a
Hub table is still included in that table's structural signature.

## Rejected alternatives

- Validating only the newest tables preserves the original asymmetric gap.
- Comparing only raw DDL can miss a damaged derived index/catalog structure.
- Duplicating every constraint in handwritten Rust creates a second schema
  language that can drift from the migration source.
- Trusting autoindex names binds the contract to SQLite implementation details.
- Allowing arbitrary extra main-schema objects makes the private Hub database
  extensible without an adopted ownership or side-effect contract.
- Automatically rebuilding unexpected objects can destroy evidence and mutate
  a corrupt database before the caller has inspected it.

## Consequences

Fresh databases and valid version 1–4 databases still reach version 5 and
preserve their data. Any old or new owned-object mismatch, rogue main object,
or shadowed PRAGMA virtual table now prevents open. Migration-time detection is
atomic: the legacy schema, version, and data remain unchanged.

This decision adds no network, provider, credential, workspace, tool, model,
Conversation, Prompt, Run, publication, memory, or schema-version behavior.
