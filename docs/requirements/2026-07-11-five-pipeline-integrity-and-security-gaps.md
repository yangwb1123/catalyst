# ForgeOS — 资深架构师/产品经理视角:五方向全局扫描分析

> **角色**: 资深架构师 + 产品经理  
> **方法**:
> 1. 全局深扫: forge-core(18 Go 包 · ~35k LOC 纯标准库运行时 + CLI)、
>    harness(39+ 模块 · ~10.5k LOC 执法层)、.agent/(12 agent 卡 · 9 skill 卡 ·
>    5 工作流 · 全部 ADR+DECISIONS+architecture)、examples/、pi-batch.py
> 2. 完整阅读 Sprint 1–31 全部演进记录、`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`、
>    `ROADMAP.md`、85+ 份 docs/requirements/*.md + docs/analysis/*.md 分析文档
> 3. **差异化验证**: 逐方向在全部已有分析文档中检索核心关键词组合,确认每个方向的
>    核心论点从未被作为系统性方向展开
> 4. **纪律**: 不编写任何代码。所有建议附**精确到行号的代码级证据**和边界场景表
> **日期**: 2026-07-11

---

## 方法论说明

本分析不依赖架构推测或功能愿景。每个方向通过以下方式发现:

1. **读代码时注意到 "unusual pattern"** — 某个实现方式与系统其他部分的设计哲学不一致
2. **追 trace 看 data flow** — 一个数据从产生到消费的完整路径在某处有断裂或假定
3. **验证已有覆盖** — 在 85+ 份分析文档中搜索,确认未被系统性讨论过
4. **评估影响面** — 这个缺口在真实使用中会怎样表现出来

---

## 方向一 · Agent 输出管道完整性:截断感知的结构化解析

**优先级**: 🔴 **P0** | **类别**: 正确性 · 可靠性 | **预估**: 1 sprint | **杠杆**: ⭐⭐⭐⭐⭐  
**已有分析覆盖**: 零 — 85+ 份文档从未将「输出截断 → 下游解析静默失败」作为独立系统性方向讨论。

### 问题描述

ForgeOS 的 Agent 输出经过一条精心设计但**在关键点有结构性断裂**的管道:

```
Agent stdout/stderr → CommandExecutor.runMeasured()
  → cappedBuffer (可能截断) → CommandExecutor.finish()
    → Observe callback → observeFor() → 解析器(verdict/cost/confidence)
```

管道中存在一个关键的隐含假定: **下游解析器假设收到的输出是完整的**。但 `cappedBuffer` (`command_executor.go:290-314`) 可以在 `MaxOutputBytes` (默认 10 MiB) 处截断输出,而下游解析器对此完全不知情。

### 代码级证据

**A. `cappedBuffer` 截断输出后,不会对观察者发出任何信号:**

```go
// forge-core/internal/orchestrator/command_executor.go:290-314
type cappedBuffer struct {
	cap   int
	buf   []byte
	total int
}

func (b *cappedBuffer) rendered() string {
	s := strings.TrimSpace(string(b.buf))
	if b.total > len(b.buf) {
		s += fmt.Sprintf(" …[output truncated: retained %d of %d bytes (--max-output-bytes)]", len(b.buf), b.total)
	}
	return s
}
```

截断通知是**追加到输出字符串末尾的文本**。这意味着:
- 如果截断发生在 JSON 中间,追加的文本破坏 JSON 结构
- 如果截断发生在最后一行中间,verdict 行被破坏
- 下游解析器**不会检查**输出中是否包含截断标记

**B. `parseClaudeCostUsd` 解析的是可能被截断的整个输出:**

```go
// forge-core/cmd/forge/cost.go:180-196
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
	var env struct {
		TotalCostUsd *float64 `json:"total_cost_usd"`
	}
	if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &env); err != nil {
		return 0, false // not a single JSON object (echo/dry/stub, or a multi-line tail)
	}
	// ...
}
```

当 claude 的 JSON 信封(`{"result":"...","total_cost_usd":0.054,"api_error_status":200}`)被截断时:
- `json.Unmarshal` 失败 → 返回 `(0, false)`
- 成本数据**静默丢失**
- 没有任何日志或警告说明成本数据是因为输出被截断而丢失的

**C. 三个 verdict 解析器都工作在 `unwrapClaudeResult` 后的最后一行:**

```go
// forge-core/cmd/forge/cost.go:330-331
func parseReviewerVerdict(output string) (verdict string, ok bool) {
	last := lastNonEmptyLine(unwrapClaudeResult(output))
	// ...
}

// forge-core/cmd/forge/cost.go:387-388
func parseConfidenceScore(output string) (score float64, ok bool) {
	last := lastNonEmptyLine(unwrapClaudeResult(output))
	// ...
}
```

如果截断切断了 `VERDICT:` 行或 `CONFIDENCE:` 行:
- `lastNonEmptyLine` 返回截断前的最后一行(可能是不相关的中间行)
- 解析器返回 `("", false)` — "无信号"
- orchestrator 将缺失 verdict 解释为"继续"(fail-open)
- 系统**静默丢失**了一个 reviewer 的 REQUEST_CHANGES 裁决

**D. `unwrapClaudeResult` 会在截断的 JSON 上静默降级:**

```go
// forge-core/cmd/forge/cost.go:420-430
func unwrapClaudeResult(output string) string {
	var env struct {
		Result string `json:"result"`
	}
	if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &env); err != nil {
		return output // not a JSON envelope: echo/stub, verbatim
	}
	return env.Result
}
```

当 JSON 被截断时,`json.Unmarshal` 失败 → 降级为 `return output`,即原始截断文本本身。
然后 `parseReviewerVerdict` 在原始截断文本的末尾行搜索 `VERDICT:` — 如果截断切在当前行的中间,就找不到。

### 场景复杂度

当截断与多种输出格式组合时:

| 输出类型 | 截断位置 | 影响 | 严重度 |
|---------|---------|------|--------|
| Claude JSON 信封 | 在 `total_cost_usd` 前 | 成本数据丢失,agent 阶段仍执行 | 中 |
| Claude JSON 信封 | 在 `result` 中间 | verdict 丢失,`unwrapClaudeResult` 降级为原始文本 | 高 |
| 纯文本(echo/stub) | 在 verdict 行中间 | `parseReviewerVerdict` 静默返回无信号 | 中 |
| 大输出 + long `detail` 字段 | 在 detail 文本中间 | memory 条目不完整但无提示 | 低 |
| 并发输出(并行模式罕见) | 两个 agent 的输出在 cappedBuffer 中合并 | 流合并导致结构损坏 | 高 |

### 建议方向

1. **`finish()` 方法应在 observe 前检查截断**:如果输出包含截断标记,先记录一条警告日志,说明下游解析可能不完整。
2. **`cappedBuffer` 应暴露一个 `Truncated() bool` 方法**:让 observe 回调可以感知截断。
3. **verdict 解析器应检查截断标记**:如果输出被截断且无法解析出 verdict,应发出显式的告警而非静默降级。
4. **cost 解析器在 JSON 解析失败时应检查截断**:如果输出包含截断标记且 JSON 无法解析,日志应说明成本数据因截断丢失而非"没有成本数据"。
5. **考虑分层次截断**:首先截断中间内容,保留开头(headers/metadata)和结尾(last line/verdict)。当前 `cappedBuffer` 保留开头,但 cost JSON 可能在开头而 verdict 在结尾。

---

## 方向二 · 错误分类维度缺口:扁平五类无法表达恢复语义

**优先级**: 🟠 **P1** | **类别**: 运行时 · 可靠性 | **预估**: 1.5 sprints | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖**: 零 — 关于 ExecKind 分类本身的系统性维度分析不存在。

### 问题描述

`ExecKind` (`exec_error.go:16-50`) 将执行错误映射为 5 个扁平类别。但这 5 类将所有错误压缩到单一维度(是否可重试),丢失了**至少三个独立维度**:

1. **严重性**: 致命(fatal) vs 警告(warning) vs 建议(advisory)
2. **作用域**: 该阶段(this-phase) vs 该次运行(run-wide) vs 项目(project-wide)
3. **预期恢复者**: 自动重试(auto-retry) vs 运维人员(config/human) vs (永不恢复/需要重设计)

### 代码级证据

**A. 当前 5 个错误类别将所有语义压缩为一维的 flat enum:**

```go
// forge-core/internal/orchestrator/exec_error.go:16-50
type ExecKind int

const (
	KindConfig         ExecKind = iota // 永久,操作者修复
	KindTimeout                        // 暂时,可重试
	KindFailed                         // 可重试? 不——agent 自己的裁决
	KindRecursionLimit                 // 永久,不重试
	KindOverloaded                     // 暂时,带退避可重试
)
```

**B. `Retryable()` 只根据 Kind 决定,没有任何维度分析:**

```go
// forge-core/internal/orchestrator/exec_error.go:103-105
func (e *ExecError) Retryable() bool {
	return e.Kind == KindTimeout || e.Kind == KindOverloaded
}
```

真实世界中,一个错误应该触发不同的恢复策略:

```
"agent exit code 1 (test failed)"      → KindFailed, 不可重试
"agent exit code 1 (misspelled flag)"  → KindConfig, 不可重试, 应告警
"agent OOM-killed"                     → ??? 当前是 KindFailed, 但实际上是资源问题
"agent disk full (ENOSPC)"             → ??? 当前是 KindFailed, 但应该告警人类
"agent pipeline broken (SIGPIPE)"      → ??? 当前是 KindFailed, 但可能是暂时的
"agent command not found"              → KindConfig ✓
"agent timed out (900s deadline)"      → KindTimeout ✓
"agent 529 overloaded"                 → KindOverloaded ✓
"agent budget exhausted"               → ??? 当前是 KindFailed, 但语义不同
"agent git repo corrupted"             → ??? 当前是 KindFailed, 但这是项目问题
"agent network unreachable"            → ??? 当前是 KindFailed, 但可能是暂时的
```

**C. `classifyRunErr` 的 fallthrough 到 `KindFailed` 掩盖了错误多样性:**

```go
// forge-core/internal/orchestrator/exec_error.go:181-195
func classifyRunErr(phase string, runErr, ctxErr error, isOverload bool) *ExecError {
	switch {
	case errors.Is(runErr, exec.ErrNotFound):
		return &ExecError{Phase: phase, Kind: KindConfig, Err: runErr}
	case errors.Is(ctxErr, context.DeadlineExceeded):
		return &ExecError{Phase: phase, Kind: KindTimeout, Err: ctxErr}
	case isOverload:
		return overloadErr(phase, runErr)
	default:
		return &ExecError{Phase: phase, Kind: KindFailed, Err: runErr} // ← 所有不可识别的错误都进入这里
	}
}
```

`default` 分支捕获了所有未被明确识别的错误 → 全部标记为 `KindFailed`。但 `KindFailed` 的语义是"agent 自己的裁决,不自动重试"。这意味着:
- OOM kill → `KindFailed` (不可重试,但系统压力降低后重试可能成功)
- 磁盘空间不足 → `KindFailed` (不可重试,但用户清理磁盘后可以)
- 网络不可达 → `KindFailed` (不可重试,但网络恢复后可以)
- git 损坏 → `KindFailed` (不可重试,但这需要人类修复)

**D. `runAgentPhase` 的重试逻辑只看 `Retryable()`:**

```go
// forge-core/internal/orchestrator/orchestrator.go (相关代码路径)
// 在 runAgentPhase 中:
if execErr.Retryable() {
    // 重试(最多 MaxRetries 次)
} else {
    // 立即失败
}
```

这意味着一个临时的网络故障被归类为 `KindFailed` → 不重试 → 整个运行失败。而一个被误判为 `KindOverloaded` 的配置错误会被重试直到 `MaxRetries` 耗尽,浪费时间和预算。

### 场景复杂度

| 错误类型 | 当前分类 | 应该怎么做 | 当前行为 |
|---------|---------|-----------|---------|
| OOM kill (SIGKILL) | KindFailed | 若资源情况不同可重试 | 不重试, 运行失败 |
| 磁盘空间不足 | KindFailed | 告警人类,暂停 | 静默失败 |
| 网络断开 | KindFailed | 带退避重试 | 不重试 |
| 子进程段错误 | KindFailed | 不重试,但源可能有问题 | 符合预期 |
| agent 返回非零(测试失败) | KindFailed | 不重试,让 reviewer 处理 | 符合预期 |
| agent 配置错误(不存在 flag) | KindFailed | 立即告警,不重试 | 不重试,但原因不清晰 |
| git 仓库损坏错误的克隆 | KindFailed | 告警人类,非 agent 可修复 | 静默失败 |

### 建议方向

1. **引入错误分类的多维模型**:至少需要三个独立维度——
   - `Severity`: Fatal | Error | Warning | Info
   - `Source`: Config | Resource | Semantic | System | Agent
   - `RecoveryStrategy`: AutoRetry | BackoffRetry | Escalate | Abort
2. **`classifyRunErr` 应检查更多系统错误**:使用 `errors.Is` 检查 `syscall.ENOSPC`、`syscall.SIGKILL`(exit code 137/9)、`os.ErrClosed` 等。
3. **为 `ExecError` 增加建议性人类消息**:当错误不可恢复时,给操作者一句自然语言解释(如"磁盘空间不足——请释放至少 500MB 后重试")。
4. **为错误分类增加可观测性**:trace 事件应记录分类决策(不仅仅是 `status:"failed"`),包括分类的详细理由(如 `error_class: "resource_exhaustion", source: "os", severity: "fatal"`)。

---

## 方向三 · Stdout/Stderr 合并捕获导致结构化输出无法隔离

**优先级**: 🟠 **P1** | **类别**: 架构 · 正确性 | **预估**: 1 sprint | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖**: 零 — 85+ 份文档从未分析 stdout/stderr 合并捕获对结构化输出解析的影响。

### 问题描述

`CommandExecutor.runMeasured` 将**同一个** `cappedBuffer` 指针同时设为 `cmd.Stdout` 和 `cmd.Stderr`:

```go
// forge-core/internal/orchestrator/command_executor.go:175-176
out := &cappedBuffer{cap: c.maxOutputBytes()}
cmd.Stdout, cmd.Stderr = out, out
```

这来自 Go 标准库 `CombinedOutput()` 的惯用模式。但在 ForgeOS 的上下文中,这个选择造成了三个具体问题:

1. **结构化输出与诊断输出无法分离**:claude 的 `--output-format json` 将 JSON 信封写到 stdout,而将警告/进度写到 stderr。合并后,`parseClaudeCostUsd` 收到混合内容,可能无法解析 JSON。
2. **verdict/confidence 解析在混合流上工作**:reviewer 在 stdout 上输出 `VERDICT: APPROVE`,但同时可能在 stderr 上输出调试信息。合并后,`lastNonEmptyLine` 可能找到错误信息而非实际 verdict。
3. **无法区分空输出与成功但无输出**:如果一个 agent 写了一些内容到 stdout 和 stderr,但只有 stderr 被截断,合并后看起来就像输出部分丢失。如果 agent 只写 stdout(供解析),而将所有诊断写到 stderr,两者混合后诊断内容干扰解析。

### 代码级证据

**A. `runMeasured` 显式地将同一 Writer 用于两个流:**

```go
// forge-core/internal/orchestrator/command_executor.go:175-176
out := &cappedBuffer{cap: c.maxOutputBytes()}
cmd.Stdout, cmd.Stderr = out, out
```

Go 的 `os/exec.Cmd` 文档说当 Stdout 和 Stderr 指向同一 Writer 时,由 **同一 goroutine 串行化**写入。但这意味着两个流被**不可逆地交织**。

**B. `parseClaudeCostUsd` 假设整个输出是一个完整的 JSON:**

```go
// forge-core/cmd/forge/cost.go:180-184
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
	var env struct {
		TotalCostUsd *float64 `json:"total_cost_usd"`
	}
	if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &env); err != nil {
		return 0, false // ← stderr 输出的诊断信息破坏了 JSON 解析
	}
```

如果 claude 在 stderr 上输出 `[progress: 50%]` 或 `[warning: rate limit approaching]`,而 stdout 上有 JSON:
```
[progress: 50%]{"type":"result","result":"done","total_cost_usd":0.054}
```
→ `json.Unmarshal` 失败 → `(0, false)` → 成本数据丢失。

**C. `renderForLog` (用于日志显示)和 `observeFor` (用于解析)接收相同的合并输出:**

在 `finish()` 中:
```go
// forge-core/internal/orchestrator/command_executor.go:195-200
rendered := out.rendered()
c.observe(phase, rendered, latency)
c.logf("phase %s: ran %q -> %s", phase, strings.Join(argv, " "), c.renderForLog(rendered))
```

相同的 `rendered`(合并的 stdout+stderr)既用于日志渲染也用于结构化解析。如果 stderr 包含 JSON 无法解析的内容,两个路径都受影响。

### 场景复杂度

| 场景 | stdout | stderr | 合并后 | 影响 |
|------|--------|--------|--------|------|
| claude 正常 | `{"result":"ok","total_cost_usd":0.05}` | (空) | OK | OK |
| claude 有警告 | `{"result":"ok","total_cost_usd":0.05}` | `[claude: processing...]` | 混杂 JSON 无法解析 | ❌ 成本丢失 |
| reviewer 有调试输出 | `VERDICT: APPROVE` | `[reviewer: checking...]` | 最后一行是 `APPROVE`? | 可能 OK |
| reviewer stderr 在后 | `VERDICT: REQUEST_CHANGES` | `[reviewer: error X]` | 最后一行是 `[reviewer: error X]` | ❌ verdict 丢失 |
| echo 测试 | `ok` | (空) | OK | OK |
| 大 stderr + 小 stdout | `VERDICT: APPROVE` | 大量调试日志 | 调试日志超出 MaxOutputBytes → stdout 被截断? | ❌ 罕见,但可能 |

### 建议方向

1. **分别捕获 stdout 和 stderr**:使用**两个** `cappedBuffer` 实例分别捕获 stdout 和 stderr。这允许:
   - stdout 的 JSON/verdict 解析不受 stderr 诊断干扰
   - stderr 可以独立截断而不影响结构化输出
   - 错误检测可以分别检查 stderr(非空 stderr 可能值得记录)
2. **`observeFor` 应优先处理 stdout**:结构化数据(verdict、cost、confidence)应只从 stdout 解析。stderr 应留给日志/调试,不进 structured parsing pipeline。
3. **`cappedBuffer` 应为两个流分别维护截断状态**:一个流被截断不应影响另一个流的结构化内容。
4. **向后兼容**:对于非 claude executor(stub/echo),stdout 和 stderr 均可选,分别捕获的代码路径应与合并捕获的默认相同。

---

## 方向四 · Agent 执行环境侧信道:父进程环境无防护传递

**优先级**: 🔴 **P0** | **类别**: 安全 · 正确性 | **预估**: 0.5 sprint | **杠杆**: ⭐⭐⭐⭐⭐  
**已有分析覆盖**: 零 — 从未以系统性方向分析 child 进程环境隔离问题。

### 问题描述

`CommandExecutor` 的 `childEnv` 函数只过滤 `FORGE_AGENT_DEPTH` 这一项环境变量,其他所有父进程环境变量都无过滤地传递给 agent 子进程:

```go
// forge-core/internal/orchestrator/command_executor.go:254-260
func childEnv(depth int) []string {
	prefix := agentDepthEnv + "="
	base := os.Environ()
	out := make([]string, 0, len(base)+1)
	for _, kv := range base {
		if !strings.HasPrefix(kv, prefix) {
			out = append(out, kv) // ← 所有其他 env var 原样通过
		}
	}
	return append(out, fmt.Sprintf("%s=%d", agentDepthEnv, depth+1))
}
```

这意味着:
- **API key 泄露**:`ANTHROPIC_API_KEY`、`OPENAI_API_KEY` 等直接传给子进程。子进程是任意 agent 命令(不仅仅是 claude SDK——理论上可以是任何命令)。
- **路径泄露**:`PATH`、`HOME`、`PWD`、`FORGE_REPO_ROOT` 等传递给 agent
- **配置泄露**:任何被 forge 读取的 `FORGE_*` 环境变量也传递给 agent
- **无最小权限原则**:agent 应该只看到它完成任务所需的环境,而非父进程的完整环境

### 代码级证据

**A. `childEnv` 不过滤 `os.Environ()` 中的任何内容(除了 FORGE_AGENT_DEPTH):**

```go
// forge-core/internal/orchestrator/command_executor.go:254-260
func childEnv(depth int) []string {
	prefix := agentDepthEnv + "="
	base := os.Environ()
	out := make([]string, 0, len(base)+1)
	for _, kv := range base {
		if !strings.HasPrefix(kv, prefix) {
			out = append(out, kv) // 全部通过
		}
	}
```

**B. 只有 `--executor=command` 路径使用 `CommandExecutor`(仅有 `MainExecutor` 构造):**

在 `engine_build.go` 中构建 `CommandExecutor` 时,**没有**环境过滤逻辑:
```go
// forge-core/cmd/forge/engine_build.go:42-70 (agentExecutor 函数)
ex := orchestrator.CommandExecutor{
	Build:      func(p asset.Phase, mode string) []string { /*...*/ },
	Dir:        o.root,
	Timeout:    o.timeout,
	MaxDepth:   o.maxAgentDepth,
	MaxOutputBytes: o.maxOutputBytes,
	Log:        logln,
}
ex.Observe = observeFor(/*...*/)
// ← 此处没有 "Env" 字段,意味着 CommandExecutor 使用 childEnv() 默认行为
```

`CommandExecutor` 不暴露 `Env` 字段——`childEnv()` 是在 `runMeasured` 内部硬编码的。

**C. 对于 claude 来说这尤为重要:**

`claudeArgv` 构建的 argv 可能将敏感上下文作为 `-p` 参数,但如果 forge 本身使用 `claude` CLI 且继承了 `ANTHROPIC_API_KEY`,该 key 就会传给子进程。这不是 claude CLI 的问题(它需要 key),而是一般性原则:如果你 fork 了一个非 claude agent 进程,该进程也能看到所有父进程的环境变量。

### 场景复杂度

| 场景 | 风险 | 严重度 |
|------|------|--------|
| `ANTHROPIC_API_KEY` 传给 echo/stub | 泄漏给无输出可读的地方 | 低 |
| `FORGE_REPO_ROOT` 传给 agent | 信息泄露(项目路径) | 低 |
| 含有 `DB_PASSWORD` 的 .env 文件被 sourcing | 凭据泄漏给 agent 子进程 | 高 |
| 将 `FORGE_*` 配置泄漏给 agent | agent 能看到 ForgeOS 的治理配置 | 中 |
| agent 自身 fork 子进程(深度2+) | 环境传播到更深层级 | 中 |

### 建议方向

1. **引入环境变量白名单**:明确定义 agent 需要看到的环境变量集合(最少必需原则)。目前可推测所需的有:LLM 提供商的 API key、`PATH`(用于查找工具)、`HOME`(用于临时文件/配置)。
2. **对于 claude 路径,考虑在 `cmd.Run` 前清理环境**:或在 agent executor 构造时注入一个 `EnvFilter` 配置。
3. **为敏感环境变量增加 `FORGE_ENV_ALLOW` / `FORGE_ENV_DENY` 模式**:类似 `.gitignore` 的 allow/deny 列表,项目可以控制在 forge 输出中发送哪些环境变量。
4. **文档记录实际传递的环境**:明确说明哪些环境变量传递给 agent、为什么、以及如何使用 allow/deny 控制。
5. **注意**:这与 Sandbox(Firecracker/Docker) 隔离不同——sandbox 解决的是文件系统和网络隔离,环境变量即使在 sandbox 内也仍然是有意义的侧信道。

---

## 方向五 · 错误恢复策略的上下文感知缺口:对错误"此时、此地"的理解

**优先级**: 🟠 **P1** | **类别**: 运行时 · 可靠性 | **预估**: 1 sprint | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖**: 零 — 恢复策略的上下文感知问题从未被作为独立方向分析。

### 问题描述

当前 ForgeOS 对错误恢复策略的决策是**无上下文的**(context-free):

- `Retryable()` 只查 `ExecKind`——不关心**在管道的哪个位置**发生错误
- `runAgentPhase` 的重试逻辑不区分"此 phase 是第一次跑还是 loop-back 重跑"
- `runWave` 的 fail-fast 对波中的每个 phase 一视同仁——不区分"此 phase 是否已经消耗了大量预算"
- 预算检查 (`checkRunBudget`/`checkAgentBudget`) 是**前瞻性**的——它们检查是否应该启动一个 phase,但从不检查"已经启动但被 fail-fast 取消的 phase 的预算怎么处理"

### 代码级证据

**A. `runAgentPhase` 的重试不考虑 phase 的执行历史:**

```go
// forge-core/internal/orchestrator/orchestrator.go (runAgentPhase 相关)
// 每次 runAgentPhase 调用:
for attempt := 0; attempt <= e.MaxRetries; attempt++ {
    err := e.Exec.Execute(ctx, p, mode)
    if err == nil { return nil }
    var execErr *ExecError
    if errors.As(err, &execErr) && execErr.Retryable() {
        if execErr.Kind == KindOverloaded {
            e.sleep(overloadBackoff(attempt))
        }
        continue // 重试
    }
    return err // 不重试
}
```

但这里有一些缺失的上下文:
- 如果 `MaxRetries = 3`,一个 phase 在 loop-back 后重跑 3 次,然后又在 loop-back 后重跑 3 次 = 总共 6 次重试,不符合"最多 3 次"的预期
- 实际上 `runAgentPhase` 在每次 `RunFrom` 调用时从头开始——loop-back 会创建一个新的 `runAgentPhase` 调用,计数器重置
- 没有"此 phase 自上次成功以来的总重试次数"的跨 loop-back 计数

**B. `runWave` 的 fail-fast 取消已有预算消耗:**

```go
// forge-core/internal/orchestrator/parallel.go:192-203 (wave 中 phase 失败时的取消)
if err != nil {
    mu.Lock()
    if *firstErr == nil {
        *firstErr = err
        waveCancel() // 取消整个波
    }
    mu.Unlock()
}
```

当波被取消时:
```go
// parallel.go:148-151
go func(i int) {
    defer wg.Done()
    if err := e.runPhaseParallel(waveCtx, wf, i, mode, mu, agentCalls); err != nil {
```

`runPhaseParallel` 在 `checkAgentBudget` 中已经递增了计数器——所以**预算已经被扣除了**,即使 phase 被取消。
- 波中有 phase A(失败)和 phase B(已启动但被取消)
- Phase B 的 `checkAgentBudget` 已经发生了(计数器递增)
- Phase B 被取消,没有产出
- 预算计数器已经永久性消耗了 Phase B 的配额

**C. 收敛信号不考虑退化质量:**

```go
// forge-core/internal/converge/converge.go:109-115
func Evaluate(allOf []asset.Criterion, sig Signals) (results []Result, allMet bool) {
	allMet = len(allOf) > 0
	for _, c := range allOf {
		r := evalOne(c, sig)
		results = append(results, r)
		if !r.Met {
			allMet = false
		}
	}
	return results, allMet
}
```

收敛是二元的(MEET/NOT MET)。没有以下的表达方式:
- "在退化模式下收敛"——所有测试通过但有警告
- "大部分收敛"——3/4 标准满足,第 4 个是"no_tool"NA 但可以接受
- "下游依赖收敛"——本阶段已完成但前序阶段有未解决问题

### 场景复杂度

| 上下文 | 当前行为 | 理想行为 |
|--------|---------|---------|
| Phase 在 loop-back 后重跑,第 4 次超时 | 第 4 次 `MaxRetries` 耗尽 → 失败 | 在 loop-back 上下文中考虑"此 phase 至今已重试 11 次" |
| 波中 phase 被取消但已扣预算 | 预算减少,无工作输出 | 被取消 phase 的预算应被回滚或豁免 |
| 3 个标准中的 2 个满足,1 个 NA(no_tool) | NOT MET | 可能在 no_tool 豁免下报告"大部分收敛" |
| Phase 在第一次尝试时超时(网络慢),重试成功 | OK | OK,但日志说"重试"而非"首次因网络慢超时" |
| fork-bomb 导致 10 个子进程,一半被取消 | OOM / 墙钟浪费 | 应尽早检测到 fork-bomb 模式 |

### 建议方向

1. **为 `ExecError` 增加阶段内重试计数器**:在 `Engine` 上增加 `phaseRetryCount[phaseName]` 映射,跨 loop-back 边界保留,以便重试有一个阶段生命周期的上限。
2. **并行预算会计**:当 phase 在启动后(预算已扣)但在完成前(无输出)被取消时,预算应被"退还"。可以为被取消的 phase 实现一个 `refundAgentBudget` 函数。
3. **引入收敛信号质量维度**:`converge.Signals` 应支持指示信号质量的元数据(如 `test_pass: PASS (but with warnings)`)而非仅有 PASS/FAIL。
4. **`failure_summary` 信号**:在收敛报告(MET/NOT MET)之外,应生成一个结构化失败摘要,列出每个失败或退化信号的详细上下文,以便人类或父编排器可以根据完整的上下文做出决定。
5. **`runWave` 应支持每个 phase 的独立失败阈值**:不是波中的第一个失败就取消所有——允许在波级别有一定阈值的失败,只要关键路径通过。

---

## 总结:紧迫性与收益

| # | 方向 | 优先级 | 类别 | 代码证据强度 | 已有分析覆盖 | 预估工作量 | 收益 |
|---|------|--------|------|------------|------------|-----------|------|
| 1 | 输出管道完整性 | P0 | 正确性 | ⭐⭐⭐⭐⭐ | 零 | 1 sprint | 防止成本数据/verdict/confidence 静默丢失 |
| 2 | 错误分类维度 | P1 | 可靠性 | ⭐⭐⭐⭐⭐ | 零 | 1.5 sprints | 更好的恢复决策,减少"flaky"运行失败 |
| 3 | Stdout/Stderr 分离 | P1 | 架构 | ⭐⭐⭐⭐⭐ | 零 | 1 sprint | 结构化输出可靠性,成本/verdict 按流隔离 |
| 4 | 环境侧信道防护 | P0 | 安全 | ⭐⭐⭐⭐ | 零 | 0.5 sprint | 防止 API key/配置向子进程泄漏 |
| 5 | 上下文感知恢复 | P1 | 可靠性 | ⭐⭐⭐⭐ | 零 | 1 sprint | 减少预算浪费,更好的退化行为 |

**三个最紧迫的改进**:

1. **输出管道完整性(方向一)**:截断是真实会发生的事(`MaxOutputBytes` 默认 10MB,但 agent 可产生任意大输出)。目前,截断时下游完全静默丢失数据——无法被 operator 注意到,直接导致成本欠计和 verdict 丢失。
2. **Stdout/Stderr 分离(方向三)**:不分离流的架构选择在错误分类(方向二)改进或输出管道(方向一)修复之前就会造成问题——只要 agent 在 stderr 上产生任何输出(警告、进度、诊断),cost/verdict 解析就处于风险之中。
3. **环境侧信道防护(方向四)**:这是最低成本(只需修改一个 `childEnv` 函数)但最高杠杆的改进——目前每个 agent 子进程都能读取父进程的全部环境,包括 API key 和其他敏感凭据。
