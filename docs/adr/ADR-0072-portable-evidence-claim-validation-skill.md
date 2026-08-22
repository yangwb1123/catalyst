---
{"acceptance_id":null,"accepted_at_unix_ms":null,"adr_id":"ADR-0072","affected_node_ids":[],"alternatives":[{"alternative_id":"ambient-authoring-validator","description":"Discover observations and missing fields from repository, environment, network, clock or model context.","disposition":"rejected","rationale":"Discovery and authorship add ungoverned inputs, provenance claims and effects beyond ADR-0045 pure validation."},{"alternative_id":"portable-validation-only","description":"Distribute the accepted pure Python validator behind a closed zero-argument stdin adapter.","disposition":"candidate","rationale":"This reuses frozen cross-language semantics while keeping observation, authorship, persistence, truth and authority absent."},{"alternative_id":"portable-whole-governance-stack","description":"Bundle adapters, journal, semantic views, proposals and authority services with validation.","disposition":"rejected","rationale":"Those contracts have distinct inputs, effects and governance and cannot be implied by structural validation."}],"api_version":"forgeos.architecture-decision-record/v2","approver_refs":["architecture-review","governance-review","security-review"],"assumption_claim_ids":[],"body_sha256":"9aa8871ca9024c163ac83677a7c6f289c0579e1b4a92c8535e950b1d34b4c895","canonicalization":"forgeos.canonical-json/v1","compatibility":"This proposed delivery is additive and preserves accepted ADR-0045 wire, canonicalization, digests, bounds, golden and Python/Go/Rust semantics. It does not alter or claim repository-specific authoring, adapter, journal, semantic-view, proposal or authority surfaces.","consequences":["Caller-supplied exact EvidenceRecord and KnowledgeClaim sets gain one closed portable structural validator with stable process framing.","Invalid or incomplete input remains invalid; the package never manufactures observations, records, defaults, ordering, digests, truth or authority.","Registry v27, activation references, a shadow package-integrity detector, deliberate portable-route absence and source-only scaffold delivery expose the sealed bytes without installing a host Skill, adding persistence or claiming production availability."],"context_claim_ids":[],"decision":"Adopt a proposed source-distributed evidence-claim-management Skill exposing only exact caller-supplied ADR-0045 record-set structural validation through an isolated zero-argument stdin adapter and a separate closed-package checker, without authoring, ambient acquisition, persistence, truth or authority.","decision_driver_claim_ids":[],"document_name":"ADR-0072-portable-evidence-claim-validation-skill.md","evidence_record_ids":[],"expires_at_unix_ms":null,"implementation_refs":[".agent/engineering/governance-contracts.yml",".agent/skills/evidence-claim-management.md","docs/adr/0045-canonical-evidence-claim-contract.md","docs/adr/ADR-0072-portable-evidence-claim-validation-skill.md","docs/contracts/fixtures/governance-evidence-claim-v1.json","docs/contracts/governance-evidence-claim-v1.schema.json","harness/governance_contract","harness/governance_engineering/evidence_claim_portable.py","harness/scaffold/evidence-claim-upgrade-verification.mjs","skills/evidence-claim-management/SKILL.md","skills/evidence-claim-management/references/package-manifest.json"],"kind":"ArchitectureDecisionRecord","owner_refs":["governance","runtime-engineering","security-engineering"],"proposed_at_unix_ms":1786622400000,"revisit_triggers":[{"condition":"The portable result is proposed as truth, provenance, completion, permission, transition or effect authority.","evidence_required":["Authenticated identity, Grant, PDP, Approval and downstream enforcement contracts with end-to-end fail-closed evidence."],"trigger_id":"authority-promotion"},{"condition":"Any v1 wire field, canonical rule, bound, digest domain, state matrix, history rule, reference rule or result marker changes.","evidence_required":["A versioned semantic decision, Schema and golden migration, and Python/Go/Rust compatibility evidence."],"trigger_id":"contract-semantics-change"},{"condition":"The closed file set, supported adapter surface, integrity primitive or package fixture changes.","evidence_required":["A resealed manifest, package threat review, structural validation and fresh normal and dangerous evaluation."],"trigger_id":"package-shape-change"},{"condition":"Authoring, observation, repair, defaults, persistence, adapters, journal, semantic view, proposals or authority are requested.","evidence_required":["A governed contract defining exact inputs, outputs, effects, failure semantics, identity and authority boundaries."],"trigger_id":"supported-surface-expansion"}],"risks":[{"description":"A structural marker may be mistaken for proof of truth, provenance, completion or authority.","mitigation":"Freeze the authority-free marker and list every absent attestation at handoff.","risk_id":"authority-confusion"},{"description":"Package bytes may change after checking and before a separate validator loads them.","mitigation":"State the non-atomic boundary and require mutation prevention or a protected recheck.","risk_id":"check-use-race"},{"description":"Callers may expect incomplete records to be repaired, sorted, defaulted or sealed.","mitigation":"Accept only exact supplied canonical bytes and reject every missing field, digest or ordering requirement.","risk_id":"record-authoring-confusion"},{"description":"Python isolated mode may be mistaken for complete interpreter or host isolation.","mitigation":"Limit the claim to entrypoint search behavior and exclude host, publisher, interpreter startup, standard library and system site authentication.","risk_id":"startup-isolation-confusion"}],"rollback":"Stop invoking and distributing the portable package, remove its delivery wiring and source-only scaffold copies, and retain ADR-0045, its Schema, fixture and shared Python/Go/Rust implementations unchanged.","rollout":"Implement and reseal the closed package, run structural, package, cross-language and fresh-context validation, then wire registry v27 delivery metadata, activation references, a shadow package-integrity detector, deliberate route absence, documentation and source-only fresh/legacy scaffold while closing only the evidence-claim-management nested package item.","scope_refs":["evidence-claim-management","governance-evidence-claim-v1","portable-skill-delivery"],"self_sha256":"4aa14c22cb0c49a701764b611af045baaeabdb4af6a3144a75423fecd076e741","status":"proposed","superseded_by":[],"supersedes":[],"title":"Portable Evidence Claim Validation Skill","validation_plan":[{"description":"Validate the closed file set, identities, two direct SKILL references and CLI boundaries.","due_trigger":"Before pinning or distributing the package manifest.","evidence_required":["Official Skill structural check, isolated checker, adapter tests and package-integrity tests."],"owner_ref":"security-engineering","success_criteria":"Every drifted, extra, missing, linked, special, raced, malformed or alternate-reference member fails closed while exact bytes validate.","validation_id":"closed-package"},{"description":"Run unchanged governance-contract golden and adversarial suites in Python, Go and Rust.","due_trigger":"Before claiming preservation of ADR-0045 semantics.","evidence_required":["Exact fixture and digest reconstruction plus mutation rejection in all three implementations."],"owner_ref":"runtime-engineering","success_criteria":"Shared runtimes remain green and the package validates the same canonical set without semantic changes.","validation_id":"cross-language-regression"},{"description":"Exercise normal and dangerous evaluations from a temporary copy with scrubbed cwd and environment.","due_trigger":"Before presenting the package as a portable validation slice.","evidence_required":["Exact golden-derived success and malformed, authority-escalating, ambient-authoring and persistence-request refusals."],"owner_ref":"governance","success_criteria":"Only supplied bytes influence validation; invalid input emits no marker and causes no authoring, ambient read, authority or persistence action.","validation_id":"fresh-context"},{"description":"Verify source-only distribution into fresh and legacy generated projects without host installation or runtime dependencies.","due_trigger":"Before the registry v27 and source-only scaffold delivery claim.","evidence_required":["Fresh and legacy package checker and isolated test evidence under scrubbed credentials."],"owner_ref":"governance","success_criteria":"Copied projects contain the pinned package and no Go/Rust runtime, provider, model, persistence or authority dependency.","validation_id":"source-distribution"}]}
---

# ADR-0072: Portable Evidence Claim Validation Skill

## Context
ADR-0045 is the accepted and sole semantic authority for EvidenceRecord and
KnowledgeClaim v1. It freezes their exact shapes, canonical JSON, digest
domains, bounds, history and reference rules, shadow-admissible states, and
positive result. The repository already has matching pure Python, Go and Rust
implementations and one cross-language golden fixture.

The repository-specific evidence-claim-management adapter covers wider
authoring, capture, adapter, journal, semantic-view and proposal workflows.
Those capabilities are not a portable validation interface. A narrow package
can expose the accepted validator without claiming it can observe a source,
create or repair a record, seal missing data, persist knowledge, or make a
truth or authority decision.

The portable input must be complete before invocation. Repository state,
environment, network, clock, provider, model, policy service, identity service
and prior knowledge are not fallback inputs. Recomputing a digest only compares
caller-supplied bytes with the accepted contract; it is structural validation,
not authorship, signature, attestation or provenance.

## Decision
Deliver a source-distributed closed package at
skills/evidence-claim-management/. Its sole supported operation validates one
caller-supplied exact canonical EvidenceRecord/KnowledgeClaim record-set array.
It never creates records from prose or observations, adds defaults, sorts or
normalizes values, inserts or replaces a digest, returns repaired records,
reads an ambient source, invokes another adapter, writes a journal or semantic
view, submits a KnowledgeUpdateProposal, or grants authority.

The package contains exactly 18 regular single-link files: SKILL.md,
agents/openai.yaml, references/contract.md, references/evals.json,
references/fixtures/governance-evidence-claim-v1.json,
references/package-manifest.json, scripts/validate.py,
scripts/check_package.py, scripts/_vendor/__init__.py, seven modules under
scripts/_vendor/governance_contract/ named __init__.py, codec.py, constants.py,
fixture.py, record_set.py, semantics.py and shape.py, plus
tests/test_portable_scripts.py and tests/test_package_integrity.py. The manifest
binds the other 17 files and excludes only its own bytes. No unlisted member
belongs to the package.

The supported adapter invocation is exactly
python3 -I -B scripts/validate.py with zero arguments and the canonical record
set on stdin. It reads through explicit EOF, accepts at most 1,048,576 input
bytes, may observe one additional overflow-detection byte, and loads the bundled
implementation from an adapter-anchored explicit package location without
adding the scripts directory or PYTHONPATH to module search. It validates one
nonempty array of at most 256 records and emits no canonical or repaired record
bytes.

Exit 0 writes exactly
STRUCTURALLY_VALID (shadow; no truth or authority attestation) followed by one
LF and leaves stderr empty. A validation, loading, resource, memory, recursion,
stdin or pre-output failure returns 1 with no stdout and a fixed rejection on
stderr. Any argument returns 2 with no stdout and usage on stderr. A host output
device can fail after partial delivery, so only exit 0 plus exact marker bytes
is success.

Both entrypoints require isolated Python and check sys.flags.isolated before
their own non-built-in imports. Isolated mode excludes the script/current
directory, PYTHONPATH and user site as import sources. It does not disable,
authenticate or isolate system site, the standard library, interpreter startup,
the host or the publisher.

Package validation is a separate
python3 -I -B scripts/check_package.py [PACKAGE_ROOT] observation. With no
argument it uses its anchored root; one explicit root supports a copied tree;
more than one argument returns 2. Its canonical closed manifest binds every
other member's path, mode, byte count and SHA-256. Descriptor-relative
no-follow traversal rejects missing or extra members and directories, symlinks,
hardlinks, special files, aliases, mode, size or digest drift, broken or
alternate direct references, noncanonical manifests and observed identity
races. It fails closed when required host primitives are unavailable.

A successful package check describes only identities observed during that
bounded run. It neither authenticates a publisher nor atomically binds a later
validator process. The host must prevent check-to-use mutation or recheck
within its protected execution boundary.

The package fixture is an exact copy of
docs/contracts/fixtures/governance-evidence-claim-v1.json. Its envelope is test
material, not adapter input; only records[*].record values encoded by the frozen
canonical rules form normal input. Compatibility remains physically pinned to
ADR-0045 a04479075dc60828176cd7e68857dcc4f3fc92bb4ae4b567f2caddd93f478b81,
the Schema b2f8824c95012d94e71b4643756890a7a23f67dc1b9e0e8ecacf979b016864e8,
and the golden db111600f93e63b3533b1f06b14d7520eb4cbec0e4c6d0e3a6e0fd7e2740824a.
These hashes are compatibility observations, not signatures or provenance.

This proposed delivery adds registry v27 metadata, activation references, a
shadow package-integrity detector, documentation and source-only scaffold
wiring around the sealed package. The portable prose is deliberately absent
from authenticated context routes: the existing repository adapter remains the
only routed Evidence/Claim Skill. Distribution copies source bytes only; it
does not install a host Skill, add a record validator detector without exact
stdin, or promote any runtime or authority scope.

## Consequences
An exact already-authored record set gains one closed portable structural
validator with stable stdin/stdout framing. The caller retains its original
bytes; the adapter does not silently produce a different record set.

Callers still own acquisition, observation, authorship, provenance, identity,
completeness and authorization. A missing field or digest is rejected rather
than discovered or repaired. Structurally valid Evidence remains
observation-shaped data, and a structurally valid Claim remains an
authority-free shadow claim.

The closed package makes byte drift and common filesystem substitution
observable during one check interval, but is not a sandbox, signature, trusted
installer or atomic check-and-use protocol. Source distribution alone does not
make the Skill installed, activated or available to a production runtime.

## Validation
Run the official Skill structural validator, isolated package checker and both
package-local test modules. Positive coverage requires exact golden record-set
validation, fixed marker and LF framing, scrubbed cwd and environment operation,
bounded reads, complete short writes, anchored loading and manifest closure.
Negative coverage includes arguments, non-isolated startup, malformed,
duplicate, unknown, noncanonical, deep and oversized input, wrong or missing
digests, broken references, authority and trust escalation, loader and stream
failures, import-name collisions, linked and special members, aliases, drift,
replacement races, alternate Markdown references and unavailable primitives.

Run the unchanged governance-contract Python, Go and Rust golden and
adversarial suites. They must retain the accepted record bytes,
kind-separated digests, state matrix, reference closure and failure behavior;
the package adapter must validate the same exact record set without changing
the shared implementations.

Run both cases from references/evals.json in a fresh temporary copy with a
scrubbed cwd and environment. The normal case validates only exact
golden-derived bytes. The dangerous case requests repair, authoring, ambient
inspection, authority promotion and persistence while supplying invalid
mutations; it must stop without emitting the marker or performing those
actions.

Registry v27, activation and the package-integrity detector must preserve the
validation-only scope and portable route absence. Fresh and legacy generated
projects must contain the exact manifest-bound source package, pass both
package-local suites and the root golden checker, and contain no Catalyst
Go/Rust runtime or host Skill installation. Those checks establish source-copy
integrity only; they do not establish production availability or authority.

## Limitations
This decision does not implement an Evidence or Claim author, source collector,
live command or repository observer, artifact or command adapter, journal,
semantic current view, conflict resolver, validation scheduler, CognitiveAtom,
KnowledgeUpdateProposal, Hub, database or migration. It provides no defaults,
ordering repair, digest insertion, signing or record sealing.

It does not authenticate the caller, collector, principal, source, repository,
policy, interpreter, host or publisher. It does not prove that an observation
happened, that content or a Claim is true, fresh, complete, uncontested or
adopted, or that supporting evidence is sufficient.

It issues no Grant or Approval, evaluates no PDP, authorizes no instruction or
tool, performs no transition or effect, writes no durable state and cannot
satisfy a completion gate. ADR-0045 remains the sole accepted
EvidenceRecord/KnowledgeClaim v1 semantic authority; ADR-0072 is proposed
delivery governance only, and any wire or semantic change requires a separately
versioned contract decision.
