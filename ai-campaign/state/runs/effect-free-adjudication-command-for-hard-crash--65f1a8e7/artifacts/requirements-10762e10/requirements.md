Requirements specification complete. 

**Output:** `ai-campaign/state/runs/effect-free-adjudication-command-for-hard-crash--65f1a8e7/artifacts/requirements-10762e10/requirements.md`

## Evidence verification — all 7 citations checked

- ✅ **Exact (5):** `cli_usage.rs:211` (the "never retried automatically… prepare a new analysis" dead-end sentence), `cli_usage.rs:202` ("a hard crash can quarantine the Project lane"), `error.rs:18` (`DispatchQuarantined` — "durably claimed and quarantined; resend is forbidden"), `args.rs:36` (`pub enum Command`), `quarantine.rs` (`strand_v4_with_local_provider` asserts `DispatchUnknown` + `active_lane.is_some()`), `FUNCTIONAL_REQUIREMENTS_AUDIT.md:215` ("safe hard-crash adjudication … remain explicitly deferred").
- ⚠️ **Line drift (1):** `scheduled_provider_request_dispatch.rs:283` is `invalid_evidence`'s body; the cited substance (SIGKILL/OOM → stranded `dispatch_unknown`, catchable signals folded into a terminal) is the doc comment at **286–290** of `cancel_on_os_signal` (:291). Same file/symbol, noted in the table.

## Key findings that shaped the spec

- **The "seq/head CAS" the acceptance demands already exists**: `terminalize.rs`'s single `BEGIN IMMEDIATE` transaction (`ensure_claim_source` exact-equality, `transition_run` guarded on `run_version=4 AND status='dispatch_unknown' AND last_event_seq=4` + claim-head digest, `release_lane`, `reject_terminal_replay` = re-adjudication rejection). No schema change needed (v23).
- **The Core fixes the outcome**: `forge-core/internal/graphterminal/receipt.go` — uncertainty artifact → `failed_uncertain`, hardcoded `ExpectedLastEventSeq: 4`/claim-head. So "failed terminal" is deterministic, and `valid_uncertainty` accepts a no-stream-observed artifact.
- **Scope boundary discovered**: ADR-0034 already shipped adjudication for the *scheduled* family (pid sidecar, `status='adjudicated'`, v23 column) and explicitly fenced off the legacy v4 family — which is exactly the gap this direction closes. The spec keeps the scheduled family and the `execute` re-entry semantics untouched.
- **Constraint from verified facts**: the Hub doesn't persist authorization/pricing bodies, so `adjudicate` takes the same exact-artifact flags as `execute` (minus consent/credential) plus the pinned Core.

The 4 acceptance checks are preserved verbatim and expanded into named, runnable tests (A1 usage/help + parse, A2 fixture strand → adjudicate → DB post-state assertions for the CAS, A3 effect-free with no credential + zero-mutation re-adjudication rejection, A4 clippy gate), plus 9 functional requirements (R1–R9) and design-stage risks.
