Audit complete. Report saved to `ai-campaign/state/runs/effect-free-adjudication-command-for-hard-crash--65f1a8e7/artifacts/adversarial_review-9c87f3a7/meta/acceptance-mapping-audit.md`.

**Verdict:** the design's mapping (A1–A4 → 6 named tests + R1–R9) is substantively sound but **incomplete** — 7 reviewer findings (all BLOCKING/MEDIUM) had no named test. I ran three baselines to anchor the audit: Rust closed-world test (ok, 11 classes), Go graphterminal pair (ok), and the full CLI quarantine test (ok, 6.14s, including `build_go_core` — proving the A2 real-Core pipeline is runnable as designed).

## Validated/amended test list

**Pre-implementation (fixture-based, runnable today):**
- **P-1** (existing, amend at impl) `every_fixed_uncertainty_class_is_closed_world_and_nonretryable` — extend 11→12 with `HardCrash` (domain test, :181) → protocol F1, S-CC1
- **P-2** (existing) Go `TestTerminalReceiptFixedOutcomeMappings` — map-driven, auto-covers the hard_crash entry; `TestTerminalControlRejectsImpossibleUncertaintyEvidence` pins chronology
- **P-3** (existing) clock-skew refusals — Go `artifact_time` case + Rust receipt-time-drift test (S-F4)
- **P-4** (existing, amend) `terminalization_persists_evidence_and_releases_lane_in_one_transition` — add direct `run_version==5`/`last_event_seq==5` assertions (D-F1); store-level zero-mutation analogue of T-4
- **P-5** (existing) `path_replacement_cannot_change_the_prepared_executable` — TOCTOU machinery (S-F1)
- **P-6** (new) busy-timeout→`DispatchQuarantined` mapping via `Unavailable` store stub, `run_service.rs` sentinel pattern (D-F3)
- **P-7** (new) stored-inspection idempotence — re-entry with *replaced* authz/pricing returns the *stored* inspection (DB-reviewer §6 caveat)

**Post-implementation:**
- **T-1 (A1)** parse/usage boundaries — amended: dual-stdin rejection (S-F5), read-before-spawn ordering probe (S-F6), idempotency-key message (S-CC3), wrong-sha256 loud failure
- **T-2 (A2)** strand → adjudicate → DB post-state CAS with **real pinned Core** — amended with direct run-level assertions; feasibility verified
- **T-3 (A3)** poisoned-`OPENAI_API_KEY` test — `CREDENTIAL_SECRET` pattern, byte-identical output+state with key absent vs poisoned (S-F12)
- **T-4 (A3)** byte-identical re-adjudication zero-mutation refusal (D-F5, S-F8a)
- **T-5** digest-mismatch **preflight** refusal, distinct message, zero mutation (S-F2)
- **T-6** distinct-refusal-message assertions per cause — not-stranded / digest / old-Core-with-re-pin-hint / CAS-conflict-kept-separate; exit-code contract; no "quarantined; resend is forbidden" text (S-F8a–d, S-F9, S-F10, P-F2)
- **T-7** busy-timeout→adjudicate variant (post half of D-F3)
- **T-8** lockstep closure — P-1 extension + scheduled-family fail-closed test for `HardCrash`
- **T-9 (A4)** clippy gate

**Honest N/A (documentation-only):** D-F2 (open-time BEGIN IMMEDIATE contention), D-F4 (journal zero headroom), S-F7 (signal-orphan), S-F13 (A3 reasoning doc); design prohibitions (reuse bridge, never `PreparedDispatchDependencies::prepare`, new `adjudication_service()`) are design-text + behaviorally proven by T-3/T-1/P-5.

**One real gap flagged:** R1–R9 are named but not enumerated in the design artifact, so individual R→test traceability is unverifiable — the artifact must list them next to the mapping before implement.
