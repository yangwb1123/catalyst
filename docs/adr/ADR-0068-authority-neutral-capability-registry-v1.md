---
{"acceptance_id":null,"accepted_at_unix_ms":null,"adr_id":"ADR-0068","affected_node_ids":[],"alternatives":[{"alternative_id":"authority-neutral-singleton-registry","description":"Freeze one physically bound content-addressed capability and a pure exact resolver.","disposition":"candidate","rationale":"This is the smallest registry that reflects delivered implementations without manufacturing authority or catalog completeness."},{"alternative_id":"planning-catalog-promotion","description":"Embed or rebuild the 140-item planning catalog and Skill map as registry entries.","disposition":"rejected","rationale":"Planning declarations are not executable contracts and most entries lack the physical bindings required by this profile."},{"alternative_id":"runtime-routing-registry","description":"Use resolution to select implementations or route runtime invocations.","disposition":"rejected","rationale":"Resolution supplies neither authenticated authority nor Grant, permission, availability, execution or effect evidence."}],"api_version":"forgeos.architecture-decision-record/v2","approver_refs":["architecture-review","governance-review"],"assumption_claim_ids":[],"body_sha256":"c71c506861965300285391829e014daf2351fb0ba5fa75408b2321b61ab4b2c0","canonicalization":"forgeos.canonical-json/v1","compatibility":"This staged v1 adds an isolated authority-neutral wire and preserves every legacy capability reference unchanged. It does not reinterpret the ADR-0056 64-8 digest, promote planning catalog entries or add a runtime consumer.","consequences":["One exact capability can be validated and resolved by declared identity without authorization or execution.","Any semantic, ownership, implementation, test or physical-content change propagates through the content-addressed digest chain.","The planning catalog coverage and adapter-generation work remains open, and Rust is not a delivered consumer."],"context_claim_ids":[],"decision":"Adopt a staged, authority-neutral, read-only, content-addressed Capability Registry v1 with exactly the local-go-package-impact-prescan/1 entry and strict Python and Go validation and resolution semantics.","decision_driver_claim_ids":[],"document_name":"ADR-0068-authority-neutral-capability-registry-v1.md","evidence_record_ids":[],"expires_at_unix_ms":null,"implementation_refs":["docs/adr/ADR-0068-authority-neutral-capability-registry-v1.md","docs/contracts/capability-registry-v1.schema.json","docs/contracts/local-go-package-impact-prescan-v1.schema.json","forge-core/internal/goimpactprescan","harness/local_go_package_impact_prescan_contract"],"kind":"ArchitectureDecisionRecord","owner_refs":["governance","runtime-engineering"],"proposed_at_unix_ms":1786593600000,"revisit_triggers":[{"condition":"A second physical capability entry is proposed.","evidence_required":["A versioned registry migration and complete physical contract bindings for every admitted entry."],"trigger_id":"additional-entry"},{"condition":"Registry resolution is proposed as input to authorization, Grant activation, invocation, transition or effect routing.","evidence_required":["A separately governed authenticated authority integration with fail-closed end-to-end tests."],"trigger_id":"authority-integration"},{"condition":"Any identity, digest domain, request, assessment, relation, bound, catalog boundary or CLI semantic changes.","evidence_required":["A new contract version, cross-language golden and adversarial compatibility evidence."],"trigger_id":"wire-change"}],"risks":[{"description":"Consumers may mistake exact declared resolution for authentication or authority.","mitigation":"All positive text and fixed assessment fields deny authentication, authorization, permission, invocation, routing, transition, execution and effect attestation.","risk_id":"authority-confusion"},{"description":"A direct or indirect self-reference could make registry identity impossible to reproduce.","mitigation":"Freeze the acyclic content-set to contract to entry to registry to request to assessment chain and forbid registry governance files in entry content sets.","risk_id":"digest-cycle"},{"description":"The historical opaque 64-8 capability digest could be treated as content-addressed or privileged.","mitigation":"Do not register repository-reader and handle the value only through ordinary structured negative resolution.","risk_id":"legacy-digest"}],"rollback":"Remove consumers of the staged v1 wire and retain historical files as inert records; do not rewrite legacy capability references or treat a prior successful resolution as revocation, authorization or state transition.","rollout":"Ship the closed Schema and proposed ADR first, then require one physical golden, strict Python and Go libraries, the single validate/resolve CLI surface, physical checking, adversarial bounds tests, scaffold propagation and repository acceptance before roadmap closure.","scope_refs":["capability-registry-v1","local-go-package-impact-prescan"],"self_sha256":"427e2e3e0826b2ec570818485f129fd32f1f86cf9e8daf95d6137b39a0d6ce5d","status":"proposed","superseded_by":[],"supersedes":[],"title":"Authority-neutral Capability Registry v1","validation_plan":[{"description":"Attack canonical framing, identity projection, relation matrices, digest cycles, authority constants and resource bounds.","due_trigger":"Before either Registry roadmap checkbox is closed.","evidence_required":["Python and Go adversarial suites with exact failure and zero-stdout CLI assertions."],"owner_ref":"governance","success_criteria":"Every malformed, resealed, ambiguous or authority-escalating input fails closed without ambient reads or execution.","validation_id":"adversarial-contract"},{"description":"Reproduce every registry object and assessment byte and digest in both delivered languages.","due_trigger":"Before repository acceptance of Capability Registry v1.","evidence_required":["One exact physical golden consumed independently by Python and Go."],"owner_ref":"runtime-engineering","success_criteria":"Python and Go produce byte-identical canonical objects and lowercase SHA-256 identities.","validation_id":"cross-language-golden"},{"description":"Verify exact file bytes, recursive selections and explicit sets for the singleton implementation and tests.","due_trigger":"Before accepting the frozen entry or after any bound file changes.","evidence_required":["Physical checker output covering hashes, byte counts, completeness, symlink, special-file and identity-stability cases."],"owner_ref":"runtime-engineering","success_criteria":"Only the exact declared regular files and complete recursive selections validate.","validation_id":"physical-content-binding"}]}
---

# ADR-0068: Authority-neutral Capability Registry v1

## Context
ForgeOS has strict contracts for Evidence, Claims, ContextPackage, CapabilityGrant,
ApprovalRecord, TransitionReceipt and several pure evaluators, but it does not have a closed
Capability Registry. The planning catalog names 140 lifecycle capabilities and maps them to
38 Skill packages. That catalog is explicitly `planning_only` and `executable:false`; its
node prose, authority lists and ownership map cannot be promoted into runtime behavior merely
because a resolver exists.

ADR-0038 describes a future Registry containing capability identity, schemas, effects, proof
obligations, ownership, implementations and tests. The Wave 4 roadmap also calls for trigger,
not-applicable, input/output, rule/gate and permission fields. A first implementation can
freeze and resolve those declarations without selecting an implementation, activating a
Grant, authorizing an invocation, evaluating a rule or gate, dispatching an effect, advancing
a transition or routing runtime work.

One existing capability is suitable for a truthful physical entry. ADR-0062 delivers a
bounded, deterministic, caller-bytes-only local Go package impact prescan in Go and Python,
with a strict Schema, exact golden, CLI/API adapters and adversarial/bounds tests. It computes
only an exact lexical reverse dependency closure and always leaves system impact UNKNOWN. It
is therefore registered under the narrow identity `local-go-package-impact-prescan` version
`1`, not under the broader planning-catalog capability `change-impact-analysis`.

The ADR-0056 through ADR-0058 fixtures contain a deliberately opaque
`capability_contract_sha256` consisting of 64 ASCII `8` characters for
`repository-reader/1`. Those accepted historical wires cannot be rewritten, and the opaque
value cannot be special-cased into a content identity. This Registry consequently does not
register `repository-reader`; a declared legacy reference receives an ordinary structured
digest mismatch if its exact key exists in some future registry, or an ID/version-not-found
result here. Neither outcome is permission.

## Decision
Adopt the staged `forgeos.capability-registry/v1` wire, kind `CapabilityRegistry`, canonical
JSON profile `forgeos.canonical-json/v1`, status `staged`, mode
`authority_neutral_read_only_contract_catalog`, and coverage
`explicit_entries_only_not_global_inventory`. Version 1 contains exactly one entry and pins
the ADR-0056 effect vocabulary digest. It does not claim a global capability inventory.

The singleton entry embeds an exact `forgeos.capability-contract/v1` and exact owner,
implementation, test and physical content-set declarations. The contract freezes capability
ID and opaque version, domain, input/output Schema refs, trigger, not-applicable predicates,
preconditions, postconditions, effects, proof obligations, failure modes, observability,
risk floor, rules, gates, permission requirements and rollback/compensation. Entry metadata
adds declared module/team ownership plus Go/Python implementation and test adapters. Owner
labels are not authenticated and adapter declarations do not prove availability or execution.

Structured content refs contain exactly `content_bytes`, `content_sha256`, `media_type`,
`path` and nullable `selector`. Paths are normalized repository-relative paths. Selectors are
null, `#`, or bounded ASCII JSON Pointer fragments; the pure resolver does not dereference
them. A physical integration checker must read only the explicitly declared repository refs,
reject symlinks and special files, bind pre/post filesystem identity, and require exact byte
length and SHA-256.

A content set contains exact `files`, `selection` and `set_sha256`. Selection is either an
explicit file list or the complete recursive regular-file set under one root matching a
sorted suffix list. Recursive mode must reject an omitted or additional matching file;
explicit mode asserts only exact listed-file identity. Content refs and content sets are
already raw-UTF-8/canonical-byte sorted and unique; validators never silently sort or
deduplicate input.

Physical recursive traversal counts the selection root once and every observed descendant
directory entry once before suffix filtering and fails before the total exceeds 65,536. The repository root,
every traversed directory and every read entry must retain the same device, inode, mode, size,
mtime and ctime identity before and after use; opened files additionally bind descriptor
identity before and after the bounded read.

String sets use raw UTF-8 byte order. Entries use capability ID, opaque version and entry ID;
named objects use their respective entry, failure, signal, requirement, obligation, gate,
rule, implementation, adapter or test ID. Content sets, content-ref arrays, predicates and
schema/fixture/verification refs use compact canonical JSON bytes. Any duplicate content ref
`path`/`selector` tuple is invalid even if its other fields differ.

The frozen physical sets are the current 18 `.go` files beneath
`forge-core/internal/goimpactprescan`, the current eight `.py` files beneath
`harness/local_go_package_impact_prescan_contract`, and an explicit three-file Python
CLI/contract/bounds suite. Their content-set digests are respectively
`549818abbb33737c9198607e2d43b56efef50b476aac507446d5501f86b4de22`,
`effade443429146470a13b55341c73228fa8d718e88be47532260663fb534bd4`, and
`3d7a072ffcaa6a222ae42ef6ac1b6135029ad5158fc38c70ce35f5afb3a28100`.
The ADR-0062 Schema and golden remain separately bound by their current physical SHA-256 and
size. Registry v1 Schema, fixture, checker, ADR and governance pin files are forbidden from
the entry implementation/test content sets, preventing the Registry from hashing itself
through an indirect physical reference.

The Capability Registry physical golden is exactly one compact canonical JSON object followed
by exactly one LF. Generic Registry, request and assessment object bytes do not include that
LF. The golden loader rejects a missing or additional terminal LF, and its exact physical
size is 28,758 bytes with SHA-256
`0ce4929ad82ce70ef0520be80b7bd3eaf47f5ff1205d0a53e12fbe1115ed11b5`; it does not accept
an arbitrary semantically equivalent fixture.

Digest construction is an acyclic chain:

```text
content set -> capability contract -> entry -> registry -> request -> assessment
```

Each object uses lowercase SHA-256 over its exact ASCII domain including terminal NUL,
followed by compact canonical JSON. Only its own identity fields are empty in its preimage:
`set_sha256`; contract ID and digest; entry ID and digest; registry ID and digest; request
digest; or assessment digest. Derived IDs use `capability-contract-`,
`capability-registry-entry-` and `capability-registry-` followed by their digest. An earlier
preimage may not contain a later identity. Proof, owner, implementation and test fields are
never removed from the identity that owns them.

The exact digest domains are:

```text
forgeos.capability-registry.content-set.v1 NUL
forgeos.capability-contract.v1 NUL
forgeos.capability-registry-entry.v1 NUL
forgeos.capability-registry.v1 NUL
forgeos.capability-registry-declared-resolution-request.v1 NUL
forgeos.capability-registry-declared-resolution.v1 NUL
```

The resolution request contains the exact current registry digest, an
`expected_reference`, a nullable `expected_contract`, and its own digest. The reference is
exact capability ID, opaque version, contract digest and origin
`current_registry|external_declared|external_legacy`. When the complete contract is present,
it is limited to the v1 singleton `local-go-package-impact-prescan/1`, and its
ID/version/digest projection must equal the reference exactly. Unknown-ID or unknown-version
structured negative comparisons must set `expected_contract` to null. With a null contract
the resolver performs only structured key/digest resolution and reports every relation
except identity as `not_evaluated`.

Resolution order is exact ID, exact opaque version, exact contract digest, then optional
byte-identical contract. Results are only `capability_id_not_found`,
`capability_version_not_found`, `capability_contract_digest_mismatch` or `resolved_exact`.
There is no SemVer range, latest, major fallback, alias, case folding, Unicode normalization,
implementation preference or environment fallback. Equal digest with unequal contract bytes
is a hard internal inconsistency and produces no assessment.

The relation keys are exactly `domain`, `effects`, `failure_modes`, `identity`,
`input_schemas`, `not_applicable`, `observability`, `output_schemas`,
`permission_requirements`, `postconditions`, `preconditions`, `proof_obligations`,
`quality_gates`, `risk_floor`, `rollback_or_compensation`, `rules` and `trigger`.
Identity is exactly `same_declared_identity`, `capability_id_not_found`,
`capability_version_not_found` or `capability_contract_digest_mismatch`. Every other key
`K` has the frozen vocabulary `same_declared_K`, `K_mismatch` or `not_evaluated`.
`resolved_exact` uses `same_declared_identity`; with a nonnull `expected_contract` all other
relations are `same_declared_K`, while a null contract makes them all `not_evaluated`.
Every non-resolved result also makes all nonidentity relations `not_evaluated`. The mismatch
values are reserved for a future declared comparator and are never emitted by the v1
resolver; equal digest with unequal contract bytes remains a hard error, not a mismatch
assessment.

The assessment binds the registry and request identities, nullable matched-key entry
identity, resolution, fixed relations, sorted reason codes and exact authority-neutral
constants. Its only positive wording is:

```text
RESOLVED_DECLARED_CAPABILITY_REFERENCE_ONLY (no registry or owner authentication,
rule or gate applicability, proof satisfaction, test pass, implementation availability,
Grant activation, authorization, permission, invocation, runtime routing, persistence,
transition, execution, or effect attestation)
```

Registry and owner authentication, rule/gate applicability and proof satisfaction are always
`not_evaluated`; authorization decision is always `none`; every permission, invocation,
routing, persistence, transition, implementation, test and effect attestation is false. The
assessment is re-derived in full and accepted only when its compact canonical bytes match.

Strict decoding rejects noncanonical JSON, duplicate or unknown fields, floats, bool-as-int,
integers outside signed int64, forbidden controls/DEL/bidi/surrogates/U+2028/U+2029,
noncanonical order, duplicate set entries, excessive depth/cardinality/string/document bytes
and digest drift. Runtime UTF-8 byte limits are authoritative; Schema string lengths are only
code-point approximations. The pure resolver consumes explicit bytes only and performs no
repository, environment, clock, credential, process, provider, database or network access.

The only command surface is `forge capability-registry`, with exactly the `validate` and
`resolve` subcommands. `validate --registry FILE|-` emits the exact validated Registry bytes
plus LF. `resolve --registry FILE|- --request FILE|-` requires exactly one input to be `-`
and emits the exact assessment bytes plus LF. Unknown, repeated, missing or positional
arguments exit 2 with empty stdout; malformed, profile or semantic failures exit 1 with
empty stdout. Both subcommands are adapters over the same pure library and must not search
for a Registry, read a planning catalog, select an implementation or execute a capability.
Each stdin document is complete only after explicit EOF. The adapters read through that EOF
under the selected document bound and may observe one additional overflow-detection byte; a
temporarily exhausted nonblocking stream is incomplete and fails closed with empty stdout.

## Consequences
- ForgeOS gains one content-addressed Capability Contract, physical implementation/test
  binding and exact read-only resolver without manufacturing authority.
- The minimal Registry and Wave 4 Registry Schema requirements can be closed only after the
  fixture, Python/Go validators, CLI/API adapters, physical checker, scaffold wiring,
  adversarial tests and repository acceptance are complete. This staged ADR and Schema alone
  do not close them.
- The planning capability catalog remains planning-only. `catalog_binding:null` explicitly
  marks this foundation capability as outside lifecycle Skill ownership and does not weaken
  the catalog-to-package coverage check.
- Registry v1 neither embeds nor rebuilds the 140-item planning catalog or its Skill map.
  The planning-catalog coverage and adapter-generation roadmap checkbox remains open.
- Rust is `not_delivered`: Registry v1 has no Rust implementation, validator, fixture
  consumer or acceptance requirement.
- `repository-reader/1` remains an independently governed ADR-0057 profile with an opaque
  historical contract reference. It is neither registered nor activated here.
- CLI and API are adapters over the same pure resolution semantics; neither becomes a new
  truth, permission or invocation path.
- Changes to capability semantics alter the contract digest; changes to owner,
  implementation, tests or physical sets alter the entry digest; either changes the registry
  digest and requires a new request binding.

## Validation
Validation must reproduce exact contract, content-set, entry, registry, request and assessment
bytes and digests across Python and Go from one physical golden. Rust is not a delivered
consumer in this slice. Validation must independently verify every declared physical file
byte count/hash, complete recursive set selection, explicit set equality and
symlink/special-file/TOCTOU rejection.

Positive cases must cover the exact singleton entry through library API and CLI adapters.
Negative cases must cover ID, version and digest mismatch; nullable opaque legacy reference;
the ADR-0056 64-`8` digest; duplicate entry key; reordered or duplicate sets; contract,
owner, implementation, test, trigger, rule, gate, proof, effect and permission mutation;
unknown vocabulary; malformed selector/path; content omission/addition/drift; digest cycle
attempts; canonical/framing/Unicode/depth/cardinality/byte limits; and typed in-memory paths
that try to bypass document ceilings.

The physical checker must reject any attempt to include the Registry's own ADR, Schema,
fixture, checker or governance pin in the singleton implementation/test sets. The pure
resolver must pass tests proving zero ambient reads and zero execution. Mismatch assessments
must retain all false/none/not-evaluated authority constants, and assessment validation must
reject any caller-resealed claim of availability, PASS, permission, authorization,
invocation, routing or effect.

Universal scaffold tests must prove exact Schema/fixture/checker propagation without claiming
that a fresh project contains Catalyst's Go implementation. Full `forge accept` remains the
repository completion authority and is not Registry trust or Capability authority.

## Limitations
This decision does not authenticate the Registry, owner, implementation, tests or physical
producer. It does not evaluate trigger/not-applicable predicates, pre/postconditions, rules,
gates, proofs or permissions. It does not activate or validate a CapabilityGrant, construct a
CapabilityInvocation, select or execute an implementation, route a runtime request, dispatch
an effect, update a transition, persist Registry state, load a plugin, generate Skill/role/
workflow adapters, or prove catalog completeness.

The singleton entry is not full change-impact analysis, selected-build reachability, complete
GraphSnapshot, ChangeImpactReport, Cost/Risk assessment or G3 evidence. System impact remains
UNKNOWN. The physical source hashes describe current repository bytes, not source
authenticity, producer identity, semantic correctness or future availability.

Revisit with a new version before admitting more entries, changing identity or content-set
preimages, adding SemVer/ranges/aliases, resolving remote refs, promoting the planning catalog,
selecting implementations, evaluating rule/gate/proof/permission applicability, integrating
CapabilityGrant or CapabilityInvocation, adding persistence, plugin distribution or runtime
routing, or granting any authority/effect meaning to `resolved_exact`.
