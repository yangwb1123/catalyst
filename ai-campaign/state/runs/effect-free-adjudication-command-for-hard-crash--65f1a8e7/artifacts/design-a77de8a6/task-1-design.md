# Design — effect-free `group graph run dispatch adjudicate` for hard-crash-quarantined v4 claims

**Status:** AMENDED (v2). Amendment incorporates, item by item, every blocking/low finding from the
three adversarial reviews (protocol `verify-cross-language-invariants.md`, DB transaction review,
security audit `audit-dispatch-adjudicate.md`) into the original design summary. Section
"Review-findings incorporation" is normative for the implementer; the rest of this document is
the design proper.

---

## 1. Evidence verification (original, re-confirmed by all three reviews)

All 7 requirement citations verified; the three reviews additionally re-verified every
load-bearing claim against current code (no drift found):

- **Exact (4):** `args.rs:36` (`pub enum Command`), `quarantine.rs`
  (`strand_v4_with_local_provider` asserts `DispatchUnknown` + `active_lane.is_some()`),
  `FUNCTIONAL_REQUIREMENTS_AUDIT.md:215` (deferred row), and the requirements doc's own
  line-drift note on `scheduled_provider_request_dispatch.rs` (:282/283/286–290/291).
- **Minor drift (3):** `cli_usage.rs:205/212` (cited 202/211, same sentences), `error.rs:19`
  (cited 18).
- **Protocol review (PASS):** `hard_crash` lockstep feasible and additive; `transition_run`
  CAS exact (`run_version=4 ∧ status='dispatch_unknown' ∧ execution_contract_present=1 ∧
  dispatch_request_present=1 ∧ dispatch_authority_released=1 ∧ last_event_seq=?4` +
  `EXISTS(seq=?4 ∧ event_sha256=?5)`); `release_lane` exact 5-key DELETE; replay
  zero-mutation; protocol v1 handshake + old-Core failure modes loud and pre-mutation.
  **One required hardening:** Rust closed-world test list must be extended (Finding 1) —
  incorporated in §7 and §9.
- **DB review (PASS):** BEGIN IMMEDIATE + 5 s busy timeout everywhere; `ensure_claim_source`
  exact-equality; post-state completeness holds transitively via `reconstruct` (load_events
  re-summation, `valid_terminal_record_state`, `validate_state_shape`, `inspection.validate()`
  exact status/v/seq/event binding); re-adjudication refusal and execute re-entry idempotence
  hold for every existing path; deadlock-free by construction. **Design-stage finding:**
  re-entry-convention asymmetry must be settled (§7.1) and `ensure_persisted` run-level
  assertions localized (§7.6). **Info findings:** busy-timeout → `Unavailable` →
  `DispatchQuarantined` variant (§7.5), open-time `BEGIN IMMEDIATE` on reads (documented
  residual, §6), journal cap edge (documented residual, §6).
- **Security audit (PASS, 7 specification gaps):** sealed-memfd pinning TOCTOU-closed
  (reuse, don't re-verify — §7.3); digest-only Hub confirmed (preflight digest cross-check —
  §7.3); never `PreparedDispatchDependencies::prepare` (reads `OPENAI_API_KEY` — §7.3);
  dual-stdin + read-before-spawn (§7.4); distinct `AdjudicationRefused` causes + explicit
  exit-code contract + no `DispatchQuarantined` copy (§7.2); poisoned-key A3 test (§9 A3);
  `hard_crash` 4 touch points (§7.7).

---

## 2. API

New CLI surface (Linux-only, same as execute):

```
group graph run dispatch adjudicate GRAPH_RUN_ID \
    --authorization FILE|- --pricing FILE|- \
    --core-bin ABSOLUTE_FILE --core-bin-sha256 SHA256
```

- New variant `GroupGraphRunDispatchCommand::Adjudicate { graph_run_id,
  authorization_source, pricing_source, core_bin, core_bin_sha256 }`.
- `parse_run_dispatch` gains `Some("adjudicate")`; new `parse_dispatch_adjudicate`
  parser (§7.4). No `--confirm-off-machine` (nothing leaves the machine), no `--include-result`
  (output reuses the Execution shape; result text is not produced — the adjudication writes a
  terminal `failed_uncertain` state with no model result).
- Idempotency-key bookkeeping: `dispatch_claim_key_error` and `accepts_idempotency_key`
  treat `Adjudicate` exactly like `Execute` — `--idempotency-key` fails with the
  "GRAPH_RUN_ID owns the single dispatch claim" message; Adjudicate itself is NOT listed in
  `accepts_idempotency_key`.
- `cli_usage.rs`: new usage line; the dead-end sentence at :202/:211 region ("…never retried
  automatically. Prepare a new analysis to make another attempt.") gains the remedy:
  "a hard-crash-quarantined claim can be adjudicated with `group graph run dispatch
  adjudicate`".
- Output: `GroupAgentGraphRunDispatchCommandCliOutput::Execution` reusing
  `GroupAgentNodeDispatchExecutionCliOutput` with a new `Adjudicated(inspection)` result
  shape (same inspection JSON as `Terminalized`, so automation sees the same fields:
  status `failed_uncertain`, run_version 5, last_event_seq 5, journal_bytes, lane released).

## 3. New service — `GroupAgentNodeDispatchAdjudicationService`

- New application crate module `group_agent_node_dispatch_adjudication` with
  `GroupAgentNodeDispatchAdjudicationService` whose constructor takes **only**
  store/codec/bridge/metadata: `GroupAgentNodeLifecycleStore`,
  `GroupAgentNodeDispatchRequestCodec`, `Arc<PinnedCoreTerminalBridge>`,
  `GroupAgentNodeDispatchMetadataSource`. **No** `GroupAgentNodeDispatchProviderFactory`,
  **no** `GroupAgentNodeCredentialSource` — "no credential" is a type-level guarantee (A3):
  no provider fields ⇒ no code path to a provider stream ⇒ no send. DB writes are confined to
  the terminalize CAS; the guarantee is "no send", not "no DB writes".
- New wiring function `adjudication_service()` in the interfaces layer constructs it with
  store/codec/bridge/metadata only. It must **never** call `execution_service()` and must
  **never** call `PreparedDispatchDependencies::prepare` (that builder reads `OPENAI_API_KEY`
  via `EnvironmentOpenAiCredentialSource`; reuse would fail keyless environments with
  `CredentialUnavailable` and silently build provider machinery when a key IS present —
  both violate A3).
- Flow (all effects local; ordering mandated by §7.4):
  1. Read operator inputs (stdin to EOF, bounded, **before any subprocess spawn**).
  2. Open Hub read-only; inspect lifecycle; `existing_lifecycle`-style guard — run status
     must be `DispatchUnknown` with claim present, active lane present, no artifact, no
     receipt; any other shape → `AdjudicationRefused::NotStranded` (zero mutation, §7.1/§7.2).
  3. **Preflight digest cross-check** (§7.3): decode + canonicalize operator
     authorization/pricing; compare `authorization_sha256` / `pricing_snapshot_sha256`
     against the persisted claim digests; mismatch → `AdjudicationRefused::DigestMismatch`
     (zero mutation, before bridge construction).
  4. `PinnedCoreTerminalBridge::new(core_bin, core_bin_sha256)` — **unchanged reuse** of the
     sealed-memfd pipeline (bytes executed == bytes hashed == pinned digest; no
     hash-then-run-by-path). Handshake failure → loud bridge error (old Core without the
     subcommand), exit 1 (§7.2 cause mapping).
  5. Build the operator-constructed hard-crash artifact: classification `HardCrash`,
     provider_poll_started=false, terminal_seen=false, stream_eof_seen=false, no usage, no
     cost — the all-false no-evidence artifact accepted by both validators
     (`valid_uncertainty` chronology `provider_poll_started ∨ (¬terminal_seen ∧
     ¬stream_eof_seen)`; Go `validUncertaintyOutcome` identical). `terminalized_at_ms` from
     the metadata source; build terminal control from the release control + stored claim +
     artifact (reusing `build_terminal_control`).
  6. `bridge.decide(&control)` — Core validates control (claim binding, artifact validity,
     clock: `artifact.created_at_ms >= claim.released_at_ms`, hardcoded
     `ExpectedLastEventSeq: 4` + claim-head). Core emits a deterministic `failed_uncertain`
     receipt (uncertainty → `failed_uncertain` in both validators). Decide failure →
     `AdjudicationRefused::CoreRefused` with a re-pin hint (§7.2) — **never** execute's
     blanket Core→`DispatchQuarantined` mapping.
  7. `terminalize_group_agent_node_dispatch(&request)` — the existing single
     `BEGIN IMMEDIATE` CAS transaction (`reject_terminal_replay`, `ensure_claim_source`
     exact-equality, guarded `transition_run`, seq-5 event insert, `release_lane` exact
     DELETE, `ensure_persisted` + localized run-level assertions §7.6). `Conflict` →
     `AdjudicationRefused::CasConflict` (retryable, §7.2); `Unavailable` (5 s busy timeout) →
     store error surfaced with a contention message (retryable, §7.5). Success → return the
     committed inspection as `Adjudicated`.

## 4. `hard_crash` classification — additive lockstep (protocol v1 stays)

- Rust: `GroupAgentNodeTerminalClassification::HardCrash` (`mod.rs`, serde snake_case) + one
  `matches!` arm in `valid_uncertainty` (`artifact_validation.rs`).
- Go: one entry `"hard_crash"` in `uncertaintyClasses` (`validate_artifact.go:9-14`); the
  map-driven `TestTerminalReceiptFixedOutcomeMappings` auto-covers the deterministic
  `failed_uncertain` outcome.
- Outcome mappings unchanged (both already class-agnostic: any `Uncertainty` →
  `failed_uncertain`).
- **Enforcement nuance (protocol Finding 1, now mandatory):** Rust `matches!` is
  non-exhaustive — the enum variant alone compiles and silently fail-closes. The closed-world
  test `every_fixed_uncertainty_class_is_closed_world_and_nonretryable`
  (`domain/tests/group_agent_node_lifecycle.rs:181`) must be extended in the SAME change
  (§9 A-hardcrash). Additive asymmetry is safe both ways (Rust-only → Hub refuses; Go-only →
  `errInvalidControl` → loud bridge failure; both zero-mutation).
- The scheduled-family validator (`group_agent_scheduled_node_lifecycle_terminal_validation.rs`)
  keeps its own closed-world list and continues to REJECT `HardCrash` — correct: ADR-0034's
  pid-sidecar adjudication owns that family. Add a negative regression asserting this fence
  (§9 A-scheduled-fence).
- Protocol versions stay 1 everywhere: the handshake version describes the wire envelope
  (subcommand + control/receipt schema), not the classification vocabulary; vocabulary growth
  is fail-closed on old binaries.

## 5. Compatibility

- `execute` re-entry, the scheduled family, and schema v23 untouched; no migration.
- Old pinned Cores fail loudly — pre-`hard_crash` Core passes the `--protocol-version`
  preflight and fails at decide → mapped to `AdjudicationRefused::CoreRefused` with a re-pin
  hint (re-pin is REQUIRED, not optional: an old Core can never adjudicate a `hard_crash`
  claim).
- Existing stranded v4 claims become adjudicable immediately (no schema/state change).

## 6. Failure modes (enumerated)

1. **Live-executor race** (a) CAS conflict: `reject_terminal_replay` / `transition_run`
   changed=0 / `release_lane` changed=0 → `AdjudicationRefused::CasConflict`, retryable,
   zero mutation (whole txn rolls back). (b) **busy-timeout variant (added):** the racing
   executor holds `BEGIN IMMEDIATE` > 5 s → `SQLITE_BUSY` → `HubStoreError::Unavailable`
   (this is exactly how the execute service strands as `DispatchQuarantined` via
   `service.rs:207-211`); adjudicate surfaces it as a store error with a contention message,
   retryable — it does NOT strand anything (adjudicate owns no lane and maps nothing to
   `DispatchQuarantined`). Executor-side terminalize after an adjudication commits still
   hits `reject_terminal_replay` inside the winner's committed state — no lost update.
2. **Re-adjudication / already-terminal re-entry** → `AdjudicationRefused::NotStranded`,
   zero mutation, §7.1/§7.2. Note: `reject_terminal_replay` refuses on artifact/receipt
   presence unconditionally, byte-identity irrelevant.
3. **Wrong operator bodies** → preflight `AdjudicationRefused::DigestMismatch` (before any
   bridge call); without the preflight the same error would surface deep in the Core as a
   redacted bridge failure — the preflight exists to make it diagnosable.
4. **Core skew (old pinned Core)** → `AdjudicationRefused::CoreRefused` with re-pin hint
   (decide-time only; the preflight handshake does not probe classification support).
5. **WAL contention** — documented residual: every `connect()` runs `BEGIN IMMEDIATE`
   (migrate/validate), so reads transiently contend for the write lock; retry loop (250 ms
   open + 5 s/10 ms) absorbs stragglers. Adjudicate's single CAS txn is ~6 statements.
6. **Stdin collision / read-before-spawn** → §7.4; parse-level rejection of `- -`.
7. **Signal orphan** — Core child is in its own process group; SIGINT orphans the
   effect-free ≤15 s child (same as execute; v4 family has no `cancel_on_os_signal`).
   Accepted residual, documented.
8. **Deadline/hash interplay** — 128 MB Core copy+hash shares the 15 s decide deadline;
   slow filesystems can spuriously time out → bridge failure message; retryable, no security
   hole.
9. **Clock skew** — Go validator requires `artifact.created_at_ms >= claim.released_at_ms`
   and Rust requires `terminalized_at_ms >= artifact.created_at_ms`; a backward wall clock
   yields a Core refusal. Correct behavior (refuse, don't fabricate); surfaces as
   `CoreRefused` with the bridge's message.
10. **Journal cap edge** — v4 journal ≤ 262 144 + max event 65 536 = exactly 327 680 at the
    cap; a max-size terminal event lands exactly on the cap with zero headroom — valid,
    documented residual.
11. **v4 has no pid-sidecar liveness proof** — accepted residual (ADR-0034 fences the v4
    family); adjudicate is operator-invoked no-send with CAS'd lane release.

## 7. Review-findings incorporation (normative for the implementer)

### 7.1 Re-entry-convention asymmetry — SETTLED: refuse (`AdjudicationRefused::NotStranded`)

The codebase has three adjacent conventions:

| Path | Re-entry behavior |
|---|---|
| v4 `terminalize` (`reject_terminal_replay`) | `Conflict("Node dispatch lifecycle is already terminal")` before any write |
| scheduled `terminalize` (`status ≠ Claimed`) | Ok no-op, returns stored inspection |
| scheduled `adjudicate` (status ≠ `'claimed'` ∨ lane none ∨ evidence present) | `Conflict("scheduled dispatch is not a hard-crashed claim; adjudication refused")` before any write |
| `execute` re-entry (`existing_lifecycle` → `AlreadyClaimed`) | Ok result with stored inspection, zero effects |

**Decision: the new `GroupAgentNodeDispatchAdjudicationService` follows the REFUSE
convention** — any re-entry on a non-stranded claim returns
`AdjudicationRefused::NotStranded { reason }` (reason enumerates the observed state: already
terminal / run not dispatch_unknown / no claim / no active lane), zero mutation, exit 1.
Rationale:

1. **Closest analogue is scheduled *adjudicate*, not scheduled *terminalize*.** The scheduled
   Ok-noop belongs to a supervisor-driven terminal path where idempotent no-op is the correct
   contract. The v4 adjudicate is an operator-invoked one-shot remedy; the v4 family's own
   terminalize and the scheduled adjudicate both refuse.
2. **Honest error mapping.** The store layer already refuses (v4 terminalize → `Conflict`).
   An Ok-noop convention would force the service to swallow the store's `Conflict` and
   guess its cause; refusing keeps the mapping 1:1 and lets the message carry the state.
3. **Operator intent.** Re-entry means the operator's `--authorization/--pricing/--core-bin`
   inputs were NOT applied. Silent exit-0 would hide that; loud refusal with the current
   state is diagnosable and retry-safe (zero mutation guarantees retry safety).
4. **Distinct from execute re-entry.** Execute's `AlreadyClaimed` exit-0 no-op is correct
   there ("already durably claimed; resend forbidden; here is the state"). Adjudicate is a
   *remedy command*: a no-op exit-0 would make automation believe a remedy was applied.
   The settled convention is documented in the service's module doc comment.

### 7.2 Exit-code contract + distinct `AdjudicationRefused` causes — SETTLED

**Exit-code contract (explicit, codebase-consistent):** `0` = success (adjudicated; stored
inspection written); `1` = any command error including every refusal cause and every store
error (matches `main.rs`'s uniform `ExitCode::FAILURE`; the codebase has no structured
per-error codes and the only semantic exit code anywhere is wave-admit partial failure);
`2` = argument/parse errors (matches `argument_error`). **Refusal vs. failure is
message-only, by design**: every `AdjudicationRefused` message is prefixed with the stable
literal `adjudication refused:` so automation can match stderr; a dedicated refusal exit
code was considered and rejected for consistency (one bespoke code for one command would
create a precedent no other surface follows — `--json` output remains the structured
channel for automation).

**One new error variant `AdjudicationRefused` with four distinct causes, each with a
distinct message (no blanket mapping):**

| Cause | When | Message (starts `adjudication refused:`) | Retryable |
|---|---|---|---|
| `NotStranded` | run status ≠ `dispatch_unknown`, or no claim, or no active lane, or artifact/receipt already present | `…: <run> is not a stranded hard-crash claim (<observed state>)` | yes (no-op) |
| `DigestMismatch` | preflight body digests ≠ persisted claim digests | `…: authorization/pricing body does not match the claimed digest (<field>)` | yes, with corrected bodies |
| `CoreRefused` | bridge decide failure (old-Core classification rejection, Core validation refusal incl. clock skew, handshake-timeout interplay) | `…: Core refused the hard-crash terminal (<bridge detail>); re-pin to a Core with hard_crash support` | yes, after re-pin |
| `CasConflict` | live-executor race (`Conflict` from terminalize) | `…: concurrent executor terminalized the claim; re-run to see the committed state` | yes |

**Explicitly NOT copied:** execute's blanket Core→`DispatchQuarantined` mapping
(`service.rs:195-196`, message "durably claimed and quarantined; resend is forbidden") is
execute-specific and would mislead for adjudicate. The adjudication service maps
Core/decide errors to `AdjudicationRefused::CoreRefused`, store `Conflict` to
`AdjudicationRefused::CasConflict`, and store `Unavailable` to a plain store error — it
never produces `DispatchQuarantined` and never reuses that message.

### 7.3 Preflight `--core-bin-sha256` digest cross-check + no credential builder — MANDATED

- **Reuse, don't re-verify (TOCTOU).** Adjudicate constructs `PinnedCoreTerminalBridge::new`
  and calls the existing `decide_json` pipeline unchanged. Prohibited: any new
  "sha256sum the path, then run the path" branch (the sealed-memfd machinery already
  guarantees bytes executed == bytes hashed == pinned digest, re-verified per invocation).
- **Mandated preflight:** after the stranded-state guard and BEFORE bridge construction,
  the service decodes + canonicalizes the operator authorization/pricing and compares
  `authorization_sha256` / `pricing_snapshot_sha256` against the persisted claim (the same
  `decode_authorization` + `validate_claim_against_sources` checks the validators run).
  Mismatch → `AdjudicationRefused::DigestMismatch` (§7.2). This is what makes a wrong body
  distinguishable from a broken Core.
- **Mandated prohibition:** the adjudication service never reuses
  `PreparedDispatchDependencies::prepare` and the wiring never passes a credential source;
  `adjudication_service()` constructs the service with store/codec/bridge/metadata only
  (§3). The A3 test proves it with a poisoned key (§9 A3).

### 7.4 stdin: dual-stdin rejection + read-before-spawn — SPECIFIED

- `parse_dispatch_adjudicate` rejects `--authorization -` combined with `--pricing -` with
  `dispatch adjudicate accepts standard input for only one artifact` (mirroring
  `dispatch_execute_args.rs:59-60`; the execute check is inline and NOT shared, so the new
  parser must implement its own — this is new code).
- Ordering (mirroring `dispatch_command.rs::execute`): (1) stranded-state guard (read-only
  DB) → (2) read stdin to EOF, bounded (reuse `read_authorization_bounded` /
  `read_pricing_bounded`, 1 MiB / 16 KiB) → (3) `PinnedCoreTerminalBridge::new` (spawns the
  preflight subprocess) → (4) decide (spawns Core). **No subprocess is spawned before stdin
  is fully consumed**; Core children always get `Stdio::piped` and never inherit CLI stdin.

### 7.5 Busy-timeout variant added to failure mode #1

Failure mode #1 now has two outcomes: (a) CAS `Conflict` →
`AdjudicationRefused::CasConflict`; (b) 5 s busy-timeout → `SQLITE_BUSY` →
`HubStoreError::Unavailable` → surfaced as a store error with a contention message
(observationally the same strand-outcome the execute service maps to
`DispatchQuarantined` at `service.rs:209-210` — but adjudicate does NOT reuse that
mapping; nothing becomes "quarantined" because adjudicate owns nothing). Both retryable.

### 7.6 `ensure_persisted` run-level post-state assertions — LOCALIZED (implementation step)

`ensure_persisted` (`terminalize.rs:248-266`) currently asserts row-level equality only;
status/run_version/last_event_seq/journal_bytes are verified only transitively by
`reconstruct` (`load_events` re-summation `count == last_event_seq` ∧ `Σbytes ==
journal_bytes`; `valid_terminal_record_state` v==5 ∧ terminal status ∧ seq==5;
`validate_state_shape` terminal ⇔ lane absent ∧ artifact ∧ receipt; `inspection.validate()`
`run.status == receipt.graph_status` exact value, `run.v == TERMINAL_VERSION`,
`validate_terminal_event` seq-5 binding, `validate_terminal_time`). **Mandated: add explicit
assertions to `ensure_persisted`** — `inspection.graph_run.run.status == terminal_status`,
`run.v == 5`, `last_event_seq == 5`, `journal_bytes == expected base + terminal-event bytes`,
event count == 5 — so the CAS post-state is locally self-evident to a reader of
`terminalize.rs` alone (assertions cannot fail in practice; they document by code). Keep the
doc comment naming the transitive readback chain it doubles. The A2 DB post-state test
asserts the same fields end-to-end.

### 7.7 Acceptance list updated — Rust closed-world lockstep test extension

Added to the acceptance mapping (§9): **A-hardcrash** extends
`every_fixed_uncertainty_class_is_closed_world_and_nonretryable`'s explicit 11-class list to
12 with `HardCrash` **in the same change as the enum variant** (the `matches!` is
non-exhaustive — without this the feature silently fail-closes and is dead on arrival), and
**A-scheduled-fence** adds a negative test that the scheduled-family validator still
rejects `HardCrash`.

### 7.8 Idempotence test asserts STORED inspection

Execute re-entry idempotence is keyed on `graph_run_id`, not request equality — a re-entry
carrying *different* authorization/pricing after terminalization silently returns the stored
terminal inspection (consistent with "never resends"). **Mandated: the idempotence
acceptance test (re-run `execute` after terminalization with deliberately DIFFERENT
authorization/pricing bodies) must assert the returned inspection is byte-identical to the
stored one** — status `failed_uncertain`, run_version 5, last_event_seq 5, journal_bytes,
claim/authorization/pricing digests unchanged, no new events — i.e., the stored inspection,
not the new inputs, is returned.

---

## 8. Migration / operational steps

- No schema migration (v23 current; v4 tables untouched v12→v23).
- Re-pin Core: `hard_crash` classification requires a Core built after the lockstep change;
  the re-pin step is REQUIRED and documented in `cli_usage.rs` (an old Core fails loudly at
  decide with a re-pin hint, §7.2 `CoreRefused`).
- Existing stranded claims are adjudicable immediately; nothing else changes.

## 9. Acceptance mapping (updated)

Named runnable tests:

- **A1** parse/usage boundaries: `adjudicate` recognized; all four flags required;
  `--authorization - --pricing -` → dual-stdin rejection message; `--idempotency-key` →
  GRAPH_RUN_ID-owns-the-claim error; usage text updated (dead-end sentence + remedy).
- **A2** `strand_v4_with_local_provider` fixture (RejectingCore → `DispatchQuarantined`,
  asserts `DispatchUnknown` + active lane) → adjudicate with the real pinned Core
  (`build_go_core`) → assert DB post-state: status `failed_uncertain`, run_version 5,
  last_event_seq 5, journal_bytes delta == terminal-event bytes, seq-5 event binds
  claim/artifact/receipt with `lane_released=true` ∧ `!retry_authorized`, lane ownership row
  deleted, `ensure_persisted` run-level assertions hold (§7.6).
- **A3** no-credential effect-freedom: run adjudication (a) with `OPENAI_API_KEY` absent and
  (b) with a **poisoned** key value present (the `CREDENTIAL_SECRET` fixture pattern);
  assert success and byte-identical output in both — proving the key is never read (the
  type-level constructor exclusion is necessary but not sufficient; the wiring could smuggle
  a credential in). Also assert `AdjudicationRefused` paths are zero-mutation (row counts +
  journal bytes unchanged).
- **A4** re-adjudication refusal: byte-identical re-adjudication of an already-terminal
  claim → `AdjudicationRefused::NotStranded`, zero mutation; digest-mismatch bodies →
  `AdjudicationRefused::DigestMismatch` pre-bridge; CAS-conflict path → `CasConflict`
  (unit-level with a concurrent terminalizer).
- **A5** execute idempotence, **asserting the stored inspection is returned (§7.8)** —
  re-invocation after terminalization with different bodies returns byte-identical stored
  state, no new events, never resends.
- **A-hardcrash** extend the 11-class closed-world list to 12 (§7.7); **A-scheduled-fence**
  negative test (§7.7).
- **A6** `cargo clippy --all-targets --all-features --offline -- -D warnings` clean.
- Derived: R1–R9 from the requirements doc (protocol v1 unchanged; scheduled family
  untouched; execute re-entry untouched; Linux-only documented; A2 fixture strand path
  unchanged), plus the audit-doc honesty steps (`FUNCTIONAL_REQUIREMENTS_AUDIT.md:215`
  deferred row now shipped; `cli_usage.rs` dead-end sentence updated; ADR-0034 v4-family
  fence note updated to point at the new command).

## 10. Residual risks (accepted, documented)

- v4 family has no pid-sidecar liveness proof — adjudicate is operator-invoked; the lane is
  released only via the CAS'd exact DELETE.
- Open-time `BEGIN IMMEDIATE` on every connection (reads included) is an undocumented
  write-lock contention amplifier; absorbed today by the retry loop.
- 128 MB hash shares the 15 s decide deadline; slow filesystems may spuriously time out.
- Journal cap edge (262 144 + 65 536 = 327 680 exact) has zero headroom.
- Signal-orphan of the effect-free Core child on SIGINT (same as execute).
