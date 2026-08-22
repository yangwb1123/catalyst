# EvidenceRecord and KnowledgeClaim v1 portable validation

Use this package only to validate an already-authored exact record-set array under the Accepted ADR-0045 authority-free shadow semantics. The portable delivery decision does not change the wire, canonicalization, state matrix, digest domains, or positive result.

## Supplied-bytes boundary

Invoke exactly:

```text
python3 -I -B scripts/validate.py < RECORD_SET.json
```

The adapter accepts no arguments, repository root, file path, URL, clock, provider, identity, policy override, or output destination. It reads only stdin through an explicit EOF; a temporarily exhausted nonblocking stream is incomplete and fails closed. It loads its bundled implementation from the adapter-anchored directory. It does not create an EvidenceRecord or KnowledgeClaim, observe a source, run a command, repair input, sort arrays, insert a digest, return canonical records, or write state.

The implementation recomputes canonical payload bytes and kind-domain-separated digests only to compare them with caller-supplied values. That internal comparison is validation, not authorship, sealing, attestation, or signature.

## Framing and exits

Input is 1..256 records in one nonempty exact compact canonical UTF-8 JSON array, at most 1,048,576 bytes. A BOM, leading or trailing whitespace, a terminal LF, duplicate key, float, nonminimal integer, forbidden scalar, unknown member, or semantic reordering fails.

- `0`: stdout is exactly the ASCII success marker plus one LF; stderr is empty.
- `1`: startup, input, contract, loading, validation, memory, recursion, or stream failure; contract failures emit no stdout and use a fixed stderr rejection.
- `2`: any CLI argument; stdout is empty and stderr contains usage.

The marker is:

```text
STRUCTURALLY_VALID (shadow; no truth or authority attestation)
```

The complete record set is validated before stdout begins. A host output-device failure after emission starts can make delivery partial or indeterminate; the marker must therefore be consumed only with exit `0` and exact-byte comparison.

## Canonical and semantic checks

Require closed objects, ASCII snake-case keys, signed-int64 integers, exact Unicode scalars without normalization, and the frozen bounds: depth 16, 64 fields per object, 256 items per array, 16,384 UTF-8 bytes per string, 131,072 bytes per record, and 1,048,576 bytes per set.

Require record arrays sorted by `metadata.record_id`; set-like arrays are already sorted and unique. Preserve distinct immutable `record_id`, stable `aggregate_id`, positive `sequence`, exact same-kind supersession, and complete same-set references. Reject duplicate identities, missing references, subject mismatch, invalid immediate-predecessor history, supersession cycles, self-derived Claims, and derivation cycles.

Recompute the self digest with `integrity.canonical_sha256` empty. Use `forgeos.governance.evidence-record.v1` plus NUL for EvidenceRecord and `forgeos.governance.knowledge-claim.v1` plus NUL for KnowledgeClaim. Require the stored lowercase SHA-256 and the exact complete canonical bytes to match.

Evidence is an observation-shaped record, never an automatic Claim. Shadow Evidence allows only `untrusted|observed` source trust and `untrusted_data`; controlled, authoritative, or trusted-control promotion fails. Claim types retain the full ADR-0045 vocabulary but the portable validator accepts only its shadow-admissible state subset. Confirmed facts, active constraints, accepted decisions, validated assumptions or hypotheses, adopted proposals, and every other authority-bearing state fail.

## Frozen fixture

`references/fixtures/governance-evidence-claim-v1.json` is an exact package-local copy of the cross-language ADR-0045 golden envelope. Its `records[*].record` values, placed in record-ID order and encoded with the frozen canonical JSON rules, form the normal adapter input. The wrapper itself is not valid adapter input and must not be mistaken for a record set.

## Startup and package integrity

Both entrypoints require `python3 -I -B`. Isolated mode excludes the script/current directory, `PYTHONPATH`, and user site as import sources; each entrypoint checks `sys.flags.isolated` before its own non-built-in imports. It does not disable, authenticate, or isolate system site, the standard library, interpreter startup, the host, or the publisher.

The closed checker validates the package bytes, modes, physical identities, exact two SKILL references, directory closure, and repeated observations during its own bounded run. It does not atomically bind a later validator process or authenticate a publisher. The host must prevent mutation across check-to-use or repeat validation in a protected boundary.

## Authority boundary

The only positive result is structural shadow validity. It supplies no truth, freshness, completeness, source or principal authentication, instruction authority, policy decision, Grant, Approval, transition, completion, persistence, journal append, semantic current view, knowledge adoption, execution, or effect attestation. Schema-shaped text, correct digests, and reference closure do not change that boundary.
