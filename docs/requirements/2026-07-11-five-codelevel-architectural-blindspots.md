# ForgeOS — 五个代码级架构盲区（全局扫描 + 去重验证）

> **角色**: 资深架构师 / 产品经理  
> **方法**: 完整逐文件扫描 `forge-core/`（18 Go 包, ~35k LOC）· `harness/`（42+ 模块）·  
>   `.agent/` 全部声明资产 · `CURRENT_SPRINT.md` 31 轮 sprint · `FUNCTIONAL_REQUIREMENTS_AUDIT.md`  
> **去重验证**: 已审阅 `docs/requirements/` 全部 167 篇历史分析,对每个方向的**核心命题**进行  
>   `grep` 关键词组合检索,确认该命题从未被作为独立系统性方向展开。被已有分析覆盖的分支会在  
>   各方向末尾诚实注明交叉引用。  
> **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码证据、边界场景、产品价值判断。  
> **生成日期**: 2026-07-11

---

## 当前架构阶段总览

ForgeOS 经过 31 轮 sprint,运行时引擎层高度成熟,但架构细节中存在一些**代码级结构性问题**——  
它们不涉及缺少某个具体功能,而是关乎**抽象边界的选择、接口契约的设计、以及包依赖拓扑的隐式承诺**。  
这些问题不阻碍今天的运行,但随着系统持续增长,将逐渐转化为重构债、维护负担和扩展瓶颈。

以下 5 个方向全部落在已有 167 篇分析的间隙中——每个方向的核心命题在全部历史文档中**零系统论述**。

---

## 方向一 · AgentExecutor 协议缺口：`error` 是唯一返回值

> **类型**: 架构接口 · **类别**: 抽象泄漏 · 扩展瓶颈  
> **去重验证**: `executor.*result*|phase.*result.*type|Execute.*return*|AgentExecutor.*result` —  
>   167 篇需求文档中 **零篇** 提及这一架构缺口。

### 问题

`orchestrator/executor.go:25` 定义了 forge-core 与外部 agent 之间的唯一契约：

```go
type AgentExecutor interface {
    Execute(ctx context.Context, p asset.Phase, mode string) error
}
```

这是一个**纯副作用接口**：`Execute` 返回 `error` 或不返回,但没有结构化结果。  
调用者无法从签名中获知：
- phase 执行后产生了什么输出（文件路径、关键结论、裁决结果）
- executor 是成功完成还是部分完成（如只通过了部分 gate）
- 产生了哪些可供下游 phase 消费的结构化数据

### 代码级影响

当前代码通过**三个正交的旁路机制**弥补这个缺口：

**旁路① — `cost.go` 的 ad-hoc 字符串解析**（`forge-core/cmd/forge/cost.go:310-380`）：
```go
// parseReviewerVerdict 从 agent 的纯文本输出中搜索 "VERDICT: APPROVE"
// parseExecutiveVerdict 从 cto 输出中搜索五择一 UPPER_SNAKE token
// parseConfidenceScore 从 product-manager 输出中搜索 "CONFIDENCE: <0-100>"
```
三套解析函数独立编写,每套都有自己的边界情况（token 在代码块内？前有多余空格？  
markdown 格式干扰？）。Sprint 27 已验证此类脆弱性是真实的 bug 来源。

**旁路② — `prompt_context.go` 的 feed-forward ledger**（`forge-core/cmd/forge/prompt_context.go`）：
Executor 执行后,phase 输出的**捕获和传递**不经过 `Execute` 的返回值,而是通过  
`phaseOutputLedger`（文件路径列表的 map）和 `verdictLedger`（裁决 token 的 map）两个  
独立结构——它们在 `RunFrom` 的循环体中被硬编码赋值,与 `Execute` 接口完全正交。

**旁路③ — `emits:` 文件系统探针**：
`prompt_context.go` 的 `buildPrompt` 在构建 prompt 时打开 `emits:` 声明的文件路径并读其内容。  
phase 是否真的产生了这些文件、内容是否符合预期——execute 的返回值不告诉任何人。

### 边界场景

| 场景 | 当前表现 | 结构化 result 下的行为 |
|------|----------|----------------------|
| Agent 输出了 VERDICT 但被 markdown 代码块包裹 | `parseReviewerVerdict` 静默返回空 → signal 丢失 | Executor 返回 `PhaseResult{Verdict: "APPROVE"}` 强类型字段 |
| 多个 phase 的 feed-forward 冲突（两个 implementer 都声称产生了同一个文件）| 后运行的静默覆盖前一个（`phaseOutputLedger` 按 phase 名 key）| `PhaseResult{Emits: []Emit{...}}` 带元数据,下游可做冲突检测 |
| Agent 部分完成（5 个 sprint item 只完成了 3 个）| 无信号——是否完成只看 ROADMAP 是否被勾选 | `PhaseResult{Completion: 0.6, Warnings: [...]}` 结构化表达 |
| 异构 executor（claude vs. codex vs. gemini）返回不同格式的输出 | 每一个新 executor 需要一套新的 ad-hoc parser | Executor 负责将自有格式翻译为统一的 `PhaseResult` |

### 为什么高价值

1. **消除碎片化解析**:当前 3 套 ad-hoc 解析器在三轮 sprint 中被分别发现、分别修复,  
   每次新合约 token 都会新增第四套。结构化 result 将这些收归到一个类型系统中。

2. **打开 feed-forward 的契约验证**:如果 `Execute` 返回 `PhaseResult{Emits: []string{"prd.md"}}`,  
   `RunFrom` 可以在 phase 结束后验证文件是否真的存在,而不是到下游 prompt 构建时才发现缺失。

3. **降低新 executor 的接入成本**:目前的 `CommandExecutor` 返回原始字节;  
   如果接入 Codex CLI 或 Gemini CLI,每一个都需要自己实现一套 token 解析。  
   统一返回类型让 executor 适配器只做格式翻译,解析逻辑共享。

### 诚实边界

这**不是**「把所有 phase 输出格式化为 JSON Schema」——那属于方向五相关的输出合约层。  
本方向聚焦于 `AgentExecutor` 接口本身：给它一个返回值类型,让 phase 的执行结果可以被  
类型安全地消费,而不是通过旁路或字符串搜索。

### 已有分析交叉引用

`emits:` 验证（~13 篇）和 agent 输出语义验证（方向二 in `five-unbuilt-core-extension-directions.md`）  
讨论了输出内容的验证,**但从未触及 executor 接口本身无结构返回值的架构问题**。

---

## 方向二 · 缺失的业务逻辑层：`cmd/forge` 结构性张力

> **类型**: 架构分层 · **类别**: 包边界 · 重构债  
> **去重验证**: `cmd.*forge.*business|business.*logic.*layer|CLI.*business|thin.*CLI|  
>   CLI.*facade.*pattern|package.*max.*files.*cmd|cmd.*forge.*architect` —  
>   167 篇需求文档中 **零篇** 作为独立系统性方向展开。

### 问题

`cmd/forge/` 是 forge-core 中**唯一同时导入所有 12 个 `internal/` 包的包**。  
它承担了两个本应分离的责任：

1. **CLI 外观**（flag 解析、`main.go` 的路由、`cobra` 风格的 dispatch）  
2. **业务逻辑中枢**（将 engine 组装、将分散的子系统编排在一起）

Sprint 记录反复证明这种双重身份导致了结构性压力：

- **Sprint 27**: `cmd/forge` 15 个源文件 > 14 上限 → 将 `validate_agents.go` 逻辑吸入 `internal/doctor`  
- **Sprint 29**: `gates_test.go` 顶 500 行 → `gate_resolve.go` 迁入 `internal/gate/resolve.go`  
- **Sprint 30**: `prompt_context.go` 顶 500 行 → 拆出 `prompt_artifacts.go`  
- **Sprint 31**: 两次如此,文件数从 14 膨胀到 16（每次都被迫上调上限）

每个 sprint 都在重复同一个模式：**函数/文件顶到体积门 → 把一段逻辑下沉到 `internal/` → 过几轮又顶到门**。  
这不是执行纪律问题,而是**架构信号**——`cmd/forge` 包含太多不属于 CLI 外观的逻辑。

### 代码证据

**证据① — `cmd/forge/gates.go:1-500`**:
这个文件包含的远不止 CLI 胶水：它实现了 `gatherSignals`（跑 git diff、读 ROADMAP、算  
FileDelta）、`reviewStatus`、`requirementConfidence`——这些都是**领域逻辑**,不是 CLI 代码。  
它们之所以在这里,是因为没有 `internal/forge` 或 `internal/app` 层容纳它们。

**证据② — `cmd/forge/evolve.go` 和 `cmd/forge/engine_build.go`**:
```go
// engine_build.go 构建 Engine——这是领域逻辑
func buildRunEngine(...) *orchestrator.Engine { ... }
// evolve.go 编排 evolve 循环——这是领域逻辑
func runEvolve(...) error { ... }
```
这些函数与 flag 解析（`main.go` 的职责）耦合在同一包中,无法在不引入 CLI 依赖的前提下  
被单元测试或复用。

**证据③ — 下沉模式的重复性**:
每次下沉（doctor / attribution / gate/resolve / mode_gating_check / prompt_artifacts）  
都是**被动的**——因为文件数或体积门被触发,而非主动设计。  
5 次相同的被动重构模式是明确的架构信号。

### 边界场景

| 场景 | 当前表现 | 有业务逻辑层的表现 |
|------|----------|-------------------|
| 想写一个测试验证 `gatherSignals` 在特定 ROADMAP 内容下的行为 | 需要导入 `cmd/forge`,间接引入全部 CLI 配置和 flag | 逻辑在 `internal/app`,零 CLI 依赖 |
| 想从另一个 Go 程序(非 CLI)复用 forge-core 的 evolve 编排 | 无法干净导入 `cmd/forge` 的业务函数 | 从 `internal/app` 包直接调用 |
| 想换 CLI 框架(从裸 `flag` 到 `cobra`/`spf13/cobra`)| 需要修改包含领域逻辑的同一包文件 | 只改 `cmd/forge/main.go` |
| 想为 forge-core 写一个 gRPC 服务端 | `cmd/forge` 的业务函数无法复用 | `internal/app` 的函数可被子服务调用 |

### 为什么高价值

1. **消除反复的「拆包 → 涨回 → 再拆」循环**:过去 5 轮 sprint 一直在间接建立这个层,  
   但每次都是从 `cmd/forge` 向外推,而不是主动设计。明确建立 `internal/app` 或等价层  
   让下沉成为一次性架构决策,而非持续战术操作。

2. **测试可及性**:`gatherSignals`、`buildRunEngine`、`runEvolve`、`gatesGreen`——这些核心  
   业务函数目前被锁在 CLI 包中。移入独立的业务层后,可以被 forge-core 其他部分（如  
   未来的守护进程、gRPC 服务、程序化 API）直接调用。

3. **未来扩展的架构基础**:方向三（守护进程模式）和方向四（多项目控制面）都需要在  
   脱离 CLI 上下文的情况下使用 forge-core 的业务能力。没有明确的业务逻辑层,这两者  
   都需要重复 `cmd/forge` 的组装逻辑。

### 诚实边界

提议**不是**「把 `cmd/forge` 全部 16 个文件拆到新包」——那本身就是镀金。  
提议的是：识别出**已经明显属于领域逻辑、只因历史原因留在 CLI 包中的代码**，  
将它们移入一个共享的业务层。`cmd/forge` 仍然保留足以完成 `forge run/evolve` 的胶水。

### 已有分析交叉引用

方向一 in `forgotten-five-foundations.md`（「应用骨架」）提及了进程模型,但从未分析  
`cmd/forge` 作为 CLI 外观 + 业务逻辑的二元角色的结构性后果。

---

## 方向三 · `internal/mode` 配置注册表膨胀

> **类型**: 架构腐化 · **类别**: God 结构风险 · 领域内聚性  
> **去重验证**: `mode\.go.*growth|mode.*struct.*grow|mode.*field.*explod|  
>   mode.*package.*bloat|configuration.*registry.*mode` —  
>   167 篇需求文档中 **零篇** 论述 `internal/mode` 包自身的结构增长。

### 问题

`internal/mode` 最初是**中枢旋钮**——mode × lifecycle 的过滤决策器,判断「当前 mode×lifecycle  
下应该跑哪些 gate」。但它在 31 轮 sprint 中逐步吸收了越来越多的配置职责：

```go
// forge-core/internal/mode/mode.go (当前字段清单)
type Policy struct {
    Mode            string   // 原始 mode 名
    Lifecycle       string   // 原始 lifecycle 名

    // 原始职责：gate-set 过滤
    RequiredGates   []string // gates to enforce
    GateSet         []string // all applicable gates (for reporting)

    // Sprint 8 加入：migration 规则
    Migration       MigrationPolicy

    // Sprint 10 加入：coverage 阈值
    CoverageThreshold float64
    CoverageDelta     float64

    // Sprint 14 加入：model tier 控制
    RouterFloor      string
    RouterCeiling    string

    // Sprint 15 加入：workflow depth（4 维度 + review + evolve）
    DiscoverDepth    string
    DesignDepth      string
    ADR              bool
    ReviewDepth      ReviewDepth
    EvolveDepth      int
    ReviewSkip       bool // derived

    // Sprint 18 加入：enforce 级别
    Enforce          string
}
```

这是**13 个字段、7 个跨 sprint 加入的不同职责领域**。它们是扁平的、无结构化的——  
没有子结构体、没有版本标记、没有「哪些字段共同属于同一个配置子域」的提示。

### 为什么这是问题

1. **内聚性破坏**:gate-set 过滤、coverage 阈值、model tier、workflow depth、enforce 级别  
   是**五个不同配置域**,彼此没有直接依赖关系。把它们全塞进同一个结构体使得：
   - 任何一个域的修改都需要重新编译所有引用 `mode.Policy` 的包
   - 新加入的 mode 子域（如 `rate_limit`、`parallelism`）天然倾向于放在这里,加剧膨胀

2. **零值语义模糊**:`Policy` 的零值曾是"不做 mode gating,全量执行"的简洁契约。  
   现在 13 个字段的零值分散在不同 sprint 加入,各自有不同的向后兼容含义——
   新来的开发者无法通过阅读零值理解默认行为。

3. **「中枢」反模式**:一个包同时是「路由中枢」「workflow depth 中枢」「配置中枢」——  
   这正是 `arch-check` 的反模式命名规则禁止的（`AGENTS.md` 明确列出禁用的目录名如  
   `utils / common / manager`）。`mode` 正在朝这个方向演化。

### 边界场景

| 场景 | 当前表现 | 子域隔离后的表现 |
|------|----------|-----------------|
| 需要为某个 mode 单独调 coverage 阈值但不改 gate-set | 修改 `Policy` 的 `CoverageThreshold`——可能影响其他子系统对该字段的依赖 | 修改 `CoveragePolicy` 子结构体,作用域清晰 |
| 想为 `coverage` 维度添加版本化的策略历史 | 需要在 `Policy` 上加 `coverageHistory` 字段 | 在 `CoveragePolicy` 上加 `History` 字段,不膨胀主结构 |
| 阅读代码:想理解「当前 mode 下 coverage 阈值是多少」| 需要理解 `Policy` 的全部 13 个字段 + 每层的 override 逻辑 | 只需看 `Policy.Coverage` 子结构 |

### 为什么高价值

1. **阻止 God 结构的形成**:13 个字段、7 个职责域共处一室是开始,不是终点。  
   随着更多 mode 感知特性加入（rate-limit、parallelism、model-pool 选择）,  
   `mode.Policy` 会自然膨胀到不可维护——除非现在建立子域隔离。

2. **提升可测试性**:`TestPolicy` 目前需要为每个测试构建完整的 `Policy{}` 字面量。  
   子域隔离后,测试 coverage 阈值的测试只需要构造 `CoveragePolicy{}`,零认知负担。

3. **保持「中枢旋钮」的可理解性**:`mode.Policy` 是 forge-core 中最广泛传递的结构体——  
   它被注入 orchestrator、gate resolver、converge、migrate、甚至 trace。  
   它的清晰度直接决定了整个系统的可理解性。

### 诚实边界

提议**不是**「把 `mode.Policy` 拆成 5 个独立包」——那是过度工程化。  
提议是：通过 **Go 嵌套结构体**（`type Policy struct { Coverage CoveragePolicy; Review ReviewPolicy; ... }`）  
在同一个包内建立子域边界。不改变任何导出 API 的消费方式（`policy.CoverageThreshold` → `policy.Coverage.Threshold`  
是一次性的机械替换）。

---

## 方向四 · 收敛信号扩展性缺口：`evalOne` 的硬编码 switch

> **类型**: 架构扩展性 · **类别**: 插件缺口 · 领域可演化性  
> **去重验证**: `converge.*plugin|signal.*registry|criterion.*registry|evalOne.*dispatch|  
>   custom.*converge.*criterion|converge.*middleware` —  
>   167 篇需求文档中 **零篇** 提出收敛信号的注册/插件模式。

### 问题

收敛引擎的核心是 `internal/converge/converge.go` 中的 `evalOne` 函数：

```go
// converge.go:116-133
func evalOne(c asset.Criterion, sig Signals) Result {
    switch {
    case c.Metric == "roadmap_completion":
        return evalRoadmap(c, sig)
    case c.Metric == "gates_status":
        ...
    case c.Metric == "requirement_confidence":
        return evalRequirementConfidence(c, sig)
    case c.Metric == "review_status":
        return evalReviewStatus(c, sig)
    case acceptanceMetrics[c.Metric]:
        return evalCriterion(c, sig)
    default:
        return Result{..., false, unknownDetail(c)}
    }
}
```

每一次新增收敛信号（Sprint 28: `review_status`、Sprint 29: `requirement_confidence`、  
Sprint 29 第二个: `file_delta`）,都需要：
1. 在 `Signals` 结构体中加字段
2. 在 `evalOne` 中加 `case`
3. 在 `gatherSignals` 中加赋值逻辑
4. 更新全部测试 fixture

这是**四次代码修改、跨 3 个文件**,只为加一个信号。如果每个信号都需要同样的四步,  
收敛引擎将随着系统增长变得更难维护。

### 代码级影响

`converge.Signals` 结构体本身也是一个增长点：

```go
// converge.go:25-90
type Signals struct {
    RoadmapCompletion    float64
    GatesGreen           bool
    RequirementConfidence float64
    ReviewStatus         string
    FileDelta            float64
    HumanApproved        bool
    Criteria             map[string]string
    GateProof            GateProof
    CodeTestRatio        float64
}
```

**9 个字段,6 个为收敛服务**。每个新 agent 合约（新的 VERDICT token 或 CONFIDENCE token）  
都需要在这面墙上开一个新洞。

### 已有替代方案的缺失

当前没有任何机制能让 workflow 作者**声明一个新的收敛判据而不改 forge-core**。  
一个使用 ForgeOS 的团队如果想说"我们的 workflow 在 `deployment_status == green` 时才收敛",  
只能 fork `internal/converge` ——这不是平台该要求的。

### 为什么高价值

1. **解锁 workflow 领域的自表达**:工作流声明 `stop_condition: {all_of: [{metric: "deploy_green", ...}]}`  
   目前只能走 `default` 分支 → 永远 unmet。一个注册/插件模式让 workflow 作者（或 ForgeOS 部署者）  
   注册自定义评估函数,无需 fork 核心代码。

2. **降低新信号的交付成本**:现在加一个信号 -> 改 4 处代码、跨 3 个文件。  
   注册模式 -> 在 `evalOne` 加一行表查找 + 实现一个 `func(Signals) Result`。  
   从 4 处修改降为 2 处。

3. **测试隔离**:新信号的测试不需要修改 `converge_test.go` 中已有的 400 行 fixture。

### 诚实边界

这不是要建立一个 SPI（服务提供者接口）框架 —— 那是 v3 的投机。  
当前只需要一个简单的**内部注册表**（`var signalEvaluators map[string]func(Signals) Result`）  
加上一个 `RegisterSignal(name string, fn func(Signals) Result)` 初始化函数。  
内置信号（roadmap、gates）保持硬编码的优先匹配；自定义信号通过注册表匹配。

### 已有分析交叉引用

~25 篇分析讨论了单个收敛信号的具体缺失（如 `review_status` 在 Sprint 28 的补线）,  
**但零篇讨论过 `evalOne` 的 switch 语句作为架构扩展性瓶颈**。方向一 in `five-codegrounded-architectural-product-gaps.md`  
提到了 `custom.*converge.*criterion` 但落在「输出合约验证」语境下,不是插件模式。

---

## 方向五 · 包依赖拓扑的隐式承诺：`cmd/forge` 作为唯一的导入集线器

> **类型**: 架构依赖 · **类别**: 隐式集线器 · 耦合风险  
> **去重验证**: `package.*hierarchy|import.*graph*forge|dependency.*structur.*forge|  
>   internal.*packag.*structur.*forge|package.*layering.*forge|import.*hub|hub.*package` —  
>   167 篇需求文档中 **1 篇** 提及依赖结构但未作展开。

### 问题

`forge-core` 的 Go 包依赖图有一个**隐式拓扑**：所有 `internal/` 包是纯叶子或低层包,  
**唯一导入全部 12 个 internal 包的只有 `cmd/forge`**。这意味着 `cmd/forge` 是整个  
应用的**唯一组装点**。

```text
                  cmd/forge (hub — imports all)
                 /    |    \       \
    internal/    /     |     \       \
    ┌─────┐ ┌────┐ ┌──────┐ ┌──────┐
    │mode │ │gate│ │asset │ │memory│ ... (12 packages)
    └─────┘ └────┘ └──────┘ └──────┘
         (leaf)  (leaf) (shared) (leaf)
```

当前这个集线器模式是可行的——因为只有一个消费者。但存在两个结构性问题：

**问题① — 没有中间层接口**:12 个 internal 包之间有一些间接依赖（`orchestrator` 依赖  
`asset`/`gate`/`mode`；`converge` 依赖 `asset`）,但这些依赖是**编译时直接 import**。  
没有接口层来解耦——如果某天想替换 `internal/memory` 的实现（比如从 JSONL 换成 SQLite）,  
每个直接 import 它的包都需要修改。

**问题② — 没有领域聚合**:12 个 internal 包代表不同领域（编排、路由、收敛、记忆、追踪、持久化...）,  
但没有一个聚合层（如 `internal/forge` 或 `internal/app`）将它们组合为「ForgeOS 运行时」这个  
有意义的产物。`cmd/forge` 的 `buildRunEngine` 实际上就是这个聚合——但它被藏在 CLI 包中。

### 依赖分析（来自 go list 的事实）

```
forge-core/internal or cmd 包依赖拓扑（简化）:
cmd/forge → asset, orchestrator, gate, mode, prompt, memory, persist, 
            trace, risk, routing, migrate, doctor, attribution, adr, yaml2json, yamlpath
            （18 个 internal 包中的 16 个,仅 converge、yaml2json 等少数不直接依赖）

internal/orchestrator → asset, gate, mode, converge
internal/converge → asset
internal/gate → (零 forge-core internal 依赖——最干净的叶子)
internal/memory → (零 forge-core internal 依赖,仅 stdlib)
internal/trace → (零依赖,仅 stdlib)
internal/prompt → (零 forge-core internal 依赖,仅 stdlib + os)
```

### 为什么这是问题

1. **`cmd/forge` 成为唯一的可运行入口**:要运行 forge-core 的任意功能,必须通过 `cmd/forge`。  
   无法将一个 forge-core 子集嵌入到其他 Go 程序（守护进程、API 服务、CI 插件）中,  
   除非承受导入整个 `cmd/forge`（及间接的 16 个包）的代价。

2. **接口缺失阻碍实现替换**:当前所有依赖都是直接的 Go import。如果未来要支持  
   `persist` 的不同后端（本地文件 vs S3 vs etcd）,没有任何接口层可以插入——  
   需要修改每一个调用了 `persist.Save` 的包。

3. **隐式集线器是未文档化的架构约束**:新加入一个 internal 包,如果要被 forge-core 使用,  
   必须确保 `cmd/forge` 正确地导入和组装它。没有显式的「这里注册你的组件」入口——  
   只有「去 cmd/forge 的某个函数里加几行代码」的传统。

### 边界场景

| 场景 | 当前表现 | 有聚合层的表现 |
|------|----------|---------------|
| 想写一个轻量级的 health-check 守护进程只启动 doctor + persist | 只能导入整个 `cmd/forge` | 从 `internal/app` 只取需要的子集 |
| 想替换 memory 后端从 JSONL 改为 PostgreSQL | 每个 import memory 的包都要改 | 通过 `interface` 层替换实现 |
| 想在 CI 脚本中直接调用 `converge.Evaluate` 做 post-deploy 验证 | 需要 import `cmd/forge` 并承受它的全部传递依赖 | 从 `internal/converge` 直接调用 |

### 为什么高价值

1. **架构文档化**:显式的聚合层（`internal/app` 或 `internal/engine`）将「ForgeOS 运行时由一个  
   编排引擎 + 收敛评估器 + 路径选择器 + 记忆存储构成」这一架构事实编码为 Go 代码,  
   而非散落在 sprint 记录中。

2. **组件化测试**:目前集成测试只能通过 `cmd/forge` 的 `TestRun_*` 进行,它们启动完整的  
   运行时。有聚合层后,可以只组装需要的组件（如 Engine + Converge 但不启动 Memory）,  
   做更聚焦的集成测试。

3. **为守护进程和分布式部署铺路**:未来 ForgeOS 的守护进程模式、gRPC 服务、Webhook  
   接收器,都需要在不承载 CLI flag 解析的情况下使用 forge-core 的核心能力。  
   聚合层是这些模式的架构前提。

### 诚实边界

提议不是「为每个 internal 包加上 interface」——那会引入不必要的间接层,且违背  
Go 社区「接受接口,返回结构」的惯例。提议是：
- 为确实可能有多个实现的子系统（persist、memory、trace 等）建立接口
- 创建一个 `internal/engine` 或 `internal/app` 包作为聚合层,把 `buildRunEngine`  
  从 `cmd/forge` 提升到领域层

### 已有分析交叉引用

`docs/requirements/2026-07-11-four-code-grounded-product-expansion-directions.md` 中方向四  
在讨论「守护进程模式」时提及需要「`forge-core` 从 CLI 包中释放出可编程 API」。  
但**从未分析当前依赖拓扑的隐式集线器模式**作为独立的结构性缺口。

---

## 总结：优先级与发展路线

| 方向 | 核心价值 | 风险等级 | 预估投入 | 独立可验证成功标准 |
|------|---------|---------|---------|-----------------|
| ① AgentExecutor 结构化结果 | 消除 3 套 ad-hoc 解析,打开 phase 输出契约 | 🔴 P1 | 中（~400 行） | `Execute` 返回结构化类型;现有 VERDICT 解析被替换为类型字段访问 |
| ② 业务逻辑层分离 | 终结「每 3 轮 sprint 拆一次包」循环 | 🔴 P1 | 中（~500 行 + 移动） | 6 个月后 `cmd/forge` 文件数稳定在 ≤ 16 无需上调上限 |
| ③ mode 子域隔离 | 阻止 God 结构蔓延,保持中枢旋钮可理解 | 🟡 P2 | 低（~200 行重排） | `mode.Policy` 字段按子域分组;新 mode 特性在子结构中添加 |
| ④ converge 信号注册表 | 让 workflow 作者表达自定义收敛条件 | 🟡 P2 | 低（~150 行） | 内置信号行为不变;workflow 声明自定义 metric 可被注册的 eval 函数处理 |
| ⑤ 包依赖拓扑与聚合层 | 让 forge-core 可被其他 Go 程序嵌入 | 🟢 P3 | 高（~800 行 + 接口设计） | 新 `internal/engine` 包;`cmd/forge` 从该包调用而非直接组装 |

**建议起点**:方向③（低投入、高可见度）→ 方向④（低投入、解锁 workflow 自表达）→  
方向①（中投入、消除架构缺口）→ 方向②（中投入、结构性改善）→ 方向⑤（高投入、长期基础设施）。

所有方向保持「在当前 18 包零外部依赖架构内可完成」的约束,不引入新的外部依赖。  
方向② 完成后,`cmd/forge` 作为 CLI 外观的角色变薄,为守护进程模式（已有多篇分析提及）  
提供直接的架构基础。
