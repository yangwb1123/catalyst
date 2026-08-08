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
internal/artifact/     append-only artifact provenance manifest + integrity verification
internal/runlock/      process lock + run identity for shared .forge state
internal/tasklist/     strict planner TASK_LIST machine contract
internal/trace/        versioned/redacted JSONL observability
internal/orchestrator/ the engine: walk phases, enforce gates, delegate agent phases
  orchestrator.go        one-pass engine: walk phases, run gates, delegate agent phases
  loop.go                LoopEngine: re-run to convergence, with max-iter + no-progress tripwire
  command_executor.go    CommandExecutor: run a real per-phase command (Claude prompt via stdin)
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
go -C forge-core run ./cmd/forge run build --mode balanced --root "$PWD"

# drive agent phases with a real agent CLI (role-card + context prompt -> Claude stdin)
go -C forge-core run ./cmd/forge run build --executor command --agent-cmd claude --root "$PWD"
# inspect the plumbing without firing an agent
go -C forge-core run ./cmd/forge run build --executor command --agent-cmd echo   --root "$PWD"

# autonomous Evolve loop (proposal-only modes require the same pinned bytes as release)
go -C forge-core run ./cmd/forge evolve evolve --mode explorer --max-iter 5 \
  --executor command --agent-cmd claude \
  --release-agent-path /absolute/operator-trusted/path/claude \
  --release-agent-sha256 <64-lowercase-hex> --root "$PWD"

# declarative deploy package (Linux only; path and content are operator-pinned)
go -C forge-core run ./cmd/forge run deploy --executor command --agent-cmd claude \
  --release-agent-path /absolute/operator-trusted/path/claude \
  --release-agent-sha256 <64-lowercase-hex> --root "$PWD"

# delegate straight to one harness gate; exit code follows the gate
go -C forge-core run ./cmd/forge gate   --root "$PWD"   # node harness/gate.mjs
go -C forge-core run ./cmd/forge check  --root "$PWD"   # python3 harness/check.py
go -C forge-core run ./cmd/forge accept --root "$PWD"   # node harness/acceptance.mjs
```

## Group Agent Graph control artifacts

The Go binary is also the sole scheduler for persisted Group Agent Graphs. It
produces canonical, effect-free interchange artifacts for planning, immutable
multi-node scheduling policy, schedule-bound initial-node contracting,
operator pricing, legacy first-node contracting, and passive release
authorization:

The commands below are a protocol map. Replace uppercase tokens with the
preceding artifact fields: read `SCHEDULE_SHA256` from `schedule.json` and
`PRICING_SNAPSHOT_SHA256` from `pricing.json` before building either contract.

```sh
forge graph-plan --graph-id GROUP_AGENT_GRAPH_ID \
  --manifest-sha256 GRAPH_MANIFEST_SHA256 --input graph.json > core-plan.json

forge graph-execution-schedule --control control.json > schedule.json

forge graph-node-pricing-snapshot --model PINNED_MODEL \
  --input-usd-micros-per-token-unit 2000000 \
  --output-usd-micros-per-token-unit 10000000 \
  --max-input-tokens 400000 > pricing.json

forge graph-scheduled-node-contract --control control.json \
  --schedule-sha256 "$SCHEDULE_SHA256" \
  --endpoint https://api.openai.com/v1/responses \
  --model PINNED_MODEL --max-output-tokens 4096 \
  --max-model-output-bytes 65536 --max-model-events 1024 \
  --timeout-ms 120000 --max-cost-usd-micros 1000000 \
  --pricing-snapshot-sha256 "$PRICING_SNAPSHOT_SHA256" \
  --max-result-bytes 262144 > scheduled-node-contract.json

# Pin pricing.json's pricing_snapshot_sha256 in the contract.
forge graph-node-contract --control control.json \
  --endpoint https://api.openai.com/v1/responses \
  --model PINNED_MODEL --max-output-tokens 4096 \
  --max-model-output-bytes 65536 --max-model-events 1024 \
  --timeout-ms 120000 --max-cost-usd-micros 1000000 \
  --pricing-snapshot-sha256 "$PRICING_SNAPSHOT_SHA256" \
  --max-result-bytes 262144 > node-contract.json

forge graph-node-dispatch-authorize \
  --control release-control.json > authorization.json

forge graph-node-terminal-receipt \
  --control terminal-control.json > terminal-receipt.json
```

`graph-node-contract` strictly validates Rust's canonical private control
snapshot and always selects `plan.waves[0][0]`; callers cannot name a node.
It freezes exact Prompts, provider configuration, budgets, zero capabilities,
and failure policy but reads no credential and performs no provider, model,
network, tool, workspace, result, memory, or writeback effect. Output is compact
canonical UTF-8 JSON with no trailing newline.

`graph-execution-schedule` accepts that same exact private v1 control only for
multi-node Graphs. It freezes one serial wave-then-authored order, Project lane
digests, authored-order direct-predecessor receipt slots, initial frontier, and
fail-fast/no-dataflow policy. The content-addressed output contains no private
manager/task/project/profile/provider/result text, observes no progress, grants
no dispatch authority, and does not advance a successor. Single-node Graphs
remain on the separate terminal lifecycle.

`graph-scheduled-node-contract` independently rebuilds that schedule from the
same exact control and accepts only its lowercase digest—not a caller-supplied
schedule, ordinal, or attempt. With neither receipts nor `--target-node`, it
selects ordinal zero and emits the content-free initial candidate. A target or
one or more full scheduled terminal receipts enters the successor path: Core
strictly accepts only exact `completed`/result receipts, canonicalizes the
candidate to the schedule's complete direct-predecessor order, and requires the
selected node to be topology-ready. An explicit zero-receipt target is valid
only for an ordinal>0 node whose direct-predecessor set is empty; it never falls
back to the initial candidate. Optional `--predecessor-content` is ≤1 MiB exact
UTF-8, requires an authenticating direct receipt, and binds the canonical first
direct predecessor. `graph-scheduled-ready-nodes` exposes the same selection
rule. Every canonical v2 candidate remains passive: it creates no lifecycle or
provider request, observes no progress, grants no authority, and cannot by
itself advance a successor.

`graph-node-pricing-snapshot` fixes the production destination to the official
OpenAI Responses endpoint and emits an immutable local pricing assertion. Its
rates and input-token ceiling are operator supplied, provenance is
`operator_asserted`, and vendor attestation is explicitly absent. The artifact
and its integer cost calculation are not a current vendor price sheet or bill
guarantee. The command reads no credential or file, constructs no provider,
and performs no network request.

`graph-node-dispatch-authorize` independently validates Rust's exact current
release-control snapshot and emits a passive content-addressed authorization.
It does not persist or release dispatch authority; Rust must still revalidate
it against current durable state and the exact pricing artifact.

`graph-node-terminal-receipt` strictly rebuilds the claimed single-node run,
authorization, pricing, lane, claim event, and bounded terminal artifact. It
emits Core's content-addressed terminal decision without reading credentials,
calling a provider, touching a database, or releasing the Project lane. Use
`--protocol-version` for the exact no-newline bridge preflight value `1`.

Provider endpoints use a conservative, byte-stable HTTPS grammar shared with
Rust: lowercase canonical DNS or dotted-decimal IPv4, an optional canonical
non-default port, and an empty or `/`-rooted unreserved path. Userinfo,
query/fragment, percent escapes, dot segments, IPv6, and spellings that would
normalize a host or port are rejected.

## Honest limitations (current scope)

These are real, intentional gaps — flagged here rather than hidden:

- **Agent phases dry-run by DEFAULT; a real executor ships.** The default
  `DryRunExecutor` only logs the routing decision (`phase <name> -> agent
  <agent> (tier <tier>)`) and invokes no LLM. But a real `CommandExecutor` is
  shipped: `--executor command --agent-cmd claude` builds a per-phase prompt
  from the agent's role card plus project context (ADRs + the hard engineering
  constraints) and invokes Claude with `-p` while sending the prompt through
  stdin, so repository context is absent from process argv and argv logs.
  `--agent-cmd echo` exercises the same plumbing without firing an agent.
  Child processes receive a minimal environment; Claude credential names are
  allowed explicitly and extra variables require `--agent-env NAME`.
- **YAML uses a native zero-dependency parser first.** `internal/yaml2json`
  parses the shipped workflow subset in Go. `harness/yaml2json.py` remains a
  compatibility fallback for unsupported input when Python is available; a
  missing shim does not disable the native path.
- **The stop condition is evaluated live against real signals.**
  `internal/converge` dispatches the **typed** `all_of` criteria
  (`{metric, operator, threshold|value}`) by metric: `roadmap_completion`
  (fraction of decided ROADMAP checklist items; `[~]` counts as not done)
  compared against the threshold with the criterion's operator, and
  `gates_status == green` against live gate state. `forge run` reports the
  per-criterion MET/NOT-MET verdict once; `forge evolve` reports it every
  iteration. An unknown metric is treated as unmet (convergence is never faked).
  External-stop workflows (`type: external`) run to the `--max-iter` safety
  bound as their expected clean stop. Recognized signals include roadmap and
  gate state, requirement confidence, review status, and named acceptance
  criteria; unknown metrics remain fail-closed.
- **No remote deploy is claimed; sandbox execution is explicit and local.**
  Deploy/rollback workflows
  generate and validate `docs/release/**`; external CI/operators perform the
  real action after human approval. Release phases reject `--agent-env`, custom
  `--agent-allowed-tools`, non-`claude` executables, writable phase declarations,
  and shell/network tools before command construction. Command-mode release is
  Linux-only and requires an absolute repository-external entry path named
  `claude` plus its operator-pinned SHA-256. The entry may resolve to a
  repository-external canonical executable with a different basename; an
  internal helper copies those frozen bytes into an anonymous executable
  `memfd`, applies and verifies every mutation-preventing seal, then rechecks
  the final digest and ELF magic before executing a read-only descriptor for
  that same sealed inode. This pins bytes, not vendor identity or a package
  signature; the digest needs an independent operator trust channel.
  The child uses a compiled minimal prompt—not the normal role-card/ROADMAP/ADR/
  memory context—and receives only exact `Edit(/<phase.emit>)` permissions under
  `dontAsk`. A whole-tree postflight rejects undeclared release-file changes.
  Validation requires a single successful Claude JSON envelope, an exact
  matching report verdict, and an unchanged prompt-reviewed product state.
  Its receipt and the later human marker bind agent/prompt, source-state and
  the current stage's fixed release-artifact-set digest (Deploy: its five files;
  Rollback: the release manifest plus four rollback files), so a later source
  or bound-stage artifact change invalidates approval. Freshness is contextual
  equality, not a wall-clock TTL. Ordinary command execution can opt into
  `--sandbox docker` or `--sandbox firecracker`; both runners are wired through
  the same fail-closed interface and enforce bounded input/output, timeout and
  memory configuration. `--sandbox-memory-mb` defaults to 512 MiB and accepts
  64–32768 MiB; retained output inherits `--max-output-bytes` (default 10 MiB)
  and overflow is explicit. Docker readiness shares the run deadline and a
  named container gets an independent bounded cleanup attempt; Firecracker's
  deadline includes prerequisite/rootfs work, serial capture is in-memory and
  bounded, and rootfs template copying rejects special files and unsafe
  injection links. This is currently an isolated argv/stdin command
  runner, not a complete coding-workspace exchange protocol: no repository
  snapshot/mount, scoped secret channel, or declared artifact sync-back is
  claimed by the runner interface.

See [ADR 0005](../docs/adr/0005-declarative-production-delivery-boundary.md),
the [release artifact contract](../docs/release/README.md), and the
[ignition guide](../docs/ignition.md).

## Routing safety floor

`routing.TierFor` distills the ForgeOS routing/modes policy. One rule is a hard
floor, independent of mode: **`architect`, `cto`, and `reviewer` always route to
Opus** (risk beats cost; a fresh-context reviewer is never down-tiered).
