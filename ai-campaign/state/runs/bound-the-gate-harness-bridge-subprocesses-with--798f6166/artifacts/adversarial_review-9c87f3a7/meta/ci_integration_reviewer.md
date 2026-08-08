The contract is complete and all claims were verified against the live tree. Summary of what was consolidated and the corrections surfaced:

## Deliverable

**`ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/acceptance-contract.md`** — single authoritative acceptance-stage contract (10 sections) merging the 4 direction checks (verbatim from pipeline.yaml), the testing_reviewer's 18 pins, and the concurrency_reviewer's amendments. It supersedes stale claims and states so explicitly.

## The four required pieces, grounded in live verification

1. **Zero-match guard (C1)** — live proof captured: today `go test ./internal/gate -run 'TestGate.*(Timeout|Deadline)' -v` prints `warning: no tests to run`, `PASS`, **exit 0**. Contract mandates counted `-v` evidence (`-count=1`, ≥2 matched names, `no tests to run` absent), a test-naming rule (`TestGate` prefix — `TestProbeAll_*` would silently escape the guard), and makes bare exit-0/green-suite claims inadmissible evidence.

2. **Stop-gate budgets (C2)** — corrected the reviewers' stale numbers against `arch-check.mjs`/`scan.mjs`: **cmd/forge is at 32/32 (headroom 0, not 31/32)** → zero new production files; all R5 flag work pinned in-place (cli_dispatch.go, main.go, engine_build.go, evolve.go, gates.go, runlock_wire.go). gate exports are **13 today** (10 gate.go + 3 resolve.go) → 19 with the 6 With-variants ≤ 30 (the "10→16" figure counts only gate.go). execbound ≤ 4 files / ≤ 10 exports, fan-in 3–5 ≪ 30 (grep-verified: only command_executor.go + sandbox_config.go reference the movable machinery — the reviewer's "4 files" was wrong), no-cycle guarantee structural.

3. **RED baseline (C3)** — verified live: `hub_output.rs:196 write_human 53 lines (max 50)`, uncommitted 26+/14- drift predating the campaign. Contract requires verbatim quoting with file:line, git-diff attribution, no "gate is green" claim (that's a hard-fail condition), and before/after arch evidence if the change touches forge-runtime/harness.

4. **Env hygiene + margin analysis (C4/C5)** — FORGE_GATE_TIMEOUT confirmed brand-new (zero grep hits); explicit t.Setenv control everywhere, PATH prepend-not-replace, no t.Parallel, explicit `--root`, strict env parsing with hard errors. Documented derivation: healthy return ≈ 0.3–0.4 s, worst-case = deadline + 2 s WaitDelay ≈ 2.3 s → ~2.2× margin at 5 s, ~1.5× worst-case under `-race`; load-bearing constraints: injected deadlines ≤ 2 s (canonical 300 ms, precedence env ≤ 1 s), stub sleeps ≥ 10 s and exits 0, triple assertion per wall-clock test.

Also included: pinned literal table (`"timed out"` + knob names, exact `retained %d of %d bytes (--max-output-bytes)` counts, `"gate: parsing acceptance --json:"` prefix, N/A-degrade stderr), the full T5–T11 inventory, SIGINT-closure contract, a runnable verification recipe with hard-fail conditions, and honest-N/A rules. Two citations I couldn't verify as claimed were corrected (exec-error literal → exec_error.go:78-82 rendering + KindTimeout classification test; fan-in count).

Pipeline config was left untouched — the contract sits in the run's artifacts for the acceptance stage to consume; no repo source files were modified.
