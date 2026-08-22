# ADR-0067: Proposed-only ADR v2 machine frontmatter

- Status: Accepted
- Date: 2026-08-12
- Owners: Governance / Architecture / Runtime Engineering
- Extends: ADR-0037, ADR-0040, ADR-0045, ADR-0059, ADR-0065

## Context

ForgeOS describes an ADR v2 shape in the engineering OS design, but the repository has no
closed document wire for a newly created ADR. Existing ADRs are human-readable Markdown with
several metadata conventions. Treating them as if they already implement ADR v2 would require
ambiguous Markdown/YAML recovery and would silently rewrite historical meaning.

The first deliverable must therefore be narrower than the eventual ADR lifecycle. It needs a
deterministic proposed document that `writes_adr`, a universal checker and fresh scaffolds can
validate without depending on undeclared external authority or currentness. The universal
checker consumes only explicit document bytes; Catalyst `writes_adr` additionally scans and
hashes legacy ADR bytes solely to freeze its existing baseline-integrity fingerprint, without
treating them as v2 documents. The current system cannot authenticate an owner or approver,
resolve an EvidenceRecord or KnowledgeClaim, prove that a graph node exists, produce an
ApprovalRecord, accept a decision, or attest architecture compliance.

This file is the one accepted, legacy-format bootstrap decision that freezes the new format.
It is not an ADR v2 output, was not created through `writes_adr`, and is not retro-validated by
the v2 checker.

## Decision

### 1. Proposed-only profile and exact fields

Adopt `forgeos.architecture-decision-record/v2`, kind
`ArchitectureDecisionRecord`, canonicalization `forgeos.canonical-json/v1`, and status exactly
`proposed`. Every v2 document carries exactly these frontmatter members, in canonical key order:

```text
acceptance_id, accepted_at_unix_ms, adr_id, affected_node_ids, alternatives,
api_version, approver_refs, assumption_claim_ids, body_sha256, canonicalization,
compatibility, consequences, context_claim_ids, decision, decision_driver_claim_ids,
document_name, evidence_record_ids, expires_at_unix_ms, implementation_refs, kind,
owner_refs, proposed_at_unix_ms, revisit_triggers, risks, rollback, rollout,
scope_refs, self_sha256, status, superseded_by, supersedes, title, validation_plan
```

In this profile `accepted_at_unix_ms` and `acceptance_id` are null; `expires_at_unix_ms` is
either null or a non-negative signed-int64 value strictly later than `proposed_at_unix_ms`.
`superseded_by` is empty. `supersedes` may declare sorted unique prior ADR IDs, but the
declaration neither resolves nor mutates them. Unsupported API, kind, status,
canonicalization, aliases or future lifecycle fields fail closed rather than falling back.

`adr_id` is `ADR-0001` through `ADR-9999`. The physical basename is exactly
`ADR-NNNN-<lowercase-hyphen-slug>.md`; its number, `adr_id`, and the body H1 number must match.
The slug is not derived from or normalized against the title. `proposed_at_unix_ms` is a
caller-declared non-negative signed-int64 timestamp, not an authenticated clock reading.

### 2. References and structured declarations

Owner, required-approver, scope, Claim and Evidence references use the ADR-0045 lowercase
identifier grammar and a 160 UTF-8 byte ceiling. Set-like arrays arrive raw-UTF-8 sorted and
unique; a checker never sorts or deduplicates them. Owner and approver refs are respectively
author-declared responsibility and required-review identities. They do not authenticate a
principal, establish separation of duty, or constitute approval. Claim and Evidence refs are
not resolved or promoted to truth. Affected nodes must look exactly like
`graph-node-<64 lowercase hex>`, but are not resolved against a GraphSnapshot and do not prove
complete impact coverage.

At least one scope, owner and approver is required. Context, driver and assumption Claims,
Evidence, affected nodes, implementation refs and risks may be empty so an author is never
forced to invent an unresolved reference. Empty arrays mean only that this proposal supplied
no such declarations; they do not attest no context, evidence, impact or risk.
Implementation refs are normalized repository-relative forward-slash paths with no empty,
dot or parent component and may end in `#L<positive-int32>`. The control roots `.git` and
`.forge` are forbidden while `.agent` is permitted. Implementation refs are locators only and
are not read by validation.

Alternatives are sorted uniquely by `alternative_id` and contain exact
`alternative_id,description,disposition,rationale` fields. At least one `candidate` and one
`rejected` alternative are required. Risks are sorted uniquely by `risk_id` and contain exact
`description,mitigation,risk_id` fields. Validation steps are sorted uniquely by
`validation_id` and contain exact `description,due_trigger,evidence_required,owner_ref,
success_criteria,validation_id` fields; each owner ref must occur in `owner_refs`. Revisit
triggers are sorted uniquely by `trigger_id` and contain exact
`condition,evidence_required,trigger_id` fields.

Consequences and nested `evidence_required` arrays preserve authored order and are not sets.
Each evidence-required array contains one to 64 nonblank narrative strings. Decision,
compatibility, rollout and rollback are required nonblank narratives. Narrative
strings are at most 4,096 UTF-8 bytes; title and identifiers are at most 160 UTF-8 bytes, and
the physical `document_name` is at most 255 UTF-8 bytes.

### 3. Markdown framing and body binding

The complete document is UTF-8 without BOM or CR and has exactly this framing:

```text
---\n
<one exact compact canonical JSON line>\n
---\n
\n
<exact body bytes ending in LF>
```

JSON-in-frontmatter is intentional. YAML tags, aliases, merge keys, implicit types,
multiline scalars and parser-dependent duplicate handling are outside the wire. The JSON line
uses ADR-0045 canonical rules: strict duplicate and unknown rejection, raw UTF-8 key ordering,
signed-int64 integers, no floats or bool-as-int, no normalization, and rejection of controls,
DEL, bidi controls, surrogates, U+2028 and U+2029.

The body uses this exact deterministic layout: `# <adr_id>: <title>\n\n## Context\n`
followed by nonblank content, then each remaining required H2 separated from the preceding
section by exactly `\n\n`. The required H2 order is `Context`, `Decision`, `Consequences`,
`Validation`, `Limitations`. The body ends in exactly one LF, contains no Unicode control
characters other than LF, no body line ends in ASCII SPACE (U+0020), and each section body equals its own
Unicode-whitespace trim and is nonempty. Outside the five required delimiters, any line-shaped
level-two marker is forbidden: after zero through three ASCII spaces, either `##` followed by
end-of-line or ASCII SPACE, or a nonempty run of `-` below nonblank text. This is a frozen lexical
rule, not a CommonMark AST claim; it applies even inside quote/list/fence-looking narrative and
does not recognize container-prefixed markers. H3 subsections are allowed. Body sections are independent narrative, not
parser-normalized copies of similarly named metadata fields.

`body_sha256` is lowercase SHA-256 over:

```text
forgeos.architecture-decision-record-body.v2\0 || exact_body_bytes
```

`self_sha256` is lowercase SHA-256 over:

```text
forgeos.architecture-decision-record.v2\0 ||
canonical_frontmatter_with_only_self_sha256_empty_and_final_body_sha256_present ||
\0 || exact_body_bytes
```

The fixed framing, physical `document_name`, `adr_id`, exact H1, body digest and self digest
bind the whole artifact without a self-reference cycle. Semantically equivalent noncanonical
bytes are invalid.

### 4. Bounds and failure semantics

The complete document is limited to 262,144 bytes, the JSON line to 65,536 bytes, and the body
to 196,608 bytes. JSON depth and object width are at most 16 and 64; every array has at most 64
items. Schema length keywords are code-point approximations; runtime UTF-8 byte checks are
authoritative. Any framing, canonical, field, ordering, cross-reference, heading, digest,
Unicode or resource-bound failure produces no successful document result. Truncation,
normalization and best-effort acceptance are forbidden.

JSON Schema is only a closed structural description. A conforming semantic checker must also
validate filename and heading equality, UTF-8 byte bounds, raw-byte ordering, structured-set
identity, validation-owner membership, normalized implementation locators, body structure and
both exact digests.

### 5. Delivery boundary

The shipped slice consists of this bootstrap decision, the closed Schema, an independently
stored physical-name-valid golden Markdown document, a strict Python checker, a strict Go
validator wired into `writes_adr`, explicit prompt guidance, executable adversarial tests and
fresh/legacy scaffold installation. `writes_adr` validates only the single newly created
candidate after its existing baseline-integrity checks. The universal checker reads only the
explicit document bytes and supplied basename; it does not scan or resolve the repository.

The only positive meaning is:

```text
STRUCTURALLY_VALID_PROPOSED_ADR_V2 (declared metadata and exact document bytes only;
no identity, ownership, approver, evidence, claim, graph, acceptance, compliance,
persistence, transition, execution, or effect attestation)
```

## Consequences

- New ADR proposals have deterministic machine metadata and exact body/content identity.
- Authors must make ownership, required review, alternatives, validation and revisit intent
  explicit without those declarations being mistaken for authority or truth. Driver refs may
  be explicit; an empty array means they are undeclared, not that no drivers exist.
- ADRs 0001 through 0066 remain unchanged legacy documents. Go may read their bytes only for
  the existing baseline-integrity fingerprint; they are never v2-parsed, retro-validated,
  migrated, rejected or silently interpreted as v2. The universal checker does not scan them.
- The physical golden fixture uses its real v2 basename; validator behavior has no virtual
  filename exception.
- A valid proposal still says nothing about whether the decision was accepted or implemented.

## Validation

Validation must include exact golden parity, Schema validation, canonical/digest recomputation,
filename/number/title binding, every required body section, raw-order and duplicate rejection,
unknown field/API/status rejection, Unicode/framing/size/depth limits, locator traversal and
line overflow rejection, reference and structured-set constraints, and mutation of both body
and frontmatter bytes. Go `writes_adr` tests must prove that malformed proposals fail before
artifact commitment while legacy baseline ADRs remain untouched. Fresh and legacy scaffold
tests must prove the checker, Schema and physical golden are copied and runnable.

## Limitations and revisit triggers

This decision does not implement proposed-to-accepted/rejected/superseded transitions,
accepted-document immutability, authenticated ApprovalRecord resolution, owner or approver
identity, separation of duty, claim/evidence/graph resolution, ADR query views, persistence,
Architecture Fitness, ADR Compliance, or production authority. No local marker, actor hint,
caller flag, filename, digest match or successful checker result may stand in for them.

Revisit with a new version before adding a status, changing any field or digest preimage,
normalizing Markdown or Unicode, permitting alternate frontmatter syntax, resolving ambient
references, migrating legacy ADRs, or connecting the record to an authenticated acceptance
and immutable supersession state machine.

## Rejected alternatives

- YAML-native frontmatter was rejected because tags, aliases, coercion, merge and multiline
  behavior differ across parsers and make duplicate/canonical byte rules ambiguous.
- Hashing only frontmatter was rejected because it would leave the human decision body
  replaceable without changing record identity.
- Making the digest the ADR ID was rejected because sequence identity and content identity
  have different lifecycles.
- Retrofitting old ADRs was rejected because their missing fields cannot be reconstructed
  without inventing owners, approvals, evidence and historical timestamps.
- Shipping `accepted` in v2 now was rejected because no authenticated approval resolution or
  immutable transition state machine exists.
