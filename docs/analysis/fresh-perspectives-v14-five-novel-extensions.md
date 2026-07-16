# ForgeOS — 第 14 轮扩展方向分析：五个新鲜架构视角

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库深扫（forge-core 15 Go 包 + cmd/forge 18+ CLI 命令 + harness 26+ 模块 +  
>   `.agent/` 完整治理骨架 + examples/ + 全部 30+ 份已有 docs/analysis/ + 最近 commit 含 Adaptive Assembly/Reflect）  
> **基线**: Sprint 26 全状态 + `feat: Loop Memory/Learning + Adaptive Assembly + Reflect` (b0c80e4)  
> **纪律**: **绝不与任何已有分析文档的核心论点重叠**。每方向标注「已有覆盖」以证明新颖性。不写代码。  
> **日期**: 2026-07-01

---

## 已有分析覆盖域速查（本文不再重复）

此前 30+ 份分析文档覆盖的领域：

| 已有覆盖域 | 对应文档 |
|---|---|
| 自适应工作流 / 信号驱动编排 | `high-value-extensions.md` 方向一 |
| 闸门自省 / 元学习闭环 | `high-value-extensions.md` 方向二 |
| 增量式治理执行 | `high-value-extensions.md` 方向三 |
| 跨项目知识联邦 | `expansion-gaps-v7-novel.md` 方向一 |
| 运行时模型质量自适应 | `expansion-gaps-v7-novel.md` 方向二 |
| 多租户安全隔离 / Agent 权限 | `high-value-perspectives-v11.md` 方向一 |
| 确定性 Replay 引擎 | `expansion-directions-v4.md` 方向四 |
| Memory 衰减 / 去重 / 可溯源 | `high-value-perspectives-v11.md` 方向四 |
| 并行 fail-fast 短路 | `edgecases-and-perf.md` §1.1 |
| 配置表面积 | `configuration-surface-and-adoption.md` |
| ADR 决策衰退 | `eighth-wave-adr-decay.md` |
| 长时数据生命周期 | `fresh-scan-strategic-expansion.md` 方向一 |
| YAML-Shim 消除 | `fresh-scan-strategic-expansion.md` 方向二 |
| 跨 Agent Prompt 注入防护 | `expansion-directions-v6.md` 方向一 |
| 置信度感知决策 | `expansion-directions-v6.md` 方向二 |
| 自愈层运行时 | `expansion-directions-v6.md` 方向四 |
| 架构度量趋势 / 早期预警 | `expansion-directions-v6.md` 方向五 |
| 收敛陷阱 / 门闩效应 | `edgecases-and-perf.md` §3 |
| 跨周期收敛状态机 | `expansion-core-five-2026-07-01.md` 方向一 |
| 配置面完整性守卫 | `expansion-core-five-2026-07-01.md` 方向二 |
| SCA 运行时 | `expansion-core-five-2026-07-01.md` 方向三 |
| Phase 级文件系统隔离 | `novel-directions-v13.md` 方向一 |
| 预算规划器 | `novel-directions-v13.md` 方向五 |
| 交互式工作流编排 | `five-extensions-v10.md` 方向一 |
| 检查点 Diff 浏览器 | `five-extensions-v10.md` 方向四 |
| Agent 输出 Schema 执法 | `novel-extensions-v12.md` 方向二 |
| 策略模拟引擎 | `novel-extensions-v12.md` 方向三 |
| 自适应 Assembly(已落地) | commit b0c80e4 |
| Reflect 自分析(已落地) | commit b0c80e4 |
| 冷启动分数卡(已落地) | commit b0c80e4 |
| 信号处理/优雅关闭 | `sprint-27-signal-handling.md` |
| 跨相位故障归因 | `novel-directions-v13.md` 方向三 |
| 执行器多样性 | `novel-directions-v13.md` 方向四 |
| ForgeOS 自身治理差距 | `expansion-forgeos-meta-governance.md` |
| 增长瓶颈/包膨胀 | `growth-bottlenecks-and-scalability.md` |

---

## 本文目录

1. [方向一：多模型共识层——高利害决策的独立交叉验证](#方向一多模型共识层)
2. [方向二：制品级溯源链——从「谁做了什么」到「谁写了哪行代码」](#方向二制品级溯源链)
3. [方向三：相位级前置条件验证——在烧钱之前发现缺了什么](#方向三相位级前置条件验证)
4. [方向四：收敛速度自适应控制——靠近目标时学会小步走](#方向四收敛速度自适应控制)
5. [方向五：执行行为异常检测——从遥测中识别「不对劲」](#方向五执行行为异常检测)

---

## 方向一：多模型共识层

> **高利害决策的独立交叉验证**  
> 类型：安全 · 治理 · 核心架构  
> 代码影响：`internal/orchestrator/` + `routing/` + `converge/` + workflow YAML  
> 紧急度：P1（影响可信度，非立即安全风险）

### 现状

所有关键决策由**单一 agent** 做出，无交叉验证：

| 决策点 | 单一裁决者 | 下游后果 |
|--------|-----------|---------|
| reviewer REQUEST_CHANGES | 一个 reviewer agent | implementer 被定向跳回 |
| architect 架构方案 | 一个 architect agent | 整个 build 阶段以此为基础 |
| cto approve/reject | 一个 cto agent | human_approval 之后唯一放行 |
| risk 分类 | 规则引擎（非 LLM | 单一决策树，无条件分支 |
| REQUEST_CHANGES 后的修复验证 | 同一个 implementer | 无独立验证修复是否正确 |

**代码证据链**：

```go
// internal/orchestrator/orchestrator.go:281
func (e Engine) agentOutcome(wf asset.Workflow, p asset.Phase, loopBacks *int) (target int, jumped bool) {
    if e.AgentVerdict == nil {
        return 0, false // no puller wired: proceed.
    }
    v, ok := e.AgentVerdict(p.Name)
    if !ok || v != reviewerRequestChanges {
        return 0, false // one agent's verdict → entire loop behavior
    }
    return e.loopBackTo(wf, p, loopBacks, "reviewer verdict REQUEST_CHANGES")
}
```

`AgentVerdict` 是一个 puller，只拉一次。没有「第二个 reviewer 确认」或「随机采样审核」。

```go
// internal/routing/routing.go:73-77
var opusFloorAgents = map[string]bool{
    "architect": true,
    "cto":       true,
    "reviewer":  true,
}
```

Opus floor 确保这些角色用最强模型，但**不确保决策被交叉验证**。一个 opus 实例可能犯错，尤其是对同一上下文存在系统偏差（position bias、confirmation bias）。

### 边界情况

| 场景 | 后果 |
|------|------|
| Reviewer 误判了一个非关键问题为 REQUEST_CHANGES | 数十美元和一个迭代被浪费 |
| Reviewer 漏判了一个安全问题 | 安全红线直接通过 |
| CTO 批准了一个有深层架构缺陷的方案 | 下游三个 implementer 和 reviewer 在此基础上工作 |
| 修复验证由同一 implementer 执行 | 修复可能只是掩盖而非根本解决 |

### 建议扩展

**Phase A（轻量）——随机抽样审核**：
- 在 workflow YAML 中新增 `consensus: {min_approvers: 2, independent: true}` 字段
- 对 `risk == critical` 或高价值 phase，自动插入第二个独立 reviewer（fresh-context），比较两份 verdict
- 两份裁决不一致时→ escalate to human（fail-open，永不静默通过）

**Phase B（完整）——共识协议**：
- 引入 `ConsensusEngine` 接口：`type ConsensusEngine func(ctx, phase, taskType, candidates[]) -> (verdict, confidence)`
- 对同一任务路由到两个不同模型（例如 Opus + Sonnet，或 Anthropic + OpenAI）
- 分歧裁决有结构化处理：简单多数、加权投票（按模型历史准确率）、或 escalate
- 共识层结果写回 trace，供后续审计

**Phase C（护城河）——对抗性验证**：
- 对安全/支付等关键变更，第二个 agent 被提示为「反对者」（devil's advocate）
- 专门找第一个方案的漏洞
- 只有「反对者」找不到致命缺陷时才通过

### 不与已有分析冲突的证明

| 相似概念 | 本文的不同之处 |
|----------|---------------|
| Agent 沙箱隔离（expansion-gaps-v7） | 沙箱隔离 agent 的执行环境，不涉及决策交叉验证 |
| Agent 身份/权限模型（v11 方向一） | 权限模型控制 agent 能做什么，不控制多个 agent 的决策一致性 |
| 置信度感知决策（v6 方向二） | 置信度是 agent **自报**的，不是外部交叉验证 |
| Prompt 注入防护（v6 方向一） | 防护 agent 被恶意输入误导，不解决 agent 自身犯错 |
| 自适应工作流（high-value-extensions 方向一） | 调整 ***什么时候*** 运行什么 phase，不改变 phase 内决策方式 |
| 跨厂商模型池（next-horizons 方向一） | 池化解决模型可用性，不是决策可靠性 |

---

## 方向二：制品级溯源链

> **从「谁做了什么」到「谁写了哪行代码」**  
> 类型：可观测性 · 审计 · 运营  
> 代码影响：`trace/` + `cmd/forge/gates.go` + `internal/memory/`  
> 紧急度：P2（长期运营质量）

### 现状

ForgeOS 目前追踪的是 **event 级别的可观测性**：

- `trace.Event` 记录 `{kind: "agent", name: "implementer", phase: "impl-1", duration_ms, cost_usd}`
- `memory.Entry` 记录 `{kind, topic, detail, source, iteration}`
- `phaseOutputLedger` 记录 phase 产出文本

**但无法回答以下问题**：
- `src/payment/checkout.go` 第 42 行的 SQL 注入漏洞是哪个 agent+iteration 写的？
- 这个文件在上次 iteration 和这次之间被改了哪些行？是谁改的？
- 某个 gate 违规（如架构分层违规）在哪个迭代首次引入？
- reviewer 的批评意见对应哪个文件的哪段代码？

**代码证据链**：

```go
// internal/trace/trace.go:58-67
type Event struct {
    Format     string `json:"_format,omitempty"`
    Seq        int    `json:"seq"`
    Kind       string `json:"kind"`
    Name       string `json:"name,omitempty"`
    // ... duration, cost, model — 但没有 file/line/artifact 字段
}
```

trace 记录**元事件**，不记录**制品变化**。当一个 implementer 写完代码，没有 event 说「我创建了 `src/service.go`，修改了 `src/main.go`，删除了 `src/old.go`」。

```go
// forge-core/internal/orchestrator/command_executor.go
func (e CommandExecutor) Execute(ctx context.Context, p asset.Phase, mode string) (string, error) {
    cmd := exec.CommandContext(ctx, e.Build(p, mode)[0], e.Build(p, mode)[1:]...)
    cmd.Dir = e.Dir
    // 没有在 git 中记录执行前/后的文件状态
    out, err := cmd.CombinedOutput()
    return unwrapClaudeResult(string(out)), classifyRunErr(err)
}
```

`CommandExecutor` 在项目目录里直接执行 claude——claude 写文件改文件，但 executor 不记录**改动前后 diff**。

### 边界情况

| 场景 | 后果 |
|------|------|
| 三个月后生产环境暴露一个 bug | 无法追溯到是哪个 iteration 引入的——无法判断是否与某个安全 review 交错 |
| 架构 drift 被 arch-check 抓到 | 知道违规了但不知道是「新引入」还是「一直存在」 |
| 安全审计需要提供「代码变更来源」 | 只能提供「iteration 7 的 implementer 跑了」，没有文件级记录 |
| Reviewer 说「函数 A 需要重构」 | 无法自动标注具体文件和行号——implementer 只能人工理解 |

### 建议扩展

**Phase A（轻量）——git-aware executor**：
- 在 `CommandExecutor.Execute()` 前后插入 `git diff --stat` 和 `git diff` 记录
- 执行前捕获 `HEAD` 的 `git diff --name-only`；执行后再次捕获；将文件变更集附加到 `trace.Event`
- 新增 `Event.FileChanges` 字段：`{added: [], modified: [], deleted: []}`

**Phase B（完整）——artifact ledger**：
- 新建 `internal/trace/artifact.go`：`ArtifactLedger` 记录每个文件在每个 iteration 的状态
- 字段结构：`{iteration, phase, agent, file, action (create|modify|delete|review), hash (git blob hash)}`
- `forge blame <file>` CLI 查文件历史——哪个 agent+iteration 写了哪一行
- `forge changelog --iteration 7` 列出一个迭代的完整文件变更集

**Phase C（审计护城河）——trace 不可篡改链**：
- trace JSONL 每一行包含前一行内容的哈希（块链式结构）
- `Event.PrevHash` 字段——检测事后篡改
- 不加密不签名，仅防**意外丢失或静默修改**（轻量级）

### 不与已有分析冲突的证明

| 相似概念 | 本文的不同之处 |
|----------|---------------|
| Trace 事件完备性（seventh-wave 方向一） | 完备性增加更多 **event 类型**，不增加 **artifact 数据** |
| Checkpoint diff 浏览器（five-extensions-v10 方向四） | 浏览器展示**状态变化**（checkpoint 前后），不追踪**文件级 line-by-line 溯源** |
| Memory 可溯源（v11 方向四） | 可溯源是指 memory 条目的**来源可追踪**，不是代码文件的来源 |
| 跨相位故障归因（v13 方向三） | 归因追踪**执行失败**的因果链，不是代码产物的来源 |
| Phase 级回滚（v13 方向一） | 回滚关注**原子恢复**，不关注**谁写了什么** |

---

## 方向三：相位级前置条件验证

> **在烧钱之前发现缺了什么**  
> 类型：性能 · 鲁棒性 · 成本控制  
> 代码影响：`preflight.go` → `internal/preflight/` 新包 + `asset.Phase` 新字段  
> 紧急度：P1（直接影响预算效率）

### 现状

当前仅有一个**全局前置检查**（`forge doctor` 风格）在 `forge run` 启动时执行：

```go
// forge-core/cmd/forge/preflight.go
func preflight(root, mode, lifecycle string) error {
    // 检查 .agent/ 目录完整性
    // 检查 harness/ 工具存在
    // 检查 git 仓库状态
    // 不检查具体 phase 的依赖！
}
```

不同 phase 对环境的**需求完全不同**：

| Phase | 需要 | 不需要 |
|-------|------|--------|
| planner | git, .agent/ | 编译器, 测试框架 |
| implementer (Go) | `go`, `golangci-lint` | Python, pytest |
| implementer (TS) | `node`, `eslint` | Go, cargo |
| gate | `node` (harness), 测试工具 | LLM 凭证 |
| reviewer | LLM 凭证 | 编译器, linter |
| qa | 测试框架 (`go test`/`vitest`) | 生产部署工具 |

当前 `preflight.go` 用最粗的粒度检查所有——如果 `go` 不存在但项目是 TS，preflight 会通过（因为它不做语言级检查）；如果 `node` 不存在而 phase 是 implementer(TS)，preflight 却不知道。

**代码证据链**：

```go
// forge-core/cmd/forge/preflight.go:3-30
// 全局、无 phase 感知、无语言感知
func preflight(root, mode, lifecycle string) error {
    // 硬编码了 3 个 check，全部是全局的
    // 没有检查：phase 0 (planner) 不需要 go；phase 1 (implementer) 需要 go 1.21+
}
```

```yaml
# .agent/workflows/build.yml
phases:
  - name: planner
    agent: planner
    # 没有 requires_tools 字段
  - name: implementer
    agent: implementer
    # 没有 requires_tools 字段
  - name: gate
    required_gates: [test, arch, secret, sca, lint]
    # gate 的执行依赖 harness 工具，但不在 phase 中声明
```

```go
// internal/asset/asset.go
type Phase struct {
    Name         string   `yaml:"name" json:"name"`
    Agent        string   `yaml:"agent,omitempty" json:"agent,omitempty"`
    RequiredGates []string `yaml:"required_gates,omitempty" json:"required_gates,omitempty"`
    // 没有 RequiresTools, RequiresEnv, MinVersion 字段
}
```

`asset.Phase` 没有工具依赖声明。ForgeOS 已经知道项目语言（`forge detect` -> `detect.go` -> `ProjectProfile`），但 phase 的依赖只能用语言间接推断，不能显式声明。

### 边界情况

| 场景 | 后果 |
|------|------|
| Go implementer phase 运行 35 分钟，写了 3 个文件，然后 `go build` 失败——因为 `go` 版本 < 1.21，语法不支持 | 烧掉 $1.50 + 35 分钟 → 全部丢弃 |
| Python implementer phase 调用 `black` 格式化，但项目没用 black | gate 阶段 lint 报告 N/A，但一个迭代浪费了 |
| `forge run` 启动时 `node` 存在；24 小时后升级了系统，`node` 被移除 | 下一迭代的 test gate 静默变成 N/A，不报错 |
| 多语言项目：phase 需要 Rust 编译器但之前一直是 Go 开发 | 直到 implementer 执行到第 10 个文件才发现 |

### 建议扩展

**Phase A（轻量）——显式 phase 依赖声明**：
- `asset.Phase` 新增 `RequiresTools []ToolRequirement` 字段
```yaml
phases:
  - name: implementer
    agent: implementer
    requires_tools:
      - name: go
        min_version: "1.21"
        check: "go version"
      - name: golangci-lint
        optional: true  # 缺少则 warn 不 fail
        check: "golangci-lint version"
```

**Phase B（完整）——验证引擎**：
- 新建 `internal/preflight/` 包，与 `internal/orchestrator/` 同一层
- `CheckPhaseDeps(phase, root) → []DepResult` 函数，在 `runAgentPhase` 之前调用
- 结果缓存（per run），避免同一工具的重复检查
- Fail-closed：无法确定的依赖视为缺失（不静默忽略），但标注为不确定

**Phase C（前瞻）——增量验证**：
- 验证不仅要跑「存在不存在」，还要跑「配置对不对」
- 例如：`eslint` 存在 but `.eslintrc` 不存在 → warn（lint gate 会 N/A）
- 利用已有的 `adapters.mjs` 探测结果，在 agent phase 启动前注入 warning 到 prompt

### 不与已有分析冲突的证明

| 相似概念 | 本文的不同之处 |
|----------|---------------|
| 预检 / forge doctor（preflight.go） | forge doctor 是**全局静态检查**，不感知 phase、不感知语言、不在执行路径上 |
| 环境 drift / 长时退化（hidden-feedback §5） | 退化是**被动检测**（出问题后分析），本文是**主动验证**（执行前预防） |
| 适配器探测（adapters.mjs） | 探测在 gate 阶段执行，在 agent 已经烧钱之后；本文在 agent 执行之前 |
| Sandbox 隔离（expansion-directions 方向一） | 沙箱解决运行时的**权限隔离**，不解决运行前的**依赖检查** |
| 冷启动零到一引导（expansion-next-frontier 方向二） | 引导是**项目初始化时**的辅助，不是每 phase 执行的验证 |
| 资源四维护栏（recursion/budget/timeout/output） | 护栏防止**运行时**失控，不检查**运行前**的依赖完备性 |

---

## 方向四：收敛速度自适应控制

> **靠近目标时学会小步走**  
> 类型：性能 · loop 控制 · 成本优化  
> 代码影响：`internal/converge/` + `internal/orchestrator/loop.go` + `asset.StopCondition`  
> 紧急度：P2（长期收益，非立即缺陷）

### 现状

LoopEngine 每迭代**完全相同的步幅**：

```go
// internal/orchestrator/loop.go:121-166
for i := start; i <= l.MaxIter; i++ {
    // 每次迭代完整执行同一 workflow
    runErr = l.Engine.RunFrom(wf, mode, *startPhase)
    // ... 收集信号，判断是否收敛
}
```

收敛判据是**绝对的，不是相对的**：
- `roadmap_completion >= 1.0`（硬目标）
- `gates_green == true`（硬目标）

不存在：
- **收敛速度估计**：每次 iteration 增加多少 roadmap 完成度？趋势是加速还是减速？
- **接近度调整**：当完成度 > 80% 时，相位数是否应该减少？模型是否应该降档？
- **步幅自适应**：当完成度增长减缓时，是否应该切换策略（从新增功能到修复质量）？

**代码证据链**：

```go
// internal/converge/converge.go:137-150
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

`Evaluate` 是纯函数，无状态，无历史感知。每次输入相同的 `Signals` → 输出相同的 `Results`。

```go
// internal/orchestrator/loop.go:166-173
func staleCount(cur, prev float64, stale int, gatesGreen, prevGatesGreen bool) int {
    if cur > prev || (!prevGatesGreen && gatesGreen) {
        return 0
    }
    return stale + 1
}
```

`staleCount` 仅检测「是否停滞」，不检测「以何种速度接近目标」。停滞计数器重置为 0 的阈值极低（只要 roadmap 比上一轮高 0.0001）。

```go
// internal/orchestrator/loop.go:93
for i := start; i <= l.MaxIter; i++ {
    // 每次迭代都跑同一个 workflow——不考虑完成度
}
```

没有完成度感知的 scope 调整。90% 完成时和 10% 完成时跑完完全相同的 phase 集。

### 边界情况

| 场景 | 后果 |
|------|------|
| 完成度 85%→86% 花了 3 个 iteration（精细打磨期） | 每次都跑完整的 planner→implementer→gate→reviewer→qa，每次烧 $2 |
| 完成度 40%→80% 很快（爆发期），然后停滞 | 无法区分「爆发后自然收敛」和「遇到硬骨头」 |
| 预算已将用完但完成度 90% | 继续全速运行最后 10% 还是降档收尾？无机制决策 |
| Roadmap 项大小极不均匀 | 一个大项占 50% 完成度，迭代一次就过半；剩下 50% 是 20 个小项 |

### 建议扩展

**Phase A（轻量）——收敛速度报告**：
- `converge.Evaluate` 新增输出 `VelocityReport`：`{iteration, delta_roadmap, velocity (delta/time), trend (accelerating|plateau|decelerating)}`
- 不改变循环控制，仅增加观测输出
- `forge evolve` 的日志多一行：`velocity: +5.2%/iter, trend: decelerating (was +8.1%)`

**Phase B（核心）——自适应 scope 调整**：
- `LoopEngine.Run` 在迭代之间插入 `AdjustScope(sig, velocity) → workflowModifier`
- `workflowModifier` 是一个函数，接收 `asset.Workflow` 输出修改后的 `asset.Workflow`
- 接近收敛（>80%）时自动：
  - 跳过 planner（直接做 item，不重新分解）
  - 合并 reviewer 和 qa 为一个 phase
  - gate 只跑有 diff 的检查
- 远离收敛（<30%）且 velocity 高时自动：
  - 从 haiku/sonnet 升档到 opus（如果预算允许）
  - 增加并行度

**Phase C（护城河）——自适应 StopCondition**：
- 从静态 YAML 条件 → 动态调整的收敛标准
- `stop_conditions` 可以有多级：
```yaml
stop_conditions:
  - when: {completion < 80%}
    criteria: {roadmap_completion >= 1.0, gates_green: true}
  - when: {completion >= 80%, velocity < 2%/iter}
    criteria: {critical_items_complete: true, gates_green: true}
  - when: {budget_remaining < 20%}
    criteria: {gates_green: true}  # 预算低时放宽到「只要 gate 绿就算」
```
- `converge.AdaptiveEvaluate` 根据 velocity + budget + completion 选择当前生效的 stop 条件

### 不与已有分析冲突的证明

| 相似概念 | 本文的不同之处 |
|----------|---------------|
| 跨周期收敛状态机（expansion-core-five 方向一） | 状态机追踪**趋势方向**（progress/plateau/regress），不改变收敛**速度**或 loop **步幅** |
| 收敛门闩效应（edgecases-and-perf §3） | 门闩是**收敛后的状态锁定**，不涉及接近收敛时的过程调整 |
| 预算优雅降级（v12 方向五） | 降级是**模型层面**的（opus→sonnet），不是**流程层面**的（简化 workflow） |
| 自适应工作流（high-value-extensions 方向一） | 自适应是**项目类型/风险信号**驱动的静态组装，不是收敛进度驱动的动态调节 |
| 置信度感知决策（v6 方向二） | 置信度是 agent **对产出的自我评估**，不是系统对**收敛速度的感知** |

---

## 方向五：执行行为异常检测

> **从遥测中识别「不对劲」**  
> 类型：可观测性 · 鲁棒性 · 运维  
> 代码影响：`internal/trace/` + `internal/orchestrator/` + 新 `internal/anomaly/` 包  
> 紧急度：P2（长跑可靠性）

### 现状

ForgeOS 拥有丰富的运行时遥测数据，但**除了最原始的统计聚合外没有任何消费**：

```
遥测数据流（现有）：
trace.jsonl → 维度: duration_ms, cost_usd, kind, phase, model, outcome
scorecards → 维度: p50/p95 latency, avg_cost, sample_count, task_type
memory → 维度: gap/decision/lesson entries, iterations
```

消费层（现有）：
- `scorecard-update.mjs` → 按 `(model, task_type)` 聚合 latency + cost
- `converge.Evaluate` → 按 roadmap + gates 判收敛
- `staleCount` → 按 roadmap 变化判停滞

**没有被覆盖的**异常模式：

| 模式 | 信号 | 现有检测 |
|------|------|---------|
| Phase 突然变慢 5x | `duration_ms` p95 基线 vs 当前 | ❌ 无——只用 scorecard 聚合历史 |
| Model 输出 token 突然骤降 | 无直接信号（需要 parse 输出） | ❌ 无——不追踪输出长度 |
| Gate 从全 PASS 变为全 FAIL | `gate_outcome` 序列 | ❌ 无——只在当次迭代检查 |
| Agent 反复修改同一文件 | `trace.Event.FileChanges`（未实现） | ❌ 无 |
| 同 iteration 内 phase 顺序异常 | phase completion 顺序 | ❌ 无——parallel 模式未追踪 |

**代码证据链**：

```go
// internal/trace/trace.go:58-150
type Event struct {
    // 所有字段都是为了事后审计
    // 没有任何字段是为了「实时异常检测」
    // 没有 "this duration is abnormal" 标记
}
```

trace 的消费者只有：
1. `scorecard.mjs` / `scorecard-update.mjs`——离线聚合
2. `converge.Signals`——只读 `RoadmapCompletion` + `GatesGreen`

没有任何模块**读 trace 做运行时诊断**。

```go
// internal/orchestrator/exec_error.go
func classifyRunErr(err error) ExecError {
    // 分类已知错误类型：timeout, overload, config, failed
    // 无法检测「这个 phase 在退化」——因为没有历史对比
}
```

```go
// forge-core/cmd/forge/cost.go
func parseClaudeCostUsd(output string) (float64, error) {
    // 只解析 cost，不解析 token 用量或输出长度
    // token 计数是评估 agent 行为是否异常的重要信号
}
```

```yaml
# .agent/routing/policy.yml
# 没有异常检测相关的配置段落
```

### 边界情况

| 场景 | 识别方式 | 当前行为 |
|------|---------|---------|
| Claude Opus 返回的 token 数从正常 ~2000 变为 ~50 | 输出长度骤降（退化模式） | 静默接受，gate 可能因为输出太少而过快通过 |
| Phase duration 从 30s 变为 300s | 执行时间 10x 但未超时 | 静默等待，不主动干预 |
| 同一 gate 连续 5 次 PASS 然后连续 3 次 FAIL | gate 状态序列模式变化 | 逐次判断，不察觉「模式翻转」 |
| 某 agent 每 iteration 都修改同一个文件又改回去 | 文件内容振荡 | 无追踪（无 artifact ledger） |
| 并行 N 个 phase 中某 phase 完成时间大幅偏离同类 | phase 间 session 时间长度的离散度 | 无组内比较 |

### 建议扩展

**Phase A（轻量）——运行时基线**：
- `trace` 包新增 `Baseline` 结构体：维护轻量级运行时统计（`min/avg/max duration` 按 phase 类型）
- `CommandExecutor.Execute` 返回时，检测 `duration > avg*threshold`（阈值可配），触发 `KindSlowExecution` trace event
- 新增 `KindAnomaly` trace kind——不打断执行，但标记异常

**Phase B（核心）——异常评分引擎**：
- 新包 `internal/anomaly/score.go`（纯函数，零外部依赖）
- `Score(snapshot AnomalySnapshot) AnomalyReport`——多元异常评分
```go
type AnomalySnapshot struct {
    PhaseDurationMs    int64
    GateOutcomes       []string  // 最近 N 次 gate 状态
    CostUsd            float64
    IterationCompletion float64
    AgentOutputLen     int       // output token 估算
    // ... 所有已有遥测信号
}
type AnomalyReport struct {
    Score       float64  // 0-1, >0.8 = 异常
    Factors     []Factor // 各维度贡献
    IsActionable bool    // 是否可自动响应
}
```
- 实现简单基线：`z-score` 超过 3σ 标记为异常（由纯净的数学逻辑推导，无需 ML）

**Phase C（响应）——异常响应策略**：
- `orchestrator.Engine` 新增 `AnomalyHandler func(AnomalyReport) Action`
- `Action` 可以是：`{type: "log"}` / `{type: "warn", message: "…"}` / `{type: "abort"}` / `{type: "retry", strategy: "downgrade-model"}`
- 例如：output 长度骤降 → warn + 插入额外 gate 检查
- 例如：gate 连续 5 次 PASS 然后 3 次 FAIL → 建议用户检查环境变更

### 不与已有分析冲突的证明

| 相似概念 | 本文的不同之处 |
|----------|---------------|
| 自愈层运行时（v6 方向四） | 自愈处理**已知**失败模式（timeout/overload），异常检测发现**未知/新兴**模式 |
| 混沌工程（seventh-wave 方向四） | 混沌工程**主动注入**故障测试韧性，异常检测**被动发现**真实异常 |
| 架构度量趋势（v6 方向五） | 趋势是**架构静态度量**（扇入/耦合/圈复杂度）的长期变化，不是**运行时行为**的异常 |
| 闸门自省/元学习（high-value-extensions 方向二） | 元学习关闭**治理策略的盲区**，不检测**实时执行异常** |
| 可靠性仪表化（v12 方向一） | 仪表化测量**闸门自身的可靠性**（假阴/假阳率），不是 agent/phase 的执行行为 |
| 信号处理/优雅关闭（sprint-27） | 优雅关闭处理**系统级信号**（SIGINT/SIGTERM），不检测**应用级异常** |

---

## 优先级综合矩阵

| 方向 | 紧急度 | 影响范围 | 实现成本 | 独特价值 |
|------|--------|---------|---------|---------|
| 一 · 多模型共识层 | P1 | 安全/治理 | 中 | 高利害决策的信任基础 |
| 二 · 制品级溯源链 | P2 | 审计/运营 | 低-中 | 长期代码质量的审计地基 |
| 三 · 相位级前置验证 | P1 | 成本/效率 | 低 | 显著减少浪费的 LLM 调用 |
| 四 · 收敛速度自适应 | P2 | 性能/成本 | 中 | 长跑收敛效率的结构性优化 |
| 五 · 执行行为异常检测 | P2 | 鲁棒性/运维 | 中 | 填补运行时「看不见」的最后一块 |

### 收敛建议

**若只做一件**：**方向三（相位级前置验证）**——实现成本最低（扩展 `asset.Phase` 结构体 + 30-50 行验证逻辑），收益最直接（阻止因缺少依赖而浪费的 LLM 调用）。与已落地的 `forge detect` 天然集成（检测到的语言/框架直接映射为 phase 依赖）。

**若做两件**：**三 + 一（前置验证 + 共识层）**——分别解决预算安全和决策可信度，两个最常被问到的问题：「这个系统怎么保证不浪费钱？」和「我怎么信任单个 agent 的判断？」

**若做三件**：**三 + 一 + 二（+ 溯源链）**——形成「事前验证 → 事中交叉确认 → 事后追溯」的完整运营三角。

---

*不写代码，只做判断。诚实标注：方向一~五均不涉及修改现有 forge-core 的 API 契约或引入外部依赖，所有扩展可在纯 Go 标准库和现有 YAML schema 扩展内实现。*
