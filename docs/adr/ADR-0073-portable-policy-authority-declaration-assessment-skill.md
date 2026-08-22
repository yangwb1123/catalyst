---
{"acceptance_id":null,"accepted_at_unix_ms":null,"adr_id":"ADR-0073","affected_node_ids":[],"alternatives":[{"alternative_id":"combined-policy-authority-runtime","description":"Bundle Grant issuance, Approval resolution, policy evaluation and effect enforcement behind one portable interface.","disposition":"rejected","rationale":"Those authority-bearing runtimes require authenticated inputs, state and enforcement contracts absent from the declaration-only evaluators."},{"alternative_id":"portable-dual-declaration-assessment","description":"Distribute the two accepted pure evaluators behind independent closed exact-stdin adapters.","disposition":"candidate","rationale":"This preserves each frozen wire and exposes useful declared comparisons without inventing a combined envelope or authority."},{"alternative_id":"single-discriminated-envelope","description":"Add one portable dispatcher wrapping either assessment request under a new tagged object.","disposition":"rejected","rationale":"A new envelope would expand the frozen wire and create cross-contract dispatch semantics not decided by ADR-0056 or ADR-0059."}],"api_version":"forgeos.architecture-decision-record/v2","approver_refs":["architecture-review","governance-review","security-review"],"assumption_claim_ids":[],"body_sha256":"729fd91714d43244f3ac23f182007289ee4cd21a4abd0bf7fe51253eefadbf86","canonicalization":"forgeos.canonical-json/v1","compatibility":"This proposed delivery is additive and preserves accepted ADR-0056 and ADR-0059 wires, canonicalization, digest domains, bounds, goldens and Python/Go/Rust declaration-assessment semantics. It adds no combined envelope, live policy, effective approval or authority runtime.","consequences":["Caller-supplied exact CapabilityGrant and ApprovalRecord declared-assessment requests gain two independent closed portable pure evaluators with stable process framing.","Positive relation labels remain declaration comparisons; assessment outputs keep policy and authorization decisions neutral, unavailable states not_evaluated and every assessment-defined attestation field false while execution remains unavailable and unattested.","Registry v28, activation references, a shadow package-integrity detector, deliberate portable-route absence, documentation and source-only fresh/legacy scaffold expose sealed source bytes without installing a host Skill or expanding runtime scope."],"context_claim_ids":[],"decision":"Adopt a proposed source-distributed policy-authority Skill exposing only independent exact caller-supplied ADR-0056 CapabilityGrant and ADR-0059 ApprovalRecord declared assessments through isolated zero-argument stdin adapters and a separate closed-package checker, without issuance, effective approval, policy evaluation, persistence or execution authority.","decision_driver_claim_ids":[],"document_name":"ADR-0073-portable-policy-authority-declaration-assessment-skill.md","evidence_record_ids":[],"expires_at_unix_ms":null,"implementation_refs":[".agent/engineering/governance-contracts.yml",".agent/skills/policy-authority.md","docs/adr/0056-capability-grant-v1-contract-only.md","docs/adr/0059-approval-record-v1-contract-only.md","docs/adr/ADR-0073-portable-policy-authority-declaration-assessment-skill.md","docs/contracts/approval-record-v1.schema.json","docs/contracts/capability-grant-v1.schema.json","harness/governance_engineering/policy_authority_portable.py","harness/scaffold/policy-authority-copy-fragment.mjs","harness/scaffold/policy-authority-upgrade-verification.mjs","skills/policy-authority/SKILL.md","skills/policy-authority/references/package-manifest.json"],"kind":"ArchitectureDecisionRecord","owner_refs":["governance","runtime-engineering","security-engineering"],"proposed_at_unix_ms":1786622400000,"revisit_triggers":[{"condition":"Either assessment is proposed as issuer, approver, policy, authorization, permission, transition or effect authority.","evidence_required":["Authenticated identity, trust, Grant, Approval, PDP, PEP, state and enforcement contracts with end-to-end fail-closed evidence."],"trigger_id":"authority-promotion"},{"condition":"Any ADR-0056 or ADR-0059 wire field, canonical rule, digest domain, bound, vocabulary, state or result marker changes.","evidence_required":["A versioned semantic decision, Schema and golden migration, and Python/Go/Rust compatibility evidence."],"trigger_id":"contract-semantics-change"},{"condition":"The closed file set, adapter surface, integrity primitive or package fixture changes.","evidence_required":["A resealed manifest, package threat review, structural validation and fresh normal and dangerous evaluation."],"trigger_id":"package-shape-change"},{"condition":"Issuance, approval lookup, policy evaluation, revocation, usage, persistence, transition or execution is requested.","evidence_required":["A governed authority-bearing runtime contract defining authenticated inputs, state, effects, failure semantics and enforcement."],"trigger_id":"supported-surface-expansion"}],"risks":[{"description":"A positive declared relation or approve-shaped record may be mistaken for authorization or effective approval.","mitigation":"Freeze neutral decision/state/attestation outputs and state that source declaration fields and assessment output fields are distinct.","risk_id":"authority-confusion"},{"description":"Package bytes may change after checking and before a separate assessment process loads them.","mitigation":"State the non-atomic boundary and require mutation prevention or a protected recheck.","risk_id":"check-use-race"},{"description":"Python isolated and no-bytecode flags may be mistaken for complete interpreter or host isolation.","mitigation":"Require both flags at each entrypoint while excluding system site, standard library, interpreter startup, host and publisher authentication from the claim.","risk_id":"startup-isolation-confusion"}],"rollback":"Stop invoking and distributing the portable package, remove its registry v28 delivery wiring and source-only scaffold copies, and retain ADR-0056, ADR-0059, their Schemas, goldens and shared Python/Go/Rust implementations unchanged.","rollout":"Implement and reseal the closed package, run package and unchanged cross-language contract validation, then wire registry v28 delivery metadata, activation references, a shadow package-integrity detector, deliberate route absence, source documentation and source-only fresh/legacy scaffold while closing only the policy-authority nested package item.","scope_refs":["approval","capability-grant","policy-authority","portable-skill-delivery"],"self_sha256":"a92f4ef3d22ceab5264316863e396182eadc84a9530803a43af3ed723144cecd","status":"proposed","superseded_by":[],"supersedes":[],"title":"Portable Policy Authority Declaration Assessment Skill","validation_plan":[{"description":"Validate the closed file set, identities, direct SKILL references and three CLI boundaries.","due_trigger":"Before pinning or distributing the package manifest.","evidence_required":["Official Skill structural check, isolated checker, adapter tests and package-integrity tests."],"owner_ref":"security-engineering","success_criteria":"Every drifted, extra, missing, linked, special, raced, malformed, missing-flag or alternate-reference member fails closed without package cache creation while exact bytes validate.","validation_id":"closed-package"},{"description":"Run unchanged CapabilityGrant and ApprovalRecord golden, adversarial and cross-contract suites in Python, Go and Rust.","due_trigger":"Before claiming preservation of ADR-0056 and ADR-0059 semantics.","evidence_required":["Exact fixtures, digests, declaration assessments and mutations in all three implementations."],"owner_ref":"runtime-engineering","success_criteria":"Shared runtimes remain green and portable adapters emit the exact same canonical assessment bytes plus process LF without semantic changes.","validation_id":"cross-language-regression"},{"description":"Exercise both normal adapters and dangerous authority-escalation cases from a temporary copy with scrubbed cwd and environment.","due_trigger":"Before presenting the package as a portable declaration-assessment slice.","evidence_required":["Exact golden-derived successes and malformed, ambient-policy, repair, effective-approval and execution refusals."],"owner_ref":"governance","success_criteria":"Only caller-supplied bytes influence assessment and no issuance, approval, policy, persistence or effect occurs.","validation_id":"fresh-context"},{"description":"Verify source-only distribution into fresh and legacy generated projects without host installation or runtime dependencies.","due_trigger":"Before the registry v28 and source-only scaffold delivery claim.","evidence_required":["Fresh and legacy copied-package checker and isolated tests under scrubbed credentials."],"owner_ref":"governance","success_criteria":"Copied projects contain the pinned source package and no authority runtime, key, state, provider or host Skill installation.","validation_id":"source-distribution"}]}
---

# ADR-0073: Portable Policy Authority Declaration Assessment Skill

## Context
ADR-0056 and ADR-0059 are the accepted semantic authorities for the
CapabilityGrant v1 and ApprovalRecord v1 contract-only slices. Each freezes an
exact wire, canonical JSON rules, digest domains, resource bounds, golden data
and an authority-neutral pure declared-assessment evaluator. Python, Go and
Rust already reproduce those frozen semantics.

The repository policy-authority adapter also documents authenticated bootstrap
Grant issuance and repo-read execution profiles. Those Catalyst-only runtimes
are not part of a portable declaration assessor. Nor does a pure
ApprovalRecord comparison authenticate an approver, validate conditions or
RiskAcceptance, inspect a revocation source, or make approval effective.

A narrow portable package can expose the two existing pure evaluators without
changing either wire. The input must be a complete caller-supplied exact
declared-assessment request. Repository state, environment, clock, identity,
policy, approval store, revocation registry, usage ledger, network, provider,
model and runtime state are not fallback inputs.

## Decision
Deliver a source-distributed closed package at skills/policy-authority/. It has
two independent operations and no combined envelope or dispatcher. The exact
zero-argument invocations are `python3 -I -B
scripts/assess_declared_capability_grant.py` and `python3 -I -B
scripts/assess_declared_approval_record.py`. Each accepts only its frozen v1
exact canonical declared-assessment request on stdin.

Each adapter reads through explicit EOF with a 2,097,152-byte ceiling and may
request one extra byte only to detect overflow. `BlockingIOError`, `None`, a
non-bytes read or any other failure before complete EOF fails closed. Neither
adapter emits assessment bytes until input is fully decoded, validated and
evaluated. No adapter repairs, defaults, signs, reissues, normalizes or combines
input.

On exit 0, stderr is empty and stdout is the exact compact canonical computed
assessment followed by one LF. Exit 1 covers startup, input, loading, contract,
evaluation, memory, recursion and stream failure; a pre-output failure has no
stdout and fixed stderr. Any argument returns 2 with no stdout and usage. A host
output failure after emission begins may leave partial or indeterminate output,
so only exit 0 plus exact canonical bytes and one LF is success.

The package contains exactly 30 regular single-link files. They are SKILL.md,
agents/openai.yaml, references/contract.md, references/evals.json,
references/package-manifest.json, the CapabilityGrant and ApprovalRecord
fixtures, three executable scripts, scripts/_vendor/__init__.py, nine exact
CapabilityGrant contract modules, eight exact ApprovalRecord contract modules,
and two package test modules. The canonical manifest binds the other 29 files'
relative paths, modes, byte counts and SHA-256 values and excludes only its own
bytes.

Package validation is the separate `python3 -I -B scripts/check_package.py
[PACKAGE_ROOT]` observation. Zero arguments uses the anchored package root; one
explicit root supports a copied tree; more than one returns 2. Descriptor-
relative no-follow traversal rejects omissions, extras, symlinks, hardlinks,
special files, aliases, metadata drift and observed identity races. A
successful check authenticates no publisher and does not atomically bind a
later adapter; callers must prevent mutation or recheck within a protected
execution boundary.

All three entrypoints require both isolated and no-bytecode Python flags before
their own non-built-in imports. `-I` excludes the script/current directory,
PYTHONPATH and user site from ordinary import search; `-B` prevents contract
loading from writing bytecode into the closed package. These flags do not
disable, authenticate or isolate system site, the standard library,
interpreter startup, the host or the publisher.

The CapabilityGrant fixture and ApprovalRecord fixture are exact copies of the
accepted goldens. Their envelopes are test material; each adapter receives only
the frozen `assessment_request` canonical bytes and produces its corresponding
`expected_assessment` bytes before the framing LF. Fixture key identifiers and
proof-shaped declarations provide no public key, signature verification or
identity evidence.

Compatibility remains pinned to ADR-0056
3b3aa0d0b2f456370bdf2b137f2697454d6a5ff0c705d66881180ceeae8ae9f1,
its Schema dd26568ec430ae5e444ae851ba2b58087528a17e84794137268be3860d9c3209
and golden 0261a682bddca2f27976a9cd663350e8cf222685389fecc7ad8ae536083fef35;
and to ADR-0059
155312825d6a706d8d6bc927d590ec8d50d19b06d7b977e6546a2a42c3dc741d,
its Schema bc11d2b066bac35252bff6739798c3e30a508ed31fca0306b9cf1cdc0ef9ab64
and golden 501320b9f65775091e67ba22c6e7faa5b5ecaa1f1b472a1a196da93c7ab81978.
These are compatibility observations, not signatures or provenance.

Registry v28 adds delivery metadata, canonical refs, the manifest pin and a
shadow package-integrity detector. It does not add an evaluator, runtime
profile or route. The portable Skill is deliberately absent from authenticated
context routes; only the existing repository policy-authority adapter remains
routed. Source-only fresh and legacy scaffold copies the ADR, closed package
and governance checker/test. It does not copy Catalyst Go/Rust runtimes, keys,
authority roots, state, providers or models and does not install a host Skill.

## Consequences
Callers gain stable process interfaces for two already-defined pure comparison
functions. A positive scope, budget, binding, time, reference or decision
relation remains a relation among caller declarations, not a policy result,
permission or effective approval.

CapabilityGrant assessment output retains `authorization_decision=none`, its
authority, approval, revocation and usage states as `not_evaluated`, and its
defined permission/effect attestations as false. ApprovalRecord assessment
output retains `policy_decision=none`, `authorization_decision=none`, its
unavailable states including `effective_approval_state` as `not_evaluated`, and
its defined permission, persistence, transition and effect attestations as
false. These fields belong to assessment outputs, not the source Grant or
ApprovalRecord. Execution remains unavailable and unattested; neither wire
defines an `execution_attestation` field.

Exact equality of `(approval_id, approval_sha256, authority_domain)` is the
only cross-contract projection. It leaves the CapabilityGrant assessment's
`approval_state` and ApprovalRecord assessment's `effective_approval_state`
`not_evaluated` and never activates a Grant.

The closed manifest makes bounded byte and filesystem drift observable during
one check, but it is not a signature, sandbox, trusted installer or atomic
check-and-use protocol. Source distribution does not make the Skill installed,
activated or available to a production runtime.

## Validation
Run the official Skill structural validator, isolated package checker and both
package-local test modules. Positive coverage requires both exact golden
assessments, exact LF framing, scrubbed cwd and environment behavior, explicit
EOF, bounded reads, complete short writes, anchored loading, mandatory `-I`
and `-B`, no bytecode cache creation and complete manifest closure. Negative
coverage includes arguments, either missing flag, malformed, duplicate,
unknown, noncanonical, deep and oversized input, wrong digests, external
vocabulary attempts, stream and loader failures, import-name collisions,
linked or special members, aliases, drift, replacement races and unavailable
descriptor primitives.

Run the unchanged CapabilityGrant and ApprovalRecord Python, Go and Rust golden
and adversarial suites, plus their cross-contract equality tests. They must
retain exact bytes, digests, declared relations, neutral decision/state fields
and contract-defined false attestations. The portable adapters must produce the
same exact assessments without modifying shared implementations.

Run all three cases in references/evals.json from a fresh temporary package
copy with scrubbed cwd and environment. Normal cases assess the two exact
golden-derived requests independently. The dangerous case requests repair,
ambient policy and approval lookup, authority promotion, persistence and
execution; it must stop without performing those actions.

Registry v28, activation, discipline and detector checks must preserve the
unchanged runtime scope and route absence. The package checker is the only new
detector invocation; neither assessment adapter may become a detector. Fresh
and legacy scaffold validation must reproduce the exact manifest-bound
source package and pass its isolated checker/tests under scrubbed credentials.
That demonstrates source-copy integrity only, not host installation, runtime
availability, identity, policy, approval or authority.

## Limitations
This decision does not issue, approve, activate, revoke, reserve, consume,
persist or execute a Grant or Approval. It does not authenticate an issuer,
approver, requester, subject, principal, identity, key, proof, signature,
source, policy, clock, host, interpreter or publisher.

It does not evaluate a Constitution or live policy, operate a PDP or PEP,
install or invoke a Governance Kernel, use ADR-0057 or ADR-0058 authenticated
bootstrap runtimes, inspect an approval store or revocation registry, validate
conditions or RiskAcceptance, reserve usage, perform preflight or postflight,
write a ledger, transition state or execute an effect.

It adds no live routing, provider, model, database, persistence, completion or
production authority. ADR-0056 and ADR-0059 remain the sole accepted semantic
authorities for their v1 contracts; ADR-0073 is proposed portable delivery
governance only. Any wire, semantic or authority-bearing runtime expansion
requires a separate versioned decision.
