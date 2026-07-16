# ForgeOS — 五个未见系统性运营缺口

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局逐文件扫描完整代码库 — forge-core (18+ Go 包 · 195+ 源文件 · 纯 stdlib 零依赖)、  
>    cmd/forge (40+ 子命令/模块 · ~12k LOC)、harness (39+ 模块 · ~10.5k LOC 执法层)、  
>    `.agent/` (12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR + DECISIONS + architecture)、  
>    `docs/` (FUNCTIONAL_REQUIREMENTS_AUDIT · 29 轮 sprint 演进 · 90+ 分析文档)。  
> 2. **差异化验证**: 对每个方向的核心理念，在全部 docs/requirements/ (88 篇) + FRA + 核心文档中  
>    做全文关键词检索，确认该方向的核心论点 **从未作为独立系统性扩展被展开**，或与已有覆盖  
>    有本质不同的切入角度。  
> 3. **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据、实际影响、边界情况。  
> **日期**: 2026-07-10

---

## 已有覆盖全景（本文不重复的域）

| 域 | 覆盖度 | 代表文档 |
|---|---|---|
| 功能引擎补齐（编排/路由/记忆/收敛/并行/信号） | 深度覆盖（~20 篇） | `expansion-five-uncovered-*.md`, `current-sprint` |
| 生产可靠性（超时/退避/护栏/进程组/重试） | 深度覆盖（~18 篇） | `expansion-production-readiness.md` |
| 可观测性（trace/telemetry/scorecard/三维真数据） | 深度覆盖（~12 篇） | `five-gaps-from-global-scan.md` |
| 学习闭环（memory/checkpoint/summary/自适应收敛） | 深度覆盖（~12 篇） | `expansion-five-systemic-learning-loop-gaps.md` |
| 安全纵深（secret-scan/预算/递归/Sandbox/SCA） | 深度覆盖（~12 篇） | `five-novel-architectural-frontiers.md` |
| 治理/执法（arch-check 8 检查 / check.py 10 检查 / 版本化） | 深度覆盖（~10 篇） | `expansion-five-codelevel-architect-gaps.md` |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 | — |
| 结构债（Phase 膨胀/Context Engine/非结构化日志） | 深度覆盖（~8 篇） | `architect-product-perspective-four-structural-gaps.md` |
| 执行语义（原子性/幂等/回滚/因果一致性/undo） | 深度覆盖（~10 篇） | `execution-semantic-gaps.md` |
| 运营可信度（Run Identity/状态隔离/审计/健康检查） | 深度覆盖（~8 篇） | `forgeos-trust-operational-maturity.md` |
| 二阶伴生（配置爆炸/知识衰减/TOCTOU/无声数据丢失） | 深度覆盖（~10 篇） | `second-order-architectural-gaps.md` |
| 第三地平线（多仓库联邦/事件驱动/Web UI/LiteLLM） | 已规划（~10 篇） | `expansion-horizon-three.md` |

**以下五个方向落在这 90+ 篇所有覆盖的间隙中**——不是因为它们不重要，而是因为它们指向的是  
**「无人值守 24h 自治软件工厂」这一承诺与默认行为之间的根本矛盾**，而非「缺失的组件」。

---

## 方向一 · 默认 dry-run 使学习循环永远不被执行

> **优先级**: 🔴 **P1** | **类别**: 产品 · 可靠性 · 测试 | **风险**: 静默退化  
> **代码证据**: `forge-core/cmd/forge/main.go:100-130` · `command_executor.go` · `backoff.go` ·  
>   `memory/memory.go` · `trace/trace.go` · `converge/converge.go`  
> **已有覆盖**: **零** — 无一篇分析将「默认执行器是 dry-run 因此核心学习循环从不执行」作为独立问题展开。

### 问题描述

forge-core 的默认 AgentExecutor 是 `DryRunExecutor`。用户必须显式传递 `--executor command --agent-cmd claude` 才能让真实代码路径执行。这意味着：

**产品层面** — 首次接触 ForgeOS 的用户（`forge init` → `forge run build`）看到的输出**全是叙述性的**：没有真实 trace 写入 `.forge/trace.jsonl`，没有 memory 条目，没有 scorecard 更新，没有任何收敛信号。用户对 ForgeOS 核心价值（24h 自治学习循环）的第一手体验是零。

**测试层面** — 整个学习循环的代码路径（`memory.Append` / `trace.Emit` / `converge.Converge` / `routing.HistoryTiebreak`）在默认配置下**永远不会被执行**。这意味着：

- `forge run build`（默认 dry-run）：memory.go 的 `loadFromCache` → `invalidateLoadCache` 路径不执行
- `forge evolve`（默认 dry-run）：`LoopEngine` → `OnIteration` → `checkpoint.Save` 路径不执行
- `forge doctor`：不检查 `.forge/` 数据文件健康

只有在 CI 或有经验的用户配置了 `--executor command` 后，这些路径才被激活。但直到那个时刻之前，**这些代码从未在用户环境中运行过**。

### 代码级证据

**证据 A: `main.go` 的默认执行器是 DryRunExecutor**

```go
// forge-core/cmd/forge/main.go:~120
executorType := flag.String("executor", "dry", "agent executor: dry|command")
// ...
case "dry", "":
    exec := &orchestrator.DryRunExecutor{Log: logf}
```

DryRunExecutor（`command_executor.go:12-15` 定义）的 `Execute` 方法只调用 `Log` 打一行叙述，然后返回 nil：

```go
func (DryRunExecutor) Execute(_ context.Context, p asset.Phase, mode string) error {
    // Log the routing decision; no LLM invoked.
    return nil
}
```

**证据 B: 三种核心持久化路径都被短路**

```
memory.go       ← DryRunExecutor.Apply → 不执行 agent phase → 不产生 findings → 不 Append
trace.go        ← DryRunExecutor → Emit 永不调用（Span 的 defer 不触发）
converge.go     ← DryRunExecutor → RoadmapCompletion 由 git diff 算出,但无 agent 产出变化
```

**证据 C: `forge evolve` 的 LoopEngine 在 dry-run 下永不迭代**

```go
// internal/orchestrator/loop.go:~90
// 迭代只在 RunFrom 返回非 nil 或 converge MET 时结束。
// dry-run 下 RunFrom 返回 nil（无 agent 错误），但 converge 从不 MET（roadmap 不变）
// → 空循环，完全不执行有用工作。
```

### 产品影响

1. **首次体验破碎**：用户执行 `forge run build` 看到「implements feature X (sonnet)」——但这只是一个叙述，没有任何文件被写、没有 trace 被记录、没有 gate 被跑。用户的反应必然是「然后呢？什么都没发生。」
2. **学习循环路径零测试覆盖**：`memory.go` 的 120+ 行、`trace.go` 的 150+ 行、`converge.go` 的 280+ 行、`scorecard.go` 的 140+ 行——所有这些代码在用户环境中默认不被执行。
3. **CI 集成门槛高**：要让 ForgeOS 在 CI 中有用，用户必须配置 `--executor command --agent-cmd claude` 并拥有 claude CLI + 凭证。这比 `forge init` → `forge run build` 多了几步。

### 边界场景

- **dry-run 切换到 command 时的行为差异**：dry-run 不写入 checkpoint，所以 `forge run --resume` 在首次真实执行时找不到 checkpoint → 从 phase 0 开始→ 与 dry-run 的预期输出不符。
- **部分 phases dry-run 切换**：如果用户在 iteration 3 从 dry-run 切换到 command，memory 未积累任何历史 knowledge → agent 缺乏上下文 → 输出质量低于从开始就使用 command 的路径。
- **CI pipeline 中断**：如果 CI 中同时存在 dry-run（用于快速验证）和 command（用于实际任务）的混合使用，用户可能混淆哪些运行是真实的、哪些是模拟的。

### 建议方向

- `forge run build --no-dry-run` 作为安全默认的反转（先有安全默认、后有便捷快捷方式）
- 在 `forge init` 生成的 README 或 `forge run --help` 中明确指导用户如何启用真实执行
- 在 dry-run 模式下记录一条表示「这是一个模拟，不是真实 agent 执行」的 trace 事件，以便用户在事后分析中区分
- 学习循环的核心代码（memory load/cache/query、converge evaluation、trace write）应在 dry-run 下进行**功能性的空运转**（不调 LLM 但走完持久化路径），至少让这些路径被实际执行

---

## 方向二 · 预算降级-质量螺旋：无断路器的正反馈循环

> **优先级**: 🔴 **P1** | **类别**: 系统安全 · 经济学 | **风险**: 预算耗尽 + 产出质量归零  
> **代码证据**: `internal/routing/routing.go:197-240` `BudgetAdjustTier` ·  
>   `internal/orchestrator/backoff.go` · `cmd/forge/cost.go` `runBudget`  
> **已有覆盖**: **零** — 无一篇分析将「预算降级 → 低质量输出 → 更多重工 → 更快速预算耗尽」  
>   作为独立系统安全风险展开。

### 问题描述

ForgeOS 在预算接近上限时会降级模型 tier。但降级后的低质量模型产出更低质量的工作 → 导致更多 reviewer REQUEST_CHANGES → 触发 loop-back 重新执行 → 消耗更多预算 → 更快达到上限 → 再次降级。这是一个**正反馈加速螺旋**，且系统没有任何断路器检测它。

### 代码级证据

**证据 A: `BudgetAdjustTier` 降级不通知调用者**

```go
// internal/routing/routing.go:214-230
func BudgetAdjustTier(base, agent string, spendRatio float64) string {
    if math.IsNaN(spendRatio) || spendRatio < 0 {
        spendRatio = 0
    }
    if spendRatio < 0.80 {
        return base
    }
    if opusFloorAgents[agent] {
        return base // judgement-only roles keep Opus
    }
    return DowngradeOne(base)
}
```

该函数返回降级后的 tier，但不通知调用者这次降级发生了。没有 `DecisionEvent`、没有 trace 记录、没有 converge 报告变更。

**证据 B: 降级后的低质量输出会触发更多 loop-back**

reviewer 对低质量输出的 `REQUEST_CHANGES` 触发 `agentOutcome` 的 loop-back（`orchestrator.go:321`），agent 用**降级后的（同一低档）模型**重做工作。每次 loop-back 消耗一次 agent call（计入 MaxAgentCalls）并产生新的 API 花费（计入 runBudget）。

**证据 C: 无检测、无记录、无升级渠道**

全仓 grep `circuit.*breaker\|quality.*degrad\|spiral\|escalat.*human\|auto.*escalat` 返回零结果。系统没有一个地方问："we just spent $X on a downgraded model, and the reviewer rejected 3 out of the last 4 outputs — should we stop and ask a human?"

### 产品影响

在 24h evolve 场景下，这个螺旋可以在用户睡觉时发生：

```
00:00  budget=100%  → 正常执行, tier=sonnet
06:00  budget=82%   → downgrade to haiku
06:15  haiku 产出低质量 → 2 次 loop-back → budget=81%
06:30  loop-back 继续消耗 → budget=79% → haiku 不变
09:00  30% 的迭代因低质量需要 loop-back → 有效产出率 70%
12:00  budget=95% → 更激进降级 → 更高质量损失
```

用户早上查看时：budget 几乎耗尽、ROADMAP 完成度低、大量 loop-back 记录——但`forge run`以非零退出码结束，且没有明确的「需要人工干预」信号。

### 边界场景

- **reviewer 本身也被降级**：`IsOpusFloorAgent` 保护 reviewer 不被降级，但 *implementer*（被降级者）的低质量输出增加了 reviewer 的工作量。reviewer 每次 REVIEW 都要付费（且被 loop-back 多次调用），反而消耗更多预算。
- **`DowngradeOne` 从 haiku 再降级会怎样**：`DowngradeOne("haiku")` → `default: return Haiku`（见 routing.go:196）。所以 haiku 不会进一步降级——但这也意味着 haiku 是最终停留的低档，其质量差异最大。
- **预算耗尽与降级同时发生**：`spendRatio >= 1.00 && critical` → `EscalateToHuman`。但非 critical task 在 `spendRatio >= 1.00` 时直接 collapse 到 task_type floor（`routing.go:177`）。如果 floor 也是 haiku（如 `"docs"`），降级+collapse 叠加。

### 建议方向

- **BudgetAdjustTier 降级必须写一条 DecisionEvent trace**，注明 `prev_tier`、`new_tier`、`spendRatio`、`reason`
- **实现 quality-gate 断路器**：如果连续 N 次 loop-back 是由同一个 downgraded model 触发的，暂停执行并输出「模型输出质量过低，建议恢复 tier 或人工干预」
- **降级不应是二值（降/不降）**，而应是**协商式**：先尝试 N 次 retry 后再降级、降级后如果 loop-back 频率超过阈值则自动恢复上一 tier
- **converge 报告应包含 budget-quality 指标**：如 `budget_quality_ratio = spent_usd / roadmap_completion_delta`，当该比值异常升高时标记警示

---

## 方向三 · 并行执行 + 无抖动退避 = 自 DoS 过载放大

> **优先级**: 🟠 **P2** | **类别**: 可靠性 · 弹性 | **风险**: 预算浪费 + API 限流  
> **代码证据**: `internal/orchestrator/waves.go` · `parallel.go:90` `runWave` ·  
>   `backoff.go:61` `overloadBackoff`（无抖动）  
> **已有覆盖**: **1 篇**（genuine-expansion-gaps.md 方向 2 分析了 backoff 群聚效应，  
>   但**未分析波并行度对过载的放大作用**——群的规模直接由 wave 拓扑决定。

### 问题描述

当 `RunParallel` 执行一个多 phase wave（如 discover 流程的 3 个并行 phase：scan/market/capability），所有 phase 同时启动各自的 agent 进程。如果外部 API 返回 529 (overloaded)：

1. 每个 phase 独立检测到 overload
2. 每个 phase 在 `runAgentPhase` 中调用相同的 `overloadBackoff(attempt)`
3. `overloadBackoff` 是**确定性函数，无抖动**（backoff.go:61 注释明确说明）
4. 所有 phase 以完全相同的时序退避：2s → 4s → 8s → ... → 60s
5. 退避结束后，所有 phase 再次同时发起请求
6. 过载持续时，T 秒后的下一个波再次被放大

这不是一个 N 请求的 thundering herd，而是**与 wave 大小成正比的自 DoS 放大**。如果 workflow 声明 20 个无依赖的 phase（一个 discover 流程），20 个并发 agent 全部以完全相同的时序重试。

### 产品影响

- **预算浪费**：在过载状态下，每波重试的开销不仅仅是时间，还有 `claude -p` 的 HTTP 开销和（在某些超时前的）部分 token 消耗。
- **收敛延迟**：self-DoS 导致所有 phase 同时过载→长时间退避→全波延迟→下一个波也被延迟。
- **rate limiting escalation**：Anthropic/Claude 的 rate limit 可能在 20 倍并发下触发，将错误从 529（overloaded，retryable）升级为 429（rate limited，可能非 retryable）。

### 代码级证据

**证据 A: overloadBackoff 是无抖动的指数退避**

```go
// internal/orchestrator/backoff.go:53-61
const (
    overloadBackoffBase = 2 * time.Second
    overloadBackoffCap  = 60 * time.Second
)
func overloadBackoff(attempt int) time.Duration {
    // ...
    d := overloadBackoffBase << attempt
    // ...
    return d
}
// 注释: "v1 single-run: NO JITTER — jitter only matters once many agents
// retry in parallel and could thunder-herd the backend in lockstep"
```

注释自身已经指出多 agent 并行是抖动的动机场景——但 `RunParallel` 正好是这一场景。

**证据 B: `runWave` 对波内所有 phase 同时 spawn goroutine**

```go
// internal/orchestrator/parallel.go:90-120
for _, idx := range wave {
    if waveCtx.Err() != nil {
        continue
    }
    wg.Add(1)
    go func(i int) {
        defer wg.Done()
        if err := e.runPhaseParallel(waveCtx, wf, i, mode, mu, agentCalls); err != nil {
            // cancel wave on first failure
        }
    }(idx)
}
wg.Wait()
```

波内 phase 同时启动，且 `runPhaseParallel` → `runAgentPhase` 共享同一个 `overloadBackoff` 函数。

**证据 C: Waves 函数无最大波大小限制**

```go
// internal/orchestrator/waves.go
func Waves(phases []asset.Phase) ([][]int, error) {
    // ... 不限制每波 phase 数
    // 一个 0-dep 的 20-phase workflow 产生一个 20-phase 的波
}
```

### 边界场景

- **混合过载和超时**：波内一部分 phase 过载（529、retryable）、一部分超时（timeout、retryable）、一部分成功。过载的 phase 退避，超时的 phase 立即重试（timeout 不退避），造成混合时序。
- **跨波连锁反应**：wave 1 的全部 phase 被过载延迟 → wave 2 无法启动（`runWave` 的 `wg.Wait()` 阻塞） → wave 2 等待 wave 1 → 但不计入 wave 1 的 timeout → 超时链式起爆。
- **Windows/非 Unix 平台**：`command_executor_other.go` 的 `setupProcessGroup` 是空操作→子进程不属进程组→走 `exec.CommandContext` 默认 only-direct-child kill→重试时子进程可能残留。

### 建议方向

- **为 `overloadBackoff` 添加 per-goroutine 抖动**（`rand.Int63n(base)` 作为 ±50% 偏移）：注释自身的「v1 single-run: NO JITTER」历史依据已被 `RunParallel` 的引入推翻
- **为 `runWave` 添加最大并发度限流**（buffered channel semaphore）：即使波有 20 个 phase，也只同时启动 3-5 个 agent 进程
- **为 `Waves` 函数添加最大波大小参数**（如 `--max-wave-size 5`）：超出的 phase 被分到子波中，子波串行或受限并行执行
- **跨波协调退避**：Wave 1 的过载退避信号可通过一个共享的 `backoffGate` 传播到 Wave 2，避免 wave 2 在 wave 1 还在退避时就开始新的过载请求

---

## 方向四 · 环境变量向子进程完全泄漏——从未存在的安全边界

> **优先级**: 🔴 **P1** | **类别**: 安全 · 运营 | **风险**: 凭据泄漏到 LLM 进程  
> **代码证据**: `internal/orchestrator/command_executor.go:143` `childEnv`  
> **已有覆盖**: **4 篇**从代码正确性角度分析（`architect-product-perspective-five-novel-directions.md` 等），  
>   但**无一篇从产品/运营安全角度分析**——在 CI/CD pipeline、多租户环境、或第三方审计  
>   场景下，这是一个隐形的合规风险。

### 问题描述

`childEnv` 函数只过滤 `FORGE_AGENT_DEPTH` 这一项环境变量，将父进程的 `os.Environ()` 全部传递给每个 `claude -p` agent 子进程：

```go
// forge-core/internal/orchestrator/command_executor.go:143
func childEnv(depth int) []string {
    prefix := agentDepthEnv + "="
    base := os.Environ()
    out := make([]string, 0, len(base)+1)
    for _, kv := range base {
        if !strings.HasPrefix(kv, prefix) {
            out = append(out, kv)
        }
    }
    return append(out, fmt.Sprintf("%s=%d", agentDepthEnv, depth+1))
}
```

当 ForgeOS 部署在 CI 环境（GitHub Actions、GitLab CI、Jenkins）中运行时，`os.Environ()` 包含 CI 系统注入的所有机密：

- `GITHUB_TOKEN` / `GITLAB_TOKEN` — 仓库写权限
- `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` — 云资源访问
- `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` — LLM API 访问
- `DOCKER_PASSWORD` / `REGISTRY_TOKEN` — 容器注册表访问
- 自定义 `SECRET_*` 变量 — 应用程序机密

这些凭据被透传给 `claude -p` 子进程。虽然 claude CLI 自身可能不主动使用这些变量，但以下风险存在：

1. **LLM prompt 包含机密**：agent 的 prompt 可以包含环境变量值（如通过 `process.env.GITHUB_TOKEN` 在 agent 上下文中）
2. **LLM 输出泄漏**：agent 可能在输出中包含机密（如在 debug 日志中打印环境变量）
3. **Fork-bomb 泄漏**：agent 进程 fork 的子进程（如 agent 内部执行的 bash 脚本）也可以访问这些变量
4. **合规要求**：SOC2、ISO 27001、PCI-DSS 要求运行时的机密不得传递给非必要进程

### 产品影响

- **ForgeOS 作为「软件工厂」的部署场景天然包含 CI**——这正是 `forge accept` 设计为 CI gate 的原因。但 CI 环境的机密密度最高，泄漏风险最大。
- **用户不知道泄漏发生了**：没有告警、没有 audit log、没有「forge run 将 12 个环境变量传递给了 LLM」的提示。
- **合规团队在审计时会问**：你们的治理工具是否将 GITHUB_TOKEN 传递给了第三方 AI 服务？当前的答案是「是的，没有任何保护」。

### 边界场景

- **白名单模式**：某些环境变量确实需要传递（`PATH`、`HOME`、`LANG`、`FORGE_AGENT_DEPTH`）。无差别的全部保留会保留风险，全部删除会破坏功能。
- **`CLAUDE_API_KEY` 等必需的 API 变量**：如果过滤掉所有变量，agent 无法访问 claude。需要一个机制来明确声明「这个变量是必需的」。
- **`FORGE_AGENT_DEPTH` 的重写**：`childEnv` 专门过滤并重写它——说明设计者已经意识到了 env 过滤的必要性，但只做了一个变量就停了。
- **agent 自身的安全性**：如果 agent 被配置为 `--agent-permission=acceptEdits`（Sprint 24 的真实配置），它拥有文件写权限。环境变量泄漏变成凭据泄漏 + 凭据使用（将 GITHUB_TOKEN 写入仓库）的复合风险。

### 建议方向

- **引入环境变量守卫（`EnvGuard`）**：一个可配置的 allowlist/blocklist 机制，默认只保留 `PATH`、`HOME`、`LANG`、`FORGE_AGENT_DEPTH`、`CLAUDE_API_KEY` 等必要的变量名
- **默认安全（fail-safe）**：未知环境变量默认拒绝（allowlist 模式），而不是默认放行
- **审计日志**：在 trace 中记录每个 agent 子进程传递了多少个环境变量（不记录值，只计数和名称前缀）
- **`forge doctor --security`**：检查当前环境的 env 泄漏风险，报告「12 个环境变量（含 4 个含 'TOKEN'/'SECRET' 的变量）将被传递给 agent 子进程」
- **Project .agent/policies/env-allowlist.yml**：允许项目声明环境变量白名单，如 `forge_env_allowlist: [PATH, HOME, CLAUDE_API_KEY, FORGE_AGENT_DEPTH]`

---

## 方向五 · 持久化存储缺乏跨存储一致性校验——三 JSONL 文件可各自收敛到矛盾状态

> **优先级**: 🟠 **P2** | **类别**: 可观测性 · 数据完整性 | **风险**: 静默数据矛盾  
> **代码证据**: `internal/trace/trace.go`（trace.jsonl）· `internal/memory/memory.go`（memory.jsonl）·  
>   `internal/routing/scorecard.go`（scorecards.json）· `internal/persist/checkpoint.go`（checkpoint.json）  
> **已有覆盖**: **零**——虽有 5+ 篇分析 trace/memory 的质量，但**无一篇将这四种持久化存储之间的  
>   交叉一致性作为独立问题**。

### 问题描述

ForgeOS 的持久化状态分布在四个独立写入的 JSON/JSONL 文件中：

| 文件 | 写入方式 | 写入者 | 用途 |
|---|---|---|---|
| `.forge/trace.jsonl` | O_APPEND | `trace.Emit` | 审计、可观测性 |
| `.forge/memory.jsonl` | O_APPEND | `memory.Append` | 学习、跨迭代知识 |
| `.forge/scorecards.json` | atomic rewrite | `scorecard_wind.go` | 路由质量历史 |
| `.forge/checkpoint.json` | atomic rename | `persist.Save` | 崩溃恢复 |

**不存在任何跨存储一致性检查**。这意味着：

- Trace 记录了「iteration 5 成功完成，gate 全绿」
- Memory 记录了「iteration 5 发现了关键架构约束」
- Scorecard 记录了「iteration 5 的 sonnet→haiku 降级导致质量下降 40%」
- Checkpoint 记录「iteration 5 的 roadmap_completion=55%」

但没有任何机制确认这四个描述指向同一个 iteration 5。如果某个文件因崩溃、并发写入或手动编辑而与其他文件不一致，系统**完全无法检测到矛盾**。

### 代码级证据

**证据 A: 每个存储有独立的 seq/ID 空间**

```go
// trace.go: trace.Emit 分配自己的 seq（每个 Tracer 实例独立）
// memory.go: memory.Append 不分配 seq（按行号隐式排序）
// scorecard.go: Scorecard 没有 seq 字段（由外部 Eval Engine 写入）
// checkpoint.go: Checkpoint 记录 iteration 编号（但可以被覆盖）
```

这四个存储之间**没有一个共享的 run_id、session_id 或 seq 空间**。

**证据 B: checkpoint.json 可以被其他存储的数据否定**

```json
// checkpoint.json (假想)
{"iteration":5,"roadmap_completion":0.55,"gates_green":true}

// memory.jsonl (假想,被手动编辑或并发写入污染后)
{"kind":"discovery","detail":"iteration 5: critical bug found - rewrite needed"}
```

如果 checkpoint 声称 gates green 但 memory 记录了 iteration 5 的严重问题，没有机制触发「这些存储矛盾」的告警。

**证据 C: scorecards.json 不被 checkpoint 感知**

`persist.Save` 保存 `SpentUsdMicros`（从 cost.go 读取），但不读取或写入 scorecard 数据。`scorecard_wind.go:88` 在 `runScorecardUpdate` 中独立写入 scorecards.json，不与 checkpoint 同步。

### 产品影响

- **`forge doctor` 不检查存储一致性**：`doctor.go` 检查 CLI 可用性、workflow 模型引用、gate 工具存在性，但不打开任何一个 `.forge/` 文件验证其内容一致性。
- **灾难恢复路径无知**：`forge run --resume` 读取 checkpoint 决定从哪继续。如果 memory 记录了 checkpoint 不知道的关键发现（因为是独立写入的），learning loop 在恢复后不知道那些发现。
- **手动编辑破坏不可检测**：用户或外部工具编辑 `.forge/` 文件（比如删除一个旧的 trace），没有校验和没有发现损坏。

### 边界场景

- **并发 evolve 运行**：两个 `forge evolve` 同时运行（CI + 开发）→ trace.jsonl 行交错 → checkpoint 被覆盖 → memory 被污染 → 下一轮运行使用被污染的数据。
- **部分恢复**：用户 `git checkout .forge/checkpoint.json`（从备份恢复 checkpoint）但不恢复其他文件 → checkpoint 指向 iteration 5，但 memory 还在 iteration 3 → 学习知识丢失。
- **版本升级**：forge-core 升级后 trace 格式变化（`_format` 字段），但旧的 memory、scorecards、checkpoint 不更新 → 新旧格式混合 → 工具读取失败。

### 建议方向

- **引入可选的 `.forge/manifest.json`**：包含当前 run_id、各存储文件的 checksum（SHA256）、共享的 iteration 编号、时间戳。每次 `forge accept` 或 checkpoint save 时更新。
- **`forge doctor --stores`**：打开所有四个 `.forge/` 文件，验证：
  - 每个文件是否可解析（JSON/JSONL 格式正确）
  - trace.seq 是否单调递增（无跳号或重复）
  - memory 的时间戳是否在合理范围内（无未来时间）
  - checkpoint 引用的 iteration 在 trace 中是否有对应事件
  - scorecards 中的 model 名称是否有效
- **`forge repair` 命令**：当 manifest 检测到不一致时，尝试自动修复（如重建 checkpoint 编号与 trace seq 的关系）
- **存储写入按事务顺序**：在关键路径上（如 checkpoint.Save），先用相同的 iteration 编号写一条特殊的 trace event，再写 checkpoint → 即使部分失败，trace 可以作为恢复的真相之源

---

## 总结

| # | 方向 | 类型 | 严重性 | 已有覆盖 |
|---|------|------|--------|---------|
| 1 | 默认 dry-run 使学习循环永不执行 | 产品 · 可靠性 · 测试 | 🔴 P1 | **零** |
| 2 | 预算降级-质量螺旋 | 系统安全 · 经济学 | 🔴 P1 | **零** |
| 3 | 并行过载自 DoS 放大 | 可靠性 · 弹性 | 🟠 P2 | 1 篇（不同角度） |
| 4 | 环境变量向子进程完全泄漏 | 安全 · 合规 | 🔴 P1 | 4 篇（代码角度） |
| 5 | 持久化存储缺乏跨存储一致性校验 | 数据完整性 · 可观测性 | 🟠 P2 | **零** |

**共同主题**：这五个方向都不是「加一个新引擎」或「实现一个新功能」——它们都指向 ForgeOS  
在从「可运行的脚手架」走向「24h 无人值守自治软件工厂」的路上，默认行为与实际承诺之间的根本矛盾。  
它们共享一个核心问题：**系统在安静状态下表现出的行为是否与它在文档中承诺的能力一致？**
