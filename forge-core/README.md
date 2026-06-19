# forge-core

The ForgeOS **v2 orchestration runtime** — a small, real state machine that
turns a declarative workflow into something that *runs itself*, stepping
through phases in order and **enforcing the real harness gates** at gate
phases.

It is written in **pure Go standard library: zero external modules, no network
fetch**. `go.mod` has no `require` block.

## What it is

```
cmd/forge/             CLI entry point
internal/asset/        load a workflow from JSON (fault tolerant)
internal/gate/         shell out to the real harness gates (gate.mjs / check.py / acceptance.mjs)
internal/routing/      (agent, mode) -> model tier, with a hard Opus safety floor
internal/prompt/       assemble an agent-phase prompt: role card + project context (ADRs, constraints)
internal/converge/     evaluate the stop condition against live signals (ROADMAP completion, gate state)
internal/orchestrator/ the engine: walk phases, enforce gates, delegate agent phases
  orchestrator.go        one-pass engine: walk phases, run gates, delegate agent phases
  loop.go                LoopEngine: re-run to convergence, with max-iter + no-progress tripwire
  command_executor.go    CommandExecutor: run a real per-phase command (e.g. `claude -p <prompt>`)
```

The engine's contract is deliberately small and honest:

- **Gate phases** (a phase whose `required_gates` is non-empty) run every named
  gate via the real out-of-band harness tools. **The first not-OK gate aborts
  the run with an error** — a red gate blocks the increment. This is the
  enforcement core.
- **Agent phases** are delegated to an `AgentExecutor` — `DryRunExecutor` by
  default (narrate routing, no LLM) or `CommandExecutor` to drive a real agent
  CLI (`--executor command --agent-cmd claude`).
- After all phases pass, the engine **evaluates** the workflow's **stop
  condition live** against real signals (ROADMAP completion, gate state) and
  reports a per-criterion MET/NOT-MET verdict — convergence is **computed, not
  merely declared**. `forge run` prints this verdict once; `forge evolve` re-runs
  the workflow and prints the **live verdict every iteration** until the
  condition converges (ForgeOS forbids round-count termination — `--max-iter` is
  only a safety bound, with a no-progress tripwire against doom loops).
- Stop criteria are **typed**: `all_of` items are objects
  (`{metric, operator, threshold|value}`, as authored in `build.yml`) dispatched
  by metric — `roadmap_completion` compares the live percentage against the
  threshold with the criterion's operator (`== >= <= > <`); `gates_status`
  requires `green` and a green gate state. An **unknown metric is NOT met**
  (honest — convergence is never faked). A workflow whose `stop_condition.type`
  is `external` (e.g. `evolve`) has no conjunction: it runs to the `--max-iter`
  safety bound, which is the **expected clean stop** (exit 0), never a failure.

## Build / test / run

```sh
# build everything
go -C forge-core build ./...

# run the unit tests (self-contained; depend on no sibling agent's output)
go -C forge-core test ./...

# format check (should print nothing)
gofmt -l forge-core
```

Run a workflow or a single gate (repo root defaults to `$FORGE_REPO_ROOT`, else
the current directory; override with `--root`):

```sh
# drive a workflow end to end (default dry-run agents + real gates)
go -C forge-core run ./cmd/forge -- run build --mode balanced --root "$PWD"

# drive agent phases with a real agent CLI (role-card + context prompt -> `claude -p`)
go -C forge-core run ./cmd/forge -- run build --executor command --agent-cmd claude --root "$PWD"
# inspect the plumbing without firing an agent
go -C forge-core run ./cmd/forge -- run build --executor command --agent-cmd echo   --root "$PWD"

# autonomous closed loop: re-run to convergence (max-iter is a safety bound, not the goal)
go -C forge-core run ./cmd/forge -- evolve build --max-iter 5 --executor command --agent-cmd claude --root "$PWD"

# delegate straight to one harness gate; exit code follows the gate
go -C forge-core run ./cmd/forge -- gate   --root "$PWD"   # node harness/gate.mjs
go -C forge-core run ./cmd/forge -- check  --root "$PWD"   # python3 harness/check.py
go -C forge-core run ./cmd/forge -- accept --root "$PWD"   # node harness/acceptance.mjs
```

## Honest limitations (current scope)

These are real, intentional gaps — flagged here rather than hidden:

- **Agent phases dry-run by DEFAULT; a real executor ships.** The default
  `DryRunExecutor` only logs the routing decision (`phase <name> -> agent
  <agent> (tier <tier>)`) and invokes no LLM. But a real `CommandExecutor` is
  shipped: `--executor command --agent-cmd claude` builds a per-phase prompt
  from the agent's role card plus project context (ADRs + the hard engineering
  constraints) and runs `claude -p <prompt>` — actually driving the agent, not
  narrating. `--agent-cmd echo` exercises the same plumbing without firing an
  agent. The real remaining limitation is therefore operational, not
  architectural: the agent CLI must be installed and have credentials/budget in
  the environment. The `AgentExecutor` interface keeps both executors local and
  swappable.
- **YAML is transcoded by a python shim.** Go's standard library has no YAML
  parser and forge-core takes zero external deps, so `forge run` shells out to
  `python3 harness/yaml2json.py` to turn `.agent/workflows/<name>.yml` into the
  JSON the runtime consumes. This is **temporary scaffolding**; it can later be
  replaced by a Go YAML library (a dependency decision for architect/cto, not
  taken here). If the shim is absent, `forge run` fails with a clear message.
  The unit tests never touch it — they parse committed JSON fixtures.
- **The stop condition is evaluated live against real signals.**
  `internal/converge` dispatches the **typed** `all_of` criteria
  (`{metric, operator, threshold|value}`) by metric: `roadmap_completion`
  (fraction of decided ROADMAP checklist items; `[~]` counts as not done)
  compared against the threshold with the criterion's operator, and
  `gates_status == green` against live gate state. `forge run` reports the
  per-criterion MET/NOT-MET verdict once; `forge evolve` reports it every
  iteration. An unknown metric is treated as unmet (convergence is never faked).
  External-stop workflows (`type: external`) run to the `--max-iter` safety
  bound as their expected clean stop. The remaining gap is breadth: only
  `roadmap_completion` and `gates_status` are recognized today; richer metrics
  (eval scorecards, coverage trends) are not yet wired.

## Routing safety floor

`routing.TierFor` distills the ForgeOS routing/modes policy. One rule is a hard
floor, independent of mode: **`architect`, `cto`, and `reviewer` always route to
Opus** (risk beats cost; a fresh-context reviewer is never down-tiered).
