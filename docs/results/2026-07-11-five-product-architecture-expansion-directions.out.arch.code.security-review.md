---

I've now thoroughly examined the ForgeOS codebase, its architecture, and the recent commit history (including the latest `905b65e [pi-batch] Stage: code-implementation` with changes across 127 files, 6378 insertions). I've also reviewed the existing security-review artifacts in `docs/results/`.

**Before I can perform a rigorous security review, I need clarification:**

You've sent me two role prompts — the **Code Implementer** prompt (indicating your role) and the **Security Engineer Role Prompt** (indicating what you want from me). However, you haven't identified **which specific subsystem or set of changes** you want reviewed.

Based on my analysis of the recent code, here are the security-relevant areas I can review:

| # | Subsystem | What Changed | Security Surface |
|---|-----------|-------------|-----------------|
| A | **Command Executor** | Context propagation (`Ctx`), SandboxConfig skeleton, `cappedBuffer` changes | Process isolation, signal handling, resource exhaustion |
| B | **Memory Caching** | Global `sync.Map` cache in `memory.go`, confidence/supersedes fields | Global mutable state, cache poisoning, TOCTOU |
| C | **Parallel Orchestration** | Wave-level cancellation, fail-fast, lock ordering contract | Race conditions, DoS, budget accounting |
| D | **Routing** | Exported internals (`Rank`, `TaskTypeFloor`, `SafetyForceOpus`), tier mapping | API surface exposure, escalation paths |
| E | **Prompt Context/Ledgers** | `phaseOutputLedger`, `verdictLedger`, feed-forward wiring | Data leakage, injection into agent prompts |
| F | **Executive Verdict Parser** | CTO 5-way verdict tokens, confidence score parsing | Signal fabrication, prompt injection |
| G | **Checkpoint Persistence** | Format versioning, retain/rotation | Data integrity, rollback attacks |
| H | **Signal Handling** | `withSignalCancellation`, SIGINT/SIGTERM propagation | Denial of service, state corruption |
| I | **Workflow Mode Gating** | `reviewStageSkipped`, `stageDepthAtMax`, `optional_for` | Enforcement bypass, privilege escalation |

**Please specify which subsystem(s) you'd like me to review**, or if you want a comprehensive review of the entire recent change set. I can prioritize by severity/risk once you clarify scope.
