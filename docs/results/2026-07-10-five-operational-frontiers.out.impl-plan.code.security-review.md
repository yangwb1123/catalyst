Now I have a thorough understanding of the codebase. Let me compile the comprehensive security review.

---

# Security Review: Five-Direction Adoption Gating — Product Trust Gaps

## Executive Summary

This is an **add-only, zero-new-dependency expansion** of ForgeOS runtime infrastructure covering 57 Go source files (~5.3K added, ~2K deleted). The implementation focuses on data model completeness (asset.Phase), convergence signal integrity (converge.Signals), context propagation, parallel orchestration fail-fast, and storage layer hardening.

**Overall Security Posture: Good** — The code demonstrates strong security awareness in several areas (recursion guards, output caps, sanitization of agent outputs, lock order contracts). However, several medium-severity findings arise from the trust model of agent outputs, the absence of input validation on certain paths, and the lack of encryption at rest.

---

## Detailed Findings

### Finding 1 — Command Injection Surface via `git` Execution

| Field | Value |
|-------|-------|
| **Category** | Input Validation |
| **Severity** | **High** |
| **Title** | Unvalidated `root` path passed to `git diff` commands |
| **Location** | `cmd/forge/gates.go:383` (`computeCodeTestRatio`), `gates.go:450` (`computeFileDelta`) |
| **Description** | Both functions construct `exec.Command("git", "-C", root, "diff", …)` where `root` originates from CLI flag `--root`, env `$FORGE_REPO_ROOT`, or fallback `"."`. While `exec.Command` avoids shell injection, the `-C <path>` flag is still processed by git. A maliciously crafted `root` containing newlines or git-option delimiters (`--output` etc.) could alter git's behavior. The `childEnv` function in `command_executor.go` likewise trusts environment variables from `os.Environ()`. |
| **Attack Scenario** | An attacker who controls the working directory or `--root` argument (e.g., through a malicious repo clone that a user runs `forge` inside) could craft a path like `/tmp/exploit --output=/tmp/leak` — though `exec.Command` passes it as a single argv element to git's `-C`, git may interpret the embedded spaces. More practically, if `root` is a symlink to another project, git diff would leak that project's uncommitted changes. |
| **Impact** | Information disclosure of git history/diffs from unintended directories; limited command-argument injection in git's option parser. Low RCE probability but non-trivial confidentiality risk. |
| **Recommendation** | Validate `root` is an actual directory path before passing to git: `fi, err := os.Stat(root); if err != nil || !fi.IsDir() { return error }`. Also canonicalize the path with `filepath.Abs()` + `filepath.EvalSymlinks()`. Consider using `git -C` only after root validation. |
| **Effort** | **S** (< 1 day) |

### Finding 2 — Race Window in Memory Load Cache (TOCTOU)

| Field | Value |
|-------|-------|
| **Category** | Threat Model (Tampering) |
| **Severity** | **Medium** |
| **Title** | Non-atomic cache freshness check in `loadCache` |
| **Location** | `internal/memory/memory.go:67-91` (`loadFromCache`, `storeToCache`) |
| **Description** | The memory store's load cache checks file modification time (`os.Stat`) against a cached mtime. Between the mtime check and the actual `os.ReadFile` (in `Load`), or between `invalidateLoadCache` and a concurrent process's `Load`, a writer process could modify the file. The cache uses `sync.Map` for map-level safety, but the freshness decision (stat → compare → return cached) is not atomic with disk state. `Append` calls `invalidateLoadCache` to clear all entries, but a concurrent `Load` could re-cache stale data between the invalidation and the new write. |
| **Attack Scenario** | In a multi-process scenario (concurrent `forge` runs on the same repo), Process A appends a knowledge entry at T0. Process B's `Load` at T0+ε sees the old mtime, serves stale cached data, and makes a convergence decision based on outdated knowledge. The corrupted entry could cause the loop to re-discover findings or make decisions based on superseded information. |
| **Impact** | Knowledge staleness leading to incorrect agent convergence decisions. Low integrity impact in single-process use; medium under concurrent access. |
| **Recommendation** | Replace mtime-based cache with a file-content hash or inode+generation check. Or, add an atomic sequence counter to the cache key that increments on every Append. For the single-process v1 use case, document that the cache is not safe for concurrent writer+reader processes. |
| **Effort** | **M** (1-3 days for full fix; S for documentation-only) |

### Finding 3 — World-Readable Runtime State Files

| Field | Value |
|-------|-------|
| **Category** | Data Protection |
| **Severity** | **Medium** |
| **Title** | Checkpoint and memory store created with `0o644` permissions |
| **Location** | `internal/persist/checkpoint.go:132` (`writeSynced`), `internal/memory/memory.go:155-156` (`Append`) |
| **Description** | `writeSynced` opens checkpoint files with `0o644` (world-readable). `Append` opens the memory store with `0o644`. These files contain: phase-level agent outputs (including code snippets, design decisions), cumulative billing costs (micro-dollar amounts), convergence state, and reviewer verdicts. On a multi-user system, any local user can read these files. |
| **Attack Scenario** | User A runs `forge evolve` on a sensitive project with PII or proprietary code in agent outputs. User B (also on the same system) reads `~/.forge/<project>/checkpoint.json` and extracts the agent's feed-forward outputs, including proprietary design details. |
| **Impact** | Information disclosure of agent-generated content, billing data, and convergence state. Violates principle of least privilege. GDPR concern if PII enters agent outputs. |
| **Recommendation** | Change permissions to `0o600` (owner-only). For `writeSynced`: `os.OpenFile(…, 0o600)`. For `Append`: same. Add a note that on shared CI runners, the `.forge` directory should be excluded from artifacts or encrypted. |
| **Effort** | **S** (< 1 day) |

### Finding 4 — Missing Path Canonicalization in YAML Template Reads

| Field | Value |
|-------|-------|
| **Category** | Input Validation |
| **Severity** | **Medium** |
| **Title** | `UsesTemplate` / `SecondaryTemplate` paths could enable file read traversal |
| **Location** | `internal/asset/asset.go:88-92` (`Phase.UsesTemplate`), `cmd/forge/prompt_artifacts.go` (if it reads these paths) |
| **Description** | `Phase.UsesTemplate` and `Phase.SecondaryTemplate` store file paths from the workflow YAML. These are read in prompt construction. An attacker who controls the workflow YAML (e.g., committed malicious `.agent/workflows/*.yml`) could specify a template path like `../../../etc/passwd` to read arbitrary files into the agent's context, or a network path to probe the filesystem layout. |
| **Attack Scenario** | A malicious contributor submits a modified `review.yml` with `uses_template: ../../../../etc/shadow`. The workflow loader reads it into the Phase struct. When the agent runs, the file content is injected into the prompt, potentially leaking sensitive system files. |
| **Impact** | Local file disclosure. Arbitrary file read within the filesystem permissions of the `forge` process. |
| **Recommendation** | Add a path traversal guard when resolving `UsesTemplate` / `SecondaryTemplate`: use `filepath.Join(repoRoot, path)` and verify the resolved path's prefix equals `filepath.Clean(repoRoot)`. Reject paths that escape the repo. |
| **Effort** | **S** (< 1 day) |

### Finding 5 — Agent Output Overload Misclassification (529 Pattern Splat)

| Field | Value |
|-------|-------|
| **Category** | Threat Model (Denial of Service) |
| **Severity** | **Medium** |
| **Title** | Terminal agent error can be misclassified as transient overload |
| **Location** | `cmd/forge/cost.go:180-220` (`classifyClaudeOverload`, `hasOverloadMarker`) |
| **Description** | `hasOverloadMarker` matches the substring `"overloaded"` (case-insensitive) in the agent's output. While gated on `is_error==true`, a real terminal failure whose error message happens to contain the word "overloaded" would be classified as transient, triggering retries-with-backoff that burn budget on a never-succeeding call. The code acknowledges this: "A FALSE POSITIVE (a real terminal KindFailed mislabeled transient) is DANGEROUS." |
| **Attack Scenario** | (a) An agent writes code that triggers an LLM provider error containing "overloaded" as incidental text. The forge runtime classifies this as overload, retries 2 more times (consuming budget), then fails. (b) A crafted adversarial input to the agent causes it to emit an error containing "overloaded", inducing the same retry loop and budget waste. |
| **Impact** | Budget depletion on non-recoverable errors. Up to `MaxRetries` × per-phase cost wasted per incident. In an evolve loop, this compounds. |
| **Recommendation** | Tighten the overload classifier: only match the exact JSON envelope pattern (`api_error_status: 529`) as the sole trigger. Remove the textual fallback entirely, or at minimum require both `overloaded_error` (the API's exact error type string, not the word "overloaded") AND `is_error==true`. |
| **Effort** | **S** (< 1 day) |

### Finding 6 — No Rate Limiting on Agent Spawns Within a Wave

| Field | Value |
|-------|-------|
| **Category** | Threat Model (Denial of Service) |
| **Severity** | **Medium** |
| **Title** | Unbounded concurrent agent spawns in parallel wave mode |
| **Location** | `internal/orchestrator/parallel.go:97-118` (`runWave`, goroutine per phase) |
| **Description** | In `RunParallel`, each dependency wave spawns one goroutine per phase simultaneously. If a workflow declares many independent phases (e.g., 50 phases with no cross-dependencies), 50 concurrent `claude -p` subprocesses would be spawned. Each `claude` call opens an HTTPS connection to Anthropic's API, potentially overwhelming the local machine's process/connection limits or triggering API rate-limiting that manifests as 529 overload, then retries, compounding the concurrency. |
| **Attack Scenario** | A workflow with 100 parallel phases is run with `--parallel`. 100 `claude` processes are spawned simultaneously. The local system runs out of file descriptors/process slots, causing failures. The Anthropic API receives 100 concurrent requests from the same API key, rate-limiting some, causing retries that overlap with new waves. |
| **Impact** | Resource exhaustion (OOM, FD exhaustion, API rate-limit triggering). Could cause the orchestrator itself to be killed by the system OOM killer. |
| **Recommendation** | Add a concurrency limiter: a semaphore or worker pool that bounds simultaneous agent spawns within a wave. Default to `min(len(wave), runtime.GOMAXPROCS(0))` or a configurable `--parallel-limit`. Also document that `--parallel` with many independent phases should be used with max-agent-calls and run-budget-usd caps. |
| **Effort** | **M** (1-3 days) |

### Finding 7 — Trusted Agent Output As Convergence Signal

| Field | Value |
|-------|-------|
| **Category** | Threat Model (Spoofing / Tampering) |
| **Severity** | **Medium** |
| **Title** | Convergence signals derived entirely from agent self-report |
| **Location** | `internal/converge/converge.go:75-90` (`evalRequirementConfidence`), `gates.go:380-410` (`requirementConfidence`) |
| **Description** | `RequirementConfidence` and `ReviewStatus` are both parsed from the agent's own output. The code explicitly acknowledges this is "honest-but-trusting": the agent's last line is parsed for `CONFIDENCE: <N>` or `VERDICT: APPROVE`. A compromised or adversarial agent can fabricate any confidence score or approval verdict. The orchestrator treats this as a soft signal (fail-open for reviewer, unmet for confidence), but it still enters the convergence check. |
| **Attack Scenario** | An attacker supplies a malicious agent card (`.agent/agents/product-manager.md`) that instructs the agent to always emit `CONFIDENCE: 100`. Every discover run then reports 100% confidence regardless of actual requirement quality, bypassing the stop condition's threshold. Or an agent that emits `VERDICT: APPROVE` without doing real review. |
| **Impact** | Agent self-report can bypass convergence gates. Low practical risk because (a) gates_status and roadmap_completion provide independent verification, and (b) agent cards are version-controlled. But in a supply-chain attack where agent cards are modified, this is a vector. |
| **Recommendation** | Strengthen the independence of `RequirementConfidence` and `ReviewStatus` thresholds: require them to be corroborated by at least one external signal (e.g., git diff evidence for confidence, or a minimum of findings produced for review). Document the "honest-but-trusting" limitation in the security model. |
| **Effort** | **M** (documentation + lightweight validation) |

### Finding 8 — Weak Fork-Bomb Guard Against Malicious Agent

| Field | Value |
|-------|-------|
| **Category** | Authentication / Authorization |
| **Severity** | **Medium** |
| **Title** | `FORGE_AGENT_DEPTH` environment variable is trivially manipulable |
| **Location** | `internal/orchestrator/command_executor.go:163-170` (`currentAgentDepth`) |
| **Description** | The recursion guard reads `FORGE_AGENT_DEPTH` from the environment. The code acknowledges: "An agent with arbitrary env control already has other escapes, so hardening to fail-secure would buy no real safety while breaking honest runs." A malicious agent that has Bash access can unset or modify `FORGE_AGENT_DEPTH` before re-invoking `forge`, bypassing the depth ceiling and causing unbounded recursive agent spawns. |
| **Attack Scenario** | An agent with Bash access runs `unset FORGE_AGENT_DEPTH && forge run build --executor=command`. The depth resets to 0, and the child is spawned without the recursion guard firing. This repeats unboundedly, creating a fork-bomb of agent processes that quickly exhausts the system's process table and the API budget. |
| **Impact** | Denial of service (resource exhaustion), cost exhaustion (API billing), and potential system instability. |
| **Recommendation** | (a) Use a process-group kill (already partially implemented via `setupProcessGroup`) rather than relying solely on the depth counter. (b) Add a PID-based depth tracker on the parent side: the parent process tracks all descendant PIDs and refuses to spawn if the count exceeds a limit. (c) Document that the env-based guard is a defense-in-depth layer, not a security boundary, and that `--max-agent-calls` and `--run-budget-usd` are the primary cost controls. |
| **Effort** | **M** (1-3 days for PID-based tracker) |

### Finding 9 — No Input Size Bounds on YAML Workflow Load

| Field | Value |
|-------|-------|
| **Category** | Threat Model (Denial of Service) |
| **Severity** | **Low** |
| **Title** | Unbounded workflow YAML can exhaust memory during loading |
| **Location** | `internal/yaml2json/yaml2json.go:32-45` (`Decode`), `cmd/forge/main.go:220-240` (`loadWorkflow`) |
| **Description** | `loadWorkflow` reads the entire workflow YAML into memory via `io.ReadAll`, then parses it into a generic Go value. There is no input size limit. An attacker who can commit a workflow file (or control a workflow path) could supply a multi-gigabyte YAML file designed to trigger OOM during parsing. The Go yaml parser also has no depth limit by default, so deeply nested YAML could cause stack exhaustion. |
| **Attack Scenario** | A contributor submits a PR that creates `.agent/workflows/build.yml` containing a deeply nested (10,000+ levels) structure. When any user runs `forge run build`, the parser either exhausts stack (panic) or heap memory, crashing the process. |
| **Impact** | Denial of service against the forge CLI. Could prevent all forge operations on a repository containing the malicious workflow. |
| **Recommendation** | Add a `io.LimitReader(r, maxWorkflowBytes)` wrapper around the YAML read (e.g., 10MB). Add a recursion depth limit in the parser (or use `json.Decoder` with `DisallowUnknownFields` for the JSON path). |
| **Effort** | **S** (< 1 day) |

### Finding 10 — Potential Information Leak via Error Messages

| Field | Value |
|-------|-------|
| **Category** | Data Protection |
| **Severity** | **Low** |
| **Title** | File paths and git diff output exposed in log messages |
| **Location** | `cmd/forge/gates.go:383-450` (`computeCodeTestRatio`, `computeFileDelta`), `internal/orchestrator/command_executor.go:155-170` (`logf`) |
| **Description** | `computeCodeTestRatio` and `computeFileDelta` read git diff output which contains full file paths relative to the repository root. If these logs are stored or shipped to an external system, they reveal the exact repository structure. Similarly, `command_executor.go` logs the full command line including prompt content (via `Build`). The `RenderLog` mechanism can filter agent outputs, but file paths from git commands are logged directly. |
| **Attack Scenario** | Logs from `forge run --executor=command` are collected into a central logging system (e.g., Datadog, ELK). The logs contain the exact file tree of the project, including files that may be sensitive (e.g., `deploy/secrets.yml`, `config/credentials.json`). An attacker with read access to the logging system can enumerate the project structure. |
| **Impact** | Low — file paths reveal project structure but not content. Could aid an attacker in targeting specific files. |
| **Recommendation** | Add an option to redact file paths in logs, or log only the count/duration instead of the full diff paths. For git commands that only need statistics (`--stat`), the paths are already aggregated, but `computeFileDelta` uses `--name-only`. Consider making the path listing conditional on a verbose/debug flag. |
| **Effort** | **S** (add redact mode) |

### Finding 11 — No Integrity Check on On-Disk State

| Field | Value |
|-------|-------|
| **Category** | Threat Model (Tampering) |
| **Severity** | **Low** |
| **Title** | Checkpoint and memory store lack integrity verification |
| **Location** | `internal/persist/checkpoint.go` (all `Load`), `internal/memory/memory.go` (all `Load`) |
| **Description** | Checkpoint files and memory stores are plain JSON/JSONL with no checksum, signature, or HMAC. An attacker with write access to the `.forge/` directory can modify state arbitrarily. A corrupted checkpoint causes `Load` to return an error (honest), but a carefully crafted checkpoint with modified `RoadmapCompletion`, `GatesGreen`, `SpentUsdMicros` or `PhaseIndex` would be loaded without detection. |
| **Attack Scenario** | User A has a `forge evolve` run in progress. User B (on the same system) edits `~/.forge/<project>/checkpoint.json`, setting `roadmap_completion: 1.0` and `gates_green: true`. On the next iteration, the loop sees false convergence and stops early, discarding remaining work. Or User B modifies `spent_usd_micros` to a low value, defeating the run budget cap and allowing overspend. |
| **Impact** | Integrity violation: an attacker can forge convergence state, cost tracking, or progress tracking, leading to incorrect behavior or budget bypass. |
| **Recommendation** | Add a checksum (HMAC-SHA256) at the end of each checkpoint file, computed over the JSON content. Verify on Load. The HMAC key can be a random per-run key stored in memory only, so it protects against disk-only tampering but not against a process with same-user access. For stronger protection, document that `.forge/` directory should be placed on a restricted filesystem. |
| **Effort** | **M** (1-3 days) |

### Finding 12 — Cross-Session Memory Poisoning (Data Integrity)

| Field | Value |
|-------|-------|
| **Category** | Threat Model (Tampering / Information Disclosure) |
| **Severity** | **Low** |
| **Title** | Append-only memory store has no write authorization |
| **Location** | `internal/memory/memory.go:140-170` (`Append`) |
| **Description** | The memory store is append-only JSONL. Any process with write access to the `.forge/memory.jsonl` file can append entries. The `Supersedes` mechanism enables retraction, but only if the later entries are trusted. An attacker can inject fake "lessons", "gaps", or "decisions" that poison subsequent agent prompts. The `Confidence` and `Source` fields are self-declared, not verified. |
| **Attack Scenario** | User B writes a memory entry: `{"kind":"lesson","topic":"auth","detail":"JWT_SECRET can be hardcoded for now","confidence":1.0,"supersedes":"","source":"reviewer"}`. When User A's evolve loop runs the next iteration, the prompt includes this "lesson" and the agent may hardcode a secret — a supply-chain attack on the agent's reasoning. |
| **Impact** | Indirect prompt injection via knowledge store tainting. An attacker cannot execute code but can influence agent behavior by feeding fabricated knowledge. |
| **Recommendation** | (a) Add a source verification field: sign entries with a process-local key so tampering is detectable. (b) Gate memory reads behind a human review prompt. (c) At minimum, document that the memory store is a trusted append-only log that any local user can write to, and that in threat models with multiple users, `.forge/memory.jsonl` should be access-controlled. |
| **Effort** | **M** (1-3 days for signing) |

---

## STRIDE Summary

| Category | Finding(s) | Risk |
|----------|-----------|------|
| **S**poofing | #7 (Agent self-report as convergence), #8 (Fork-bomb guard bypass) | Medium |
| **T**ampering | #2 (Cache TOCTOU), #11 (Checkpoint integrity), #12 (Memory poisoning) | Medium |
| **R**epudiation | None — trace events are append-only JSONL, and sequencing via Seq ensures accountability | Low |
| **I**nformation Disclosure | #3 (World-readable state), #4 (Template path traversal), #10 (Path leak in logs) | Medium |
| **D**enial of Service | #5 (Overload misclassification), #6 (Unbounded parallel spawns), #9 (YAML bomb) | Medium |
| **E**levation of Privilege | #1 (Command injection surface) | Low |

---

## OWASP Top 10 (2021) Mapping

| OWASP Category | Relevant Finding(s) |
|----------------|---------------------|
| A01: Broken Access Control | #3 (world-readable state), #8 (weak recursion guard) |
| A02: Cryptographic Failures | #11 (no integrity checks on state files) |
| A03: Injection | #1 (git command paths), #4 (template path traversal), #12 (memory poisoning → indirect prompt injection) |
| A04: Insecure Design | #7 (trusting agent self-report), #5 (overload misclassification) |
| A05: Security Misconfiguration | #3 (0o644 perms), #6 (no parallel concurrency limit) |
| A07: Identification & Auth Failures | #8 (env-based guard bypassable) |
| A09: Security Logging & Monitoring | #10 (path leaks); Trace events are well-structured for audit |
| A10: SSRF | None directly; API calls are to Anthropic only (hardcoded in claude binary) |

---

## Final Summary

### Overall Security Posture: **Good**

The codebase demonstrates strong security-aware engineering: context propagation for graceful cancellation, explicit lock order contracts, agent output sanitization (`sanitizeAgentOutput`), recursion guards, output size caps (`cappedBuffer`), budget-aware cost guards, and a clear layering boundary that isolates vendor knowledge. The `secret-scan.mjs` provides a real, honest gate. The explicit acknowledgment of "honest-but-trusting" and "rather miss than mis-fire" design choices shows mature security decision-making.

### Top 3 Critical Issues

1. **Command injection surface via unvalidated `root` path** (Finding #1) — Multiple `git` commands use an unvalidated path parameter. While `exec.Command` prevents shell injection, git's option parsing and path traversal could leak information or alter behavior.

2. **Race window in memory load cache** (Finding #2) — The mtime-based cache can serve stale data under concurrent access, potentially causing incorrect convergence decisions in multi-process scenarios.

3. **World-readable runtime state files** (Finding #3) — Checkpoints and memory stores at `0o644` leak agent outputs, cost data, and convergence state to any local user.

### Top 3 Quick Wins

1. **Fix file permissions** (Finding #3) — Change `0o644` → `0o600`. One-line change in `writeSynced` and `Append`. **Effort: S**

2. **Validate root path before git commands** (Finding #1) — Add `os.Stat` + `filepath.Abs` before `computeCodeTestRatio` / `computeFileDelta`. **Effort: S**

3. **Harden overload classifier** (Finding #5) — Remove the textual `"overloaded"` match; rely solely on `api_error_status: 529`. **Effort: S**

### Security Debt

| Item | Effort | Priority |
|------|--------|----------|
| Integrity checksums on checkpoint/memory files (Finding #11) | M | Medium |
| Concurrency limiter for parallel agent spawns (Finding #6) | M | Medium |
| PID-based depth tracker (Finding #8) | M | Low |
| Template path traversal guard (Finding #4) | S | Medium |
| Memory store write authorization (Finding #12) | M | Low |
| Input size limit on workflow YAML (Finding #9) | S | Low |
| Strengthen convergence signal corroboration (Finding #7) | M | Low |

### Recommendations for the Product Security Model

1. **Document the trust boundary**: Agent outputs are "honest-but-trusting" — they can be fabricated by a compromised agent or agent card. Convergence gates relying on self-report (`RequirementConfidence`, `ReviewStatus`) should be treated as advisory.

2. **Defense in depth for fork-bomb prevention**: The env-based `FORGE_AGENT_DEPTH` is acknowledged as breakable. Add a complementary PID-counter approach and document `--max-agent-calls` / `--run-budget-usd` as the true cost controls.

3. **Supply-chain resilience**: Workflow YAML files, agent cards, and templates are all loaded from the repo. If any of these are attacker-controlled (malicious PR), the forge runtime can be coerced into path traversal, memory poisoning, or information disclosure. Consider pinning workflow schemas or validating them against a strict schema before loading.

4. **Incorporate into threat model update**: The parallel mode opens new DoS vectors (unbounded concurrent spawns) and new race conditions (lock order contract must be maintained as new mutexes are added). Make the lock order contract a required review item for any PR adding mutable shared state.
