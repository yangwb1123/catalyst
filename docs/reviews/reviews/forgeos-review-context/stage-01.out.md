All evidence gathered. Here is the Stage 01 architecture review.

---

# Stage 01 — Architecture Review: Graph Scheduled Successor Protocol (Wave-Parallel Planning)

**Stage 00 output:** N/A (not pasted). Reviewed against the "Proposed architecture" bullets and verified ADR-0031/0035, current code, and tests.

**Roles:** Principal Architect · Backend Architect · CTO

**Validation run:** `node harness/acceptance.mjs` → **ACCEPTED** (9 pass · 0 fail · 2 honest N/A); Go 1330 tests, Rust 556 tests, clippy/arch-check 8/8 clean. Gates are green, but **no test admits a second successor candidate into one run** — the top finding below is established by code trace (observed, not inferred from a failing run).

---

## 1. Findings

### 🔴 Finding 1 — High
| Field | Content |
|---|---|
| **Title** | The v20 "one candidate per node per run" invariant is not enforced; admission still rejects every candidate after the first per run, and the v18/v16 one-per-run request/lifecycle walls make the successor chain deadlock after one node |
| **Surface** | Nested module (Rust infrastructure store + schema) |
| **Location** | `forge-runtime/.../group_agent_scheduled_node_successor/write.rs:108-133` (`reject_existing_candidate_identity`: `find_all_by_run(...).next()` at 122, `find_by_schedule` at 128); `schema_contract/v18_sql.rs:5,9` (`graph_run_id`/`schedule_id` UNIQUE on the provider-request table); `schema_contract/v16_sql.rs:5` (`graph_run_id` UNIQUE on the lifecycle table); `schema_contract/v20_sql.rs:99-100` |
| **Evidence** | The v20 migration deliberately *removed* the one-per-Run UNIQUE and Sprint 66 declares "one candidate per node per run" — but `admit_locked` still runs `reject_existing_candidate_identity` whose "Graph Run" and "schedule" entries return a conflict for **any** pre-existing candidate row (`find_all_by_run(transaction, &request.graph_run_id)?.into_iter().next()`). The comment at write.rs:117-119 ("v20: one candidate per node… any candidate proves the Graph Run is not pristine") is self-contradictory: the candidate sidecar provably does *not* touch the Run journal (v20 CHECK `expected_last_event_seq = 1`, pristine v1/seq-1 preserved). Additionally, the provider-request table keeps `UNIQUE(graph_run_id)` + `UNIQUE(schedule_id)` (v18) and the dispatch-lifecycle table keeps `UNIQUE(graph_run_id)` (v16) — so after node N's request/lifecycle row exists, node N+1's row can never be inserted. No test in the repo admits two candidates/requests into one run (verified by grep across infrastructure/application/interfaces tests); every ordinal-N test prepares a **fresh** run with a single candidate. |
| **Failure scenario** | Serial chain: admit ordinal-0 candidate (v14) → dispatch → terminalize → admit ordinal-1 candidate (first v20 row, OK) → `provider-request prepare` → **violates v18 `UNIQUE(graph_run_id)`** → chain stops at node 1. Wave case: diamond with frontend+backend both ready → `wave-admit` admits the first, reports the second as "rejected: … already belongs to another idempotency key" (the `rejected` array in `wave_command.rs` absorbs it silently). |
| **Impact and likelihood** | The declared capability ("v20 store keeps one candidate per node per run"; wave-admit "对每个节点…生成候选并 admit") cannot be exercised beyond one candidate. All failure modes are **fail-closed** (clean conflict errors, no corruption, no authority leak), so this is a functional/compatibility defect of the delivered slice, not a security hole. Likelihood: 100% on the second admit; the current diamond fixture masks it because each receipt set yields exactly one ready node. |
| **Fix** | (a) In `reject_existing_candidate_identity`, drop the "Graph Run" and "schedule" one-per checks; keep the per-node slot (`find_by_run_node_attempt`) and per-ordinal slot (`find_by_schedule_ordinal_attempt`); correct the comment. (b) Plan a v21 migration (same data-preserving rebuild pattern as v19→v20) keying provider-request and dispatch-lifecycle rows per `(graph_run_id, node_id, attempt)` instead of per-run. (c) Add store tests (two candidates, same run, different nodes) and a wave-admit E2E with two ready nodes. |
| **Risk/effort** | (a) low risk, ~1 day incl. tests. (b) medium — touches the effectful dispatch fence (ADR-0030/0032 semantics must be restated per-node); do not fold into this slice if effectful multi-node dispatch stays deferred, but the v20 candidate admission fix is required **now** for the declared planning slice. |

### 🟡 Finding 2 — Medium
| Field | Content |
|---|---|
| **Title** | ADR-0035's "`predecessor_terminal_receipts` are exactly the receipts of the candidate's direct predecessors" is not implemented: Go carries the full consumed set and a wave-sibling candidate with an empty predecessor set is unrepresentable |
| **Surface** | Nested module (Go core selector + Rust domain validation) |
| **Location** | `forge-core/internal/graphscheduledcontract/build_successor.go` — `verifyReceipts` (179) → `successorCandidate` → `buildSuccessorRequest` passes the **full** consumed receipt list into `PredecessorTerminalReceipts`; `validate.go:76` requires `len(PredecessorTerminalReceipts) > 0` for successor scope; `build_successor_test.go` (`TestBuildSuccessorSelectsOrdinalOne`) locks len==1 for the diamond's backend (which has **0** direct predecessors); Rust `scheduled_contract_admission_validation.rs:129-133` (`validate_successor_node`) checks only subset coverage, so supersets pass |
| **Evidence** | Diamond fixture (`group-agent-graph-execution-schedule-v1.json`): backend(ordinal 1, wave 0, `direct_predecessor_node_ids: []`). After frontend's receipt, `selectReadyNode` picks backend, but the candidate embeds frontend's receipt — the exact case ADR-0035 says must *not* be carried ("other consumed receipts are evidence of progress but are not carried by this candidate's request"). The v20 table CHECK `predecessor_receipt_count BETWEEN 1 AND 31` encodes the same contradiction. |
| **Failure scenario** | Any wave where a sibling has an empty direct-predecessor set: its canonical candidate (zero receipts) cannot be built or admitted; the system silently substitutes the whole consumed prefix as "evidence". Downstream consent/disclosure (ADR-0033) and digest goldens are built on the wrong shape. |
| **Impact and likelihood** | Contract drift between a published ADR and a fail-closed protocol; the ADR's "empty predecessor set" wave-parallel claim is not materializable. No security impact (receipts are durable-verified before admission). Likelihood: guaranteed in diamond topologies. |
| **Fix** | Either (a) filter `PredecessorTerminalReceipts` to `node.DirectPredecessorNodeIDs` in `buildSuccessorRequest`, relax the `>0` requirement to "≥0 with coverage", and update the v20 CHECK + goldens; or (b) amend ADR-0035 to "superset evidence" semantics and update its wording. Prefer (a) — it matches the ADR and tightens the evidence binding. Add a golden for the empty-preds wave-sibling candidate. |
| **Risk/effort** | (a) low-medium (digest/golden churn is contained to the successor slice); (b) trivial but perpetuates the drift. |

### 🟡 Finding 3 — Medium
| Field | Content |
|---|---|
| **Title** | Stale protocol documentation in the fail-closed successor package contradicts the implemented ADR-0035 semantics |
| **Surface** | Nested module (Go core) |
| **Location** | `build_successor.go:8-18` (`BuildSuccessor` docstring: "verified contiguous prefix", "receipts must be ordered by execution ordinal"); `types.go:2-4` (package doc: "cannot select a successor"); `command.go:98` (`readPredecessorReceipts`: "in ordinal order") |
| **Evidence** | `verifyReceipts`/`selectReadyNode` implement ADR-0035 topological selection (any order, no prefix requirement); `BuildSuccessor` and `ReadySuccessorNodes` are successor selection — the package doc's "cannot select a successor" is false since Sprint 66. |
| **Failure scenario** | A future maintainer relies on the docstring to assume serial-prefix ordering, or the docs drift further from the Rust admission's actual subset semantics. |
| **Impact and likelihood** | Maintainability/honesty defect in a security-sensitive protocol; the docstring describes a *stricter* rule than enforced, which reads as a false safety claim. Certain. |
| **Fix** | Rewrite the three comments to state topological-ready selection and the evidence-chain binding. One-line doc edits + review. |
| **Risk/effort** | None/trivial. |

### 🟢 Finding 4 — Low
| Field | Content |
|---|---|
| **Title** | Dead code: `successorPredecessorsCovered` is never called |
| **Surface** | Nested module (Go core) |
| **Location** | `build_successor.go:230-244` |
| **Evidence** | `grep -rn successorPredecessorsCovered` → definition only, no callers in production or tests; the same semantics exist as `predecessorsCoverReceipts` (`validate.go:90`). |
| **Impact** | Dead code in a fail-closed protocol invites confusion; the two helpers can drift. |
| **Fix** | Delete it (its logic is covered by `predecessorsCoverReceipts` + tests). |

### 🟢 Finding 5 — Low
| Field | Content |
|---|---|
| **Title** | `ReadySuccessorNodes` returns `errInvalidCandidate` for an empty ready list, conflating "graph complete" with "invalid input" |
| **Surface** | Nested module (Go core CLI) |
| **Location** | `build_successor.go` (`ReadySuccessorNodes`: `if len(ready) == 0 { return nil, errInvalidCandidate }`); `ready_command.go` maps it to exit 1 "cannot compute ready successor nodes" |
| **Evidence** | Consuming all nodes and asking for the next wave is a legitimate terminal state of a finished graph; the CLI reports it as a planning fault. `verifyReceipts` also rejects `len(receipts) >= len(schedule.Nodes)`. |
| **Impact** | Operator-facing honesty wart: a completed graph's planning view is indistinguishable from a corrupt request. |
| **Fix** | Return a distinct empty-list result (or a dedicated "graph complete" sentinel) and let the CLI print `[]` with exit 0. Add a full-consumption test. |

### 🟢 Finding 6 — Low
| Field | Content |
|---|---|
| **Title** | Sandbox wiring side effects and brittle error classification in the orchestrator |
| **Surface** | Stock binary (forge-core orchestrator) |
| **Location** | `sandbox_config.go` — `sandboxConfigError` (value receiver) writes `c.Sandbox.Runner = runner` through the caller's `*SandboxConfig`; `executeSandboxed` classifies via `strings.Contains(err.Error(), "timed out")` and maps parent cancellation (`runCtx.Err() != nil`) to `KindTimeout` |
| **Evidence** | Value-receiver method mutating pointed-to shared config is surprising but idempotent (auto-wire runs once). The string match is a second, redundant signal behind `runCtx.Err()`, so classification is correct in practice; parent-cancel→KindTimeout conflates cancellation with timeout (the host path treats these distinctly via `finish`). |
| **Impact** | Maintainability; no security or correctness defect observed in current flows (documented honest tests skip live daemon/KVM when absent). |
| **Fix** | (Optional) hoist auto-wiring to construction time or make `Runner` resolution explicit; prefer typed sentinel errors over string matching. |

---

## 2. Proposed ADR

**Title:** ADR-0036 — Per-node successor candidate, request, and lifecycle slots with evidence-chain progression

**Status:** Proposed (blocks the wave-parallel planning slice as declared)

**Context:** ADR-0035 relaxed Go's successor selection to topologically-ready (wave-parallel base), and Sprint 66 rebuilt the v20 successor-candidates table to per-node slots (`UNIQUE(graph_run_id, node_id, attempt)`), removing the one-per-Run UNIQUE. However, the Rust admission store still rejects any second candidate per run (`reject_existing_candidate_identity`: "Graph Run"/"schedule" conflict), and the v18 provider-request and v16 dispatch-lifecycle tables still carry `UNIQUE(graph_run_id)`. The result: a run can hold exactly one candidate, one request, and one lifecycle, at any ordinal — the successor **chain** (ordinal 0 → 1 → 2) and same-wave sibling candidates are structurally impossible, although every sprint since 60 describes them.

**Decision:**
1. Successor admission allows one candidate per `(graph_run_id, node_id, attempt)` — drop the "Graph Run" and "schedule" one-per checks; keep per-node and per-ordinal slot conflicts and the idempotency-key replay path unchanged.
2. Provider-request (v18) and dispatch-lifecycle (v16/v19) tables are rebuilt in a v21 migration keying rows per `(graph_run_id, node_id, attempt)` (data-preserving rebuild, same pattern as v19→v20). The global active-lane index (one active lane per `project_lane_sha256`) is unchanged — wave-parallel *planning* may produce N candidates, but *execution* remains serial per project lane until the orchestration slice is designed.
3. The candidate's `predecessor_terminal_receipts` are exactly its direct predecessors' receipts (ADR-0035 as written); consumed-but-unrelated receipts remain progress evidence only.
4. Admission order for same-wave siblings is unconstrained (topological readiness, not ordinal prefix); every candidate still requires durable-terminalized receipts for its direct predecessors, byte-verified in the immediate transaction.

**Consequences:** Successor chain progression becomes implementable; wave-admit can materialize a full ready wave; digest domains and goldens for successor candidates change shape (receipt-set filtering); the v21 migration follows the established rebuild/rollback/downgrade-fixture pattern; the effectful dispatch fence (ADR-0030/0032) semantics must be restated per-node but its claim/lane/terminalize invariants are untouched.

**Risks:** v21 migration churn across downgrade fixtures and dispatch re-entry tests; per-node lifecycle rows weaken the "one lifecycle per run" invariant — the hard-crash adjudication (ADR-0034) must key on `(run, node, attempt)`; scope creep into the effectful slice — mitigated by splitting (a) candidate admission fix (this slice) from (b) v21 request/lifecycle slots (next effectful slice).

**Alternatives considered:**
- *A. One fresh Run per node* — each node gets its own graph run/control export. Rejected: destroys the single graph-run identity the schedule, journal, receipts, and lane protocol bind to; multiplies control exports; makes the "pristine v1/seq-1" invariant meaningless per node; contradicts the one-run-per-graph execution model fixed since ADR-0017/0018.
- *B. Single mutable candidate slot (latest-wins upsert)* — one candidate row replaced per advance. Rejected: breaks the immutable sidecar + idempotent-replay design that all admission tests and digest goldens depend on; a replaced candidate erases evidence of prior states and reintroduces ordering races the UNIQUE per-node slots were introduced to solve.
- *C. Keep one-per-run and call the chain "single-hop only"* — Rejected: contradicts ADR-0031's stated purpose ("successor contract candidate consuming verified predecessor receipts") and the sprint-declared deliverables; it would reduce the subsystem to a planning view with no execution path.

---

## 3. Package tree + import map

```
forge-core (Go core — scheduler owner; pure stdlib)
├── internal/graphscheduledcontract/      (11 prod files, 12 exports — new: build_successor.go, ready_command.go)
│    outgoing → graphdispatch · graphplan · graphschedule · scheduledterminal
│    incoming ← cmd/forge/cli_dispatch.go · graphscheduledrelease{types,validate_contract}.go
│    fan-in 3 prod importers (budget 30) · no cycles (arch-check 8/8 clean)
├── internal/orchestrator/                (15 prod files)
│    ├── sandbox_config.go → sandbox{Runner} · docker · firecracker
│    ├── sandbox/sandbox.go               (SPI: Runner, 19 lines)
│    ├── docker/docker_runner.go          (implements Runner structurally, 107 lines)
│    └── firecracker/firecracker_runner.go(implements Runner structurally, 450 lines ≤ 500)
│    incoming ← cmd/forge (7 files)
forge-runtime (Rust — store owner; layered: interfaces → application → domain, infrastructure outer)
├── crates/interfaces/…/wave_command.rs · scheduled_contract_command.rs
├── crates/application/…/scheduled_successor_service.rs
├── crates/domain/…/scheduled_contract_admission_validation.rs
└── crates/infrastructure/…/sqlite_hub/
     ├── group_agent_scheduled_node_successor/{write,read,rows}.rs
     └── schema_contract/v20_sql.rs
```

**Cycles:** none (arch-check circular-dependency check = 0; manual trace confirms `graphscheduledcontract` is a leaf consumer). **Boundary verdict:** the Go/Rust split (Go builds canonical artifacts, Rust admits/persists) matches the established handshake pattern ("Go 仍是唯一调度 owner"); the `sandbox.Runner` port is minimal and correctly placed (imported only by the orchestrator). Two independent responsibilities are bundled in this review's scope (graph protocol vs. sandbox wall) but share no code and have disjoint owners — no overlap defect.

## 4. Public API/SPI signatures (compatibility & ownership)

```go
// graphscheduledcontract (Go Core-owned; canonical JSON contracts shared with Rust)
func BuildSuccessor(snapshot graphdispatch.ControlSnapshot, scheduleSHA256 string,
    options graphdispatch.ExecutionOptions, receipts []scheduledterminal.Receipt,
    predecessorContent string, targetNodeID string) (ScheduledNodeContractCandidate, error)
    // v2; signature changes break goldens + wave-admit — treat as frozen
func ReadySuccessorNodes(snapshot graphdispatch.ControlSnapshot, scheduleSHA256 string,
    receipts []scheduledterminal.Receipt) ([]string, error)  // new; consumed only by Rust wave-admit via CLI
func ReadyCommand(args []string, stdin io.Reader, stdout, stderr io.Writer) int // CLI adapter, registered in cmd/forge/cli_dispatch.go
func Command(args []string, stdin io.Reader, stdout, stderr io.Writer) int      // initial + successor contract CLI (--target-node)
func DecodeCandidate(reader io.Reader) (ScheduledNodeContractCandidate, error)  // cross-package consumer: graphscheduledrelease

// orchestrator/sandbox (SPI; orchestrator-owned)
type Runner interface {
    Run(ctx context.Context, argv []string, timeout time.Duration) (output string, exitCode int, err error)
} // implemented structurally by docker.Runner, firecracker.FirecrackerRunner; wired at sandbox_config.go

// Rust store (infrastructure-internal, pub(super))
admit / admit_with_before_reread / read::list / read::show  // idempotency-key replay; one-per-(run,node,attempt) after fix
```
Every option (`--target-node`, `--predecessor-receipt`, `--predecessor-content`, `SandboxConfig{Type,Image,Kernel,MemoryMB,TimeoutSec}`) has a live consumer; no speculative configuration found. No broad `Deps` interfaces; `graphdispatch.ExecutionOptions` is reused, not widened.

## 5. State-ownership table

| State | Writer | Store | Consistency | Recovery |
|---|---|---|---|---|
| Execution schedule (v13) | Go Core (`graph-execution-schedule`); Rust admit | SQLite v13 sidecar, immutable, content-addressed | digest-bound to control; one per run | Go rebuilds from control; admission re-verifies digest; same-key replay |
| Initial candidate (v14) | Go Core; Rust admit | SQLite v14, immutable | one per run (initial scope); `BEGIN IMMEDIATE` key-first | idempotency replay preserves identity/bytes |
| Successor candidates (v20) | Go Core build; Rust `successor admit`/`wave-admit` | SQLite v20, immutable | **declared** one per (run,node,attempt); **actual** one per run (Finding 1) | same-key replay; no deletes; needs fix |
| Predecessor receipts | Go Core terminal control; Rust terminalize | SQLite v16 lifecycle sidecar (terminalized) | append-only evidence; retry/successor authority false by construction | read-only export; byte-verified at each successor admit |
| Provider request (v18) | Rust prepare (pure codec from admitted candidate) | SQLite v18, immutable | one per run today (UNIQUE graph_run_id/schedule_id) — chain-blocking | replay; needs v21 per-node slots |
| Dispatch lifecycle (v16/v19) | Rust claim/terminalize; Go Core receipt | SQLite v19 | one per run today; lane exclusive-until-terminal; status incl. adjudicated | quarantine, no resend; explicit `dispatch adjudicate` (ADR-0034) |
| Sandbox config | CLI flags → `SandboxConfig` | in-memory only | auto-wire fail-closed; unknown type = permanent KindConfig | per-process; nothing durable to recover |
| VM/container run | runner (docker/firecracker) | ephemeral (`--rm`; fresh mke2fs image per run) | no state leaks between runs by construction | kill+reap on stop; no journal |

Multi-replica: none claimed — single local Hub/WAL, "not remote exactly-once" stated honestly in ADR-0030. Atomicity: all Hub writes are key-first `BEGIN IMMEDIATE` with full re-read before commit.

## 6. Recommendation

**Approve with Changes** — the boundaries and state ownership are sound (Go = scheduler/canonical-artifact owner, Rust = durable store owner, sandbox behind a minimal fail-closed port; layer map and all 8 arch checks pass; no cycles; deterministic fake-runner and golden tests exist for every non-live path). The wave-parallel **planning view** (`ReadySuccessorNodes` + `--target-node` materialization) works, but the declared store invariant "one candidate per node per run" is **not realized at the admission boundary**, and the effectful successor chain is structurally capped at one node per run by v16/v18 UNIQUEs.

**Preconditions for ship (must-fix):**
1. **Finding 1 (High):** remove the one-per-run "Graph Run"/"schedule" conflict checks in `reject_existing_candidate_identity`; add store + wave-admit tests with ≥2 candidates per run. The v21 per-node request/lifecycle slots may be deferred to the next effectful slice **only if** the ADR-0036 decision records them explicitly (no silent re-claim of "successor chain works").
2. **Finding 2 (Medium):** align `predecessor_terminal_receipts` with ADR-0035 (exact direct predecessors, incl. empty set) or amend the ADR; add the empty-preds golden.
3. **Finding 3–5 (Medium/Low):** stale protocol docstrings, dead `successorPredecessorsCovered`, and the empty-ready error conflation.

**Deferred (explicitly):** effectful multi-dispatch orchestration (honestly deferred in sprint docs); v16/v18 per-node slot migration (next slice, per ADR-0036); sandbox Finding 6 cleanups.

**Validation:** observed — `forge accept` ACCEPTED with the cited pass counts; direct code trace of write.rs/v16/v18/v20 SQL/validate.go with exact lines; dead-code and "no second-admit test" established by grep. Inferred — none of the gate results cover the multi-candidate admission path (no such test exists); the chain-blocking behavior is established by trace, not by a failing test run.
