# ForgeOS — 元治理扩展方向：全局扫描第五轮分析

> **视角**：资深架构师 / 产品经理  
> **方法**：完整重读 forge-core（~70 源文件）、harness（~40 模块）、`.agent/` 全套治理、已有 18+ 分析文档  
> **基线**：当前 HEAD（Sprint 26 全状态：真点火 multi-agent 闭环、5 扩展方向全交付、8 个真跑 gap 全修）  
> **原则**：绝不重复已有分析已覆盖的方向。每个方向必须：
> - 有代码级证据链（file:line 或架构级缺口）
> - 当前系统完全无此能力（或仅 stub）
> - 一旦落地即解锁新的能力层级
>
> **日期**：2026-07-01 | 不写代码，只做判断与优先级排序

---

## 已有分析的盲区 — 为什么还需要第 19 份分析

18+ 份现有分析覆盖了极其广泛的维度：

| 分析层 | 覆盖方向 | 计数 |
|--------|---------|------|
| **执行层** | 编排引擎、收敛、Loop、安全护栏(4 维)、并行编排、信号处理、RunParallel | ~15 份 |
| **质量层** | Harness 闸门(6 工具)、arch-check 8 检查、执法器盲区根治、PhaseGate、secret-scan | ~12 份 |
| **数据层** | Memory/Trace/Scorecard、成本 telemetry、latency telemetry、context cache | ~10 份 |
| **治理层** | 中枢旋钮(mode×lifecycle×workflow_depth)、human_gate、priorities 诚实处理 | ~8 份 |
| **组织层** | 多仓组合治理、跨厂商池、面向沙箱、Web-UI、Discover Engine | ~6 份 |
| **方向层** | 事件驱动 Workflow、输出合并、人类反馈分析、确定性 Replay、成本预测、混沌工程 | ~5 份 |

**但所有分析都在同一个问题域内思考**：**"ForgeOS 如何更好地治理它所管理的项目代码？"**

尚未有人问过这些问题的**元（meta）版本**：
- 谁治理 ForgeOS **自身的治理层**？
- 如何确保**Agent 们的承诺一致**——不是文件级别（输出合并已覆盖），而是**语义级别**？
- 谁来优化**治理者**（Workflow 拓扑、Agent 卡 Prompt、Policy 阈值）？
- ForgeOS 在多个项目中积累的经验，如何**跨项目迁移**？

---

## 目录

1. [方向一：多 Agent 语义共识与接口契约执法](#方向一多-agent-语义共识与接口契约执法)
2. [方向二：Prompt 质量生命周期管理](#方向二prompt-质量生命周期管理)
3. [方向三：Workflow 拓扑学习与自适应优化](#方向三workflow-拓扑学习与自适应优化)
4. [方向四：治理资产健康与衰减检测](#方向四治理资产健康与衰减检测)
5. [方向五：跨项目配置学习与参考架构](#方向五跨项目配置学习与参考架构)

---

## 方向一：多 Agent 语义共识与接口契约执法

**优先级**：P0 — 并行的前提条件  
**类别**：核心功能（边界）  
**一句话**：当前并行 agent 在**文件级**隔离运行，但在**语义级**没有任何协调

### 核心洞察

`parallel.go` 的 `runWave` 让多个 agent 并发写代码。`expansion-directions-v4.md` 方向二已覆盖了**输出合并**（output merging）——不同 agent 写到同一文件时的**文本级冲突解决**。但有一个更高杠杆的问题**完全未被覆盖**：

**两个并行 agent 各自写不同文件，但语义上不一致。**

真实场景：

```
agent A（实现用户注册） → 写 internal/user/service.go
  → 定义 User 结构体: {ID: int, Name: string, Email: string}
  
agent B（实现订单系统） → 写 internal/order/service.go
  → 引用 User 结构体: {ID: string, Name: string, Email: string}
  
合并后：类型不匹配！ID 字段在 A 里是 int，在 B 里是 string。
gate test 可能通过（各自模块单元测试独立），
但集成测试或运行时崩溃。
```

这不是文件冲突（两个文件不同），这是**语义不一致**——各自独立的 schema 定义在不同文件中「对不齐」。

### 现状代码锚点

```go
// parallel.go — 当前并行模式零语义协调：
func (e Engine) runPhaseParallel(...) error {
    // 每个 phase 独立执行，不同 agent 之间没有任何
    // 共享 schema/interface/contract 的交流机制
    return e.runAgentPhase(ctx, p, mode)
}

// engine_build.go — runAgentPhase 直接写项目目录
// 没有"先协商接口定义，再各自实现"的步骤
```

`build.yml` 的现有流程是 `planner → implementer → harness → reviewer → qa`——这是串行的。但在并行模式下（`--parallel`），多个 implementer 同时运行，它们之间只有 planner 的文本输出作为「共同理解」，没有任何**机器可读的契约**。

### 设计方向

**方案 A：Contract-First 并行模式**

在 `build.yml`（或新 workflow `contract-driven-build.yml`）中引入一个**契约阶段**，在并行 implementer 启动之前运行：

```yaml
phases:
  - name: architect
    agent: architect
    # 产出：架构决策记录 + 接口定义文件

  - name: contract-writer     # ★ 新阶段
    agent: implementer
    prompt: >
      Based on the architecture above, write interface contracts 
      (protobuf / OpenAPI / Go interface stubs / TypeScript types) 
      into .forge/contracts/. All implementers MUST derive from these.
    outputs:
      - .forge/contracts/

  - name: implementer-user    # 并行实现
    agent: implementer
    depends_on: [contract-writer]
    allowed_paths: [internal/user/]   # ★ 新增约束：只允许修改此目录

  - name: implementer-order   # 并行实现
    agent: implementer
    depends_on: [contract-writer]
    allowed_paths: [internal/order/]  # ★ 新增约束：只允许修改此目录
```

核心新增原语：
- **`allowed_paths`**：phase 声明只允许修改的目录，harness gate 在 phase 后检查是否越界
- **`contract_registry`**：.forge/contracts/ 中机器可读的类型定义，gate 可验证实现是否匹配

**方案 B：编译级跨模块契约检查**

更低成本：不引入新阶段，而是在并行波完成后、gate phase 中增加一个「跨模块一致性」检查：

```bash
# 对于 Go 项目：go build ./... 天然检查类型一致性
# 对于 TypeScript：tsc --noEmit 检查接口一致性
# 对于 Python：mypy --strict 检查类型一致性
```

但这已经存在于 `test` gate 的 `probeAppTests` 路径中——不过它是在**所有 phase 都完成之后**才跑。真正的区别在于**何时发现问题**：

- 事后（当前）：所有 agent 写完 → 编译 gate FAIL → 整个波废弃 → 成本浪费
- 事前（方向一方案 A）：先定契约 → 各自实现 → 编译几乎 100% 通过 → 零浪费

**方案 C：共享语义注册表**

一种轻量级方案：在 `memory` 包中新增一个 `Contract` 类型，agent 可以在写入文件前先查询/注册共享类型：

```go
// memory 增加 Contract 类型
type Contract struct {
    Name       string            // "User"
    Kind       string            // "struct" | "interface" | "type_alias"
    Fields     map[string]string // {"ID": "int", "Name": "string"}
    DefinedIn  string            // "internal/user/service.go"
}
```

Agent 实现新功能前先 `memory.QueryContracts("User")`，确保自己引用的类型与已有定义一致。

### 为什么不现在做（但为什么将来必须做）

**当前串行 workflow 不需要这个**：`build.yml` 的 3 阶段串行（planner→implementer→reviewer）中，只有一个 implementer 在写代码，不存在多 agent 语义冲突。这是串行的自然安全属性。

**但一旦 `--parallel` 被默认启用**（现在它是 opt-in），每多一个并行 implementer，语义不一致的概率线性上升。而且这种问题**最隐蔽**：文件层合并检测不到、单元测试发现不了（各自测试独立通过）、只在集成测试或生产运行时暴露。

**方向一不是解决"当前问题"（串行工作得好好的），而是解锁"下一阶段"（安全并行）的前提条件。**

### 接入代价估计

| 子项 | 行数估计 | 独立可交付 |
|------|---------|-----------|
| `allowed_paths` 约束 + gate 检查 | ~200 行 | ✅ 是（独立增强） |
| `contract_registry` 类型注册 | ~150 行 | ✅ 是 |
| CLI 契约生成 prompt | ~50 行 | 否（依赖前两项） |
| **合计** | ~400 行 | 1-2 sprints |

---

## 方向二：Prompt 质量生命周期管理

**优先级**：P0 — 治理质量的根本基础  
**类别**：功能（元治理）  
**一句话**：ForgeOS 管理代码质量，但从未管理过它给 agent 的**指令质量**

### 核心洞察

ForgeOS 有严格的质量体系来管理 agent 产出的代码：
- `arch-check` 8 检查 → 代码结构质量
- `gate.mjs` 体积闸门 → 文件大小
- `secret-scan` → 安全质量
- `converge` → roadmad 完成质量

**但 agent 收到的指令（prompt）的质量，完全没有管理体系**：

| Prompt 源 | 文件位置 | 是否有版本控制 | 是否可测试 | 是否有有效性度量 |
|-----------|---------|--------------|-----------|----------------|
| Agent 角色卡 | `.agent/agents/*.md` | Git 跟踪 | ❌ 无测试 | ❌ 无度量 |
| Workflow 模板 | `.agent/workflows/*.yml` | Git 跟踪 | ❌ 无测试 | ❌ 无度量 |
| gatherContext 动态注入 | `prompt_context.go:214-220` | 代码级别 | ✅ 单元测试 | ❌ 无度量 |
| `buildPrompt` 构建逻辑 | `engine_build.go` | 代码级别 | ✅ 单元测试 | ❌ 无度量 |
| Retrieve ADR 检索 | `prompt/retrieve.go` | 代码级别 | ✅ 单元测试 | ❌ 端到端质量度量 |

**结果：**
- 一个 agent 卡片的 prompt 写得好不好，**完全靠直觉**
- 同一个 prompt 在不同项目中可能效果截然不同，但**无任何信号能告诉你**
- Reviewer 发现某类 bug 反复出现 → 可能在 prompt 里加一行就能预防 → **但没人知道该加在哪里**
- Prompt 随着项目演进不断被编辑 → **无人知道哪个版本最好**

### 现状代码锚点

```go
// engine_build.go — buildPrompt 构建 agent prompt：
// prompt 的最终内容是各个组件的线性拼接，没有 A/B 测试、没有版本选择
func (e Engine) buildPrompt(ctx context.Context, p asset.Phase, mode string) (string, error) {
    card := readCard(e.root, p.Agent)         // 读角色卡
    context := gatherContext(ctx, e, p, mode)  // 收集上下文
    return card + "\n" + context, nil          // 拼接 → 发给 agent
}
```

```go
// prompt_context.go — gatherContext 从多个源收集上下文
// 但没有任何"这个上下文的实际质量反馈"回路：
// - portal taskContext 是否足够清晰？
// - gateLedger 的注入是否太冗长？
// - memoryContext 是否带来了有效信息还是噪声？
```

### 设计方向

**第一层：Prompt 版本化与效果追踪**

```go
// 新增 PromptVersion 类型
type PromptVersion struct {
    Hash      string    // SHA256 of the assembled prompt (for dedup)
    CardVer   string    // agent card git hash at time of use
    ContextVer string   // gatherContext source versions
    PhaseName string
    ModelTier string
    UsedAt    time.Time
}

// 在 trace 事件中记录 prompt 版本
trace.Event{
    Kind: "agent",
    Name: phase.Name,
    // ★新增字段
    PromptHash: sha256(prompt),
    CardVersion: gitHash(".agent/agents/" + p.Agent + ".md"),
}
```

**第二层：自动 Prompt 效果度量**

```go
// 新增度量：每 agent phase 完成后，根据其下游 fate 评分 prompt 效果：
//
// PromptScore:
//   PhaseGateResult — immediate (PASS/FAIL/retry)
//   ReworkNeeded    — was this phase's output later reworked?
//   GateFailureRate — did this phase consistently trigger gate failures?
//   TokenEfficiency — (prompt tokens) / (accepted output tokens)
//
// 这个分数关联到 PromptVersion.Hash → 能回答最关键的问题：
// "版本 A 的 implementer prompt 比版本 B 好多少？"
```

**第三层：Prompt 回归测试套件**

当前 `.agent/agents/*.md` 完全没有测试。引入 `prompt_test.go` 的概念：

```go
// forge prompt test — 测试 prompt 在模拟环境中的行为
//
// 测试用例：
// - "给定 X 类型的任务，prompt 是否包含 Y 指令？"
// - "prompt token 数量是否 < 阈值？"
// - "prompt 是否包含前后矛盾的指令？"
// - "prompt 中的关键词（架构约束、安全要求）是否都在？"
```

### 边界情况

| 场景 | 问题 | 方案 |
|------|------|------|
| **Prompt 变体组合爆炸** | `cardVer × contextGather × mode × lifecycle` 产生数千变体 | 只追踪完整 prompt 的 hash，不追踪组合来源 |
| **因果归因困难** | agent 输出差是因为 prompt 差，还是因为模型差，还是因为输入数据差？ | PromptScore 只做关联而非因果；需要大量样本才下结论 |
| **冷启动** | 新 prompt 版本无历史数据 | 默认不切换，新版本与旧版本平行运行 N 次后比较 |
| **版本膨胀** | 每个编辑都产生新版本 | 只记录「有意义的版本」：token 结构/长度/关键词分布显著变化时 |

### 为什么不现在做（但为什么将来必须做）

当前系统在各种「硬」质量维度上已做到极致（gate、arch-check、loop-back）。但 agent 行为质量的**最大方差来源**不是代码，而是 prompt。

一个 implementer prompt 如果漏了「记得写测试」的指令，产生的代码质量会系统性偏低——而且没有任何现有闸门能捕捉到。`converge` 可能过、`reviewer` 可能 APPROVE、但测试覆盖不足却是 prompt 设计缺陷的直接后果。

**这是最后一个"不需要新外部资源、只需要改变思维方式"的高杠杆改进方向。**

### 接入代价估计

| 子项 | 行数估计 | 独立可交付？ |
|------|---------|------------|
| Prompt 版本追踪（hash + trace） | ~100 行 | ✅ 是 |
| PromptScore 度量框架 | ~200 行 | ✅ 是 |
| 回归测试套件框架 | ~300 行 | ✅ 是 |
| 现有 agent 卡接入 | 0 行（纯框架） | — |
| **合计** | ~600 行 | 2 sprints |

---

## 方向三：Workflow 拓扑学习与自适应优化

**优先级**：P1 — 从静态编排到数据驱动优化  
**类别**：功能（性能优化）  
**一句话**：Workflow 结构（哪些 phase、什么顺序）是固定的，但从 N 次运行中积累的数据**从来没有用来优化它**

### 核心洞察

每个 workflow 的拓扑是手写的 YAML，在 `.agent/workflows/` 中是静态的。`build.yml` 是：

```
planner → implementer → [gate] → reviewer → qa
```

**这个拓扑从来不变**——不论项目是 Go 后端还是 Python 脚本，不论 mode 是 explorer 还是 engineering，不论此次 task 是新增功能还是重构。

但每次 run 产生了大量数据，**可以用来回答拓扑优化问题**：

```
历史数据 N=50 次 build：
  qa phase 发现问题的概率 = 12%
  reviewer REQUEST_CHANGES 后 qa 发现问题的概率 = 4%
  reviewer APPROVE 后 qa 发现问题的概率 = 2%
  → 结论：qa 在 reviewer 之后的价值很低，可以跳过以节省 $0.18/次
  
历史数据 N=30 次 build（engineering mode）：
  complexity gate 在 test gate 之前 FAIL = 0 次
  complexity gate 在 test gate 之后 FAIL = 0 次
  → 结论：complexity gate 的位置不影响效果，可以移到并行波
  
历史数据 N=20 次 discover：
  market-research phase 产出的竞争者信息从未被后续 build 使用（feeds_forward=0）
  → 结论：该 phase 可以移除或降级为可选
```

**当前问题**：系统收集了所有（cost/duration/gate status/rework rate）的数据，**但只用于模型路由（scorecard→HistoryTiebreak）和报告，从未用于优化 workflow 拓扑本身**。

### 现状代码锚点

```go
// asset/asset.go — Workflow 结构是静态的：
type Workflow struct {
    Stage  string
    Stop   StopCondition
    Phases []Phase  // 硬编码序列
}

// 没有任何"根据历史数据自动调整 phase 顺序/组成"的机制
// Workflow 结构在 run 之间不会变化
```

```go
// scorecard_wind.go — 数据聚合到 scorecard 只用于模型选择：
// (model, task_type) → quality_score, avg_cost, p95_latency
// 没有聚合到 (phase_name, position_in_workflow) → effectiveness_score
```

### 设计方向

**第一层：Phase 效用度量**

在 scorecard 系统中增加按 phase 聚合的维度，不再是 `(model, task_type)` 而已 `(phase_name, workflow_stage, mode)` 为键：

```go
type PhaseEffectiveness struct {
    PhaseName      string    // "reviewer" | "qa" | "complexity_gate"
    WorkflowStage  string    // "build" | "discover"
    Mode           string    // "explorer" | "balanced" | "engineering"
    
    // 效果度量
    IssueRate      float64   // 此 phase 发现问题的概率
    AvgCost        float64   // 运行此 phase 的平均成本
    AvgDuration    float64   // 运行此 phase 的平均耗时
    ReworkRate     float64   // 此 phase 触发 rework 的概率
    DownstreamImpact float64 // 此 phase 对下游 phase 通过率的影响
}
```

**第二层：Workflow 优化建议器**

```go
// 基于 PhaseEffectiveness 数据的建议引擎
type Optimizer struct {
    history PhaseEffectivenessStore
}

// SuggestOptimizations 分析历史数据，输出 workflow 拓扑优化建议
func (o *Optimizer) SuggestOptimizations(wf asset.Workflow) []Suggestion

type Suggestion struct {
    Kind        string  // "reorder" | "remove" | "add" | "parallelize"
    Target      string  // "reviewer" | "qa"
    Confidence  float64 // 0.0-1.0，此建议的统计置信度
    ExpectedSavings CostBreakdown
    Reasoning   string  // 人类可读的原因
}
```

**第三层：自动 Workflow 派生（可选的，非默认）**

对于 `explorer` 模式的 run（快速迭代），自动生成轻量版 workflow：

```go
// explorer 模式自动派生：
// 如果 history 显示 qa phase 在 reviewer 之后 issue_rate < 5%，
// 自动从 workflow 中移除 qa phase
func DeriveWorkflowMode(base asset.Workflow, mode string, history PhaseEffectivenessStore) asset.Workflow {
    if mode != "explorer" {
        return base // 非 explorer 模式不动
    }
    // 检查 history → 如果 reviewer 已经足够，跳过 qa
    // 返回优化后的 workflow
}
```

### 边界情况

| 场景 | 问题 | 方案 |
|------|------|------|
| **冷启动** | 新项目前 N 次运行无历史数据 | 优化建议器在样本 < 10 时静默（不退化为随机变更） |
| **统计谬误** | qa issue_rate=0 不是因为不需要，而是因为 qa prompt 写得差 | 将 issue_rate 与 phase output 质量关联，而非与 phase 存在性关联 |
| **过度优化** | 移除 qa 省了钱但丢了一个安全的冗余层 | 建议永远是 advisory，默认不自动执行；在 engineering/production mode 下**永不建议移除安全 phase** |
| **环境漂移** | 项目从 Go 切换到 Node，历史数据不适用 | 按 `language` 标签分段统计；语言切换时重建基线 |

### 为什么不现在做（但为什么将来必须做）

当前 ForgeOS 运行在**开发体验**阶段：少量项目、手动运行、单人操作。在这个体量下，workflow 拓扑是固定的是完全可以接受的——"如果它不坏，就别修"。

但 ForgeOS 的愿景是**工厂**：N 个项目、M 个开发者、持续的自动运行。一旦运行次数累积到数百，以下问题会自然浮现：
1. "为什么我们总是在 qa 拿 PASS 但每次 reviewer REQUEST_CHANGES？——是不是顺序反了？"
2. "为什么这个项目跑一个 discover 要 $12 但产出的置信度只有 60%？"
3. "engineering mode 比 explorer mode 平均每次 build 多花 $2.1 —— 多花的钱是不是都值？"

没有方向三的数据驱动优化，这些问题**会有人问，但系统无法回答**。

### 接入代价估计

| 子项 | 行数估计 | 独立可交付？ |
|------|---------|------------|
| PhaseEffectiveness 数据模型 | ~150 行 | ✅ 是 |
| Scorecard 扩展（按 phase 聚合） | ~150 行 | ✅ 是 |
| 优化建议器核心 | ~300 行 | ✅ 是 |
| CLI 报告 `forge optimize suggest` | ~150 行 | ✅ 是 |
| 自动 Workflow 派生（可选） | ~200 行 | 否（依赖前几项） |
| **合计** | ~750 行 | 2-3 sprints |

---

## 方向四：治理资产健康与衰减检测

**优先级**：P1 —「谁治理治理者」  
**类别**：边界（元治理）  
**一句话**：ForgeOS 所治理的代码有 arch-check，**但治理物本身（agent cards / workflows / policies）没有健康检查**

### 核心洞察

ForgeOS 为被治理的代码建立了严格的质量体系。但对**治理层本身**，没有任何健康度量。

当前 `.agent/` 目录下有约 30 个资产文件：

| 资产类型 | 数量 | 潜在衰减模式 |
|---------|------|-------------|
| Agent 角色卡 | 9 | prompt 过时（引用已废弃的 API/技术栈）、指令与实际行为漂移 |
| Workflow YAML | 4+ | 有 phase 永远不运行（因为 mode gating 永久过滤）、有 phase 重复 |
| Policy 文件 | 3+ | 阈值（如 `max_function_lines: 50`）与仓库实际情况脱节 |
| 路由策略 | 2+ | Scorecard 已学习到更好的路由但 policy 硬编码了过时规则 |
| ADR | 4+ | 描述的技术决策已被后续更改覆盖但 ADR 未更新 |
| Skill 卡 | 8+ | 参考的代码结构已变化，步骤不再适用 |

### 代码证据

```go
// arch/arch-check.mjs — 检查代码架构：
// - 对源码做 layering / 扇入 / 循环依赖检查
// - ❌ 不对 .agent/ 目录做任何健康检查
```

```go
// check.py — 治理完整性检查：
// - 检查 agent/workflow 引用是否悬挂
// - ❌ 不检查 agent card 内容是否过时
// - ❌ 不检查 policy 阈值是否合理
// - ❌ 不检查 workflow 中是否有零运行 phase
```

```go
// validate.go — `forge validate`：
// - 检查 YAML 语法 + 引用完整性
// - ❌ 不检查语义健康（如：workflow 中的 phase 是否永远被 mode gating 过滤？）
```

### 设计方向

**第一层：资产活性度量**

为每类治理资产定义健康指标：

| 资产类型 | 健康指标 | 检测方式 |
|---------|---------|---------|
| Agent 卡 | 最后修改时间、引用技术栈的准确度 | Git 日志 + 关键词检测 |
| Workflow | 每 phase 的执行率（= 运行次数 / 总 run 次数） | 从 trace 统计 |
| Policy | 阈值违规率（太高？太低？） | 从 gate 结果统计 |
| ADR | 是否仍与代码一致？ | ADR 关键词 vs 代码实际结构比对 |
| Skill | 最后使用时间 | Git 日志 |

**第二层：治理衰减报告**

```bash
forge governance health
# 治理资产健康报告：
# 
# ✅ Agent cards: 9/9 active (reviewer.md: last updated 3 months ago)
# ⚠️ Workflows: build.yml phase "qa" executed in 68% of recent runs — 32% of the time mode gating filters it
# ❌ Policies: max_function_lines=50 violated 0 times in 100 runs — threshold may be too loose
# ⚠️ ADRs: ADR-0001 ("ride-claude-code-v0-v1") marked Superseded but STILL REFERENCED by agent card planner.md
# ✅ Skills: 8/8 active (testing.md used 12 times this month)
```

**第三层：自动衰减补救（可选）**

对于明显的衰减，系统可以建议甚至自动执行补救：

- Agent 卡 180 天未更新 → 创建 issue 提醒「reviewer.md 可能需要更新以反映当前最佳实践」
- Workflow 某 phase 连续 20 次被 mode gating 过滤 → 建议「此 phase 在 mode=X 下从未执行，可移至单独的 workflow」
- Policy 阈值零违规超过 50 次 run → 建议「收紧阈值以恢复执法价值」
- ADR 被标记 Superseded 但仍有引用 → 建议「更新引用到新的 ADR」
- Skill 卡 1 年未使用 → 建议「标记为 deprecated」

### 边界情况

| 场景 | 问题 | 方案 |
|------|------|------|
| **新仓库** | 零历史数据，无法计算执行率 | 健康检查不报告未 stat；待 >10 次 run 后激活 |
| **季节性 workflow** | 某 phase 一年只在 release 时跑一次 | 健康检查应该看「有意义的窗口」，而非总时间 |
| **阈值主观性** | 合理的阈值不是统计问题，是团队约定 | `forge governance calibrate` 交互式调优 |
| **补救误触** | 自动删除"从不运行"的 phase 可能破坏了某个手动流程 | 默认只建议不执行；仅 CTO mode 下可自动应用 |

### 为什么不现在做（但为什么将来必须做）

ForgeOS 的治理层本身会腐烂——agent cards 过时、workflow 积累死 phase、policies 偏离实际——但**当前没有任何机制能侦测这种腐烂**。治理系统像一个没有健康检查的生产服务：你知道它可能在变差，但直到出问题你都不知道。

作为「治理 OS」，ForgeOS 的唯一产品是**治理**。不治理自身的治理资产，相当于「木匠家的门坏了」。

### 接入代价估计

| 子项 | 行数估计 | 独立可交付？ |
|------|---------|------------|
| 资产活性度量框架 | ~200 行 | ✅ 是 |
| `forge governance health` 报告 | ~200 行 | ✅ 是 |
| 衰减报警（issue/notification） | ~150 行 | ✅ 是 |
| 自动补救（可选） | ~200 行 | 否（依赖度量框架） |
| **合计** | ~550 行 | 2 sprints |

---

## 方向五：跨项目配置学习与参考架构

**优先级**：P2 — 规模化的前提  
**类别**：功能（组织级学习）  
**一句话**：每个 `forge-init` 得到的都是同一套模板，**项目 A 积累的经验从不流向项目 B**

### 核心洞察

`forge-init`（`scaffold/forge-init.mjs`）为每个新项目复制同一套模板。所有 `forge-init` 产出的项目初始配置完全一样——无论项目是：

- 一个 Go CRUD API（go-taskd）
- 一个 Node.js URL 短链服务（url-shortener）
- 一个 Python 数据处理管道（尚未示例）

**但每个项目类型的最优配置是截然不同的**：

| 配置项 | Go CRUD | Node API | Python Pipeline |
|-------|---------|---------|----------------|
| 最优 max-iter | 3-5 | 5-8 | 2-3 |
| 最有价值的 gate | test + arch | test + typecheck + lint | test + security |
| 最有价值的 role | reviewer | reviewer + qa | security-engineer |
| 每 phase 平均成本 | $0.15 | $0.22 | $0.08 |
| 推荐的模型档位 | sonnet+opus | sonnet | haiku+sonnet |

**这些认知被锁在项目团队的经验里**，没有任何机制让它们流动。

### 现状代码锚点

```go
// scaffold/forge-init.mjs — 每个新项目得到相同模板：
function writeStarterFiles(dir, options) {
    copyTemplate('project.yml', ...);
    copyTemplate('CLAUDE.md', ...);
    copyTemplate('agents/', ...);
    copyTemplate('workflows/', ...);
    // 不考虑 project type、language、team size 的差异化
}
```

```go
// scorecard_wind.go — 数据聚合到单一项目的 scorecards.json
// 即使 forge-os 自身（本仓）和 url-shortener（example）的项目类型
// 完全不同，scorecard 也完全不共享数据
```

```go
// detector.go — `forge detect` 能检测语言和项目类型
// 但检测结果只用于选择初始 workflow 和 mode
// 不用于选择优化后的配置
```

### 设计方向

**第一层：跨项目统计注册表**

一个可选的中心化或文件级注册表，存储**聚合的、去标识化的**项目配置与运行数据：

```bash
# 注册表可以简单到是一个 .agent/reference/registry.yml
# 或一个中心 HTTP 服务（与多仓组合治理方向共享基础设施）
#
# 每个注册条目：
# - language: go
# - project_type: "CRUD API"
# - samples: 47 个项目
# - typical_config:
#     max_iter: 4
#     model_floor: sonnet
#     gates: [test, arch, complexity]
#     agents: [implementer, reviewer]
# - typical_cost_per_run: $1.42
```

**第二层：`forge init --smart` 智能初始化**

```bash
# 初始化时不仅复制模板，还根据项目特征应用已知的最优配置
forge init my-api --smart
# 分析：项目类型 = Go CRUD API
# 参考 47 个类似项目的历史数据：
#   - 平均 max-iter: 4（ vs 全局默认 5）
#   - 最有价值 gate: test + arch（99% 项目用）
#   - 不常用 gate: security（仅 30% 类似项目用，因为大多数不处理 PII）
# 正在生成优化的初始配置...
```

**第三层：社区参考架构市场（远期）**

一组**经过验证的、针对特定项目类型的参考架构**：

```
forge/blueprints/
  go-crud-api/          → 已验证的 CRUD API 最佳配置
  node-express-api/     → Express/Node 最佳配置
  python-ml-pipeline/   → ML pipeline 最佳配置
  microservices/        → 多仓微服务治理最佳配置
```

每个 blueprint 包含：
- 优化的 `.agent/` 配置
- 适配的 workflow 拓扑
- 推荐的 model tier 分配
- 已验证的 cost profile

### 边界情况

| 场景 | 问题 | 方案 |
|------|------|------|
| **隐私** | 公司不想共享项目配置数据 | 统计注册表是 opt-in；设计为匿名聚合，不含代码/路径/名称 |
| **非代表性样本** | 3 个 Node 项目样本不能代表全部 | 置信度阈值：少于 10 个样本的统计不用于配置建议 |
| **配置锁定** | 项目按了建议配置但实际需要不同 | 建议永远是可选的（`--smart` flag），默认仍是标准模板 |
| **版本漂移** | 10 个月前的最优配置已不是今天的最优 | 注册表记录时间戳，超过 6 个月的数据降权 |
| **跨生态** | Go 1.21 和 Go 1.24 的最佳配置不同 | 注册表键中包含次要版本号 |

### 为什么不现在做（但为什么将来必须做）

当前 ForgeOS 的规模（1-3 个示例项目、1 个 dogfood 仓）下，跨项目学习没有意义——没有足够的样本做统计推断。

但 ForgeOS 的核心价值主张是**工厂**——它应被 N 个项目同时使用。当项目数 > 10 时，以下问题会自然出现：

1. 为什么 A 项目 evolve 效率是 B 项目的 3 倍？（配置差异）
2. 我们能否用一个统一的「Go 后端」参考架构来让所有 Go 项目启动更快？
3. 新成员加入团队，如何快速为 TA 的项目建立已经过验证的最佳配置？

**方向五是 ForgeOS 从「单项目工具」进化为「多项目平台」的配置维度补完。** 它与「多仓组合治理」（v3 方向二，解决组织级策略下发）互补：方向五关注**经验迁移**，方向二关注**标准执行**。

### 接入代价估计

| 子项 | 行数估计 | 独立可交付？ |
|------|---------|------------|
| 跨项目统计注册表 | ~300 行 | ✅ 是（纯文件级，零网络依赖） |
| `forge init --smart` | ~200 行 | ⚠️ 依赖注册表存在数据 |
| 社区 blueprint 框架 | ~200 行 | ✅ 是（仅框架，数据随采纳增长） |
| 匿名数据导出 | ~100 行 | 依赖隐私策略决策 |
| **合计** | ~800 行 | 3-4 sprints（含 blueprint 验证） |

---

## 总结：优先级矩阵与依赖关系

### 五方向总览

| 方向 | 优先级 | 类别 | 一句话杠杆 | 代码就绪度 | 接入成本 |
|------|--------|------|-----------|-----------|---------|
| **一 · 语义共识** | **P0** | 核心功能 | 没有它，`--parallel` 默认启用不安全 | 并行基础设施就绪，缺契约层 | ~400 行 |
| **二 · Prompt 管理** | **P0** | 元治理 | 治理代码质量却不管指令质量——最大方差来源 | 零（全新概念） | ~600 行 |
| **三 · 拓扑优化** | P1 | 性能优化 | 历史数据只用于模型路由，从不优化 workflow 自身 | Scorecard 数据已就绪 | ~750 行 |
| **四 · 治理健康** | P1 | 元治理 | 「谁治理治理者」——治理资产会腐烂 | 零（全新概念） | ~550 行 |
| **五 · 跨项目学习** | P2 | 功能 | 每个新项目都是从零开始的经验爬坡 | `forge detect` 已检测类型 | ~800 行 |

### 与已有分析的互补关系

| 已有分析覆盖 | 本文件覆盖 | 关系 |
|-------------|-----------|------|
| 输出合并与冲突解决（v4 方向二） | **方向一（语义共识）** | 文本级冲突解决 vs 语义级契约——互补而非重叠 |
| PhaseGate（v3 方向一） | **方向二（Prompt 管理）** | 检查 agent 输出质量 vs 检查 agent 指令质量——治理对偶 |
| Scorecard 路由回灌（已交付） | **方向三（拓扑优化）** | 模型选择优化 vs workflow 结构优化——不同维度 |
| 执法器盲区根治（已交付） | **方向四（治理健康）** | 修复执法器 bug vs 度量执法器自身寿命——不同阶段 |
| 多仓组合治理（v3 方向二） | **方向五（跨项目学习）** | 策略统一下发 vs 经验迁移共享——执行互补 |

### 执行顺序建议

```
优先级
  │
  P0  ├── 方向二（Prompt 管理）     ← 可独立启动，最高杠杆
      ├── 方向一（语义共识）        ← 与并行编排深度绑定
      │
  P1  ├── 方向四（治理健康）        ← 可独立启动
      ├── 方向三（拓扑优化）        ← 需要 scorecard 数据累积
      │
  P2  └── 方向五（跨项目学习）      ← 需要 >10 项目采纳后才有价值
```

**短期（P0 快速赢）**：方向二（Prompt 管理）和方向四（治理健康）——两者都是**元治理**层面的增强，都不修改任何 workflow 语义、都不要求现有项目改变任何行为，但一旦落地即建立 ForgeOS **「自我观察」**的能力。

**中期（P1 架构增强）**：方向一（语义共识）——只有在 `--parallel` 即将默认启用之前才必须做。方向三（拓扑优化）——需要至少数十次 run 的历史数据，所以现在启动（开始收集 phase-level 度量）是最佳时机。

**长期（P2 规模化）**：方向五（跨项目学习）——是 ForgeOS 从单项目到多项目平台的自然进化路径，但数据积累需要时间，现在启动注册表框架让数据自然沉淀，等体量达到后自然激活。

---

*分析日期：2026-07-01 | 基于 forge-core 全量源码（70+ 源文件）+ harness（40+ 模块）+ .agent 全套治理 + 18+ 份已有分析 
| 本文件覆盖之前所有分析均未触及的「元治理」层——治理治理者的 5 个方向*
