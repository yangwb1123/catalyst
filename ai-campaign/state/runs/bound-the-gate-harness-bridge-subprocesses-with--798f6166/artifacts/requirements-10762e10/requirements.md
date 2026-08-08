Done. The requirements specification is at:

**`ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/requirements-10762e10/requirements.md`**

## Summary

**Evidence verification — all 10 citations checked against the repo:**
- ✅ Exact: `gate.go:65-75` (`run()` — bare `exec.Command` + `CombinedOutput`), `gate.go:138-149` (`ProbeAll` — bare `cmd.Output()`), `engine_build.go:351/354-366` (runProbe wiring `gate.ProbeAll` on the `forge run` critical path), `gates.go:349/423` (git diff shims), `yamlpath.go:134` (python3 shim), `cli_dispatch.go:26-28` (gate/check/accept CLI entries), `command_executor.go:215-224` (the solved pattern: `CommandContext` + process group + cappedBuffer)
- ✅ With drift: `command_executor.go:280-310` — the intended `commandContext` function actually spans 299-308; same region, noted in the table

**Key findings that shaped the spec:**
- Layering constraint: `orchestrator` imports `gate`, so the bounded-run mechanism must be copied/extracted, never imported back (gate_runtime.go, orchestrator.go)
- The orchestrator's `runGates` calls the injected `RunGate` with no ctx — the deadline must be established inside the probe runner, not derived from the orchestrator ctx
- `ResolveGate` runs `complexity→Gate`/`arch→Check` **live** per gate resolution, so the run/evolve paths spawn node/python3 beyond just `ProbeAll`
- Confirmed the grandchild-pipe hazard: `acceptance.mjs:65-96` forks gate.mjs/check.py/`node --test`, and `acceptance-project.mjs:110-116` runs `go test`

**Spec structure:** evidence table → confirmed problem → scope (git/yamlpath shims explicitly out of scope with rationale) → 7 requirements (R1 ctx-bound bridge, R2 process-tree kill, R3 output cap 10 MiB, R4 honest truncation/timeout errors, R5 timeout config: 10m default + `--timeout` flag + `FORGE_GATE_TIMEOUT` env, R6 wiring all 4 cmd/forge call sites, R7 regression safety) → the 4 acceptance checks preserved verbatim and expanded into named, testable tests (stub harnesses, assertions, wall-clock bounds) → caller-impact table → design-stage risks.
