# Trust-surface audit — subprocess-bounding design (bound-the-gate-harness-bridge)

Scope (fresh-context, independent): strict fail-safe parsing of the new env knobs,
PATH resolution + t.Setenv stubbing, no-shell/no-injection, safe truncation of
attacker-controlled output, process-group kill on every exit path, and whether
failure-mode #6 (garbage env) has a concrete mitigation. Every claim below was
verified against live code and by empirically running `time.ParseDuration` against
adversarial inputs (Go 1.26, the module's toolchain).

## Verdict

**Mechanically sound; config-semantics ambiguous.** The three trust axes that live
in code today (no-shell argv, cappedBuffer truncation, unix process-group kill) are
verified intact, and the extraction design preserves them. The two NEW trust
surfaces — env/flag parsing and the PATH stub tests — are where the deliverable set
is internally inconsistent: the two review artifacts specify **contradictory
semantics** for the same adversarial inputs, and the design artifact itself contains
no mitigation text for failure-mode #6. Blocking until reconciled at design_gate.

---

## 1. Env/flag parsing — strict parser, but TWO contradictory semantic tables

### Parser strictness: verified fail-safe (empirical)

`time.ParseDuration` (the only viable parser; `flag.Duration` uses the same) errors
on every adversarial input tested:

| input | result |
|---|---|
| `"abc"`, `"10 m"`, `"1h "`, `" 300ms"` | error |
| `"300ms\n"`, `"300ms\n500ms"` (newline) | error |
| `"１0m"`, `"10ｍ"` (fullwidth unicode) | error |
| `"999999999999999999999999h"`, `"2562048h"` (int64-ns overflow) | error |
| `"300ms\x00x"`, `"10m;rm -rf /"`, `"1e9"` | error |
| `"-1s"`, `"-300ms"` (**negative**) | **parses OK** — semantic rejection required |
| `"0"` | **parses OK** — semantic meaning must be defined |
| `"2562047h47m16.854775807s"` (max int64 ns ≈ 292 y) | parses OK — huge-but-valid |

So the parser is a hard gate against garbage, whitespace, newline, unicode, overflow,
and injection-shaped strings. That makes the *semantic* layer load-bearing: negative
and `"0"` both parse cleanly, so whichever resolver exists must reject/interpret them
**before any spawn** — exactly what Options v2 `Validate()` (hardening §3) does. The
CLI flag path inherits the same strictness via `flag.Duration`, and the delegate
precedent (main.go:60-74) already maps a parse error to exit 2 — the right fail-loud
shape.

### Blocking conflict A — garbage env: error vs default

- **hardening-subprocess-lifecycle.md §4.1 + T6**: `"abc"` / `"1h "` → **hard error**
  naming the variable *and* value; exit 2 (CLI) / exit 1 before any spawn (run/evolve).
- **acceptance_mapping_reviewer.md Pin 12**: env garbage → **default (10m), no panic**.

Both are defensible (loud-fail vs bounded-fallback), but the implementer cannot
satisfy both, and the difference is user-visible behavior on a typo. The hardening
rationale ("treating `"1h "` as 10m false-fails a legitimate slow suite") is the
stronger fail-safe argument; recommend: **garbage → hard error**. The design_gate
must publish ONE table.

### Blocking conflict B — `"0"`: unbounded vs default

- **hardening §3**: `--timeout 0` → `Unbounded` (the documented escape; negative
  rejected).
- **testing Pin 12**: env `"0"` → default; flag `"0"` → default; `-1` → unbounded
  sentinel (this last one is stale — Options v2 rejects negative, so Pin 12 as
  written cannot be implemented as-is either).

From a fail-safe standpoint `0 → default (10m)` is safer (a typo'd `0` cannot
reintroduce the hang the direction fixes); from a convention standpoint
`0 → Unbounded` matches `CommandExecutor.Timeout`'s documented zero=no-deadline
(main.go:141, command_executor.go:139-146) and keeps a CLI-visible escape hatch.
Either is acceptable once picked; the contradiction is the problem. Note the same
two reviewers AGREE on the bytes side (`--max-output-bytes 0` → default), so the
asymmetry timeout-0 vs bytes-0 must also be stated deliberately.

### Gap C — `FORGE_GATE_MAX_OUTPUT_BYTES` does not exist anywhere

Grep across `.go/.mjs/.py` and all artifacts: **zero hits** for both env names (both
are brand-new knobs). The design gives the cap only a `--max-output-bytes` flag
(gate/check/accept subcommands); the run/evolve probe path gets a **fixed 10 MiB
default with no operator override**. That is fail-safe (the cap is always on) but
asymmetric with `FORGE_GATE_TIMEOUT`, and the audit premise is only half-met. Either
add `FORGE_GATE_MAX_OUTPUT_BYTES` with the identical strict parser + the resolved
semantic table (recommended — one resolver, one test table), or document the
asymmetry in the design. If added: negative → error, garbage → error, `0` → default
(the two reviewers already agree here), huge → `strconv.Atoi` overflow error.

### Gap D (minor) — no sanity upper bound on timeout

`"2562047h47m16s"` is accepted (≈292 y, operationally unbounded). Not a hang vector
(the deadline still fires and group-kill still works, and the output cap is
independent), and overflow already errors. Non-blocking: the timeout FAIL text must
print the *effective* deadline (hardening §3 already requires this) so a huge value
is visible; optionally warn on >24h.

---

## 2. PATH resolution + t.Setenv stubs — mechanism verified, one NEW pin

Verified the mechanism end-to-end: `exec.Command("node", …)` resolves argv[0] via
`exec.LookPath` against the *test process's* `PATH` at construction time; the child
(no `cmd.Env` set today in gate.go's `run`) inherits the parent env at execve, so a
stub dir prepended via `t.Setenv` intercepts both the parent-side resolution and the
child's own lookups. The existing pins are correct and sufficient:
- prepend-never-replace PATH (Pin 4) — also what the harness needs: acceptance-
  kernel.mjs:41 `spawnSync` spreads `process.env`, so a replaced PATH would break the
  harness's own git/go/node resolution if the real harness ever ran;
- no `t.Parallel` (Pin 5) — `t.Setenv` panics under it and the os.Stdout swap is
  process-global;
- explicit `--root` + explicit env control (Pin 6) — `FORGE_REPO_ROOT` and a
  CI-set `FORGE_GATE_TIMEOUT` must not leak in;
- the stub replaces the interpreter binary, so the real acceptance.mjs (which
  re-spawns gates ~4× nested, test_acceptance.mjs:91) never runs (Pin 7);
- stub is a `#!/bin/sh` script — absolute interpreter, PATH-independent — in a
  `t.TempDir()` (0700, test-user-owned): no cross-user hijack, no argv dependence.

**NEW PIN (P19) — env snapshot must be taken at spawn time, never cached.**
The hardening amendment (§4.2) makes execbound set the child env explicitly (full
inheritance). If an implementer builds that env from `os.Environ()` **at package
init or cached in a package-level var**, the t.Setenv stub PATH would be bypassed:
`LookPath` finds the stub but the child's real `node`/`python3` resolution or the
inherited env would diverge — at best a vacuous test, at worst the real harness runs
nondeterministically. The orchestrator precedent already snapshots at call time
(childEnv reads `os.Environ()` per invocation, env_policy.go:80-103) — execbound must
do the same, and one stub test should assert the child actually saw the stub PATH
(e.g. stub `printf '%s' "$PATH"` and assert the stub dir is present) to prove the
injection, not just the interception.

Also verified: gate spawns must NOT adopt the orchestrator's minimal-env allowlist
(`defaultAgentEnv`) or `RestrictedEnv` fixed-host-PATH — that policy is for
untrusted agent commands; the harness is repo-trusted and needs ambient
PATH/HOME/certs (hardening §4.2 states this explicitly; code confirms the harness
spreads `process.env`). The trade-off (a compromised parent env controls which
`node` runs) is the standard trusted-CLI boundary and is correctly out of scope.

---

## 3. No shell, no injection — verified clean

- Zero occurrences of `sh -c` / `bash -c` / `/bin/sh` in `internal/gate` and
  `cmd/forge` non-test Go code (grep). All spawns are `exec.Command` with **fixed
  argv**: `["node","harness/gate.mjs"]`, `["python3","harness/check.py","."]` (argv[1]
  is the literal `"."`), `["node","harness/acceptance.mjs","--json"]` (gate.go:69,
  92, 100, 140). The extraction preserves this (execbound takes argv arrays; no
  shell mode exists in the design).
- **Gate names never reach argv.** Verified the full flow: `runProbe.runGate(name)`
  (engine_build.go:374-382) → `ResolveGate(root, name, probe)` (resolve.go:72) →
  `Gate(root)`/`Check(root)` (resolve.go:150-152) — the name is a lookup key into
  the probe map and `Result.Name`; it is interpolated only into log lines and error
  text (gate_runtime.go:47, `fmt.Errorf("phase %s: required gate %q not OK: %s")`),
  never into a command line. The only attacker-influenceable string in a spawn is
  `root`, which goes into `cmd.Dir` (a bad dir is a start failure → honest FAIL) —
  never argv.
- **Harness output is never executed.** ProbeAll output is `json.Unmarshal`ed into
  `probeRow` (gate.go:152), then mapped into status/category maps. A hostile
  acceptance.mjs can inject arbitrary criterion names (map keys / log text) but no
  code path turns output into a command. The harness's own grandchildren
  (acceptance-kernel.mjs:41, acceptance-project.mjs:109-111) use `spawnSync(cmd,
  args)` — argv arrays, no shell string.
- **Subprocess output as truncation evidence** (§4) rides in `Result.Output` / error
  strings only — same non-execution guarantee.

---

## 4. Truncation of attacker-controlled output — verified sound, incl. JSON wrap

`cappedBuffer` (command_executor.go:385-423) verified:
- retains **exactly** `cap` bytes whenever total ≥ cap (fills to room, drops the
  rest);
- `Write` returns `len(p)` always → the child **never blocks** on a full pipe and
  kernel pipe buffers stay bounded (drain, not backpressure);
- marker `" …[output truncated: retained %d of %d bytes (--max-output-bytes)]"` is
  appended in **both** `rendered()` and `observed()` — the truncation evidence
  survives into every user-visible surface, and counts are exact/deterministic
  (assertable as literals, per Pin 9/11);
- truncated-JSON wrap: `ProbeAll`'s parse error preserves the back-compat prefix
  `"gate: parsing acceptance --json:"` (gate.go:152) — the design wraps it with
  retained/total bytes without breaking the prefix (Pin 10). The guaranteed-invalid-
  under-any-cut stub (unterminated array) is the right test shape: a truncated valid
  prefix would otherwise parse as a *shorter* JSON document and silently change the
  verdict set — the "attacker-controlled cut makes honest-but-wrong" hazard the
  unterminated-array pin eliminates. Verified that `json.Unmarshal` of a truncated
  array errors (incomplete array literal) — deterministic.

One note: the probe path is the one place truncation can *change semantics* (a
truncated-but-valid JSON prefix would misreport gates) — the design's wrap makes the
truncation visible, but the count wrap does not make the error *distinguishable*
from a genuinely malformed harness. That is acceptable (both are FAIL-loud), and the
retained/total counts let an operator diagnose. Stated, not blocking.

---

## 5. Process-group kill / exit paths — verified against code

- `setupProcessGroup` (command_executor_unix.go:25-58) verified: `Setpgid` +
  `Cancel = syscall.Kill(-pid, SIGKILL)` (group kill; ESRCH best-effort) +
  `WaitDelay = processGroupGrace = 2s` (pipe-close backstop). Non-unix is a no-op
  and — confirmed — **`WaitDelay` is set only in the unix file**, so the hardening
  finding that non-unix has no pipe-close backstop is real; moving `WaitDelay` to
  common execbound code is correct and portable (os/exec `WaitDelay` is
  platform-independent).
- All 12 rows of the hardening exit inventory map onto verified code paths:
  `Engine.RunGate = func(string) gate.Result` with **no ctx** (orchestrator.go:95;
  `callGate`, gate_runtime.go:72-78); `runProbe.runGate` locks only name bookkeeping
  then calls `ResolveGate` unlocked (engine_build.go:374-382); the run ctx exists and
  is wired (`eng.Ctx = ctx`, main.go:259; SIGINT via `withSignalCancellation`,
  evolve.go:469); per-wave fail-fast ctx exists (`waveCtx`, parallel.go:155) — so the
  closure-capture threading (A1.2) has real seams to hang on: `resolveStageHostBoundary`
  (runlock_wire.go:103, one call site engine_build.go:459) and `buildLoop`
  (evolve.go:276, one call site evolve.go:252). Both are one-call-site signatures —
  the zero-churn claim checks out.
- The standalone `delegate` (main.go:60-74) today has **no signal context** and the
  gate child is spawned **without** Setpgid — so today's Ctrl-C works by
  group-sharing accident; the design's Setpgid would break it without the delegate's
  own `signal.NotifyContext` (A1.2). Verified: this is the one place the extraction
  would otherwise regress SIGINT behavior, and the amendment closes it.
- The grandchild-pipe hazard is real but weaker than the orchestrator's: verified
  `spawnSync` default stdio is `'pipe'` (acceptance-kernel.mjs:41) — grandchildren
  write to **node-owned** pipes, so the direct-child kill releases forge's pipe and
  `cmd.Run()` returns; the load-bearing reason for group kill is the orphan leak
  (wedged `go test` holding `.git` locks/temp dirs corrupting the *next* probe).
  Verified this matches the hardening doc's framing.

---

## 6. Failure-mode #6 (garbage env): hand-wave in the design artifact, concrete but CONTRADICTORY in the reviews

The audit question, answered honestly:
- **The design artifact itself is a hand-wave.** `task-1-design.md` is 18 lines; the
  failure-modes bullet is `"8 failure modes (… garbage env, …) each with mitigation"`
  — no mitigation text exists in the artifact the implementer reads first. Its
  config bullet even carries the superseded `"negative timeout = explicit unbounded
  escape"` semantics that Options v2 rejects.
- **The mitigation IS concrete — but only in the review layer, and split across two
  contradictory sources**: hardening §4 (strict `time.ParseDuration`, fail-loud
  naming var+value, exit 2/1 before any spawn, negative rejected, `0`→Unbounded,
  full-inheritance child env, Options `Validate()` before fork) vs testing Pin 12
  (garbage→default, `0`→default, `-1`→unbounded). §8 of the hardening doc
  supersedes the negative rule but says nothing about the `0`/garbage conflict.

**Fix required before implement**: design_gate must emit one authoritative
semantic table for {garbage, "", "0", negative, huge, unicode, newline} ×
{env, flag} × {timeout, bytes}, and the design artifact must carry it. Recommend
(fail-safe direction): garbage/whitespace/unicode/newline → hard error naming
var+value; negative → error (both); `""` → default; `0` → **default for both knobs**
(typo-safe; `Unbounded` remains reachable only via the named `Options.Unbounded`
bool in code — the orchestrator rewire maps its zero → `Unbounded`); huge-but-valid
→ accepted with the effective deadline printed in any timeout text.

---

## Consolidated finding list

| # | Severity | Finding |
|---|----------|---------|
| C1 | **Blocking** | Garbage-env semantics contradict across artifacts (hard error vs default); design_gate must publish one table |
| C2 | **Blocking** | `"0"` semantics contradict (Unbounded vs default); testing Pin 12's `-1 → unbounded` is stale vs Options v2 |
| C3 | **Blocking** | Failure-mode #6 exists only as a word in the design artifact; mitigation lives only in review artifacts |
| G1 | Gap | `FORGE_GATE_MAX_OUTPUT_BYTES` exists in no artifact; run/evolve cap is fixed 10 MiB (fail-safe but asymmetric) |
| G2 | Gap | No sanity upper bound on timeout (~292 y max accepted); non-blocking if effective deadline is printed |
| P19 | New pin | execbound child-env snapshot must be os.Environ() at spawn time, never package-init-cached, or t.Setenv stubs are bypassed |
| — | Verified clean | No shell/injection; fixed argv; gate names never reach argv; truncation exact + marker on both surfaces; unix group-kill + WaitDelay verified; non-unix WaitDelay gap real; delegate SIGINT regression real and closed by A1.2; ParseDuration strict on all adversarial classes |
