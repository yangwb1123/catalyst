Design complete. Summary of what was produced and verified:

## Evidence verification (7 citations, all substantively confirmed)

- **Exact (4):** `args.rs:36` (`pub enum Command`), `quarantine.rs` (`strand_v4_with_local_provider` asserts `DispatchUnknown` + `active_lane.is_some()`), `FUNCTIONAL_REQUIREMENTS_AUDIT.md:215` (deferred row), and the requirements doc's own line-drift note on `scheduled_provider_request_dispatch.rs` (:282/283/286–290/291) — accurate.
- **Minor drift (3):** `cli_usage.rs:205/212` (cited 202/211, same sentences), `error.rs:19` (cited 18).
- **Amplified, all confirmed:** the terminalize CAS (BEGIN IMMEDIATE, `ensure_claim_source` exact-equality, `transition_run` guarded `run_version=4 AND status='dispatch_unknown' AND last_event_seq=?4` + claim-head `event_sha256`, `release_lane` exact DELETE, `reject_terminal_replay`), Core's deterministic `failed_uncertain` + hardcoded `ExpectedLastEventSeq: 4`, the no-stream-observed uncertainty artifact being valid in both validators, ADR-0034 explicitly fencing off the legacy v4 family, schema v23 current, and the Hub persisting only claim digests (not authorization/pricing bodies).

## Design decisions (delivered to `artifacts/design-a77de8a6/task-1-design.md`)

- **API:** new `group graph run dispatch adjudicate GRAPH_RUN_ID --authorization --pricing --core-bin --core-bin-sha256`; new `GroupGraphRunDispatchCommand::Adjudicate` variant; dedicated `GroupAgentNodeDispatchAdjudicationService` whose constructor **structurally excludes** providers/credentials (making "no credential" a type-level guarantee for A3); one new error variant `AdjudicationRefused`; output reuses the existing Execution shape.
- **Key call:** new additive `hard_crash` classification (Rust enum + Go `uncertaintyClasses` map, lockstep) instead of reusing `cancelled` — the codebase itself documents the signal-vs-crash distinction; protocol versions stay 1.
- **Compatibility:** execute re-entry, scheduled family, and schema v23 untouched; old pinned Cores fail loudly (re-pin step documented); no migration needed — existing stranded claims become adjudicable immediately.
- **Failure modes:** 10 enumerated (live-executor race → CAS conflict, re-adjudication zero-mutation refusal, stale artifacts, Core skew, WAL contention, stdin collision…), including the accepted residual risk that v4 has no pid-sidecar liveness proof.
- **Acceptance mapping:** A1–A4 → 6 named runnable tests (parse/usage boundaries, fixture strand → adjudicate → DB post-state CAS assertions, no-credential effect-freedom, byte-identical re-adjudication refusal, execute idempotence, clippy), plus derived R1–R9 and audit-doc honesty steps.
