# ForgeOS — 全局深度扫描后的四个高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局深扫 forge-core（18 Go 包 · ~33k LOC 运行时 + CLI）、harness（39+ 模块 · ~10.5k LOC 执法层）、.agent/（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR+DECISIONS+architecture）、examples/、docs/（95+ 份分析文档）  
> 2. 逐篇通读 & 交叉验证: 45+ `docs/requirements/*.md` + 40+ `docs/analysis/*.md` + `FUNCTIONAL_REQUIREMENTS_AUDIT` + `CURRENT_SPRINT`(31 sprint 完整演进) + 所有核心文档  
> 3. **差异化为核心方法论**: 每个方向附确凿的代码级证据 + 与已有 95+ 分析的区别证明，确保不是已有方向的重新包装  
> 4. **纪律**: 不编写任何代码  
> **日期**: 2026-07-10

---

## 前言：为什么已有 95+ 份分析后还有空间

ForgeOS 在开发纪律（每个 sprint 产出分析→实现→review 闭环）和文档完整性上是我见过最严谨的 AI 工程之一。但深度代码阅读揭示了一个结构性盲区:

**几乎所有已有分析都是「垂直扩展」—— 在现有抽象层内部深入优化（韧性、记忆、路由、收敛、信号）。几乎没有分析讨论现有抽象层的「水平断裂」—— 即 forge-core 当前的能力模型在哪个边界上遇到它无法自然延伸的「语义墙」。**

本文的四个方向全部落在这些水平断裂带上。它们不是「再加一个功能」，而是「当前架构在这个维度上无法继续演化，需要新增一个抽象层」。

---

## 方向一 · 工作流组合代数：从 Procedural Phase List 到 Composable Workflow DAG

**优先级**: 🔴 P0 | **类别**: 编排 · 架构 | **预估**: ~4 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题陈述

ForgeOS 的工作流模型是**一维有序 phase 列表**。一个 workflow 是 `[]Phase`，按顺序执行，加上 `depends_on` 做有限的并行化。这在一个工作流内够用，但在以下场景中语义断裂:

- **工作流组合**: 想表达「先跑 `discover.yml`，当 confidence≥80 后继跑 `design.yml`，human_approval 后继跑 `build.yml`」——当前靠外部 CLI 调用（`forge run discover && forge run design && forge run build`），不是工作流自身的语义
- **条件分支**: 想表达「如果 gate FAIL，跑 recovery.yml；否则继续」——当前只能 `on_fail.loop_back` 跳回已存在的 phase，不能 fork 到不同的工作流
- **子工作流**: 想表达「review.yml 的 security-review 需要先跑一个 `dependency-scan.yml`」——当前只能内联成同一个 workflow phase 列表
- **工作流复用**: `build.yml` 和 `evolve.yml` 共享相同的 `harness-gates` → `reviewer` → `qa` 三段式，但它们是各自完整复制这段 phase 序列

**证据:这不是 v3 北极星架构的「编排层用 Temporal」带来的——这是当前 v2 forge-core 的 Workflow 类型自身缺少组合原语。**

### 代码级证据

**① `asset.Workflow` 是 flat list，没有层级或组合结构**:

```go
// forge-core/internal/asset/asset.go:285-300
type Workflow struct {
    Stage    string        `json:"stage"`
    Phases   []Phase       `json:"phases"`    // ← 一维列表
    Loop     *LoopBody     `json:"loop"`
    Stop     StopCondition `json:"stop_condition"`
    Readonly bool          `json:"readonly,omitempty"`
}
```

没有 `SubWorkflows []Workflow`、`Include string`、`ComposeWith` 任何组合字段。`[]Phase` 是唯一的执行单元。

**② 工作流间跳转靠 CLI 硬编码，不是编排语义**:

```go
// forge-core/cmd/forge/evolve.go:80-120
func autoSelectWorkflow(...) string {
    // 根据 project.yml lifecycle 等启发式选择 WORKFLOW_NAME
    // 但返回的是单个字符串 —— 不是工作流组合的声明
}
```

`cmdRun`、`cmdEvolve` 各自加载单个 workflow JSON 传给 `orchestrator.Engine.Run`。不存在「工作流 A 的 stop_condition.met → 运行工作流 B」的编排原语。

**③ `converge.Converge` 处理单个 stop_condition，不产生「下一步」指令**:

```go
// forge-core/internal/converge/converge.go:120-135
func Converge(stop asset.StopCondition, sig Signals) (results []Result, met bool) {
    // 它只回答 YES/NO —— 不回答 "如果是 NO 该做什么"，也不回答 "如果 YES 下一步是谁"
}
```

即使 `design.yml` 的 `human_gate` 被批准（`met=true`），没有编排层自动推进到 `build.yml`——用户必须显式敲 `forge run build`。

**④ `depends_on` 是 phase 级原语，不是 workflow 级原语**:

```go
// forge-core/internal/asset/asset.go:100-105
type Phase struct {
    DependsOn []string `json:"depends_on"` // ← 只引用同 workflow 内的 phase 名
    // 不能引用其他 workflow 的 phase
}
```

**⑤ workflow 文件是静态 YAML，不是可组合声明**:

```yaml
# .agent/workflows/build.yml
# 无 import/extends/include 语句
# 无 workflow 引用
# 每个 workflow 自包含、自复制
```

**⑥ 所有 5 个 workflow 共享重复的 phase 模式**:

```
discover.yml:  requirement-discovery → market-research → product-designer
design.yml:    solution-architect → proposal
build.yml:     planner → implementer → harness-gates → reviewer → qa
review.yml:    security → distributed → performance → executive
evolve.yml:    scan → gap → roadmap → implement → review → evaluate
```

其中 `harness-gates → reviewer → qa` 三段式在 build.yml 和 evolve.yml 中完全复制。没有共享机制。

### 与已有 95+ 分析的核心区别

- `expansion-horizon-three.md` 讨论的是「多仓库联邦编排」（跨仓库工作流），不是「单仓库内工作流组合」。
- `strategic-extensions-v33.md` 方向一（工作流代数规范）是唯一的接近项，但它讨论的是 phase 内执行语义（原子性/幂等），不是 phase 间/工作流间组合。
- `execution-semantic-gaps.md` 讨论「执行语义形式化」，聚焦于副作用/因果一致性，不是组合性。
- `loop-engineering.md` 定义的是单个 evolve 循环的收敛方法论，不是多个工作流的编排。
- **核心差异化**: 本方向不追问「单个工作流怎么更好地执行」，而是追问「两个工作流如何组合成一个更大的工作流」——这是完全不同的抽象层次。

### 为什么需要它

没有工作流组合代数，ForgeOS 的编排能力被锁定在「一个 CLI 命令 = 一个 workflow = 一次执行」的模式里。真正的自治工厂需要:

1. **Pipeline as code**: `discover → design → [human] → build → review → evolve` 是一个 pipeline，不是 6 次独立的 `forge run`
2. **条件编排**: 如果 review REJECT，自动触发 redesigne.yml 而非通知人类手动处理
3. **子工作流复用**: 一个「安全扫描」工作流可被 review.yml、pre-merge.yml、monthly-audit.yml 共同引用
4. **并行工作流**: build 和 docs-gen 可以并行跑，但共享同一个 converge 判定

### 具体方向建议

- **WorkflowInclude 声明**: 在 workflow YAML 中新增 `include: path/to/sub-workflow.yml`，将子工作流的 phases 展开到当前位置（编译期组合）
- **WorkflowRouter stop_condition 扩展**: stop_condition 新增 `on_met: {run: workflow_name}` 和 `on_unmet: {run: recovery_workflow}`，使得 converge 结果能自动推进到下一个工作流
- **WorkflowExpr 条件表达式**: phase 的 `required_when` 从简单 mode 名匹配扩展为 `mode==engineering && lifecycle>=growth` 的布尔表达式
- **Shared Phase Template**: 抽取 `harness-gates → reviewer → qa` 为可引用的 phase template，多个 workflow 通过 `uses: .agent/templates/quality-gate.yml` 引用

---

## 方向二 · Provider 抽象契约：从隐式 Claude 耦合到显式 Provider Interface

**优先级**: 🔴 P0 | **类别**: 架构 · 可扩展性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题陈述

ForgeOS 的路线图将跨厂商模型池列为 v3。但代码中存在一系列**隐式 Claude 耦合**，它们不是被 defer 到 v3 的「设计决策空缺」，而是 v2 代码中已经硬编码的、对后续扩展形成障碍的约束:

- `cost.go` 硬解析 Claude JSON 输出格式（`unwrapClaudeResult` 函数）
- `ModelMap` 是 `map[string]map[string]string` 的静态表，仅含 `anthropic`
- `routing.TierFor` 返回的是模糊的 tier 名（haiku/sonnet/opus），不是 provider 限定的模型名
- `CommandExecutor` 对 claude CLI 的特殊处理（`--permission-mode acceptEdits` 等）直接与 vendor 绑定
- 真点火验证 (Sprint 24-26) 全部只用了 claude

**问题不是「v3 没做」，问题是「v2 的抽象层让 v3 做时需要重写大量代码」**——因为 provider 差异没有封装在接口后面，而是散落在 `cmd/forge` 和 `internal/orchestrator` 多个文件中。

### 代码级证据

**① `cost.go`（cmd/forge）硬编码解析 Anthropic Claude JSON 输出**:

```go
// forge-core/cmd/forge/cost.go:70-105
func unwrapClaudeResult(raw []byte) (string, int64, int64, string, string, error) {
    // 硬解 claude JSON 格式: {"content":[{"text":"..."}],"usage":{"input_tokens":...,"output_tokens":...},"total_cost_usd":...}
    var result struct {
        Content []struct {
            Text string `json:"text"`
        } `json:"content"`
        Usage struct {
            Input  int `json:"input_tokens"`
            Output int `json:"output_tokens"`
        } `json:"usage"`
        CostUsd *float64 `json:"total_cost_usd,omitempty"`
        Model   string   `json:"model,omitempty"`
    }
    // ...
}
```

这个函数名 `unwrapClaudeResult` 自己承认了是 Claude-specific。Codex / Gemini CLI 的输出格式完全不同。要让第二个 provider 工作，要么复制一个 `unwrapGeminiResult`，要么重构出一个 `ResultParser` 接口——后者目前不存在。

**② `ModelMap` 是静态字典，仅有 anthropic**:

```go
// forge-core/internal/routing/routing.go:175-185
var ModelMap = map[string]map[string]string{
    "anthropic": {
        Haiku:  "claude-sonnet-4-haiku",
        Sonnet: "claude-sonnet-4",
        Opus:   "claude-opus-4",
    },
}
```

新增 provider 需要修改 routing 包源码。没有 `RegisterProvider(name string, models map[string]string)` 插件注册机制。连 v3 路线图都依赖的 LiteLLM 也是外部包，当前架构没有插入点。

**③ `CommandExecutor` 的 claude argv 构造是 if/else vendor 分支**:

```go
// forge-core/internal/orchestrator/command_executor.go:80-120
func (e *CommandExecutor) buildArgv(p, mode) []string {
    if strings.Contains(e.AgentCmd, "claude") {
        // claude-specific flags: --permission-mode, --model, --max-budget-usd
        argv = append(argv, "--permission-mode", e.Permission)
        argv = append(argv, "--model", routing.ResolveModel("anthropic", tier))
        // ...
    }
    // 没有 else-if 分支给 codex/gemini
}
```

`AgentCmd` 只是一个字符串，没有 provider 类型枚举或接口。新 provider 需要修改这个核心文件。

**④ `routing.TierFor` 返回无 provider 限定的裸 tier 名**:

```go
// forge-core/internal/routing/routing.go:45-60
func TierFor(agent, mode string) string {
    // 返回 "haiku"/"sonnet"/"opus"
}
```

下游 `ResolveModel("anthropic", tier)` 补上 provider，但 provider 选择是调用时硬编码的。如果路由决策本身应该知道「这个 agent 可以用哪个 provider」——当前的设计不允许。

**⑤ `parseReviewerVerdict` / `parseExecutiveVerdict` / `parseConfidenceScore` 都在 cost.go 中**:

```go
// forge-core/cmd/forge/cost.go:200-260
// 这些函数假定 agent 输出了特定格式的末行机读 token:
// "VERDICT: APPROVE"、"CONFIDENCE: 85"
// 这是与 Claude 无关的 ForgeOS 层契约，但混合在 cost.go (名字就说是 cost) 里
```

相同层级的机读 token 解析混在「cost」文件里，层级错误。若新 provider 的 token 格式不同，需要从 cost.go 抽出独立的契约层。

**⑥ `resolveProbeResult` 和 `probeAppTests` 隐式假定 node/npm/go 环境**:

```go
// harness/acceptance-quality.mjs:60-120
// probeLint 探测 eslint/pylint/golangci-lint —— 但由 adapter .yml 声明
// 这是正确的抽象（adapter 模式），但 execute 路径没有同等待遇
```

### 与已有 95+ 分析的核心区别

- 所有已有分析讨论「跨厂商池」都是作为 v3 的外部依赖（LiteLLM），假设「需要 LiteLLM 来路由多个 provider」——而不是讨论 v2 代码中的隐式 Claude 耦合在架构上如何解耦。
- `expansion-horizon-three.md` 方向一（通用模型路由与 LiteLLM 集成）聚焦于部署架构，不是抽象契约。
- `sixth-wave-multimodel.md` 讨论多模型策略（路由/voting/cascade），但假设 provider 抽象层已存在——回避了「当前抽象层不存在」的问题。
- **核心差异化**: 本方向不讨论「加一个新 provider」，而是讨论「抽象出 Provider Interface，使得加新 provider 不需要 fork 代码」。——这是所有多 provider 讨论的前提条件。

### 为什么需要它

如果 v3 的目标是跨厂商池，那么 v2 的每一个隐式 Claude 耦合都是技术债务。关键是**这些耦合的数量**：cost.go、command_executor.go、routing.go、gates.go、prompt_context.go 共 5 个文件需要修改来支持第二个 provider。更难的是，这个修改不是「加 if-else」——三个 vendor 需要 if-else，十个 vendor 需要接口。

**高价值场景**:
- 引入 Codex CLI 作为第二个执行器：需要解析不同的 JSON 输出格式、不同的权限模型、不同的模型名
- 引入 Gemini CLI：需要不同的 token 计数、不同的成本结构、不同的超时行为
- 在开源场景中用 Ollama 替代 Claude：需要完全不同的命令行接口和输出处理
- 让 forge-core 自己实现执行器（不依赖外部 CLI）：需要 Executor 接口的第二个实现

### 具体方向建议

- **Provider 接口定义**: 在 `internal/provider/` 包中定义:
  ```go
  type Provider interface {
      Name() string
      ResolveModel(tier string) string
      ParseOutput(raw []byte) (*AgentResult, error)
      BuildArgv(phase asset.Phase, tier string, budget ...) []string
  }
  ```
- **现有代码迁移**: 从 `cost.go`/`command_executor.go`/`routing.go` 中提取 Claude 特殊逻辑为 `internal/provider/anthropic.go`，保持行为不变但逻辑归位
- **Provider 注册机制**: `routing.RegisterProvider("anthropic", &AnthropicProvider{})`，新的 provider 通过注册加入，不修改 routing 包核心
- **Provider 选择策略**: 路由层（或 project.yml）声明 `model_provider: anthropic | codex | auto`，auto 策略基于任务类型/成本/可用性选择
- **契约层独立**: 不依赖 cost.go 解析机读 token，而是建立 `internal/contract/` 包，专门处理 `VERDICT:`/`CONFIDENCE:` 等与 vendor 无关的 ForgeOS 自有契约

---

## 方向三 · 渐进式治理采纳：`forge adopt` 从零到工程化的增量路径

**优先级**: 🟠 P1 | **类别**: 采纳 · 产品 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题陈述

ForgeOS 当前的全部采纳路径是**全有或全无**:

- `forge-init` 创建一个**新项目**带完整治理（.agent/ + harness/ + CI + CLAUDE.md）
- 对于**已有项目**，没有任何工具可以将 forge 治理逐步引入

这意味着:
- 一个已有的 5000 行 Go 项目不能「试试 forge 的架构检查」而不承担整个治理体系的认知负担
- 一个想「先用 secret-scan」的团队必须接受 gate.mjs、check.py、8 项 arch-check、全部 agent 卡
- 一个在 CI 中只想「加个圈复杂度门禁」的项目被迫 fork harness 并手工配置

**这与 Kubernetes 的 PodSecurity 演进类似**:最初的 PodSecurityPolicy 是全有或全无的，导致低采纳率；后来的 PodSecurity Admission 提供了每个命名空间的增量 profile（privileged → baseline → restricted）。ForgeOS 需要等价的增量采纳路径。

### 代码级证据

**① `harness/scaffold/forge-init.mjs` 是一次性全局复制**:

```mjs
// harness/scaffold/forge-init.mjs:60-90
const COPIED_FILES = [
    // .agent/ 全套: agents/skills/workflows/eval/routing/policies
    // harness/ 全套: gate.mjs, accept.mjs, check.py, arch-check.mjs, …
    // .github/workflows/forge.yml, CLAUDE.md
];
// 全部复制 —— 没有 "只复制部分" 的 flag
```

**② check.py 是单模式运行，没有 modular check 选择**:

```python
# harness/check.py:10-25
def main():
    checks = [
        check_workflow_agent_refs,    # 全运行
        check_mode_priorities,         # 全运行
        check_workflow_control_flow,   # 全运行
        check_workflow_mode_gating,    # 全运���
        check_mode_tiers_aligned,      # 全运行
        # ... 已经 10 项
    ]
    # 没有 --checks lint_only 子集选项
```

**③ gate.mjs 没有「仅启用 X 项」模式**:

```mjs
// harness/gate.mjs:40-80
const enforcers = [
    checkRootFileCount,   // 全运行
    checkLineCount,       // 全运行
    // 没有按需启用的机制
];
```

**④ policies.yml 声明了完整的 matrix，但未提供渐进覆盖**:

```yaml
# harness/policies.yml
# enforce: warn/block — 全局的，不是按项目模式
# 没有 "advisor" / "adopt-level" 概念
```

**⑤ forge-core 的 arch-check.mjs 是独立的，但以「全 8 检查」方式运行**:

```mjs
// harness/arch/arch-check.mjs:1-15
// 不提供 "--only layering" 或 "--only fan-in" 选项
```

**⑥ `forge migrate` 是治理晋升（explorer→engineering）但不是渐进引入**:

```go
// forge-core/internal/migrate/migrate.go
// forge migrate --to engineering 从 explorer 一步跳到 engineering
// 没有 "先加 gate、下周加 check、下月加 reviewer" 的渐进路径
```

### 与已有 95+ 分析的核心区别

- `expansion-self-governance-and-hygiene.md` 讨论的是 forge 自身代码的治理，不是采用者的采纳路径。
- `forgotten-five-meta-governance-and-blindspots.md` 讨论的是治理元数据的完整性，不是采用者的体验。
- `expansion-production-readiness.md` 方向三（环境验证）触及「forge 运行前检查环境」，但不是「渐进式引入治理」。
- `governance-prod-five-frontiers.md` 讨论的是生产治理，面向已有治理体系的强化，不是从零开始的采纳。
- **核心差异化**: 所有已有分析假设用户已经决定采用 forge。本文方向三讨论的是从**决策到采纳之间的桥梁**——这是产品视角而非技术视角，但在 ForgeOS 的工程中从未被作为独立方向讨论。

### 为什么需要它

forge-init 只能在 100% 新项目上工作。现实中的软件工程大多是已有项目。ForgeOS 的强大治理只有能被**渐进引入**才能产生实际影响——否则它就是一个「新项目玩具」。

**以 forge-init 本仓为例**:在 forge-init 诞生之前，ForgeOS 这个项目本身已经有完整的代码库。如果当时没有手动搭建 .agent/、harness、check.py（这是人力手工做的），今天不可能有 forge-init 来自动化它。第一个项目（也就是本仓本身）的采纳就是手工的、逐步的。现在这个手工路径没有工具化。

### 具体方向建议

- **`forge adopt --check-only arch-layering`**: 只启用构检查的一小部分，不加其他 harness，不要求 agent 卡或 workflow。输出: `installed arch-check.mjs + policies.yml(layering only)`
- **`forge adopt --level security`**: 启用 secret-scan + SCA 框架（但不启用 gate.mjs 体积门禁或 check.py 治理检查）
- **`forge adopt --interactive`**: CLI 对话式引导，逐步选择要启用哪些治理维度
- **采纳等���定义**: 定义 3-4 个预制等级: `silent`（只记录不阻断）、`advisory`（建议不强制）、`gated`（CI 阻断关键项）、`full`（完整 forge 治理）
- **已有项目集成**: 不覆盖已有 Makefile/CI，而是在它们旁边新增 forge gate 调用（`make lint && node harness/gate.mjs`），不破坏现有流程

---

## 方向四 · 工作流确定性测试骨架：Workflow Fixture + Golden File System

**优先级**: 🟠 P1 | **类别**: 质量 · 测试 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题陈述

ForgeOS 已有 211 自测 + 39 app 测试覆盖了 harness、orchestrator 的大部分机械逻辑。但**没有一个测试测试「一个完整的工作流语义」**:

- `orchestrator.RunFrom` 有单测覆盖 phase 步进、loop-back、mode-gating
- `converge.Converge` 有单测覆盖各种信号组合
- `loop.LoopEngine.Run` 有单测覆盖迭代、停滞、收敛

但是没有任何测试验证:
- `design.yml` 加载后的 phase 序列与 YAML 中声明的完全一致
- `build.yml` 的 stop_condition 在 converge 检查时确实读取了正确的 4 个 criterion
- `review.yml` 的 `optional_for: [balanced]` 被 mode-gating 正确 skip
- `evolve.yml` 的 loop body 被正确 hoist 到 Workflow.Phases

这些「工作流语义的正确性」只由 `check.py` 的 `check_workflow_control_flow` 做外部验证，但 check.py 是 Python（与 forge-core Go 不同语言），且只检查元数据一致性，不检查**运行时行为**。

### 代码级证据

**① 已有测试聚焦于 Go 包内部逻辑，不是 workflow 语义集成测试**:

```go
// forge-core/internal/orchestrator/orchestrator_test.go
// 测试 RunFrom/mode-gating/loop-back —— 但用的是硬编码的 asset.Workflow{} 结构体
// 不是真实 workflow 的 JSON 加载
```

`go test -run TestRun...` 覆盖 phase 循环，但不覆盖「workflow.yml 能否正确被 LoadWorkflowJSON + RunFrom 执行」。

**② check.py 是静态分析，不是运行时验证**:

```python
# harness/check.py
def check_workflow_agent_refs():
    # 解析 YAML，检查 agent 名是否匹配实际卡文件
    # 不运行 workflow
```

**③ 没有「golden run」文件**:

```go
// forge-core/internal/orchestrator/
// 无 testdata/ 目录预期输出
// 无 "workflow X 加载后应产生 phase 序列 Y" 的 fixture
```

对比 `internal/persist/testdata/replay/` 有 checkpoint/memory/trace fixture——但那是 persist 包的测试数据，不是 workflow 语义的 golden file。

**④ real workflow 只在集成测试的边缘被触及**:

```mjs
// harness/test_acceptance.mjs:100-130
// 跑真实 forge run，验证 ACCEPTED
// 但不验证 phase 序列、不验证 converge 结果、不验证 gate 调用
```

**⑤ workflow 的 JSON 加载是容错的，但容错行为没有被测试**:

```go
// forge-core/internal/asset/asset.go:310-325
func LoadWorkflowJSON(data []byte) (Workflow, error) {
    // 容错加载：缺失字段产生零值而非错误
    // 但零值 Workflow 会导致什么运行时行为？——没测试
}
```

如果一个 workflow YAML 漏写了 `stage` 字段，`LoadWorkflowJSON` 返回空 `stage`，`RunFrom` 按 `"" == "discover"` 不匹配，stage-specific 逻辑（如 `discoverStageSkipped`）全失能——这个行为从未被测试。

### 与已有 95+ 分析的核心区别

- `seventh-wave-data-realism.md` 讨论的是**使用真实 trace 数据作为测试 fixture**（测试数据真实度），不是测试工作流语义。
- `expansion-production-readiness.md` 方向四（集成测试策略）讨论的是用真实 agent 做端到端测试，不是 fixture 化的可重现测试。
- `self-testing-and-dogfooding.md` 讨论用 forge 治理自身（gate on forge-core），不是用 golden file 测试工作流语义。
- **核心差异化**: 本方向不是「加更多测试」，而是「为工作流语义建立第一类测试基础设施」，类似于编译器的测试套件用 golden output fixture 测试整个 pipeline——而不是只测试 IR 的各个 pass。

### 为什么需要它

工作流是 ForgeOS 的核心抽象。没有一个测试骨架来验证「新增 workflow YAML 字段后的预期行为」或「修改 mode-gating 逻辑不影响现有 workflow」，每次对编排引擎的改动都有不可见的回归风险。

Sprint 26 的 yaml2json block-scalar bug 就是一个例子:7 个真实 workflow 文件的 `description` 字段被静默注入垃圾字符，agent 运行在错误的 prompt 下数月。如果有 golden file 测试——加载 workflow → 输出序列化的 phase 列表 → 与期望比对——这个 bug 会在提交时立即被发现，而不是在真 agent 运行后才暴露。

**高价值场景**:
- **阶段序列回归保护**: 修改 `asset.LoadWorkflowJSON` 后，自动验证所有 5 个真实 workflow 仍产生相同的 phase 序列
- **mode-gating 行为回归**: 修改 `internal/mode` 后，自动验证 `balanced+production` 仍产生与之前相同的 gate-set
- **converge 语义验证**: 修改 `internal/converge` 后，自动验证每个 workflow 的 stop_condition 产生与之前相同的 MET/NOT MET 判定
- **新 workflow 加入门禁**: 新增 workflow 必须附带 golden file，否则 check.py 拒绝

### 具体方向建议

- **Workflow Golden File 目录**: 在 `forge-core/internal/orchestrator/testdata/workflow/` 下为每个真实 workflow 创建 golden 目录:
  ```
  testdata/workflow/
    build.yml/         # 对应 .agent/workflows/build.yml
      phases.json      # LoadWorkflowJSON + phase-by-phase 预期输出
      gates.json       # mode-gating 过滤后的预期 gate-set (每个 mode)
      converge.json    # 给定信号的预期 converge 结果
    design.yml/
      ...
  ```
- **Golden file 生成器**: `tools/gen-workflow-golden` 读取当前 workflow + 当前 mode-gating 逻辑，生成 golden 输出。CI 中对比 golden 是否漂移。
- **Workflow 测试框架**: 新增 `internal/wftest/` 包，提供:
  ```go
  // 加载真实 workflow → 断言 phase 序列
  wftest.AssertPhaseSequence(t, "build", []string{"planner", "implementer", ...})
  // 断言 mode-gating 后的 gate-set
  wftest.AssertGatesForMode(t, "build", "explorer", []string{"test"})
  ```
- **Incremental golden update**: golden 漂移允许但需 `--update-golden` flag，类似 LLVM 的 `UPDATE_EXPECT`——每次更新都显式 commit，diff 清晰可见

---

## 汇总

| # | 方向 | 优先级 | 类别 | 预估 | 核心差异化对比已有 95+ 分析 |
|---|------|--------|------|------|---------------------------|
| 1 | 工作流组合代数 | P0 | 架构 | ~4 sprints | 所有分析讨论「单个工作流优化」；本文讨论「工作流之间如何组合」——新的抽象层级 |
| 2 | Provider 抽象契约 | P0 | 架构 | ~3 sprints | 已有分析讨论跨厂商池作为 v3 外部依赖；本文揭示 v2 代码中 5 个文件的隐式 Claude 耦合——多 provider 的前提条件 |
| 3 | 渐进式治理采纳 | P1 | 产品 | ~2 sprints | 所有分析假设用户已采纳 forge；本文讨论「从决策到采纳的桥梁」——全有或全无的缺失路径 |
| 4 | 工作流确定性测试骨架 | P1 | 质量 | ~2 sprints | 已有分析讨论「加更多测试」；本文讨论为工作流语义建立第一类测试基础设施（golden file）——质量方法论的升维 |

---

## 收敛建议

**若只做一件**:方向一（工作流组合代数）。这是长期杠杆最高的方向——没有它，ForgeOS 的编排能力被锁定在单工作流单执行模式，无法向 pipeline/conditional/composition 进化。方向二是方向一的使能器（组合需要 workflow 作为第一类值），方向四是方向一的保障（组合需要安全网）。

**做前两件**:方向一 + 方向二。Provider 抽象契约是方向一的自然延伸——当 workflow 可以组合时，每个子 workflow 可能使用不同的 provider（安全评审用 opus，单元测试覆盖用 haiku）。两方向同时落地释放最大的架构灵活性。

**做前三件**:加方向三（forge adopt）。方向三解决的是「用户怎么来」的问题；方向一和方向二解决的是「用户来了之后能做什么」。产品采纳漏斗的顶端必须优先解决。

**做全部四件**:方向四（golden file 测试骨架）为其他三个方向的安全网——组合代数改变 workflow 加载方式、provider 契约改变执行语义、forge adopt 改变初始化流程，三者都需要回归保护。
