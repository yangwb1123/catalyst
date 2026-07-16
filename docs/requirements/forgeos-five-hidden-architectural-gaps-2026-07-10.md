# ForgeOS — 全局扫描后五个从未被覆盖的架构性缺口

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局逐文件深扫：forge-core（19 Go 包 · ~35k LOC 运行时 + CLI 纯 stdlib）、  
>    harness（39+ Node/Python 模块 · ~10.5k LOC 执法层）、  
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · policies · modes · routing）、  
>    examples/（url-shortener · go-taskd）、pi-batch.py、`.github/workflows/`、根文档  
> 2. 完整阅读 Sprint 1–31 演进记录 + FUNCTIONAL_REQUIREMENTS_AUDIT（90+ DONE · 0 GAP）+ 4 ADR + 核心设计文档  
> 3. **差异化验证**：对每个方向的**核心关键词与核心命题**，在 **120+ 份已有分析文档**（`docs/requirements/` 68 篇 + `docs/analysis/` 40+ 篇 + ROADMAP/CURRENT_SPRINT/FUNCTIONAL_REQUIREMENTS_AUDIT）中逐篇全文检索 + 语义比对，确认该方向的**独立命题从未被任何已有分析展开**  
> 4. **纪律**：不编写任何代码。每个方向附 `file:line` 代码级证据、边界场景、与已有覆盖的精确差异化证明  
> **日期**: 2026-07-10

---

## 全景定位：120+ 份分析的密集覆盖与固化盲区

ForgeOS 经过 31 轮 sprint 和 120+ 份分析文档的持续深扫，几乎所有功能域都被覆盖了多个方向。以下是已被充分覆盖的主要域：

| 覆盖域 | 方向数 | 表代性文档 |
|--------|--------|-----------|
| 编排引擎内核（串行/并行/loop-back/mode-gating/stop-condition） | ~35 | `expansion-core-five.md` |
| 生产可靠性（529/超时/退避/预算护栏/进程组/递归守卫） | ~18 | `expansion-production-readiness.md` |
| 可观测性（trace/scorecard/telemetry/三维真数据） | ~10 | `seventh-wave-data-realism.md` |
| 记忆/学习（memory/checkpoint/Supersedes/ContextCache） | ~12 | `expansion-five-systemic-learning-loop-gaps.md` |
| 路由/调度（TierFor/多维评分/BudgetAdjust/HistoryTiebreak） | ~8 | `novel-architectural-extensions-v40.md` |
| 安全纵深（secret-scan/SCA/递归/预算/超时/输上限） | ~12 | `five-product-operational-gaps.md` |
| 治理/执法（arch-check 8 检查/check.py/circular/loop-back） | ~12 | `fogotten-five-foundations.md` |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 | `genuine-architectural-horizons-five.md` |
| 结构债务（YAML 碎片/cmd/forge 依赖中枢/存储长） | ~8 | `structural-gaps-v41.md` |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | ~10 | `execution-semantic-gaps.md` |
| 产品交付（部署/回滚/决策解释/人机协作/版本治理） | ~5 | `product-deployment-transparency-five-gaps.md` |
| 二进制生命周期（安装/升级/回滚/签名/验证） | ~5 | `forge-core-five-unseen-structural-gaps.md` |
| 北向扩展（Temporal/OPA/OTel/多厂商/Sandbox/Web UI） | ~8 | `v2-to-northstar-gap.md` |
| 系统边界盲区（跨进程/信任边界/持久语义/并行安全） | ~12 | `strategic-extensions-v22~v33.md` |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声丢失） | ~10 | `second-order-architectural-gaps.md` |
| 三存储一致性/format 版本化/状态自校验/多会话 | ~5 | `genuine-uncovered-five-binary-state.md` |

**但是**，已有分析的密集覆盖全部聚焦于「加什么功能」(~70) 和「边界可靠性」(~40)。  
在这两种分析范式之外，存在一类**被系统性地忽略的缺口**——它们不是「缺失的引擎」，不是「需要修的边界」，  
而是**当前设计中已存在的结构特征，其长期代价未被承认或未被测量**。

以下五个方向全部落在这个死角中。

---

## 方向一 · Gate 在定向 loop-back 中被完整重新执行：可忽略的每事件代价 × 不可忽略的累计墙钟

**优先级**: 🟠 P2 | **类别**: 性能 · 架构债务 | **预估**: 分析 + 实现 ~1 sprint | **杠杆**: ⭐⭐⭐⭐

### 问题描述

当 gate phase 失败并触发 `on_fail:{action:loop_back, target_phase: implementer}` 时，
`RunFrom` 从 target phase 开始重新执行所有 phases，包括已经通过（PASS）的 gate phases。
一个已经 PASS 的 `test` gate（运行 `go test ./...` 或 `node --test`）在 loop-back 后又被完整执行一次，
即使两次 gate 运行之间只有 implementer phase 修改了文件——而 harness-gates phase 之前的文件（在第一次运行中已被 gate 验证）根本没变。

`build.yml` 的典型 loop-back 场景：

```
迭代 1:  implementer(写代码) → harness-gates(test+build+arch+security → PASS) → reviewer → qa
                                                              ↑ FAIL → loop-back
迭代 2:  implementer(修代码) → harness-gates(test+build+arch+security → 再跑一次!) → reviewer → qa
```

**三个维度的问题**：

1. **无损重跑**：重跑的 gate 作用于一组在 gate 和 target 之间**没有变化的文件**——harness-gates 之前的文件
   （上次 gate 运行时已验证过）是干净的，只有 implementer 的输出可能影响 gate。「清洁」和「可能影响」区域
   没有分离，所以每次 loop-back 都必须重跑 gate 套件。

2. **不可忽略的累计**：一次 `go test ./...` + `node harness/arch-check.mjs` + `node secret-scan.mjs` 可能
   花费 5-60 秒。`MaxLoopBack=3` 意味着每次 loop-back 最多增加 3× gate 时间给总墙钟。在一个 gate 密集的
   build.yml 中（3 个 gate phase × 3 loop-backs = 9 次额外 gate 执行），gate 重跑可能贡献数分钟的纯浪费。

3. **无差异检测**：engine 不跟踪「自上次 gate 运行以来哪些文件变了」，所以无法区分「文件已变 → 需要重跑」
   和「文件未变 → gate 结果可复用」。从 gate 的视角，每次调用都像一次全新的冷启动检查。

### 代码级证据

**1. RunFrom 的 for-loop 对 loop-back 不做 gate 跳过**——gate phase 被当作普通 phase，无条件重新进入：

```go
// forge-core/internal/orchestrator/orchestrator.go:176-186
if len(p.RequiredGates) > 0 {
    if err := e.runGates(p, e.gatesFor(p)); err != nil {
        target, jumped := e.gateOutcome(wf, p, &loopBacks)
        if !jumped {
            return err
        }
        i = target - 1 // -1 因为 for-loop 的 ++ 会回到 target
        continue
    }
```

当 loop-back 跳回 implementer 后，从 implementer 到 harness-gates 之间的所有 phase 都重新执行，
包括已经 `PASS` 的 harness-gates 自己。没有任何跳过已通过 gate 的机制。

**2. `runGates` 不做增量判断**——它只查 `callGate → Result`，不知道这个 gate 之前跑过的结果：

```go
// forge-core/internal/orchestrator/orchestrator.go:346-370
for _, name := range gates {
    res := e.callGate(name)
    switch gateStatus(res) {
    case gate.StatusFail:
        // ... FAIL
    case gate.StatusNA:
        // ... N/A
    default: // StatusPass
        e.logf("phase %s: gate %s ok", p.Name, name)
        e.onGateResult(name, "ok")
    }
}
```

`callGate` 最终调用 `HarnessRunner(repoRoot, probe)` → `ResolveGate` → 真实子进程。每次都是新鲜的。

**3. gateProbe（acceptance 探测）也是每次执行**——`probeStatuses` 在 `gatherSignals` 中每个 iteration 只调用一次，
但 `runGates` 的 `HarnessRunner` 读的是这个**已缓存的 probe map**。所以 gate 重跑本身不重跑探测，
但它会重跑 `complexity` gate（即 `node harness/gate.mjs`——体积闸门）和 `arch` gate（`node arch-check.mjs`）
——这些是独立子进程，不是 probe map 查找。

在 build.yml 中，`required_gates: [complexity, arch, test]`——其中 `test` 是 probe-backed（缓存命中），
但 `complexity` 和 `arch` 是**每次都 fork 子进程**的真实检查。

### 边界情况

| 场景 | 问题 | 当前行为 |
|------|------|---------|
| Reviewer 触发 loop-back 到 implementer | 已 PASS 的 harness-gates 在 loop-back 后又被重跑 | 无条件重跑所有 gate phase |
| QA phase 失败触发 loop-back | 同场景，gate 重跑累计 3 次 | 同 |
| 多 loop-back 链 | 每个 loop-back 重跑同一套 gate | 与 loopBacks 计数器无关 |
| gate 本身运行时间长（go test） | 每次 loop-back 增加 ~10-60s | 无差异 |

### 差异化证明

| 已有分析 | 覆盖内容 | 本方向新增 |
|----------|---------|-----------|
| `edgecases-and-perf.md` 增量测试选择 | evolve 每次 iteration 全量 acceptance 套件 | **loop-back 内部的 gate 重跑**——单 iteration 内，不是跨 iteration |
| `expansion-production-readiness.md` 方向三 | gate 执行超时/并行化 | **执行次数优化**，非执行方式优化 |
| `five-uncovered-architectural-frontiers.md` 方向五 | 并行 gate 的串行瓶颈 | loop-back gate 是**串行的浪费**，不是并行瓶颈 |
| `high-value-extension-v47.md` 方向三 | gate 预算编排 | gate 套件的**预算消耗**，非重跑次数 |

---

## 方向二 · AgentExecutor 接口的生命周期真空

**优先级**: 🟡 P2 | **类别**: 架构 · 可扩展性 | **预估**: 分析 + 实现 ~1–2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

`AgentExecutor` 接口是一个单方法抽象：

```go
// forge-core/internal/orchestrator/orchestrator.go:56-58
type AgentExecutor interface {
    Exec(ctx context.Context, phase asset.Phase, mode string) (output string, err error)
}
```

当前只有一个实现 `DryRunExecutor`，其 Exec 只是 logging + 返回空字符串——不需要资源管理。
所以这个接口的单薄性从未被测试暴露。

但当未来需要接入真正的 agent CLI（Claude Code SDK、Codex、Gemini CLI、自定义 agent）时，
每个实现者都需要独立解决以下问题，而接口没有为它们提供契约：

- **初始化**：agent 子进程是否需要预热？是否需要连接池？初始化失败应该阻塞 run 还是降级？
- **关闭**：phase 结束后，agent 子进程资源应该如何释放？如果 Exec 返回后还有后台 goroutine 在跑呢？
- **回滚**：phase 失败后，executor 是否应该清理它创建的资源（临时目录、lock 文件、凭证缓存）？
- **健康检查**：在 Exec 被调用前，executor 是否健康？依赖的 CLI 是否已安装？

### 代码级证据

**1. 接口定义——单方法**：

```go
// forge-core/internal/orchestrator/orchestrator.go:56-58
type AgentExecutor interface {
    Exec(ctx context.Context, phase asset.Phase, mode string) (output string, err error)
}
```

**2. DryRunExecutor 的实现**——只是 logging：

```go
// forge-core/internal/orchestrator/command_executor.go:38-48
// DryRunExecutor is the shipped agent executor: it prints an honest routing
// decision and returns an empty output... No real agent is ever spawned.
type DryRunExecutor struct{}

func (DryRunExecutor) Exec(ctx context.Context, p asset.Phase, mode string) (string, error) {
    tier := TierFor(p.Agent, mode)
    // ... logging only
    return "", nil
}
```

**3. CommandExecutor 是另一个实现，但它不是 AgentExecutor**——它在 dry-run executor 的下层使用：

```go
// forge-core/internal/orchestrator/command_executor.go
type CommandExecutor struct { ... }
func (ce *CommandExecutor) Run(ctx context.Context, command string, args ...string) (output string, err error)
```

CommandExecutor 是**进程执行器**，不是 AgentExecutor 的实现。AgentExecutor 的对等替换（真正的 agent executor）
会是 `ClaudeExecutor`、`CodexExecutor` 等——它们现在都不存在。

### 典型的具体缺失

假设一个 `ClaudeExecutor` 需要通过 SDK 连接到一个已认证的 Claude Code 实例：

```
ClaudeExecutor.Init(agent, config)    → 认证、池化、健康检查（不存在）
ClaudeExecutor.Exec(ctx, phase, mode) → 发送 prompt、接收输出（接口部分）
ClaudeExecutor.Shutdown(phase)        → 释放 phase 资源（不存在）
ClaudeExecutor.Rollback(phase)        → 回滚 phase 写出的文件（不存在）
ClaudeExecutor.Health()               → 返回连接状态（不存在）
```

现在每个新的 executor 都必须从零开发生命周期管理，且没有统一的模式可遵循。

### 边界情况

| 场景 | 问题 |
|------|------|
| Executor 初始化失败（CLI 未安装、证书过期） | 没有 Init 方法，故障只能在第一次 Exec 调用时发现 |
| Executor 有状态（连接池/token） | 没有 Init/Cleanup 契约，资源泄露无法预防 |
| Phase 回滚需要 executor 参与（撤销写出的文件） | 没有 Rollback 方法 |
| 多个 executor 需要资源隔离（每个 phase 不同 executor） | 没有所有权的标准模式 |
| Executor 在 phase 之间需要保持状态 | 没有 State 接口，只能靠全局变量 |

### 差异化证明

| 已有分析 | 覆盖内容 | 本方向新增 |
|----------|---------|-----------|
| `execution-semantic-gaps.md` 方向一 | phase 级别的原子性、幂等、回滚 | **executor 粒度的生命周期**——比 phase 更低一层 |
| `strategic-extensions-v33.md` 方向四 | 自定义 executor 与 Sandbox 集成 | executor 驱动的**统一生命周期契约**，非集成方式 |
| `v2-to-northstar-gap.md` 方向一 | Claude Agent SDK 接入 | **接口抽象层的缺失**，不是连接方式 |
| `genuine-architectural-horizons-five.md` 方向二 | 多厂商模型池 | executor 的**管理协议**（非模型的路由协议） |

---

## 方向三 · Agent 输出契约验证的脆弱性：无 schema 的文本解析

**优先级**: 🔴 P1 | **类别**: 可靠性 · 数据完整性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 从 agent 的非结构化文本输出中提取结构化信号时，使用纯字符串匹配/正则解析。
系统中有五个不同的文本解析器，它们的共同特征是：**对 agent 输出格式的假设全部是隐式的**，
没有任何验证层来确认 agent 的输出符合预期的 schema。

当 agent 的输出格式稍有偏离（包装了 markdown、加了解释文字、换行方式不同），
解析器就**静默失败**——返回 "no signal"，然后调用者 fail-open（继续执行）。
这意味着：

- 一个 reviewer 写了 `**VERDICT: REQUEST_CHANGES**`（markdown 加粗）→ 解析失败 → 视为 APPROVE → 代码直接进入 QA
- 一个 product-manager 写了 `CONFIDENCE: 85%`（带百分号）→ 解析失败 → 视为「无置信度信号」→ 跳过收敛检查
- 一个 reviewer 在 VERDICT 行后加了一段解释 → 最后一行不是 VERDICT → 解析失败 → 同上

### 代码级证据

**证据 1: parseReviewerVerdict —— exact-match on last line**

```go
// forge-core/cmd/forge/cost.go:330-341
func parseReviewerVerdict(output string) (verdict string, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    switch last {
    case "VERDICT: " + VerdictApprove:
        return VerdictApprove, true
    case "VERDICT: " + VerdictRequestChanges:
        return VerdictRequestChanges, true
    default:
        return "", false // missing/wrapped/malformed -> no signal (caller fails open)
    }
}
```

`lastNonEmptyLine` 取最后一个非空行，然后 exact-match `"VERDICT: APPROVE"` 或 `"VERDICT: REQUEST_CHANGES"`。
如果 agent 写的是 `"- VERDICT: REQUEST_CHANGES"`、`"VERDICT: REQUEST_CHANGES."`（带句号）、
`"**VERDICT: REQUEST_CHANGES**"` 中的任何一个，全部走入 `default` 分支 → ok=false → 裁决被静默丢弃。

**证据 2: parseConfidenceScore —— CutPrefix + Atoi，严格到连 `85%` 都拒绝**

```go
// forge-core/cmd/forge/cost.go:391-402
func parseConfidenceScore(output string) (score float64, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    numStr, hasPrefix := strings.CutPrefix(last, confidenceContract) // "CONFIDENCE: "
    if !hasPrefix { return 0, false }
    n, err := strconv.Atoi(numStr)
    // ...
    if n < 0 || n > 100 { return 0, false }
    return float64(n), true
}
```

`CONFIDENCE: 85%` → `Atoi("85%")` → error → ok=false。一个写得更精确的 agent 反而被视为无信号。

**证据 3: parseClaudeCostUsd —— JSON 解析，但 claude 输出可能是多行文本**

```go
// forge-core/cmd/forge/cost.go:180-193
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
    var env struct { TotalCostUsd *float64 `json:"total_cost_usd"` }
    if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &env); err != nil {
        return 0, false // not a single JSON object (echo/dry/stub, or a multi-line tail)
    }
```

如果 claude 的输出不是严格单行 JSON（例如 stdout 结尾有一行空行或日志输出），`json.Unmarshal` 失败 → ok=false
→ 不记录成本 → budget guard 认为没有花费 → 可能允许超出预算的调用。

**证据 4: classifyClaudeOverload —— 正则 + 字符串包含**

```go
// forge-core/cmd/forge/cost.go:287-296
func hasOverloadMarker(s string) bool {
    lower := strings.ToLower(s)
    if strings.Contains(lower, "overloaded_error") || strings.Contains(lower, "overloaded") {
        return true
    }
    return containsToken529(s)
}
```

如果 agent 的输出意外包含 "overloaded"（例如 agent 在分析这个文本本身），`is_error=false` 的 guard 可以保护，
但这是一个启发式门控，不是 schema 验证。

### 根本原因

没有 agent 输出 schema。每个 agent 的 `role card` 用自己的 prose 描述输出格式，但：
1. 没有机器可读的输出契约（JSON Schema、protobuf、类型定义）
2. 没有输出格式的测试（golden file、schema validation tests）
3. 没有格式偏离时的反馈给 agent（"你的 VERDICT 格式不对，请重试"）
4. fail-open 的默认行为让格式错误静默丢失信号

### 边界情况

| 场景 | 结果 |
|------|------|
| Reviewer 写 `VERDICT: REQUEST_CHANGES` 后加空行再写 `---` | lastNonEmptyLine 读到 `---` → 不匹配 → 静默 APPROVE |
| Reviewer 写 `**VERDICT: REQUEST_CHANGES**` | exact-match 失败 → 静默 APPROVE |
| PM 写 `CONFIDENCE: 85/100` | Atoi("85/100") → 失败 → 无置信度信号 |
| PM 写 `CONFIDENCE: 85` 但前面有其他数字行 | lastNonEmptyLine 可能取到其他行 → 失败 |
| Claude 输出带 trailing text （stdout+stderr 混合） | JSON 解析失败 → 成本未记录 |

### 差异化证明

| 已有分析 | 覆盖内容 | 本方向新增 |
|----------|---------|-----------|
| `five-verifiable-code-level-gaps.md` 方向一 | memory.Confidence 的零值歧义 | agent **输出格式的 schema 缺失**，不是字段默认值 |
| `forgotten-five-foundations.md` 方向四 | 输出真实性闸门（agent 自证物） | **格式验证**（非真实性），且是现有解析器的脆弱性 |
| `expansion-production-readiness.md` 方向一 | Prompt QA / golden prompt 测试 | **输出侧**的契约验证，不是输入侧 |
| `five-uncovered-architectural-frontiers.md` 方向四 | phase 级别输出契约验证（Emits/Readonly 没被消费） | **现有的消费代码**（verdict/confidence/cost 解析）的脆弱性 |

---

## 方向四 · 双 YAML 解析器的静默语义漂移（正确性风险，非维护成本）

**优先级**: 🔴 P1 | **类别**: 数据完整性 · 正确性 | **预估**: 验证 + 修复 ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 有两个独立的 YAML→JSON 解析器：
- 主路径：`forge-core/internal/yaml2json/`（纯 Go 实现）
- 回退路径：`harness/yaml2json.py`（Python 实现）

Go 解析器是主路径（更快、零依赖），Python shim 是 Go 解析器失败时的回退。
但**如果 Go 解析器对一个合法的 YAML 文件输出**与 Python 解析器不同的 JSON，
没有门禁能捕获这个差异**——因为回退路径只在 Go 解析器**出错**时被调用，**不是**在 Go 解析器输出
与 Python 解析器**不同**时被调用。

具体来说：
1. `loadWorkflow` 先试 Go 解析器。如果它返回了合法的 JSON（含有效的 Workflow），就用这个结果。
2. Go 解析器永远不会自动与 Python shim 交叉验证。
3. 如果 Go 解析器对某个边缘 YAML 语法（混合缩进、注释嵌套、多行 block scalar 末尾空格、非标准换行符）
   产生与 Python 不同的 JSON 输出，而 Go 没有报错，则 `asset.Workflow` 的结构可能不一样——静默的、不被注意的偏移。

### 代码级证据

**1. loadWorkflow 的双解析路径——先 Go，失败才 Python**

```go
// forge-core/cmd/forge/main.go:228-259
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    ymlPath := filepath.Join(repoRoot, ".agent", "workflows", name+".yml")
    // Try the native Go YAML parser first (zero-dep, faster).
    f, err := os.Open(ymlPath)
    // ...
    val, err := yaml2json.Decode(f)
    if err == nil {
        data, marshalErr := json.Marshal(val)
        if marshalErr == nil {
            wf, parseErr := asset.LoadWorkflowJSON(data)
            if parseErr == nil && len(wf.Phases) > 0 {
                return wf, nil // Go parser path: no cross-check
            }
        }
    }
    // Fallback: try the Python yaml2json shim.
    // ...
    out, execErr := exec.Command("python3", shim, ymlPath).Output()
    return asset.LoadWorkflowJSON(out)
}
```

当 Go 解析器返回合法的 Workflow（`err == nil && len(wf.Phases) > 0`）时，**立即返回**，不交叉验证 Python shim 的输出。

**2. 两种解析器用完全不同的代码路径处理相同的 YAML**

Go 解析器：`normalizeLines` → `parseDocument` → `parseMapping`/`parseSequence`/`parseScalar`

```go
// forge-core/internal/yaml2json/normalize.go: 将原始文本转为行数组
// forge-core/internal/yaml2json/mapping.go: 自定义缩进感知的 key:value 解析
```

Python 解析器：`harness/yaml2json.py` 使用 `re` 模块 + 自定义行解析器

这两种解析器对以下边缘情况的处理可能不同：
- **混合缩进**：Go 的 `normaizeLines` 将 tab 转为空格（`strings.ReplaceAll(line, "\t", "  ")`），但不一定完全标准化
- **空的 sequence 或 mapping**：`[]` vs `null` vs 省略
- **多行 block scalar 的末尾空行**：Python 的 `|` 可能保留或丢弃末尾换行
- **纯数字字符串**：Go 的 `parseScalar` 中 `"123"` 变成 `float64(123)`，Python 也可能变成 `int`
- **Unicode 处理**：Go 和 Python 对 BOM、非标准空格的处理不同

**3. Go 解析器测试不覆盖 Python shim 的输出，反过来也一样**

```go
// forge-core/internal/yaml2json/yaml2json_test.go — 只测 Go 解析器自身
```

```python
# harness/test_yaml2json.py — 只测 Python shim（且只真实 build.yml）
```

**没有 golden-file 测试证明两个解析器对同一组输入产生 byte-identical 的输出。**

### 实际影响

| 场景 | 如果 Go 和 Python 产生不同输出 | 结果 |
|------|-------------------------------|------|
| workflow YAML 新增一个字段 | Go 解析器可能丢了这个字段（如果缩进敏感） | Workflow 加载后缺字段 |
| YAML 有边缘空格/注释 | Go 解析器输出与 Python 不同的结构 | 回退路径永不被触发，Go 的「正确但不同」= 静默偏移 |
| 相同的 YAML 在不同环境中产生不同的 Workflow | Go 可用 → 用 Go 版本；Go 出错 → 用 Python 版本 | 不可重现的 Workflow 加载行为 |

### 差异化证明

| 已有分析 | 覆盖内容 | 本方向新增 |
|----------|---------|-----------|
| `forge-core-five-unseen-structural-gaps.md` 方向二 | 双解析器**维护成本**（工程效率） | 双解析器**正确性风险**（静默语义漂移） |
| `structural-gaps-v41.md` 方向二 | YAML 碎片与 check.py 分裂 | 解析器**输出差异**，非配置文件碎片 |
| `five-novel-architectural-frontiers.md` 方向一 | 零依赖约束的成本（手写 YAML 解析器已出 bug） | **双解析器交叉验证的缺失**（非单解析器的 bug） |

---

## 方向五 · 工作流资产 Schema 字段漂移：「ADDED HERE ONLY」模式的技术债

**优先级**: 🟠 P2 | **类别**: 架构 · 声明-实现间隙 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐

### 问题描述

`asset.Phase` 结构体包含多个标记为「ADDED HERE ONLY」的字段——它们在 JSON 解码时被正确解析和携带，
但没有任何代码消费它们。这个模式当前应用于 4 个字段（且还在增长）：

| 字段 | 声明位置 | 消费状态 | 注释原文 |
|------|---------|---------|---------|
| `RequiresTools` | `asset.go:121` | 未消费 | `ADDED HERE ONLY: this field is decoded and carried on Phase, but nothing in forge-core reads it yet` |
| `Readonly`（Phase） | `asset.go:131` | 未消费 | `ADDED HERE ONLY: this field is decoded on both Phase and Workflow, but nothing in forge-core enforces it yet` |
| `Readonly`（Workflow） | `asset.go:288` | 未消费 | `ADDED HERE ONLY: this field is decoded, but nothing in forge-core enforces it yet` |
| `SecondaryTemplate` | `asset.go:141` | 未消费 | `ADDED HERE ONLY: this field is decoded and carried on Phase, but nothing in forge-core reads or injects it yet` |

### 为什么这是问题

这不仅仅是「TODO」——它表示 **asset schema 的声明表面大于运行时的实际执行集**。具体风险：

**1. 工作流 YAML 中写实的字段被静默忽略**

`.agent/workflows/review.yml` 中 `performance-reliability-review` phase 声明了
`secondary_template: 06-production-readiness.md`——这个字段被加载到 Go 结构体中，
但 `buildPromptWithEmits` 的 `appendArtifactContext` 函数**永不读取它**。
代理永远不会得到第二个模板。工作流的意图丢失而没有任何警告。

**2. schema drift 无检测**

当 YAML schema 演化时（新增字段、修改现有字段），Go struct 和 YAML 文件之间的一致性
没有自动化检测。`check.py` 的 `check_workflow_phase` 验证 `agent`/`description`/`required_gates`
等核心字段，但对 `secondary_template`、`requires_tools`、`readonly` 这些可选字段不做语义验证。

**3. 新加入的开发者无从知晓「哪些字段真的在工作」**

如果有人在 review.yml 中设置 `secondary_template`，期望代理得到两个审查模板，
但没有代码消费它——他们只能从代码注释（而不是从运行行为）发现这个差距。
这是「声明但未实现」的一个独特子类：不是管线组合（如 reviewer loop-back），
而是**结构体级别的声明-实现间隙**。

### 代码级证据

**1. `appendArtifactContext` 只消费 `UsesTemplate`，不消费 `SecondaryTemplate`**

```go
// forge-core/cmd/forge/prompt_artifacts.go
func appendArtifactContext(ctx []string, repoRoot string, emitsFiles []string, usesTemplate, secondaryTemplate string) []string {
    // 处理 emits... 
    if usesTemplate != "" {
        // 读模板并注入...
    }
    // secondaryTemplate 未被读取
    return ctx
}
```

**2. `requires_tools` 被加载但只被 consumesToolsGuard 用于提示——没有实际的工具探测或执行拦截**

```go
// forge-core/cmd/forge/prompt_context.go:420-428
// requiresToolsGuard implements discover.yml's requires_tools degrade-and-flag
// contract... forge-core has NO live tool probe, so this decides from STATIC
// executor config alone...
func requiresToolsGuard(p asset.Phase, isCommandExec, isClaude bool, allowedTools, logln func(string)) string {
    if len(p.RequiresTools) == 0 {
        return "" // no declared tool requirement: no-op
    }
```

即使 `requires_tools: [web_search]` 声明了，系统也**不验证工具是否实际可用**——
它只从静态配置推断「如果有 executor + claude + allowedTools 包含关键词 → 认为可用」。

**3. 没有 schema 校验命令**

`forge validate` 子命令存在（`cmd/forge/validate.go`），但它只验证 checkpoint、memory 等运行时状态——
没有 `forge validate --workflow-schema` 命令来检查 YAML 字段是否被当前 forge-core 版本实际消费。
也没有 `forge validate --consumed-fields` 来对比 asset schema 与运行时实现。

### 边界情况

| 场景 | 问题 | 严重性 |
|------|------|--------|
| YAML 写了 `secondary_template` 但 forge-core 不读 | 代理看不到第二个模板 | 静默功能缺失 |
| YAML 写了 `requires_tools` 但工具实际不可用 | 声明=需要，运行时=无 | 声明-实现漂移 |
| Workflow 从旧版本升到新版本，新增字段从「ADDED HERE ONLY」→「已消费」 | 中间版本没有人知道哪些字段在哪个版本开始生效 | 版本依赖 |
| 新 agent 卡引用 `requires_tools` 但不理解它其实不执行 | 设计假定了「这个字段存在且可消费」 | 误导 |

### 差异化证明

| 已有分析 | 覆盖内容 | 本方向新增 |
|----------|---------|-----------|
| `execution-semantic-gaps.md` 方向三 | phase 声明的「输出描述」未被消费（Emits） | **schema 字段级别的声明-实现间隙**（系统化的 "ADDED HERE ONLY" 审计） |
| `FUNCTIONAL_REQUIREMENTS_AUDIT.md` | 功能级别的 DONE/GAP 清单 | **struct field 级别的消费清单**（非功能级别） |
| `forgotten-five-meta-governance.md` 方向五 | governance 声明 vs 实现漂移（YAML/check.py） | **Go struct field 级别的漂移**（同一个问题，更低一层） |
| Sprint 12 审计 | 「声明 vs 实现」系统审计 | 本方向是将审计模式**持续化、自动化**的建议（非一次性的审计事件） |

---

## Top-N 推荐

| 优先级 | 方向 | 为什么此时做 |
|--------|------|-------------|
| **P1** 🥇 | 方向三 · Agent 输出契约验证脆弱性 | 最危险——解析器静默失败，信号丢失无告警。影响 reviewer loop-back、confidence-based converge、cost budget 三条 load-bearing 路径。修复成本低（添加 schema 定义 + 验证层 + 测试），杠杆最高。 |
| **P1** 🥇 | 方向四 · 双解析器静默语义漂移 | 潜在的正确性漏洞——当 Go 和 Python 解析器对合法 YAML 输出不同 JSON，系统静默接受其中一个而不知。长期治理依赖于资产加载的正确性，这个正确性目前无人验证。 |
| **P2** 🥈 | 方向二 · AgentExecutor 生命周期真空 | 不影响当前功能（DryRunExecutor 不需要生命周期），但架构性债——当第一个非 dry-run executor 接入时必须解决。建议在接入前完成。 |
| **P2** 🥈 | 方向一 · Gate loop-back 重跑浪费 | 影响不大（每次浪费几秒到几十秒）但在多 loop-back 场景下线性累积。改进收益是墙钟优化和资源节约。 |
| **P2** 🥈 | 方向五 · ADDED HERE ONLY 字段漂移 | 治理 hygiene 问题——不影响当前运行，但长期使 schema 膨胀不可控。建议建立「每个 field 必须在发布前被消费」的 gate。 |

## 不做的诚实说明

以下看似「缺口」的方向在现有 120+ 分析中已被充分覆盖，本文不再重复：

1. **Memory.jsonl 全量重读**：cache 在单次 run 内有效（mtime 不变），仅 evolve iteration 边界触发一次 invalidate。当前 overhead 可忽略。
2. **Cost 微美元精度**：`math.Round(usd * 1e6)` 已经纠正了截断问题。已有 `SpentUsdMicros` 和 `costEmitter` 都正确使用 `math.Round`。
3. **Mode gating 产生零 gate 的执行**：`GatesGreen` 的 vacuous-green guard（`provenCount == 0 → false`）已在 converge 层捕获此情况。
4. **并行编排的语义鸿沟**：RunParallel 与 RunFrom 的差异已在代码注释和 parallel.go 文档中诚实列出，作为 opt-in 的已知 trade-off。
