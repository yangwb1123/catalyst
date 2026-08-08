# Adversarial audit — `dispatch adjudicate` command surface

Audited deliverable: design summary for "Effect-free adjudication command for hard-crash-quarantined
dispatch claims and Project lanes" (design-a77de8a6), against the actual repository code
(`forge-runtime` crates + `forge-core` graphterminal package), in the five mandated areas:
TOCTOU, authorization/pricing body handling, stdin-collision/subprocess isolation,
AdjudicationRefused error paths, and the A3 no-credential type-level guarantee.

Method: every claim traced to code. All paths below are `forge-runtime/crates/...` unless noted.
Severity: BLOCKING = the design as delivered does not specify a property the implementation
must have (design-gate material); MEDIUM/LOW = gaps that degrade safety/UX/error quality but
are closed by the underlying machinery.

---

## Area 1 — TOCTOU between `--core-bin-sha256` verification and `--core-bin` execution

**Verdict: the existing pinning machinery is TOCTOU-closed at the fd level. The design's
"plus the pinned Core" is consistent — but the design must state, explicitly, that adjudicate
reuses `PinnedCoreTerminalBridge` unchanged and must not introduce a hash-then-run-by-path flow.**

Evidence chain (verified):
- `infrastructure/src/core_terminal_bridge.rs:96-122` (`prepare_binary`): absolute + canonical
  path equality (rejects symlinked/relative paths), symlink rejection, size bound, exec-mode
  check, `File::open`, then `same_file(link, opened)` (dev/ino/len/mtime) — a swap between the
  pre-open stat and the open is caught by inode mismatch.
- `infrastructure/src/core_terminal_bridge/pinned_executable.rs:32-52` (Linux): `memfd_create`
  (ALLOW_SEALING|EXEC) → copy from the opened fd with `copied != size` guard (catches
  concurrent truncation/growth) → seal (WRITE|SHRINK|GROW|EXEC|SEAL) + `fchmod 0500` →
  `hash_bounded(&mut file, size)` computed over the **sealed memfd** → executed via
  `/proc/self/fd/N` (`command_path()`). Bytes executed == bytes hashed == operator-pinned digest,
  same sealed inode. Path replacement after open is irrelevant; replacement before open fails
  the inode check or the memfd digest check.
- Re-verification per invocation: `invoke` (`core_terminal_bridge.rs:124-159`) re-runs
  `prepare_binary_bounded` for the preflight handshake **and** for every `decide_json` — a swap
  between preflight and decide is re-hashed and re-checked.
- Regression test exists: `pinned_executable.rs` `path_replacement_cannot_change_the_prepared_executable`.

Findings:
1. **[BLOCKING] Reuse, don't re-verify.** The design must specify adjudicate constructs
   `PinnedCoreTerminalBridge::new(core_bin, sha256)` and calls the existing `decide_json`
   pipeline. Any new "sha256sum the path, then run the path" branch would re-open the TOCTOU
   this audit area exists to close. The design summary's "plus the pinned Core" implies this
   but does not state the prohibition.
2. **[LOW] Deadline covers hashing too.** `prepare_binary_bounded` (`core_terminal_bridge.rs:161-174`)
   runs copy+hash inside the same deadline as child exec (5s preflight / 15s decide,
   `:49-50`); a 128 MB Core on a slow filesystem can spuriously fail with "Core terminal process
   timed out" (`MAX_CORE_BINARY_BYTES` = 128 MB, `:45`). Not a security hole; the design's failure
   list should document the interplay.
3. **[LOW] Old-Core failure is only visible at decide time.** The preflight handshake
   (`graph-node-terminal-receipt --protocol-version` → `"1"`, `core_terminal_bridge.rs:78-87` and
   `forge-core/cmd/forge/cli_dispatch.go:47`) does **not** probe classification support. A Core
   built before `hard_crash` was added passes preflight and fails at `decide_json` with the
   redacted "Core terminal process failed" (`:152-158`). The design's "old pinned Cores fail
   loudly (re-pin step documented)" is true but surfaces as a generic bridge failure — see
   Area 4 finding 1c for the required mapping.
4. **[INFO]** The digest is operator-supplied; the guarantee is "executed bytes == pinned
   digest", not "trusted forge binary" — same-user threat model, consistent with ADR-0034.
   Non-Linux returns "sealed Core execution requires Linux" (`pinned_executable.rs:88-101`);
   adjudicate inherits the documented Linux-only property (`cli_usage.rs` execute warning).

---

## Area 2 — Authorization/pricing body handling (Hub persists only digests)

**Verdict: the requirements constraint is verified correct for the v4 family, and the
claim↔body binding is enforced by BOTH validators — so supplying wrong bodies cannot corrupt
state. What the design must add is an explicit preflight digest cross-check (error quality)
and an explicit prohibition on reusing the credential-reading dependency builder.**

Verified facts:
- The v4 claims table persists digests only — `insert_claim` columns
  (`infrastructure/src/sqlite_hub/group_agent_node_lifecycle/claim.rs:121-151`):
  `authorization_sha256`, `pricing_snapshot_sha256`, `dispatch_request_sha256`,
  `logical_request_sha256`, `request_body_sha256` (BLOB(32)) — no `authorization_json` /
  `pricing_json` columns. Contrast: the scheduled family table **does** persist bodies
  (`schema_contract/v23_sql.rs:93` `authorization_json,pricing_json`).
- Asymmetry to document: the full provider request **body** IS recoverable from the Hub — it is
  embedded in the claim (`application/src/group_agent_node_dispatch_execution/build.rs`,
  `claim.request_body_bytes = request.provider_request_bytes`) and re-verified against the
  dispatch_request row by `validate_terminal_source`
  (`.../group_agent_node_lifecycle/source.rs:44-60`). Authorization/pricing bodies are not
  recoverable — hence the operator-supplied flags, and hence the digest binding is the only
  link between the bodies and the claim.
- The binding is enforced structurally: `validate_claim_against_sources`
  (`domain/src/group_agent_node_lifecycle/validation.rs:171-214`) requires
  `claim.authorization_sha256 == authorization.authorization_sha256` and
  `claim.pricing_snapshot_sha256 == pricing.pricing_snapshot_sha256` (plus the full
  claim↔event↔release binding), and the Core port runs `control.validate()` before deciding
  (`core_terminal_bridge.rs` port impl). The Go Core enforces the same
  (`forge-core/internal/graphterminal/validate_claim.go:44`, `validate_control.go:129-131`).
  `ensure_claim_source` (`.../terminalize.rs:107-122`) additionally requires
  `inspection.claim == request.control.claim`, and the control's claim is cloned from the DB
  inspection — so bodies cannot re-write the claim.
- Wrong-body outcome today: control validation fails inside the Core invocation → redacted
  bridge error. **Safety holds; diagnosability does not.**

Findings:
1. **[BLOCKING] Preflight digest cross-check.** The adjudication service must, before any bridge
   invocation: decode the operator bodies, canonicalize, and compare `authorization_sha256` /
   `pricing_snapshot_sha256` against the persisted claim, refusing with a distinct message
   ("authorization/pricing body does not match the claimed digest"). Otherwise a wrong body is
   indistinguishable from a broken Core. (The validators already expose exactly this check —
   `decode_authorization` + `validate_claim_against_sources` are callable preflight.)
2. **[BLOCKING] Never `PreparedDispatchDependencies::prepare`.** That builder
   (`interfaces/src/group_agent_graph/dispatch_execution_adapters.rs:42-63`) **reads
   `OPENAI_API_KEY`** and constructs a provider. Reusing it for adjudicate would (a) fail with
   `CredentialUnavailable` in a keyless environment and (b) silently build provider machinery
   when a key IS present — both violating A3. The adjudicate wiring must be a new function
   (store/codec/bridge/metadata only). See Area 5.
3. **[LOW] Clock-skew refusal.** The Go validator requires artifact `created_at_ms >=
   claim.released_at_ms` (`validate_artifact.go` `artifactBindsClaim`), and Rust
   `validate_terminalize_request` requires `terminalized_at_ms >= artifact.created_at_ms`
   (`domain/.../terminal_validation.rs`). A wall clock that moved backward since the claim
   yields a Core refusal. Correct behavior (refuse, don't fabricate), but the design should
   list it as a documented refusal cause.

---

## Area 3 — stdin-collision and subprocess isolation

**Verdict: the execute pattern is sound and the new parser must replicate it; the design must
state the dual-stdin rejection and the read-stdin-before-spawn ordering explicitly, because
the adjudicate parser is new code and the execute check is not shared.**

Verified facts:
- Dual-stdin rejection exists in execute parse (`dispatch_execute_args.rs:97-101`,
  "dispatch execute accepts standard input for only one artifact") and readiness
  (`:86-89`), and in the scheduled family (`scheduled_contract_args.rs:99`).
- Ordering in `dispatch_command.rs::execute` (`:59-64`): `inspect_existing_execution`
  (read-only DB) → `read_inputs` (`:104-116`, consumes stdin fully to EOF, bounded
  1 MiB / 16 KiB via `read_authorization_bounded`/`read_pricing_bounded` `:421-467`) →
  bridge `new()` (spawns preflight subprocess) → decide (spawns Core). stdin is consumed
  before ANY subprocess spawn; Core children always get `Stdio::piped`
  (`core_terminal_bridge/process.rs:15-27`), never inherit the CLI stdin.
- Child isolation (`process.rs`): `env_clear()`, `arg0` = display path, piped stdio,
  `process_group(0)` (`:143-146`), bounded stdout/stderr reads with deadline, process-group
  KILL on timeout (`:152-168`), non-zero exit / non-empty stderr / empty stdout all fail closed
  (`core_terminal_bridge.rs:147-158`).

Findings:
1. **[BLOCKING] Parser must reject `--authorization - --pricing -`.** `parse_dispatch_adjudicate`
   is new code; the execute parser's check is inline and not shared. The A1 parse test set must
   include the dual-stdin rejection case (the current acceptance names only "parse/usage
   boundaries").
2. **[BLOCKING] Read-before-spawn ordering.** The design must state adjudicate consumes stdin
   (if `-` used) to EOF before `PinnedCoreTerminalBridge::new` (which spawns the preflight
   subprocess). Keeping the execute ordering verbatim is sufficient.
3. **[LOW] Signal-orphan note.** The Core child is in its own process group; SIGINT to the CLI
   orphans the (effect-free, ≤15 s) Core child — same as execute. The v4 family has no
   `cancel_on_os_signal` wiring (that exists only in the scheduled family,
   `scheduled_provider_request_dispatch.rs:140-169`). The design's failure list mentions
   "stdin collision"; add the signal-orphan case for completeness.

---

## Area 4 — AdjudicationRefused error paths

**Verdict: the zero-mutation property is already guaranteed by the single-transaction
terminalize CAS; the gaps are distinct-message surfacing and an explicit exit-code contract.**

Verified facts:
- Current error enum: `application/src/group_agent_node_dispatch_execution/error.rs:5-20`
  (8 variants; no AdjudicationRefused). CLI: every Group Graph Run error → `eprintln!` +
  `ExitCode::FAILURE` (1) (`interfaces/src/main.rs:83-98`). No structured exit codes anywhere;
  refusal-vs-conflict is message-only (scheduled-family precedent: distinct messages via
  `io::Error` at `scheduled_provider_request_dispatch.rs:241,260` and
  `.../group_agent_scheduled_node_lifecycle/adjudicate.rs:40`).
- Zero-mutation refusal machinery already exists inside one `BEGIN IMMEDIATE` transaction
  (`terminalize.rs`): `reject_terminal_replay` (`:94-106`, "already terminal" — the
  re-adjudication refusal), `ensure_claim_source` (`:107-122`), `transition_run` CAS guarded on
  `run_version=4 AND status='dispatch_unknown' AND last_event_seq=?4` + claim-head digest
  (`:227-249`), `release_lane` exact DELETE (`:260-275`). Any refusal rolls back — no partial
  state. The design's "re-adjudication zero-mutation refusal" maps exactly to
  `reject_terminal_replay`.
- Execute's status guard (`application/src/group_agent_node_dispatch_execution/service.rs:125-136`):
  DispatchUnknown|Completed|Failed|FailedUncertain → inspect; AwaitingDispatchAuthorization →
  None; other → InvalidState. Adjudicate needs the analogous guard.

Findings:
1. **[BLOCKING] Distinct refusal messages.** The design must enumerate and give distinct
   messages to at least: (a) not stranded (run status ≠ dispatch_unknown, or no claim, or
   artifact/receipt already present — zero-mutation), (b) authorization/pricing digest mismatch
   vs claim (Area 2f1), (c) Core rejected the hard-crash artifact — this is the old-Core case,
   which today redacts to "Core terminal process failed"; the design must map bridge failure at
   decide stage to AdjudicationRefused with a re-pin hint (or add a classification-support probe
   to the handshake — a new protocol surface that itself must then be versioned; picking the
   simpler message-mapping is fine, but the design must choose), (d) CAS conflict (live-executor
   race) — keep as the store's `conflict` error, distinct from AdjudicationRefused, retryable.
2. **[LOW] Exit-code contract.** Everything exits 1 today. Decide explicitly whether refusal is
   exit 1 message-only (scheduled-family precedent) or a distinct code for automation; do not
   leave implicit.
3. **[INFO] Do not reuse `DispatchQuarantined` for adjudicate** — its message ("durably claimed
   and quarantined; resend is forbidden") is execute-specific and would mislead. The new
   `AdjudicationRefused` variant is the right home; the execute service's blanket
   Core→DispatchQuarantined mapping (`service.rs:155-158`) must NOT be copied into the
   adjudication service.

---

## Area 5 — No-credential type-level guarantee (A3)

**Verdict: the guarantee is structurally achievable and real — with two wiring traps that the
design must prohibit and one acceptance test that must prove the credential is never read.**

Verified facts:
- The execute service's struct holds `providers: Arc<dyn GroupAgentNodeDispatchProviderFactory>`
  and `credentials: Arc<dyn GroupAgentNodeCredentialSource>` and takes them in `new`
  (`application/src/group_agent_node_dispatch_execution.rs:60-80`). The send path is
  `claim()` → `read_credential()` + `build(provider)` → `dispatch_claimed()` provider stream
  (`.../dispatch_execution/service.rs:82-105,138-163`). A service without those fields has no
  code path to a provider stream.
- `GroupAgentNodeLifecycleStore` (`domain/src/group_agent_node_lifecycle/claim.rs:228-247`) is
  persistence-only (claim/terminalize/inspect); `claim_group_agent_node_dispatch` is not a send
  (the send is the provider stream), and a re-claim conflicts with the existing claim row
  (`claim.rs` store `claim_by_run` → AlreadyClaimed). So "no credential ⇒ no provider ⇒ no
  send" holds at the type level; adjudication still WRITES (terminalize) — the design must not
  oversell "effect-free" as no-DB-writes.
- The Core port is the sealed local binary, effect-free by contract
  (`forge-core/internal/graphterminal/command.go` usage text). The type-level guarantee does
  not extend to the pinned binary (full operator privileges, network access) — same-user threat
  model per ADR-0034; the design already accepts this residual, keep it explicit.

Findings:
1. **[BLOCKING] New wiring function.** The design must specify `adjudication_service()` that
   constructs the service with store/codec/bridge/metadata only, and explicitly prohibit
   `execution_service()` and `PreparedDispatchDependencies::prepare` (Area 2f2) — the latter
   reads `OPENAI_API_KEY` via `EnvironmentOpenAiCredentialSource`
   (`dispatch_execution_adapters.rs:81-87`).
2. **[MEDIUM] A3 test must prove no credential read.** The fixture already owns the pattern:
   `CREDENTIAL_SECRET` sentinel (`tests/group_agent_node_dispatch_execution_cli_support/mod.rs:36`)
   and the `RejectingCore` strand path (`quarantine.rs:28-60`). The no-credential test should
   run adjudication with (a) `OPENAI_API_KEY` absent and (b) a poisoned key value present, and
   assert success and byte-identical output in both — proving the key is never read. "Constructor
   has no provider/credential parameters" alone is necessary but not sufficient: the CLI wiring
   could still smuggle one in.
3. **[INFO] Document the reasoning.** The design should state the guarantee as "no provider, no
   credential, therefore no provider stream, therefore no send; DB writes are confined to the
   terminalize CAS" so the design gate and future maintainers cannot misread it.

---

## Cross-cutting findings

1. **[BLOCKING] Lockstep `hard_crash` classification — 4 touch points, verified feasible.**
   - Rust enum: `domain/src/group_agent_node_lifecycle/mod.rs:51-65` (serde `snake_case` →
     "hard_crash").
   - Rust validator: `valid_uncertainty` match list
     (`domain/src/group_agent_node_lifecycle/artifact_validation.rs:112-131`). The chronology
     predicate `provider_poll_started || (!terminal_seen && !stream_eof_seen)` accepts the
     all-false no-evidence artifact, and the `missing_usage` special case passes for non-
     `missing_usage` classes — verified.
   - Go map: `forge-core/internal/graphterminal/validate_artifact.go:9-15`; Go
     `validUncertaintyOutcome` (`:72-79`) accepts the same all-false artifact — verified.
   - Outcome mapping needs no change: Rust `receipt_outcome_matches_artifact` and Go
     `receiptOutcome` both map any uncertainty classification → `failed_uncertain` — verified.
   - Protocol versions stay 1: control/receipt/artifact protocol version constants are
     untouched by a classification addition — verified. Old Cores fail at decide (Area 1f3).
2. **[INFO] A2 fixture is real-executable-feasible.** The strand fixture uses a `RejectingCore`
   port (`quarantine.rs:201-203`) while `build_go_core` builds the real pinned Go binary
   (`mod.rs:371`) — the A2 test can strand, then adjudicate with the real Core and assert the
   DB post-state CAS (status `failed_uncertain`, run_version 5, lane row deleted). Feasible as
   designed.
3. **[INFO] CLI surface slots.** `parse_run_dispatch` needs `Some("adjudicate")`
   (`interfaces/src/group_agent_graph/args.rs:64-77`); `cli_usage.rs` needs the usage line +
   the dead-end sentence update (`cli_usage.rs:202/211` region); `dispatch_claim_key_error` and
   `accepts_idempotency_key` (`interfaces/src/args.rs:170-208`) must treat Adjudicate like
   Execute (idempotency key owned by GRAPH_RUN_ID) so `--idempotency-key` fails with the right
   message.

---

## Summary

Safety-critical machinery (sealed-memfd pinning, claim↔body digest binding in both validators,
single-transaction terminalize CAS, no-provider service construction) is present and verified;
the design's load-bearing claims ("plus the pinned Core", "Hub persists only digests",
"constructor structurally excludes providers/credentials", "hard_crash lockstep", "re-
adjudication zero-mutation refusal") all check out against code. What the 15-line design
deliverable does NOT yet specify — and must, before implement — is:

1. Adjudicate reuses `PinnedCoreTerminalBridge` unchanged; no hash-then-run-by-path (A1).
2. Preflight digest cross-check of operator bodies vs persisted claim, with a distinct refusal
   message (A2).
3. Never `PreparedDispatchDependencies::prepare` / never the credential source in adjudicate
   wiring; new `adjudication_service()` (A2/A5).
4. New parser rejects `--authorization - --pricing -`; stdin read before any subprocess spawn
   (A3).
5. `AdjudicationRefused` with enumerated distinct messages (not stranded / digest mismatch /
   old Core at decide / CAS conflict kept separate), explicit exit-code contract, zero-mutation
   statement (A4).
6. A3 acceptance test runs adjudicate with poisoned `OPENAI_API_KEY` present and asserts
   success + identical output (A5).
7. Lockstep `hard_crash` in all 4 classification touch points + old-Core failure documented as
   decide-time-only (cross-cutting).

None of these are holes in the existing machinery; all are design-specification gaps that the
implementer would otherwise have to guess about.
