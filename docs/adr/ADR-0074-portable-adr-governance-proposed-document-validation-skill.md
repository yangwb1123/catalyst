---
{"acceptance_id":null,"accepted_at_unix_ms":null,"adr_id":"ADR-0074","affected_node_ids":[],"alternatives":[{"alternative_id":"derive-basename-from-frontmatter","description":"Use a zero-argument stdin adapter and infer document_name from the supplied bytes.","disposition":"rejected","rationale":"Inference makes the frozen independent basename equality check tautological and weakens ADR-0067 semantics."},{"alternative_id":"new-portable-request-envelope","description":"Wrap basename and document bytes in a new JSON or base64 request object.","disposition":"rejected","rationale":"ADR-0067 freezes no request envelope, Schema or digest, so this would create an undecided domain wire."},{"alternative_id":"one-basename-argument-plus-stdin","description":"Preserve the existing evaluator pair as one lexical basename argument and exact document stdin bytes.","disposition":"candidate","rationale":"This keeps the inputs independent and portable without adding repository reads or domain semantics."}],"api_version":"forgeos.architecture-decision-record/v2","approver_refs":["architecture-review","governance-review","security-review"],"assumption_claim_ids":[],"body_sha256":"a18646f93391a1413d690853a35e5a2ca6a17eb498dcf970696e3606074fb875","canonicalization":"forgeos.canonical-json/v1","compatibility":"This proposed delivery is additive and preserves accepted ADR-0067 framing, metadata, basename equality, body, digest, bounds, Schema, golden and Python/Go validation semantics. It adds no request envelope, authoring, lifecycle or runtime authority.","consequences":["Caller-supplied exact Proposed ADR v2 bytes gain one closed portable validator while the independently supplied lexical basename retains its frozen equality role.","The lexical basename remains a caller assertion and neither structural success nor digest equality proves a physical file, repository identity, owner, approver, evidence, acceptance or compliance.","Registry v29, deliberate portable-route absence and source-only fresh/legacy scaffold distribute sealed source without changing runtime scope, installing a host Skill or copying the Go writes_adr runtime."],"context_claim_ids":[],"decision":"Adopt a proposed source-distributed adr-governance Skill exposing only ADR-0067 Proposed-document validation through exactly one caller-supplied lexical basename argument and an isolated explicit-EOF exact-stdin adapter, without a new domain wire, repository scan, authoring, lifecycle, persistence or authority.","decision_driver_claim_ids":[],"document_name":"ADR-0074-portable-adr-governance-proposed-document-validation-skill.md","evidence_record_ids":[],"expires_at_unix_ms":null,"implementation_refs":[".agent/engineering/governance-contracts.yml",".agent/skills/adr-governance.md","docs/adr/0067-proposed-only-adr-v2-frontmatter.md","docs/adr/ADR-0074-portable-adr-governance-proposed-document-validation-skill.md","docs/contracts/architecture-decision-record-v2.schema.json","docs/contracts/fixtures/ADR-9001-proposed-boundary.md","harness/governance_engineering/adr_governance_portable.py","harness/scaffold/adr-governance-copy-fragment.mjs","harness/scaffold/adr-governance-upgrade-verification.mjs","skills/adr-governance/SKILL.md","skills/adr-governance/references/package-manifest.json"],"kind":"ArchitectureDecisionRecord","owner_refs":["governance","runtime-engineering","security-engineering"],"proposed_at_unix_ms":1786622400000,"revisit_triggers":[{"condition":"The lexical basename, document framing, metadata, body, digest, reference, bound or positive marker semantics change.","evidence_required":["A separately versioned semantic decision, Schema and golden migration, and Python/Go compatibility evidence."],"trigger_id":"contract-semantics-change"},{"condition":"The closed package shape, adapter argument or stdin framing, integrity primitive or scaffold distribution changes.","evidence_required":["A resealed manifest, package threat review, structural validation and fresh normal and dangerous evaluation."],"trigger_id":"package-shape-change"},{"condition":"Authoring, repair, acceptance, immutable lifecycle, supersession, compliance, reference resolution, persistence or effects are requested.","evidence_required":["A governed authority-bearing contract defining authenticated inputs, state, transitions, failures, persistence and enforcement."],"trigger_id":"runtime-surface-expansion"},{"condition":"A successful marker or basename is proposed as identity, ownership, approval, truth, compliance, completion or effect authority.","evidence_required":["Authenticated identity, ApprovalRecord, evidence-resolution, lifecycle and enforcement contracts with end-to-end fail-closed evidence."],"trigger_id":"structural-authority-promotion"}],"risks":[{"description":"A supplied basename or matching document_name may be mistaken for proof of a physical repository file.","mitigation":"Name the argument lexical-only and freeze that no path read, repository observation or file identity attestation occurs.","risk_id":"basename-identity-confusion"},{"description":"Structural Proposed validation may be mistaken for acceptance, compliance or implemented architecture.","mitigation":"Emit the full authority-neutral marker and retain every Accepted lifecycle, compliance and evidence-resolution capability as unavailable.","risk_id":"compliance-authority-confusion"},{"description":"Package bytes may change between an integrity check and a separately started validator.","mitigation":"State the non-atomic boundary and require mutation prevention or a protected recheck.","risk_id":"package-check-use-race"},{"description":"Python isolated and no-bytecode flags may be mistaken for complete host or interpreter isolation.","mitigation":"Require both flags while excluding system site, standard library, interpreter startup, host and publisher authentication from the claim.","risk_id":"startup-isolation-confusion"}],"rollback":"Stop invoking and distributing the portable package, remove only its registry v29 delivery wiring and source-only scaffold copies, and retain ADR-0067, its Schema, golden, Python validator and Catalyst Go integrations unchanged.","rollout":"Implement and reseal the closed package, run package and unchanged Python/Go contract validation, then wire registry v29 delivery metadata, activation refs, a shadow package-integrity detector, deliberate route absence, source documentation and source-only fresh/legacy scaffold while closing only the adr-governance nested package item.","scope_refs":["adr-governance","architecture-decision-record-v2","portable-skill-delivery"],"self_sha256":"15c996fc2286a011a1b99f1d859b506cd6658b0f0e40afbaf97af767dcfb7d65","status":"proposed","superseded_by":[],"supersedes":[],"title":"Portable ADR Governance Proposed Document Validation Skill","validation_plan":[{"description":"Validate the exact closed file set, identities, direct Skill references and the one-basename-argument stdin boundary.","due_trigger":"Before pinning or distributing the package manifest.","evidence_required":["Official Skill structural check, isolated checker, adapter tests and package-integrity tests."],"owner_ref":"security-engineering","success_criteria":"Every drifted, extra, missing, linked, special, raced, malformed, missing-flag, wrong-argument or alternate-reference member fails closed while exact bytes validate.","validation_id":"closed-package"},{"description":"Run unchanged ADR-0067 golden and adversarial validation in Python and Go, including current writes_adr binding regressions.","due_trigger":"Before claiming preservation of ADR-0067 semantics.","evidence_required":["Exact Schema, golden, document bytes, basename relations and digest reconstruction in both implementations."],"owner_ref":"runtime-engineering","success_criteria":"Shared validators remain green and the portable adapter emits the exact authority-neutral marker without semantic changes.","validation_id":"cross-language-regression"},{"description":"Exercise normal and dangerous requests from a temporary package copy with scrubbed cwd and environment.","due_trigger":"Before presenting the package as portable Proposed validation.","evidence_required":["Exact golden success and basename inference, repair, authoring, acceptance, repository-scan, lifecycle and authority refusals."],"owner_ref":"governance","success_criteria":"Only the supplied label and bytes influence validation and no ambient read, mutation, lifecycle, persistence or authority action occurs.","validation_id":"fresh-context"},{"description":"Verify source-only delivery into fresh and legacy generated projects without host installation or runtime dependencies.","due_trigger":"Before the registry v29 and source-only scaffold delivery claim.","evidence_required":["Fresh and legacy copied-package checker, isolated tests and governance evidence."],"owner_ref":"governance","success_criteria":"Copied projects contain the pinned source package while excluding Go writes_adr runtime, host Skill installation, identity, approval, lifecycle and authority infrastructure.","validation_id":"source-distribution"}]}
---

# ADR-0074: Portable ADR Governance Proposed Document Validation Skill

## Context
ADR-0067 is the accepted semantic authority for the Proposed-only
ArchitectureDecisionRecord v2 contract. It freezes exact Markdown framing, a
single canonical JSON frontmatter line, the complete metadata field set,
resource bounds, reference shapes, body layout, basename relations and
domain-separated body and self digests. The existing Python and Go validators
already reproduce those semantics for their respective universal and
Catalyst-only call sites.

The frozen evaluator consumes two independent caller inputs: exact document
bytes and a supplied basename. There is no request JSON, request Schema or
request digest. Deriving the basename from frontmatter would make filename
validation tautological, while wrapping the pair in a new JSON or base64
envelope would create a new domain wire. A portable process boundary therefore
must preserve the pair directly.

The repository ADR Governance adapter also documents authoring and the current
Go `writes_adr` attempt boundary. Those behaviors include repository baseline
observation and artifact or receipt integration and are not part of a portable
pure validator. Proposed structure likewise provides no acceptance, lifecycle,
identity, evidence, compliance or effect authority.

## Decision
Deliver a source-distributed closed package at `skills/adr-governance/`. Its
only semantic operation validates one complete caller-supplied Proposed ADR v2
document against exactly one caller-supplied lexical basename. The exact
package-root invocation is `python3 -I -B
scripts/validate_declared_proposed_adr.py ADR-NNNN-slug.md`; stdin supplies the
raw document bytes and must reach explicit EOF.

The basename is a lexical label, not a path and not proof of a physical file,
repository location or identity. It remains independent from the bytes so the
validator can require exact equality with frontmatter `document_name`, the ADR
sequence and the H1 sequence. The adapter accepts no zero-argument mode, path,
repository root, URL, environment fallback or new request envelope.

Input is bounded at 262,144 bytes and the reader may request byte N+1 only to
detect overflow. `BlockingIOError`, `None`, a non-bytes read, a writer-open pipe
without EOF or any other pre-validation stream failure fails closed. The
adapter performs no output until the complete input and basename have passed
the unchanged ADR-0067 validator.

On exit 0, stderr is empty and stdout is exactly
`STRUCTURALLY_VALID_PROPOSED_ADR_V2 (declared metadata and exact document bytes
only; no identity, ownership, approver, evidence, claim, graph, acceptance,
compliance, persistence, transition, execution, or effect attestation)` plus
one LF. Input, loading, validation, resource or pre-output failure returns 1
with empty stdout and fixed stderr. Any argument count other than exactly one
returns 2 with empty stdout and usage. An output failure after emission begins
may leave partial or indeterminate stdout, so only exit 0 and exact complete
bytes count as success.

The package contains exactly 25 regular single-link files. They are `SKILL.md`,
`agents/openai.yaml`, contract and evaluation references, exact Schema and
golden copies, the canonical manifest, two executable scripts, one vendor
namespace initializer, six exact ADR v2 Python modules, seven exact governance
support modules and two package test modules. The manifest binds the other 24
members' relative paths, modes, byte counts and SHA-256 values and excludes only
its own bytes.

The separate checker invocation is `python3 -I -B scripts/check_package.py
[PACKAGE_ROOT]`. Zero arguments use the anchored package root; one explicit
root supports copied-tree validation; more than one returns 2. Descriptor-
relative no-follow traversal rejects missing, extra, linked, special, aliased,
mode-drifted, byte-drifted or identity-raced members. A successful check proves
only the observed package bytes and neither authenticates a publisher nor
atomically binds a later validation process.

Both public scripts require isolated and no-bytecode flags before their own
non-built-in imports. `-I` excludes the script and current directories,
`PYTHONPATH` and user site from ordinary import search, while `-B` prevents
package bytecode writes. They do not disable, authenticate or isolate system
site, the standard library, interpreter startup, the host, publisher or shell.

The bundled Schema and golden are exact copies with SHA-256 values
`ff3f00b1060b2d777b142947ef1ec9c0920782613d941aa672aecd242cf0341b`
and `b37dba8cc6d2750bb0ed73c7ee5b3ae61ad25551ec258584ed14618f1cb5c194`.
The external ADR-0067 semantic decision remains pinned at
`78c7d484cfb0e448c4c896440d4ea272a8e32a60f947539a3ad739baaeead71e`.
These are compatibility observations, not signatures, provenance or identity.

Registry v29 adds the independent
`adr_governance_portable_proposed_document_validation` delivery block,
canonical references, manifest pin and shadow package-integrity detector. It
does not change `architecture_decision_record_v2`, runtime scope or shipped
evaluators and adds no route or runtime. The portable Skill is deliberately
absent from authenticated context routes; the existing repository ADR
Governance adapter remains routed and documents the repository-root exact
portable commands.

Source-only fresh and legacy scaffold copies the sealed package, ADR-0074 and
portable governance checks while retaining the existing ADR-0067 Schema and
golden scaffold. It does not copy Catalyst Go `writes_adr` runtime, install a
host Skill, inspect or migrate project ADRs, generate a document, or add
identity, approval, lifecycle, persistence or effect infrastructure.

## Consequences
Callers gain a stable portable process interface for the already-defined
Proposed-only validator without a new domain envelope. The independent basename
continues to detect renamed or mismatched supplied documents, but it remains
only a caller assertion and carries no filesystem or repository attestation.

The fixed positive marker confirms structural validity of the supplied bytes
and label only. Declared owner, approver, Claim, Evidence, affected-node,
implementation, time and lifecycle fields remain unverified declarations.
Schema validity or digest equality alone remains insufficient.

The closed manifest makes bounded byte and filesystem drift observable during
one checker run. It is not a signature, sandbox, trusted installer or atomic
check-and-use protocol. Source distribution does not make the Skill installed,
routed, available to a host or authorized for production use.

## Validation
Run the official Skill structural validator, isolated package checker and both
package-local test modules. Positive coverage requires the exact golden through
the same one-basename-argument stdin adapter, exact marker and LF framing,
explicit EOF, mandatory `-I/-B`, anchored loading, scrubbed cwd and environment,
complete short writes and no bytecode cache creation.

Negative adapter coverage includes missing or extra arguments, path-like or
non-ASCII labels, sequence and basename mismatch, empty, malformed, duplicate,
unknown, noncanonical, deep and oversized input, BOM and CR, body or heading
drift, digest mutation, nonblocking writer-open pipes, unusual reads, loader
failure, import shadows, partial output and flush failure. Package integrity
coverage includes closed paths and directories, modes, sizes, hashes, direct
references, aliases, links, special files, descriptor primitive absence and
file, directory and ancestor replacement races.

Run the unchanged ADR-0067 Python adversarial suite and Go `internal/adrv2` plus
current `writes_adr` binding regressions. They must preserve exact Schema,
golden, framing, basename, body, digest, bounds and Proposed-only semantics. Go
parity is Catalyst-only evidence and is not copied into the portable package.

Run normal and dangerous fresh-context evaluation from a temporary package
copy with scrubbed environment. Requests to infer a basename, repair, author,
reseal, accept, supersede, persist, resolve references or scan a repository must
be refused. Registry, activation, discipline, route and detector checks must
preserve unchanged scope and make only the checker shadow/non-load-bearing.
Source-only fresh and legacy scaffold must reproduce exact pinned bytes and run
the copied checker, tests and governance suite without host installation.

## Limitations
This decision does not author, default, infer, normalize, sort, repair, rewrite,
reseal, sign, accept, reject, deprecate, supersede, migrate, persist or execute
an ADR. It does not scan a repository, validate legacy ADRs, bind `writes_adr`
artifacts or receipts, or attest that a caller basename names any physical file.

It does not authenticate authors, owners, approvers, identities, keys, hosts,
publishers or interpreters; resolve ApprovalRecords, Claims, Evidence or graph
nodes; prove truth, provenance, coverage, time, currentness, implementation,
immutability or architecture compliance; or create a Graph edge, lifecycle
transition, completion, permission, routing, execution or effect.

ADR-0067 remains the sole accepted semantic authority for Proposed ADR v2.
ADR-0074 is proposed portable packaging and source-only delivery governance
only. Accepted lifecycle, immutable documents, supersession transitions,
compliance, DECISIONS query merging, legacy import and every authority-bearing
runtime require separately versioned decisions and evidence.
