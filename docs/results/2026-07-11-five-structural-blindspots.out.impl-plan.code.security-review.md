Now I have a thorough understanding of the codebase. Let me compile the comprehensive security review.

---

# Principal Security Engineer — ForgeOS Subsystem Security Review

**Review Date:** 2026-07-12
**Codebase:** ForgeOS (forge-core Go runtime + Node/Python harness)

---

## Executive Summary

ForgeOS is an orchestration runtime that drives AI agents (Claude) through declarative software development workflows. The codebase demonstrates **strong security awareness** — recursion guards, output caps, process-group isolation, fail-closed error handling, and a custom secret scanner are already in place. The architecture is a CLI tool (no network surface), so many traditional web-app categories (authN/authZ, session management, CSRF) are out of scope.

**Overall Security Posture: Needs Improvement** — the foundation is solid, but there are real gaps in symbolic-link handling, TOCTOU race conditions, denial-of-service through unthrottled git operations, and a dangerous trust boundary where emitted-file paths from workflows can read arbitrary files.

---

## Findings

### Finding #1: Path Traversal via Workflow-Declared Emit Paths

| Field | Value |
|-------|-------|
| **Category** | Input Validation / Data Protection |
| **Severity** | **High** |
| **Title** | Unrestricted file read through `emits` and `uses_template` paths |
| **Location** | `forge-core/cmd/forge/prompt_artifacts.go:38-40` and L74-76 |
| **Description** | `emitsContext()` and `templateContext()` resolve file paths relative to `repoRoot` and read their content into agent prompts. The `emits` array and `uses_template` value come directly from the workflow YAML asset (`asset.Phase.Emits`, `asset.Phase.UsesTemplate`). While workflows are version-controlled governance assets, a compromised workflow (or a malicious PR introducing a `.yml` change) can specify `emits: ["../../etc/shadow"]` to exfiltrate arbitrary files into the agent's context window, which then goes to the LLM provider. |
| **Attack Scenario** | An attacker with commit access to `.agent/workflows/*.yml` adds a phase with `emits: ["/etc/passwd", "../../.ssh/id_rsa"]`. The next `forge run` reads these files and injects their content into the prompt sent to the LLM API. |
| **Impact** | Exfiltration of sensitive files (SSH keys, cloud credentials, source code) to an external AI provider's API, violating data governance policies. |
| **Recommendation** | Implement a path-safety check that rejects paths containing `..`, symlinks escaping the repo root, or paths outside an allowed emit directory (e.g., `.agent/`, `docs/`). Use `filepath.Clean` + prefix check: |
| **Effort** | **S** (< 1 day) |

```go
// In emitsContext, before os.ReadFile:
func safePath(repoRoot, path string) (string, error) {
    if filepath.IsAbs(path) {
        return "", fmt.Errorf("absolute path not allowed: %s", path)
    }
    clean := filepath.Clean(filepath.Join(repoRoot, path))
    if !strings.HasPrefix(clean, filepath.Clean(repoRoot)+string(filepath.Separator)) {
        return "", fmt.Errorf("path escapes repo root: %s", path)
    }
    // Also resolve symlinks and re-check
    real, err := filepath.EvalSymlinks(clean)
    if err != nil {
        return "", fmt.Errorf("cannot resolve path: %w", err)
    }
    if !strings.HasPrefix(real, filepath.Clean(repoRoot)+string(filepath.Separator)) {
        return "", fmt.Errorf("symlink escapes repo root: %s", path)
    }
    return real, nil
}
```

---

### Finding #2: TOCTOU Race Condition on Approval/Rejection Markers

| Field | Value |
|-------|-------|
| **Category** | Authentication / Authorization |
| **Severity** | **High** |
| **Title** | Time-of-check/time-of-use on `.forge/*.approved` and `.forge/*.rejected` marker files |
| **Location** | `forge-core/cmd/forge/gates.go:172-178` (`humanApproved`), L249-257 (`resolveRejectionStartPhase`) |
| **Description** | `humanApproved()` uses `os.Stat()` to check for an approval marker, then the marker is consumed by a later `os.Remove()`. There is no file locking or atomic rename. An attacker with concurrent filesystem access can: (1) race to create a `.forge/*.approved` file between the Stat and the actual gate evaluation, or (2) race to re-create an approval marker after it was consumed, causing a double-approval. `resolveRejectionStartPhase` has a similar TOCTOU: it stats, checks, then removes — another process could race between the stat and the remove. |
| **Attack Scenario** | Process A runs `forge run` and the human_gate check fails (no approval). Process B creates `.forge/design.approved` 100µs after A's stat, allowing a cross-process approval bypass without the intended human decision. |
| **Impact** | Bypass of the human-gate approval control — the highest-leverage security gate in the system can be circumvented by a concurrent unprivileged process. |
| **Recommendation** | Use atomic marker creation combined with a lock file or exclusive file creation: |
| **Effort** | **S** (< 1 day) |

```go
// Atomic approval check using O_CREATE|O_EXCL to prevent races:
func checkAndClaimApproval(root, stage string) bool {
    path := approvalPath(root, stage)
    // Try to acquire a lock file atomically
    lock := path + ".lock"
    f, err := os.OpenFile(lock, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o644)
    if err != nil {
        return false // lock exists: someone else is processing
    }
    f.Close()
    defer os.Remove(lock)
    // Now safely check and remove the marker
    if _, err := os.Stat(path); err != nil {
        return false
    }
    return os.Remove(path) == nil
}
```

---

### Finding #3: Denial of Service via Unthrottled Git Operations

| Field | Value |
|-------|-------|
| **Category** | Threat Model (DoS) |
| **Severity** | **Medium** |
| **Title** | Unbounded `git diff` execution with no timeout or size guard |
| **Location** | `forge-core/cmd/forge/gates.go:340`, L417; `forge-core/cmd/forge/route.go:289` |
| **Description** | `computeCodeTestRatio`, `computeFileDelta`, and `gitChangedPaths` each execute `git diff` with no timeout, no output cap, and in the case of `computeFileDelta`, parse the entire output into memory (`strings.Fields`). A repo with a very large working tree or a massive diff (e.g., rebase of thousands of commits) could produce gigabytes of diff output, causing OOM or an unbounded hang. These functions are called per-iteration in `forge evolve`, so the waste multiplies. |
| **Attack Scenario** | An attacker contributes a large synthetic commit that touches 500,000 files. On the next `forge run`, every git diff call reads and parses the entire diff into memory. The orchestrator OOMs or hangs. |
| **Impact** | Denial of service — the forge run never completes; orchestrator process is killed by OOM killer; `.forge/` state may be corrupted if killed mid-write. |
| **Recommendation** | Add timeouts and output size bounds to all git commands: |
| **Effort** | **S** (< 1 day) |

```go
import "time"

func gitDiff(ctx context.Context, root string, args ...string) ([]byte, error) {
    ctx, cancel := context.WithTimeout(ctx, 30*time.Second)
    defer cancel()
    cmd := exec.CommandContext(ctx, "git", append([]string{"-C", root}, args...)...)
    out, err := cmd.Output()
    if err != nil {
        return nil, err
    }
    if len(out) > 10<<20 { // 10 MiB cap
        return nil, fmt.Errorf("git diff output exceeds 10 MiB (%d bytes)", len(out))
    }
    return out, nil
}
```

---

### Finding #4: Unvalidated Criterion Threshold — Denial of Service via NaN/Infinity

| Field | Value |
|-------|-------|
| **Category** | Input Validation |
| **Severity** | **Medium** |
| **Title** | `Threshold` float64 comparison does not guard against NaN/Inf |
| **Location** | `forge-core/internal/converge/converge.go:242-272` (`compare()`), `forge-core/internal/asset/asset.go:63` (`Threshold *float64`) |
| **Description** | The `compare()` function directly compares float64 values using standard operators. A `Threshold` value of `NaN` (which can legally appear in JSON as `NaN` in some serializers, or via `strconv.ParseFloat("nan")`) — or `+Inf`/`-Inf` — will cause undefined comparison behavior. Per IEEE 754, all comparisons with NaN return false, which means a NaN threshold could cause a criterion to be permanently unmet (denial of convergence) or, worse, certain edge cases with `==` against NaN could produce unexpected results. While `json.Unmarshal` rejects `NaN`/`Inf` in strict mode, the `Criterion.UnmarshalJSON` uses a relaxed path via an alias that may accept custom float values. |
| **Attack Scenario** | A manipulated workflow YAML contains `threshold: NaN`. The convergence check for `roadmap_completion >= NaN` always evaluates to false, preventing the run from ever converging (a DoS on the workflow). |
| **Impact** | Permanent denial of convergence — the loop runs indefinitely until max-iter budget is exhausted. |
| **Recommendation** | Add NaN/Inf guards in `compare()` and in `Criterion.UnmarshalJSON`: |
| **Effort** | **S** (< 1 day) |

```go
func compare(left float64, op string, right float64) bool {
    if math.IsNaN(left) || math.IsNaN(right) || math.IsInf(left, 0) || math.IsInf(right, 0) {
        return false // NaN/Inf cannot meaningfully satisfy a comparison
    }
    switch op { ... }
}

// Also add validation in UnmarshalJSON:
func (c *Criterion) UnmarshalJSON(data []byte) error {
    // ... existing code ...
    if a.Threshold != nil && (math.IsNaN(*a.Threshold) || math.IsInf(*a.Threshold, 0)) {
        return fmt.Errorf("asset: threshold must be a finite number, got %v", *a.Threshold)
    }
}
```

---

### Finding #5: Symlink Following in File Walking (Harness Scans)

| Field | Value |
|-------|-------|
| **Category** | Threat Model (Information Disclosure / Tampering) |
| **Severity** | **Medium** |
| **Title** | Secret scanner and arch scanner may follow symlinks outside the repo root |
| **Location** | `harness/secret-scan.mjs:125-133` (`walkFiles`), `harness/arch/scan.mjs` (if similar pattern) |
| **Description** | `walkFiles()` in `secret-scan.mjs` uses `statSync()` (not `lstatSync`) on each entry, which follows symbolic links. If a symlink pointing to `/etc/` or another sensitive directory exists in the repo, the scanner will traverse into it, potentially scanning (and exposing in findings) files outside the intended scope. The `SKIP_DIRS` set does not guard against symlink-to-parent-directory attacks (`ln -s / symlink_etc`). |
| **Attack Scenario** | An attacker commits a symlink: `ln -s /etc .forge/../../../etc_hack`. The secret scanner follows it and reads `/etc/shadow` contents. If the scanner produces `--json` output, this data is now in the scanner's findings output, which could be printed or logged. |
| **Impact** | Information disclosure — system files outside the repo are read by the scanner and potentially exposed in CLI output or CI logs. |
| **Recommendation** | Replace `statSync` with `lstatSync` and explicitly refuse to follow symlinks, or verify the resolved path stays within the repo root: |
| **Effort** | **S** (< 1 day) |

```javascript
export function walkFiles(root, acc = []) {
  let entries;
  try { entries = readdirSync(root); } catch { return acc; }
  for (const name of entries) {
    if (SKIP_DIRS.has(name)) continue;
    const full = join(root, name);
    let st;
    try { st = lstatSync(full); } catch { continue; } // Use lstatSync
    if (st.isSymbolicLink()) continue; // Skip symlinks entirely
    if (st.isDirectory()) walkFiles(full, acc);
    else if (scannableName(full)) acc.push(full);
  }
  return acc;
}
```

---

### Finding #6: Agent Environment Variable Inheritance Transmits Entire Env Block

| Field | Value |
|-------|-------|
| **Category** | Data Protection |
| **Severity** | **Medium** |
| **Title** | CLI agent inherits the full parent environment including secrets |
| **Location** | `forge-core/internal/orchestrator/command_executor.go:295-299` (`childEnv`) |
| **Description** | `childEnv()` copies the entire parent process environment via `os.Environ()` and passes it to the spawned agent command. This includes `FORGE_AGENT_DEPTH`, but also all other environment variables — cloud provider credentials (`AWS_ACCESS_KEY_ID`, `GOOGLE_APPLICATION_CREDENTIALS`), API keys, database URLs, SSH agent sockets, and any other secrets present in forge's environment. The agent (claude) inherits these verbatim, and any tool execution (bash/git/test) within the agent inherits them too. While this is necessary for many dev workflows (the agent needs git credentials), there is no mechanism to scrub sensitive vars before passing to an untrusted agent task. |
| **Attack Scenario** | A compromised or malicious workflow defines an agent phase that runs `env` or `printenv` and exfiltrates the output. All cloud provider credentials, API tokens, and secrets from the parent environment are leaked. |
| **Impact** | Complete credential compromise — cloud provider access, database access, API keys all leaked to an attacker. |
| **Recommendation** | Implement an allowlist/blocklist for environment variable inheritance: |
| **Effort** | **M** (1-3 days) |

```go
// In childEnv, filter sensitive variables:
var sensitiveEnvPrefixes = []string{"AWS_", "AZURE_", "GOOGLE_", "GCP_", "DB_", "DATABASE_", "SECRET_", "TOKEN_", "PASSWORD", "PRIVATE_KEY", "SSH_"}

func isSensitiveVar(key string) bool {
    upper := strings.ToUpper(key)
    for _, prefix := range sensitiveEnvPrefixes {
        if strings.HasPrefix(upper, prefix) {
            return true
        }
    }
    return false
}

func childEnv(depth int, allowKeys []string) []string {
    prefix := agentDepthEnv + "="
    base := os.Environ()
    out := make([]string, 0, len(base)+1)
    for _, kv := range base {
        eq := strings.IndexByte(kv, '=')
        if eq < 0 { continue }
        key := kv[:eq]
        if isSensitiveVar(key) { continue } // block sensitive
        if !strings.HasPrefix(kv, prefix) {
            out = append(out, kv)
        }
    }
    return append(out, fmt.Sprintf("%s=%d", agentDepthEnv, depth+1))
}
```

---

### Finding #7: No Visibility into Emitted-Data Flow to LLM Provider

| Field | Value |
|-------|-------|
| **Category** | Data Protection / Compliance |
| **Severity** | **Medium** |
| **Title** | Unauditable data sent to LLM API — files, memory, gate results all go to external provider |
| **Location** | `forge-core/cmd/forge/prompt_context.go`, `forge-core/cmd/forge/prompt_artifacts.go` (entire prompt assembly) |
| **Description** | The entire assembled prompt — including ROADMAP content, ADR files, prior phase outputs, emitted artifacts, hard constraints from AGENTS.md, and gate results — is sent to the LLM provider (Anthropic's Claude API) as a single prompt. There is **no data-classification layer**, **no redaction**, **no audit trail** of what was sent. A developer working on a proprietary codebase may inadvertently include files containing PII, trade secrets, or credentials in the context window, and there is no mechanism to detect, log, or block this before transmission. The `observe` callback captures output, but there is no equivalent for input content. |
| **Attack Scenario** | An engineer has a `secrets.env` file (accidentally git-ignored but present) in the repo. The workflow's prompt reads `./.agent/ROADMAP.md` as usual, and the agent card instructs the LLM to "read the project context" — but a tool call or phase emit accidentally reads the `.env` file. This data is transmitted to Anthropic's API and retained per their privacy policy. |
| **Impact** | Data leakage of sensitive information to third-party AI provider; potential GDPR/PII compliance violation; breach of corporate IP protection policies. |
| **Recommendation** | Implement a **Data Classification Layer** that scans prompt content before sending: |
| **Effort** | **L** (> 3 days) |

```
1. Add a `--data-classification` flag: public | internal | confidential | restricted
2. Implement a pre-flight scan of assembled prompt content against secret patterns
   (reusing the existing secret-scan.mjs patterns)
3. Log a manifest of {phase, file_sources, token_count, classification} before each API call
4. Block transmission of "restricted" classified data unless explicitly overridden
5. Add a `--prompt-audit-log` option that writes each prompt and classification verdict to a file
```

---

### Finding #8: Agent Command-Time Argument Injection via Workflow Phase Name

| Field | Value |
|-------|-------|
| **Category** | Input Validation |
| **Severity** | **Low** |
| **Title** | Phase name included in log formatting, not in argv, but agent output could be misconstrued |
| **Location** | `forge-core/internal/orchestrator/command_executor.go:215` — `c.logf("phase %s: ran %q -> %s")` |
| **Description** | The `finish()` method logs the full argv using `strings.Join(argv, " ")`. While `exec.Command` prevents shell injection (no shell involved), the log entries could be ingested by a log aggregator that parses them as structured data. A phase name containing newlines or special characters (e.g., `planner\n2026-07-12 00:00:00 [ERROR] forge: invalid token`) could inject fake log entries. Phase names come from the workflow YAML, which is version-controlled, so the practical risk is low. |
| **Attack Scenario** | A workflow YAML contains a phase named `implementer\n[ERROR] forge-runner: authentication failure`. Log aggregators parsing the forge output see a fake error entry and trigger alerting. |
| **Impact** | Low — log injection into a CLI tool's stderr/stdout; unlikely to cause automation failure unless raw forge output is piped into a structured parser. |
| **Recommendation** | Sanitize phase names in log output by escaping control characters: |
| **Effort** | **S** (< 1 day) |

```go
func sanitizeForLog(s string) string {
    return strings.Map(func(r rune) rune {
        if r < 0x20 && r != '\n' && r != '\t' { return -1 }
        return r
    }, s)
}
```

---

### Finding #9: Hardcoded Paths and Missing Root Enforcement in Harness Scripts

| Field | Value |
|-------|-------|
| **Category** | Input Validation / Threat Model |
| **Severity** | **Low** |
| **Title** | Harness scripts derive root from script location, enabling CWD-based path confusion |
| **Location** | `harness/secret-scan.mjs:53-55` (`ROOT` = `dirname(HARNESS_DIR)`), `harness/acceptance.mjs` root discovery |
| **Description** | `secret-scan.mjs` sets `ROOT` to the parent of the harness directory (i.e., the repo root) using `dirname(fileURLToPath(import.meta.url))`. This means the scanner's scope is determined by where the script is installed, not by the `--root` flag an operator might pass. If someone runs `node /path/to/forge/harness/secret-scan.mjs` from an unrelated directory, the scanner scans the forge installation directory, not the intended target. Similarly, the `SKIP_DIRS` and `SCAN_EXTS` skip `.forge/` but do not enforce that the scan stays within an explicitly-provided root boundary. |
| **Attack Scenario** | A CI pipeline misconfiguration runs `cd /tmp && node /opt/forge/harness/secret-scan.mjs` — this scans `/opt/forge/` instead of `/tmp/`, potentially missing secrets in the actual target codebase. Or an operator runs it from `/etc/` and the scanner traverses sensitive system directories. |
| **Impact** | Low — mis-scoping reduces the scanner's effectiveness. This is an honest-proxy limitation: the scanner scans what it's pointed at. |
| **Recommendation** | Accept an explicit `--root` argument and validate the resolved path is a trusted directory: |
| **Effort** | **S** (< 1 day) |

```javascript
// In main(), add argument parsing:
function main() {
  const args = process.argv.slice(2);
  const jsonFlag = args.includes('--json');
  const rootIdx = args.indexOf('--root');
  let scanRoot = ROOT;
  if (rootIdx >= 0 && rootIdx + 1 < args.length) {
    scanRoot = resolve(args[rootIdx + 1]);
    if (!statSync(scanRoot).isDirectory()) {
      console.error(`forge-secret-scan: --root ${scanRoot} is not a directory`);
      process.exit(2);
    }
  }
  const report = scanRepo(scanRoot);
  // ...
}
```

---

### Finding #10: World-Readable State Files in `.forge/`

| Field | Value |
|-------|-------|
| **Category** | Data Protection |
| **Severity** | **Low** |
| **Title** | Checkpoint, trace, memory, and approval markers created with world-readable permissions |
| **Location** | `forge-core/internal/memory/memory.go:199` (`os.OpenFile(..., 0o644)`), `forge-core/internal/persist/checkpoint.go:159` (`0o644`), `forge-core/cmd/forge/evolve.go:485` (`0o644`), `forge-core/cmd/forge/migrate.go:174` (`0o644`) |
| **Description** | All files in `.forge/` — checkpoints, trace logs, memory store, approval markers — are created with mode `0o644` (world-readable). The trace log (`trace.jsonl`) records every phase's agent output verbatim, which may contain proprietary source code, business logic, or secrets generated by the agent. On multi-tenant systems (CI runners, shared dev servers), any user on the same machine can read these files. |
| **Attack Scenario** | In a CI environment where multiple pipelines share a build node, User A's `forge evolve` run creates `trace.jsonl` in the shared workspace with mode `0o644`. User B reads the trace file and extracts proprietary source code generated by the AI agent. |
| **Impact** | Information disclosure of proprietary AI-generated code and internal business logic to other users on the same system. |
| **Recommendation** | Create state files with `0o600` (owner-only read/write) and the parent `.forge/` directory with `0o700`: |

```go
// In forgeDir or wherever .forge/ is created:
func ensureForgeDir(root string) error {
    path := forgeDir(root)
    if err := os.MkdirAll(path, 0o700); err != nil {
        return err
    }
    // Ensure the directory retains 0700 even if it already existed
    return os.Chmod(path, 0o700)
}

// In all file creation sites: use 0o600 for files
f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0o600)
```

---

### Finding #11: Scorecard File Handling Without Integrity Check

| Field | Value |
|-------|-------|
| **Category** | Threat Model (Tampering) |
| **Severity** | **Low** |
| **Title** | Scorecards JSON is loaded and trusted without signature or integrity verification |
| **Location** | `forge-core/internal/routing/scorecard.go:81` (`os.ReadFile(path)`) |
| **Description** | `LoadScorecards()` reads a JSON file from `.agent/routing/scorecards.json` and uses its data to influence model-tier routing decisions (via `HistoryTiebreak`). There is no integrity check, digital signature, or cryptographic verification. An attacker who can write to this file can manipulate which model tier is chosen for which task type — potentially downgrading security reviewers to cheaper models or upgrading low-risk tasks to expensive models (budget exhaustion attack). The file is in the version-controlled `.agent/` directory, so in normal operation it is subject to git integrity, but a compromised local checkout or CI workspace bypasses this. |
| **Attack Scenario** | An attacker with write access to the repo's `.agent/routing/scorecards.json` modifies the quality scores: a cheap model (Haiku) is recorded as having 100% quality on security tasks. `HistoryTiebreak` selects Haiku for the security reviewer, bypassing the Opus safety floor intended for security-critical work. |
| **Impact** | Low in practice (the file is version-controlled, and safety floors via `IsOpusFloorAgent` override history), but undermines the integrity of the learning-loop routing system. |
| **Recommendation** | Add JSON schema validation on load and log a warning on unexpected values: |

```go
// Validate scorecard structure on load
func validateScorecard(cards map[string][]ScorecardEntry) error {
    for taskType, entries := range cards {
        for _, e := range entries {
            if e.QualityScore < 0 || e.QualityScore > 1 {
                return fmt.Errorf("scorecard: invalid quality_score %f for %s/%s", e.QualityScore, taskType, e.Model)
            }
            if e.Samples < 0 {
                return fmt.Errorf("scorecard: negative samples for %s/%s", taskType, e.Model)
            }
        }
    }
    return nil
}
```

---

### Finding #12: Go Native YAML Parser — No Billion Laughs / Decompression Bomb Protection

| Field | Value |
|-------|-------|
| **Category** | Input Validation / Denial of Service |
| **Severity** | **Medium** |
| **Title** | Custom YAML parser can consume unbounded memory on deeply nested or alias-referencing input |
| **Location** | `forge-core/internal/yaml2json/` (entire package) |
| **Description** | The Go-native YAML parser (`yaml2json`) is a custom recursive-descent parser that handles the YAML subset used by ForgeOS configs. It does not support YAML anchors/aliases (`&`/`*`), which is the vector for Billion Laughs attacks. However, it does handle deeply nested maps and sequences recursively. A carefully crafted YAML file with many levels of nested indentation (e.g., 100,000 levels) could cause stack overflow or memory exhaustion. Additionally, the `normalizeLines` function calls `strings.Split` up-front, loading the entire file into memory — a 2GB workflow YAML would consume 2GB+ of RAM. |
| **Attack Scenario** | An attacker commits a workflow YAML with 50,000 levels of nested maps (`a: {b: {c: ...}}`) via inline syntax. The Go parser's recursive `parseMapping`/`parseSequence` functions overflow the call stack and crash the forge process, or consume all available memory. |
| **Impact** | Denial of Service — forge crashes on any workflow phase that requires parsing the malicious YAML file. |
| **Recommendation** | Add recursion depth and input size limits: |

```go
const maxRecursionDepth = 1000
const maxInputBytes = 100 << 20 // 100 MiB

func Decode(r io.Reader) (any, error) {
    data, err := io.ReadAll(r)
    if err != nil { return nil, err }
    if len(data) > maxInputBytes {
        return nil, fmt.Errorf("yaml2json: input exceeds %d bytes", maxInputBytes)
    }
    lines := normalizeLines(string(data))
    return parseDocument(lines, 0, 0) // pass depth counter
}

func parseDocument(lines []line, pos, depth int) (any, int, error) {
    if depth > maxRecursionDepth {
        return nil, 0, fmt.Errorf("yaml2json: max recursion depth exceeded")
    }
    // ... rest of parsing, increment depth on recursive calls
}
```

---

### Finding #13: Acceptance Probe Leaks the Full Command Output in Gate Error Messages

| Field | Value |
|-------|-------|
| **Category** | Data Protection / Information Disclosure |
| **Severity** | **Low** |
| **Title** | Gate failure messages include the full combined stdout+stderr of failed probes |
| **Location** | `forge-core/internal/gate/gate.go:53-59` (`run()`), `forge-core/internal/orchestrator/orchestrator.go:250` |
| **Description** | When a gate fails, the error message returned to the orchestrator includes `cmd.CombinedOutput()` verbatim. This output could contain source code, error messages with file paths, environment variable dumps, or other sensitive data. The error message is then logged and potentially shown in the CLI output. For example, if `node harness/acceptance.mjs` crashes with a stack trace containing file paths or test code, that entire output is included in the error message that propagates up. |
| **Attack Scenario** | A test file contains a hardcoded credential (needed for testing an API integration). The test fails, and the combined output (including the credential) is printed in the forge gate error message, visible in CI logs. |
| **Impact** | Low — credential exposure in error messages; output is already generated by running tests that should be clean. |
| **Recommendation** | Truncate gate output in error messages and add a `--verbose` flag for full output: |

```go
const maxGateOutputInError = 1024 // 1KiB

func newResult(name string, ok bool, output string) Result {
    output = strings.TrimSpace(output)
    if !ok && len(output) > maxGateOutputInError {
        output = output[:maxGateOutputInError] + fmt.Sprintf("\n…[output truncated to %d bytes]", maxGateOutputInError)
    }
    ...
}
```

---

## STRIDE Threat Model Summary

| Threat | Applicable? | Key Risks |
|--------|-------------|-----------|
| **S**poofing | ✅ Limited | Approval markers can be written by any local user; no cryptographic identity for human approvals |
| **T**ampering | ✅ Yes | Scorecards, workflow YAML, approval markers all modifiable by local users; no integrity verification |
| **R**epudiation | ✅ Partial | Trace logs capture agent decisions, but no immutable audit trail exists; no signing of key operations |
| **I**nformation Disclosure | ✅ Yes | Path traversal in emits (Finding #1), symlink following in scanners (Finding #5), env var leakage to agents (Finding #6), world-readable trace files (Finding #10) |
| **D**enial of Service | ✅ Yes | Unbounded git diff (Finding #3), NaN threshold (Finding #4), YAML bomb (Finding #12), recursive agent depth is bounded (good) |
| **E**levation of Privilege | ✅ Limited | TOCTOU in approval markers (Finding #2) could allow unauthorized workflow progression |

---

## OWASP Top 10 Mapping

| OWASP Category | Covered? | Notes |
|----------------|----------|-------|
| A01: Broken Access Control | ⚠️ Partial | Approval markers are the main access control; TOCTOU weakness (Finding #2) |
| A02: Cryptographic Failures | ✅ N/A | No custom crypto; relies on OS file permissions |
| A03: Injection | ⚠️ Partial | `exec.Command` prevents shell injection; path traversal in emits (Finding #1) |
| A04: Insecure Design | ⚠️ Partial | Agent env inheritance (Finding #6) is a design-level concern |
| A05: Security Misconfiguration | ⚠️ Some | World-readable state files (Finding #10) |
| A06: Vulnerable Components | ✅ N/A | Zero external deps in Go core; PyYAML via safe_load |
| A07: Identification/Auth Failures | ⚠️ Partial | Weak approval mechanism (Finding #2) |
| A08: Software/Data Integrity | ⚠️ Partial | Unverified scorecards (Finding #11) |
| A09: Security Logging/Monitoring | ⚠️ Weak | No input audit trail (Finding #7) |
| A10: SSRF | ✅ N/A | No outbound connections from forge-core |

---

## Final Summary

### Overall Security Posture: **Needs Improvement**

The codebase shows clear security engineering maturity — recursion guards, output caps, process-group isolation, fail-closed semantics, and a custom secret scanner are all present and well-designed. However, several real vulnerabilities exist in the trust boundaries between workflow assets and the execution environment.

### Top 3 Critical Issues

1. **Path Traversal in Emit/Template File Reads (High)** — An attacker who controls a workflow YAML can read arbitrary files from the filesystem and inject them into the LLM prompt. This is the highest-risk finding because it bypasses all existing data governance.
2. **TOCTOU Race on Approval Markers (High)** — The human-gate approval mechanism — the most important security control in the system — is vulnerable to race conditions that could allow a bypass.
3. **Agent Inherits Full Environment (Medium)** — The entire parent environment, including cloud credentials, is passed to the agent CLI. A compromised workflow phase can exfiltrate all of them.

### Top 3 Quick Wins

1. **Path traversal fix in `emitsContext()`** (S effort) — Add `filepath.Clean` + prefix check + symlink resolution. Single afternoon fix.
2. **Add timeouts to all `git diff` calls** (S effort) — 30-second context deadline on git commands prevents DoS from large working trees.
3. **Set `.forge/*` file permissions to `0600`** (S effort) — Simple mode change on all file creation sites; big privacy win for shared systems.

### Security Debt

| Item | Priority | Description |
|------|----------|-------------|
| Data classification layer for prompts | High | No mechanism to classify or redact data before sending to LLM API |
| Immutable audit trail | Medium | Trace logs are append-only files with no signing; a local user can modify them |
| Cryptographic identity for approvals | Medium | Approval markers are files on disk; no cryptographic proof of who approved |
| Dependency vulnerability scanning (SCA) | Low | Listed as v3 roadmap item; only pattern-based secret scanning exists today |
| Sandboxed agent execution | Low | `SandboxConfig` is a skeleton; agents run directly on the host |
| Fuzzing for Go YAML parser | Low | Custom parser handles a nontrivial subset; no fuzz tests exist |
