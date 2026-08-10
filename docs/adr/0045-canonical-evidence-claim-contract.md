# ADR-0045: Canonical Evidence and Claim Contract Kernel

- Status: Accepted
- Date: 2026-08-09
- Owners: Governance / Architecture / Runtime Engineering
- Extends: ADR-0037 and ADR-0038

## Context

The engineering OS already names evidence, claims, context, authority and transitions, but it has no shared wire contract for distinguishing an observation from
an assertion. Existing backend, frontend and architecture shadow reports bind useful artifacts, yet their producers and provenance remain declarative. Promoting
those reports directly into a durable truth ledger would preserve ambiguous identity, conflicting status semantics and language-specific JSON digests as permanent
data. Adding a Hub migration now would also create an irreversible persistence surface before replay, authority and compatibility behavior exists.

The first Governance/Decision Kernel slice therefore needs a smaller boundary: strict records, deterministic bytes, cross-language digests and fail-closed
structural semantics. It must not imply that an Agent authenticated itself, that an observation is true, that an approver had authority or that a claim may satisfy a
completion gate.

## Decision

1. Ship exactly two v1 record kinds: `EvidenceRecord` and `KnowledgeClaim`. `ContextPackage`, `CapabilityGrant`, `ApprovalRecord`, `TransitionReceipt` and
   `KnowledgeUpdateProposal` remain planned and must not be represented through aliases.
2. Give every record three non-interchangeable identities: immutable `record_id`, stable logical `aggregate_id` and positive `sequence`. A newer record names exact
   prior records of the same aggregate through `supersedes_record_ids`; digest is content identity and is never reused as record identity.
3. Keep one kind-scoped `status.state`. Do not add a second epistemic status or infer authority from a state string. KnowledgeClaim validates against the full
   type-by-state matrix and then against the narrower shadow admissibility matrix. Confirmed facts, accepted decisions, active constraints, waivers and every other
   authoritative state are rejected in this slice.
4. Bind records to declared project, scope, source revision/tree, policy digest, context digest, creation principal and run. These values improve traceability but are
   not authenticated attestations. Evidence additionally binds a collector, parameters, observation time, snapshot, locator and content digest.
5. Use `forgeos.canonical-json/v1`: parse strict JSON with duplicate-key rejection; accept only exact compact UTF-8 canonical record bytes; sort ASCII snake-case
   object keys lexicographically by byte; preserve Unicode scalar values without normalization; escape only JSON-required characters; reject ASCII controls, bidi
   controls, U+2028 and U+2029; permit signed int64 integers only; reject floats; and enforce the bounded depth, item, field, string, record and record-set limits in
   `.agent/engineering/governance-contracts.yml`.
6. Compute a lowercase bare SHA-256 over the ASCII digest domain, a NUL byte and the canonical record bytes after replacing
   `integrity.canonical_sha256` with the empty string. Domains are kind-specific. The stored digest is then inserted and the complete record is canonicalized again.
   Canonical input and expected digest must both match; a semantically equivalent non-canonical input is invalid.
7. Evidence and claims are separate. Evidence records bounded observations and may be valid, invalid, unavailable or expired. Claims classify Fact, Constraint,
   Decision, Inference, Assumption, Hypothesis, Lesson, Proposal or Unknown and separately list supporting evidence, contradicting evidence and derived claims.
   Evidence never automatically confirms a claim.
8. Shadow evidence permits only `untrusted_data` and source trust `untrusted` or `observed`. `trusted_control`, `controlled` and `authoritative` are rejected because
   this repository has no authenticated producer or grant runtime. Human attestations are attested, not direct tool observations.
9. Assumptions and hypotheses require bounded validation plans and integer `confidence_micros`; unknowns require a queue reference. Confidence is absent for claim
   types that do not define it. Referenced records must exist in the same validation set and have the expected kind; Evidence references cover the Claim subject
   and remain disjoint between supporting and contradicting sets. Derived Claims may cross subjects, but self-reference and derivation cycles are invalid.
10. Implement the same codec and pure semantic validator in Python, Go and Rust against one golden fixture. The Python implementation is copied by scaffold and is
    the universal shadow checker. Go and Rust are Catalyst-only reference implementations unless a governed project explicitly adopts them.
11. The only positive result is `STRUCTURALLY_VALID (shadow; no truth or authority attestation)`. It cannot satisfy `forge accept`, authenticate provenance, mutate
    knowledge, write Hub state, authorize tools, transition lifecycle state or produce a production effect.

## Canonical encoding details

- Arrays retain declared order, while set-like arrays must already be sorted and unique.
- JSON object keys are schema-defined ASCII names; unknown keys are rejected before digest comparison.
- Boolean and null use lowercase JSON literals. Integers use minimal base-10 notation with no leading plus or leading zero.
- Strings are UTF-8 and use double quotes. Quote and reverse solidus are escaped; forbidden controls are rejected rather than normalized or re-escaped.
- Repository locators are relative, normalized forward-slash paths and cannot contain empty, dot or parent segments.
- `valid_until_unix_ms` is exclusive when present and must be greater than `valid_from_unix_ms`.
- Valid Evidence has an artifact digest and no reason; invalid/expired Evidence keeps its artifact and gives a reason; unavailable Evidence has no artifact and gives
  a reason. Expired Evidence records an end no later than creation. Observation precedes creation and validity cannot precede observation.
- Human attestations are `attested`; external sources are `derived`; only repository, test, gate, runtime and artifact observations collected by a tool/service may
  be `direct`. Command locators carry an exit code, repository locators carry an optional positive line range and other locator kinds carry neither.
- Shadow claims use null decision authority. Confidence is required only for Assumption, Hypothesis and Inference; a validation plan is required only for
  Assumption/Hypothesis; a queue reference is required only for Unknown. Fact, Constraint and Lesson need supporting Evidence; Inference needs Evidence or a
  derived Claim. Optional review deadlines cannot precede creation.
- Record sets are sorted by `record_id`; record IDs and `(kind, aggregate_id, sequence)` tuples are unique. Supersession is same-kind, same-aggregate, lower-sequence
  and acyclic. Sequence one supersedes nothing; a later sequence names at least its immediate predecessor. The Claim derivation graph is independently acyclic.

Any change to these byte rules, member meanings, digest domains or admissibility semantics requires a new version and compatibility decision. Editing golden output
without a version change is not a migration strategy.

## Consequences

- Governance records now have a portable, bounded representation and one cross-language digest rather than three language-default JSON interpretations.
- Candidate evidence and knowledge can be exchanged and reviewed without collapsing observation, inference, decision and authority into one vague document.
- Historical Memory files and ADRs are not silently upgraded. A later import proposal must preserve source identity, conflicts and replay behavior.
- No durable truth, materialized view, idempotent append protocol, authenticated identity, separation of duties, capability grant, transition receipt or revocation
  mechanism exists yet. Those are explicit next slices rather than hidden implications of this contract.
- Schema validity alone remains insufficient: semantic, canonical-byte, digest and reference validation are all required.

## Rejected alternatives

- Add the complete Governance Envelope and ledger in one migration: rejected because authority, replay, revocation and recovery contracts are not implemented.
- Reuse existing `Evidence`, `Claim`, Memory or ADR shapes: rejected because their identity, status and digest semantics are incompatible or underspecified.
- Use RFC-style floating JSON numbers or confidence values: rejected because cross-language precision and canonical spelling are unsafe for this ABI.
- Use language-default JSON encoding: rejected because key order, HTML escaping, Unicode escaping and integer behavior differ.
- Permit confirmed facts when direct evidence is declared: rejected because declared directness does not authenticate collector identity or review authority.
- Treat the digest as the record ID: rejected because stable logical history, external references and content identity have different lifecycles.
