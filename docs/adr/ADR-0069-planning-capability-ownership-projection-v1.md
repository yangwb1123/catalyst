---
{"acceptance_id":null,"accepted_at_unix_ms":null,"adr_id":"ADR-0069","affected_node_ids":[],"alternatives":[{"alternative_id":"ambient-path-reader","description":"Read the two planning files from conventional repository paths inside the pure projector.","disposition":"rejected","rationale":"Ambient reads make the request incomplete, introduce path and TOCTOU behavior, and obscure which exact bytes were projected."},{"alternative_id":"planning-logical-ownership-projection","description":"Project complete ownership and logical adapter locators from two explicitly embedded exact planning YAML sources.","disposition":"candidate","rationale":"This closes the declared coverage computation without promoting planning data into the Registry or claiming files and Skills exist."},{"alternative_id":"registry-promotion-or-runtime-adapters","description":"Populate the ADR-0068 Registry or generate and activate host adapters directly from the ownership map.","disposition":"rejected","rationale":"Planning ownership supplies neither physical Capability Contracts nor Skill availability, authority, invocation or runtime evidence."}],"api_version":"forgeos.architecture-decision-record/v2","approver_refs":["architecture-review","governance-review"],"assumption_claim_ids":[],"body_sha256":"c1dbafc35a9cab89e827de7e89ad8f253b8a145eba0aece661b5b3198d45755d","canonicalization":"forgeos.canonical-json/v1","compatibility":"This proposed v1 consumes only explicit exact source bytes and leaves both planning YAML files and the staged ADR-0068 singleton Registry unchanged. No legacy capability or Skill reference is reinterpreted.","consequences":["The 140 declared fine capabilities are projected to exactly one primary planning owner without hiding their 145 lifecycle-node occurrences.","Logical .agent/skills locators remain unresolved declarations and do not claim a file, portable Skill package or implementation exists.","Strict source parsing and content-addressed output are delivered independently in Python and Go without YAML normalization."],"context_claim_ids":[],"decision":"Adopt a proposed planning-only pure projection contract that consumes two caller-supplied exact bounded YAML byte strings, verifies complete unique capability ownership, and derives content-addressed logical adapter references without physical resolution, Registry mutation or runtime authority.","decision_driver_claim_ids":[],"document_name":"ADR-0069-planning-capability-ownership-projection-v1.md","evidence_record_ids":[],"expires_at_unix_ms":null,"implementation_refs":[".agent/skills/capability-ownership-projection.md","docs/adr/ADR-0069-planning-capability-ownership-projection-v1.md","docs/contracts/fixtures/planning-capability-ownership-projection-v1.json","docs/contracts/planning-capability-ownership-projection-v1.schema.json","forge-core/internal/planningownership","harness/planning_capability_ownership_projection"],"kind":"ArchitectureDecisionRecord","owner_refs":["architecture","governance","runtime-engineering"],"proposed_at_unix_ms":1786651200000,"revisit_triggers":[{"condition":"The projection is proposed as input to authorization, invocation, routing or another authority-bearing runtime.","evidence_required":["A separately versioned authenticated authority integration with fail-closed end-to-end evidence."],"trigger_id":"authority-integration"},{"condition":"A logical adapter reference is proposed to prove a file, Skill package or implementation exists or is available.","evidence_required":["A separately governed physical package contract, bounded resolver and availability semantics."],"trigger_id":"logical-ref-physical-resolution"},{"condition":"The source shape, accepted YAML subset, projection fields, canonicalization, bounds or digest preimage changes.","evidence_required":["A new contract version with independent Python and Go golden and adversarial compatibility evidence."],"trigger_id":"source-or-wire-change"}],"risks":[{"description":"Consumers may treat a derived .agent/skills path as proof that a loadable Skill exists.","mitigation":"Fix physical resolution and availability to not_performed and not_evaluated, and prohibit filesystem access in the projector.","risk_id":"logical-ref-confusion"},{"description":"Python and Go YAML defaults may disagree on coercion, duplicates, aliases or folded scalars.","mitigation":"Freeze the exact supported syntax, scalar typing, duplicate rules, folding behavior and resource bounds; generic parsers alone are insufficient.","risk_id":"source-parser-divergence"},{"description":"Ignored catalog prose or authority fields could be smuggled into runtime meaning.","mitigation":"Close and bind their shapes but semantically project only node capability occurrences and the explicit ownership package fields.","risk_id":"source-smuggling"}],"rollback":"Stop producing or consuming the proposed projection and retain the two source YAML files and ADR-0068 Registry unchanged; prior projection bytes remain inert planning artifacts and are not revocation or transition records.","rollout":"Deliver the strict independent Python and Go pure projectors, one exact cross-language golden, the bounded product CLI, governance and source-only scaffold wiring, and close only complete unique declared ownership coverage plus logical adapter-reference derivation while the ADR remains proposed.","scope_refs":["capability-ownership-projection-v1","planning-capability-catalog"],"self_sha256":"95982bd03ce7bc5d12fe56a6eb7c18b533fef1798c66eea490bb62ef9b530386","status":"proposed","superseded_by":[],"supersedes":[],"title":"Planning Capability Ownership Projection v1","validation_plan":[{"description":"Reproduce the exact request, 140 bindings, coverage and final projection bytes independently in Python and Go.","due_trigger":"Before any delivery or roadmap closure for the projection.","evidence_required":["One physical golden built from the exact pinned catalog and map source bytes."],"owner_ref":"runtime-engineering","success_criteria":"Both runtimes emit byte-identical canonical output and every domain-separated digest matches.","validation_id":"cross-language-golden"},{"description":"Attack the frozen YAML and canonical JSON profiles, base64/source binding, bounds, duplicates and forbidden syntax.","due_trigger":"Before repository acceptance of the implementation slice.","evidence_required":["Positive boundary and N+1 adversarial tests in both delivered runtimes."],"owner_ref":"governance","success_criteria":"Malformed, ambiguous, oversized, resealed or partially consumed input fails closed with no projection.","validation_id":"parser-adversarial"},{"description":"Recount every catalog occurrence and enforce one global primary owner for every unique fine capability.","due_trigger":"Before generating any logical adapter reference.","evidence_required":["Coverage tests for missing, extra, duplicate-primary, repeated-node and dangling ownership cases."],"owner_ref":"architecture","success_criteria":"The current sources derive exactly 17 nodes, 145 occurrences, 140 capabilities, 38 packages and 140 bindings without loss.","validation_id":"projection-coverage"}]}
---

# ADR-0069: Planning Capability Ownership Projection v1

## Context
ForgeOS has two planning artifacts that are deliberately not runtime state. The capability
catalog declares 17 lifecycle nodes and 145 capability occurrences representing 140 unique
fine capabilities. The ownership map assigns those 140 unique capabilities to 38 declared
Skill package names. Both sources say `planning_only` and `executable:false`.

ADR-0068 intentionally excludes these files from its staged singleton Capability Registry.
That Registry contains only the physically bound `local-go-package-impact-prescan/1` entry;
its resolver must not read or rebuild a planning catalog, generate adapters, select an
implementation or manufacture runtime authority. Changing that Registry to close the separate
catalog-coverage requirement would collapse a planning declaration into a runtime contract.

The ownership map describes logical package names. A derived path such as
`.agent/skills/delivery-planning.md` is useful for planning and audit, but its derivation cannot
prove that a file exists, that it is a valid portable `SKILL.md`, that a host can load it, or
that an implementation is available. The delivered honest boundary is therefore a bounded pure
projection from two explicit exact source byte strings, with complete ownership coverage and
logical adapter references but no physical resolution or runtime meaning.

## Decision
Adopt `forgeos.planning-capability-ownership-projection-request/v1` and
`forgeos.planning-capability-ownership-projection/v1` as a proposed planning-only pure
projection contract. The output kind is `PlanningCapabilityOwnershipProjection`, its status is
`planning_only`, and its mode is
`planning_only_declared_ownership_and_logical_adapter_refs`.

### Explicit source request
The request embeds the catalog and ownership-map source bytes separately as canonical RFC 4648
base64. Each source record contains exactly `content_base64`, `content_bytes`,
`content_encoding`, `content_sha256`, `document_name`, `media_type` and `source_role`.
Decoded byte count and raw SHA-256 must match. The source bytes are not called canonical YAML:
identity is the exact decoded raw bytes, and no YAML reserialization participates in a digest.

The source names are exactly `capability-catalog.v1.yml` and
`capability-skill-map.v1.yml`. They are logical names inside the explicit request, not paths to
read. The projector performs no repository search, path resolution, environment discovery or
fallback. The map's declared `source_catalog` must equal the catalog source basename.

The current source observation is pinned by the delivered cross-language golden: the catalog is
33,000 bytes with raw SHA-256
`bc6efe535539c5f129af51486d8e81b9844b5ee6448fae2bce649fc159658d74`; the map is 5,924
bytes with raw SHA-256
`bfb2277fe66cd9f0c609b5be10ad77ad0969603edd19e5a6ccbe38b8e3409462`.
These are exact-source observations, not signatures, provenance or a rule that an ambient file
with the same name is current.

### Frozen YAML subset
Both decoded sources must use the ASCII subset of UTF-8, LF-only framing with exactly one
terminal LF, and no BOM, CR, TAB, trailing horizontal whitespace, DEL, non-ASCII byte or C0
control other than LF. The parser consumes the complete stream. It supports space-indented
block maps and sequences including a bare nested `-`, compact sequence mapping entries, flow
maps and sequences including empty `{}` and `[]`, plain scalars, double-quoted scalars without
escape bytes, lower-case `true`, `false` and `null`, canonical signed-int64 decimal integers,
and folded block scalar `>-`.

The document root is a block map or block sequence. After a block mapping colon at end of line,
or after a bare sequence dash, the next nonblank exactly indented child is likewise a block map
or block sequence; it cannot be a standalone plain, quoted, typed or flow value. Plain, quoted,
typed and flow values, including empty `[]` and `{}`, occur only inline after `key: ` or `- `.
Blank physical lines are skipped at document start, between block entries, and between such a
key or bare dash and its next nonblank child. A blank line instead terminates folded content.

Every plain mapping key, and every complete double-quoted/no-escape key after decoding, must
match `[A-Za-z0-9][A-Za-z0-9._/-]*`, be at most 16,384 UTF-8 bytes, and not equal `<<`. A block
mapping colon is followed either immediately by LF for a nested value, or by exactly one ASCII
space and an immediate nonspace inline value. A flow mapping colon is followed by exactly one
ASCII space and an immediate nonspace value. A block sequence entry is either a bare `-` whose
next nonblank line is an exactly two-space-indented nested value, or `-` followed by exactly one
ASCII space and an immediate nonspace inline value; additional spaces after the dash are invalid.

Scalar typing is attempted before plain-string fallback. Exact lower-case `true`, `false` and
`null`, plus canonical signed-int64 decimal `0` or `-?[1-9][0-9]*` within range, are typed;
canonical negative values including `-1` and MinInt64 are therefore valid. After a block context
removes its mapping or sequence syntax prefix, a plain-string fallback is nonempty, has no
leading or trailing padding, and is at most 16,384 UTF-8 bytes. A flow context skips or trims
ASCII space at token edges while preserving interior spaces. For that non-typed fallback, the
first byte is none of `-`, `?`, `:`, comma, `[`, `]`, `{`, `}`, `#`, `&`, `*`, `!`, `|`, `>`,
single quote, `%`, `@` or backtick, and its final byte is not `:`. A double quote may
only delimit a complete double-quoted scalar; an embedded or unmatched double quote and every
backslash or escape are invalid.

In a block plain scalar, internal `[`, `]`, `{`, `}`, comma and nonfinal colon are data. A block
sequence item is the exception: an outside-quote colon at bracket/brace nesting depth zero
selects compact-mapping syntax and must satisfy the mapping-colon rule, while a colon inside
bracket/brace nesting remains scalar data. The independent square/curly counters used only for
block top-level-colon scanning never fall below zero: during the scan, an unmatched `]` or `}`
is scalar data and leaves the corresponding depth at zero, so a later colon is top-level. These
counters do not require balanced brackets in a block plain string. In flow, value-leading `[`
or `{` starts a structural collection; otherwise internal `[` and `{` are plain-scalar data.
Outside double quotes, comma,
`]` and `}` terminate the current flow token and are not internal scalar data. Colon is not a
flow-value delimiter, but the trailing-colon prohibition still applies.

Every nonfinal flow-map or flow-sequence entry is followed by a comma, and every final entry is
followed by its matching closing `}` or `]`; a trailing comma immediately before that closing
delimiter is invalid. A compact mapping begun by an inline block-sequence item may continue
after skipped blank lines only with mapping pairs exactly two spaces beyond the sequence dash's
indentation. A sequence line or any differently indented line in that continuation is invalid.

For `>-`, nonempty content is indented exactly two spaces beyond its key, adjacent nonblank
exact-indent lines fold with one ASCII space, a blank line ends the scalar, the final line break
is stripped, and the joined scalar is at most 16,384 UTF-8 bytes. Other block scalar styles are
invalid. The bytes `#`, `&`, `*`, `!`, single quote and backslash are forbidden at every
position outside a complete double-quoted scalar; this is a lexical ban, not only a ban at YAML
indicator positions. Folded content is stricter: those six bytes are forbidden at every
position even when surrounded by double quotes; each physical content line must contain an even
number of double quotes, and balanced double quotes without those bytes are literal data. In
other complete double-quoted
scalars, `#`, `&`, `*`, `!` and single quote are data, while backslash remains forbidden because
escapes are unsupported. Anchors, aliases, merge keys, explicit tags, physical-line directives
and document markers as defined below, implicit timestamps, floats, broad numeric spellings,
infinities, NaN and YAML 1.1 boolean spellings are invalid. Duplicate map keys fail before
construction at every depth.

Directive and document-marker screening is physical-line contextual: a complete
indentation-stripped syntax line equal to `---` or `...`, or beginning with `%`, is rejected.
Those byte sequences elsewhere are evaluated by the ordinary scalar rules rather than treated
as ambient document syntax; in particular, inline mapping or sequence scalar `...` is a valid
string. Folded content retains its stricter per-content-line marker/directive rules above.

Every collection, mapping key and scalar consumes one token. A folded `>-` scalar consumes one
scalar token rather than one token per content line, while its joined bytes still count toward
the 16,384-byte scalar limit. Generic ignored node arrays remain nonempty arrays of 1–512
frozen-profile YAML values under the global depth, collection, token, key, sequence and scalar
bounds; the projector invents no item schema or semantics for them. Unsupported syntax,
trailing content and any resource overflow fail closed.

YAML depth starts with the root block collection at 1. Each mapping value or sequence item node,
whether scalar, folded scalar, flow value or block/flow collection, increments depth by one;
mapping keys consume a token but no depth. The maximum is 16 and is checked before parsing a
child: a node at depth 16 is valid, so an empty flow collection may occupy depth 16, but any
nonempty child would be depth 17 and is rejected before that child is parsed.

This profile is intentionally narrower than YAML 1.2. Python `safe_load` alone and the existing
generic Go `yaml2json` parser alone are not sufficient validators. A runtime may reuse a parser
only behind checks that enforce this entire lexical, scalar, duplicate, resource and
full-consumption profile. Cross-runtime output is defined by this contract rather than parser
defaults.

### Closed planning shapes and coverage
The catalog header must be `forgeos.design/v1`, kind
`AIEngineeringCapabilityCatalog`, status `planning_only` and `executable:false`. Its exact
top-level field set and every node's exact field set are frozen in the Schema. The map has the
same API, status and executable value, kind `CapabilitySkillOwnershipMap`, its exact top-level
field set is frozen, and every package contains exactly `implementation_wave`, `includes` and
`skill`.

Only catalog `nodes[].id`, catalog `nodes[].capabilities`, and map
`packages[].{skill,implementation_wave,includes}` contribute semantic ownership. All other
accepted fields are closed and bounded shape-checked but not interpreted, evaluated or
promoted into rules, gates, authority, runtime behavior or adapter content.

Specifically, catalog `authority_semantics`, `canonical_vocabulary`, `control_plane_joins`,
`gates`, `risk_levels` and `universal_node_contract` are mappings with 0–64 entries under the
complete frozen YAML bounds; an empty mapping uses the allowed `{}` syntax. Their keys and
values have no additional schema or meaning here. Catalog `decision_ref` and `runtime_note`,
and each node's `name`, `owner_lens` and `purpose`, are nonempty strings of at most 16,384 UTF-8
bytes. Catalog `extension_decision_refs` is a nonempty 1–512 array whose items are such bounded
nonempty strings, with no additional ordering or uniqueness rule.

Each node's ignored `activities`, `authority`, `entry_criteria`, `escalation`, `exit_criteria`,
`forbidden`, `handoff`, `inputs`, `memory_updates`, `outputs`, `quality_gates` and `rules` field
is a nonempty 1–512 array of arbitrary frozen-profile YAML values under all global parser
bounds; no item schema, ordering or uniqueness rule is added. Mapping `skill_specification` is
a nonempty string of at most 16,384 UTF-8 bytes, and `mapping_rules` is a nonempty 1–512 array
of such strings, again without an ordering or uniqueness rule. These validations close source
shape only and do not promote any ignored value into projection semantics.

Catalog node IDs are unique two-digit strings. Each node's capability list is nonempty and has
no duplicate. There are 1–64 nodes. Capability identifiers are 1–160 UTF-8 bytes and match
`[a-z0-9][a-z0-9._:/-]*`; the catalog has at most 512 unique capabilities and 4,096 total
capability occurrences. There are 1–64 packages. Package Skill names are unique, 1–160 UTF-8
bytes and match `[a-z0-9][a-z0-9._-]*`; their derived logical adapter reference must also fit
the 192-byte bound. Each package's `implementation_wave` is an integer from 1 through 6, and
each `includes` list contains 1–512 unique capability identifiers. Globally at most 512 mapped
capabilities occur, and every mapped capability occurs in exactly one package. The set of
unique catalog capabilities must equal the set of mapped capabilities exactly. Missing,
duplicate-primary or dangling ownership, and an invalid package shape or Skill name, produce no
projection.

The current exact sources consequently derive 17 nodes, 145 occurrences, 140 unique
capabilities, 38 packages and 140 bindings. These counts are derived output, not parser input
defaults. A capability used by multiple lifecycle nodes still has one primary owner; its
binding retains every distinct containing node ID and the exact occurrence count, so the
projection loses no catalog occurrence.

### Binding and projection wire
Bindings are raw-UTF-8 sorted uniquely by `capability_id`. Each binding contains exactly
`binding_sha256`, `capability_id`, `catalog_node_ids`, `catalog_occurrence_count`,
`declared_logical_adapter_ref`, `implementation_wave`, `owner_skill`, `physical_resolution`,
`request_sha256` and `skill_availability`. Node IDs are already raw-UTF-8 sorted and unique;
validators never silently sort or deduplicate an input projection.

The logical adapter reference is derived exactly as
`.agent/skills/` plus `owner_skill` plus `.md`. It is a declared planning locator only.
`physical_resolution` is always `not_performed` and `skill_availability` is always
`not_evaluated`; the projector must not stat, open, parse or otherwise resolve that reference.
It does not claim a portable package directory, `SKILL.md`, `agents/openai.yaml`, script,
asset, host adapter or callable implementation exists.

Coverage records exact binding, occurrence, node, mapped-capability, package and unique-
capability counts. `unmapped_capability_ids` and
`unreferenced_mapping_capability_ids` are both empty in every positive output. The complete
projection embeds the exact validated request and repeats its digest; all output members are
re-derived, and validation requires byte-identical compact canonical reassembly.

### Canonical identity chain
Request and projection JSON use `forgeos.canonical-json/v1`: exact compact UTF-8 bytes,
raw-UTF-8 ordered ASCII snake-case keys, signed-int64 integers, no floats or bool-as-int, no
duplicate or unknown fields, no Unicode normalization, and rejection of controls, DEL, bidi
controls, surrogates, U+2028 and U+2029. Runtime UTF-8 byte limits are authoritative.

The digest chain is acyclic:

```text
exact source bytes -> request -> binding -> projection
```

Raw source hashes are ordinary SHA-256 over the exact decoded YAML bytes without a domain.
The remaining lowercase SHA-256 domains are:

```text
forgeos.planning-capability-ownership-projection-request.v1 NUL
forgeos.planning-capability-ownership-binding.v1 NUL
forgeos.planning-capability-ownership-projection.v1 NUL
```

The request preimage is its compact canonical object with only `request_sha256` empty. A
binding preimage has only `binding_sha256` empty and includes the final request digest. The
projection preimage is the complete compact canonical result with only `projection_sha256`
empty, including the exact request and all final binding digests.

### Positive meaning and delivery boundary
The only positive result is:

```text
PROJECTED_PLANNING_CAPABILITY_OWNERSHIP_ONLY (complete declared primary-owner coverage
and logical adapter references for the supplied planning sources only; no file existence,
Skill availability, Registry mutation, authentication, authorization, permission,
invocation, routing, execution, persistence, transition, or effect attestation)
```

Every positive result fixes adapter file existence and Skill availability to `not_evaluated`,
authorization to `none`, persistence to `none`, attestations to empty, and all authentication,
ownership authority, Registry mutation, Grant/PDP activation, CapabilityInvocation,
permission, implementation selection, routing, execution, transition and effect flags to
false.

This delivered proposed slice includes independent Python and Go pure projectors, one exact
cross-language golden request/projection, the product CLI, universal checker, governance
wiring, tests and source-only scaffold. The product surface is exactly
`forge capability-ownership project --catalog FILE|- --mapping FILE|-`; option order is
interchangeable and exactly one source must be stdin. Usage errors return 2. Input and semantic
rejections return 1. Argument, input and semantic validation finishes before the first stdout
write, so those failures emit zero stdout bytes; success emits the exact compact canonical
projection plus one LF. A lower-level stdout short/write failure returns 1, but stdout is not
transactional: any already-written partial stream is an invalid canonical artifact and must be
discarded. Python `validate` and `--golden` surfaces are universal/internal checker operations,
not additional product `forge` commands.

A stdin source or projection is complete only after explicit EOF. The delivered adapters read
through that EOF under the selected input bound and may observe one additional overflow-
detection byte; a temporarily exhausted nonblocking stream is incomplete. The Python internal
checker fails that condition closed with exit 1 and empty stdout rather than accepting the
currently available prefix as a complete document.

For a named `FILE` source, the product CLI requires a nonempty regular leaf whose bytes fit the
catalog or mapping source bound. It rejects a leaf symlink, directory or special file and
requires the opened leaf's identity, type, mode, size and modification time to remain stable
across the bounded read. The Unix adapter additionally compares change time. The Windows
adapter relies on a reparse-point/no-follow `CreateFile` with read-only sharing plus those common
stability checks and does not separately compare change time. A Go platform without the
required no-follow regular-file adapter fails closed. This is a leaf check only:
parent-component symlinks are not prohibited, and the read does not claim directory
confinement, a current repository source, an atomic repository snapshot or stability outside
that individual read. The other source remains the one and only stdin input described above.

The scaffold copies the exact two planning sources, ADR, Schema, golden, Python pure
projector/checker and tests. It does not copy the Catalyst-only Go implementation, generate any
of the 38 declared owner Skills or host adapters, or reinterpret an already-present same-name
Markdown file as physical resolution or availability. Delivery closes only complete unique
declared primary-owner coverage and logical adapter-reference derivation.

## Consequences
- The planning catalog and ownership map gain a deterministic target projection without being
  copied into or interpreted by the ADR-0068 Registry.
- Complete primary ownership becomes machine-checkable while repeated lifecycle use remains
  visible through all node IDs and the exact occurrence count.
- Logical adapter locators can be generated reproducibly without pretending their target files
  or portable Skill packages exist.
- Any exact source-byte change changes the request and every downstream binding/projection
  digest, even when decoded ownership happens to be unchanged.
- Implementations must share a narrow YAML profile rather than inherit divergent Python or Go
  parser coercion, alias or duplicate behavior.

## Validation
The delivered implementation builds one exact golden from the pinned current source bytes and
proves byte-identical request, 140 bindings, coverage, logical references and final projection
in independent Python and Go implementations. Tests independently recount 17 nodes, 145
occurrences, 140 unique capabilities, 38 packages and 140 bindings.

Adversarial tests must cover malformed and noncanonical base64, byte-count/hash drift, source
swap, basename mismatch, BOM/CR/TAB/final-LF errors, duplicate keys, comments, anchors, aliases,
merge keys, tags, directives, document markers, every unsupported scalar form, block-folding
edge cases, trailing content and every parser resource boundary.

Semantic mutations must cover duplicate node and Skill IDs, duplicate capability within a
node or package, one capability assigned to two packages, missing and extra ownership,
implementation-wave overflow, reordered/duplicate bindings or node IDs, lost occurrence
counts, wrong logical references, resealed authority flags, digest drift and canonical JSON
limits. Zero ambient repository reads and zero file resolution or execution must be tested.

The ADR v2 checker validates this document's exact framing, metadata, body and digests, and the
Schema parses as Draft 2020-12 JSON. Focused governance, cross-language, scaffold and
architecture tests plus full repository acceptance and a fresh-context review provide delivery
evidence; Schema validation alone never proves projection semantics or delivery.

## Limitations
This decision does not alter ADR-0068, add an entry to its singleton Registry, or treat a
planning capability as a Capability Contract. It does not generate an adapter file or portable
Skill package, verify any `.agent/skills` path, validate `SKILL.md`, load a plugin, select an
implementation, construct a CapabilityInvocation, activate a Grant/PDP, authorize permission,
evaluate a rule/gate, route work, execute a capability, persist state, advance a transition or
attest an effect.

The projection authenticates neither source, author, owner nor repository state. SHA-256 gives
content identity, not signature, provenance, currentness or truth. Exact coverage means only
that the two supplied planning declarations agree under this profile; it does not prove that
the 140 capabilities are complete for engineering, that any Skill is production-ready, or
that a host supports it.

Revisit with a new version before accepting other YAML syntax or source shapes, changing any
field or digest preimage, resolving logical references physically, generating actual Skill
packages or host adapters, importing bindings into a Registry, or connecting the projection to
CapabilityInvocation, Grant/PDP, runtime routing, persistence, transition or effect authority.
