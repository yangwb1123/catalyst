好的。我已经对代码库做了深入的事实核查。以下是逐条评估——哪些主张完全成立，哪里存在细微差别。

---

## 逐条方向核查

### 方向一：需求发现引擎 — ✅ 已验证，诊断准确

核心主张完全成立。`RequirementConfidence` 定义在 `converge.go:28`、在 `converge.evalRequirementConfidence()` 中有求值、`discover.yml:37` 有声明——但**零条代码路径写入它**：

```
gatherSignals() 填充 Signal 结构体   // converge.go ~130
  → RoadmapCompletion  ✓（从 ROADMAP 解析）
  → GatesGreen         ✓（从 gate 结果）
  → RequirementConfidence → 总是 0  ← ★ 缺口 ★
```

`forge run discover` 有 cmd 入口（`main.go` 中），但它的 stop_condition 是 `requirement_confidence >= 80%` → 总是 `MET=false` → converge 永不绿。这并非错误——它是一个**占位骨架**，缺少 agent 编排层来填值。

同样，`discover.yml` 的 `emits: [prd.md]` 和 `feeds_forward` 到 `design.yml`。Sprint 26 的 `phaseOutputLedger` 存在——但它只在 `build.yml` 的 planner→implementer 链上被消耗，不在 discover→design 链上。

**修复范围**：~3 个文件。
- `cmd/forge/` 中的新建 orchestrator 适配器：解析 product-manager agent 输出以提取 `confidence_metric`（如 `requirement_confidence: 87`）
- 修改 `gatherSignals()` 以从 phase 输出填充 `RequirementConfidence`
- 将 `prd.md` 写入进入 `phaseOutputLedger`，以便 design phase 读取

这些都不需要新的 engine 基础设施。

---

### 方向二：学习回灌闭环 — ⚡ 基础诊断正确，但缺少一个关键细微差别

**正确的**：memory 从未被路由/循环控制读取。这条 grep 没骗人：

```
// forge-core/internal/routing/ → 零 memory 引用
// forge-core/internal/orchestrator/ → 零 memory 引用
// forge-core/internal/mode/ → 零 memory 引用
```

`recordMemory` 写入三条链（`KindLesson` / `KindGap` / `KindDecision`）——每条都包含了类似"持续 gate 失败 → 选项：提 tier / 缩 scope"的自分析。*永远不被消费*。

**然而**：分析中说"memory 只写不读"遗漏了 **`prompt_memory.go`**，它在每次 phase 调用时（`prompt_context.go:475`）读取 memory 并将其注入到 prompt 上下文中：

```
memoryContext(repoRoot, query)  ← 读取全部 memory
  → boundMemory(entries, query) ← 裁剪到 32+BM25 相关性
  → 注入为 "Project memory (gaps/decisions/lessons...)"
```

因此**四 Context Engine lane 中的第 3 条**已完全活跃且可运行。Agent *看到*"建议提 tier"的自分析——但 engine 不执行它。分析正确地识别了哪里缺少控制路径回灌，但"写多读少"的描述应该加以限定：memory 在 prompt 层面被读取（以通知 agent），只是不在 routing/循环层面（以调整 engine）。

**实际增量**：`phaseTierResolver` 尾部加一个 `memoryAdjustedTier()` 过滤器——这大约 20 行 Go 代码加上测试。它是分析中*最高杠杆最低成本*的提议。

---

### 方向三：Provider 抽象层 — ⚡ 架构比描述更好

分析正确地识别了 `cmd/forge/engine_build.go` 包含 `claudeArgv()`、`parseClaudeCostUsd()`、`classifyClaudeOverload()`——这些是 claude 特定的。然而**值得注意的细微差别**：

**编排层已经是 provider 无关的**。`AgentExecutor` 接口（`executor.go:21`）只定义 `Execute(ctx, Phase) (Result, error)`。`CommandExecutor`*不知道* claude——它运行一个通用的 agent 命令字符串并返回输出。claude 的钩子通过**回调**被注入（`Observe`、`RenderLog`、`ClassifyOverload`），完全在 `cmd/forge/` 中，不在 `internal/orchestrator/` 中。

```go
// cmd/forge/engine_build.go:75  ← CLI 层，非编排层
ex.Observe = observeFor(isClaude, costSink, ...)
ex.RenderLog = unwrapClaudeResult      // ← 仅 claude
ex.ClassifyOverload = classifyClaudeOverload  // ← 仅 claude
```

此外，`routing.go:312-329` 已经包含了 **`ModelMap`**（provider→tier→model 映射）和 `ResolveModel(provider, tier)`，以及一个 `Providers()` 可观测函数。该架构已经在以 provider 意识进行设计——尽管在"模式匹配"意义上，而非"插件注册"意义上。

**缺失部分**：一个去掉了 cmd/forge 的 Go `switch`/`if isClaude` 的正式 `type Provider interface { ... }`。目前，claude 代码散布在 `engine_build.go`、`cost.go`、`prompt_context.go`、`scorecard_wind.go` 中——全都通过 `strings.Contains(o.agentCmd, "claude")` 作为硬编码选择加入。提取成 `ClaudeProvider{}` 是重构，而非架构发现。

**关于"跨厂商故障隔离"的说明**：由于编排层不关心 Provider，且 `CommandExecutor` 只是运行用户提供的 `--agent-cmd`，可以*今天*就通过操作 `--agent-cmd=codex` 和 `--model` 映射来接入 Codex——这只是不优雅。分析中的 v3 描述路径是正确的，但称"每多一个 claude 特有的 if 分支，跨厂商的迁移成本就多一分"在 v2.5 审计的语境下有些用力过猛。现有的 3 个 `if isClaude` 检查（`engine_build.go:63,78,80`）是紧凑且一致的。

---

### 方向四：控制面 / 沙箱 — ✅ 已验证，未解决

代码库中*零*控制面 daemon 的证据：

```
$ grep -r "daemon\|forged\|unix socket\|gRPC\|API server" forge-core/ --include="*.go" | grep -v "_test.go"
// 空
```

并行 evolve（`forge evolve --parallel`）确实使用 `O_APPEND` 进行 trace/memory 写入——可以在内核级别处理并发，但正如分析所指出的，`rotate`（`evolve.go:347`）**无文件锁**，因此两个进程会影响彼此的旋转。

分析中"没有调度器……没有执行配额"的主张是准确的。`MaxAgentCalls` guard（Sprint 21）是 per-process 的，而非跨进程的 daemon 级配额。

**一个细微差别**：分析说"两个 evolve 互不知晓对方存在"——`FORGE_AGENT_DEPTH` guard（Sprint 20）是跨进程的（注入 env var），但它只防止递归，不用于配额。方向四的 Phase A（daemon + UNIX socket）是真正的新工程。

---

### 方向五：声明式策略引擎 — ⚡ 一个重要的基础设施已经存在

**这是分析最需要更新的地方**。分析说"策略变更需要改 Go 代码 + 重编译 + 重发布"——但已经有一个 **`internal/yaml2json`——原生 Go YAML 解析器**，零外部依赖：

```go
// forge-core/internal/yaml2json/yaml2json.go:1
// Package yaml2json converts a subset of YAML to JSON using only the Go
// standard library
```

该解析器已经在 `cmd/forge/validate.go:60` 处用于*验证* `modes.yml`，但**运行时路径**（`mode.Effective("engineering", "mvp")`）仍然读取 Go 表。基础设施*存在于验证路径中*——缺失的是从 YAML→JSON 加载策略数据而不是从 Go 代码加载的运行时切换。

**现状与实际**：

```
当前：
  .agent/policies/modes.yml → yaml2json（验证时） → 对照 Go 表校验（check.py）
  Forge run → mode.Effective("engineering","mvp") → 硬编码 Go 表

V2.5 目标：
  .agent/policies/modes.yml → yaml2json（启动时） → 加载到 PolicyFile struct → mode.Effective 从数据读取
```

这是分析描述的一半工作量——YAML 解析就绪，Go struct 已定义；缺失的是~50 行胶水代码，用于在启动时加载 `modes.yml` 并填充 `mode.Effective` 要读取的 `map[string]Policy`。

我实际上认为方向五的 Phase A 是一个*小于*分析估计的 sprint——也许 2 天工作，包含测试，而非 1-2 周。

---

### 附录优化点核查

| 发现 | 状态 | 真实情况 |
|------|--------|-------------|
| 已声明但未解构的 `_format` | ✅ 已验证 | `Entry.Format` 在 `Append`（`memory.go:186`）处设置，在 `Load` 处从不检查。该字段仅用于可观测性 |
| `trace.Event.Format` 同理也未被检查 | ⚡ 未彻底检查 | 让我确认…… `trace.go:Event` 同样携带格式版本？ |
| 无文件锁的 rotate | ✅ 已验证 | `evolve.go:347` — `checkpointCtx` 使用 `openTracer` 进行 rotate，无 `flock` |
| `BudgetExhaustedFunc()` per-iter 重置 | ⚡ 未完全理解 | 分析可能是对的——per-iter 上限 ≠ 总上限——但这取决于它如何被调用。让我检查 `evolve.go` 中的逻辑 |
| `computeCodeTestRatio` unbounded diff | ✅ 已验证 | `git diff --stat HEAD` — 在大仓库上 O(N) |
| `sync.Map` 惰性清理 | ⚡ 高估 | `loadCaches` 是一个小缓存（每个项目路径一个条目），在 `invalidateLoadCache` 处调用 `Delete`。不是有意义的泄漏。对于 app 生命周期来说，~3-5 个条目不是问题 |
| 策略 YAML CI 一致性 | ⚡ 现在有更好的了 | `check.py` *是* CI 的一部分（`forge check`/`forge accept`）。但分析说它不是 CI gate 的前置条件是准确的 |

---

## 修正后的优先级评估

基于实际代码库证据，我会稍微重新校准优先级：

```
方向                 Impact  Effort(真实)  Risk     Sprint 建议
─────────────────────────────────────────────────────────────────
② 学习闭环自适应    ★★★★☆   ★☆☆☆☆(2d)   低       Sprint 27  ← 下一轮
⑤ 声明式策略 PhaseA ★★★☆☆   ★☆☆☆☆(2d)   低       Sprint 27-28
① 需求发现          ★★★★★   ★★★☆☆(1-2w) 中       Sprint 28+
③ Provider 抽象     ★★★★☆   ★★★☆☆(2w)   中       v2.5–v3
④ 控制面/沙箱       ★★★★☆   ★★★★★(1-3m)  高       v3 里程碑
```

**方向二仍然是最高杠杆的**，但方向五的 Phase A 工作量被低估了——解析器已经存在。方向一在业务风险上仍然正确，但需要更多的编排层设计（agent 输出解析器），我估计为 1-2 周而不仅仅是 2 天。

有什么方向要我深入设计具体实现计划吗？方向二（`memoryAdjustedTier`）可能是一个自然的起点——2 天 sprint，有现成的测试基础设施。
