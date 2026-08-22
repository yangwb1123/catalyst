---
{"acceptance_id":null,"accepted_at_unix_ms":null,"adr_id":"ADR-0071","affected_node_ids":[],"alternatives":[{"alternative_id":"ambient-context-builder","description":"Let the portable Skill discover repository, policy, route, clock, tokenizer or model context from the host.","disposition":"rejected","rationale":"Ambient discovery would make the request incomplete, weaken cache identity and cross the frozen ADR-0055 pure-input boundary."},{"alternative_id":"portable-authority-free-adapter","description":"Distribute the existing strict Python ContextPackage v1 implementation in a closed Skill with a zero-argument stdin adapter.","disposition":"candidate","rationale":"This makes the accepted deterministic projection reusable without changing its wire, adding a provider or manufacturing authority."},{"alternative_id":"provider-integrated-runtime","description":"Have the Skill retrieve sources, select a production tokenizer, compile a prompt and invoke a model or policy decision point.","disposition":"rejected","rationale":"Provider, retrieval, tokenizer, prompt, identity and authority contracts are outside ADR-0055 and are not available in this slice."}],"api_version":"forgeos.architecture-decision-record/v2","approver_refs":["architecture-review","governance-review","security-review"],"assumption_claim_ids":[],"body_sha256":"92f2a415e51fac94f3ce61203b7eb3152efb4e18a0233f91e2fc00558cf4b84d","canonicalization":"forgeos.canonical-json/v1","compatibility":"This proposed delivery is additive and leaves the accepted ContextPackage v1 Schema, golden, digest domains, bounds, Python/Go/Rust semantics and existing host adapters unchanged. It does not imply that any other planning-only Skill package is implemented.","consequences":["An exact caller-supplied ContextPackage v1 request can be assembled and fully revalidated through a source-distributed zero-argument stdin adapter.","The portable package remains limited to the frozen fixture UTF-8 byte counter and therefore cannot stand in for a production model tokenizer or provider integration.","Fresh and upgraded projects receive a closed source package, not a host-installed Skill, runtime grant, model context, cache, journal or authority service."],"context_claim_ids":[],"decision":"Adopt a proposed closed portable context-engineering Skill that vendors the existing ContextPackage v1 Python reference implementation and exposes only deterministic stdin assembly and strict package-integrity validation, without changing ADR-0055 semantics or adding source acquisition, provider, model, PDP, authority or persistence behavior.","decision_driver_claim_ids":[],"document_name":"ADR-0071-portable-context-engineering-skill.md","evidence_record_ids":[],"expires_at_unix_ms":null,"implementation_refs":[".agent/skills/context-engineering.md","docs/adr/0055-shadow-context-package-v1.md","docs/adr/ADR-0071-portable-context-engineering-skill.md","docs/contracts/context-package-v1.schema.json","docs/contracts/fixtures/context-package-v1.json","harness/context_package_contract","skills/context-engineering/SKILL.md","skills/context-engineering/references/package-manifest.json"],"kind":"ArchitectureDecisionRecord","owner_refs":["context-engineering","governance","runtime-engineering","security-engineering"],"proposed_at_unix_ms":1786654800000,"revisit_triggers":[{"condition":"Any ContextPackage v1 wire field, bound, eligibility precedence, category order, lane rule, redaction or truncation rule, digest domain or fixed result changes.","evidence_required":["A separately versioned contract decision, exact golden and cross-language compatibility evidence."],"trigger_id":"contract-semantics-change"},{"condition":"A tokenizer other than the frozen fixture counter, source retriever, prompt compiler, provider or model invocation is proposed.","evidence_required":["A separately governed adapter contract binding executable identity, inputs, failures, limits and non-authority semantics."],"trigger_id":"external-tokenizer-or-provider"},{"condition":"The closed package file set, internal adapter surface, integrity primitive or scaffold distribution boundary changes.","evidence_required":["A resealed manifest, package threat review, structural validation and fresh plus legacy scaffold evidence."],"trigger_id":"package-shape-change"},{"condition":"A ContextPackage or its lane labels are proposed as truth, executable instruction, permission, approval, completion, current memory or effect authority.","evidence_required":["Authenticated identity, Grant, PDP, Approval and downstream enforcement contracts with end-to-end fail-closed evidence."],"trigger_id":"runtime-authority-promotion"}],"risks":[{"description":"The bundled UTF-8 byte counter may be mistaken for a usable production-model tokenizer.","mitigation":"Name it fixture-only, reject every other identity and forbid estimates or fallback counters.","risk_id":"counter-fixture-confusion"},{"description":"Consumers may execute text from instruction_candidates or treat trusted_context as authenticated.","mitigation":"Keep instruction_allowed false for every snippet and require handoff language that both lane names are non-authoritative data classifications.","risk_id":"instruction-lane-confusion"},{"description":"A copied package may be modified, linked or raced before execution.","mitigation":"Use a closed physical manifest with descriptor-relative no-follow, single-link, regular-file, mode, size, digest and repeated-identity checks that fail closed when primitives are unavailable.","risk_id":"package-integrity-race"},{"description":"A successful package may be mistaken for proof that all relevant sources were discovered, redacted or current.","mitigation":"Accept only explicit caller-supplied candidates and ranges, preserve omissions and declared freshness, and state that acquisition and completeness are not performed.","risk_id":"source-completeness-confusion"}],"rollback":"Stop invoking and scaffolding the portable package, remove only its delivery wiring, and retain ADR-0055 Schema, golden and existing Python/Go/Rust implementations unchanged; prior package bytes remain inert authority-free artifacts.","rollout":"Create the Skill with the official initializer, freeze its closed manifest after implementation stabilizes, run structural, package, unit, cross-language, normal and dangerous evaluations, then wire registry v26, activation, detector, routes, documentation and fresh/legacy scaffold and close only the context-engineering nested roadmap item.","scope_refs":["context-engineering","context-package-v1","portable-skill-delivery"],"self_sha256":"ed72467dddb730de425278d49c8c6bdb9e6f8a82904c8fa5a8eda6ce339fd101","status":"proposed","superseded_by":[],"supersedes":[],"title":"Portable Context Engineering Skill","validation_plan":[{"description":"Validate the exact closed file set, physical identities, direct references, adapter boundary and package-local tests.","due_trigger":"Before pinning the portable package manifest or enabling scaffold propagation.","evidence_required":["Official Skill structural validation, closed package checker, mode and link adversarial tests, argument and output framing tests."],"owner_ref":"security-engineering","success_criteria":"Every missing, extra, linked, special, raced, malformed or drifted package member and every partial output fails closed without emitting a ContextPackage.","validation_id":"closed-package"},{"description":"Run the unchanged ContextPackage v1 golden and adversarial suites in Python, Go and Rust and compare package-local output to the frozen golden.","due_trigger":"Before claiming the portable adapter preserves ADR-0055.","evidence_required":["Schema and golden physical pins, byte-identical assembly, digest reconstruction and mutation rejection in all existing runtimes."],"owner_ref":"runtime-engineering","success_criteria":"The package-local adapter emits the same unique canonical package for the frozen request and no ContextPackage v1 semantic or wire byte changes.","validation_id":"cross-language-regression"},{"description":"Exercise normal and dangerous fresh-context cases without relying on repository-specific hidden context.","due_trigger":"Before closing the context-engineering nested roadmap item.","evidence_required":["Fresh normal assembly and adversarial injection, missing-required, invalid-redaction, wrong-counter, ambient-source and authority-escalation evaluations."],"owner_ref":"context-engineering","success_criteria":"The Skill uses only supplied request bytes, emits the fixed authority-free result or a failure, and never retrieves, invokes, persists or upgrades authority.","validation_id":"fresh-context"},{"description":"Verify source-only distribution into new and legacy projects.","due_trigger":"Before the registry v26 delivery claim.","evidence_required":["Fresh forge-init and legacy forge-upgrade package checks plus complete acceptance under scrubbed provider credentials."],"owner_ref":"governance","success_criteria":"Both scaffolds contain a valid closed package and adapter while excluding Catalyst Go/Rust runtimes, host installation, provider credentials and authority state.","validation_id":"scaffold-delivery"}]}
---

# ADR-0071: Portable Context Engineering Skill

## Context
ADR-0055 already accepts the complete ContextPackage v1 semantic contract. It freezes the
exact build request and package shapes, selection and omission order, three structured lanes,
redaction and optional truncation behavior, resource ceilings, six digest domains, cache
revalidation and the authority-free positive result. Independent Python, Go and Rust
implementations reconstruct the same exact golden.

The existing `.agent/skills/context-engineering.md` is a ForgeOS repository adapter, not a
portable Skill package. The universal scaffold copies the Python contract checker and its
sources, but it does not provide a closed `skills/context-engineering/` package with
`SKILL.md`, `agents/openai.yaml`, deterministic package-local automation, physical integrity
validation or fresh-context evaluations. The 38-package roadmap therefore still has no
delivered `context-engineering` nested item.

Packaging must not reopen ADR-0055. ContextPackage v1 consumes caller-supplied exact bytes; it
does not discover repository files, obtain current policy or routes, inspect a wall clock,
retrieve context, authenticate a source, decide authority, compile a provider prompt, invoke a
model or persist a cache or memory. Its `instruction_candidates` and `trusted_context` names
are structural classifications, and every selected snippet still fixes
`instruction_allowed=false`.

The only bundled token counter is the cross-language fixture counter
`forgeos.token-counter.utf8-bytes/v1`, whose identity digest is
`44799f99769528ecb46bcad483faf2d8ff4ab086bf32b2fe692a18f0eebea3cf`.
It counts exact canonical projection UTF-8 bytes. It is not a tokenizer estimate for any
provider or model, and the portable slice must reject rather than guess another identity.

## Decision
Deliver a source-distributed closed package at `skills/context-engineering/`. Its frontmatter
contains only `name` and `description`; the description triggers only when an exact caller-
supplied ContextPackage v1 request already exists and explicitly excludes source discovery,
retrieval, provider/model invocation, prompt compilation, authentication, authorization and
persistence. Detailed frozen semantics live one reference hop from `SKILL.md`.

The package contains exactly 16 regular single-link files: `SKILL.md`,
`agents/openai.yaml`, `references/contract.md`, `references/evals.json`,
`references/fixtures/context-package-v1.json`, `references/package-manifest.json`,
`scripts/assemble.py`, `scripts/check_package.py`, `scripts/_vendor/__init__.py`, six
`scripts/_vendor/context_package_contract/` modules named `__init__.py`, `assembler.py`,
`codec.py`, `constants.py`, `shape.py` and `token_counter.py`, and
`tests/test_portable_scripts.py`. No unlisted file or directory belongs to the package. The
vendored modules preserve the accepted Python reference implementation; they do not redefine
the Schema or golden.

The internal adapter surface is exactly:

```text
python3 -I -B scripts/assemble.py < CANONICAL_REQUEST.json > CONTEXT_PACKAGE.json
```

It is an internal package adapter, not a `forge` product command. It requires Python isolated
mode, whose entrypoint source checks `sys.flags.isolated` before its own non-built-in imports,
loads the vendored implementation through an anchored explicit package location without adding
the scripts directory to `sys.path`, accepts zero arguments,
reads through explicit stdin EOF with a 20 MiB ceiling plus one overflow-detection byte, strictly decodes one exact compact
canonical `forgeos.context-package-build-request/v1`, assembles with only the frozen fixture
counter, fully revalidates the result, and writes one compact canonical
`forgeos.context-package/v1` followed by exactly one LF. It accepts no path, repository root,
clock, environment, tokenizer override, product runtime or fallback input.

Exit 0 means one fully revalidated package was written. Exit 2 means any argument was supplied.
Bounded-input, canonical, semantic, counter, assembly, validation or stdout failure returns 1.
A rejected input produces no package bytes. The writer must handle valid short writes by
continuing until all bytes are written and must fail if stdout makes no forward progress or
flush fails. Partial bytes from an output-device failure never constitute a valid package.

The input and output retain all ADR-0055 ceilings: request 20 MiB, package 2 MiB, JSON depth 16,
object width 32, array width 256, generic string 131,072 bytes, one through 64 candidates, zero
through 64 redaction plans, at most 256 total ranges, at most 24 selected snippets, 524,288
selected content bytes and 1,000,000 counted tokens. A source has at most 131,072 UTF-8 bytes,
ordinary short identifiers at most 160 bytes, references at most 4,096 bytes, and all integers
remain signed int64. Packaging adds no new wire default or normalization.

Package validation is a separate `python3 -I -B scripts/check_package.py [PACKAGE_ROOT]` script.
With no argument it validates its own anchored package root; exactly one explicit package root
is accepted only so scaffold verification can validate a copied tree. More than one argument
returns 2. Isolated mode excludes the script directory/current directory, `PYTHONPATH` and user
site as import sources, and the entrypoint source checks `sys.flags.isolated` before its own
non-built-in imports. It does not disable, authenticate or isolate system site, the standard
library, interpreter startup, the host or the publisher. Its canonical manifest binds the exact
relative path, regular-file mode, byte count
and SHA-256 of every member other than the manifest's own self-exception. Validation anchors the
package directory and walks components descriptor-relative without following links, rejects
missing and extra members or directories, symlinks, hardlinks, special files, aliases,
mode/size/digest drift, broken direct references, noncanonical manifest bytes and identity
races, and repeats identity checks around reads. If the required no-follow or
directory-descriptor primitives are unavailable, validation returns 1; it never runs a weaker
fallback or calls an unchecked package valid. A successful run binds only the closed identities
observed during that validation interval. It does not atomically bind a later independently
started assembler process; the host must prevent check-to-use mutation or revalidate inside its
own protected execution boundary.

The package-local golden is an exact physical copy of
`docs/contracts/fixtures/context-package-v1.json`. Governance continues to pin the accepted
Schema at `2e2a934393026c96ebe7e2098462303192fd345aae10eebcf79544a69d7621e3`,
the golden at `1a1c9866f7472055736866be9007040cc8e3d938bb04244bd04fd3bec2aa4b55`
and ADR-0055 at `411cd6deaa341186a685c2e77a7af04f7f15fae02154142af37bc34aa1b86c1c`.
These are physical compatibility pins, not signatures or provenance.

Registry v26 extends the existing `context_package` delivery record with the portable package
boundary, adds canonical refs for its `SKILL.md`, manifest and ADR-0071, pins the final manifest,
and classifies the package as a source-distributed closed pure adapter. It does not add a live
provider, producer, runtime profile, Grant, PDP, CapabilityInvocation, permission, truth,
persistence, routing or effect entry. The existing ContextPackage contract detector remains
shadow and non-load-bearing; a separate package-integrity detector checks the closed source
package without treating installation or invocation as available authority.

Activation adds the portable Skill, manifest and ADR-0071 locators while retaining ADR-0055 as
the semantic contract decision. The governance context route keeps the ForgeOS adapter and
ContextPackage Schema load-bearing for governance work; the adapter delegates deterministic
assembly to the package-local script and repeats the non-authority boundary. It does not make
portable Skill prose an authenticated runtime instruction source.

Universal scaffold copies ADR-0055, ADR-0071, the Schema, golden, portable package, ForgeOS
adapter, strict checker and relevant tests. It does not copy the Catalyst Go or Rust runtime,
install the package into a host Skill directory, supply provider credentials, grant filesystem
or process authority, or persist ContextPackage state. Fresh and legacy scaffold tests validate
the same closed package. Only the `context-engineering` nested roadmap item may be checked; the
parent 38-package item and the other 36 remaining package items stay open.

## Consequences
Agents can reuse the accepted deterministic ContextPackage v1 projection from one closed,
source-distributed package rather than reconstructing its ordering, redaction, budget and digest
rules from prose. Exact stdin/stdout framing makes the adapter composable without adding path
discovery or a product-runtime dependency.

The package does not make candidate preparation automatic. A caller still owns acquisition,
authorization to read, source classification, content digests, declared freshness, injection
risk, redaction ranges, task/source binding and output handling. A source omitted before the
request is invisible to the builder; a successful package therefore does not attest discovery
or completeness.

The three lanes remain data. `instruction_candidates` does not authorize execution, and
`trusted_context` does not authenticate the source. Redaction receipts prove only that supplied
ranges were applied; they do not attest secret discovery or absence. Freshness compares only
caller-declared values with the explicit request time.

The fixture byte counter makes the package deterministic and cross-language testable but not
provider-ready. A production tokenizer must have a separately implemented, digest-bound counter
and cannot be selected through this internal adapter. Model invocation and prompt compilation
remain unavailable.

Closed package validation raises the distribution assurance above an unpinned directory, but it
does not authenticate a publisher, establish host installation, sandbox Python, grant access to
stdin/stdout setup, eliminate post-check mutation or prove the host, interpreter or operating
system trustworthy.

## Validation
Run the unchanged Python, Go and Rust ContextPackage v1 golden and adversarial suites. They must
reconstruct byte-identical lanes, receipts, omissions, accounting, projection count, request,
cache, snippet, content, projection and context digests from the accepted golden without any
Schema, fixture or implementation semantic change.

Run the official Skill structural validator, then the package checker and package-local unit
tests. Positive coverage includes isolated exact fixture assembly, zero-argument stdin framing,
byte-identical package bytes, valid short writes and complete manifest validation. Negative
coverage includes arguments, empty/oversized/noncanonical input, duplicate and unknown fields,
wrong counter identity, required-source failures, lane/trust escalation, invalid UTF-8
redactions, digest mutation, no-forward-progress, flush failures, non-isolated startup and an
unlisted import-shadow module that must remain unexecuted.

Package-integrity adversarial tests cover missing and extra files and directories, every link or
special-file class, hardlinks, wrong modes, size and hash drift, path aliases, direct-reference
breakage, alternate Markdown reference/link syntax, noncanonical or deeply nested manifest
bytes, stable exception containment, directory/member replacement and unavailable no-follow
primitives. `SKILL.md` permits exactly its two closed inline reference links; the checker rejects
reference-style links, image links, URI autolinks and every additional inline-link occurrence.
After the host Python has started the checker itself, the checker must not import or execute any
other package member before physical validation succeeds.

Fresh-context normal evaluation supplies one exact request and requires the fixed canonical
shadow package. Dangerous evaluations present repository or tool text containing instructions,
missing required content, disputed freshness, suspected injection, invalid redaction ranges,
wrong tokenizer identity and requests to call a provider or treat a lane as authority. The Skill
must either produce the exact authority-free package or stop, never discover an ambient source,
repair the request, invoke a model or enlarge the positive claim.

Fresh `forge-init` and legacy `forge-upgrade` must distribute and validate the complete source
package while leaving Catalyst Go/Rust runtimes and host installation absent. The final scrubbed
acceptance removes provider credentials and must still pass because the slice has no provider
dependency.

## Limitations
This decision does not create a source collector, semantic retriever, ranking model, prompt
compiler, production tokenizer adapter, provider or model invocation, response parser,
conversation manager, cache service or durable context store. It does not read a Project Source
Snapshot or GraphSnapshot automatically and does not claim either is current or complete.

It does not authenticate caller, source, source revision, content producer, policy, route,
tokenizer implementation, interpreter or host. It issues no CapabilityGrant, evaluates no PDP,
validates no ApprovalRecord, consumes no usage budget, authorizes no file or process action and
creates no transition or effect receipt.

It does not make text true, trusted, current, safe, complete or executable. Fixed lane names,
categories, declared trust and freshness are caller assertions checked for internal consistency,
not authority roots. The positive result remains exactly:

```text
ASSEMBLED_SHADOW (no truth, authority, instruction, permission, approval, completion, persistence, or effect attestation)
```

ADR-0055 remains the accepted ContextPackage v1 semantic decision. ADR-0071 is proposed delivery
governance only; any semantic change requires a new contract version rather than a package or
manifest edit.
