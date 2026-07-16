# ForgeOS — 全局扫描后的高价值扩展方向（v19：未被覆盖的结构性前沿）

> **角色**：资深架构师 / 产品经理  
> **方法**：全局代码库深扫（forge-core 13+ Go 包 + cmd/forge 18+ CLI 命令 + harness 26+ 模块 +  
>   `.agent/` 完整治理骨架 + examples/ + 30+ 份已有 docs/analysis/ 逐份交叉核对）  
> **基线**：Sprint 27 全状态（真点火正交验证 + Context 传播 + 跨迭代 checkpoint + 并行编排 +  
>   ContextCache + gate ledger feed-forward + HumanApproval 门闩）  
> **纪律**：每方向与已有分析交叉确认无重叠。不写代码。

---

## 已有 30+ 份分析覆盖域（本文不再重复）

以下域已被充分覆盖，本文不再涉及：

| 域 | 覆盖文档 |
|------|----------|
| 自适应工作流 / 信号驱动编排 | `high-value-extensions.md` 方向一 |
| 闸门自省 / 元学习闭环 | `high-value-extensions.md` 方向二 |
| 增量式治理执行 / git-diff 执法 | `high-value-extensions.md` 方向三 |
| 跨项目知识联邦 / 组织学习 | `expansion-gaps-v7-novel.md` 方向一 |
| 运行时模型质量自适应 | `expansion-gaps-v7-novel.md` 方向二 |
| 多租户安全隔离 / Agent 权限模型 | `expansion-gaps-v7-novel.md` 方向四 |
| 确定性 Replay / 调试引擎 | `expansion-gaps-v7-novel.md` 方向三 |
| Memory 衰减 / 去重 / 可溯源 | `high-value-perspectives-v11.md` 方向四 |
| 平行引擎 fail-fast 短路 | `edgecases-and-perf.md` §1.1 |
| 配置表面积 / 跨文件一致性 | `configuration-surface-and-adoption.md` |
| ADR 架构决策衰退审计 | `eighth-wave-adr-decay.md` |
| 长运行时数据生命周期 | `fresh-scan-strategic-expansion.md` 方向一 |
| YAML-Shim 消除 / Go-Native Asset | `fresh-scan-strategic-expansion.md` 方向二 |
| 跨 Agent Prompt 注入防护 | `expansion-directions-v6-novel-perspectives.md` 方向一 |
| 自愈层运行时 | `expansion-directions-v6-novel-perspectives.md` 方向四 |
| 架构度量趋势分析 / 早期预警 | `expansion-directions-v6-novel-perspectives.md` 方向五 |
| 收敛理论隐藏陷阱 | `edgecases-and-perf.md` §3 |
| ForgeOS 自我测试缺口 | `self-testing-and-dogfooding.md` |
| 置信度感知决策引擎 | `expansion-directions-v6-novel-perspectives.md` 方向二 |
| Growth bottlenecks / cmd/forge 膨胀 | `growth-bottlenecks-and-scalability.md` |
| Meta-governance 自身治理差距 | `expansion-forgeos-meta-governance.md` |
| 跨周期收敛状态机 | `expansion-core-five-2026-07-01.md` 方向一 |
| 统一验证引擎（三语言消除） | `expansion-core-five-2026-07-01.md` 方向二 |
| 实时可观测性层（流式事件） | `expansion-core-five-2026-07-01.md` 方向三 |
| 跨平台安装器 / 自举 | `expansion-core-five-2026-07-01.md` 方向四 |
| 并行编排死锁/竞态 | `edgecases-and-perf.md` §1 + S27 |
| 信号处理 / 优雅关闭 | `sprint-27-signal-handling.md` |
| 代码库健康趋势追踪 | `strategic-extensions-v18-uncovered-frontiers.md` 方向一 |
| 相位级资源画像与预算分配 | `strategic-extensions-v18-uncovered-frontiers.md` 方向二 |
| 配置实现一致性验证（元治理） | `strategic-extensions-v18-uncovered-frontiers.md` 方向三 |
| Agent 输出多维质量评估 | `strategic-extensions-v18-uncovered-frontiers.md` 方向四 |
| 跨运行异常检测 / 趋势监控 | `strategic-extensions-v18-uncovered-frontiers.md` 方向五 |
| Loop-back 文件污染 | `expansion-blind-spots-v16.md` 方向二 |
| 工具相位（非 LLM 执行） | `expansion-blind-spots-v16.md` 方向四 |
| .forge 目录并发安全 | `expansion-blind-spots-v16.md` 方向一 |
| MQTT/WASM 集成 | `mqtt-and-wasm-integration.md` |
| 多项目拓扑编排 | `expansion-blank-spots-v15.md` 方向一 |
| 自愈 ROADMAP | `expansion-core-five-2026-07-01.md` 方向四 |

---

## 本文 5 个方向（从代码微观结构 + 长期运维推演，逐项确认未被已有分析覆盖）

---

## 方向一：自适应上下文窗口预算——从「静态截断」到「感知内容优先级的动态分配」

### 代码级证据

当前上下文构建完全依赖**静态硬上限**：

```go
// internal/prompt/prompt.go
const adrTopK = 6       // 无论 ADR 库多大，永远最多塞 6 条
const taskCap = 4000    // 无论 ROADMAP 多长，永远截断到 4000 rune
```

```go
// internal/prompt/retrieve.go — TF-IDF 检索，但 top-K 是常数
func Retrieve(docs []Doc, query string, topK int) []Doc {
    // ... TF-IDF 打分，选 topK
}
```

这些上限**与当前模型的实际上下文窗口完全脱钩**。Claude Opus 的 200K token 窗口对以下三者静默无感：

1. ADR 库从 5 个增长到 50 个 → `adrTopK=6` 仍然只选 6 条，哪怕窗口空了一半
2. ROADMAP 从 200 token 增长到 8000 token → `taskCap=4000` 粗暴截断，哪怕窗口仍有 100K 余量
3. 收敛历史（gate 结果 + agent 决策）从不作为上下文注入——即使有空间可填入

同时，**ContextCache** 已经展示了「区分稳定前缀与易变后缀」的能力：

```go
// internal/prompt/cache.go — 明确将上下文分为三层
func GatherCached(cache *ContextCache, repoRoot, query string) []string {
    // (1) TASK lane — 每次重读（agent 可写）
    // (2) ADR lane — 检索输出每次重算（query 变化）
    // (3) CONSTRAINTS lane — 缓存不变内容
}
```

但分层只服务于**本地 IO 缓存**，不服务于**上下文窗口预算分配**。三条 lane 的总大小无法被约束或感知：

```go
// 没有代码问：这三条 lane 加起来多大？
// 没有代码问：这些 token 里 80% 是固定的 boilerplate，5% 是真正的 task-specific？
// 没有代码问：gate 结果、memory 条目、converge 历史——它们也竞争同一窗口，但谁先谁后？
```

### 高价值扩展

**自适应上下文窗口预算器（Adaptive Context Budget）**——将静态 cap 升级为基于内容的动态分配：

```
当前:
  ADRs:     top-6 (固定)           ← 即使 ADR 库有 100 条
  ROADMAP:  前 4000 rune (固定)    ← 即使窗口有 150K 余量
  Constraints: 前 6 条 (固定)      ← 硬约束全文可能 200 token，占不满也浪费
  Memory:    never injected        ← 虽然 memory 已有全部知识
  GateLedger: never injected       ← 虽然 reviewer 阶段需要 gate 结果

扩展后:
  ┌─ 上下文预算分配器 ──────────────────────────────────┐
  │                                                      │
  │  1. 测量可用窗口 (availableTokens)                   │
  │     - 模型固定开销 (role card + base prompt) ≈ 5K     │
  │     - 剩余 token = model_window - fixed_overhead     │
  │                                                      │
  │  2. 按优先级分配剩余 token                            │
  │     P0 (always):    AGENTS.md 硬约束                  │
  │     P1 (high):      ROADMAP 当前任务 (但有 token cap) │
  │     P2 (medium):    ADR 检索结果 (按相关性加权)        │
  │     P3 (variable):  Memory 查询结果 (按置信度+时效)    │
  │     P4 (context):   gate 前次裁决 + 收敛历史           │
  │                                                      │
  │  3. 每条 lane 的 token 预算 = 可用窗口 × 权重         │
  │     - window full → P3/P4 静默降级为省略              │
  │     - window 空余很多 → P2 topK 从 6 提升到 12       │
  │     - 某 lane 内容不重要 → 权重学习 (历史反馈)         │
  └──────────────────────────────────────────────────────┘
```

**新增 `internal/prompt/budget.go`**：

```go
// Budget 管理一次 phase 的上下文预算分配
type Budget struct {
    ModelWindow   int  // 模型上下文窗口 (e.g. 200000 for opus)
    FixedOverhead int  // role card + base prompt 的固定开销
    Allocations   map[string]int  // lane → token 预算
}

// Allocate 按策略分配预算
func (b *Budget) Allocate(lanes []Lane) {
    available := b.ModelWindow - b.FixedOverhead
    // P0 先拿（AGENTS.md）
    // P1 拿固定比例（ROADMAP）
    // P2-P4 按权重竞争剩余
    // 不足的 lane 被降级：topK 减少、截断更短、或省略
}

// Lane 是上下文的一条输入
type Lane struct {
    Name     string   // "adr" | "task" | "constraints" | "memory" | "gate_results"
    Priority int      // 0=always, 1=high, 2=medium, 3=variable
    Weight   float64  // 历史权重（基于该 lane 对 agent 输出质量的贡献）
    Data     string   // 内容
    TokenLen int      // 内容的 token 预估（从字符数估算）
}
```

**为什么高价值**：

1. **15 分钟迭代 vs 15 秒迭代**：当前每个 phase 都塞一模一样的 ADR+Constraints 前缀（ContextCache 只换 task），但 agent 真正需要的内容可能只占 10%。当代码库膨胀到 500+ ADR 文件时，固定 topK 要么爆窗口，要么浪费窗口——系统无法感知。

2. **Evolve 循环的上下文遗忘**：当前 evolve 每次迭代都重新检索 ADR、重新读取 ROADMAP，但**从不携带上一次迭代的收敛历史**。如果 agent 在第 3 次迭代重复犯第 1 次迭代已经犯过的错误（因为不记得），那就是上下文预算分配失败——不是窗口不够大，而是信息优先级错了。

3. **跨 lane 的竞争可测量**：有了预算分配器，就可以回答「哪些上下文 lanes 被截断了？agent 因为缺失什么信息而做出了错误决定？」——这是当前完全不可见的调试维度。

### 边界情况

- **token 估算精度**：字符数 ≠ token 数。需要一个轻量估算器（Go 标准库没有 tiktoken）。v1 可以用纯启发式（每字符 0.25 token，或每英文单词 1.3 token），诚实标注为估算。
- **lane 退化到空**：当所有 P0-P4 的总需求超过可用窗口时，P3/P4 整体省略。这比「每个 lane 都塞一点，但每条都不够」更诚实。
- **模型窗口变化**：不同 model tier（Haiku = 200K, Sonnet = 200K, Opus = 200K）目前窗口相同，但 cross-vendor 后可能不同。`Budget.ModelWindow` 必须 per-tier 可配。
- **并行模式**：并行 phase 各自独立竞争上下文，互不影响。没有全局预算分配。

---

## 方向二：跨工作流编排引擎——从「线性脊柱」到「有状态的条件过渡」

### 代码级证据

当前的生命周期编排完全是**线性的、无状态的、单次触发的**：

```go
// cmd/forge/evolve.go — 只运行一个 workflow 的 loop
func cmdEvolve(args []string) int {
    // ... 加载一个 workflow (build.yml)
    // ... 运行 LoopEngine.Run
    // ... 退出
}
```

各个 workflow 之间**没有运行时编排**：

```
人工流程:
  forge run discover.yml  ──→  # 需求探索
  forge run design.yml    ──→  # 方案设计 (→ human_gate)
  forge evolve build.yml  ──→  # 构建 (→ converge)
  forge run review.yml    ──→  # 安全/分布式/性能/CTO 审查
  forge evolve evolve.yml ──→  # 持续演化
```

每一步都是**人工或脚本串联**的。没有：
- 自动条件过渡（discover 收敛后自动触发 design）
- 并行工作流（discover 和 market-research 同时跑）
- 失败分支（design human_gate REJECTED → 自动回到 discover）
- 跨工作流状态查询（build 中需要引用 design 的输出——但 `phaseOutputLedger` 只在内一个 workflow 的 phase 之间传）

```go
// internal/asset/asset.go — Workflow 是孤立的
type Workflow struct {
    Stage  string   // 只有一个 stage 标签，没有上下游关系
    Phases []Phase
    Stop   StopCondition
    // 没有 NextWorkflow, OnConverged, OnRejected 字段
}
```

工作流之间的**唯一联系是人工和文件系统**——design.yml 的 human_gate `on_approved.next_stage: build` 是一个声明性标签，但**没有程序读取它来触发下一次 run**：

```go
func nextStageLabel(stop asset.StopCondition) string {
    if stop.OnApproved.NextStage == "" {
        return "(no next_stage declared)"
    }
    return "next_stage=" + stop.OnApproved.NextStage
    // 只是标签打印！没有自动触发！
}
```

### 高价值扩展

**跨工作流编排引擎（Meta-Orchestrator）**——将五个 spine workflow 之间的过渡升级为有状态的状态机：

```
当前:
  discover ──→ design ──→ [human_gate] ──→ build ──→ review ──→ evolve
     ↓            ↓            ↓              ↓         ↓           ↓
   孤立        孤立          孤立            孤立       孤立        孤立
  run done  → run done → approve → run done → run done → run done

扩展:
  ┌──────────────────────────────────────────────────────┐
  │  Meta-Orchestrator                                   │
  │                                                      │
  │  state = discover                                    │
  │  on_enter: run discover.yml                          │
  │  on_converged: transition → design                   │
  │  on_failed: log + notify                             │
  │                                                      │
  │  state = design                                      │
  │  on_enter: run design.yml                            │
  │  on_converged (human_approved): transition → build   │
  │  on_rejected: transition → discover (重新设计)        │
  │                                                      │
  │  state = build                                       │
  │  on_enter: run evolve build.yml                      │
  │  on_converged: transition → review                   │
  │  on_stale: re-enter planner (当前 loop-back)         │
  │                                                      │
  │  state = review                                      │
  │  on_enter: run review.yml                            │
  │  on_converged (cto_approved): transition → production│
  │  on_rejected: transition → build (redesign)          │
  │                                                      │
  │  meta-stop: all stages converged + human approved    │
  └──────────────────────────────────────────────────────┘
```

**新增 `forge pipeline` 子命令**：

```
forge pipeline <lifecycle>           # 从 discover 跑完整生命周期
forge pipeline --from design         # 从指定 stage 开始
forge pipeline --dry-run             # 打印计划不执行
forge pipeline --watch               # 持续观察等待 human_gate
```

**新增 `.agent/pipeline.yml`**：

```yaml
# 生命周期状态机描述
pipeline:
  initial: discover
  states:
    discover:
      workflow: discover.yml
      on_converged:
        target: design
      on_failed:
        target: initial      # 重启探索
    design:
      workflow: design.yml
      on_approved:           # human_gate 批准
        target: build
      on_rejected:           # human_gate 拒绝
        target: discover     # 回到探索，不是回到设计
    build:
      workflow: build.yml
      on_converged:
        target: review
      on_stale:
        target: build        # 再跑一次 build（当前 loop-back 机制）
    review:
      workflow: review.yml
      on_converged:          # CTO approve
        target: production
      on_rejected:
        target: build        # 需要大修
```

**为什么高价值**：

1. **真·无人值守愿景的第一步**：ForgeOS 的终极主张是「Idea → Production 无人值守」。当前架构做到了「一个 workflow 内无人值守」，但五个 workflow 之间的串联需要人手动触发。meta-orchestrator 消除了 transition 上的人工操作——这才是「全生命周期自动化」的最后一公里。

2. **human_gate 的 rejection 路径**：当前 `on_rejected` 的 LoopBack 只会从一个 phase loop-back 到另一个 phase（同工作流内），但**被拒绝的设计应当触发的是另一个工作流**（回到 discover 重新探索需求，而不是重新写同一个设计文档）。当前架构不支持跨工作流跳转。

3. **跨工作流状态可追溯**：第五次 build converge 后，操作员想知道「这是第五次 build 了，之前四次的 review 结果是什么？」——当前系统只能看历史日志，没有结构化查询。

### 边界情况

- **长期运行的等待**：human_gate 可能需要等待数小时/数天。pipeline 需要一个持久化等待机制（当前 v1 的 `--approved` 标记是瞬时的）。对应 `durable_wait` 的 v2 路线图。
- **并行工作流**：discover 可以同时跑 requirement-discovery 和 market-research（已经靠 parallel 引擎支持，但 meta-orchestrator 需要能等待两个并行工作流都收敛）。
- **循环检测**：如果 discover → design → build → review → discover 形成了无限循环，meta-orchestrator 需要循环上限和告警。
- **部分失败**：review 的 security-engineer 过了但 distributed-engineer 失败了——这是工作流内部分失败，但 meta-orchestrator 应该能区分「需要人工介入」和「可以重试」。

---

## 方向三：相位输出合同校验——从「声明即真相」到「运行时输出验证」

### 代码级证据

`asset.Phase` 已经声明了输出合同字段：

```go
// internal/asset/asset.go
type Phase struct {
    // ... 现有字段
    Emits            []string   `json:"emits,omitempty"`       // 声明将产生的文件路径
    ConfidenceMetric string     `json:"confidence_metric,omitempty"`  // 声明将产出置信度分数
    FeedsForward     bool       `json:"feeds_forward"`         // 声明输出会传递给下游相位
    FreshContext     bool       `json:"fresh_context,omitempty"`
    UsesTemplate     string     `json:"uses_template,omitempty"`
}
```

但这些字段**只被声明，从未在运行时被验证**：

```go
// internal/prompt/prompt_context.go — cmd/forge 的 prompt 构建器
func buildPrompt(...) string {
    // 读到 phase.Emits — 但从不检查输出文件是否存在
    // 读到 phase.ConfidenceMetric — 但从不解析 agent 输出中是否真的有置信度分数
    // 读到 phase.FeedsForward — 但不验证 downstream phase 是否真的收到了数据
}
```

具体缺失：

1. **Emits 文件验证**：planner `emits: [task-plan.md]`，但 agent 执行完后，没有任何代码检查 `task-plan.md` 是否存在：
```go
// 缺失的检查
if len(p.Emits) > 0 {
    for _, file := range p.Emits {
        if _, err := os.Stat(filepath.Join(root, file)); os.IsNotExist(err) {
            // ⚠️ planner 声明了 task-plan.md 但没生成
            // 当前: 静默通过
            // 应该: 告警或阻止收敛
        }
    }
}
```

2. **FeedsForward 数据流转验证**：planner 声明 `feeds_forward: true`，但下游 phase（implementer）是否真的接收到了 planner 的输出？当前 `phaseOutputLedger` 会记录，但**不校验 downstream phase 的 prompt 是否真的包含了这些内容**。

3. **ConfidenceMetric 解析验证**：discover.yml 的 requirement-discovery phase 声明 `confidence_metric: requirement_confidence`，但 `converge.go` 的 `RequirementConfidence` 信号**零值意味着「没有数据」**，无法区分「agent 跑了但没报告置信度」和「agent 没跑这个 phase」：
```go
// converge.go
func evalRequirementConfidence(c asset.Criterion, sig Signals) Result {
    if sig.RequirementConfidence == 0 {
        return Result{... "no discover phase data"} // 无法区分"没跑"还是"跑了但没报"
    }
}
```

4. **UsesTemplate 但不校验模板是否匹配输出**：review.yml 的 phases `uses_template: .ai/prompts/02-security-rfc-review.md`，但 agent 输出从不被校验是否符合模板结构（模板要求的安全发现格式 vs agent 实际输出的格式）。

### 高价值扩展

**相位输出合同校验框架（Output Contract Validation）**——在 phase 执行后、gate 之前，自动验证 agent 产出是否符合 Phase 声明的合同：

```
相位执行后:
  1. Agent 输出 (stdout / 文件写入)
     │
     ▼
  2. 输出合同校验器
     │
     ├── Emits 校验: 声明了 task-plan.md? → 文件存在? 非空?
     ├── ConfidenceMetric 校验: 声明了 requirement_confidence?
     │   └→ agent 输出中解析出数值? 在 [0,100] 内?
     ├── FeedsForward 完整性: 声明了 feeds_forward?
     │   └→ phaseOutputLedger 有内容? 下游 prompt 包含正确引用?
     └── UsesTemplate 结构匹配: 使用了模板?
         └→ 输出与模板要求的格式大致匹配?
     │
     ▼
  3. 结果:
     - PASS: 合同完全满足
     - WARN: 部分满足 (文件存在但为空)
     - FAIL: 合同被违反 (声明的文件不存在)
     │
     ▼
  4. fail-closed: 违反合同 → phase 标记为失败 (可重试/可 loop-back)
     fail-open: 部分满足 → 注入 warning 日志，不阻塞
```

**新增 `internal/validate/contract.go`**（不是新命令行工具，而是可以被 orchestrator 调用的包）：

```go
type ContractViolation struct {
    Phase   string
    Field   string // "emits" | "confidence_metric" | "feeds_forward"
    Expect  string // 期望
    Actual  string // 实际
    Severity Severity // WARN | FAIL
}

// VerifyPhaseOutput 验证一个 phase 的输出是否满足其声明合同
func VerifyPhaseOutput(root string, p asset.Phase, phaseOutput string) []ContractViolation {
    var violations []ContractViolation
    for _, file := range p.Emits {
        path := filepath.Join(root, file)
        if _, err := os.Stat(path); os.IsNotExist(err) {
            violations = append(violations, ContractViolation{
                Phase: p.Name, Field: "emits",
                Expect: fmt.Sprintf("file %s exists", file),
                Actual: "file not found",
                Severity: SeverityFAIL,
            })
        }
    }
    // ...
    return violations
}
```

**为什么高价值**：

1. **治理的「自我一致性」**：ForgeOS 的架构原则要求声明式治理。如果 phase 声明自己会产出 task-plan.md 但实际上没有，这个「声明 ≠ 实现」本身就是治理腐败——而 ForgeOS 自己恰恰应该是最先检测到这种腐败的系统。

2. **诊断失焦 agent 输出**：Sprint 25 真点火暴露了 agent 在 acceptEdits 模式下不知道要做什么（因为缺少任务注入）。那时是通过修 prompt 解决。有了合同校验，即使 prompt 漏了，**或者 agent 偏离了方向**，系统也能在 phase 结束时用合同检查抓住——而不是在 gate 阶段通过 test 失败才间接发现。

3. **为 ConfidenceMetric 提供真实区分度**：当前 `RequirementConfidence==0` 同时表示「没跑」和「跑了但没报」。合同校验能将前者标记为合同违反（FAIL），后者标记为正常的零值——让 converge 的 honesty 更精确。

### 边界情况

- **文件路径解析**：Emits 可能是相对路径（`docs/adr/001-foo.md`）或绝对路径。需要一个统一的 `resolveEmitPath(root, emit)` 函数。
- **非确定性文件生成**：某些 phase 产生非确定性文件名（UUID 命名的临时文件）。合同应允许多种匹配模式（glob）。
- **并行模式**：并行 phase 的合同各自独立验证——一个 phase 的输出失败不影响另一个 phase 的输出。
- **向后兼容**：现有 workflow 声明的 Emits 是空的 → 不触发验证。无行为变化。

---

## 方向四：迭代输出合并——从「破坏性替换」到「增量式差异合并」

### 代码级证据

当前 loop-back 机制是完全**破坏性的**：

```go
// internal/orchestrator/orchestrator.go — loop-back 跳转
func (e Engine) loopBackTo(wf asset.Workflow, p asset.Phase, loopBacks *int, reason string) (target int, jumped bool) {
    // ... 找到 target phase，修改循环 index
    *loopBacks++
    return idx, true  // 跳回 target phase，让 RunFrom 重新执行
}
```

当 loop-back 跳回 `implementer` phase 并重新执行时：

1. **旧输出被完全覆盖**：agent 重新写文件，旧版本被 git 覆盖。任何 reviewer 认为「这部分做对了，但那部分要改」的区分都被丢弃。
2. **不存在增量合并**：如果 agent 第一次产生了 3 个文件的实现，reviewer 只对其中 1 个有修改意见——loop-back 后，agent 重新写所有 3 个文件（可能引入新的差异，也可能把做对的改错）。
3. **没有联合分析**：implementer 第一次跑产生 file_a.go，loop-back 后产生 file_a.go+v2——但 reviewer 没有 diff 历史，只能看最终版本。

具体看 pipeline 数据流：

```go
// cmd/forge/prompt_context.go — gateLedger 的 inject
// 只传递最终 gate 结果，不传递修改历史
func gatherGateLedger(log []GateRecord) string {
    // 只注入每次 gate 的最终裁决
    // 没有「上次的 implementer 输出 vs 这次的差异」信息
}
```

而 `phaseOutputLedger` 也只记录**最终输出**：

```go
// cmd/forge/prompt_context.go
type phaseOutputLedger struct {
    outputs map[string]string // phase 名 → 最终输出文本
}
// 不是 map[string][]string 或 map[string]Diff
```

### 高价值扩展

**迭代输出合并系统（Iterative Output Consolidation）**——在 loop-back 时，不直接覆盖旧文件，而是运行一个**差异检查+合并**过程：

```
迭代 1: implementer 产生 file_a.go, file_b.go, file_c.go
迭代 2: loop-back → implementer 重新执行

当前行为:
  file_a.go ← 新版本 (可能改了做对的 file_a)
  file_b.go ← 新版本
  file_c.go ← 新版本

扩展行为:
  1. Diff 新旧输出:
     - file_a.go: 只改了注释   ← 保留旧版 (style 无关)
     - file_b.go: 修复了 bug   ← 采用新版
     - file_c.go: 新版本删掉了重要逻辑 ← 合并（保留被删的功能 + 采纳新修复）

  2. 自动合并:
     - 无冲突: 自动选取最佳版本
     - 有冲突: 标记 + 继续 (不阻塞，留给人或下次 review 解决)

  3. 输出差异报告:
     - reviewer 被告知: "相对于上次 implementer 输出，本次改变了 file_b.go"
     - gate 被告知: "本次迭代的 file_a.go 未变——上次已通过 lint，本次可跳过"
```

**核心原则**：合并是**可选的、增量式的**，完全向后兼容。无 `depends_on` 的 phase 仍然是破坏性替换——与现有行为完全一致。

```go
// internal/asset/asset.go — Phase 扩展
type Phase struct {
    // ... 现有字段
    MergeStrategy string `json:"merge_strategy,omitempty"` // "replace" | "diff" | "smart"
}

// internal/consolidate/consolidate.go — 新包
// ConsolidateOutput 合并同一 phase 的前后两次输出
func ConsolidateOutput(root string, p asset.Phase, prevOutputs, newOutputs map[string]string) (map[string]string, []DiffReport) {
    switch p.MergeStrategy {
    case "diff":
        return diffMerge(prevOutputs, newOutputs)
    case "smart":
        return smartMerge(prevOutputs, newOutputs)  // 文件级自动决定
    default: // "replace"
        return newOutputs, nil  // 向后兼容
    }
}
```

**为什么高价值**：

1. **loop-back 的浪费经济**：Sprint 25 真点火证实了 loop-back 是真实的。每个 loop-back 重跑 implementer 重新写所有文件。如果 reviewer 只要求改一个文件，那另外 N 个文件的重写是纯粹的成本浪费（token + 时间 + 可能的引入缺陷）。

2. **防止「明明做对的部分被改错」**：这是当前 loop-back 最隐蔽的风险——agent 重写文件时可能引入回归。合并系统能维护「已验证正确的版本」不变，只采纳 reviewer 要求修改的部分。

3. **为 reviewer 提供变化视图**：reviewer 现在只看到 agent 最终提交的代码。如果有合并系统，reviewer 可以在 diff-baseline 上工作——只检查本次变更的内容，不需要重新审阅不变的部分。

### 边界情况

- **语义冲突**：两个版本对同一个函数做了不同修改（一个修复 bug，一个增加了日志），合并需要检测修改区域重叠。
- **二进制文件**：`.png`、`.svg`、锁文件不适合行级 diff。`merge_strategy` 应自动检测二进制文件并回退到 `replace`。
- **loop-back chain**：三次 loop-back 后，有三个版本需要合并。合并策略需要处理 N > 2 的情况——通常是两两合并（iter1 + iter2 → merged, merged + iter3 → final）。
- **新建/删除文件**：如果旧版本有 file_d.go 但新版本删除了它，合并应该保留它（除非 reviewer 明确要求删除）还是采纳删除？——需要 `merge_strategy` 的删除策略配置。

---

## 方向五：跨模型输出共识验证——从「单模型信任」到「多模型交叉检查」

### 代码级证据

当前的路由系统确保**高质量模型用于高风险任务**：

```go
// internal/routing/routing.go
func TierFor(agent, mode string) string {
    if opusFloorAgents[agent] {
        return Opus  // architect/cto/reviewer 永远走 Opus
    }
    // ... 其他 agent 走 mode 默认 tier
}
```

但是**没有交叉验证**。一个 phase 由**一个模型**执行，结果被**一个模型**评审：

```
implementer (Sonnet) → 产生代码
reviewer (Opus) → 评审代码

问题: 
  - 如果 Opus 的评审是错的，无人发现
  - 如果 Sonnet 的实现在 reviewer 的盲区（reviewer 不理解的语言/框架）出了问题，无人发现
  - implementer 和 reviewer 共用同一供应商（anthropic），可能共享同类偏见/盲区
```

`HistoryTiebreak` 的 `candidate` 选择是为了**路由成本优化**而设计的，不是为了**质量验证**：

```go
// internal/routing/routing.go — candidate 是 tier 降级列表
func CandidatesForTier(tier string) []string {
    switch tier {
    case Opus:
        return []string{Opus, Sonnet, Haiku}  // 从高到低，不是交叉验证
    // ...
    }
}
```

同时，SCA 框架（`sca.mjs`）证明了 ForgeOS 可以运行**外部工具来验证 agent 输出**——但 SCA 只检查第三方依赖的漏洞，不检查 agent 自己写的代码的正确性。

记分卡 `scorecard.json` 的 `quality_score` 是**二值的**（基于 converge met/not-met），无法区分「两个不同模型都达标但一个更优」：

```go
// cmd/forge/scorecard_wind.go
modelQuality := 0.0
if verdict.accepted || verdict.converged {
    modelQuality = 1.0  // 所有成功都一样: 1.0
}
```

### 高价值扩展

**选择性跨模型共识验证（Selective Cross-Model Consensus）**——对高风险/高不确定性的 phase，部署第二个（不同供应商或不同 tier 的）模型对输出做独立验证：

```
标准路径:
  implementer (Sonnet) → 写代码 → gate → reviewer (Opus) → approve
  单模型执行，单模型评审

共识路径 (高风险):
  implementer (Sonnet) → 写代码
        │
        ├─▶ reviewer (Opus)         → approve/reject    ← 主评审
        │
        └─▶ consensus-check (Haiku2) → 验证              ← 轻量级交叉验证
              (不同供应商: openai/gpt-4o-mini)
              目的:
                - Reviewer OPUS 认为代码质量 8/10
                - Haiku2 独立评分: 如果也是 8/10 → 置信度高
                - 如果 Haiku2 评分 3/10 → 分歧 → 标记为高风险
                - 如果 Haiku2 评分 9/10 → 有 bias 吗? → 记录趋势
```

关键设计区别：

1. **共识检查器不是第二评审者**——它不执行完整的 STRIDE 威胁建模或架构审查。它只做**快速、低成本、独立的角度验证**：检查 reviewer 的结论是否有明显偏差或矛盾。

2. **共识检查只对筛选后的 phase 启用**（不是所有 phase）：
   - `lifecycle=production` 或 `mode=engineering` 时自动启用
   - 高风险路由（`risk == "critical"` 或 `risk == "high"`）时强制启用
   - 历史分歧率高的 agent 类型（scorecard 显示 `reviewer_approve_rate < 80%`）启用

3. **共识检查使用不同供应商**——以减少同源偏见。如果主路由是 Anthropic，共识检查可以路由到 OpenAI（或本地模型）。

**新增 `internal/consensus/consensus.go`**：

```go
// ConsensusResult 是交叉验证的输出
type ConsensusResult struct {
    Phase         string
    PrimaryTier   string
    PrimaryResult string  // reviewer 的裁决
    SecondaryTier string
    SecondaryScore float64 // 0.0-1.0 共识检查器的评分
    Agreement     ConsensusLevel // FULL | PARTIAL | DISAGREE
    Deviation     string   // 分歧描述
}

// CheckConsensus 对指定 phase 的输出做交叉验证
// secondary 是第二个 AgentExecutor（不同模型/供应商）
func CheckConsensus(primary, secondary orchestrator.AgentExecutor,
    phase asset.Phase, mode string) (*ConsensusResult, error) {
    // 1. 主 executor 执行（已经由标准流程做完）
    // 2. 副 executor 执行（轻量级，只检查主输出的关键结论）
    // 3. 比较两个输出
    // 4. 返回一致性评估
}
```

**为什么高价值**：

1. **检测 reviewer 盲区**：reviewer 也是模型——它也有幻觉、偏见、和「模式化」倾向。Sprint 25-26 的真实运行表明 reviewer 可以准确发现代码问题，但**无法保证 reviewer 自身的判断没有系统性盲区**。跨供应商检查提供了最基本的 sanity check。

2. **为记分卡提供供应商比较数据**：当前记分卡只记录了 `quality_score`（二值）和 `model` 标签。有了共识验证，就有了**同一个 task 在两个不同供应商/模型下的表现对比数据**——这是路由系统进行数据驱动的供应商选择的真实基础。

3. **对抗「假收敛」**：如果一个 agent 写的代码通过了所有 gate、获得了 reviewer 的 APPROVE，但 consensus checker 发现实现与需求有根本性不匹配——这就是当前系统完全无法检测的「假设正确但实际错误」场景。

### 边界情况

- **第二消费者的额外成本**：共识检查必然增加 token 消耗。应对策略：只在 `mode=engineering` 或 `lifecycle=production` 时启用；基于记分卡统计的动态启用（history 显示 reviewer 分歧率低 → 降低共识检查频率）。
- **两阶段分歧处理**：共识检查发现分歧后，不是简单否决——而是标记为「需人工介入」或传递给第二个（更高级）reviewer。
- **供应商特定能力差异**：如果主路由是 Anthropic 而共识检查是 OpenAI，某些任务（代码生成）可能两者都强，但某些任务（语义理解）可能有系统差异。共识检查需要维护每个任务类型 + 供应商对的校准基线。
- **非确定性输出**：同一模型对同一问题两次给出不同回答。共识检查的「分歧」可能是模型非确定性造成的，而非真正的判断差异。需要多次采样或阈值容忍。

---

## 总结：优先级、关联性与启动路径

| 方向 | 紧急度 | 代码影响 | 前置依赖 | 与已有分析的关系 | 核心收益 |
|------|--------|----------|----------|-----------------|----------|
| **① 自适应上下文预算** | P2 | 中（新增 `internal/prompt/budget.go`，修改 `GatherCached`） | 无 | 全新角度——已有分析未涉及上下文窗口的动态分配 | 防止代码库膨胀导致上下文浪费/丢失；从「静态截断」到「感知优先级」 |
| **② 跨工作流编排** | P1 | 大（新包 `internal/meta/` + `forge pipeline` 子命令 + `.agent/pipeline.yml`） | `engine_build.go` 的 workflow 构建器可复用 | 已有分析覆盖「多项目」和「单个 workflow 编排」，但未覆盖「5-spine workflow 之间的条件过渡」 | 实现「Idea→Production 全生命周期无人值守」的最后编排层 |
| **③ 输出合同校验** | P2 | 中（新增 `internal/validate/contract.go`，在 `engine_build.go` 的 phase 执行后插入） | 方向①（非必选但有助于理解 context 使用） | 与「Agent 输出多维质量评估」（v18 方向四）互补：v18 侧重质量趋势，本方向侧重合同遵守 | 抓 agent 输出偏离声明的第一道防线 |
| **④ 迭代输出合并** | P3 | 中（新增 `internal/consolidate/` 包 + Phase.MergeStrategy 字段） | 无 | 与「Loop-back 文件污染」（v16 方向二）相邻但不重叠——污染是残留文件，合并是内容覆盖 | 减少 loop-back 浪费，保护已验证正确的输出 |
| **⑤ 跨模型共识验证** | P3 | 大（新包 `internal/consensus/` + 第二 AgentExecutor 接口扩展 + 记分卡扩展） | 需要多供应商 API key（跨厂商池 v3） | 与「运行时模型质量自适应」（v7 方向二）相邻——但 v7 侧重选模型，本方向侧重交叉验证 | 对抗单模型盲区；为记分卡提供供应商比较基线 |

**推荐的启动路径**：

1. **最快启动**：方向③（输出合同校验）——独立于其他方向，可增量加入 `engine_build.go`。只需要实现 `VerifyPhaseOutput` 函数并在每个 agent phase 完成后调用。最小改动（<50 行核心代码 + 测试）即产生价值：在真点火中抓出缺失的 task-plan.md。

2. **中期高价值**：方向①（自适应上下文预算）——ContextCache 已经定义了「稳定前缀 vs 易变后缀」的边界，预算分配器可以逐步添加到现有架构中。先实现可用窗口测量 + P0-P2 分配；P3-P4 留待后续。

3. **长期愿景**：方向②（跨工作流编排）+ 方向⑤（共识验证）——都需要更深入的核心架构变更，但一旦落地，是实现 ForgeOS 全自动化愿景的关键两步。

---

*分析日期：2026-07-01 | forge-commit: 基于 Sprint 27 全量源码扫描（含 parallel engine、Context propagation、ContextCache）*  
*与 30+ 份已有分析文档逐份交叉核对，确认无重叠*
