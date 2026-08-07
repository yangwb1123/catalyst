# Stage 03 — Distributed Systems Review: Wave-Parallel Storage (schema v20–v22) and Orchestration

**Stage 01 output:** reviewed (`docs/reviews/reviews/forgeos-review-context/stage-01.out.md`).
Stage 01 Findings 1a/1b (per-run one-shot walls) are closed in code: v20 per-node candidate
slots, v22 per-node provider-request/lifecycle slots, identity checks symmetric.
**Topology (verified, not assumed):** single local SQLite hub file per user state dir
(`PRAGMA journal_mode=WAL`, `synchronous=FULL`, `secure_delete=ON`, `foreign_keys=ON`);
one writer at a time via `BEGIN IMMEDIATE`; the Go core is a **pure** subprocess (control +
receipts on stdin, no hub access); there are no replicas, caches, buses, or remote stores.
**Roles:** Distributed Systems Engineer · Database Architect.

**Validation run:**
- `node harness/acceptance.mjs` → **ACCEPTED** (9 pass · 0 fail · 2 honest N/A: lint partial, coverage).
- `cargo test -p forge-runtime-cli --test cli_group_agent_scheduled_node_wave_admit` → 4/4 pass.
- `cargo test -p forge-runtime-infrastructure` → all suites green.
- Empirical reproductions (this review, Python/sqlite3 against the **actual shipped SQL strings**):
  - TEST1: the `adjudicate` store UPDATE fails at the v19 table shape — `no such column: adjudicated_at_ms`.
  - TEST2: the `MIGRATE_V21_TO_V22_SQL` batch fails on a lifecycle row with `status='adjudicated'` — `CHECK constraint failed: status IN ('claimed','terminalized','quarantined')`; with only claimed/terminalized rows it migrates cleanly (TEST3).
- Honest N/A: no concurrency (multi-writer) tests for the wave tables, no v16–v22 migration tests, no adjudication tests, and the effectful multi-node dispatch success path (external OpenAI Responses endpoint; documented LiteLLM id-drift in `docs/external-resource-verification.md`) is unreachable in this environment.

---

## 1. Findings

### 🔴 Finding 1 — High
| Field | Content |
|---|---|
| **Title** | Hard-crash adjudication (ADR-0034) is non-functional at every schema version, and v22 additionally removes the `adjudicated` state the migration chain must carry — stranded claims are unrecoverable without hand-editing the database |
| **Surface** | Nested module (Rust infrastructure store + schema contract) |
| **Location** | `forge-runtime/crates/infrastructure/src/sqlite_hub/group_agent_scheduled_node_lifecycle/adjudicate.rs:51` (`UPDATE … SET status='adjudicated', lane_active=0, adjudicated_at_ms=?1`); `schema_contract/v22_sql.rs` (lifecycle `status` CHECK `IN ('claimed','terminalized','quarantined')`, no `adjudicated_at_ms` column); `schema_contract/v19_sql.rs:53-59` (only v19-v21 allow `'adjudicated'`, still **no** `adjudicated_at_ms` column); reachable CLI `group graph run scheduled-contract provider-request dispatch adjudicate` (`scheduled_contract_args.rs:60`, `scheduled_provider_request_dispatch.rs:172-186`) |
| **Evidence** | `grep -rn "adjudicated_at_ms"` across all schema SQL files: **zero matches** — the column exists only in the CLI request (`scheduled_provider_request_dispatch.rs:185`) and the domain struct (`group_agent_scheduled_node_lifecycle.rs:339`). TEST1 (this review, v19 table shape): `no such column: adjudicated_at_ms`. TEST2: the shipped v22 migration aborts with a CHECK violation on an `'adjudicated'` row (a state v19's own CHECKs allow). No test in the repo exercises adjudication (grep across all test trees: none). |
| **Failure scenario** | Executor hard-crashes after `claim` (lane_active=1, no terminal evidence). Operator runs `dispatch adjudicate` → store UPDATE fails (`no such column`). The stranded lane is never released; the wave cannot advance past this node; the partial unique index `…_project_lane_active` blocks any future claim in that lane forever. If the operator hand-fixes the row to `status='adjudicated'` (v19-v21 schema allows it), the next v21→v22 migration then fails its CHECK on every open — the hub is **permanently unopenable** by the v22 binary (migration runs inside `BEGIN IMMEDIATE` at every open, `schema.rs:120-137`), an availability outage |
| **Impact and likelihood** | Availability/recovery: the documented fail-mode contract (ADR-0034: "operator proves old executor stopped, releases lane") is dead code. Likelihood: any hard crash between claim and terminalize (or any operator need to release a lane) — 100% failure when exercised. No data corruption (fail-closed), but zero recovery paths |
| **Fix** | (a) Plan v23: add `adjudicated_at_ms` column + `'adjudicated'` status + `adjudicated_at_ms >= created_at_ms` CHECK to the lifecycle table (same data-preserving rebuild pattern), carry `adjudicated_at_ms` through the migration, restore the UPDATE (drop the stray `\\`), add an E2E test (claim → simulate crash → adjudicate → lane free → successor claim succeeds). (b) If wave-parallel intentionally drops adjudication: delete the command/domain variant, and make the v22 migration fail fast with a clear "unsupported adjudicated lifecycle" message instead of a raw CHECK error. (a) is preferred — the pid-sidecar evidence (`ExecutorPidSidecar`, `scheduled_provider_request_dispatch.rs`) is still written today |
| **Risk/effort** | (a) medium: one more schema version + digest golden churn; (b) low. Both require a migration-version test for v21→v22 with an adjudicated row |

### 🟡 Finding 2 — Medium
| Field | Content |
|---|---|
| **Title** | No multi-writer concurrency tests exist for any wave-parallel table; only fault-injection (late re-read rollback) and single-connection atomicity are covered |
| **Surface** | Nested module (Rust infrastructure store tests) |
| **Location** | `forge-runtime/crates/infrastructure/src/sqlite_hub/tests/group_agent_scheduled_node_{contract,provider_request}_atomicity.rs` (fault-injection only); no `thread::spawn`/two-connection test in `sqlite_hub/tests` (the only concurrent-writer test in the crate is `tests/sqlite_group_model_analysis.rs`, a different table) |
| **Evidence** | grep of `sqlite_hub/tests/*.rs` for `thread::spawn|join()`: none. The atomic primitives are correct by construction (`BEGIN IMMEDIATE` + guarded `INSERT … WHERE EXISTS` + UNIQUE slots + partial unique lane index), but none of the race outcomes — duplicate claim of one lane from two connections, duplicate prepare of one node slot with different keys, concurrent wave-admits of the same run, 5s busy-timeout expiry — is asserted |
| **Failure scenario** | A future refactor (e.g., moving the lane check out of the write transaction, or replacing `busy_timeout`) regresses exactly-once without any test failing |
| **Impact and likelihood** | Maintainability/regression risk on the exact-once core of the effectful pipeline. No current defect observed |
| **Fix** | Add a two-connection test module: same-lane concurrent claims (one Claimed, one Conflict), same-slot concurrent prepares (one Created, one Conflict / one Replayed), concurrent `wave-admit` re-runs, and a held-write-lock busy-timeout test |
| **Risk/effort** | Low; ~1-2 days including the busy-timeout harness |

### 🟡 Finding 3 — Medium
| Field | Content |
|---|---|
| **Title** | Read path is O(nodes) re-validation with full body re-hash per node; lifecycle operations on a 32-node wave re-validate every sibling's provider-request body (up to 16 MiB each) inside their write transaction |
| **Surface** | Nested module (Rust infrastructure store) |
| **Location** | `group_agent_graph_run/read.rs:125-139` (run inspect validates all children); `group_agent_scheduled_node_provider_request/read.rs:64-88` (`validate_graph_run_binding` iterates all rows, `validate_exact_provider_body` re-encodes and re-validates the body); `group_agent_scheduled_node_lifecycle/read.rs:169-204` (`reconstruct` → deep run inspect + deep provider-request inspect); v22 body bound `1..=16777216` bytes |
| **Evidence** | Every `claim`/`terminalize`/`adjudicate`/`inspect` of one lifecycle re-inspects the whole run, and each provider-request row re-runs SHA-256 + codec exact-body validation on its full blob. A 32-node wave with max-size bodies implies ~512 MiB hashed per lifecycle read; each write holds `BEGIN IMMEDIATE` during the reads, so concurrent readers/writers hit the 5s busy timeout earlier |
| **Failure scenario** | Wave-parallel scale (32 nodes, large bodies): claim/terminalize latency grows linearly in the number of siblings; long write transactions increase SQLITE_BUSY contention for concurrent CLI operations on the same hub |
| **Impact and likelihood** | Performance/capacity degradation, not correctness. Likelihood: guaranteed at scale, invisible in current fixtures (1-2 nodes) |
| **Fix** | Make run-binding validation metadata-only (stored `provider_request_bytes` + `provider_request_sha256` columns are already there): skip full body decode/hash when the digest columns match the stored contract digest; spot-check bodies on explicit `inspect`. Keep the full deep path for `inspect` only |
| **Risk/effort** | Low-medium; digest columns already durably bind the body |

### 🟡 Finding 4 — Medium
| Field | Content |
|---|---|
| **Title** | Store-level claim idempotency lacks the replay-equality check that prepare/admit enforce: a conflicting re-claim returns `AlreadyClaimed` with the original inspection instead of rejecting the mismatch |
| **Surface** | Nested module (Rust infrastructure store) |
| **Location** | `group_agent_scheduled_node_lifecycle/claim.rs:20-27` (existing row → `reconstruct` → `AlreadyClaimed { inspection }`, no comparison with the incoming request); contrast `group_agent_scheduled_node_provider_request/write.rs:150-163` (`ensure_replay`) and `group_agent_scheduled_node_successor/write.rs:245-253` |
| **Evidence** | `claim.rs` returns before any equality check against `request.claim` / `request.authorization` / `request.provider_request_body`. The application service (`group_agent_scheduled_node_dispatch_execution.rs`) preflights inputs before claiming, which narrows the divergence, but the store contract itself accepts any input for an already-claimed `provider_request_id` |
| **Failure scenario** | A caller re-issues a claim for the same provider request with a different authorization/pricing snapshot (e.g., a stale retry after a pricing refresh); the store reports success with the **original** authority. The retry silently proceeds with inputs it believes were accepted |
| **Impact and likelihood** | Protocol semantics inconsistency (idempotent replay without equality), low security impact (the returned authority is the durable original; no mutation). Likelihood: low, but it violates the store's own "exact replay" convention |
| **Fix** | On the `AlreadyClaimed` path, compare stored `claim` + `release_control` + `authorization` + `provider_request` against the request and return `Conflict` on mismatch (mirror `ensure_replay`); add a store test |
| **Risk/effort** | Low; ~half a day |

### 🟢 Finding 5 — Low
| Field | Content |
|---|---|
| **Title** | Stale version gates and error strings in the read-only open helpers misreport the supported schema range |
| **Surface** | Nested module (Rust infrastructure store) |
| **Location** | `schema.rs:114-116` (`open_existing_current_read_only_database` error text says "current schema version 18", accepts `SCHEMA_VERSION`=22); `schema.rs:119-121` (`open_existing_dispatch_preflight_read_only_database` accepts `[11..=22]`, error text says "11..=21") |
| **Evidence** | Text vs array mismatch at both sites |
| **Failure scenario** | An operator debugging a read-only-open failure sees a wrong version requirement and misdiagnoses the state file |
| **Impact and likelihood** | Operational clarity/honesty defect only |
| **Fix** | Derive the requirement string from the accepted-version list; add a unit assertion |
| **Risk/effort** | None/trivial |

### 🟢 Finding 6 — Low
| Field | Content |
|---|---|
| **Title** | `wave-admit` without `--idempotency-key` cannot replay: the generated per-invocation base key turns a re-run into identity conflicts ("belongs to another idempotency key") instead of replays |
| **Surface** | External frontend (Rust CLI) |
| **Location** | `group_agent_graph/wave_command.rs:120-140` (`generated_idempotency_key` per invocation; derived per-node keys); `group_agent_scheduled_node_successor/write.rs:96-134` (identity rejection) |
| **Evidence** | The derived key includes `process::id()` + monotonic counter + nanos (`state_path.rs:46-56`), so a re-run without a user key always derives new keys; the wave-admit docstring promises replay only "with the same --idempotency-key" |
| **Failure scenario** | Operator runs wave-admit (no key), the process dies mid-wave, re-runs without a key: already-admitted nodes are rejected with a confusing identity error, and the remaining nodes may still admit — partial wave with misleading output |
| **Impact and likelihood** | Fail-closed but operationally confusing; the rejection message does not hint that a key enables replay |
| **Fix** | When the key is auto-generated, print it in the summary ("re-run with --idempotency-key … to replay"); or persist the derived base key next to the run |
| **Risk/effort** | None/trivial |

### 🟢 Finding 7 — Info
| Field | Content |
|---|---|
| **Title** | Migration window is an unannounced availability break: the v21→v22 table rebuild (full blob copies) holds the single write lock; concurrent openers retry 5s then fail with Unavailable |
| **Surface** | Nested module (Rust infrastructure store) |
| **Location** | `schema.rs:76-90` (`OPEN_RETRY_TIMEOUT` 5s), `schema.rs:150-160` (`BEGIN IMMEDIATE` around migrations), v22 double table rebuild |
| **Evidence** | Migration is not incremental and there is no maintenance-mode/readiness signal; a second CLI process opening during a large migration fails after 5s |
| **Failure scenario** | Large hub with many 16 MiB provider-request bodies migrates longer than 5s while a second terminal runs any hub command |
| **Impact and likelihood** | Availability glitch on a rare event; recoverable by retry |
| **Fix** | Document the migration window; consider raising the open retry timeout during migration or publishing the migration state |
| **Risk/effort** | None/trivial (documentation) to low (timeout) |

---

## 2. State table

| State | Writer | Consistency | Atomic primitive | Recovery |
|---|---|---|---|---|
| `group_agent_graph_scheduled_node_successor_candidates` (per-node successor contract) | `successor/write.rs::admit` (`BEGIN IMMEDIATE`, 5s busy) | Strong (single file, WAL, FULL) | idempotency-key replay + per-(run,node,attempt) and per-(schedule,ordinal,attempt) UNIQUE slots + 4-way identity check + guarded insert | Same key → `Replayed` (exact-equality checked); different key → Conflict (fail-closed) |
| `group_agent_graph_scheduled_node_provider_requests` (per-node provider request, flags immutable — every flag column has a fixed CHECK value) | `provider_request/write.rs::prepare` (`BEGIN IMMEDIATE`) | Strong | idempotency-key replay + 5-way identity check + guarded `INSERT … WHERE EXISTS` (pristine run head seq/event-sha256 + contract flags) | Same key → `Replayed` (exact-equality checked); source drift → Conflict |
| `group_agent_graph_scheduled_node_dispatch_lifecycles` (per-node dispatch lifecycle) | `claim.rs` (insert), `terminalize.rs` (update), `adjudicate.rs` (update — **broken**, Finding 1) | Strong | claim: atomic insert `status='claimed', lane_active=1` under partial unique index `(project_lane_sha256) WHERE lane_active=1` + `reject_owned_lane`; terminalize: single UPDATE releases lane + persists artifact/evidence atomically | Duplicate terminalize → idempotent no-op returning current inspection; duplicate claim → `AlreadyClaimed` (no equality check — Finding 4); crashed claim → adjudication (**broken** — Finding 1) |
| `group_agent_graph_runs` + `run_events` (pristine v1/seq-1 head) | run journal writer (pre-wave) | Strong | head `(last_event_seq, event_sha256)` re-verified inside every prepare/claim transaction | Immutable during the whole wave; any drift → Conflict (fail-closed) |
| Go-core planning (`graph-scheduled-ready-nodes`, `--target-node` materialization) | stateless subprocess | — (pure function) | control + receipts on stdin; no hub access | Go failure → non-zero exit → fail-closed |
| `ExecutorPidSidecar` (pid/hostname file) | CLI execute | Local file | written before claim, removed after terminalize | Evidence for adjudication — currently unusable (Finding 1) |

No caches, no replicas, no cross-store writes: cross-replica invalidation and unsafe
cross-store transactions are N/A (single store; Go core never touches the hub).

## 3. Failure matrix

| Failure/injection | Required behavior | Observed behavior | Evidence/gap |
|---|---|---|---|
| Duplicate prepare, same idempotency key | replay, no double row | `Replayed` after exact-equality check | `write.rs:150-163`; atomicity tests pass |
| Duplicate prepare, same (run,node,attempt) slot, different key | reject | `Conflict` via 5-way identity check | `identity.rs`; no concurrent test |
| Two writers claim same project lane concurrently | one owner | Serialized by `BEGIN IMMEDIATE`; second → Conflict (partial unique index + `reject_owned_lane`) | `claim.rs:20-27,96-114`; **no two-connection test** (Finding 2) |
| Duplicate terminalize (retry after success) | idempotent success | Non-claimed status → returns current inspection | `terminalize.rs:33-44` |
| Executor crash after claim, before terminalize | operator adjudication releases lane (ADR-0034) | **Adjudicate UPDATE fails: `no such column: adjudicated_at_ms` (TEST1); v22 CHECK forbids `'adjudicated'`** | Finding 1; lane stranded forever |
| v21→v22 migration with an adjudicated row | data-preserving | **Migration aborts with CHECK failure; hub permanently unopenable (TEST2)**; claimed/terminalized rows migrate cleanly (TEST3) | Finding 1 |
| Crash mid-wave-admit (partial wave) | partial state + replay | Partial admission persists; re-run with same key replays admitted nodes; without a key → identity conflicts | `wave_command.rs`; Finding 6 |
| Concurrent wave-admits of same run (retry storm) | serialized, no double admission | `BEGIN IMMEDIATE` + slots serialize; >5s wait → Unavailable (fail-closed) | `BUSY_TIMEOUT` 5s; no stress test |
| v21 binary opens v22 hub | fail closed | `reject_unsupported_schema` → Corrupt error | `schema.rs:58-62` |
| v22 binary opens v21 hub | migrate | Migrates on open (single batch, rollback on failure) | `schema.rs:150-176`; TEST2/3 |
| Wall clock steps backward between claim and terminalize | fail closed | `terminalized_at_ms >= created_at_ms` CHECK rejects | `v22_sql.rs` |
| Wall clock steps backward mid-wave (created_at_ms) | correct ordering | No guard; `created_at_ms DESC, id DESC` lists can misorder | `read.rs find_all_by_run`; Low |
| Provider endpoint down during execute | quarantine, lane released | `persist_quarantine` stores artifact, releases lane, forbids resend | `group_agent_scheduled_node_dispatch_execution.rs` terminalize path |
| Concurrent immutable read-only pre-check while writer active | no torn state | `mode=ro&immutable=1` + before/after stat-identity guard; stale read possible → claim gate corrects | `schema.rs:97-132`; Low risk, fail-closed |
| Power loss mid-commit | durable | WAL + `synchronous=FULL` | `schema.rs:145-149` |
| Go core missing/broken | fail closed | non-zero exit → wave-admit fails before admission | `wave_command.rs run_go_with_stdin`; drift test passes |
| Migration longer than open retry (5s) | availability | concurrent openers fail Unavailable during big migrations | Finding 7 |

## 4. State machines

**Successor contract candidate** (per node; single transition, never updated):

```
absent --admit(idempotency-key, slot checks, pristine run)--> admitted
absent --admit(same slot, different key)--> Conflict        (absorbing per slot)
admitted --admit(same key, exact input)--> Replayed
```

**Provider request** (per node; single transition, flags frozen by CHECKs):

```
absent --prepare(key, slot checks, guarded INSERT: pristine run head + contract flags)--> prepared
prepared --prepare(same key, exact input)--> Replayed
```

**Dispatch lifecycle** (per node; the only mutable state machine):

```
absent --claim(lane unique, pristine run, atomic insert lane_active=1)--> claimed
claimed --terminalize(control+receipt)--> terminalized   (lane_active=0, evidence stored)
claimed --terminalize(no control)--> quarantined         (lane_active=0, artifact only)
claimed --adjudicate(operator, pid proof)--> adjudicated (lane_active=0)  [BROKEN: Finding 1]
claimed --terminalize(retry)--> no-op, returns current inspection
any terminal state --anything--> absorbing (no transitions out; lane index freed)
```

Wave semantics: all wave nodes share the same pristine run head; there is no journal
advance during a wave — concurrency control is entirely per-node/per-lane/per-key
slots plus `BEGIN IMMEDIATE`. Wave advancement is operator-driven (collect terminal
receipts → next wave-admit), deterministic in schedule serial order
(`ReadySuccessorNodes` iterates `schedule.Nodes`, `build_successor.go:132-147`).

## 5. Ordering assumptions, cross-store transactions, required tests

**Ordering assumptions (all currently satisfied):**
1. Wave planning order = schedule serial order (deterministic; no map iteration).
2. The run journal stays pristine (v1/seq-1) for the whole wave; every prepare/claim
   re-verifies `(last_event_seq, event_sha256)` inside its transaction.
3. Per-node slots (`graph_run_id,node_id,attempt`), per-ordinal slots
   (`schedule_id,execution_ordinal,attempt`), idempotency keys, and the active-lane
   partial index are the only cross-row ordering dependencies.
4. `created_at_ms DESC, id DESC` list ordering assumes a monotonic-ish wall clock;
   backward steps can misorder lists but never corrupt (CHECKs guard the terminal time).

**Unsafe cross-store transactions:** none. The Go core is a pure subprocess; the hub is
the only store; receipt export → Go planning → admission TOCTOU is closed by re-validating
the pristine head and exact digests inside the admission transaction.

**Required tests (missing today):**
- **Race/fault-injection:** two-connection same-lane claim; two-connection same-slot
  prepare; concurrent wave-admits; busy-timeout expiry (Finding 2).
- **Migration:** v21→v22 with an `'adjudicated'` row (currently fails, Finding 1);
  a v16→v22 full-chain data-preservation test with per-node rows from multiple nodes;
  mixed-version: v21-binary-opens-v22-db fail-closed (currently only covered by code trace).
- **Adjudication E2E:** claim → simulated crash → adjudicate → lane free → successor
  claim succeeds (feature is dead today, Finding 1).
- **Clock skew:** terminalized/adjudicated `at_ms < created_at_ms` rejection; mid-wave
  backward step.
- **Recovery:** kill mid-wave with/without user key; replay assertions (Finding 6).

## 6. Recommendation and multi-replica readiness

**Readiness:** the wave-parallel storage design is sound at its core — single-writer
SQLite with `BEGIN IMMEDIATE`, guarded inserts, idempotency-key replay, per-node/per-lane
slots, deterministic pure-function planning, and fail-closed behavior on every drift
path. The gates are green (forge accept: 9 pass · 0 fail · 2 N/A).

**Ship decision: NO** — condition: fix Finding 1 (adjudication). The wave-parallel
slice ships a schema (v22) that permanently disables the documented hard-crash recovery
path and cannot migrate a database containing the `'adjudicated'` state its own earlier
schema chain (v19-v21) legally allows. Everything else is test/operational debt
(Findings 2-7), not blocking.

**Must-fix (before ship):**
1. v23 schema restoring `adjudicated_at_ms` + `'adjudicated'` status, carrying the
   column through migration, and an E2E adjudication test; alternatively delete the
   feature and make the v22 migration reject adjudicated rows with a clear error.
2. Migration test v21→v22 with an adjudicated row.

**Deferred (explicitly):** Finding 2 (concurrency tests), Finding 3 (read-path
O(nodes) re-validation), Finding 4 (claim replay equality), Findings 5-7.

**Multi-replica readiness — exact conditions (not met today):**
1. **Single-writer today; multi-replica requires externalizing every stateful feature**
   (the hub file, the pid sidecar, and the operator receipt pipeline) — the checklist's
   "externalize every enabled stateful feature" is violated by design: the hub is a
   local file with no replication or failover. Until a hosted/network store replaces
   the local file, multi-replica is **not ready**.
2. The idempotency-key + slot + pristine-head CAS pattern is portable and is the right
   foundation; the wall-clock `*_at_ms` fields must be replaced by a replica-safe
   ordering (logical sequence or store-assigned timestamp) before any cross-node claim.
3. The `immutable=1` read-only pre-check must be dropped in favor of the atomic claim
   gate (the only correct arbitration point) once replicas exist.
4. Adjudication must work before any failover story is credible (Finding 1).
5. Required evidence before claiming readiness: the Finding 2 race suite passing
   against two writers, and a mixed-version test matrix (N/N+1 binary × db version).

**Validation actually run vs inferred:** acceptance gate output, wave-admit E2E (4/4),
infrastructure suites, and the two SQL-level reproductions (TEST1/TEST2/TEST3) are
observed facts. Concurrency outcomes, migration-with-adjudicated-row, and effectful
dispatch success are inferred from code or N/A (no external endpoint); the two
reproductions make the Finding-1 failure concrete rather than inferred.
