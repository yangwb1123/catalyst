# Design-gate verification — effect-free `dispatch adjudicate`

I independently re-verified the amended design (`task-1-design.md`, 388 lines) against the current tree and the three adversarial reviews. Every code-level claim I spot-checked holds exactly.

## Independent code spot-checks (all confirmed)

| Claim | Check result |
|---|---|
| `terminalize.rs` CAS: BEGIN IMMEDIATE + 5 s busy timeout; `reject_terminal_replay` before first write; `transition_run` guarded on `run_version=4 ∧ status='dispatch_unknown' ∧ execution_contract_present=1 ∧ dispatch_request_present=1 ∧ dispatch_authority_released=1 ∧ last_event_seq=?4 ∧ EXISTS(seq=?4 ∧ event_sha256=?5)`; `release_lane` exact 5-key DELETE | ✅ read verbatim at `terminalize.rs:41–48, 49–58, 214–253, 255–283` |
| `HardCrash` absent in **both** languages; Rust enum has 13 variants, Go `uncertaintyClasses` has 11 entries; `receiptOutcome` class-agnostic `uncertainty→failed_uncertain`; `ExpectedLastEventSeq: 4` hardcoded (`receipt.go:50`) with Rust lockstep (`terminal_validation.rs:178`) | ✅ `rg HardCrash/hard_crash` → empty; both validators read verbatim |
| Closed-world test at `domain/tests/group_agent_node_lifecycle.rs:181` with explicit 11-class list; `valid_uncertainty` `matches!` non-exhaustive | ✅ read verbatim — protocol F1 is real |
| `strand_v4_with_local_provider` fixture: RejectingCore → asserts `DispatchQuarantined` (A2 basis) | ✅ read verbatim |
| `args.rs:36` `pub enum Command`; `dispatch_claim_key_error`/`accepts_idempotency_key` slots; `parse_run_dispatch` has no `adjudicate` yet (additive) | ✅ read verbatim |
| `error.rs:18–19` `DispatchQuarantined` "durably claimed and quarantined; resend is forbidden"; `service.rs:195–198` blanket Core→`DispatchQuarantined` mapping | ✅ read verbatim — design's "never copy" prohibition is meaningful |
| `cli_usage.rs:205/212` dead-end sentence (minor drift from cited 202/211, disclosed) | ✅ |

## Blocking-finding resolution matrix (all resolved, none silently dropped)

- **Protocol F1** (closed-world extension): mandated "in the SAME change" — §4, §7.7, §9 A-hardcrash + A-scheduled-fence. **Resolved.**
- **DB design-stage** (re-entry asymmetry): settled to REFUSE with 4-point rationale and a convention table — §7.1. **Resolved.**
- **DB low** (`ensure_persisted` run-level assertions) → §7.6 mandated; **DB infos** (busy-timeout → §7.5 + failure #1b; `BEGIN IMMEDIATE` on reads + journal cap → §6/§10 documented residuals); **DB §6 caveat** (stored-inspection idempotence) → §7.8 + A5. **Resolved.**
- **Security's 7 spec gaps** (all design-gate blocking): bridge reuse/no hash-then-run → §7.3; preflight digest cross-check → §7.3 + flow step 3; never `PreparedDispatchDependencies::prepare` + new `adjudication_service()` → §7.3/§3; dual-stdin + read-before-spawn → §7.4; four distinct `AdjudicationRefused` causes + exit-code contract + no `DispatchQuarantined` copy → §7.2; poisoned-`OPENAI_API_KEY` A3 → §9 A3; lockstep 4 touch points → §4/§7.7. **Resolved.**
- **Test-plan's 7 untested findings**: each maps to a named test (P-1…P-7 pre-implementation, T-1…T-9 post-implementation; design §9's own A1–A6 includes them). **Resolved.**

## One residual item — assessed, non-blocking

The test-plan audit's sole remaining flag: R1–R9 are named but not individually enumerated in the design artifact (traceability nicety "must be listed … before implement"). The design §9 states R1–R9's full substance — protocol v1 unchanged; scheduled family untouched; execute re-entry untouched; Linux-only documented; A2 fixture strand path unchanged; plus the three audit-doc honesty steps — and the audit's Part 2/4 tables already trace every finding to exactly one test. The R1–R9 numbering originates in the requirements artifact, which the design references by name; no R-property is missing from the design, and this is a one-line documentation step at implementation time, not a design-soundness defect. All three adversarial reviews passed with zero remaining blockers; the design gate reviewer verified all 8 mandated items as incorporated with settled conventions.

VERDICT: PASS - every blocking finding (protocol closed-world lockstep, DB re-entry asymmetry/assertions/busy-timeout, all 7 security spec gaps, all 7 untested acceptance findings) is resolved in the amended design with settled normative conventions, independently confirmed against the code; the sole residual (R1-R9 individual enumeration) is a non-blocking traceability note whose substance is already stated in section 9.
