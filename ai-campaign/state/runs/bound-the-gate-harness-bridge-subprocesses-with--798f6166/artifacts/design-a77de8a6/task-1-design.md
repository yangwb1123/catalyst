Design written to `ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/design-a77de8a6/task-1-design.md`. Summary:

## Evidence verification

**All 10 citations verified exact** (one drift, honestly admitted by the source: `commandContext` at 299-308 not 280-310). Corroborated beyond the summary: `ResolveGate` live-spawns `Gate`/`Check` from 4 real sites (engine_build.go:381, runlock_wire.go:129, resolve.go:72, gates.go:56); the grandchild-pipe hazard is real (acceptance.mjs forks gate.mjs/check.py/node --test, acceptance-project.mjs forks go test); the orchestrator's solved pattern is real (CommandContext + process group + cappedBuffer, 10 MiB default).

**One critical finding**: the requirements artifact is **not** the claimed full spec — it's a 17-line summary. I reconstructed R1-R7 semantics from the summary + the direction JSON in `pipeline.yaml` (which carries the 4 acceptance checks verbatim) and flagged this in the design.

## Design decisions

- **Extract, don't copy**: new leaf package `internal/execbound` holds the bounded-run mechanism (CommandContext + process-group kill + cappedBuffer + honest truncation marker); `orchestrator` rewires to it (behavior byte-identical, its test suite is the net), `gate` consumes it. Copying rejected — the grandchild-pipe logic must not drift. Layering preserved: no cycle (orchestrator→gate→execbound).
- **API**: additive `GateWith/CheckWith/AcceptWith/ProbeAllWith/ResolveGateWith/GatesGreenWith(root, Options)`; legacy funcs become wrappers — zero caller churn. `Options{Timeout, MaxOutputBytes}`: zero-value = safe default (10m/10MiB), negative timeout = explicit unbounded escape. Deadline established **inside** the probe runner (verified: `runGates` calls `RunGate(name)` with no ctx).
- **Honest failures (R4)**: timeout → `FAIL` with "timed out" text naming the knob (never NA); truncation marker rides in Output; ProbeAll truncation-broken-JSON wraps the parse error with retained/total bytes.
- **Config (R5)**: `--timeout`/`--max-output-bytes` on the gate/check/accept subcommands; `FORGE_GATE_TIMEOUT` env for run/evolve. **Explicitly rejected**: reusing run/evolve's existing `--timeout` for gates — a user's `--timeout=30s` for agents would silently cap the acceptance suite and break working runs.
- **The one intentional break**: >10m gate spawns now FAIL instead of hanging-then-succeeding, with documented escape hatches.
- **Out of scope** (per requirements): git shims (gates.go:349/423) and yamlpath python3 shim.
- **8 failure modes** (false timeout on slow repos, non-unix best-effort kill, probe-timeout→N/A degrade, truncated JSON, garbage env, SIGINT orphans now self-terminating, extraction regression, arch-budget trips) each with mitigation.
- **Acceptance mapping**: the 4 direction checks → named tests with stub `node`/`python3` on PATH via `t.Setenv`, concrete assertions, and 5s wall-clock bounds (300ms deadlines + 16× headroom); plus config-precedence and regression tests beyond the 4.
