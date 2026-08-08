All evidence gathered. Here is the Stage 04 review.

---

# Stage 04 — Implementation Review: Sandbox isolation runners + CLI wiring

**Roles:** Staff Engineer + Tech Lead · **Subsystem:** Firecracker microVM / Docker runners and CLI wiring
**Prior Stage 01–03 findings:** N/A — none were provided to this session (only the scope file `docs/reviews/examples/sandbox-context.yaml`, untracked).

## 1. Findings (sorted by severity)

| # | Severity | Title | Surface | Location | Evidence | Failure scenario | Impact & likelihood | Fix | Risk/effort |
|---|---|---|---|---|---|---|---|---|---|
| F1 | High | Sandboxed execution silently drops the claude prompt (PromptViaStdin + sandbox) | CLI wiring | `command_executor.go:167-176` (sandbox branch), `sandbox_config.go:87-108`, `sandbox/sandbox.go:19-27` | `prepareInput` strips `-p <prompt>` into `input`/`useStdin` (L167); the sandbox branch calls `executeSandboxed(runCtx, p.Name, argv, timeout)` (L175) and `input` is never passed on; `sandbox.Runner.Run(ctx, argv, timeout)` has no stdin parameter; the only wiring test (`TestSandboxRunnerReceivesArgvAndTimeout`) sets no `PromptViaStdin`, so it asserts the un-stripped argv and masks the bug | `forge run --executor=command --sandbox docker` (production: `isClaude`→`PromptViaStdin=true`): the guest runs `claude [flags]` with no prompt and empty stdin → claude errors or hangs; the agent never receives its task | The sandbox feature is functionally dead for its primary purpose (running a real agent). Likelihood: 100% on the production path | Thread `input` through `executeSandboxed` into the Runner contract; docker: `cmd.Stdin`; firecracker: inject `/forge-stdin` into the rootfs and redirect in the guest init | Interface change, contained (3 implementers); medium effort |
| F2 | High | `guestOutput` corrupts any guest stdout line containing `] ` | Stock binary (forge) | `firecracker_runner.go:402-426` (`guestOutput` strip at ~L417) | **Live-proven**: guest `/bin/echo 'LEFT] RIGHT'` → extracted output `"RIGHT"` (real microVM, 1.4 s boot). The unit fixture `TestGuestOutputExtractsBetweenMarkers` encodes an invented `[anonymous-instance:serial]` prefix; the real serial stream is raw guest bytes (only kernel lines carry `[ 0.000000]` timestamps) | Any agent output containing `] ` (claude JSON `"result": "...] ..."`, diff text, `[FAIL]`-style lines) is mangled before `Observe`/verdict/cost parsing | Captured agent output is corrupted; contract-violating output fidelity. Likelihood: any output containing `] ` | Strip only a leading kernel-timestamp (`^\s*\[\s*\d+\.\d+\]\s*`) or nothing; correct the unit fixture; add a `LEFT] RIGHT` regression test | Low risk; small effort |
| F3 | High | Sandbox path bypasses the `MaxOutputBytes` resource guard (unbounded capture) | Stock binary (forge) | `docker_runner.go:57-58` (`bytes.Buffer` unbounded), `firecracker_runner.go:106-111` + `guestOutput` `os.ReadFile` (unbounded serial.log disk + memory), `sandbox_config.go:92-94` (cap applied only after the runner returned) | `command_executor.go:69-72` documents `MaxOutputBytes` as the guard that "cannot OOM the orchestrator"; both runners retain unbounded output before the cap exists | A runaway/malicious agent inside the sandbox emits gigabytes → host OOM (docker) or /tmp disk fill + OOM on read (firecracker) | The sandbox is exactly the boundary meant to contain this; guard bypass. Likelihood: high for buggy/runaway agents | docker: capture into a capped+draining writer (reuse `cappedBuffer`); firecracker: `os.Pipe` + drain goroutine into a capped buffer instead of the serial file | Low risk; medium effort |
| F4 | High | `FirecrackerRunner.MemoryMB` is declared and documented but never applied | Stock binary (forge) | `firecracker_runner.go:43-44`, `boot()` (no `PUT /machine-config`), `sandbox_config.go:72` (auto-wire drops it), CLI has no memory flag | grep: `MemoryMB` appears only at the declaration; the docker twin honors it (`docker_runner.go:53-54`); live guest boots at Firecracker's default 128 MiB | Operator sets `MemoryMB=2048` → silent no-op; real agent workloads (claude+node) in 128 MiB → guest OOM; "0 uses 128" default is dangerously small | Documented control silently inert. Likelihood: 100% for any firecracker memory config | `PUT /machine-config` (`mem_size_mib`) in `boot()`; thread `MemoryMB` through the auto-wire; optional `--sandbox-memory` flag; fake-API-server test | Low risk; small-medium effort |
| F5 | Medium | Sandbox error classification drifts from the typed contract: cancellation → retryable timeout; message-string matching | Stock binary (forge) | `sandbox_config.go:98-101` | `executeSandboxed` maps any `runCtx.Err() != nil` (both `context.Canceled` and `DeadlineExceeded`) to `KindTimeout`; the host path (`exec_error.go` `classifyRunErr`) maps only `DeadlineExceeded`→`KindTimeout`, `Canceled`→`KindFailed`; `KindTimeout` is `Retryable()` (`exec_error.go:117-121`). Also `strings.Contains(err.Error(), "timed out")` contradicts exec_error.go's "keeps that judgement out of the caller's guesswork" contract | Ctrl-C (SIGINT) during a sandboxed phase → `Canceled` → `KindTimeout` → orchestrator retries the phase and spawns another agent (budget burn) instead of aborting | Retry/cancel semantic drift between host and sandbox paths. Likelihood: every user interrupt | `errors.Is(runCtx.Err(), context.DeadlineExceeded)`→`KindTimeout` else `KindFailed`; typed timeout sentinel from runners; drop string matching | Low risk; small effort |
| F6 | Medium | Transient marker-read / debugfs errors escalate to permanent `KindConfig`; docker daemon blips likewise | Stock binary (forge) | `firecracker_runner.go:339-353` (`waitForMarker`), `356-371` (`readMarker`); `docker_runner.go:44-49` (`checkReady` runs `docker info` per Run) | Any `readMarker` error → `configFault` (permanent, non-retryable); not-found detection matches only two literal strings; the host polls a live ext4 image the guest is concurrently writing | A transient debugfs I/O error (image momentarily inconsistent) or a busy daemon aborts the run as permanent config; the retry machinery (designed exactly for transient faults) never engages | Availability; flaky VM runs kill whole runs. Likelihood: low-medium | Bounded retry of read errors before escalation; classify daemon probe failures distinctly | Low risk; small effort |
| F7 | Medium | Docker timeout path leaks orphan containers | Stock binary (forge) | `docker_runner.go:60` (`exec.CommandContext` on the docker client only; no `setupProcessGroup`, no container cleanup) | **Live-proven**: `docker run --rm alpine sleep 30`, client SIGKILLed (what `CommandContext` does on timeout) → container remained `Up`; `--rm` does not stop it | Timeout (retryable, so repeated) leaves running containers consuming host RAM/CPU with no bound; each retry leaks another | Resource exhaustion on the host. Likelihood: every docker timeout | On timeout issue `docker rm -f` (via `docker create/start/wait` or a kill path); add live cleanup test | Low-medium risk; medium effort |
| F8 | Low | Dead code: `copyFile` never referenced | Stock binary (forge) | `firecracker_runner.go:428-441` | grep: definition only; `copyTree` is the live path | — | Maintainability | Delete | None; trivial |
| F9 | Low | `waitForSocket` early-exit detection can never fire | Stock binary (forge) | `firecracker_runner.go:227` | `cmd.ProcessState` is populated only by `Wait()`; the only `Wait()` is in `stop()`, after `waitForSocket` returns — `ProcessState` is always nil here | An early VMM crash is detected only by the fixed 10 s deadline, with the wrong message and 10 s delay | Diagnostics latency; dead branch | Reap in a goroutine and select on its channel, or drop the branch | Low risk; trivial |
| F10 | Low | Misplaced doc comment: "sandboxConfigError enforces the isolation boundary…" attached to `prepareInput` | Stock binary (forge) | `command_executor.go:196-198` | comment describes `sandboxConfigError` (`sandbox_config.go:40`) | — | Maintainability | Move the comment | None; trivial |
| F11 | Low | `--sandbox` silently no-ops under the default dry executor | CLI | `executor.go:34-36` (returns `DryRunExecutor` when `executor != "command"`); flags at `main.go:196-198` | no warning anywhere for sandbox flags with a dry executor | `forge run --sandbox docker` without `--executor command` → flags parsed, never consumed, no warning; an operator believing isolation is active gets host narration | Honesty/comprehension gap | Warn "ignored (dry executor)" or reject `--sandbox` unless `--executor=command` | Low risk; small |
| F12 | Info | `sandbox` package has zero tests; Runner contract boundary untested | Tests | `sandbox/sandbox.go` | `go test ./internal/orchestrator/sandbox/` → "[no test files]" | — | Contract drift at the package boundary | Add contract tests (output/exit/err semantics) | None; small |
| F13 | Info | Docs drift: ROADMAP still lists Firecracker as BLOCKED-EXTERNAL (2026-07-27) | Docs | `ROADMAP.md:66` and the "明确遗留缺口" paragraph | `docs/external-resource-verification.md` (2026-08-05) records host VERIFIED; the live test executes here (see §7) | Operators read stale blocking status | Docs/contract drift | Refresh the ROADMAP row (LiteLLM second-credential part remains accurate) | None; trivial |
| F14 | Info | Docker isolation is network-none only (root, default caps, writable rootfs, no seccomp); `docker info` probe per Run adds latency | Stock binary (forge) | `docker_runner.go:34-75` | matches the documented contract ("no network") exactly | — | Weaker boundary than the firecracker path; latency | Track as hardening backlog, not a defect | — |

## 2. Gate report

| Check | Command/source | Result | Evidence | Required action |
|---|---|---|---|---|
| Volume/root caps | `node harness/gate.mjs` | PASS | 1272 files, ≤500 lines/file, root ≤15 | none |
| Architecture (8 checks) | `node harness/arch/arch-check.mjs` | PASS | 8/8 (layering, package, fanin, cognitive, naming, function-length, cycles, drift-guard) over 1020 source files | none |
| Governance | `python3 harness/check.py` | PASS | 12 checks | none |
| Secrets | `node harness/secret-scan.mjs` | PASS | 1229 files, 0 hardcoded secrets | none |
| Build | `go build ./...` (forge-core) | PASS | clean | none |
| Vet/typecheck | `go vet ./...` | PASS | clean | none |
| Tests | `go test -count=1 ./...` (forge-core) | PASS | 1330 tests, incl. `internal/orchestrator`, `docker`, `firecracker`, `cmd/forge` | none |
| Race | `go test -race` (orchestrator, docker, firecracker) | PASS | clean | none |
| Acceptance | `node harness/acceptance.mjs` (`forge accept`) | PASS (ACCEPTED) | 9 pass · 0 fail · 2 honest N/A | none for the gates; findings F1–F7 are contract defects the gates do not model |
| Lint (go) | golangci-lint | N/A (honest) | not installed — harness-reported, not fabricated | none |
| Coverage (go) | harness coverage adapter | N/A (honest) | no tool/config — harness-reported | none |

## 3. In-scope violations (measured vs. governing threshold)

No numeric gate is violated. The violations are **contract** violations — the governing "threshold" is the documented contract:

| File/function | Measured value | Governing contract | Exemption |
|---|---|---|---|
| `command_executor.go` Execute sandbox branch → `sandbox.Runner` | prompt (`input`) never delivered to the runner | `PromptViaStdin` doc: "sensitive repository context never reaches ps" — delivery is the point | none |
| `firecracker_runner.go` `guestOutput` | `"LEFT] RIGHT"` → `"RIGHT"` (live) | runner contract: "returns the guest stdout" (faithful capture) | none |
| `docker_runner.go` Run / `firecracker_runner.go` serial.log | unbounded capture | `MaxOutputBytes` doc: "cannot OOM the orchestrator" | none |
| `firecracker_runner.go` `MemoryMB` / auto-wire | field never read; no machine-config call | field doc: "caps the microVM RAM; 0 uses 128" | none |
| `sandbox_config.go` `executeSandboxed` | Canceled→KindTimeout; string-matched "timed out" | `exec_error.go` typed-kind contract; host-path parity | none |
| `docker_runner.go` timeout path | orphan container `Up` after client SIGKILL (live) | runner contract: fresh `--rm` container per run (no leak implied) | none |

## 4. Refactoring plan (smallest safe set)

| Target | Extraction/change | Destination | Tests | Effort |
|---|---|---|---|---|
| Prompt delivery | Extend `Runner.Run` with a `stdin string` param; thread `input` from `prepareInput` → `executeSandboxed`; docker: `cmd.Stdin`; firecracker: write `/forge-stdin` before `mke2fs`, redirect in guest init | `sandbox/sandbox.go`, `sandbox_config.go`, `docker_runner.go`, `firecracker_runner.go` | Wiring test with `PromptViaStdin=true` asserting the fake runner receives the prompt; live stdin-echo tests for both runners | M |
| Output fidelity | Replace bare `] ` strip with kernel-timestamp-only strip (or none); fix the unit fixture to the real raw-serial format | `firecracker_runner.go` | Unit regression with `LEFT] RIGHT`; live microVM probe | S |
| Bounded capture | docker: `cappedBuffer` instead of `bytes.Buffer`; firecracker: `os.Pipe` + drain goroutine into a capped buffer (drop serial file) | `docker_runner.go`, `firecracker_runner.go` | Over-cap unit tests; live large-output guest test | M |
| MemoryMB wiring | `PUT /machine-config` in `boot()`; thread through auto-wire; optional `--sandbox-memory` | `firecracker_runner.go`, `sandbox_config.go`, `main.go` | Fake API-server test asserting the body; docker `--memory` argv test | S–M |
| Typed classification | `errors.Is(DeadlineExceeded)`→KindTimeout, `Canceled`→KindFailed; sandbox timeout sentinel; drop string matching | `sandbox_config.go`, runners | Negative tests: Canceled→KindFailed; timeout without message text | S |
| Docker orphan cleanup | On timeout, remove the container (`docker create/start` or `rm -f` path) | `docker_runner.go` | Live test: kill client, assert container gone | M |
| Marker-read resilience | Bounded retry of `readMarker` errors before `configFault` | `firecracker_runner.go` | Injected-failure unit test | S |
| Dead code / dead branch | Delete `copyFile`; reap via goroutine in `waitForSocket` | `firecracker_runner.go` | existing suite | S |
| Honesty/comments | Move misplaced comment; warn or reject `--sandbox` under dry executor | `command_executor.go`, `executor.go` | CLI-level test | S |
| Docs | Refresh ROADMAP Firecracker status row | `ROADMAP.md` | n/a | S |

## 5. Final proposed interface signature (required — F1)

```go
// sandbox/sandbox.go
// Runner executes a command inside an isolated environment. Run returns the
// captured output, the guest exit code (0 on success), and an infrastructure
// error (config fault, timeout, or transport failure). A non-zero code with
// a nil error is a clean run that failed. stdin carries the command's standard
// input (the claude prompt under PromptViaStdin); "" means no input.
type Runner interface {
	Run(ctx context.Context, argv []string, stdin string, timeout time.Duration) (output string, exitCode int, err error)
}
```

Implementers: `docker.Runner` (via `cmd.Stdin`), `firecracker.FirecrackerRunner` (via injected `/forge-stdin` + guest init redirect), and the test `fakeRunner`.

## 6. Technical-debt table

| Severity | Item | Owner | Disposition | Reason |
|---|---|---|---|---|
| High | F1 prompt drop | Sandbox owner | **Must fix** this stage | Feature is non-functional for real agents |
| High | F2 output corruption | Sandbox owner | **Must fix** this stage | Proven data corruption of captured agent output |
| High | F3 unbounded capture | Sandbox owner | **Must fix** this stage | Resource-guard bypass defeats the isolation boundary |
| High | F4 dead MemoryMB | Sandbox owner | **Must fix** this stage | Documented control inert; 128 MiB default blocks real workloads |
| Medium | F5 classification drift | Orchestrator owner | Fix with F1 batch | Retry/cancel semantics must stay uniform |
| Medium | F6 transient→permanent escalation | Sandbox owner | Fix this stage | Availability |
| Medium | F7 docker orphan leak | Sandbox owner | Fix this stage | Proven leak, accumulates under retries |
| Low | F8/F9/F10 dead code, dead branch, comment | Sandbox owner | Fix opportunistically with F1 batch | Maintainability |
| Low | F11 dry-executor silence | CLI owner | Fix this stage | Honesty |
| Info | F12 sandbox package tests | QA | Backlog | Contract boundary coverage |
| Info | F13/F14 docs drift, docker hardening | Docs/DevOps | Backlog | Non-defect |

## 7. Recommendation

**Overall readiness:** The isolation mechanics are genuinely solid and **live-verified on this host**: real KVM microVM boot in ~1.4 s with fresh-ext4-per-run, debugfs marker read-back, serial capture, fail-closed auto-wiring, typed-kind skeleton, honest env-gated skips, and a green `forge accept` (9 pass / 0 fail / 2 honest N/A). But the end-to-end agent path is broken at three independent points (F1 prompt never delivered, F2 output corrupted, F3 unbounded capture), one documented control is inert (F4), and the docker timeout leaks containers (F7). The gates do not model these defects — they are contract violations, not threshold violations.

**Critical/High counts:** 0 Critical · 4 High (F1–F4).

**Validation run vs. inferred:**
- *Executed:* all gates in §2 (build, vet, 1330 tests, `-race`, arch 8/8, check 12/12, secret-scan, `forge accept`); live firecracker microVM test; live docker success + exit-7 tests; live auto-wire docker; two adversarial live probes (F2 corruption **reproduced**, F7 orphan **reproduced**).
- *Inferred:* claude-in-sandbox end-to-end behavior (blocked by the F1 prompt drop — moot until fixed); debugfs race frequency (F6); memory pressure at 128 MiB (F4) — structurally evident, not reproduced.

**Ship decision: `no`** for the sandbox feature as wired. The runners are close: condition is **fix F1–F4 (must) and F5–F7 (cheap, same batch), then re-run `forge accept` + live probes**.

**Must-fix:** F1 (prompt delivery + interface), F2 (output fidelity), F3 (bounded capture), F4 (MemoryMB wiring).

**Explicitly deferred:** F13/F14 (docs refresh, docker isolation hardening); F12 (sandbox package tests) — backlog.

**Unresolved merge blockers:** none beyond the must-fix list; the gates themselves are green and the change is not gated on a numeric violation.
