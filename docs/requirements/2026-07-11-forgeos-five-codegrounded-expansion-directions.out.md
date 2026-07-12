Code evidence verified across **all 5 directions**. Every `file:line` reference checks out. Now let me give you my architectural synthesis and prioritization feedback.

---

## Feedback: Global Code Scan — Verification & Synthesis

### Verification Results

| # | Direction | Key Claims | Verification |
|---|---|---|---|
| 1 | Memory Compaction Blind Spot | Content-agnostic retention; `summarizeBlock` uses same map key for topic counting; `CompactAgeSeconds=86400` discriminates only on age | **Confirmed** — `memory_compact.go:108-121` pure mechanical; `summarizeBlock:149-166` uses `topics` map for both enumeration AND dedup (the `total` int saves it, but the code shape is fragile) |
| 2 | Runtime State Versioning & Cross-Process Safety | `rebuild.go` never checks Format; no file locking anywhere | **Confirmed** — `rebuild.go`: 0 matches on `Format` across entire file. `persist/checkpoint.go`: `Load` reads + decodes raw, no version check. Zero `flock`/`LockFileEx`/`fcntl` calls across entire forge-core tree. `.forge/` files exist with no lock protocol. |
| 3 | pi-batch.py Fork | Standalone orchestration outside governance; timeout double-count | **Confirmed** — 499 lines, root-level, **absent from** `forge-init.mjs:86-130` `COPIED_FILES`. Timeout bug at `pi-batch.py:236-245`: both reader threads consume the full remaining budget independently. |
| 4 | Context Retrieval No Feedback Loop | `Invalidate` has zero production callers; retriever is pure stateless TF-lite | **Confirmed** — `cache.go:95-102` `Invalidate` defined + tested (`cache_test.go:180-184`) but unreachable from production prompt paths. `retrieve.go` pure keyword-only. |
| 5 | Agent Card ↔ Workflow Phase Contract Gap | `check_workflow_agent_refs` only validates NAME; no `emits`/`readonly`/`requires_tools` cross-reference | **Confirmed** — `check.py:200-218` membership test only. Workflow files (`discover.yml:48`, `design.yml:12,43,48`) use all three fields. Agent cards declare Outputs/Permissions/Tools in prose. Zero cross-validation exists. |

---

### Architecture-Level Observations

**1. The missing-learning-loop pattern unifies Directions 1, 4, and 5.**

Direction 1 (memory compaction) lacks a *retention policy that learns* from entry importance. Direction 4 (context retrieval) lacks a *ranking that learns* from usage signal. Direction 5 (governance) lacks a *contract that learns* from drift between two declaration points.

Each is a **stateless decision point** in a system whose entire value proposition is the *learning loop*. The `Eval → scorecard → Router` pattern (well-documented in the codebase) should extend to:
- `Eval → scorecard → CompactPolicy` (Direction 1)
- `Eval → scorecard → RetrieveRanking` (Direction 4)
- `Eval → scorecard → ContractCheck` (Direction 5)

This is not three separate features — it's one architectural gap expressed in three places. Fixing them with the same pattern (feedback-adjusted stateful policy) is cheaper and architecturally more coherent than fixing each independently.

**2. Direction 2 (state format) is the highest-risk, lowest-observability bug class.**

Silent data corruption (Direction 2) is the hardest to diagnose post-hoc: the evidence is overwritten by the process that caused it. The cross-process race (two `forge evolve` loops in one repo) is the most dangerous because both processes are *correct from their own perspective* — they just silently corrupt each other's state. This is the direction most likely to produce a **customer-data-loss incident** in production.

The versioning sub-problem (Direction 2A) has a cheap immediate fix: validate `_format` on read in 3 places. The file-lock sub-problem (Direction 2B) is a new subsystem but structurally well-understood.

**3. Direction 3 (pi-batch.py) is a symptom of a deeper architectural question the project hasn't answered.**

The existence of pi-batch.py as a standalone root-level Python script is *symptomatic*, not causal. The project has two agent CLIs (forge and pi) and no abstraction for "agent-agnostic batch task execution." The real work is: should the orchestration layer in forge-core become the canonical batch executor for *any* agent CLI? If yes, Direction 3 is 1–2 sprints of integration work. If no, pi-batch.py will have a successor (gemini-batch.py, codex-batch.py) for every new CLI.

This is a product decision the analysis correctly surfaces but doesn't resolve.

**4. The document has one structural weakness.**

Direction 2 conflates **two orthogonal concerns** under one heading:
- **2A: Schema versioning** — a data-format contract problem (what happens when v2 writes and v1 reads?)
- **2B: Cross-process file locking** — a concurrency safety problem (what happens when two processes write the same file?)

These have different failure modes, different fixes, and different priority levels. Versioning is ~3 localized edits (check Format on read); file locking is a new subsystem. I'd split Direction 2 into two separate tracks in your prioritization.

---

### Prioritization Recommendation (for 1–2 sprints)

| Priority | Direction | Why | Effort | Quick Win Possible? |
|---|---|---|---|---|
| **P0** | 2B (File locking) | Silent data corruption; hard to diagnose; production-incident risk | ~1 sprint | No — needs new subsystem |
| **P1** | 2A (Format validation) | Cheapest immediate correctness gain; 3 reads to validate | ~1 day | **Yes** — `checkpoint.go` Load, `rebuild.go` ExtractRebuildPairs, `memory.go` Load |
| **P1** | 5 (Contract governance) | Closes governance gap IN the governance layer; highest symbolic value | ~1 sprint | Partial — `emits` cross-ref is ~50 lines of Python |
| **P2** | 1 (Importance-weighted retention) | Prevents critical findings from being compacted; ~30 LOC change | ~1 sprint | **Yes** — add `Priority` field + filter in `compactByKind` |
| **P2** | 4 (Retrieval feedback loop) | Architectural completeness; but low immediate impact for v1 corpus | ~1.5 sprints | Partial — wire `Invalidate` is 1 LOC |
| **P3** | 3 (pi-batch integration) | Product decision, not a bug; architectural drift but no acute risk | ~2 sprints | Depends on direction chosen |

The P1s (2A + 5) in combination cost ~1 week and close the most embarrassing gaps: "format markers exist but are never read" and "governance checker checks names but not contracts." These are the highest signal-to-noise improvements in the list.

---

### One Direction You Missed

There's a **Direction 6** suggested by the pi-batch.py analysis that extends beyond it. Your analysis notes that `pi-batch.py` is not in `COPIED_FILES`, but the deeper pattern is:

**The forge-init scaffold has no "install third-party agent" protocol.**

`harness/scaffold/forge-init.mjs` copies exactly `harness/` tools + `.agent/` skeleton. There's no `forge init --with pi` or `forge install-agent pi` that adds the `pi` agent's batch executor, or any mechanism to extend `COPIED_FILES` for a new agent CLI. Every new agent CLI the project supports requires a code change to `forge-init.mjs`.

The uncovered direction: **ForgeOS needs an agent-plugin interface that makes adding a new agent CLI (pi, Codex, Gemini CLI, OpenHands) a configuration change, not a code change.** This is what makes Direction 3's resolution scale — if the batch executor lives behind an agent-agnostic interface in forge-core, then pi-batch.py is just one YAML config entry *per agent CLI* that maps `agent_name → executor_cmd + timeout_policy + output_contract`.

Is this worth adding to the document, or is it too early (the project only has forge + pi and the second is a special case)?
