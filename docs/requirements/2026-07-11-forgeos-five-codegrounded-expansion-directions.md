# ForgeOS: Global Code Scan — Five Genuinely Uncovered Expansion Directions

> **Role**: Senior Architect / Product Manager  
> **Method**: Exhaustive full-repo scan (forge-core 18 Go packages / 63 non-test source files / ~32k LOC production code; harness 39+ modules / ~10.5k LOC; `.agent/` full declaration skeleton; `pi-batch.py`; `examples/`; `ai-dev/` parallel universe; all sprint records 1–31; FUNCTIONAL_REQUIREMENTS_AUDIT.md; cross-referenced against 90+ existing docs/requirements analysis files)  
> **Constraint**: No code written. Every direction backed by code-level evidence (`file:line`).  
> **Goal**: Identify directions that are **genuinely not covered** in the existing ~90 analysis documents, have real code-grounded evidence, carry high product/architectural value, and are bounded enough to be actionable within 1–2 sprints.

---

## Relationship to Existing Analysis

This document was written after reading every existing file in `docs/requirements/` (~90 files) and `docs/` (~40 additional files). The following domains are **saturated** by prior analysis and are explicitly **not** revisited here:

| Saturated Domain | Representative Docs |
|---|---|
| Workflow asset silent degradation / tolerant loading | `2026-07-11-codegrounded-edge-cases-and-extensions.md` direction-1 |
| Forward-only checkpoint / No true rollback | `2026-07-11-codegrounded-edge-cases-and-extensions.md` direction-2 |
| Memory density decay / append-only semantics | `2026-07-11-codegrounded-edge-cases-and-extensions.md` direction-3, `high-value-perspectives-v11.md` |
| Convergence theory / partial-convergence visibility | `2026-07-11-codegrounded-edge-cases-and-extensions.md` direction-4 |
| Parallel engine deadlock / lock-order / fail-fast | `edgecases-and-perf.md`, multiple structural docs |
| Loop-back / mode-gating / resume correctness | All orchestration docs (~35 files) |
| Trace/scorecard/telemetry learning loop | ~16 files |
| Security: secret-scan / SCA / readonly / prompt injection | ~14 files |
| Arch-check: layering / fan-in / function length | ~12 files |
| CLI: detect / preflight / doctor / status / migrate | ~8 files |
| 529 backoff / recursion guard / budget guard / output cap | ~18 files |
| Multi-repo / Web UI / Sandbox / federated architecture | ~8 files |
| Configuration surface / mode_gating drift guard | Sprint 30–31 deliverables |
| Cross-process checkpoint safety / memory file-lock | `expansion-deep-analysis.out.md`, `expansion-direction-analysis.md` (high-level notes, not systematic) |

---

## Direction 1: Memory Compaction's Semantic Blind Spot — Content-Agnostic Retention Destroys Signal

### Current State

`internal/memory/memory_compact.go` implements the compaction/retention policy for the cross-session knowledge store. It is purely **mechanical**: entries are grouped by `Kind`, then within each kind the oldest entries beyond `keepPerKind` are **replaced by a summary entry** (`summarizeBlock`), and the rest are discarded. The trigger is a simple **count threshold** (`DefaultCompactThreshold = 500`), and the retention boundary is strictly **age-based** (`CompactAgeSeconds = 86400`, 24 hours).

```go
// memory_compact.go:32-34
const DefaultCompactThreshold = 500      // entry count > this triggers compaction
const DefaultCompactKeepPerKind = 20      // per-kind verbatim retention ceiling
const CompactAgeSeconds = 86400           // 24-hour age boundary
```

### The Blind Spot

The compaction makes **zero distinction between entry content**. A critical security finding ("credentials leaked in index.html"), an irreversible architecture decision ("migrate from Postgres to DynamoDB"), or a budget-busting observation ("API cost $1200 overnight") all compete on equal footing with "status check OK" or "metrics nominal" entries. The only criterion is: **is this entry older than 24h AND beyond the per-kind limit?** If yes, it gets compressed into a terse summary string regardless of substance.

```go
// memory_compact.go:108-121 — compactByKind: mechanical count-based, content-agnostic
for _, kind := range kinds {
    kindEntries := byKind[kind]
    if len(kindEntries) <= keepPerKind {
        compactedEntries = append(compactedEntries, kindEntries...)  // preserve all
        continue
    }
    keep := kindEntries[len(kindEntries)-keepPerKind:]         // keep recent N
    summarized := kindEntries[:len(kindEntries)-keepPerKind]   // compact the rest
    // ...
}
```

Additionally, `summarizeBlock` has a subtle structural flaw:

```go
// memory_compact.go:149-166
topics := make(map[string]int)
total := 0
for _, e := range entries {
    topics[e.Topic]++
    total++
}
```

It uses the SAME map key namespace for both single-topic enumeration AND total count. If the Kind string (e.g., `"observation"`) happens to match a Topic string (e.g., `"observation"`), the topic count contaminates the total — it's already `topics["observation"]` holding the entry count, and there's no separate `total` tracking that's map-independent. (The `total` variable is an integer counter, so this is a readability/debugging concern, not a correctness bug — but it signals the ad-hoc nature of the summarization logic.)

### Why This Matters

In a 24h evolve loop, the system discovers findings at varying rates. The first 6 hours might produce 400 "scan-complete" entries (one per evolve iteration) and 3 critical security findings. After compaction:
- The 3 critical findings → summarized into one line: `"compacted 3 observation entries; topics: credential_leak:1, dependency_CVE:1, misconfig:1"`
- The 400 routine entries → 20 verbatim + a similar summary
- The next iteration's `Retrieve` scores the summary against the query. A keyword match on "credential" pulls the summary, which says "credential_leak was found" but the actual **fix details, reproduction steps, and remediation verification** are lost

The agent then re-discovers the credential leak because the detail was compacted away — the loop becomes amnesiac about the one thing it must NOT forget.

### Suggested Direction

Introduce **importance-weighted retention**: allow entries to carry an optional `Priority` field (0=normal, 1=important, 2=critical). The compact policy then becomes:
- `keepPerKind` for priority-0 entries (current behavior)
- `keepPerKind * 2` for priority-1 (retain more)
- Priority-2 entries are **never compacted** (always preserved verbatim)

The priority assignment is **not** a new agent output — it's derived from existing signals:
- An entry whose `text` matches a `security_findings` probe result → priority-2
- An entry that references a cost/budget exceedance → priority-1
- An entry explicitly flagged by an agent (`IMPORTANCE: high` in output) → priority-1

**Value**: Prevents the loop from forgetting its most important lessons. Makes the memory store resilient to high-volume noise.

**Scope**: ~1 sprint. Changes confined to `internal/memory/memory_compact.go` + `internal/memory/memory.go` (add Priority field) + scoring logic in the CLI layer or evolve loop.

---

## Direction 2: Runtime State Format — Zero Versioning × Zero Cross-Process Safety = Silent Data Corruption

### Current State

ForgeOS maintains three runtime state files under `.forge/`:

| File | Format Marker | Version Field | Update Pattern |
|---|---|---|---|
| `checkpoint.json` | `_format: forgeos.checkpoint.v1` | **None** — the marker is just a field name, not a version | Atomic rewrite (write temp + rename) |
| `trace.jsonl` | `_format: forgeos.trace.v1` (per event) | None — the format string is the only version signal | Append (O_APPEND write) |
| `memory.jsonl` | `Format: forgeos.memory.v1` (per entry) | None — same | Append (O_APPEND write) + atomic rewrite for compaction |

### The Versioning Gap

There is **no migration path**. A hypothetical `forgeos.trace.v2` event layout would be:
1. Written by a newer forge binary
2. Read by an older `forge scorecard rebuild` command that parses `kind`/`name`/`status`/`duration_ms` — the new fields it doesn't know about are silently ignored by `encoding/json`
3. The old code produces **wrong output** (not failed output) because missing new-format fields are read as zero-values but the semantics have changed

The `_format`/`Format` markers exist but are **never checked** by any consumer:

```go
// trace.go:36-40
// Format carries the on-disk format version identifier (e.g. "forgeos.trace.v1"),
// so downstream tooling can detect format changes. An empty value (pre-format
// versioning) is treated as "forgeos.trace.v1" for backward compatibility.
```

But `Emit` auto-assigns the format:

```go
// trace.go:106-110
if ev.Format == "" {
    ev.Format = "forgeos.trace.v1"  // always v1, always overwritten
}
```

And `forge scorecard rebuild` (the main consumer) never checks the format:

```bash
grep -n "Format\|format" /home/u1/catalyst/forge-core/internal/attribution/rebuild.go
# -> Zero matches. The rebuild path never reads the format field.
```

### The Cross-Process Race Gap

Worse: **no file locking exists**. Two concurrent `forge run` or `forge evolve` processes in the same repo would:

1. **trace.jsonl**: Both append to the same file via `os.O_APPEND` on Unix. The kernel makes each `write()` atomic, but the two processes interleave — the trace stream becomes an unpredictable merge of two independent runs. Neither process knows the other exists.

2. **memory.jsonl**: Same pattern — append-only, no lock. If process A compacts (atomic rename), process B's file handle points to the renamed-away file, and B's next append silently goes to a detached inode.

3. **checkpoint.json**: Process A atomically rewrites. Process B atomically rewrites. The last writer wins — the other's checkpoint is silently discarded.

```go
// persist/checkpoint.go — SaveCheckpoint writes temp + rename
// No lock acquisition before the write
```

```go
// persist/checkpoint.go — ResumeCheckpoint reads and returns
// No version validation, no lock acquisition
```

### Why This Matters

This is **silent data corruption**. In a multi-user CI environment or a developer running `forge run` in one terminal while an automated `forge evolve` loop runs in another:
- Traces from the two runs are interleaved — scorecard telemetry becomes garbage
- Memory from one run is partially consumed by the other — decisions from different contexts mix
- Checkpoint becomes a race — resumes may jump to unexpected states

ForgeOS' own CI (`.github/workflows/forge.yml`) runs `forge run build --executor dry` — a single-process context, so this hasn't surfaced. But the architecture explicitly targets 24h autonomous operation (the evolve loop), and in that setting multiple orchestrator instances are a production concern.

### Suggested Direction

Two orthogonal but complementary work streams:

**A) File-lock protocol**. Use a portable advisory lock for `.forge/` operations:
- On Unix: `flock()` or `fcntl()` on a `.forge/.lock` file
- On Windows: `LockFileEx()` 
- Lock acquisition before any write to checkpoint/trace/memory; release after
- Non-blocking acquire with a fail-closed error: "another forge process holds the lock on this repo" rather than silent corruption

**B) Schema versioning contract**. Formal version negotiation:
- `checkpoint.json` needs an explicit `version` field (integer, start at 1)
- Any consumer that reads a `version > compiled-in-max` rejects the file with an actionable message ("this checkpoint was written by a newer forge binary (format v2); this binary supports up to v1. Upgrade forge.")
- `trace.jsonl`/`memory.jsonl` format markers should be validated by their readers (currently zero validation exists)
- Add a `forge migrate --state` command that can upgrade state files across format versions (following the existing `forge migrate --to engineering` pattern)

**Value**: Prevents a class of silent data corruption bugs that become near-impossible to debug post-hoc. Essential before ForgeOS can safely support multi-instance or CI-parallel deployments.

**Scope**: ~2 sprints. The file-lock protocol is ~1 sprint (Go `internal/flock` package, wiring into `persist`/`memory`/`trace` writers + readers). The schema versioning is another sprint (add fields, validate on read, `migrate` subcommand).

---

## Direction 3: Forked Orchestration — `pi-batch.py` Exists Outside All ForgeOS Governance

### Current State

`/home/u1/catalyst/pi-batch.py` is a **standalone batch executor** for the `pi` agent CLI. It is 480 lines of Python, sitting in the repo root — directly violating the "root directory ≤ 15 files" governance that ForgeOS enforces on every project it governs.

```python
# pi-batch.py:1-2
#!/usr/bin/env python3
"""pi-batch -- serial/parallel batch executor for pi agent."""
```

### The Fork

The script implements a complete parallel orchestration system:
- YAML/JSON task file loading (`load_tasks`)
- Serial and parallel execution modes (`run_serial`, `run_parallel`)
- Thread-pool-based concurrent execution (`ThreadPoolExecutor`)
- Timeout-per-task with subprocess management (`_run_task_process`)
- Output file management and summary reporting (`save_result`, `print_summary`)

This is **structurally identical** to what forge-core's `orchestrator` package does — but for the `pi` agent instead of `forge`. Both take declarative task definitions, execute agent phases, manage timeouts and output, and produce artifacts. But:

| Dimension | forge-core | pi-batch.py |
|---|---|---|
| Agent | forge (Go binary) | pi (Python CLI) |
| Tests | ~200+ Go tests | **Zero** |
| CI | `.github/workflows/forge.yml` | **None** |
| Governance | `forge accept` (6 real gates + 5 N/A) | **None** |
| Location | `forge-core/`, `harness/` | **Repo root** |
| Runtime | Go stdlib, zero deps | Python, optional PyYAML |
| File count in root | 8 (with pi-batch.py → 9 of 15 budget) | 1 file using 1/15 of the budget |

### Specific Code-Level Issues

**1. Timeout double-count bug** (noted in Sprint 27 honesty):

```python
# pi-batch.py:236-245
def _run_task_process(...):
    tout = Thread(target=_read_stream, args=(proc.stdout, prefix, stdout_lines), daemon=True)
    terr = Thread(target=_read_stream, args=(proc.stderr, prefix, stderr_lines), daemon=True)
    tout.start()
    terr.start()
    remaining = lambda: max(0.0, timeout - (time.monotonic() - start))
    try:
        tout.join(timeout=remaining())   # both threads get the FULL timeout budget
        terr.join(timeout=remaining())   # effectively 2× configured timeout
        proc.wait(timeout=remaining())
    except subprocess.TimeoutExpired:
        proc.kill(); raise
```

Both reader threads get the **full remaining timeout budget independently**. If the task outputs nothing on stderr but fills stdout for the full timeout, `terr.join(timeout=remaining())` returns immediately (daemon thread is done), but `tout.join(timeout=remaining())` could take nearly the full timeout on its own. Then `proc.wait(timeout=remaining())` subtracts the already-elapsed time — but the effective total wait is `timeout × 1.5` to `timeout × 2`, depending on timing.

**2. Undiagnosed `FileNotFoundError`**:

```python
# pi-batch.py:281-283
except FileNotFoundError:
    msg = "'pi' not found in PATH." if os.path.isdir(workdir) else f"working directory not found: {workdir}"
```

This conflates the `cwd` not existing with the binary not existing — `FileNotFoundError` from `subprocess.Popen` can also be raised when the **binary itself** doesn't resolve, even if the cwd exists. The `os.path.isdir(workdir)` guard only catches one cause.

**3. No forge-init integration**:

`pi-batch.py` is NOT in `harness/scaffold/forge-init.mjs`'s `COPIED_FILES` list. A scaffolded project through `forge init` does NOT inherit this batch-execution capability. It's a bespoke tool for this repo only, with no copy-anyway contract.

### Why This Matters

This is **architectural drift in the codebase**. The project has two orchestration systems for two different agent CLIs, with completely different governance standards. As ForgeOS's stated goal is to be "the OS for AI-native software engineering" that sits above **any** agent CLI (Claude Code, Codex, Gemini CLI, pi, OpenHands), having pi-batch.py as a one-off Python script outside the governance layer means:

1. Every new agent CLI needs its own batch executor — the pattern repeats
2. No shared timeout/retry/resource-guard mechanism between forge and pi orchestration
3. The pi path has zero observability (no trace.jsonl, no memory.jsonl, no scorecard)
4. A root-level file sets a bad precedent for governed projects

### Suggested Direction

Either **integrate** pi-batch.py into the ForgeOS governance model or **deprecate** it in favor of a generic agent-agnostic batch executor inside `forge-core`:

- **Option A (Integrate)**: Move pi-batch.py into `harness/agents/pi-batch.mjs` (Node, to match other harness tools), add `COPIED_FILES` to forge-init, add tests, wire it into the existing timeout/resource-guard patterns from forge-core, and add a `forge run --batch-tasks tasks.yaml` subcommand that reuses the orchestrator's RunParallel (waves) for batch execution

- **Option B (Generic executor)**: Build a thin generic `forge run --batch` that reads a tasks.yaml and dispatches each task through the existing orchestrator.engine agent executor (which already supports the `--agent-cmd` flag for different CLIs). This is architecturally clean — pi-batch.py's functionality becomes a flag on the existing orchestration engine rather than a fork

**Value**: Eliminates architectural drift. One orchestration path for all agent CLIs. The pi path gets governance, observability, and fault tolerance that the standalone script lacks.

**Scope**: ~2 sprints (Option B). New `internal/batch` package for YAML batch-file loading, `cmd/forge/batch.go` CLI dispatch, support in `RunParallel` for batch mode.

---

## Direction 4: Context Retrieval Has No Feedback Loop — Blind Injection

### Current State

`internal/prompt/retrieve.go` implements a pure keyword-based retriever (TF-lite with IDF-aware scoring):

```go
// retrieve.go:1-9
// Package prompt is forge-core's Context/Memory retriever...
// v1: a pure keyword / term-frequency retriever, Go standard library only...
// It is NOT semantic. ... True semantic / embedding retrieval is v3 work.
```

The retriever is called by `Gather` in `prompt.go`:

```go
// prompt.go:77-102
// Relevant ADRs are RETRIEVED: candidate decisions are scored against query
// (derived from the phase/agent) and only the top-K most relevant are injected.
```

### The Blind Injection Problem

The retriever scores documents against the query, returns top-K, and the prompt builder injects them verbatim. **But there is zero feedback on whether the injected context was actually useful**:

1. **No usage signal**: The agent received 3 ADRs in its prompt. Did it reference them? Did the output change based on them? The system has no way to know.

2. **No relevance feedback**: An ADR that was retrieved but made the agent **worse** (distracted it, consumed context window with irrelevant info) is indistinguishable from a perfectly relevant ADR. The retriever cannot learn from negative examples.

3. **No cache invalidation gating**: The `ContextCache.Invalidate()` method exists but has **zero callers**:

```go
// cache.go:151-159
// Invalidate() is the hook for exactly that [dropping ADR cache on write].
// v1 has no writer of ADRs, so v1 never calls it; it exists so the v2
// author finds the seam already cut.
```

The cache has a mutex (`mu`) for thread safety and an `Invalidate()` method, but neither is ever exercised — dead code today. The cache's own doc admits this is "marginal" savings:

```go
// cache.go:11-22
// It saves LOCAL I/O only: a readdir of docs/adr + a firstHeading read per ADR...
// That is the WHOLE v1 win, and it is marginal.
// It does NOT save a single claude token.
```

4. **Score-based without feedback-driven re-ranking**: The retriever uses term frequency and inverse document frequency (IDF-lite). There is no mechanism to boost ADRs that were influential in past phases or down-rank ADRs that were injected but never referenced.

### Why This Matters

The context window is the most expensive resource in the system. Every token injected into the prompt costs money (API billing) and cognitive budget (the agent has less attention for other things). Blind injection means:
- An expanding ADR corpus increasingly crowds out task-relevant context
- The retriever has no incentive to improve — it's a stateless, non-learning component in a system otherwise built on learning loops (trace → scorecard → router)
- The system cannot distinguish between "we injected the right ADR and the agent used it" vs. "we injected noise and the agent ignored it"

This is the **missing learning loop** for the Context Engine — mirroring the learning loop that already exists for routing (Eval → scorecard → Router). Without it, context injection quality cannot improve with usage.

### Suggested Direction

Introduce a lightweight **context-usage signal** that doesn't require LLM introspection:

1. **Token-level trace**: Before injection, record which ADR IDs were injected. After the phase, scan the agent's output for `ADR-XXXX` references. An ADR that appears in both the injected set AND the output is "used". An ADR that was injected but never referenced is "unused."

2. **Feedback-adjusted retrieval scoring**: Boost ADRs that were historically "used" in similar phases. Down-rank ADRs that were repeatedly injected but never referenced. This turns the retriever from a stateless scorer into a **learning ranker** — the same Eval→scorecard→Router pattern applied to context.

3. **Wire Invalidate()**: When a `writes_adr` phase actually produces a new ADR (v2), call `ContextCache.Invalidate()` so the retriever picks up the new document. This is a one-line change but closes the dead-code gap.

**Value**: Each token in the prompt earns its place. The retriever improves with use instead of statically decaying. Completes the learning-loop architecture for the Context Engine (mirroring the Router's learning loop).

**Scope**: ~1.5 sprints. `internal/attribution` gets a `ContextUsage` type; `retrieve.go` gets a `FeedbackAdjust` method; `prompt_context.go` records used ADR IDs from agent output; the scorecard schema gets optional `context_used`/`context_injected` fields.

---

## Direction 5: Agent Card ↔ Workflow Phase Contract Drift — The Missing Governance Validation

### Current State

`harness/check.py` performs 10 governance integrity checks. The two most relevant are:

**`check_workflow_agent_refs`** (check.py:200-218): Validates that every workflow `agent:` value resolves to a file in `.agent/agents/`. This catches a workflow referencing a non-existent agent card.

**`check_agent_sections`** (check.py:187-197): Validates that every agent card `.md` file contains required section headers by keyword match.

### The Missing Validation Layer

Neither check validates the **cross-reference between what a workflow phase declares and what the agent card promises**. Specifically:

**A) `emits:` vs. agent card output boundaries**

The agent cards declare what they produce (in prose sections like "Outputs" or "Artifacts"). The workflow phases declare `emits:` paths. But there is **no check** that the two are consistent:

```yaml
# discover.yml — P1 emits requirement-draft.md
- name: requirement-discovery
  agent: product-manager
  emits:
    - requirement-draft.md
```

```markdown
# .agent/agents/product-manager.md
## Outputs
- requirement-draft.md (requirement description + confidence score)
- prd.md (product requirements document)
```

If the workflow emits `requirement-draft.md` but `product-manager.md`'s "Outputs" section doesn't mention it — or vice versa — there is **no governance signal**. The documentation drifts silently.

**B) `readonly:` vs. agent card stated permissions**

Workflow phases declare `readonly: true/false`. Agent cards state their permissions (e.g., "writes code" vs. "reviews code only"). But there is **no check** that a phase marked `readonly: true` uses an agent card whose stated permissions are also read-only:

```yaml
# build.yml — implementer writes code
- name: implementer
  agent: implementer
  readonly: false
```

```markdown
# .agent/agents/implementer.md
## Permissions
- Writes code files in any directory specified by the task plan
```

If the agent card says "reviews code only" but the workflow phase says `readonly: false` (allowing writes), the phase grants write permission to an agent that should never write — a **privilege escalation**.

**C) `requires_tools:` vs. agent card tool declarations**

```yaml
# discover.yml — P2 requires web_search + web_fetch
- name: market-research
  agent: researcher
  requires_tools: [web_search, web_fetch]
```

If the `researcher.md` agent card doesn't list `web_search` among its tools, or lists it as optional when the workflow declares it required, there's a contract mismatch. No check exists.

### Code-Level Evidence

```python
# check.py:200-218 — only validates agent NAME exists, not CONTRACT
def check_workflow_agent_refs(agent_root):
    valid = _agent_card_names(agent_root / "agents") | PSEUDO_AGENTS
    for path in sorted((agent_root / "workflows").glob("*.yml")):
        for phase in _collect_phases(data):
            agent = phase.get("agent")
            names = agent if isinstance(agent, list) else [agent] if agent else []
            for name in names:
                if name not in valid:
                    issues.append(f"{path}: workflow references unknown agent '{name}'")
    # That's it — no emits/readonly/requires_tools cross-validation
```

The `check_agent_sections` function only validates HEADER PRESENCE by keyword match — it never reads the content of those sections to extract contract terms:

```python
# check.py:167-175
def check_agent_sections(agent_root):
    for path in sorted((agent_root / "agents").glob("*.md")):
        structural = _structural_text(path.read_text(encoding="utf-8"))
        for label, synonyms in REQUIRED_AGENT_SECTIONS:
            if not any(syn in structural for syn in synonyms):
                issues.append(f"{path}: missing required section '{label}'")
    # Never reads what the sections say — only that they exist
```

### Why This Matters

This is a **governance gap in the governance layer itself**. ForgeOS's entire value proposition is that it prevents drift between design intent and implementation reality. But the most fundamental contract — **which agent does what, with which permissions, producing what artifacts** — has no enforcement bridge between the two declaration points (agent cards and workflow files).

This is not an academic concern. In a real evolve loop operating over weeks, an agent card might be updated to restrict permissions (e.g., "the reviewer should NEVER write files"), but if the workflow phase is not updated in lockstep to `readonly: true`, the governance fails silently.

### Suggested Direction

Add a new governance check `check_phase_agent_contract` that validates:

1. **`emits:` ↔ agent card "Outputs" section**: For each workflow phase, parse the referenced agent card's "Outputs" section. Every `emits:` path in the workflow must appear in the agent card's documented outputs. A mismatch is a WARN (documentation drift) or FAIL when the cast is safety-relevant (e.g., a phase emits to a sensitive path the agent card doesn't authorize).

2. **`readonly:` ↔ agent card "Permissions" section**: Parse the agent card's Permissions section. If the agent card describes only read-only actions (review, analyze, assess), but the workflow phase has `readonly: false`, flag as a FAIL. If the agent card describes read-write actions but the phase is `readonly: true`, flag as a WARN (potentially unnecessary restriction).

3. **`requires_tools:` ↔ agent card "Tools" section**: Parse the agent card for tool declarations (optional vs. required). Cross-check against workflow `requires_tools`. A mismatch is a WARN.

The function follows the existing `check.py` pattern: pure, returns string issues, integrated into `CHECKS` and `forge accept`.

**Value**: Closes a governance blind spot in the governance layer itself. Prevents silent privilege escalation (agent gets write permission when it should only read). Prevents silent drift between what agents are documented to produce and what workflows actually expect.

**Scope**: ~1 sprint. New Python functions in `harness/check.py` (or a new `harness/contract_check.py` for line budget), plus ~20 test cases in `harness/test_check.py`. Requires consistent section naming convention in agent cards — the check can start with a `TODO` for cards that don't follow the convention (honest N/A behavior).

---

## Summary

| # | Direction | Code Evidence | Value | Scope |
|---|---|---|---|---|
| 1 | Memory Compaction Semantic Blind Spot | `memory_compact.go:108-166` — content-agnostic, map-key re-use, critical findings compacted alongside noise | Prevents the evolve loop from forgetting critical findings | ~1 sprint |
| 2 | Runtime State Versioning & Cross-Process Safety | `persist/checkpoint.go` (no version/lock), `trace.go:106-110` (auto-assigns v1, never validated), `memory/memory.go` (appends with no lock) | Prevents silent data corruption in multi-process/CI settings | ~2 sprints |
| 3 | Forked pi-batch.py Orchestration | `pi-batch.py` (480 lines, root-level, zero tests, documented timeout bug) + `forge-init.mjs` COPIED_FILES (not included) | Eliminates architectural drift; one governance path for all agent CLIs | ~2 sprints |
| 4 | Context Retrieval Has No Feedback Loop | `retrieve.go` (pure stateless TF-lite), `cache.go` (Invalidate with zero callers), no usage-tracking in prompt builder | Makes context injection a learning loop instead of blind injection | ~1.5 sprints |
| 5 | Agent Card ↔ Workflow Phase Contract Drift | `check.py:200-218` (only validates agent NAME exists), `check.py:187-197` (only validates HEADER presence, never cross-references `emits`/`readonly`/`requires_tools`) | Closes governance blind spot in governance layer; prevents silent privilege escalation | ~1 sprint |
