# Acceptance-mapping audit — A1–A4 → named tests, vs. all reviewer findings

Auditor: fresh-context agent. Inputs: design artifact (task-1-design.md summary), the four
adversarial reports (security_reviewer, db_transaction_reviewer, protocol_reviewer +
verify-cross-language-invariants.md), and the actual suites listed below. Every named test was
checked against the suite it claims to follow, and three baseline runs were executed:

- `cargo test -p forge-runtime-domain --offline --test group_agent_node_lifecycle
  every_fixed_uncertainty_class_is_closed_world_and_nonretryable` → **ok** (11 classes today)
- `go test ./internal/graphterminal -run 'TestTerminalReceiptFixedOutcomeMappings|TestTerminalControlRejectsImpossibleUncertaintyEvidence'` → **ok**
- `cargo test -p forge-runtime-cli --offline --test cli_group_agent_node_dispatch_execute
  v4_reinvocation_reports_quarantine_without_inputs_core_consent_or_credential` → **ok** (6.14 s,
  includes `build_go_core` of the real pinned Go Core) — proves the A2 fixture pipeline is
  real-Core-runnable end to end.

Verdict: the design's mapping (A1–A4 → 6 named tests + derived R1–R9) is **substantively
sound but incomplete**. Seven reviewer findings (all BLOCKING/MEDIUM) have no named test:
Rust closed-world lockstep extension, poisoned-key A3, digest-mismatch preflight refusal,
busy-timeout variant, byte-identical re-adjudication refusal (named but underspecified),
stored-inspection idempotence (named but unasserted), distinct-refusal messages (untested).
Amended list below; every reviewer finding is traced to exactly one test (or an honest N/A).

---

## Part 1 — Amended named test list

Legend: **[PRE]** = runnable today, fixture-based, no new product code (may land before
implementation; runs green now). **[POST]** = requires the new `adjudicate` command / service /
`hard_crash` variant. Pattern column cites the suite file the test must mirror.

### A. Pre-implementation (fixture-based)

| ID | Name | Pattern (verified) | Real Core? | Finding trace |
|---|---|---|---|---|
| P-1 **[PRE, existing, amend at impl]** | `every_fixed_uncertainty_class_is_closed_world_and_nonretryable` — closed-world list **extended 11→12 with `HardCrash`** + `!retry_authorized` | `crates/domain/tests/group_agent_node_lifecycle.rs:181` (explicit class list; `uncertainty()` helper at :351; runs green today, verified) | no (domain unit) | P-F1 (protocol reviewer Finding 1 — the required hardening); S-CC1 |
| P-2 **[PRE, existing]** | Go `TestTerminalReceiptFixedOutcomeMappings` — iterates `uncertaintyClasses` map, so the hard_crash map entry is **auto-covered** for the deterministic `failed_uncertain` outcome; `TestTerminalControlRejectsImpossibleUncertaintyEvidence` pins the chronology (unchanged for hard_crash) | `forge-core/internal/graphterminal/terminal_test.go:58` / `:90` (runs green, verified) | no (Go unit) | P-F1/P-F3, S-CC1 |
| P-3 **[PRE, existing]** | Clock-skew refusals: Go `TestTerminalControlRejectsBindingAndCostDrift` "artifact_time" (artifact `created_at_ms < claim.released_at_ms`); Rust `terminal_receipt_time_drift_makes_lifecycle_and_graph_run_reads_fail_closed` (`sqlite_group_agent_node_lifecycle.rs`) | terminal_test.go:149-150; sqlite_group_agent_node_lifecycle.rs:177 | no | S-F4 (documented refusal cause — already covered on both sides) |
| P-4 **[PRE, existing, amend]** | `terminalization_persists_evidence_and_releases_lane_in_one_transition` — re-entry `Conflict` + `assert_counts([1,0,1,1], 5, 1)` = the **store-level zero-mutation refusal analogue** of T-4. **Amendment (D-F1):** add direct post-state assertions `run_version==5`, `last_event_seq==5` (status/`FailedUncertain`/lane-none already asserted; `ensure_persisted` only verifies these transitively via `reconstruct`) | `crates/infrastructure/tests/sqlite_group_agent_node_lifecycle.rs:76-110` + `assert_counts` :243 | no (infra) | D-F1, D-F5, S-F8a |
| P-5 **[PRE, existing]** | `path_replacement_cannot_change_the_prepared_executable` — the sealed-memfd TOCTOU machinery adjudicate **reuses unchanged** (design must state the prohibition, not re-test) | `crates/infrastructure/src/core_terminal_bridge/pinned_executable.rs:138` | no | S-F1 (machinery regression) |
| P-6 **[PRE, new]** | Busy-timeout→`DispatchQuarantined` mapping (execute path): store wrapper whose `terminalize_group_agent_node_dispatch` returns `HubStoreError::Unavailable{sentinel}` → service returns `DispatchQuarantined`, zero provider/credential side effects, claim still `dispatch_unknown` | app `run_service.rs` `assert_store_failure` sentinel pattern (`:149-158`); `ExecutionHarness` (application/tests/group_agent_node_dispatch_execution_support/ports.rs) | no | D-F3 (the 5 s real `SQLITE_BUSY` path itself stays documented; a full-latency lock-hold test is honest-N/A'd, see Part 3) |
| P-7 **[PRE, new]** | Stored-inspection idempotence: re-invoke `execute` after terminalization with **replaced authorization/pricing bytes** → returns the **stored** terminal inspection (CLI already inspects before reading inputs, `dispatch_command.rs:59-64`), `state_bytes()` unchanged | `cli_group_agent_node_dispatch_execute.rs` (`v4_reinvocation…` + `replace_authorization`/`replace_pricing` fixture methods); extend `completed_dispatch_terminalizes_and_reinvocation_never_resends` (app) | yes (CLI) | DB-reviewer §6 caveat (idempotence keyed on graph_run_id, not request equality) |

### B. Post-implementation

| ID | Name | Pattern (verified) | Real Core? | Finding trace |
|---|---|---|---|---|
| T-1 **(A1)** | Parse/usage boundaries — new `help_and_argument_failures…`-style test: usage line in help; missing required flags; unknown subcommand; **`--authorization - --pricing -` dual-stdin rejection**; `--idempotency-key` → "GRAPH_RUN_ID owns the single dispatch claim" (needs `Adjudicate` arm in `dispatch_claim_key_error`, `args.rs:170-181`); wrong `--core-bin-sha256` → loud pre-mutation failure; **read-before-spawn ordering probe** (`--authorization -` + empty stdin + nonexistent core-bin on a stranded claim → authorization-parse error surfaces, not a core-bin error, proving stdin is consumed before `PinnedCoreTerminalBridge::new`) | `cli_group_agent_node_dispatch_execute.rs:10-46` + `missing_source_fails_before_core_credential_or_result_disclosure`; dual-stdin precedent `cli_group_agent_scheduled_node_dispatch_readiness.rs:383` ("cannot both read from stdin"); execute's own `dispatch_execute_args.rs:59` | yes (CLI) | S-F5, S-F6, S-F1, S-CC3 |
| T-2 **(A2)** | Fixture strand → `adjudicate` → DB post-state CAS assertions: `Fixture::new()` → `strand_v4_with_local_provider()` (RejectingCore strand, `quarantine.rs:28-60`) → real pinned Core via `build_go_core` (`mod.rs:371`, verified) → `adjudicate` CLI. Assert stdout Execution shape: `graph_status=="failed_uncertain"`, `lane_active==false`, `retry_authorized==false`, `dispatch_performed_this_invocation==false`, `database_written_this_invocation==true` (mirrors `assert_quarantine_output`); then store-level post-state: `assert_counts([1,0,1,1], 5, 1)` + **direct** `run_version==5`/`last_event_seq==5` (D-F1 amendment), artifact+receipt present, lane row deleted | `assert_quarantine_output` (`cli_group_agent_node_dispatch_execute.rs:227-241`); `assert_counts` (`sqlite_group_agent_node_lifecycle.rs:243`) | **yes — real pinned Core** (feasibility verified: sibling CLI test built and ran the Go Core in 6.14 s) | A2; S-CC2; D-F1; D-§4 |
| T-3 **(A3)** | No-credential effect-freedom with **poisoned `OPENAI_API_KEY`**: adjudicate twice — (a) key absent, (b) `OPENAI_API_KEY=CREDENTIAL_SECRET` (`mod.rs:36` sentinel) → both succeed, **byte-identical stdout and byte-identical `state_bytes()`**; proves the key is never read (behavioral proof of the structural no-provider constructor + the `adjudication_service()` wiring prohibition — never `PreparedDispatchDependencies::prepare`, S-F3/S-F11) | `CREDENTIAL_SECRET` + env plumbing (`cli_group_agent_node_dispatch_execute.rs:29,69`; `mod.rs` `execute_with_result_visibility`) | yes (CLI) | S-F12 (MEDIUM, explicitly demanded); S-F3; S-F11 |
| T-4 **(A3)** | Byte-identical re-adjudication zero-mutation refusal: re-run the identical adjudicate command after success → `AdjudicationRefused` ("already terminal"), `state_bytes()` byte-identical, claim/lane/artifact rows unchanged | `v4_reinvocation_reports_quarantine_without_inputs_core_consent_or_credential` (state-before/after pattern) + P-4 store analogue | yes (CLI) | D-F5 (design-stage: v4 picks the Conflict convention over the scheduled Ok-noop), S-F8a |
| T-5 **(A3)** | Digest-mismatch **preflight** refusal: `replace_authorization`/`replace_pricing` then adjudicate → distinct "authorization/pricing body does not match the claimed digest" refusal **before any bridge invocation**, zero mutation, exit per contract | `assert_safe_failure` (`cli_group_agent_node_dispatch_execute.rs:247-253`); preflight check is `decode_authorization` + `validate_claim_against_sources` (domain validation.rs:171) | yes (CLI) | S-F2 (BLOCKING) |
| T-6 **(A3)** | Distinct-refusal-message assertions **per cause**: (a) not-stranded/already-terminal, (b) digest mismatch (=T-5), (c) old pinned Core at decide → `AdjudicationRefused` **with re-pin hint** (pre-`hard_crash` Core passes the version-1 handshake and fails at `decide` — P-F2), (d) CAS conflict from a live-executor race → stays the store `Conflict` error, retryable, **not** `AdjudicationRefused`. Assert: exit-code contract honored (scheduled precedent = exit 1, message-only — `main.rs:83-98`; S-F9); none of the messages contain execute's "durably claimed and quarantined; resend is forbidden" (S-F10) | `assert_safe_failure`-style per-cause assertions; (d) concurrent-terminalize pattern (`concurrent_claimants_have_one_authority…`, infra; `concurrent_execute_streams_the_durable_claim_exactly_once`, app) | yes (CLI) | S-F8 (BLOCKING, all four sub-causes), S-F9, S-F10, P-F2 |
| T-7 **(A3)** | Busy-timeout → adjudicate variant (post-implementation half of D-F3): `Unavailable` from the terminalize CAS → adjudicate refusal/quarantine variant with a message **distinct from CAS conflict**, zero mutation, claim stays `dispatch_unknown` (adjudicable on retry) | P-6 store-stub pattern | no (app) | D-F3 |
| T-8 **(cross)** | Lockstep closure: (a) P-1 extension lands with the enum variant; (b) **scheduled-family fail-closed**: `HardCrash` rejected by `group_agent_scheduled_node_lifecycle_terminal_validation.rs:104-125`'s own closed-world `matches!` (new small domain test); (c) Go map entry auto-covered by P-2. Both asymmetry directions loud + zero-mutation (Hub refuses control; Core refuses via `errInvalidControl`) | domain closed-world test pattern; scheduled validator matches! list | no | P-F1, P-F3, S-CC1 (4 touch points: Rust enum mod.rs:51, `valid_uncertainty` artifact_validation.rs:112, Go map validate_artifact.go:9, outcome mapping unchanged) |
| T-9 **(A4)** | `cargo clippy --all-targets --all-features --offline -- -D warnings` — final gate | gate verbatim from acceptance | — | A4 |

## Part 2 — A1–A4 → test traceability (design's own mapping, validated)

| Acceptance | Design's named test | Audit verdict |
|---|---|---|
| A1 subcommand + cli_usage listing | T-1 parse/usage boundaries | **Validated + amended** (adds dual-stdin S-F5, ordering probe S-F6, idempotency-key S-CC3, wrong-sha256 S-F1) |
| A2 stranded claim → failed terminal, seq/head CAS, lane released | T-2 fixture strand → adjudicate → DB post-state | **Validated + amended** (adds direct run_version/last_event_seq assertions per D-F1; real-Core runnability verified) |
| A3 effect-free + re-adjudication rejected | T-3 no-credential effect-freedom, T-4 byte-identical re-adjudication refusal | **Validated + amended** (T-3 must be the poisoned-key variant S-F12; T-4 must assert byte-identical state) |
| A4 clippy clean | T-9 clippy | Validated |

Derived R1–R9: the design artifact names them but does **not enumerate** them, so individual
R→test mapping cannot be audited (honest gap — see Part 3). The 6+9 named tests above are the
runnable projection of A1–A4; R1–R9 must be listed next to their tests in the design artifact
before implement so the mapping is checkable.

## Part 3 — Honest N/A (findings with no runnable test; documentation/design text only)

- **D-F2** (open-time `BEGIN IMMEDIATE` on every connection is an undocumented write-lock
  contention amplifier): no test — existing WAL behavior is covered by
  `hot_wal_v4_reinvocation_reads_the_claim_without_logical_writes_or_resend`; the design's
  failure-mode list must document it.
- **D-F4** (journal_bytes zero headroom at max-size terminal event, 327 680 exact): no test —
  boundary is valid; document.
- **S-F7** (signal-orphan: SIGINT orphans the effect-free ≤15 s Core child; v4 has no
  `cancel_on_os_signal`): no test — pre-existing execute behavior; document in the failure list.
- **S-F13** (A3 reasoning "no provider, no credential, therefore no send; DB writes confined to
  the terminalize CAS"): documentation.
- **S-F1/S-F3/S-F6/S-F11 design prohibitions** (reuse bridge unchanged; never
  `PreparedDispatchDependencies::prepare`; read-before-spawn; new `adjudication_service()`):
  enforced by design text + review, **behaviorally proven** by T-3 (key never read), T-1 (sha256
  + ordering probes), P-5 (TOCTOU machinery). A full-latency 5 s lock-hold busy test is
  N/A'd; the mapping (SQLITE_BUSY → `Unavailable` → quarantined/refused) is tested at the stub
  level (P-6/T-7).

## Part 4 — Required-test inventory (user-mandated items, all present)

1. Rust closed-world lockstep extension → **P-1/T-8a**
2. Poisoned-`OPENAI_API_KEY` no-credential A3 test (CREDENTIAL_SECRET pattern) → **T-3**
3. Digest-mismatch preflight refusal test → **T-5**
4. Busy-timeout→`DispatchQuarantined` variant → **P-6** (execute, pre) + **T-7** (adjudicate, post)
5. Byte-identical re-adjudication zero-mutation refusal → **T-4** (+P-4 store analogue)
6. Stored-inspection idempotence assertion → **P-7**
7. Distinct-refusal-message assertions per cause → **T-6** (a–d + exit code + no-DispatchQuarantined-text)

## Part 5 — Verification evidence (what was actually run/checked)

- Suite patterns verified by reading: `crates/infrastructure/tests/sqlite_group_agent_node_lifecycle.rs`
  (CAS post-state + `assert_counts`), `crates/domain/tests/group_agent_node_lifecycle.rs` (closed-world
  list at :181, `uncertainty()` helper, shared golden fixture), `forge-core/internal/graphterminal/terminal_test.go`
  (map-driven outcome + strict-wire rejections), `cli_group_agent_node_dispatch_execute.rs` +
  `group_agent_node_dispatch_execution_cli_support/` (Fixture, `CREDENTIAL_SECRET`, `strand_v4_with_local_provider`,
  `build_go_core`, `assert_safe_failure`/`assert_quarantine_output`).
- Real pinned Core: `go build ./cmd/forge` OK; full CLI quarantine test (builds Core inside
  `Fixture::new()`) passed in 6.14 s → T-2's real-Core pipeline is runnable as designed.
- Targeted test runs: Rust closed-world (ok, 11 classes), Go graphterminal pair (ok) → P-1/P-2
  baselines green; their extensions are implementation steps, correctly tagged.
