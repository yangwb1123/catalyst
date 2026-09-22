# Owner-scoped inventory snapshot canonicalization v1

`forge-device-inventory-snapshot-canonical-v1.json` freezes a pure value
boundary for a caller-declared owner-scoped inventory snapshot. Each snapshot
has a bounded identifier, a caller-supplied observation time, one exact owner
tuple, and rows carrying the same owner tuple. The owner and every row remain
unverified declarations.

Canonicalization validates the fixed values, copies the input rows without
mutating them, and orders rows by `(device_id, instance_id)` using bytewise
identifier order. A duplicate composite key or a row belonging to another
owner is rejected. The result has no inventory authority, reservation, or
execution meaning.

The optional `canonical_sha256` is a domain-separated integrity label over the
canonical values. Its preimage starts with
`forge.device-inventory-snapshot-canonical/v1`, a NUL byte, then length-prefixed
UTF-8 fields for snapshot ID, decimal observation time, owner tuple, and each
ordered row's identifiers and owner tuple. The digest is not an identity
proof, authentication, freshness check, or permission.

Go, Rust, and Flutter consume the same strict fixture. This slice does not add
a route, clock source, persistent table, registration, heartbeat listener,
discovery, reservation, scheduler, dispatch, or Runner execution. ADR-0039
remains planning-only and ADR-0114 remains Proposed with null acceptance
fields.

The Aero-ID, Aero-Vault, and Snaplink Audit Governance Go receivers and the
Aero-IM Rust receiver consume byte-identical mirrors with strict unknown-field
and duplicate-key rejection. They recompute the canonical ordering and digest
while keeping the owner and inventory declarations unverified and all authority
bits false.
