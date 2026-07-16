# ForgeOS — 五个高价值扩展方向（全局扫描 · Sprint 31 基线）

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全仓全局深扫: forge-core 18+ Go 包（~7.3k non-test LOC + 11.2k test LOC）、  
>    `cmd/forge` 16+ CLI 子命令、harness 39+ 模块、`.agent/` 完整治理骨架  
> 2. 通读所有已有扩展分析文档（docs/requirements/ 34 篇 + docs/analysis/ 41 篇 + 其余 docs，共约 85+ 已有方向）  
> 3. 逐方向交叉验证核心论点——确保本文 5 个方向在已有分析中**从未作为独立方向展开**  
> 4. **纪律**: 不编写任何代码。每个方向附代码级证据 + 边界场景 + 与已有分析的明确边界。  
> **日期**: 2026-07-10  

---

## 已有覆盖全景（本文不重复的方向）

以下域已被 ~85+ 已有方向充分覆盖，本文不重复讨论其核心论点：

| 已被充分覆盖的域 | 代表性文档 | 方向数 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back） | 大部分 requirements 文档 | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md` · `v34.md` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 健康契约） | `expansion-production-readiness.md` · `v34` | ~8 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md` · `v33` | ~10 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` | ~10 |
| 系统边界盲区（级联截断/信任边界/持久语义/可移植性/并行安全/并发隔离） | `strategic-extensions-v22~v33.md` · `v38` | ~12 |
| Go 库 API 边界契约 / 测试元治理 / 混沌韧性验证 / 产物质量治理 / Schema 版本化 | `structural-gaps-v41.md` | ~5 |
| 跨进程守护 / 治理热加载 / Trace CLI / 可插拔扩展 / 状态自校验 | `forgotten-five-foundations.md` | ~5 |
| 二进制分发 / 状态灾难恢复 / 结构化输出 / 多会话协调 / 数据生命周期 | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` | ~5 |
| 工作内存预算 / 文件血统追踪 / 跨迭代一致性审计 / 自升级协议 / 上下文压力测试 | `strategic-extensions.md` | ~5 |
| 执行语义日志 / Phase 间意图一致性 / 内部遥测 / 产出物 Schema 强制 / 配置漂移检测 | `five-genuinely-uncovered-frontiers.md` | ~5 |
| **总计已有覆盖** | **75+ 份文件** | **~85+ 方向** |

---

## 本文的五个方向

| # | 方向 | 分类 | 核心代码影响 | 优先级 |
|---|------|------|-------------|--------|
| 1 | **Prompt 结构敏感性 & Ablation 测试框架** | 质量 · 可演化性 | `cmd/forge/prompt_context.go` · `internal/prompt` | 🟡 P2 |
| 2 | **跨 Workflow 管线执行引擎** | 架构 · 自动化 | `asset.Workflow` · 新 pipeline 执行器 | 🔴 P1 |
| 3 | **Agent 输出语义正确性闸门** | 治理 · 信任 | `internal/converge` · `internal/gate` · 新 semantic gate | 🔴 P1 |
| 4 | **基于历史遥测的预测性运行估算** | 经济 · 可观测性 | `internal/routing/scorecard.go` · `internal/converge` | 🟠 P1 |
| 5 | **多 Agent 分歧裁决协议** | 韧性 · 治理 | `orchestrator.go` loop-back 逻辑 · 新 resolution 层 | 🟠 P2 |

---

## 方向一 · Prompt 结构敏感性 & Ablation 测试框架

**类型**: 质量 · 可演化性  
**优先级**: P2 | 预估: ~1.5 sprint

### 差异化证明

在全部已有 85+ 方向中，搜索以下概念**零命中**为独立方向：

| 关键词 | 命中情况 |
|--------|---------|
| prompt ablation / prompt sensitivity / prompt structure testing | 零独立方向 |
| prompt A/B test / prompt versioning | `five-uncovered-frontiers.md` 方向二提及"prompt snapshot"但关注**可观测性**（记录 prompt 组成），非 **prompt 结构敏感性实验** |
| prompt order optimization / section ordering | 零 |
| model-specific prompt layout (Opus vs Haiku attention differences) | 零 |

最近的 `strategic-extensions.md` 方向二讨论「工作记忆预算与动态上下文装配」——关注的是 **什么内容进入 prompt**（容量/优先级），而非 **prompt 的结构和顺序如何影响 agent 行为**。两者正交。

### 现状：代码级证据

`cmd/forge/prompt_context.go` 的 prompt 装配是**固定顺序**的硬编码序列：

```go
// prompt_context.go:340-430 (buildPromptWithEmits)
//
// 组装顺序（硬编码）：
//   1. 角色卡（readCard → agent card 全文）
//   2. 硬约束（AGENTS.md 红线 bullet）
//   3. Task（ROADMAP.md 当前项）
//   4. ADR（Gather → 最多 adrTopK=6 条）
//   5. Memory（appendMemoryContext → 最多 memoryCap=32 条）
//   6. Gate 裁决（gateLedger → harness 结果）
//   7. Phase 输出（phaseOutputLedger → feeds_forward 内容）
//   8. Reviewer 发现（reviewFindingsLedger → REQUEST_CHANGES 详情）
//   9. Emits 产物（appendArtifactContext → 前序 phase 产出文件）
//   10. Uses/Secondary Template（appendTemplateLane → .ai/prompts/*.md）
//
// 这个顺序在整仓中只有一个版本，从未被测试过是否最优。
```

**证据 A：无 Prompt 版本化**

`internal/prompt/prompt.go:24-26` 只定义了容量常量：

```go
const adrTopK = 6      // ADR 最多 6 条
const taskCap = 4000    // ROADMAP body 最多 4000 runes
```

但没有 `promptVersion` 或 `promptStrategy` 这样的概念来改变装配策略。全仓只有一个 `buildPrompt` 函数、一种装配策略。

**证据 B：无模型感知的 prompt 结构适配**

`PhaseTier`（`executor.go:50-66`）在运行时知道模型 tier（Opus/Sonnet/Haiku），但 `buildPromptWithEmits` 只接收字符串 tier 用于日志注入，**从不根据 tier 调整 prompt 结构**：

```go
// prompt_context.go:350-360
func buildPromptWithEmits(repoRoot string, p asset.Phase, mode string,
    tierOf func(p asset.Phase) string, ...) string {
    tier := tierOf(p) // ← 只用于日志，不改变结构
    // ...
}
```

Opus（200K context）和 Haiku（50K context）收到完全相同的 prompt 结构，只是总容量不同。

**证据 C：Prompts 无回归测试**

所有 10 个 `.ai/prompts/*.md` 文件（`ls .ai/prompts/00-*.md 09-*.md`）是 git-tracked 的文本模板，但没有任何测试验证「Agent 卡描述的 prompt 结构」与「实际运行时注入的 prompt 结构」之间的差异。`doctor.EvaluateWorkflowModels` 校验模板存在，但不校验 prompt 结构完整性。

### 核心方向

引入 **Prompt Ablation Testing 框架**，使得 prompt 结构成为可实验、可度量的变量：

```
当前状态                         目标状态
───────────                      ───────────
buildPrompt 硬编码                buildPrompt 由 PromptStrategy 驱动
无 prompt 版本化                  prompt 带 version hash，可追溯
无回归测试                        prompt fixture 快照，结构变更时告警
所有模型同一结构                   按 model tier 适配结构
```

具体子方向：

1. **PromptStrategy 接口**：`type PromptStrategy func(p Phase, tier string) PromptPlan` —— 将「什么内容、以什么顺序、多少容量」封装为策略。默认策略 == 当前硬编码行为（向后兼容）。
2. **策略映射表**：按 `(model_tier, phase_type, mode)` 选择策略。例如 `Opus+architect → full_adr_first`、`Haiku+implementer → task_only`。
3. **Prompt 快照与回归检测**：每次装配后在 trace 中写入 prompt 结构快照（lane 数量、顺序、估计 token 数、源文件列表），跨版本自动比较结构差异。
4. **Ablation 模式**：`forge run --prompt-ablation` 运行同一 phase 的多个 prompt 变体，比较 agent 输出质量（gate PASS rate、迭代次数、成本）—— 系统化发现最优结构。

### 边界场景

| 场景 | 行为 |
|------|------|
| 无策略配置（默认） | 使用 hardcoded 兼容策略，byte-for-byte 不变 |
| 策略中引用了不存在的 lane | 降级 + 告警（fail-open），退回默认策略 |
| 不同模型 tier 最佳结构不同 | 策略表可配置，非硬编码 |
| Ablation 模式增加成本 | 必须是显式 opt-in（`--prompt-ablation`），默认不启用 |
| Prompt 快照文件损坏 | 跳过（同 trace 的 fail-open 模式） |

### 为什么高价值

当前 ForgeOS 每年在 LLM API 上的运行成本几乎全部花在 prompt 上，但 prompt 的结构——可能是影响 agent 输出质量的最关键变量——从未被系统化地研究过。没有 prompt 版本化，就无法知道「升级 agent 卡后 agent 表现变差是因为 prompt 还是因为模型变化」。没有 ablation，就无法知道「memory 注入到底帮了 implementer 还是让它分心」。这是**系统成本结构中最大的未测量变量**。

---

## 方向二 · 跨 Workflow 管线执行引擎

**类型**: 架构 · 自动化  
**优先级**: P1 | 预估: ~2 sprint

### 差异化证明

| 关键词 | 已有覆盖情况 |
|--------|------------|
| workflow pipeline / workflow chaining | `high-value-extensions.md` 一笔提及「discover→design→build 目前手动」但未展开；`expansion-directions-v14.md` 讨论事件触发（webhook gateway）但关注**外部触发**而非**内部 workflow 管线编排** |
| pipeline execution engine | 零独立方向 |
| inter-workflow state machine | 零 |
| OnApproved.NextStage executor | 零 |

### 现状：代码级证据

**证据 A：`asset.Workflow` 的 `OnApproved` 有 `NextStage` 字段但零消费**

```go
// asset.go:227-234
type OnApproved struct {
    NextStage string   `json:"next_stage"`  // e.g. "review", "build"
    Emit      []string `json:"emit,omitempty"`
}
```

这个字段被 YAML 解码（`design.yml:62-69` 声明了 `on_approved.next_stage: build`），但全仓搜索 `NextStage` 的**消费者**：

```bash
$ grep -rn "\.NextStage\|NextStage\|next_stage" forge-core/ --include="*.go" | grep -v "_test\|\.json"
# → asset.go:228 (struct 定义)
# → main.go:119 (reportHumanGate 的 fmt.Sprintf)
# → 零执行逻辑
```

`reportHumanGate` 只是 _print_ 了 next_stage 字段（`main.go:119 "next stage: %s"`），没有任何代码来 **执行** 这个跳转。用户看到「next stage: build」后需要手动敲 `forge run build`。

**证据 B：workflow 间无状态机或管线**

5 个 workflow 的声明中明确描述了它们应该串成一条管线：

```
discover.yml → design.yml → review.yml → build.yml → evolve.yml
   next_stage:    next_stage:    on_approved:
   design         build          next_stage: (none, terminal)
```

但不存在任何 `pipeline.yml` 或 `PipelineExecutor` 来声明和执行业务线。`forge` CLI 的 `cmdRun`、`cmdEvolve` 各自独立，互不知晓。

**证据 C：workflow 间的数据传递靠文件系统，无契约**

design.yml 的 solution-architect phase 产出 `docs/design/proposal.md` 和 `cost-estimate.md`，但 build.yml 的 planner phase **无法知道这些文件的 schema 或预期内容**。跨 stage 的数据契约完全靠 agent 卡的文字描述，没有结构化验证。

### 核心方向

引入 **Pipeline 声明 + Pipeline Executor**，使 ForgeOS 能在无人值守下自动通过完整脊柱：

```
forgerun pipeline                        # 自动跑 discover → (wait human) → design → (wait human approval) → build → evolve
forge pipeline status                    # 查看当前 pipeline 执行状态
forge pipeline pause / resume / abort    # 生命周期管理
```

具体设计：

1. **Pipeline 声明模型**：新 `pipeline.yml` 或扩展 `project.yml`，声明 workflow 序列及阶段间传递的数据契约：
   ```yaml
   pipeline:
     stages:
       - workflow: discover
         on_complete:
           confidence >= 80 → design
           confidence < 80 → pause (human review needed)
       - workflow: design
         on_approve: build
       - workflow: review
         on_approve: build
       - workflow: build
         on_complete: evolve
       - workflow: evolve
         stop: converge
   ```

2. **Pipeline Executor**：新执行器（类似 `LoopEngine` 但跨 workflow），管理各 workflow 实例的生命周期：
   - 按序启动 workflow
   - 等待 human_gate 批准（持久化等待，非内存 polling）
   - 传递 `Signal` 数据（前序 workflow 的收敛信号作为后继的初始上下文）
   - 处理故障恢复（拒绝后重试、超时后暂停、checkpoint 续跑）

3. **跨 Stage 数据契约验证**：design.yml 的 artifact 产出 schema 由 pipeline 声明，build.yml 的 planner 可验证前序数据完整性。

### 边界场景

| 场景 | 行为 |
|------|------|
| Human approval 等待数天 | Pipeline 持久化 checkpoint，跨进程恢复 |
| Discover confidence < 80 | 暂停 pipeline，通知人类决策 |
| Build 阶段 REJECTED | 根据 on_rejected 策略（回退 design / 通知人 / 终止） |
| Evolve 循环永不收敛 | Pipeline 级最大墙钟预算（非单 workflow 预算） |
| 并行 pipeline | 未来扩展（如同时跑多个独立 feature pipeline） |

### 与已有覆盖的明确边界

- **与 `expansion-directions-v14` 的事件驱动不同**：v14 讨论的是外部事件触发（webhook / PR merge → 触发 build）。本文讨论的是 ForgeOS **内部** workflow 间的自动化编排。
- **与 `expansion-horizon-three` 的管线组合不同**：horizon-three 讨论多仓库、多项目协同的顶层编排。本文聚焦于**单个项目的 5-stage 脊柱自动化**。
- **与 `high-value-extensions.md` 的瀑布编排不同**：后者仅作为痛点一笔提及，未提出任何执行模型或有状态管线机制。

### 为什么高价值

「从 idea 到 production 的全自动管线」是 ForgeOS 的终极承诺（PROJECT.md G1-G5）。但目前这个承诺在最后一个环节断掉了——5 个 workflow 各自封闭，用户需要手动衔接。消除这个 gap 是让 ForgeOS 从「强大的自动化编排器」升级为「真正的自治软件工厂」的决定性一步。

---

## 方向三 · Agent 输出语义正确性闸门

**类型**: 治理 · 信任  
**优先级**: P1 | 预估: ~2 sprint

### 差异化证明

| 关键词 | 已有覆盖情况 |
|--------|------------|
| semantic correctness gate / behavioral verification | `novel-extensions-v12.md` 方向二「跨阶段语义一致性」关注**跨 agent 一致性**（planner 说要做 A，implementer 做了 A），非**输出本身是否正确** |
| output correctness / wrong but passes tests | `genuinely-novel-expansion-directions.md` 讨论 test quality meta-gate 但关注**测试本身质量**（testing.md 的覆盖率），非**输出行为验证** |
| hallucination detection / semantic hallucination guard | 零独立方向（主要讨论 prompt injection 防御，非 agent 输出真实性） |

### 现状：代码级证据

ForgeOS 现有 gate 全部验证**机械属性**：

```go
// harness/arch/arch-check.mjs — 8 个检查
//   1. layering（包依赖方向）
//   2. package（包命名约定）
//   3. fanin（扇入上限）
//   4. cognitive（认知复杂度）
//   5. anti-pattern（反模式命名）
//   6. function-length（函数 ≤ 50 行）
//   7. circular-dep（循环依赖 = 0）
//   8. drift-guard（架构漂移）

// harness/secret-scan.mjs — 1 个检查
//   硬编码 secret

// harness/sca.mjs — 1 个检查
//   CVE 漏洞（依赖 OSV DB）

// harness/acceptance-quality.mjs — 2 检查
//   lint、coverage

// harness/acceptance.mjs — 4 检查
//   gate、check、test、app-test
```

**没有 gate 能回答「agent 写的代码做了它被要求做的事吗」**。

**证据 A：Converge 信号中无语义正确性度量**

```go
// converge.go:96-147 Signals struct
type Signals struct {
    RoadmapCompletion float64  // ROADMAP 勾选比例
    GatesGreen        bool     // 闸门全绿
    RequirementConfidence float64  // 需求置信度（agent 自报）
    ReviewStatus      string   // 评审裁决
    FileDelta         float64  // 文件变化与 ROADMAP 匹配度
    HumanApproved     bool     // 人类批准
    Criteria          map[string]string  // 逐 criterion 结果
    CodeTestRatio     float64  // 测试代码比例
    // ↑ 全部是结构性/过程性度量，无语义正确性度量
}
```

没有任何 `SemanticCorrectness` 或 `BehavioralCorrectness` 信号。

**证据 B：`prompt_context.go` 的 gateLedger 只注入 gate 裁决文字**

gate 裁决被注入 agent prompt 供参考（`appendFeedbackLanes` 的 `gates` lane），但注入的是**文本**，不是**结构化错误分类**。agent 需要从 gate 输出文本中自行理解问题所在。没有「输出行为 → 失败模式」的映射。

### 核心方向

引入 **语义正确性门（Semantic Correctness Gate）**，从多个维度检测 agent 输出的行为正确性，独立于机械 gate：

```
语义 Gate 套件：
├── 契约回溯门（Contract Trace Gate）
│   └─ planner 说「实现 X」，implementer 的代码 diff 中是否包含 X 的关键元素
│   └─ reviewer 要求「修复 A」，diff 中是否出现对 A 的修改
│
├── 测试行为门（Test Behavior Gate）  
│   └─ 对新增/修改的生产代码，是否存在对应的新 test case？
│   └─ 测试是否实际断言了预期的行为（非仅 happy path）
│
├── 代码与注释一致性门（Code-Comment Alignment Gate）
│   └─ 函数注释描述的行为与实际代码是否一致？
│   └─ 检测"注释说做了 A 但代码做了 B"
│
├── 输入输出门（IO Contract Gate）
│   └─ 函数的参数/返回值类型是否匹配接口声明？
│   └─ 错误处理路径是否被覆盖？
│
└── 回归预防门（Regression Prevention Gate）  
    └─ 现有测试在新修改上是否仍 PASS（已有）？
    └─ 新增修改是否影响已有功能的输入输出契约？
```

这些 gate 的核心机制是**启发式代码分析**（非 LLM 再次推理），通过 AST 模式匹配、diff 语义分析、测试断言密度测量等方式实现——保持 forge-core 零外部依赖、确定性执行。

### 边界场景

| 场景 | 行为 |
|------|------|
| 启发式误报（false positive） | gate 标记为 WARN 而非 FAIL（参考 enforce: warn 模式），不阻断流水线 |
| 文件类型不支持语义分析 | 诚实跳过（N/A 模式），不伪造通过 |
| 契约追溯需要跨文件 | diff 分析基于 git 工作树，不依赖跨文件关系图（v2 scope） |
| 非结构化输出（如设计文档） | 跳过低级语义检查，可选 LLM 基础的模式匹配 |

### 与已有覆盖的明确边界

- **与 `novel-extensions-v12.md`（跨阶段语义一致性）不同**：v12 关注**多个 agent 产出之间的一致性**（planner 说做 A 但 implementer 做了 B）。本文关注**每个 agent 产出本身是否正确**（implementer 说做 A、代码也做了 A，但做错了 A）。
- **与 `structural-gaps-v41.md`（产物质量治理）不同**：v41 关注产物**格式/schema 合法性**（proposal.md 是否符合 YAML frontmatter 规范）。本文关注产物**行为正确性**（代码是否实现了需求）。
- **与 `genuinely-novel-expansion-directions.md`（测试质量门）不同**：后者关注**测试用例质量**（断言密度、边界覆盖），本文关注**生产代码行为**（代码是否做了预期的事）。

### 为什么高价值

现有机械 gate 可以保证代码"看起来工整"，但无法保证「做了正确的事」。在无人值守的 24h 演进场景下，「通过了所有 gate 但行为错误」是最危险也是最无声的失败模式——它会静默地引入功能性缺陷，直到生产环境触发。语义闸门是防止静默缺陷的最后一道防线。

---

## 方向四 · 基于历史遥测的预测性运行估算

**类型**: 经济 · 可观测性  
**优先级**: P1 | 预估: ~1 sprint

### 差异化证明

| 关键词 | 已有覆盖情况 |
|--------|------------|
| predictive cost estimation / pre-run budget prediction | `novel-architectural-extensions-v40.md` 方向四 `forge plan` 关注的是**执行计划生成**（列出将跑哪些 phase、gate），非 **cost/time 预测** |
| run estimation from historical data | `expansion-direction-analysis.md` 提及 `PredictiveTier`，但聚焦于**运行时 budget 降级策略**（已经花了多少、预估还剩多少），非 **运行前的总成本估算** |
| cost forecasting / duration forecasting | 零独立方向 |
| scorecard-based prediction | 零 |

### 现状：代码级证据

**证据 A：Scorecard 数据被收集但从未用于预测**

```go
// routing/scorecard.go:28-48
type Scorecard struct {
    Model         string  `json:"model"`
    TaskType      string  `json:"task_type"`
    QualityScore  float64 `json:"quality_score"`
    Samples       int     `json:"samples"`
    UpdatedAt     string  `json:"updated_at"`
    Mode          string  `json:"mode,omitempty"`
    PassRate      float64 `json:"pass_rate,omitempty"`      // ← 历史通过率
    AvgIterations float64 `json:"avg_iterations,omitempty"` // ← 历史平均迭代次数
    ReworkRate    float64 `json:"rework_rate,omitempty"`    // ← 历史返工率
}
```

这些字段（`PassRate`, `AvgIterations`, `ReworkRate`, `CostUsdMicros` in trace）**精确地描述了**历史上各类型任务的花费和耗时。但它们**唯一的消费者**是 `HistoryTiebreak`（`scorecard.go:80-95`），用来在**同 tier 候选模型中择优**——不用于预测当前 run 的花费。

**证据 B：trace 数据的 cost/duration 信息只用于事后统计**

```go
// trace.go:32-48
type Event struct {
    // ...
    DurationMs    int64  `json:"duration_ms"`              // ← 每 phase 耗时
    CostUsdMicros int64  `json:"cost_usd_micros,omitempty"` // ← 每 phase 花费
    Model         string `json:"model,omitempty"`           // ← 使用的模型
    // ...
}
```

全仓搜索 `DurationMs` 和 `CostUsdMicros` 的读取路径：

```bash
$ grep -rn "DurationMs\|CostUsdMicros" forge-core/cmd/forge/*.go
# → scorecard_wind.go: 读取并写入 scorecard
# → 无预测消费者
```

Trace 数据只流向 scorecard（事后聚合），不流向任何预测模型。

**证据 C：无 pre-flight 分析**

`cmd/forge/preflight.go` 存在但只做**环境检查**（检查 Python/Node 等依赖是否存在），不做**运行预测**（不估算花费/时长/文件变化范围）。

```go
// preflight.go: 检查依赖工具是否存在
// 不回答问题：
//   - "这个 build 预计花多少钱？"
//   - "预计跑多久？"
//   - "预计改多少文件？"
```

### 核心方向

引入 **Predictive Run Estimator**，在 `forge run` / `forge evolve` 实际执行前回答经济问题：

```
forge run build --dry-run --estimate
→ Estimated cost: $0.35-0.85 (based on 12 historical runs)
→ Estimated wall-clock: 3-8 min (phase count: 5, avg iterations: 2.1)
→ Estimated file changes: 3-7 files (task type: feature, complexity: medium)
→ Budget check: PASS ($0.85 < $3.00 --run-budget-usd)
→ Confidence: MEDIUM (6 of 12 historical runs had similar scope)
```

估算引擎的工作流：

1. **提取运行特征**：workflow type（build/evolve/discover）、phase 数量、gate 数量、mode/lifecycle、ROADMAP 剩余项数
2. **匹配历史数据**：从 scorecards.json + trace.jsonl 中找相似特征的历史运行
3. **计算置信区间**：基于匹配样本量，输出 P50/P90 cost/duration
4. **Budget 交叉验证**：将估算结果与 `--run-budget-usd` 比较，超预算则在起跑前告警

### 边界场景

| 场景 | 行为 |
|------|------|
| 零历史数据（新项目首次运行） | 诚实输出「无可用历史数据」，使用 tier 默认价格表估算（≈ 上界） |
| 历史数据模式与当前显著不同 | 按 recency 衰减旧数据权重（复用已有的 `decayWeight`），或输出低置信度 |
| 估算大幅偏离实际 | 不阻断（估算永远只是估算）；实际花费记入下一次估算的训练数据 |
| 成本预算已耗尽 | 在跑前即阻断（比跑一半被 budget guard 终止更友好） |

### 与已有覆盖的明确边界

- **与 `forge plan` / `novel-architectural-extensions-v40` 方向四不同**：`forge plan` 生成**执行计划**（哪些 phase、以什么顺序、gate 配置）。本文关注**经济和时间估算**（要花多少钱、跑多久）。
- **与 `BudgetAdjustTier` / `expansion-direction-analysis.md` 不同**：后者是运行时的自适应降级策略（已花费 > 预算比例 → 降模型 tier）。本文是运行前的预测。
- **与 `internal/migrate` 的 plan 模式不同**：migrate 的 plan 列出将修改的文件列表。本文的 estimate 量化其经济影响。

### 为什么高价值

**成本不确定性是 ForgeOS 采用的最大心理障碍**。用户不敢跑 `forge evolve` 24h，不是因为系统不强大，而是因为不知道它会花多少钱。一个准确的预测引擎把「盲盒」变成「可管理的风险」。结合 `--run-budget-usd` 的硬上限和预测性告警，用户可以获得：「预计花费 $1-3，上限 $5，不会超。」

---

## 方向五 · 多 Agent 分歧裁决协议

**类型**: 韧性 · 治理  
**优先级**: P2 | 预估: ~2 sprint

### 差异化证明

| 关键词 | 已有覆盖情况 |
|--------|------------|
| multi-agent arbitration / disagreement resolution | `expansion-direction-analysis.md` 方向一「多 Agent 仲裁」讨论 false-positive cyclone 和无限辩论问题，但聚焦于**检测循环**（同一 reviewer 连续 N 次 REQUEST_CHANGES 后降级），非**结构性裁决协议** |
| agent escalation / third-opinion gate | 零独立方向 |
| structured disagreement resolution | 零 |

### 现状：代码级证据

**证据 A：Disagreement 只有一个出口——loop-back**

当 reviewer 输出 `REQUEST_CHANGES` 时，唯一的反应是 loop-back 到 implementer：

```go
// orchestrator.go:340-360
func (e *Engine) agentOutcome(phase string, verdictResult string) error {
    if verdictResult == "request_changes" { // 即 REVIEW_CHANGES
        return e.loopBackTo("implementer")
    }
    // ...只有这一个出口
}
```

**不存在**以下路径：
- implementer 回应「这改动不合理，因为 X」
- 第三方 agent 介入裁决
- 升级给人类

**证据 B：没有 disagreement 的分类**

全仓搜索 disagreement/conflict/arbitration/mediation/dispute 关键词：

```bash
$ grep -rni "disagree\|arbitrat\|mediat\|dispute\|conflict\|third.opinion\|escalat" forge-core/ --include="*.go"
# → 零
```

**证据 C：loop-back budget 耗尽后只 abort**

当 `MaxLoopBack = 3` 耗尽时：

```go
// orchestrator.go:350-355
if e.loopCount >= e.MaxLoopBack {
    return fmt.Errorf("loop-back budget exhausted: ...")
    // → abort，无 fallback
}
```

耗尽后没有降级路径（如自动降 model tier 重试、换 reviewer、或者把争议点注入下一轮由新的 agent 对处理）。

### 核心方向

引入**分层分歧裁决协议（Tiered Disagreement Resolution Protocol）**，为 agent 间的分歧提供结构化出口：

```
Level 1: Agent 间协商（Agent Negotiation）
├── Reviewer 输出 REQUEST_CHANGES + 具体发现
├── Implementer 收到后可以：
│   ├── 接受并修改（当前 loop-back 行为，不变）
│   └── 输出 JUSTIFICATION（「因为 X 架构决策，我选择不应用 Y 修改」）
│       └── 注入 reviewer prompt 作为第二回合
│
Level 2: 第三方裁决（Third-Party Arbitration）
├── 如果 Level 1 再次发生分歧
├── 引入第三方 agent（architect 对架构分歧、security 对安全问题）
├── 第三方 agent 阅读双方论点并输出最终裁决
│
Level 3: 人类升级（Human Escalation）
├── 如果 Level 2 未能解决（或判定为需要人类决策）
├── 注入分歧点、双方论点、第三方裁决到 human gate
├── 暂停 pipeline，等待人类决策
└── 人类决策后继续（accept / reject / modify）
```

实现增量：

1. **`JUSTIFICATION` 契约**（类似 `VERDICT` / `CONFIDENCE` 机读契约）：agent 输出末行 `JUSTIFICATION: <reason>` 注明对不同意见的反对理由。
2. **`ArbitrationPhase` 声明**：workflow 中可声明 `on_disagreement: { escalate_to: "architect", max_rounds: 2 }`。
3. **分歧计数器**：跟踪每个 reviewer-implementer 对的连续分歧次数，触发自动升级。
4. **分歧记录**：写入 trace，建分歧知识库（跨迭代学习——相同分歧不重复发生）。

### 边界场景

| 场景 | 行为 |
|------|------|
| 双方都合理（架构权衡分歧） | 第三方裁决或直接升级给人类 |
| Reviewer false positive 循环 | 分歧计数器触发升级到 architect，快速打回 false positive |
| Implementer 固执不改 | 分歧升级，第三方确认 implementer 是对的 → override reviewer |
| 同一分歧跨迭代重复出现 | 分歧知识库阻止重复仲裁，直接应用上次裁决 |
| 人类不回应 | pipeline checkpoint 等待（同 human_gate 的 durable_wait 模式） |

### 与已有覆盖的明确边界

- **与 `expansion-direction-analysis.md` 方向一（多 Agent 仲裁）不同**：后者主要检测 false-positive cyclone（同一 reviewer 连续 reject）。本文提出的**分层升级协议**更广——包含了 agent 间的初次协商、第三方介入、人类升级等完整链路，并且有明确的机读契约（`JUSTIFICATION`）支撑。
- **与 `expansion-deep-analysis.out.md` 的 tool-call schema 不同**：后者关注 multi-vendor 的 tool-use 抽象。本文关注 agent 间分歧的结构化协商。
- **与当前 loop-back 机制不同**：loop-back 是单向的（reviewer → implementer）。分歧裁决是双向的（implementer 也可以坚持自己的做法）。

### 为什么高价值

在真点火多 agent 协作中（Sprint 24-26 已验证真实存在），agent 间的分歧是**必然发生的**——不同 agent 对同一问题有不同的权衡判断。当前系统用 loop-back + budget exhaust 处理分歧，本质上是「假装分歧不存在」。引入结构化的分歧裁决协议，是将 agent 协作从**单向命令链**升级为**真正的多智能体协商系统**的关键步骤。

---

## 总结

| 方向 | 类型 | 优先级 | 预估 | 核心主张 |
|------|------|--------|------|---------|
| ① Prompt 结构敏感性 & Ablation 测试框架 | 质量 | P2 | 1.5 sprint | prompt 结构的优化是最大的未测量杠杆 |
| ② 跨 Workflow 管线执行引擎 | 架构 | P1 | 2 sprint | 5 个 workflow 的手动衔接是自治工厂的最后缺口 |
| ③ Agent 输出语义正确性闸门 | 治理 | P1 | 2 sprint | 机械 gate 无法防止「全绿但做错了」 |
| ④ 基于历史遥测的预测性运行估算 | 经济 | P1 | 1 sprint | 成本不确定性是采用的最大心理障碍 |
| ⑤ 多 Agent 分歧裁决协议 | 韧性 | P2 | 2 sprint | 智能体分歧需要结构化协商，而非单向 loop-back |

五个方向的共同特征：它们都触及了 ForgeOS 当前架构中**已经存在但尚未系统化的维度**——prompt 结构、workflow 编排、输出验证、经济预测、agent 协商。它们不依赖 Firecracker / LiteLLM / 外部数据库，在 v2 纯 Go 标准库约束下可实现。
