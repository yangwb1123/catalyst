# harness — ForgeOS out-of-band enforcement

Host-independent "load-bearing wall": the harness verifies structural
constraints and governance integrity **outside** any generated app, so it is
the source of truth even when the app is broken. It is intentionally polyglot.

## Tools

| Command | Runtime | Purpose |
| --- | --- | --- |
| `node harness/gate.mjs` | Node.js | Structural gate: per-file line cap (code files) + root file count. Reads `harness/policies.yml`. |
| `python3 harness/check.py [repo_root]` | Python 3 + **PyYAML** | Governance integrity (`forge check`): validates `.agent/` YAML, references, routing/modes, acceptance, and the Agent Engineering registries. |
| `python3 harness/agent_engineering_check.py [repo_root] [evidence-package.yml]` | Python 3 + **PyYAML** | Validates v1 activation/discipline/rule/detector/context/workflow contracts and, optionally, one source-bound evidence package. It never mints completion. |
| `python3 harness/backend_decision_check.py [repo_root] [backend-package.yml]` | Python 3 + **PyYAML** | Validates the byte-pinned backend policy/Skill/schema contract and, optionally, one BackendDecisionPackage whose bounded repository evidence is digest-, proof-type-, class- and subject-checked. Producer/reviewer identity and whole-tree/context provenance remain declarative; it is shadow-only and never mints approval or completion. |
| `python3 harness/frontend_design_check.py [repo_root] [frontend-package.yml]` | Python 3 + **PyYAML** | Validates the byte-pinned AFDS policy/Profile/Pattern/Skill/schema contract and, optionally, one FrontendDesignPackage with flow/state/action, exact-subject proof, bounded artifact/PNG checks, a `business_ui_composition`, and optional `geometry_measurement_receipts`. Composition/report validation is declarative: it neither runs a browser/native client nor mints visual-quality, approval or completion claims. |
| `python3 -B harness/approval_record_contract_check.py --golden [repo_root]` | Python 3 | Validates the ADR-0059 ApprovalRecord golden bytes/digests and declared assessment only. Instance mode accepts explicit request/assessment files; neither mode imports local markers or grants effective approval/authorization. |
| `python3 -B harness/transition_receipt_contract_check.py --golden [repo_root]` | Python 3 | Validates the accepted ADR-0060 TransitionReceipt contract-only golden bytes/digests and declared assessment only. Instance mode accepts explicit request/assessment files; neither mode imports workflow/ledger state nor grants transition authority. |
| `python3 -B harness/knowledge_update_proposal_contract_check.py --golden [repo_root]` | Python 3 | Validates the accepted ADR-0061 KnowledgeUpdateProposal contract-only record closure, five golden digests, and declared assessment. Instance mode never reads a current knowledge head, persists/applies a mutation, or grants truth/adoption/authority. |
| `python3 -B harness/local_go_package_impact_prescan_contract_check.py --golden [repo_root] \| <envelope.json>` | Python 3 | Strictly re-derives the ADR-0062 golden wrapper or validates one bounded pure local Go lexical package impact-prescan envelope. It reads no live repository state, runs no producer/tool, leaves system impact unknown, and grants no full Impact Closure, authority, completion, persistence, execution, or effect claim. |
| `node harness/acceptance.mjs` | Node.js | Formal full acceptance: every probe runs live against one pre/post-bound source candidate. Static and independent workload probes use bounded OS-process parallelism; candidate drift still fails closed. |
| `node harness/acceptance.mjs --json` | Node.js | The same live schedule as strict structured input for `forge accept`; cache replay is physically disallowed. |
| `node harness/acceptance.mjs --cache` | Node.js | Explicit advisory acceleration: after a live warm-up, source-only scanners replay from source/tool/environment fingerprints. Execution probes and SCA remain live because installed dependency trees or advisory DBs may be outside that fingerprint. Hits are labelled, never print `ACCEPTED`, and always exit `2`, so they cannot satisfy CI. |
| `node harness/select-tests.mjs [changed paths...]` | Node.js | Changed-file incremental advisory signal; unmapped paths explicitly require the full gate. |

## Dependencies

- **`node harness/gate.mjs`** — Node.js only (zero npm deps).
- **Rust/Java acceptance adapters** — Node.js built-ins only. Project tools remain
  ecosystem-native: Cargo for Rust; repository-local `mvnw`/`gradlew` for Java.
  Missing tools are reported `N-A/no_tool` (production-blocking), never PASS.
- **Acceptance parallelism** — Node supervises independent OS processes. Static
  and workload probes overlap in a bounded pool; recursive Node suites run at
  four test files at once, require a canonical positive TAP count with zero
  skipped tests, and coverage owns an artifact
  transaction while Go/Rust/Node/Python outputs stay in generated locations.
  Tune top-level fan-out with `FORGE_ACCEPT_CONCURRENCY` (default `4`), worker
  and schedule deadlines with `FORGE_ACCEPT_WORKER_TIMEOUT_MS` and
  `FORGE_ACCEPT_TOTAL_TIMEOUT_MS`, and Python coverage shards with
  `FORGE_COVERAGE_WORKERS` (default `8`). On Linux, trusted util-linux
  `/usr/bin/unshare` starts each worker in a private user/PID/mount namespace;
  containment executables are ownership/mode checked and launched through pinned
  descriptors. The coordinator must run in the initial user namespace because
  overflow UIDs cannot prove host-root ownership; nested formal launch fails
  closed. A trusted `/bin/sh` PID 1 waits for the worker and reaps adopted children.
  Inability to create that containment fails the worker closed. A detached IPC
  guardian reaps the outer namespace launcher if the coordinator disappears.
  Non-Linux hosts fail formal workers before spawn: private process groups and
  `taskkill` cannot provide a birth-identity-bound kill-on-close guarantee.
- **Formal candidate binding** — descriptor-relative double hashing binds each
  checkpoint, while an independent raw-inotify journal watches every ancestor
  and source directory across the complete live-probe interval, rejecting even
  write-and-restore (ABA) drift and path rebinding. Source
  hardlinks are refused because an outside alias could evade a directory event
  journal. The journal uses a descriptor-bound private `.forge` barrier; if the
  host cannot provide safe descriptors, initial-namespace executable trust, or
  an allowlisted local filesystem, formal rows fail closed instead of claiming
  a single candidate. This boundary covers ordinary inotify-visible operations,
  not adversarial `mmap` writes, privileged mount changes, or remote filesystems.
- **Advisory cache** — stored only at `.forge/acceptance-cache.json` under a
  private directory/file. Cacheable source-only scanner rows fingerprint visible
  source inputs, runtime/tool/package identities, and the complete effective
  environment before and after live work. Execution rows stay live because
  installed dependency/build caches are not completely bound. Symlinks, drift,
  or unsafe file shapes fail closed;
  cached FAIL/N-A rows retain their status and are visibly advisory. SCA stays
  live when its advisory DB is external. Durable descriptor-anchored storage is
  Linux-only; other hosts fall back live. Formal, JSON, CI, and imported
  `collect()` paths stay live. Changed-candidate selection remains the separate
  `select-tests.mjs` advisory.
- **`python3 harness/check.py`** — Python 3 + **PyYAML** (`pip install pyyaml`).
  PyYAML is the sole third-party requirement; if it is missing the tool exits
  `2` with an actionable message rather than crashing.

## Exit codes

- `gate.mjs`: `0` pass · `1` block (when `enforce: block`) · `2` cannot read policies.
- `check.py`: `0` PASS · `1` FAIL (lists issues) · `2` PyYAML unavailable.

## Tests

```sh
node --test harness/test_gate.mjs     # Node gate pure-function + import-safety tests
python3 harness/test_check.py         # governance checker tests (stdlib unittest only)
python3 -m unittest harness.test_agent_engineering_check
python3 -m unittest harness.test_backend_decision_check
python3 -m unittest harness.test_frontend_design_check
python3 -m unittest harness.test_frontend_business_ui_geometry
python3 -m unittest harness.test_frontend_business_ui_composition_boundaries
```

## Conventions

- Workflow phases name a descriptive role-stage in `name:` but their `agent:`
  field must reference a **canonical role-card stem** directly
  (`.agent/agents/<stem>.md`) or the `harness` pseudo-agent. `check.py` enforces
  this as a pure membership check — no alias indirection — so workflow/card
  drift cannot be silently frozen into the checker.
- Agent Engineering v1 is `shadow`: its schemas and references are enforced,
  while context selection and W0-W3 routing remain policy-only. `forge accept`
  stays the sole completion authority.
- Business UI Geometry uses the same shadow AFDS package and existing capability
  owners. `harness/frontend_design/composition.py` checks bounded composition
  structure and references; `harness/frontend_design/geometry.py` checks declared
  report bindings and observations. Neither helper is a browser/native runner or
  trusted provenance service, and fail/inconclusive/not-executed observations
  cannot be converted into a positive verdict by an advisory score.
