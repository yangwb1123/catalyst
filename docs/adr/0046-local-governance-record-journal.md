# ADR-0046: Local Governance Record Journal

- Status: Accepted
- Date: 2026-08-09
- Owners: Governance / Architecture / Runtime Engineering
- Extends: ADR-0045

## Context

ADR-0045 defines strict, bounded and cross-language `EvidenceRecord` and `KnowledgeClaim` v1 bytes, but deliberately has no persistence. A useful next slice must
preserve those exact bytes across restart and replay without prematurely implementing a truth ledger, knowledge lifecycle or authority system. Treating a latest
row as a current fact would be unsafe: v1 producers are still declarative, authoritative Claim states remain inadmissible, evidence freshness is not evaluated and
conflicts are not adjudicated.

The persistence decision is difficult to reverse because record identity, idempotency, append conflicts, sequence continuity and migration behavior become durable
contracts. The slice therefore stays local, additive and narrow enough to validate independently before Context, Grant, Approval, Transition or knowledge-apply
work is attempted.

## Decision

1. Add `GovernanceRecordJournal v1`, a local append-only journal for exact canonical `EvidenceRecord` and `KnowledgeClaim` v1 record sets only. It does not accept a
   new record kind, wire alias or authoritative state and does not modify the Evidence/Claim v1 schema, canonicalization, digest domains or golden bytes.
2. Accept one bounded `GovernanceRecordAppendRequest` containing a caller-supplied `idempotency_key` and one exact `canonical_record_set_json` string. A set contains
   1–256 records and at most 1 MiB UTF-8; the key is non-blank and at most 256 UTF-8 bytes. Validate canonical bytes, digests, shadow admissibility and request-local
   structure before opening the Hub. Resolve references to durable journal state inside the same immediate transaction, before any insert, so validation cannot race
   a concurrent append.
3. Compute `record_set_sha256` as SHA-256 over `"forgeos.governance.record-set.v1\0"` followed by the exact record-set UTF-8 bytes. Compute `request_sha256` over
   `"forgeos.governance.record-journal.append-request.v1\0"`, then unsigned 64-bit big-endian key length, key bytes, unsigned 64-bit big-endian record-set length and
   exact record-set bytes. Derive `batch_id` as `governance-record-batch-` plus `request_sha256`. Append time is receipt metadata and is excluded from both digests.
4. Make a bounded append atomic. Store the batch, every exact canonical record and its ordinal, and the structural-head projection in one transaction or store none.
   A record ID maps to one immutable byte sequence; `(record_kind, aggregate_id, sequence)` is unique. A new sequence must extend the exact structural predecessor,
   without a gap, and the existing or same-batch immediate predecessor must appear in `supersedes_record_ids`.
5. Resolve supporting, contradicting, derived and supersession references against the union of existing journal records and the request. Reject dangling, wrong-kind,
   wrong-subject and cyclic references. Admit at most 1,024 distinct stored dependency records, at most 16,777,216 canonical bytes across the candidate batch plus
   its loaded dependency closure, and at most 256 `derived_from_claim_record_ids` edges from a candidate to a transitive premise. These are public
   resource-exhaustion admissibility limits, not truth, authority or semantic-quality thresholds. Exceeding one rejects the atomic append; a batch cannot use
   insertion order or last-write-wins to hide a conflict.
6. Give an idempotency key exactly one request digest. The first append returns disposition `stored`. Repeating the same key with the same exact request returns the
   original durable receipt with disposition `exact_replay`; it does not generate a new time or row. The same key with different bytes, or the same records under a
   different key, is a conflict rather than replay. Failed batches have no partial receipt or projection update.
7. Expose metadata-only inspection by default. Exact `canonical_record_json` is returned only after an explicit reveal request. The receipt vocabulary is exactly
   `stored|exact_replay`; it never says accepted, approved, confirmed, trusted or completed.
8. Maintain one atomically updated, deterministically rebuildable `GovernanceStructuralHead` for each `(record_kind, aggregate_id)`. It means only “highest contiguous
   sequence stored by this journal.” Its fixed interpretation is `structural_sequence_only`; it is not current truth, active knowledge, valid evidence, freshness,
   conflict resolution, authority or a hard-gate verdict.
9. Add an additive SQLite v25 migration with initially empty batch, record and structural-head storage. Do not backfill Memory, ADR, files or pre-v25 tables. Opening a
   supported v24 database may migrate it to v25; read-only journal commands require the current schema and never create or migrate a database. Older binaries must
   reject v25 rather than silently downgrade it.
10. Keep all authority restrictions from ADR-0045. The journal does not authenticate a principal or collector, promote a Claim state, evaluate validity windows,
    select a conflict winner, apply knowledge, issue a Grant/Approval, advance a Transition or create a production effect. `forge accept` remains the sole completion
    authority.

## Public contract

`docs/contracts/governance-record-journal-v1.schema.json` defines the append request/receipt, record inspection, bounded inspection-list envelope and structural-head
shapes. Its `x-forgeos-reference-closure-limits` annotation freezes the three resource-exhaustion admissibility limits above. Append takes a path or bounded stdin
through `forge-runtime --idempotency-key KEY governance journal append --file PATH`. Reads are
`forge-runtime governance journal show RECORD_ID [--include-record]`,
`forge-runtime governance journal list [--kind KIND] [--aggregate-id ID] [--limit N] [--include-record]` and
`forge-runtime governance journal head KIND AGGREGATE_ID`. No public rebuild command is exposed in this slice; deterministic rebuild is an internal store recovery
contract.

`forge-init` and `forge-upgrade` distribute the governance contract, Skill and shadow checker assets; they do not install the Rust `forge-runtime` executable or a
SQLite journal. A scaffolded Agent may execute the commands above only after detecting a project-approved `forge-runtime` whose help/API surface is compatible with
`forgeos.governance-journal/v1`. When that executable is absent or incompatible, persistence is `not_executed`; the Agent must not claim `stored`, `exact_replay` or
durability without the command's matching receipt.

Metadata inspection reports the owning batch/ordinal, immutable identity, kind/aggregate/sequence, canonical digest and byte count, record creation time and original
append time. Lists are deterministically ordered by append time descending, then record ID descending by UTF-8 byte order. Explicit reveal returns the exact stored
record string and performs no semantic reinterpretation.

The compatibility vector is the 3,411-byte canonical record set assembled from the ADR-0045 golden records in ascending record-ID order with idempotency key
`journal-replay`: `record_set_sha256=d895441903bf26d4f68402e7f85a377f1d59127941e20ab2f411756d6d8c9650`,
`request_sha256=0fe2b4e4d6e58a4f9322256094365716016673a4710d86a5c0c6572bb9f7e00e`, and `batch_id` is the declared prefix plus that request digest.

## Compatibility and recovery

- Exact replay reconstructs the original receipt and never writes a second copy.
- Migration is expand-only; v24 data and existing behavior remain unchanged while all journal tables start empty.
- Structural heads can be deleted and rebuilt atomically from immutable journal rows; immutable batches and records are the recovery source.
- Corrupt schema, mismatched bytes/digests, non-contiguous history or a projection that disagrees with immutable records fails closed. This slice does not repair or
  discard immutable records automatically.
- Append and head inspection deliberately run `COUNT/MIN/MAX` over the covered `(record_kind, aggregate_id, sequence)` index before trusting a structural head. This
  is an integrity-first O(records-in-aggregate) check that detects a deleted middle sequence; the current local-journal capacity assumption is at most 100,000
  versions in one aggregate and a representative p99 summary scan below 100 ms. Those values are planning thresholds, not admission limits. Crossing either threshold
  requires a versioned rolling integrity accumulator plus periodic independent full-prefix audit before scale-up; optimization must not replace the missing-sequence
  guarantee with an O(1) head lookup alone.
- A future lifecycle/conflict/freshness view must be a separately versioned projection with explicit authority and migration semantics; it cannot rename the
  structural head or reinterpret old receipts.

## Consequences

- Candidate Evidence/Claim records can survive process restart and be replayed exactly, without turning storage into epistemic approval.
- Batch idempotency, immutable byte ownership and contiguous structural history are decided before durable data accumulates.
- Storing exact records increases local database size and makes explicit reveal a sensitivity boundary; metadata-only defaults reduce accidental disclosure but do
  not replace filesystem and process access controls.
- Callers must distinguish structural persistence from business adoption. A stored Claim may still be candidate, contested, stale in the real world or based on an
  unauthenticated producer.

## Rejected alternatives

- Store only parsed columns: rejected because exact canonical replay and cross-language byte identity would be lost.
- Name the projection `current_claim`, `truth` or `active_knowledge`: rejected because sequence position does not establish validity, freshness or authority.
- Treat duplicate bytes under a new idempotency key as success: rejected because it makes one durable batch ambiguous and weakens auditability.
- Backfill ADR/Memory automatically: rejected because legacy identity, provenance, conflicts and admissible states are not defined.
- Add lifecycle, authenticated approval, conflict arbitration and durable storage together: rejected because it would make several low-reversibility contracts
  inseparable and exceed the evidence available for this slice.
