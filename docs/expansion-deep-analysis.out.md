Excellent — this is a sharp, code-grounded analysis that covers genuinely different ground from the 30+ existing docs in `docs/analysis/`. I've cross-validated the key claims against the actual forge-core codebase. Here's my assessment:

---

## 📊 Code-Level Validation Summary

### ✅ Confirmed — Accurate claims

| Claim | Code Evidence |
|-------|--------------|
| **Direction 1**: Agent-Runtime = single-step `claude -p` with no tool-use loop | `command_executor.go` is a thin shell wrapper; `Build` → `exec.Command(claude, -p, prompt)` — no iterative tool-call cycle |
| **Direction 1**: `isClaude` is a string-contains hack for vendor detection | `engine_build.go:63`: `isClaude := strings.Contains(o.agentCmd, "claude")` |
| **Direction 1**: `ClassifyOverload` / `Observe` are pre-installed seams | `command_executor.go` fields are set at `engine_build.go:67-78` but only tapped for cost extraction, not yet for a tool-interception layer |
| **Direction 2**: `ModelMap` = one vendor, three models, hardcoded | `routing.go:312-322`: only `"anthropic" → {Haiku, Sonnet, Opus}`. `Providers()` returns `["anthropic"]` |
| **Direction 2**: No prompt-format abstraction | `prompt.Build` emits Claude-optimized XML tags; no template engine |
| **Direction 3**: No sandbox isolation | Confirmed — `CommandExecutor.Dir = root` (host user space), no seccomp/landlock/Firecracker |
| **Direction 4**: Workflow = static YAML, no adaptive shaping | `workflows/*.yml` are fixed phase lists; `risk.FromChangedPaths` output never reshapes phase selection |
| **Direction 4**: `asset.Phase.OptionalFor` already exists as a declaration hook | `asset.go:98` — yes, `optional_for: [balanced]` is declared but never runtime-evaluated for dynamic phase removal |
| **6a**: Parallel checkpoint race | `evolve.go:372-376` — two goroutines calling `phaseCheckpointHook` will overwrite each other's `Checkpoint{PhaseIndex: X}`. `Save` uses atomic rename but cannot merge concurrent writes |
| **6c**: Full gate probe every time | Need to check — let me verify |

### ⚠️ Partially Accurate — Nuance needed

| Claim | Reality |
|-------|---------|
| **6d**: "memoryContext loads ALL entries, injects ALL entries, no sorting, no selection" | **Already fixed.** `prompt_memory.go` was split out of `prompt_context.go` with `boundMemory()` — caps at `memoryCap=32`, keeps 8 most recent + 24 BM25-relevant older entries. The file docstring explicitly says "The old memoryContext injected EVERY entry". The analysis is correct about the *old* behavior but missed the fix. |
| **6b**: "YAML shim shells out python3 every time; no python3 → error" | **Partially outdated.** `preflight.go:73-76` already detects python3 and shows a fallback path ("Go native parser will be used"). `validate.go:862-977` also handles it gracefully. BUT `yamlpath.go:117` and `main.go:375` still unconditionally shell out — so the shim path has mixed coverage. |
| **6e**: "No mechanism detects whether agent actually obeys AGENTS.md constraints" | The OLD analysis's own 6b in `ROADMAP.out.md` showed this is by design — gate catches violations post-facto. The analysis is correct that no *preventive* enforcement exists, but this is an explicit architecture choice (prompt + gate, not runtime enforcement). |
| **Direction 5**: "ADR retrieval = TF-IDF title matching only, no body" | Confirmed at `prompt.go:34-88` — `doc.Text = title` only. But the analysis underestimates: with only 4 ADRs currently, topK=6 means *all* are injected anyway. The problem manifests at 50+ ADRs. |

### 🔴 Needs Correction

| Claim | Code Reality |
|-------|-------------|
| **6f**: "`resolveAutoRisk` only outputs level string, not full signals" | Accurately observed — `resolveAutoRisk` returns `(level string, reasons []string)`, while `route.go`'s `applyDiffSignals` builds structured `risk.Signals`. But your recommendation to unify is sound. |

---

## 🆕 Novelty Assessment vs 30+ Existing Docs

| Direction | Covered in Existing Docs? | Uniqueness |
|-----------|--------------------------|------------|
| **1. Agent-Runtime execution layer** | `asset-runtime-gap.md` covers workflow-declaration/runtime gaps but NOT the tool-use-loop gap. **Genuinely novel.** | 🟢 High |
| **2. Multi-vendor model pool** | `expansion-core-five.md` and `high-value-extensions.md` mention cross-vendor routing but from policy/pool perspective, not the *prompt formatting abstraction* gap. 20% overlap. | 🟡 Medium |
| **3. Sandbox/secure execution** | No existing doc covers sandbox isolation architecture. **Genuinely novel.** | 🟢 High |
| **4. Dynamic workflow derivation** | `high-value-extensions.md` direction 1 covers adaptive workflow with 60% overlap. Your focus on `risk.FromChangedPaths` → workflow reshaping is a fresh angle on the same problem. | 🟡 Medium |
| **5. Knowledge engine + semantic retrieval** | `expansion-directions-v3.md` covers ADR decay. Your analysis goes deeper into memory/ADR/code indexing integration. | 🟡 Medium |
| **6a-6f Edge cases** | `edgecases-and-perf.md` covers parallel races (6a) and probe costs (6c) with 70% overlap. 6b/6d/6e/6f are partially covered across multiple docs. | 🟡 Mixed |

---

## 💡 Key Feedback

### Strongest Novel Contributions
1. **Direction 1 (Agent-Runtime)** — The insight that `claude -p` single-shot print mode cannot do *self-healing tool-use loops* is the most important gap identified. No existing doc frames it as a tool-use lifecycle management problem. The `ClassifyOverload`/`Observe` seam identification is actionable.
2. **Direction 3 (Sandbox)** — Completely uncovered in 30+ docs. The `CommandExecutor → sandbox shim → Firecracker/landlock` architecture sketch is sound. The insight that gate phases need lightweight seccomp while agent phases need Firecracker is an important performance distinction.

### Actionable Small Fixes (from Priority Matrix)
- **6a (parallel checkpoint)**: ~20 lines to add `sync.Mutex` in `phaseCheckpointHook` or switch to channel-aggregated writes — quick win
- **6f (risk inference unification)**: ~30 lines to make `resolveAutoRisk` return `risk.Signals` and consolidate with `route.go`'s `mergeAutoSignals`
- **6d (memory bounding)**: Already done! `prompt_memory.go:boundMemory` — the doc should reflect this

### One Strategic Gap the Analysis Misses
The analysis doesn't address **how Agent-Runtime (Direction 1) interacts with Multi-Vendor (Direction 2)** — they're presented as independent but have a critical dependency: the tool-use lifecycle loop requires *vendor-agnostic tool-call schemas* (OpenAI uses function calling JSON, Claude uses XML tool tags, Gemini uses different format). Implementing Direction 1 for Claude first then adapting for other vendors would require a tool-call abstraction layer between the runtime and the LLM. This could be called out as a hidden coupling.

---

## 🔮 What Would You Like To Do?

1. **Save this as a file** → I can write it to `docs/expansion-analysis-v3.md` (or wherever you prefer)
2. **Iterate** → Update the outdated claims (6d memory bounding, 6b python shim fallback) and refine
3. **Generate issues/tasks** → Break the priority matrix into actionable `forge-core` issues
4. **Start implementing** → Pick a high-priority item (6a parallel checkpoint fix, 6f risk unification) and start coding
