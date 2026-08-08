All checks complete. Here's the verification report.

---

# Verification: `internal/execbound` extraction vs ForgeOS constraints

**Verdict: feasible and constraint-compatible, with two gaps the design must close before build.** Everything was verified against live code (`forge-core` @ f98a1ec + working tree) and `harness/arch/arch-check.mjs`.

## 1. Import graph / cycle — ✔ no cycle

Verified in code:
- `internal/gate` imports **only** `converge` internally (resolve.go:16). No orchestrator edge.
- `internal/orchestrator` imports `gate` (7 production files) — the `orchestrator→gate` edge exists.
- The movable machinery (`commandContext`, `cappedBuffer`, `setupProcessGroup`, `maxOutputBytes`) is **generic** — zero references to `asset.Phase` or any orchestrator type (only context/os/exec/time/strings/fmt/syscall). A leaf `execbound` is cleanly extractable.
- Projected graph: `orchestrator → gate → execbound`, plus `orchestrator → execbound` directly (the summary's chain notation omits this second edge, but it's required for the rewiring and harmless). `execbound` imports nothing internal → **no cycle can be introduced**. `checkCircular`'s dir-graph DFS only gains edges *into* a leaf.
- Blast radius beyond the summary: **`sandbox_config.go:95,121` also uses `commandContext`/`cappedBuffer`** — the rewiring must cover it too (4 orchestrator production files, not just command_executor.go). Tests exist as the net on both sides (`command_executor_test.go` 22 exports, `command_executor_unix_test.go`).

## 2. Fan-in & layering per arch-check.mjs — ✔

| Check | Projection |
|---|---|
| `checkFanin` (max 30) | execbound importers: 4 orchestrator files + `gate/gate.go` = **5** ≪ 30 |
| `checkPackage` (32 files / 30 exports) | execbound ~4 files, ≤10 exports. `gate` today exports **10** (gate.go 7 + resolve.go 3); +6 With funcs → 16 ≤ 30 |
| `checkLayering` | Forbidden edges key on `dir_aliases` (domain/application/…); rules.yaml explicitly scopes **"the legacy Go core … is not yet fully classified"** — no `internal/*` dir matches an alias, so no layering edge applies. Layering for Go = cycle + fanin, both pass |
| `checkAntiPatterns` | `execbound` is not in the junk-name list (utils/common/…) |
| `checkCognitive` | Under `forge-core/internal/` → top-level module count unchanged (4) |
| `checkDrift` | Checks only `file.max_lines`/`root.max_files` equality between `.arch/rules.yaml` and `policies.yml` — adding a package touches neither. **Drift-guard safety of shared bounded-run is structural**: one copy consumed by both packages can't drift; the cycle check would also catch any future `execbound→orchestrator` edge |

⚠️ **Baseline is currently RED — unrelated**: `forge-arch [FAIL] function-length — forge-runtime/crates/interfaces/src/hub_output.rs:196 write_human 53 lines (max 50)`. Confirmed pre-existing working-tree drift (uncommitted edits, 26+/14-), not this campaign. Honest reporting: the Stop gate is not green before this change either.

## 3. Go core zero-dependency — ✔

`forge-core/go.mod` is `module forgeos/forge-core / go 1.26` with **no require block** (no go.sum). All extraction-source code is stdlib-only. Constraint holds — provided the trap is respected: execbound must not import `internal/asset` (the current machinery doesn't; only `CommandExecutor` itself does, and it stays in orchestrator).

## 4. With-API audit — ✔ zero churn, with one mandatory design detail

Full production caller inventory (verified):
- `Gate`/`Check`/`Accept` — cli_dispatch.go:26–28 (`delegate`); Gate/Check also internal at resolve.go:150/152 (ResolveGate's complexity/arch cases)
- `ProbeAll` — gates.go:310
- `ResolveGate` — engine_build.go:381 (`runProbe.runGate`, injected as `Engine.RunGate` at engine_build.go:110), runlock_wire.go:129, internal resolve.go:72 (GatesGreen loop)
- `GatesGreen` — gates.go:56 (`gatherSignals`)

Wrappers-with-default-Options keep all 8 call sites + `Engine.RunGate func(name) gate.Result` signature unchanged → **zero forced churn**. The "deadline inside the probe runner" claim checks out: `runGates → callGate → RunGate(name)` carries no ctx (gate_runtime.go:27–48, 74–80); options bake into the closure at injection sites (engine_build.go:110, evolve.go:280). All gate spawn sites are exactly `gate.go:69 (run)` and `gate.go:140 (ProbeAll)` — complete coverage.

## Gaps to close in the design (findings)

1. **Timeout zero-meaning conflict (must fix)**: `CommandExecutor.Timeout == 0` is documented **unbounded**, and production's `--timeout` defaults to 0 (main.go:141). But the design's execbound `Options.Timeout == 0` = 10m default. Naive rewiring silently gives `forge run --executor=command` agent phases a 10m deadline — violating "byte-identical". The design must specify: orchestrator maps its zero → execbound negative (the unbounded sentinel). `MaxOutputBytes` needs no mapping (0→10 MiB on both sides).
2. **The design artifact is 18 lines** — no API signatures, no Options fields, no execbound package layout, no timeout mapping, no test list. The substantive design lives only in the campaign prompt. For a stage-gated pipeline where Review reads artifacts, that's a deliverable-quality gap; this verification had to be reconstructed from code.
3. **`cmd/forge` is at 31/32 files (headroom 1)** — the R5 flag work must stay in-place (cli_dispatch.go / engine_build.go / evolve.go), no new cmd/forge files, or `checkPackage` trips.
4. Honest-behavior note, consistent with the design: the 10m default bounds `forge run`/`evolve` gate steps too (via gates.go:56 and engine_build.go:381 paths), not just the CLI subcommands — that asymmetry vs unbounded agent phases is intentional but should be stated.

**Claims verified as accurate**: the 4 ResolveGate live-spawn sites (engine_build.go:381, runlock_wire.go:129, resolve.go:72, gates.go:56 — the latter two via GatesGreen); the grandchild-pipe hazard (gate's `run()` is a bare `exec.Command`+`CombinedOutput` with no ctx/cap; acceptance.mjs re-spawns gates ~4× nested per test_acceptance.mjs:91); the orchestrator's solved pattern (process-group kill + Cancel override + WaitDelay, 10 MiB default). No constraint violations found beyond items 1–4 above.
