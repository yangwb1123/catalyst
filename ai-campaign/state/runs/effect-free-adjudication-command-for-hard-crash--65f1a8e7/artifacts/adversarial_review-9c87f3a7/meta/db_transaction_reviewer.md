All evidence gathered. Here is the review.

---

# Review: terminalize CAS SQLite transaction semantics (vs schema v23)

**Scope:** `forge-runtime/crates/infrastructure/src/sqlite_hub/group_agent_node_lifecycle/terminalize.rs` (v4 family), `group_agent_scheduled_node_lifecycle/{terminalize,adjudicate}.rs` (scheduled family), schema v12→v23 SQL, the readback validation chain in `domain`, Core's `graphterminal/receipt.go`, and the application-level `group_agent_node_dispatch_execution/service.rs`. Every design claim below was checked against current code, not the summary text.

## 1. Claim-by-claim verification (all confirmed)

| Claim | Verdict | Evidence |
|---|---|---|
| BEGIN IMMEDIATE | ✅ | `transaction_with_behavior(TransactionBehavior::Immediate)` + `busy_timeout(5s)` per call (`terminalize.rs:41–48`); identical pattern in `claim.rs:30–38`, scheduled `terminalize.rs:23–28`, scheduled `adjudicate.rs:19–26` |
| `ensure_claim_source` exact-equality | ✅ | `terminalize.rs:60–75`: `claim == control.claim ∧ active_lane == control.active_lane ∧ artifact none ∧ receipt none ∧ run == control.graph_run ∧ plan == control.plan ∧ events == journal_events` |
| Guarded transition | ✅ | `terminalize.rs:180–218`: `UPDATE ... SET run_version=5,status=?1,last_event_seq=5,journal_bytes=journal_bytes+?2 WHERE id=?3 AND run_version=4 AND status='dispatch_unknown' AND execution_contract_present=1 AND dispatch_request_present=1 AND dispatch_authority_released=1 AND last_event_seq=?4 AND EXISTS(SELECT 1 ... seq=?4 AND event_sha256=?5)`; `changed==1` else `Conflict` |
| `release_lane` exact DELETE | ✅ | `terminalize.rs:220–246`: 5-key DELETE (`lane_ownership_id, graph_run_id, dispatch_id, project_lane_sha256, claim_event_sha256`), `changed==1` else `Conflict` |
| `reject_terminal_replay` | ✅ | `terminalize.rs:49–58`: artifact/receipt row present → `Conflict("already terminal")` **before any write** (zero mutation) |
| Core `ExpectedLastEventSeq: 4` + deterministic `failed_uncertain` | ✅ | `graphterminal/receipt.go:50` (`ExpectedLastEventSeq: 4, ExpectedLastEventSHA256: claim.ClaimEventSHA256`), `receiptOutcome` uncertainty→`failed_uncertain` (:63–70); Rust lockstep `read.rs:270–296` (`expected_last_event_seq == 4`) |
| No-stream-observed uncertainty valid in both validators | ✅ | `artifact_validation.rs:128`: `provider_poll_started ∨ (¬terminal_seen ∧ ¬stream_eof_seen)`; Go `validReceiptHeader`/`kindValid` agree |
| Schema v23 current; no migration needed | ✅ | `mod.rs` `CURRENT_SCHEMA_VERSION = 23`; v13–v22 touch node-lifecycle tables only via FK references (v13:6, v16:6, v22:15/141); v23 rebuilds only the **scheduled** lifecycle table |

## 2. WAL contention

**Configuration:** WAL + `synchronous=FULL` + `secure_delete` (`schema.rs:161–164`); open-time busy 250 ms + 5 s/10 ms retry loop for `DatabaseBusy|DatabaseLocked` (`schema.rs:48–78`); per-write-op busy 5 s.

**Real amplifier — reads take the write lock.** Every `connect()` (every store method, *including every read*) runs `migrate_or_validate` → `BEGIN IMMEDIATE` even when the DB is already v23 (`schema.rs:196–212`); `validate_locked_schema` then runs catalog + per-table structural-signature + index-inventory validation under that lock. So every operation is a transient write-lock contender. In practice all writer transactions are short (single CAS transaction, ~6 statements), so the 250 ms open budget plus the 5 s retry loop absorbs stragglers; risk is low but structural and undocumented.

**Contention outcome under live-executor race:** two terminalizes of one run serialize on `BEGIN IMMEDIATE`; the loser either (a) wins the lock after the winner commits and fails `reject_terminal_replay` → `Conflict`, or (b) times out at 5 s → `SQLITE_BUSY` → `HubStoreError::Unavailable` (`full_contract.rs:441–456`). The service maps **both** to `DispatchQuarantined` (`service.rs:207–211`) — safe, no lost update, but the design's failure-mode #1 ("live-executor race → CAS conflict") should note the busy-timeout variant also strands the claim as quarantined.

## 3. Deadlock

**Deadlock-free by construction.** One connection per operation (`self.connect()?` per store method, `mod.rs:60–78`), one transaction per connection, zero nesting (all terminalize helpers take the passed `Transaction`), no lock-ordering (a single lock in SQLite WAL), readers never hold the write lock. The only blocking primitive is busy-wait bounded by timeout. The application-layer TOCTOU (re-inspect on a separate Deferred connection after the network call, `service.rs:169–175`, then terminalize on a fresh connection) is closed by the DB CAS itself — the losing executor's terminalize hits `reject_terminal_replay`/CAS inside the winner's committed state. No corruption path found.

## 4. Post-state assertion completeness vs schema v23

**Direct assertion (`ensure_persisted`, `terminalize.rs:248–266`) omits run-level fields** — no explicit check of `status == terminal_status(...)`, `run_version == 5`, `last_event_seq == 5`, `journal_bytes` delta, or event count. **However, completeness holds transitively**, because the readback inspection passes through `reconstruct` which enforces:

- `load_events`: event count == `last_event_seq` **and** Σ event JSON bytes == `journal_bytes` (`group_agent_graph_run/read.rs:232–247`) — the `journal_bytes+?2` arithmetic and `last_event_seq=5` are re-verified by re-summation;
- `valid_terminal_record_state`: `v==5 ∧ terminal status ∧ last_event_seq==5 ∧ present-flags` (`group_agent_graph_run/terminal_validation.rs:12–29`);
- `validate_state_shape`: terminal status ⇔ lane absent ∧ artifact ∧ receipt (`read.rs:307–325`);
- `inspection.validate()` → `validate_terminal`: **`run.status == receipt.graph_status` (exact value)**, `run.v == TERMINAL_VERSION`, JSON exactness, `validate_terminal_event` (seq-5 event binds claim/artifact/receipt, `lane_released=true`, `!retry_authorized`), `validate_terminal_time` (`inspection_validation.rs:57–89`; `read.rs:297–306`);
- `decode_*`: row-vs-blob exact equality for every stored column (`read.rs:88–190`).

**Schema v23 consistency:** all terminalize SQL targets v12-shaped tables (runs, events, lane ownerships, terminal artifacts/receipts, claims) untouched by v13–v23. The events table's per-seq CHECK (`seq=5 ⇒ event_version=5 ∧ kind='node_lifecycle_terminalized'`, `v12_sql.rs:104–150`) matches `insert_event`'s hardcoded values (`terminalize.rs:120–139`); the runs compound CHECK accepts exactly the v5 shape written by `transition_run` (`v12_sql.rs:70–95`). The schema would reject a partial/wrong transition — status set to any non-terminal value fails the CHECK, `last_event_seq≠5` fails, journal overflow fails the 327 680 cap. **Edge:** v4 journal ≤ 262 144 + max event 65 536 = exactly 327 680 — a max-size terminal event lands exactly on the cap, zero headroom but valid.

**Finding (low):** the run-level assertions live only in the transitive readback chain, not in `ensure_persisted`. A reader of `terminalize.rs` alone cannot see that status/run_version/last_event_seq/journal_bytes are verified. Recommend adding `inspection.graph_run.run.status == terminal_status(...)`, `run.v == 5`, `last_event_seq == 5` to `ensure_persisted` (or its doc comment) so the CAS post-state is locally self-evident.

## 5. Re-adjudication refusal

- **v4 terminalize re-entry:** `reject_terminal_replay` → `Conflict` before any write, inside the IMMEDIATE transaction — zero mutation; tested (`sqlite_group_agent_node_lifecycle.rs:76–110`: second terminalize → `Conflict`, counts unchanged `[1,0,1,1]`, 5 events).
- **Scheduled adjudicate:** refuses `Conflict` pre-write when `status≠'claimed' ∨ lane none ∨ evidence present` (`adjudicate.rs:32–41`); a re-adjudication after success sees `status='adjudicated'` → refused; zero mutation. v23 CHECK enforces `adjudicated ⇒ lane_active=0 ∧ adjudicated_at_ms` (`v23_sql.rs:60–66`); regression test `adjudicate_update_columns_and_status_are_live_through_v23`.
- **Caveat (important):** the design's *new* `GroupAgentNodeDispatchAdjudicationService` for the v4 family **does not exist in the repository** — only its scheduled-family analogue and `reject_terminal_replay`. "Re-adjudication zero-mutation refusal" for the new command is design-stage; the requirements claim "`reject_terminal_replay` = re-adjudication rejection" is accurate only for terminalize re-entry.
- **Asymmetry to resolve in the design:** scheduled *terminalize* treats post-terminal re-entry as an Ok no-op returning the stored inspection (`terminalize.rs:37–45`), while v4 terminalize refuses with `Conflict`. The new adjudication service must pick one convention and document it.

## 6. Execute re-entry idempotence

Confirmed and tested, with one caveat:

- `service.rs:31–40`: any existing lifecycle (status ∈ {DispatchUnknown, Completed, Failed, FailedUncertain}) → `AlreadyClaimed` with **zero effects** (no credential read, no provider build, no network); `AwaitingDispatchAuthorization` → proceeds.
- Claim CAS yields exactly one `Claimed` under concurrency (`claim.rs:69–74`); tests: `concurrent_claimants_have_one_authority_and_one_non_replay_result` (infra), `concurrent_execute_streams_the_durable_claim_exactly_once`, `completed_dispatch_terminalizes_and_reinvocation_never_resends`, `core_failure_quarantines_claim_and_reinvocation_never_resends` (application).
- **Caveat:** idempotence is keyed on `graph_run_id`, not on request equality — a re-entry carrying *different* authorization/pricing after terminalization silently returns the stored terminal inspection. Consistent with the "never resends" contract, but the design's idempotence test should assert that the *stored* inspection (not the new inputs) is returned.

## 7. Findings summary

1. **(Low)** `ensure_persisted` local assertion gap — run-level fields verified only transitively via `reconstruct`'s validation chain; localize them.
2. **(Low/info)** Open-time `BEGIN IMMEDIATE` on every connection, including reads, is an undocumented write-lock contention amplifier; retry loop absorbs it today.
3. **(Info)** Busy timeout after 5 s surfaces as `Unavailable` → `DispatchQuarantined`; add this variant to the design's live-executor failure mode.
4. **(Info)** `journal_bytes` zero headroom at max-size terminal event (262 144 + 65 536 = 327 680 exact).
5. **(Design-stage)** The v4 adjudication service and its "re-adjudication zero-mutation refusal" are unimplemented; existing analogues verified sound, but the re-entry-convention asymmetry (scheduled Ok-noop vs v4 Conflict) must be settled before implementation.

## Verdict

The terminalize CAS claims are **all substantively accurate against the code and schema v23**: BEGIN IMMEDIATE with 5 s busy timeout, exact-equality `ensure_claim_source`, guarded `transition_run` (run_version=4/status/last_event_seq=?4/claim-head digest), exact DELETE lane release, and pre-write `reject_terminal_replay` — sound under WAL, deadlock-free by construction, and post-state assertions complete (transitively). Re-adjudication refusal and execute re-entry idempotence claims hold for every *existing* path and are tested. Residual items are low-severity documentation/localization gaps plus one genuinely design-stage area: the v4 adjudication service itself does not yet exist, and its refusal convention should mirror one of the two existing (asymmetric) re-entry patterns.
