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
claim that the future AADM solver, context runtime, capability runtime,
knowledge graph or Device Fabric exists.
