# Agent Engineering governance

This directory is the active, machine-validated entry point for ForgeOS Agent
engineering policy. It deliberately does not duplicate the lifecycle DAG or the
planning-only AI Engineering OS catalog.

- `disciplines.yml` records the 14 engineering disciplines and their honest
  implementation state.
- `activation.yml` supplies the versioned shadow default, including for legacy
  projects whose identity config predates Agent Engineering v1.
- `rules.yml` separates automatic red lines from review heuristics and planned
  controls; the four automatic v1 red lines have canonical semantic digests.
- `detectors.yml` binds automatic rules only to load-bearing `forge accept`
  probes with exact commands, exit-code verdict propagation, and positive and
  negative tests.
- `context-routes.yml` declares typed, bounded, deterministic context selection.
- `../skills/context-engineering.md` and `docs/contracts/context-package-v1.schema.json`
  define the authority-free pure ContextPackage assembler. It records exact selected,
  omitted, redacted and truncated inputs under byte/snippet/token budgets and a
  content-addressed cache key; all instruction candidates remain
  `instruction_allowed:false`, and the package is not wired into provider prompts.
- `../skills/policy-authority.md` and `docs/contracts/capability-grant-v1.schema.json`
  define only the strict CapabilityGrant wire and authority-neutral declared-relation
  assessment. The shadow detector is non-load-bearing: it authenticates no issuer,
  validates no Approval/revocation/usage state, and grants no authorization,
  permission, pre/postflight receipt, persistence, execution or effect authority.
- ADR-0059 delivers the contract-only `ApprovalRecord v1` wire, detached-proof
  identity, caller-time declared assessment and exact CapabilityGrant ApprovalRef
  projection. It never imports local approval markers, flags or actor hints, and
  authenticates no approver, authority, proof or SoD. Condition, RiskAcceptance,
  revocation, effective approval, authorization, persistence and effects remain
  unavailable; the accepted slice keeps its shadow detector non-load-bearing and
  grants no runtime authority.
- ADR-0060 delivers the accepted contract-only `TransitionReceipt v1`. It freezes the
  declared state graph, receipt/predecessor/recovery relations and exact Grant/
  Approval reference comparisons, but authenticates no controller, current state,
  precondition, waiver, Grant or Approval. Its detector remains shadow and it does
  not append a ledger, persist or perform a transition, completion or effect;
  acceptance does not grant runtime authority.
- ADR-0057 adds one Catalyst-only runtime profile,
  `authenticated_bootstrap_repo_read_grant_issuance_v1`. An operator-deployed,
  non-Agent `forge-kernel` authenticates only externally pinned root, signed policy
  and signed request inputs, and may issue only bootstrap `repository-reader/v1`
  `repo.read` exact-path Grants plus durable signed receipts for local/development/test.
  File mode/effective-UID checks are not OS-principal or HSM isolation. The portable
  checker remains shadow; scaffold/upgrade installs no kernel, root, key or state and
  returns `not_executed` without a compatible externally deployed runtime.
- `workflow-profiles.yml` maps L0-L4 materiality to W0-W3 increasing governance
  profiles while reusing the existing workflows.
- `../eval/completion-evidence.schema.yml` defines a source-bound task evidence
  package. It deliberately has no completion or acceptance decision field.
- `backend-decision-gates.yml` defines the per-trigger L1–L4/W1–W3 backend decision sequence,
  conditional model-role separation, 14 review dimensions and production-readiness
  vocabulary. `../eval/backend-decision-package.schema.yml` defines its bounded,
  file-resolved and proof-type/class/subject-checked artifact without granting approval
  or completion authority.
- `frontend-design-gates.yml` and `frontend-profiles.yml` define the shadow AFDS
  decision sequence, risk floors, product Profiles, page Patterns, operation/state/action
  contracts, and evidence honesty boundaries. `../eval/frontend-design-package.schema.yml`
  keeps artifacts separate from proof claims and binds render evidence to a declared
  capture context without claiming authenticated tool or reviewer provenance.
- `../skills/ui-geometry.md` is a conditional supporting procedural adapter, not a
  fourth capability owner. The existing AFDS package can bind a strict
  `business_ui_composition` source artifact and optional
  `geometry_measurement_receipts`; those records express business-to-layout
  relationships and declared runner observations, not authenticated browser/native
  execution or a visual-quality verdict.
- `governance-contracts.yml` and `../skills/evidence-claim-management.md` define
  strict EvidenceRecord/KnowledgeClaim identity, canonical bytes, state matrices and
  shadow admissibility plus a narrow local exact-record append journal. The journal's
  1,024-record/16,777,216-byte/256-depth closure bounds are resource-exhaustion
  admissibility limits. Its v1 semantic view always evaluates the current structural
  tail at explicit caller time, uses exact-v27 live `mode=ro`/`query_only` access and
  one Deferred snapshot, and never selects a historical tail. Per-view history,
  reference closure and complete owning batches share 1,024 records/16 MiB; multi-head
  verification additionally shares 65,536 records/256 MiB/1,000,000 work units.
  SQLite may coordinate SHM locks or create/remove empty WAL/SHM sidecars, so this is
  logically Hub-read-only rather than filesystem-effect-free. Scaffold copies these
  governance assets but no Rust `forge-runtime`; unavailable or incompatible runtime
  execution is `not_executed`. `stored`, `exact_replay`, structural heads and semantic
  projections never become truth, freshness, conflict resolution, approval, completion
  or knowledge writeback.
- The shipped ADR-0065/0066 slices and `../skills/knowledge-graph-curation.md`
  add two explicitly negotiated authority-free GraphSnapshot profiles over
  caller-supplied exact ADR-0053 bytes. The second adds package-scoped lexical
  test source-set nodes and module-to-test edges without changing the first
  profile or golden. Go/test coverage stays disjoint and `partial`, while
  system/freshness stay `unknown`; source presence is not test execution,
  result, coverage, verification, G3, Assessment Join, impact or authority.
- ADR-0067 Proposed-only ADR v2 adds `../skills/adr-governance.md`, a closed
  document Schema, physical golden and strict checker. It validates only one
  explicit new Proposed document (or the pinned golden); `writes_adr` applies
  the same contract only to its current unique candidate. Declared owners,
  approvers, Claim/Evidence and Graph-node refs are not authenticated or
  resolved. Legacy v2 parsing, retro-validation and migration remain
  unavailable; Go `writes_adr` still scans and hashes legacy ADR bytes solely
  for its existing baseline-integrity fingerprint. ApprovalRecord, acceptance,
  immutable supersession/compliance, Graph coverage, persistence and lifecycle
  authority remain unavailable.
- ADR-0068 adds `../skills/capability-registry.md` and an authority-neutral,
  staged singleton Capability Registry v1. Go/Python validators and resolvers,
  an explicit-input CLI, physical checker and cross-language golden bind only
  `local-go-package-impact-prescan/1`. They do not read the planning catalog,
  generate catalog-to-package adapters, authenticate ownership, activate
  Grant/PDP, construct CapabilityInvocation, select or execute an
  implementation, load plugins, or perform runtime routing.
- ADR-0069 adds `../skills/capability-ownership-projection.md` and a delivered
  planning-only pure projection over two caller-supplied exact YAML sources.
  Python/Go and one exact golden prove 140 unique fine capabilities each have
  exactly one of 38 declared primary owners while retaining 145 occurrences;
  derived `.agent/skills/*.md` refs stay unresolved. The source-only scaffold
  generates no owner Skill/host adapter, and the projection neither mutates
  ADR-0068 nor supplies authentication, Grant/PDP, invocation or routing authority.
- ADR-0070 adds `../skills/project-snapshot.md` and a source-distributed
  `../../skills/project-snapshot/` package. Its Linux-only Catalyst producer captures
  two exact bounded worktree endpoints under a fixed pre-read path policy; the
  pure decoder is source-portable, while live capture remains Linux-only; package
  sources can be copied without copying or installing the Go runtime.
  Atomicity/currentness/completeness and secret absence stay unproved, Git/HEAD stay
  unauthenticated, and scaffold presence never grants installation, permission,
  Graph/Registry/Grant/PDP/Invocation, persistence, routing or effect authority.
- ADR-0071 adds a closed `../../skills/context-engineering/` source package around
  the unchanged ADR-0055 pure ContextPackage builder. Its zero-argument adapter
  consumes exact canonical stdin bytes; the separate checker accepts only an
  optional explicit package root for scaffold verification. Neither discovers
  sources, calls a provider/model, compiles a live prompt, installs a host Skill,
  authenticates a publisher, binds check-to-use atomically, or supplies
  Grant/PDP/Approval, truth, instruction, completion, persistence, routing or effect
  authority. Python `-I` excludes script/current directory, `PYTHONPATH` and user
  site, but does not isolate system site, stdlib, interpreter startup or the host.
- ADR-0072 adds a closed `../../skills/evidence-claim-management/` source package
  around the unchanged ADR-0045 pure validator. Its zero-argument adapter consumes
  already-authored exact canonical record-set bytes through explicit EOF; the
  separate checker accepts zero or one package-root argument. Registry v27 keeps
  this validation-only delivery outside authenticated routes. Source-only scaffold
  does not install a host Skill, observe or author records, repair or persist data,
  access journal/semantic-view/proposal state, or grant truth, instruction,
  Grant/PDP/Approval, completion, routing, transition, execution or effect authority.
- ADR-0073 adds a closed `../../skills/policy-authority/` source package around
  the unchanged ADR-0056 and ADR-0059 pure declaration evaluators. Two independent
  zero-argument adapters consume exact canonical requests through explicit EOF;
  the separate checker accepts zero or one package-root argument. Registry v28
  keeps this source delivery outside authenticated routes and runtime scope. It
  adds no combined envelope, issuance, effective Approval, policy/PDP/PEP,
  ADR-0057/0058 runtime, persistence, transition, execution or effect authority.
  Source-only fresh and legacy scaffold now copies the sealed source package and
  governance checks; it installs no host Skill or runtime and grants no authority.
- ADR-0074 adds a closed `../../skills/adr-governance/` source package around
  the unchanged ADR-0067 Proposed-document validator. The validator accepts
  exactly one basename argument plus exact stdin through explicit EOF; that
  caller lexical label preserves filename-string equality but proves no physical
  file or repository identity. Registry v29 keeps runtime scope unchanged and
  the portable Skill outside authenticated routes. Source-only fresh and legacy
  scaffold copies source without a host Skill or Go `writes_adr` runtime; no
  authoring, repair, acceptance, compliance, lifecycle, persistence or authority
  is delivered.

The semantic read surface is explicit and caller-time-bound:

```text
forge-runtime governance journal view KIND AGGREGATE_ID --as-of-unix-ms N
forge-runtime governance journal conflicts --as-of-unix-ms N [--limit N]
forge-runtime governance journal validation-jobs --as-of-unix-ms N [--due-only] [--limit N]
```

`python3 -B harness/check.py` validates these contracts. The validator proves
schema integrity, canonical capability ownership, real detector wiring, closed
context/trust/budget semantics, bounded profile autonomy and evidence-package
structure. `python3 -B harness/backend_decision_check.py . <package.yml>` can
validate one backend package; `python3 -B harness/frontend_design_check.py . <package.yml>`
can validate one frontend package, including bounded composition/report structure,
cross-references and digest bindings. Both detectors remain shadow and non-load-bearing.
`python3 -B harness/governance_contract_check.py . <canonical-record-set.json>` validates
one exact compact Evidence/Claim set and returns only an authority-free structural result.
A separate bounded integration checker,
`python3 -B harness/governance_engineering/semantic_view.py .`, validates the canonical
semantic registry, Schema structure, golden binding and Skill markers; it is not a
generic arbitrary-output instance validator.
A journal append separately preserves already-valid exact bytes; metadata-only reads and
`structural_sequence_only` heads do not reinterpret record semantics.
A structured receipt and declared producer/reviewer identity are still not cryptographic proof that a command ran or a distinct principal reviewed it;
`forge accept` remains the only completion authority. The validator does not
claim that the future AADM solver, context runtime, complete capability runtime,
knowledge graph or Device Fabric exists.

ADR-0075 adds a closed `../../skills/knowledge-graph-curation/` source package around the two unchanged ADR-0065/0066 exact-request partial projectors. Registry v30 adds only source-delivery metadata and the package checker remains shadow/non-load-bearing; neither projector is a detector or authenticated route, and no live capture, persistence, impact or authority is added.

ADR-0076 adds a closed `../../skills/change-impact-cost-risk/` source package around the unchanged ADR-0062 exact-request lexical ImpactPreScan projector. Registry v31 adds only source-delivery metadata and the package checker remains shadow/non-load-bearing; the projector is neither a detector nor an authenticated route, system impact remains UNKNOWN, and no live capture, complete Impact/Cost/Risk, persistence or authority is added.

ADR-0078 adds only Registry v32 Proposed candidate metadata and a checker-only shadow for the unchanged ADR-0077 WorkIntent v1 Python/Go/Rust parity evidence. Source distribution is Python-only, Go and Rust remain Catalyst-only, runtime scope and context routes remain unchanged, and validity adds no acceptance, semantic authority, G0 closure, lifecycle, permission, persistence or effect.

ADR-0080 adds only Registry v33 Proposed candidate metadata and one checker-only shadow for the unchanged ADR-0079 Authenticated Architecture Decision Approval v1 structural prerequisite. Distribution copies only dependency-free Python validation sources and exact artifacts, never a future Go service, production keys or state. Scope and routes remain unchanged; structural success adds no Ed25519 verification, authentication, authorization, receipt issuance, root/time/revocation currentness, CAS, durability, Accepted lifecycle, G0 closure, persistence or effect.

ADR-0083 adds Registry v34 split evidence for ADR-0081 Catalyst-only Go approval authority and the ADR-0082 Python lifecycle structural candidate. It preserves the full scope hash, wires one lifecycle checker-only shadow and distributes no Go authority, production key or state. Structural validity and `forge accept` are not lifecycle acceptance, source mutation, compliance, permission or effect authority.

ADR-0085 adds Registry v35 Catalyst-only ADR-0084 lifecycle authority evidence and an exact source-only governance distribution. The exact44 Go authority aggregate is audited only in Catalyst; generated projects receive no Go authority, trust material or state and add no route, Skill, service or runtime profile.

ADR-0087 adds Registry v36 candidate governance for the ADR-0086 unverified legacy read-only import. Its shadow detector records only the checker's real zero-argument argv; an operator supplies request bytes on stdin and closes EOF. Python source is distributable, while the Catalyst-only Go parity package, routes, Skills, runtime, state and authority are excluded.

ADR-0089 advances the active policy to Registry v37 without changing its scope digest. It registers ADR-0088 only as an operational-reference structural subclosure, runs the exact pinned-golden checker argv and source-distributes Python only. Go/Rust parity, runtime registration, authority, DecisionTransaction and the complete Kernel ABI remain outside the delivered boundary.

ADR-0090/0091's repository slice is delivered in Registry v38 without changing the complete scope mapping or digest, while both ADRs remain Proposed. It registers only a structural reference family across CognitiveAtom v2, DecisionTransaction v1 and one-way operational references. The checker-only shadow runs the exact pinned-golden argv; exact19 distribution copies dependency-free Python source/governance only, while Catalyst exact13 Go, flat exact9 Rust and shared `lib.rs` registration stay repository-only. All 22 attestations are false and no Skill, route, runtime, PDP or controller is added. The narrow roadmap item passed formal `forge accept` and is complete; ADR-0038 remains ADOPTED-PARTIAL, and DecisionCapsule, AuthorizedTransactionSpec, authenticated PDP and the rolling controller remain open.

ADR-0092/0093 in Registry v39 record only a pending Decision Capsule structural replay repository Candidate. The complete scope mapping and digest stay unchanged; the checker-only shadow runs the exact pinned golden, and exact19 source distribution copies dependency-free Python core/governance only. Catalyst exact15 Go, exact14 Rust and shared `lib.rs` registration remain repository-only. All 32 attestations, both replay controls and all seven completion claims are false; no Skill, route, runtime, Reflection consumer, persistence, PDP or controller is added. Both ADRs always remain Proposed/null, and the narrow roadmap item stays unchecked until independent review and formal `forge accept`; ADR-0038, full DecisionCapsule and AuthorizedTransactionSpec remain open.
