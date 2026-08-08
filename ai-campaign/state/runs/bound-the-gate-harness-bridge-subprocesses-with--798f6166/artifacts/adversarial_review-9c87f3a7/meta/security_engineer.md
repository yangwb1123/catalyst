Audit complete. Report: `ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/adversarial_review-9c87f3a7/meta/trust-surface-review.md`

## Verdict: mechanically sound, config-semantics ambiguous — 3 blocking, 2 gaps, 1 new pin

**Verified clean against live code:**
- **No shell / no injection** — zero `sh -c`/`/bin/sh` in gate+cmd/forge; all spawns are `exec.Command` with fixed argv (checked gate.go:69/92/100/140). Gate names reach only `Result.Name`/log text, never argv (`runProbe.runGate` → `ResolveGate` → `Gate`/`Check` confirmed); `root` goes only into `cmd.Dir`. Harness grandchildren use `spawnSync(cmd, args)` argv arrays.
- **Parsing** — ran `time.ParseDuration` empirically: garbage, whitespace, newline, fullwidth-unicode, int64 overflow, NUL, `";rm -rf /"` **all error**. Two inputs parse cleanly: **negative and `"0"`** — so semantic rejection is load-bearing, exactly what Options v2 `Validate()` provides.
- **Truncation** — `cappedBuffer` retains exactly cap, never blocks the child, marker on both `rendered()`/`observed()`; JSON wrap preserves the `"gate: parsing acceptance --json:"` prefix; unterminated-array stub is the right guarantee (a truncated-valid prefix would silently shorten the verdict set).
- **Process-group kill** — `Setpgid` + `-pid SIGKILL` + `WaitDelay 2s` verified (command_executor_unix.go:25-58); the non-unix `WaitDelay` gap is real; `Engine.RunGate` genuinely carries no ctx; both threading seams (`resolveStageHostBoundary`, `buildLoop`) are one-call-site signatures; delegate today has no signal ctx, so the SIGINT regression + A1.2 fix checks out.

**Blocking findings:**
1. **Garbage-env semantics contradict across artifacts**: hardening §4 says garbage → hard error; testing Pin 12 says garbage → default. Same for `"0"`: Unbounded vs default (and Pin 12's `-1 → unbounded` is stale vs Options v2's negative-rejection).
2. **Failure-mode #6 is a hand-wave in the design artifact itself** — the 18-line artifact lists "garbage env" among 8 failure modes with no mitigation text; the concrete mitigation exists only in review artifacts, and those disagree.
3. Same contradiction must be resolved in **one authoritative table** at design_gate.

**Gaps:** `FORGE_GATE_MAX_OUTPUT_BYTES` exists in **no artifact** (only the flag; run/evolve gets a fixed 10 MiB — fail-safe but asymmetric — add it or document); no upper sanity bound on timeout (~292 y accepted; harmless if effective deadline is printed).

**New pin (P19):** if execbound sets the child env explicitly, the `os.Environ()` snapshot must be taken at spawn time — a package-init-cached env would bypass the t.Setenv stub PATH and run the real node (vacuous test). The orchestrator precedent already snapshots per-invocation (env_policy.go:80).
