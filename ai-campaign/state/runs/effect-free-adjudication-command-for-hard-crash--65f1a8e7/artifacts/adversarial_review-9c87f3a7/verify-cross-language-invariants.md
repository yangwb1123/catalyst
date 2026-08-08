# Verification: adjudicate design cross-language invariants vs. codebase

Verifier: independent fresh-context agent (no prior involvement in design/requirements).
Scope: 5 invariants the design relies on, checked against current tree + two targeted
test runs (Go `internal/graphterminal`, Rust `forge-runtime-domain` closed-world test).
No code modified.

## Invariant 1 — Rust `hard_crash` enum / Go `uncertaintyClasses` lockstep

**Verdict: VERIFIED as feasible, additive, fail-closed in both directions — with one
enforcement nuance (Rust side is test-enforced, not compile-enforced).**

Current state (grep-confirmed): `hard_crash` exists in NEITHER language. Base lockstep is exact:

- Go `forge-core/internal/graphterminal/validate_artifact.go:9-14` — `uncertaintyClasses` map,
  11 entries: provider_error, http_error, transport_error, timeout, cancelled,
  eof_before_terminal, missing_usage, tool_call, protocol_error, trailing_data, local_limit.
- Rust `forge-runtime/crates/domain/src/group_agent_node_lifecycle/mod.rs:51-68` —
  `GroupAgentNodeTerminalClassification` enum, 13 variants (Completed, Length + same 11),
  `#[serde(rename_all = "snake_case")]`. Class lists match 1:1.
- Validation semantics lockstep: Go `validUncertaintyOutcome` (validate_artifact.go:76-86) and
  Rust `valid_uncertainty` (artifact_validation.rs:113-131) use the identical formula:
  class ∈ list ∧ (provider_poll_started ∨ (¬terminal_seen ∧ ¬stream_eof_seen)) ∧ missing_usage rule.
  A hard_crash artifact (poll never started, nothing observed) satisfies the chronology in both — no change needed there.
- Outcome mapping is already class-agnostic in both: Go `receiptOutcome`
  (receipt.go:90-100) maps any `uncertainty` → `failed_uncertain`; Rust
  `receipt_outcome_matches_artifact` (terminal_validation.rs:289-303) has
  `(Uncertainty, _) => FailedUncertain`. Adding the class requires **no outcome changes**.

Additive-change surface (what the implementation must touch):
- Go: one map entry. `TestTerminalReceiptFixedOutcomeMappings` (terminal_test.go:58) iterates the
  map — new class auto-covered for the deterministic outcome. Map-driven, no other Go change.
- Rust: one enum variant + one arm in the `matches!` in `valid_uncertainty`
  (artifact_validation.rs:115-126). The scheduled-family validator
  (`group_agent_scheduled_node_lifecycle_terminal_validation.rs:104-125`) shares the enum but
  keeps its own closed-world list — HardCrash stays rejected there (correct: scheduled family
  has its own ADR-0034 pid-sidecar adjudication).

**Nuance (design claim "lockstep" is weaker than compile-enforced):** Rust `valid_uncertainty`
and the scheduled validator use `matches!` (non-exhaustive boolean) — adding the variant does NOT
break compilation; the new class silently evaluates `false` (fail-closed) until the arm is added.
Lockstep is therefore enforced by (a) the Core being the single receipt authority — the Go Core
must accept the control before any receipt exists, and the Rust Hub must accept the receipt
afterwards, so any asymmetric addition fails loudly with zero mutation; (b) the Go map-driven
test; (c) the Rust test `every_fixed_uncertainty_class_is_closed_world_and_nonretryable`
(domain/tests/group_agent_node_lifecycle.rs:181) whose explicit 11-class list must be extended
by hand — this is the drift risk to put in the acceptance list (A2 must include extending it).
Both directions of asymmetry are safe: Rust-only → Hub refuses control validation; Go-only →
`BuildReceipt` → `errInvalidControl` → bridge `decide` fails.

Ran: Go `TestTerminalReceiptFixedOutcomeMappings|TestTerminalControlRejectsImpossibleUncertaintyEvidence` — ok.
Ran: Rust `every_fixed_uncertainty_class_is_closed_world_and_nonretryable` — ok (11 classes).

## Invariant 2 — CAS guards on `transition_run`

**Verdict: VERIFIED EXACT.**

`forge-runtime/crates/infrastructure/src/sqlite_hub/group_agent_node_lifecycle/terminalize.rs`:
- Single `BEGIN IMMEDIATE` transaction (`transaction_with_behavior(TransactionBehavior::Immediate)`, :31-38).
- `transition_run` (:214-253): `UPDATE group_agent_graph_runs SET run_version=5,status=?1,
  last_event_seq=5, journal_bytes=journal_bytes+?2 WHERE id=?3 AND run_version=4 AND
  status='dispatch_unknown' AND execution_contract_present=1 AND dispatch_request_present=1 AND
  dispatch_authority_released=1 AND last_event_seq=?4 AND EXISTS(SELECT 1 FROM
  group_agent_graph_run_events WHERE graph_run_id=?3 AND seq=?4 AND event_sha256=?5)`.
  - ?4 = `receipt.expected_last_event_seq` — Go Core hardcodes 4 (receipt.go:73) and validates
    `== 4` (validReceiptHeader receipt.go:87-88); Rust `validate_terminal_receipt` also hardcodes
    `expected_last_event_seq == 4` (terminal_validation.rs:178). Lockstep on the seq guard.
  - ?5 = domain digest of `receipt.expected_last_event_sha256` = `claim.ClaimEventSHA256`
    (claim-head). The claim-head is also chained in the seq-5 event
    (`previous_event_sha256 == claim.claim_event_sha256`, terminal_validation.rs:360).
  - `changed == 1` else conflict "Graph Run cursor, claim head, or terminal state changed" (:248-252).
- Schema conjunct matches the state machine exactly (v12_sql.rs:88-95: run_version=4 ∧
  dispatch_unknown ∧ contract=1 ∧ request=1 ∧ authority=1 ∧ last_event_seq=4; post-state v5
  conjunct `status IN ('completed','failed','failed_uncertain') AND last_event_seq=5` :96-103;
  `scheduler_protocol_version = 1` CHECK :34).
- Preceding guards in the same txn: `reject_terminal_replay` (:144-159) then `ensure_claim_source`
  (:161-181) — exact equality on claim, active_lane, artifact IS NULL, receipt IS NULL, run, plan,
  journal events. The `INSERT ... seq=5` of the terminal event is also guarded by the whole txn.

## Invariant 3 — `release_lane` exact DELETE

**Verdict: VERIFIED EXACT.**

terminalize.rs:255-283: `DELETE FROM group_agent_project_lane_ownerships WHERE
lane_ownership_id=?1 AND graph_run_id=?2 AND dispatch_id=?3 AND project_lane_sha256=?4 AND
claim_event_sha256=?5` — five-key exact match including both the claim-head digest and the lane
identity; `changed == 1` else conflict "exact active Project lane ownership is missing". Runs
inside the same BEGIN IMMEDIATE txn after `transition_run`, so a 0-row lane delete rolls back the
entire terminalization (including the run transition) — no partial state.

## Invariant 4 — replay / re-adjudication zero-mutation

**Verdict: VERIFIED.**

- `terminalize_locked` ordering (:67-105): `request.validate()` → `reject_terminal_replay`
  (artifact OR receipt row present ⇒ conflict "Node dispatch lifecycle is already terminal",
  :144-159) → `ensure_claim_source` → load → validate source → **then** first write
  (`insert_artifact`, write stage BeforeArtifact). Rejection happens before any INSERT/UPDATE/
  DELETE, inside a transaction that errors out — zero mutation on refusal, byte-identity
  irrelevant (refusal is unconditional once terminal rows exist).
- `ensure_persisted` (:285-301) verifies the committed state exactly matches the request after
  commit — read-back corruption guard.
- Execute re-entry idempotence confirmed separately: `existing_lifecycle` (application/
  group_agent_node_dispatch_execution/service.rs:130-150) returns `AlreadyClaimed` for
  DispatchUnknown/terminal states before any preflight/claim; claim start also maps
  `AlreadyClaimed` (:55-56, :115-116).
- Live-executor race (design failure mode #1): a racing executor's terminalize hits the CAS
  (`transition_run` changed=0 or `reject_terminal_replay` or `release_lane` changed=0) → conflict
  error, whole txn rolls back. Sound.

## Invariant 5 — protocol version 1 + old-pinned-Core failure modes

**Verdict: VERIFIED SOUND AND COMPLETE.**

- Handshake: `PinnedCoreTerminalBridge::new` (infrastructure/core_terminal_bridge.rs:76-85)
  invokes `graph-node-terminal-receipt --protocol-version` and requires stdout == `"1"`; Go side
  prints `"1"` (forge-core/internal/graphterminal/command.go:50-51; registered in
  cli_dispatch.go:47). Failure ⇒ "Core terminal protocol handshake failed" at bridge
  construction — before any DB access (bridge is pure, no store handle).
- Old Core predating the subcommand: subcommand unregistered → Go CLI writes usage to stderr,
  exits nonzero → bridge `invoke` fails on non-empty stderr / non-success status
  ("Core terminal process failed", core_terminal_bridge.rs:183-194). Loud.
- Old Core WITH the subcommand but pre-`hard_crash` map: `BuildReceipt` → `validateControl` →
  `validUncertaintyOutcome` → unknown class ⇒ `errInvalidControl` ⇒ exit 1 ⇒ bridge fails. Loud.
  It cannot silently emit a different-class receipt: the classification travels inside the
  control, the Core's map is closed-world, and even a misbehaving Core's output is rejected by
  Rust `validate_receipt_against_control` (exact `terminal_control_sha256 == snapshot_sha256`
  and `expected_last_event_sha256 == claim_head`, terminal_validation.rs:238-254) plus full
  field-exact `validate_receipt_against_terminal_evidence` (:257-276).
- Version 1 stays sound: the handshake version describes the wire envelope (subcommand +
  control/receipt schema), not the classification vocabulary; vocabulary growth is fail-closed
  on old binaries. Version 1 is also schema-enforced (v12_sql.rs:34 `scheduler_protocol_version
  = 1`) and constant-enforced on both sides (Rust `*_PROTOCOL_VERSION: u16 = 1` mod.rs:13-21;
  Go `TerminalArtifactProtocol`/`TerminalReceiptProtocol = 1` types.go:18-20, checks
  receipt.go:87, validate_control.go:41).
- All failure modes occur pre-mutation: bridge construction + `decide` live in the interfaces
  layer before any terminalize call; the design's adjudicate flow (bridge built before
  terminalize, same shape as execute_dispatch in dispatch_command.rs:279-296) preserves this.
- Accepted residual (design, matches codebase): v4 family has no pid sidecar — ADR-0034
  (docs/adr/0034-hard-crash-adjudication.md) explicitly fences off "the legacy v4 family" and
  notes the v4 family "keep[s] their own fences"; liveness is not provable, so adjudicate is
  operator-invoked no-send (passive receipt, CAS'd lane release) — consistent.

## Supporting facts the design relies on (all confirmed)

- Hub persists only claim digests for v4: `insert_claim` (group_agent_node_lifecycle/claim.rs:113-160)
  stores authorization/dispatch_request/logical_request/request_body/pricing_snapshot/project_lane/
  claim_event SHA-256s + compact claim blob, **no** authorization/pricing bodies ⇒ adjudicate
  must take `--authorization --pricing` (contrast: scheduled family v22 lifecycle DOES store
  bodies — v22_sql.rs:215). Design constraint is real.
- Schema v23 current (v23_sql.rs) adds scheduled `'adjudicated'` status + `adjudicated_at_ms`;
  v4 runs table untouched across v12→v23 ⇒ no migration needed for v4 adjudicate. Confirmed.
- Fixture `strand_v4_with_local_provider` (interfaces/tests/.../quarantine.rs) builds the stranded
  claim with `RejectingCore` ⇒ `DispatchQuarantined`, asserting DispatchUnknown + active lane —
  the A2 basis exists.
- `validate_terminal_event` (terminal_validation.rs:335-354) pins seq=5, terminal event version,
  and `previous_event_sha256 == claim_event_sha256`; `insert_event` hardcodes seq 5. Consistent
  with the design's seq-4→seq-5 CAS story.

## Findings for the design (not blockers)

1. **Rust lockstep is test-enforced, not compile-enforced** (`matches!` is non-exhaustive).
   The design's acceptance list must explicitly include extending
   `every_fixed_uncertainty_class_is_closed_world_and_nonretryable` (and any scheduled-family
   closed-world test) — otherwise `hard_crash` could be merged into the enum while the validator
   list silently rejects it (fail-closed, but the feature would be dead on arrival).
2. Old-pinned-Core "re-pin step" is correctly documented as REQUIRED, not optional: an old Core
   can never adjudicate a hard_crash claim; only re-pinning (or accepting refusal) is possible.
3. No changes needed to `receipt_outcome_matches_artifact`, `outcome()` test helper
   (ports.rs:288, wildcard `_`), or the collector — hard_crash is operator-constructed evidence,
   never produced by the provider collector; the design's additive claim holds with a minimal
   2-file + 2-test touch surface.

VERDICT: PASS — all five cross-language invariants are verified sound and complete against the
current codebase; the design's load-bearing claims (BEGIN IMMEDIATE, transition_run CAS with
run_version=4/dispatch_unknown/last_event_seq=?4/claim-head event_sha256, release_lane exact
DELETE, replay zero-mutation, version-1 handshake + loud old-Core failure) are exact matches;
the only hardening required is the Rust test-list extension noted in Finding 1.
