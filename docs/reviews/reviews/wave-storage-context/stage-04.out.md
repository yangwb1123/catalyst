All evidence is gathered. I have verified the full change set (commits 37bf7c1 → 2b1a618), run the gates, traced every primary file, and cross-checked against ADR-0035/0036, sprint claims, and the Stage-01 review. Here is the Stage 04 review.

---

# Stage 04 — Implementation Review: Wave-parallel storage (schema v20–v22) and orchestration

**Roles:** Staff Engineer · Tech Lead
**Prior findings:** Stage 01 (docs/reviews/reviews/forgeos-review-context/stage-01.out.md): 1 High (per-run walls), 2 Medium, 3 Low. Stages 02–03: not present in repo (N/A). Disposition of Stage-01 findings verified against current code: High Finding 1a fixed (37bf7c1), 1b fixed (v22, b56a1b5); Finding 2 fixed (v21, 35de32a); Finding 3 fixed (docstrings rewritten); Finding 4 fixed (`successorPredecessorsCovered` gone); **Finding 5 still open** (empty-ready error conflation); Finding 6 (sandbox) out of scope.

## 1. Findings

### 🟡 Finding 1 — Medium
| Field | Content |
|---|---|
| **Title** | `validate_graph_run_binding` comment claims "the first request is returned" but the loop returns the last request, and the only caller discards the value |
| **Surface** | Nested module (Rust infrastructure store) |
| **Location** | `forge-runtime/crates/infrastructure/src/sqlite_hub/group_agent_scheduled_node_provider_request/read.rs:59-71`; caller `group_agent_graph_run/read.rs:131-133` |
| **Evidence** | Comment at read.rs:60-61: "the first request is returned for the run's single-dispatch view". Code at 62-71: `let mut result = None; for stored in stored_all { … result = Some(validate_decoded(record, body, contract)?); } Ok(result)` — every iteration overwrites `result`, so the **last** row wins; `find_all_by_run` orders `created_at_ms DESC, id DESC` (rows.rs:157-171), so the survivor is the **oldest** request. The only caller (group_agent_graph_run/read.rs:131) binds the result to nothing: `…::validate_graph_run_binding(connection, &inspection, scheduled_contract.as_ref())?;` |
| **Failure scenario** | A future maintainer consumes the return as "the run's request view" (as the docstring invites) and gets the oldest node's inspection in a multi-node run; or trusts the comment while auditing the fail-closed validator |
| **Impact and likelihood | Maintainability/honesty drift in a security-sensitive store where byte-exactness is load-bearing. No current misbehavior (value unused); latent wrong-value contract. Certain that comment and code disagree |
| **Fix** | Smallest safe: change the return type to `Result<(), HubStoreError>` (the validate-all loop is the whole point), delete the "single-dispatch view" sentence, update the caller. Add a store test with two requests in one run asserting run inspection passes (already implicitly covered by `two_nodes_in_one_run_prepare_provider_requests_through_v22`) |
| **Risk/effort** | No breaking risk (value unused today); ~1h + test |

### 🟡 Finding 2 — Medium
| Field | Content |
|---|---|
| **Title** | `wave-admit` hardcodes provider/model/budget/pricing literals, including a fabricated test digest, and offers no flags to align them |
| **Surface** | Stock binary (Rust CLI → Go core) |
| **Location** | `forge-runtime/crates/interfaces/src/group_agent_graph/wave_command.rs:147-166` (`build_target_candidate`) |
| **Evidence** | Hardcoded: `--endpoint https://api.openai.com/v1/responses --model gpt-5.2 --max-output-tokens 4096 --max-model-output-bytes 65536 --max-model-events 4096 --timeout-ms 300000 --max-cost-usd-micros 1000000 --pricing-snapshot-sha256 cccc…(×64) --max-result-bytes 262144`. `gpt-5.2` appears nowhere else in the repo (grep: only wave_command.rs); the `cccc…` digest is the domain test fixture `DIGEST_C` (`crates/domain/…/dispatch_tests.rs:5`), not a registry quote digest. The Go core's own command (`command.go:17-20`, `command_args.go:75-79`) takes these as operator flags; the manual successor admit path lets the operator pass them. `parse_wave_admit` (scheduled_contract_args.rs) accepts none of them |
| **Failure scenario** | Operator runs wave-admit → candidates admitted carrying `pricing_snapshot_sha256 = cccc…`. The effectful chain (Sprint 58/59: readiness re-verifies pricing with the digest domain; dispatch execute requires explicit `--pricing FILE`) can never accept a real quote — the wave's candidates are structurally undispatchable and the CLI gives no warning. In a run whose initial contract was built with the operator's real `--model`, the wave nodes silently carry a different model/endpoint |
| **Impact and likelihood | Contract inconsistency within one Graph Run; silent dead-end for the declared orchestration path (fail-closed, no security exposure). Certain for any real pricing snapshot |
| **Fix** | Add pass-through flags `--endpoint/--model/--max-output-tokens/--max-model-output-bytes/--max-model-events/--timeout-ms/--max-cost-usd-micros/--pricing-snapshot-sha256/--max-result-bytes` (mirroring the Go core command), fail-closed when absent; thread them through `execute_wave_admit → materialize_node → build_target_candidate` via a small `WaveAdmitOptions` struct. Update `cli_usage` + tests (flag passthrough verified in the Go-core stderr/JSON) |
| **Risk/effort** | Breaking only for scripts relying on the current implicit defaults; ~0.5–1 day incl. tests |

### 🟡 Finding 3 — Medium
| Field | Content |
|---|---|
| **Title** | `wave-admit --idempotency-key` is accepted and silently dropped; every wave node gets a fresh random key, so wave-admit can never replay or resume |
| **Surface** | Stock binary (Rust CLI) |
| **Location** | `scheduled_contract_args.rs` `parse_wave_admit` (stores into `*idempotency_key`); `wave_command.rs:118-129` (`materialize_node` always calls `idempotency_key("scheduled-wave-admit")`); `args.idempotency_key` consumed only at run_command.rs:208 and scheduled_contract_command.rs:152,218 (grep-verified) |
| **Evidence** | The `WaveAdmit` command variant (`scheduled_contract_args.rs:464-470`) carries no key field; the parse function's `*idempotency_key = Some(value)` at ~line 448 has no reader on this path. `state_path::idempotency_key` is random per call (pid+nanos+counter, state_path.rs:52-57), so a retry after partial admission generates new keys and the already-admitted node lands in `rejected` instead of `Replayed` |
| **Failure scenario** | Wave of 2 nodes: node A admitted, node B conflicts. Operator re-runs wave-admit after fixing B → A is rejected as "already belongs to another idempotency key" (correct fail-closed, but the replay path — the store's core idempotency design — is unreachable from this command) |
| **Impact and likelihood | Honesty defect (declared option has no effect) + no resume capability for the multi-node orchestration. Certain |
| **Fix** | Thread the user key (or generate one) and derive a deterministic per-node key (`{key}-{node_id}`) in `materialize_node`; test: re-running wave-admit with the same `--idempotency-key` yields `Replayed` for the already-admitted node |
| **Risk/effort** | Low; ~3h incl. test |

### 🟢 Finding 4 — Low
| Field | Content |
|---|---|
| **Title** | `wave-admit` exits 0 when some wave nodes are rejected; partial failure is indistinguishable from full success by exit code |
| **Surface** | Stock binary (Rust CLI) |
| **Location** | `wave_command.rs:32-48` (per-node errors → `rejected`); `main.rs:80-98` (`run_group_agent_graph_run` returns `ExitCode::SUCCESS` for any `Ok(output)`) |
| **Evidence** | No code maps non-empty `rejected` to a non-zero exit; the JSON does carry `rejected`, so a JSON consumer can detect it, but a script keying on exit status cannot |
| **Failure scenario** | Automation admits wave 1 (2 ready nodes, one rejected), proceeds to dispatch based on exit 0, silently skips a node |
| **Fix** | Return a distinct non-zero exit code when `rejected` is non-empty (keep JSON); add a test with a conflicting node |
| **Risk/effort** | Low; ~2h |

### 🟢 Finding 5 — Low
| Field | Content |
|---|---|
| **Title** | Stale "dispatch re-entry schema version 12..=21" error message after v22 was admitted |
| **Surface** | Nested module (Rust infrastructure) |
| **Location** | `schema_contract/reentry.rs:43` |
| **Evidence** | The allowed list at reentry.rs:40 now includes 22; the message still says `12..=21` |
| **Fix** | Update the message to `12..=22` |
| **Risk/effort** | Trivial |

### 🟢 Finding 6 — Low
| Field | Content |
|---|---|
| **Title** | No test materializes ≥2 ready nodes in one wave-admit invocation; the `rejected` array and multi-node wave path are unexercised; no two-lifecycle-per-run insertion test exists |
| **Surface** | Nested module + stock binary tests |
| **Location** | `interfaces/tests/cli_group_agent_scheduled_node_wave_admit.rs` (all 3 tests admit exactly 1 node); infrastructure tests (`two_nodes_in_one_run_prepare_provider_requests_through_v22` covers requests only) |
| **Evidence** | Diamond fixture yields exactly 1 ready node per receipt set (backend zero-pred; sso blocked by frontend+backend); serial-three yields 1 per wave. A 2-ready-node wave requires a topology with two zero-pred successors sharing one consumed predecessor (e.g. `frontend → {sso1, sso2}`) — no such fixture. The v22 lifecycle per-node slot is enforced by schema/digest only; the store path inserting a second lifecycle in one run is unreachable until effectful multi-node dispatch exists. Stage-01 Finding 1 fix (c) asked for "a wave-admit E2E with two ready nodes" — delivered at store level, not at CLI level |
| **Fix** | Add a fan-out fixture (two zero-pred successors) + wave-admit test asserting `wave.len() == 2` and a conflict test asserting `rejected.len() == 1` with exit ≠ 0 (Finding 4). Note the lifecycle-slot coverage gap in the sprint docs |
| **Risk/effort** | Low; ~0.5 day |

### 🟢 Finding 7 — Low
| Field | Content |
|---|---|
| **Title** | Stage-01 Finding 5 remains open: empty ready set is reported as a planning fault |
| **Surface** | Stock binary (Go core) |
| **Location** | `forge-core/internal/graphscheduledcontract/build_successor.go:143-146` (`ReadySuccessorNodes`: `if len(ready) == 0 { return nil, errInvalidCandidate }`); `ready_command.go:49` ("cannot compute ready successor nodes", exit 1) |
| **Evidence** | Verified present in current code; not claimed fixed in Sprints 74-77 |
| **Failure scenario** | A fully-consumed graph's next wave is a legitimate empty list; the CLI reports exit 1 as if the input were corrupt |
| **Fix** | Return `[]` with exit 0 (or a distinct "graph complete" result) + a full-consumption test |
| **Risk/effort** | Trivial |

### 🔵 Finding 8 — Info
| Field | Content |
|---|---|
| **Title** | Duplicate `PRAGMA user_version = 22;` mid-batch in the v22 migration |
| **Location** | `v22_sql.rs:136` and `:225` |
| **Evidence** | All prior migrations (v16-v21) set user_version exactly once at the end. Harmless — `schema.rs` runs `execute_batch` inside `BEGIN IMMEDIATE`/`COMMIT` (schema.rs:186-200) and the rollback tests (`schema_transaction_rollback.rs`) pass — but the first occurrence is dead weight and misleading if the batch is ever split (ADR-0036 explicitly rejected splitting) |
| **Fix** | Remove the first occurrence; keep a comment noting the single-batch constraint |

### 🔵 Finding 9 — Info
| Field | Content |
|---|---|
| **Title** | Misleading comment in the wave-admit happy-path test: claims the fabricated receipt "was correctly rejected" while asserting zero rejections |
| **Location** | `cli_group_agent_scheduled_node_wave_admit.rs:60-70` |
| **Evidence** | The comment describes the drift test's behavior; in this test the receipt passes Go's digest check and is simply not carried (backend has zero direct predecessors). The assertion `rejected.len()==0` contradicts the comment's wording |
| **Fix** | Rewrite the comment to say the receipt is accepted as consumed-set evidence and not carried by the zero-receipt candidate |

### 🔵 Finding 10 — Info
| Field | Content |
|---|---|
| **Title** | Duplicate contract loaders with different depths in provider_request/read.rs |
| **Location** | `load_contract_lightweight` (read.rs:76-88) vs `load_source_contract` (read.rs:115-128) |
| **Evidence** | Both resolve a `scheduled_contract_id` across the contract and successor tables; the difference (lightweight decode vs `inspect_in_snapshot`) is deliberate — the deep variant re-validates against control/schedule and the light one prevents recursion — but nothing in the code states the invariant, so a future edit can silently weaken `validate_stored` |
| **Fix** | Add a shared doc comment or unify via a `deep: bool` parameter with the recursion rationale (ADR-0036 already records the stack-overflow incident) |

**Verified clean (no findings):** per-node identity slots now mirror the v22 UNIQUE constraints (successor 4 probes, provider-request 5 probes); replay paths re-validate byte-exactness and reject key reuse with different input; migration INSERT…SELECT cannot violate the new UNIQUEs (v18/v16 allowed ≤1 row per run); the active-lane global uniqueness (`WHERE lane_active = 1`) is preserved — execution stays serial per project lane, matching ADR-0036; Go core bounds receipts at 64 KiB before wave planning (command.go:101-121), so the wave path's missing Rust-side preflight is covered; no credentials or sensitive personal data in any observable record (digests/endpoints/flags only); `has_graph_run_child` semantics still correct under multi-row runs (Sprint 77 verified).

## 2. Gate report

| Check | Command/source | Result | Evidence | Required action |
|---|---|---|---|---|
| Volume gate | `node harness/gate.mjs` | PASS | 1264 files, ≤500 lines/file, root ≤15 | none |
| Architecture (8 checks) | `node harness/arch/arch-check.mjs` | PASS | layering/package/fanin/cognitive/anti-pattern/function-length/circular/drift-guard, 1017 files | none |
| Governance | `harness/check.py` (via acceptance) | PASS | governance integrity clean | none |
| Secret scan | `node harness/secret-scan.mjs` (via acceptance) | PASS | no hardcoded secrets (pattern scan) | none |
| Acceptance | `node harness/acceptance.mjs` | **ACCEPTED** | 9 pass · 0 fail · 2 honest N/A | none |
| Rust tests | `cargo test --all-targets --all-features` | PASS | 923 tests total; targeted re-runs: provider-request 10/10, successor 4/4, node-lifecycle 7/7, wave-admit CLI 3/3 | add Finding 6 fixtures |
| Go tests | `go test ./...` (forge-core) | PASS | 1330 tests | none |
| Lint | rust: `cargo clippy … -D warnings`; go/python: tool absent; ts: eslint unconfigured | PASS / N/A | clippy PASS; go/python/ts honestly N/A | none |
| Coverage | adapter probe | N/A | no tool configured — not a verdict (honest) | none |
| Typecheck/build | `go vet` / `cargo check` / `cargo build` | PASS | all green | none |
| SCA | OSV advisory scan | PASS | 0 known-vulnerable (5 manifests) | none |

## 3. In-scope violations vs thresholds

No 🔴 hard-gate violation: every machine-enforced check passes with no exemptions. The findings above are 🟡 规范-level (Reviewer discretion) or 📋 discipline items: comment/code drift (F1), literal ownership + option surface (F2), accepted-but-ignored flag (F3), exit-code semantics (F4), stale message (F5), coverage gaps (F6, F7), and cosmetic debt (F8–F10). Measured values: `validate_graph_run_binding` 29 lines (≤50 ✓); `wave_command.rs` 245 lines (≤500 ✓); fan-in of `scheduled_contract_command` reuse path 30 (≤30 ✓, at the budget edge by design per Sprint 73).

## 4. Refactoring plan

| Target | Extraction/change | Destination | Tests | Effort |
|---|---|---|---|---|
| `provider_request/read.rs` `validate_graph_run_binding` | Return `Result<(), …>`; drop vestigial `Option`; fix "first request" comment | same file; caller `group_agent_graph_run/read.rs:131` | existing two-node prepare test asserts run inspect passes | S (~1h) |
| `wave_command.rs` `build_target_candidate` | Extract `WaveAdmitOptions` (endpoint/model/budgets/pricing); thread from `execute_wave_admit`; parse new flags in `scheduled_contract_args.rs` | `wave_command.rs` + `scheduled_contract_args.rs` + `cli_usage.rs` | flag passthrough + missing-flag rejection tests | M (~1 day) |
| `wave_command.rs` `materialize_node` | Derive per-node key from user `--idempotency-key` (or random) | same | replay test (same key → `Replayed`; different key → conflict) | S (~3h) |
| `main.rs` / wave output path | Non-zero exit when `rejected` non-empty | `main.rs` | two-node wave with one conflict | S (~2h) |
| `reentry.rs:43` | Message `12..=21` → `12..=22` | same | none needed (message only) | S |
| Wave-admit E2E | Fan-out fixture (frontend → sso1, sso2) + 2-ready-node wave test | `interfaces/tests/` + `group_agent_graph_run_support` | `wave.len()==2`; `rejected` path | M (~0.5 day) |
| Test scaffolding | Remove `_unused`/`#[allow(dead_code)]` + stale comment | `cli_group_agent_scheduled_node_wave_admit.rs` | existing suite | S |
| `v22_sql.rs` | Remove first `PRAGMA user_version = 22` | same | schema rollback tests | S |
| Go `ReadySuccessorNodes` | Empty ready → empty list, exit 0 | `build_successor.go` + `ready_command.go` | full-consumption test | S |

## 5. Proposed interface signatures (only if interface changes)

The only required interface change is the wave-admit option surface (Finding 2):

```rust
// scheduled_contract_args.rs — parse_wave_admit additions (fail-closed: required)
--endpoint <HTTPS_URL>  --model <MODEL>  --pricing-snapshot-sha256 <SHA256>
// optional budget mirrors of the Go core command:
--max-output-tokens N  --max-model-output-bytes N  --max-model-events N
--timeout-ms N  --max-cost-usd-micros N  --max-result-bytes N

// wave_command.rs — internal plumbing (no public surface change):
pub(crate) struct WaveAdmitOptions { endpoint: String, model: String, /* … */ }
fn build_target_candidate(go_core, control, node_id, receipts, schedule_sha256,
    options: &WaveAdmitOptions) -> Result<Vec<u8>, Box<dyn Error>>
```

No store or domain signatures need to change; `validate_graph_run_binding` becomes `Result<(), HubStoreError>` (internal, `pub(in crate::sqlite_hub)`).

## 6. Technical-debt table

| Severity | Owner | Disposition | Reason |
|---|---|---|---|
| Medium (F1) | Rust store | Fix in this slice | False docstring in fail-closed validator; vestigial return invites misuse |
| Medium (F2) | Rust CLI | Fix in this slice | Hardcoded model/pricing make wave-admit candidates structurally undispatchable; cross-node inconsistency |
| Medium (F3) | Rust CLI | Fix in this slice | Declared option silently dropped; no replay/resume |
| Low (F4) | Rust CLI | Fix in this slice | Partial failure invisible to exit-code consumers |
| Low (F5) | Rust store | Fix in this slice | Stale range message |
| Low (F6) | Rust tests | Fix in next slice (after F2/F4) | Needs fan-out fixture; CLI-level multi-node wave unproven; lifecycle per-node slot insertion untested (unreachable until effectful multi-node dispatch) |
| Low (F7) | Go core | Deferred (documented) | Carried from Stage-01; orthogonal to this slice |
| Info (F8–F10) | Rust | Fix opportunistically | Cosmetic; no behavior impact |

## 7. Recommendation

**Overall readiness:** high. The Stage-01 High finding is fully closed at store level (per-node candidate slots + v22 request/lifecycle slots, both schema- and store-tested, fail-closed throughout), ADR-0035/0036 are accurately implemented, all 923 Rust + 1330 Go tests and every hard gate pass, and the sprint documentation is honest (including the effectful-execution deferral). The v22 migration is data-preserving, single-batch, digest-locked, and rollback-tested.

**Ship decision: yes, with the three Medium items as must-fix preconditions** (they are small, contained, and all in the wave-admit CLI surface and one store function). F1–F3 do not block the storage slice itself (all store invariants are correct), but they block honest delivery of the *orchestration command*: wave-admit currently fabricates pricing evidence (F2), cannot be resumed (F3), and its doc contract is wrong (F1).

**Critical/High counts:** 0 Critical · 0 High (all prior High findings verified fixed).

**Must-fix:** F1 (return type + comment), F2 (wave-admit option flags, remove hardcoded literals), F3 (idempotency-key plumbing). Recommended in-slice: F4 (exit code), F5 (message), F6 (fan-out fixture).

**Explicitly deferred:** F7 (empty-ready semantics, carried from Stage-01); effectful multi-node dispatch and the two-lifecycle-per-run insertion test (F6 tail) until the next effectful slice; F8–F10 cosmetic.

**Validation run vs inferred:** observed — `forge accept` ACCEPTED (9 pass/0 fail/2 N/A); gate.mjs PASS; arch-check 8/8 PASS; targeted test binaries re-run (provider-request 10/10, successor 4/4, node-lifecycle 7/7, wave-admit CLI 3/3); code trace of all eight primary files, v16/v18/v20/v21/v22 SQL diffs, migration transactionality (schema.rs), identity probes vs UNIQUE constraints, receipt bounding in Go, and the idempotency-key/exit-code paths. Inferred — none of the machine gates cover the multi-node wave-admit CLI path (no such test exists, F6), and the two-lifecycle insertion path is schema-only; both are stated as gaps, not passes.
