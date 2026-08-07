Both migration failures are reproduced against the exact repository SQL. The review is complete; here is the report.

---

# Stage 02 — Security and Protocol Review: Wave-parallel storage (schema v20–v22) and orchestration

**Roles:** Security Engineer + Protocol Expert. All findings below are grounded in current code (commit `e3bbc94` + one uncommitted test-strengthening diff), executed tests, and reproduced behavior. OIDF certification is **N/A** — no OAuth/OIDC surface exists in this subsystem; the applicable normative contracts are ADR-0031/0035/0036.

## Assets, actors, trust boundaries, attacker capabilities

| Item | Definition |
|---|---|
| Assets | Hub SQLite DB (run journal, schedules, initial/successor candidates, provider-request bodies incl. private prompts, dispatch lifecycles, terminal receipts, lane claims, pricing snapshots); private control snapshot; provider request bytes |
| Actors | CLI operator (local, trusted), Go core (`graph-scheduled-ready-nodes` / `graph-scheduled-node-contract`, trusted generator), Rust runtime/hub (enforcement point), provider endpoint (out-of-band; execute path not reachable in this environment) |
| Trust boundaries | ① CLI↔Go core: subprocess, argv/stdin (control bytes piped, no shell); ② CLI↔hub: SQLite file, `PRAGMA foreign_keys=ON`, WAL, `synchronous=FULL`; ③ Go/Rust evidence handoff: receipts carried inside content-addressed candidates (domain-separated digests); ④ hub↔provider: outbound HTTPS only during execute (not in this slice) |
| Attacker capabilities | Local operator with file access; crafted candidate/receipt JSON via `successor admit`; env/flag-controlled Go-core path (`--go-core`, `FORGE_CORE_DIR` — equivalent to local code exec, treated as trusted); malicious graph/schedule content; adversarial provider responses (execute path) |

Ordered data flow: `export_control` → Go `graph-scheduled-ready-nodes` (receipts digest-verified, run-bound) → per-node `graph-scheduled-node-contract --target-node` (target must be topologically ready) → candidate JSON (content-addressed) → Rust `successor admit` (pristine-run gate → per-node/per-ordinal slot identity → control/schedule exact-match → per-receipt durable-lifecycle verification) → later `prepare`/`claim`/`dispatch` (each with own binding checks).

## 1. Findings (sorted by severity)

### Finding 1 — High — v21→v22 (and v17→v18) migration fails with FK violation on any hub with dispatch history; hub becomes permanently unopenable
- **Surface:** stock binary (hub open/migrate path)
- **Location:** `forge-runtime/crates/infrastructure/src/sqlite_hub/schema_contract/v22_sql.rs` (`MIGRATE_V21_TO_V22_SQL`, stmt 3: `DROP TABLE group_agent_graph_scheduled_node_provider_requests`; same pattern in `v18_sql.rs`); `schema.rs` `configure()` sets `PRAGMA foreign_keys = ON` before `migrate_or_validate`
- **Evidence:** Reproduced with the exact repository SQL and real CHECK/FK definitions. A v16 hub with one provider-request row + one lifecycle row: `v17->v18 FAILED: FOREIGN KEY constraint failed` (DROP of parent `provider_requests` while child `dispatch_lifecycles` still references it — SQLite performs an implicit DELETE on DROP, which violates the child's `ON DELETE RESTRICT` under FK enforcement). Same result on a v21 hub with lifecycle rows: `v22 stmt 2 FAILED: FOREIGN KEY constraint failed` at the DROP. The existing test suites never exercise a *populated* v16→v22 upgrade (migration tests stop at v15; `full_contract.rs` builds batches on an **empty** in-memory DB, where the implicit DELETE deletes zero rows and passes).
- **Failure scenario:** Any hub that ever performed a real effectful dispatch (≥1 lifecycle row — the primary purpose of v16+) opens with this binary: `BEGIN IMMEDIATE` → migration → FK error → ROLLBACK → open fails. Every subsequent command fails; the hub is unopenable by the new build (old binaries still open it; no data loss, but upgrade is impossible).
- **Impact/likelihood:** Availability/upgrade break for exactly the population the feature serves; violates ADR-0036's "旧库按版本逐个升级,全部数据保留". Likelihood: certain (any lifecycle row); first broken step is v17→v18, so v16/v17 hubs and any v18–v21 hub that dispatched after upgrading are affected.
- **Fix:** Inside the migration transaction, `PRAGMA defer_foreign_keys = ON` (single-batch commit re-checks all FKs against the final state, which is consistent), or `PRAGMA foreign_keys = OFF` + `PRAGMA foreign_key_check` after; add integration tests that seed provider-request **and** lifecycle rows at v16 and at v21 and migrate to v22 under FK enforcement. (Reordering table rebuilds cannot fix name-based FK DROPs.)
- **Risk/effort:** Low breaking risk; small SQL change + one data-bearing migration test per affected step.

### Finding 2 — Medium — Successor admission never binds predecessor receipts to the candidate's Graph Run (cross-run evidence reuse)
- **Surface:** stock binary (`successor admit` / `wave-admit` admission path)
- **Location:** `forge-runtime/crates/application/src/group_agent_node_execution/scheduled_successor_service.rs` `verify_receipt_binding` (compares `node_id`, `attempt`, `receipt_id`, `receipt_sha256`, `provider_request_id`, `dispatch_id` — **not** `lifecycle.graph_run_id`); `GroupAgentScheduledNodePredecessorReceipt` carries no run binding; domain `scheduled_contract_validation.rs::valid_receipt` checks identifiers only. Contrast: Go `verifyReceipts` (`build_successor.go`) does enforce `receipt.GraphRunID == snapshot.GraphRunID`.
- **Evidence:** Node IDs are operator-authored (test fixtures: `"frontend"`, `"backend"`) and identical across runs of the same graph. A candidate for run A whose schedule requires predecessor P can embed P's genuine terminal receipt from run B (same graph): `predecessors_covered` (schedule preds ⊆ carried receipts) passes, `verify_predecessor_evidence` finds the durable terminalized lifecycle by `provider_request_id` and all six compared identities match. No later gate re-checks evidence: `scheduled_provider_request_service.rs::prepare` and the claim/release chain never call `verify_predecessor_evidence` (grep-verified). The Go-generated pipeline is safe (run-bound receipts); only hand-crafted candidate JSON reaches the Rust gap.
- **Failure scenario:** Operator has a second run B of the same graph where P terminalized; for run A (P never terminal/failed), hand-crafts a candidate for X carrying P's run-B receipt; admission succeeds; prepare→claim→dispatch of X proceeds while P is not terminal in run A.
- **Impact/likelihood:** Violates ADR-0031/0035's no-send invariant ("no node is ever selected before its direct predecessors are provably terminal" in the same Run). Requires local hub access + same-graph re-run + crafted JSON: Medium impact, low likelihood; the invariant is claimed fail-closed ("Every binding still fails closed on drift") but one binding is missing.
- **Fix:** In `verify_receipt_binding`, require `inspection.graph_run.graph_run_id == candidate.graph_run_id` (and ideally `project_lane_sha256`); add a test: admit a successor for run A using a genuine receipt exported from run B of the same graph → must be rejected.
- **Risk/effort:** Low breaking risk (tightens an invariant); small change + one test.

### Finding 3 — Low — `INSERT_REQUEST_SQL` guard has an operator-precedence bug (`A OR B AND C` ≠ `(A OR B) AND C`)
- **Surface:** stock binary (`provider-request prepare`)
- **Location:** `forge-runtime/crates/infrastructure/src/sqlite_hub/group_agent_scheduled_node_provider_request/write.rs` `INSERT_REQUEST_SQL` WHERE clause: the initial-candidate `EXISTS` (A) is OR-ed with (successor-EXISTS (B) AND effect-free-flags (C)); intended `(A OR B) AND C`
- **Evidence:** As written, a contract found in the **initial** candidate table bypasses the `provider_request_present=0`/`execution_authority_released=0`/… guard entirely. Currently unreachable: schema CHECKs force all effect flags to 0 for passive candidates, and `identity::reject_existing` rejects contract reuse before the INSERT. The guard is dead defense-in-depth with misleading semantics.
- **Failure scenario:** A future flag relaxation (attempts are per-node in v22) would silently allow preparing a provider request against an already-effectful contract; today the failure surfaces as a raw UNIQUE-constraint error instead of the intended conflict.
- **Impact/likelihood:** None exploitable today; latent correctness/defense-in-depth defect. Low.
- **Fix:** Add the parentheses; add a unit test that forces the flag state via raw SQL and asserts the guard rejects.
- **Risk/effort:** Zero breaking risk; one-line SQL + test.

### Finding 4 — Low — `wave-admit` derived idempotency keys can overflow the 256-byte bound
- **Surface:** stock binary (`wave-admit`)
- **Location:** `forge-runtime/crates/interfaces/src/group_agent_graph/wave_command.rs` `materialize_node`: `format!("{base}-{node_id}")`
- **Evidence:** A user key of 256 bytes plus `-` plus a ≤128-byte node ID exceeds `MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES`; admission then fails with a conflict for every node, and without a user key, re-runs after a partial success use a fresh base and produce per-node conflicts (non-zero exit) instead of replays.
- **Failure scenario:** Operator passes a long `--idempotency-key` (or re-runs without one after a partial wave) → confusing per-node rejections.
- **Impact/likelihood:** Usability/automation only; fail-closed. Low.
- **Fix:** Bound the base key (`min(len, 256 - 1 - node_id.len())`) or hash the derivation; document that re-runs must reuse the key. Test: max-length key round-trip.

### Finding 5 — Info — No test exercises a data-bearing upgrade path (root cause of Finding 1)
- **Location:** `crates/infrastructure/src/sqlite_hub/schema_migration_tests.rs` (seeds stop at v6 panels), `schema_v5…v15_migration.rs`; `full_contract.rs` builds expected schemas on an empty DB
- **Evidence:** `forge-accept` and all suites pass while the populated v16→v18/v21→v22 path fails in reproduction. Recorded as missing evidence per the review checklist, not as a pass.

### Finding 6 — Info — Fabricated receipts are accepted as planning input; effectful execute path is not verifiable here
- **Location:** `ready_command.go`/`verifyReceipts` (digest self-consistency only); `docs/external-resource-verification.md`
- **Evidence:** The wave tests deliberately admit a fabricated receipt as consumed-set evidence; admission of any candidate that *carries* receipts is durably gated (fan-out test rejects both nodes, exit non-zero — Finding 4 of the prior round is closed via `main.rs` exit code). A fabricated receipt grants no authority the schedule does not already grant (zero-receipt siblings are admissible by design, ADR-0035/v21). Real provider execute (LiteLLM id-drift defect) is documented as unreachable in this environment; the success path is verified only by application-layer tests, honestly labeled.

## 2. RFC matrix

No OIDF/OAuth normative surface exists in this subsystem; claims of OIDF conformance are **N/A (not claimed, not certified)**. The applicable contracts are the ADRs:

| Standard/section | Normative requirement | Status | Evidence | Gap |
|---|---|---|---|---|
| ADR-0036 §v20 | Per-node candidate slots; run/schedule one-shot checks removed; identity check per-node/per-ordinal/per-request | Verified | `successor/write.rs reject_existing_candidate_identity` matches `UNIQUE(graph_run_id,node_id,attempt)` + `UNIQUE(schedule_id,execution_ordinal,attempt)` (v21 SQL); tests pass | None |
| ADR-0036 §v21 | Zero-receipt candidates; `predecessor_receipt_count 0..=31`; Go filters receipts to direct predecessors | Verified | v21 CHECK `BETWEEN 0 AND 31`; `build_successor.go filterDirectPredecessors`; `scheduled_contract_validation.rs predecessors_valid`; digest identical v20/v21 as documented | None |
| ADR-0036 §v22 | Per-node provider-request + lifecycle slots; multi-row run-binding walk with lightweight contract decode (no recursion) | Verified | v22 SQL table-level UNIQUEs; `provider_request/read.rs validate_graph_run_binding` + `load_contract_lightweight` (no run re-inspect); stack-overflow fix present | Finding 3 (guard precedence), Finding 1 (migration) |
| ADR-0036 §迁移 | Single batch/version; data-preserving chain; digest-locked | **Violated** | `PRAGMA user_version=22` single batch OK, but populated upgrade fails (Finding 1) | Migration-with-data untested |
| ADR-0035 | Topologically-ready selection; candidate carries exactly direct predecessors' receipts; serial behavior preserved | Verified | `selectReadyNode`/`ReadySuccessorNodes`; fan-out and diamond CLI tests; `validate_successor_node` coverage check | Finding 2 (receipt→run binding at Rust admission) |
| ADR-0031 | Receipts = evidence, never authority; admission verifies receipts against durable terminalized lifecycles; replay preserves identity/time/bytes | Partially verified | `verify_predecessor_evidence` + `verify_receipt_binding` (6 identities), `ensure_replay`; `export_predecessor_receipt` terminalized-only | Run binding missing (Finding 2); `admit_successor` file receipts are size-preflight only (by design, candidate binds digest) |
| ADR-0031 §safety | Fail-closed on drift; no-send/no-resend/no-leakage | Verified for carried evidence | Fan-out rejection test; `is_pristine_source`; control/schedule exact match | Finding 2 |

## 3. STRIDE

| Category | Concrete threat | Existing mitigation | Residual gap |
|---|---|---|---|
| Spoofing | Fabricated receipt used to materialize successors | Go digest domain + canonical strict decode; Rust durable-lifecycle verification of every carried receipt | Fabricated receipts accepted as planning input (harmless; Finding 6); cross-run genuine receipts (Finding 2) |
| Tampering | Hand-crafted candidate JSON | Content-addressed contract/request digests; exact byte/canonical checks; control/schedule re-derivation | None beyond Finding 2 |
| Repudiation | — (local audit via run journal v1/seq-1 head constraints; events appended on effectful transitions) | Verified | Execute path unverifiable here (Finding 6) |
| Info disclosure | Prompt/contract plaintext exposure | Default views redact; `--include-contract` explicit (ADR-0031) | Conflict messages reveal slot existence (local CLI; Info) |
| DoS | Migration bricking; oversized inputs | 64KB receipts, 4MB candidate, 16MB body, 1MB content caps; busy_timeout; BEGIN IMMEDIATE | **Finding 1: any hub with dispatch history cannot open** |
| Elevation | Prepare against effectful contract; lane double-claim | Pristine-run gate; per-node slots; lane_active partial unique index; identity checks | Guard precedence (Finding 3, currently unreachable) |

## 4. Trust-boundary summary

```
[operator/CLI] ──control (stdin)──▶ [Go core (trusted, env/flag-selected)]
      │                                      │ candidate JSON (digested)
      │ argv: --go-core / FORGE_CORE_DIR     ▼
      │                               [Rust successor admit]
      │                                      │ ① pristine run ② slot identity
      │                                      │ ③ control/schedule exact match
      │                                      │ ④ receipts → durable lifecycles
      ▼                                      ▼
[Hub SQLite (FK=ON, WAL, FULL)] ◀──prepare/claim──▶ [dispatch lifecycle sidecar]
      ▲
      └── run inspect: multi-row binding walks (lightweight decode, no recursion)
```
Boundary ④ is where Finding 2 (missing run binding) and Finding 1 (FK-enforced DROP inside migration) sit.

## 5. Tests run, tests required, conformance, ship condition

**Run (all pass):** `forge-accept` (`node harness/acceptance.mjs`) — **ACCEPTED**: 9 pass / 0 fail / 2 N/A (lint N/A for go/python/typescript tooling absent; coverage not configured). `cargo test -p forge-runtime-infrastructure` (169+ suites, all green), `cargo test -p forge-runtime-application` (all green), `cargo test -p forge-runtime-cli --test cli_group_agent_scheduled_node_wave_admit` (4/4), `go test ./internal/graphscheduledcontract ./internal/scheduledterminal` (PASS), `go test ./...` (1330 tests, via gate). Reproduction of Finding 1 executed against exact repository SQL (v17→v18 and v21→v22 both `FOREIGN KEY constraint failed`).

**Required but missing:** (1) populated v16→v22 and v21→v22 migration tests (provider request + lifecycle rows, FK ON) — would have caught Finding 1; (2) cross-run receipt-reuse rejection test (Finding 2); (3) concurrent wave-admit replay test; (4) `foreign_key_check` after migration.

**Published conformance evidence:** None for OIDF — none claimed; internal ADR contracts are the standard (matrix above). Real provider execute success path remains unreachable in this environment (documented `docs/external-resource-verification.md`); every success-path claim is backed by application-layer tests, not a live provider.

**Overall readiness:** The wave-parallel semantics, evidence chain for carried receipts, identity/slot enforcement, replay behavior, and fail-closed gates are well implemented and tested. However, the upgrade path — a stated ADR-0036 property — is broken for exactly the hubs the feature serves, and the successor evidence chain is missing one binding (run identity).

**Critical/High counts:** 0 Critical / 1 High (Finding 1) + 1 Medium (Finding 2).

**Ship decision: NO.** Precise condition: ship when (a) the v21→v22 (and v17→v18) migrations run under `defer_foreign_keys` (or FK-off + `foreign_key_check`) with new data-bearing migration tests passing; (b) `verify_receipt_binding` enforces the lifecycle's `graph_run_id == candidate.graph_run_id` with a cross-run rejection test; (c) Findings 3–4 addressed or explicitly deferred in writing.

**Must-fix:** Findings 1, 2. **Explicitly deferred:** 3, 4 (Low), 5, 6 (evidence gaps; Finding 6 is a documented environment limitation, not a defect).

**Validation run vs inferred:** Migration failure — directly reproduced from repository SQL; Go/Rust test results — executed; gate output — executed; cross-run reuse and guard-precedence impact — traced through code and domain validation (no live exploit attempted); provider execute path — N/A in this environment per project documentation.
