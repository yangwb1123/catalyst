The hardening review is complete. **Deliverable**: `ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/adversarial_review-9c87f3a7/meta/hardening-subprocess-lifecycle.md` (no repo source touched — only the run's state artifact, so no gate run needed).

## What I verified against the repo (not just the summary)

All 5 concern areas checked at real call sites: `Engine.RunGate` is `func(string) gate.Result` called with **no ctx** (orchestrator.go:95, gate_runtime.go:72-78); standalone `forge gate|check|accept` (`delegate`, main.go:60-74) has **no signal context**; `runProbe.runGate` (engine_build.go:374) and `loopProbe.refresh` (gates.go:306) lock only the probe — live `Gate`/`Check` spawns from `ResolveGate` run unlocked; wave size is unbounded (waves.go:21 pure fan-out); `WaitDelay` is set **only** in the unix build-tagged file (command_executor_unix.go:57) though it's portable; `spawnSync` (acceptance-kernel.mjs:41) confirms grandchildren write to node-owned pipes — so the pipe-hang is weaker than the orchestrator's case, but the **orphan leak** is the load-bearing reason for group kill.

## The 4 blocking findings

1. **SIGINT regression the design would introduce**: today's Ctrl-C works *because there's no Setpgid* (child shares the foreground group). Adding process groups without specifying who cancels the gate's ctx leaves every gate child orphaned on Ctrl-C — the design's failure-mode #7 was listed but never mechanized. Fix: ctx threading via closure capture at the two CLI seams (`resolveStageHostBoundary`/`buildLoop` gain a ctx param) — **zero change to `Engine.RunGate`**, and it also closes the wave-fail-fast stall for free.
2. **Fan-in memory**: 10 MiB × unbounded wave size. Fix: package-level spawn semaphore (cap 4) in `internal/gate`, ctx-aware; documented bound `≤ 4 × 2 × cap ≈ 80 MiB`.
3. **Negative-timeout footgun**: collides with `CommandExecutor`'s documented *zero = no deadline* — orchestrator rewiring would make negative load-bearing in production. Fix: Options v2 — negative **rejected**, explicit `Unbounded bool`, orchestrator zero maps to `Unbounded`.
4. **Non-unix**: `WaitDelay` is portable but unix-only today; moving it to common code gives Windows the pipe-close backstop it currently lacks, plus honest degradation logging only when a kill fires.

Plus a 12-row exit-path inventory (the kill-correctness contract), 7 acceptance-test additions (T5–T11), and a supersedes list marking exactly which parts of the original design are amended vs. confirmed.
