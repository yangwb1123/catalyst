# ForgeOS — 五个结构性「毛细缺口」：已构建但未连通的系统能量释放点

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局扫描 forge-core（18 Go 包 + cmd/forge，纯 stdlib 零依赖）、harness（~15 模块，包含 gate/check/accept/adapters/secret-scan/SCA/select-tests）、`.agent/`（5 工作流 / 12 agent 卡 / 9 skill 卡 / policies / routing / modes）、`.arch/rules.yaml`、`pi-batch.py`  
> 2. 完整阅读 FUNCTIONAL_REQUIREMENTS_AUDIT（90+ DONE / 14 GAP 全部关闭）、CURRENT_SPRINT.md（31 个 sprint 演进记录）、全部 ADR + DECISIONS  
> 3. **差异化验证**: 在已有 120+ 份分析文档（`docs/requirements/` 80+ 篇 + `docs/analysis/` 40+ 篇）中，对每个方向做全文关键词检索 + 语义比对，确认该方向的**核心命题未被任何已有分析作为独立系统性缺口展开过**  
> 4. **纪律**: 不编写任何代码。每个方向附精确 `file:line` 代码证据、边界场景、产品价值判断。

---

## 全景定位

ForgeOS 经过 31 轮 Sprint 和 120+ 份分析文档的反复深扫，其编排内核、治理闭环、安全护栏和性能优化已被深度覆盖。但深度阅读代码后发现一个重复出现的模式：**存在一些已构建完整、已通过测试、却被零消费的基础设施组件**，以及**一些在代码中已经「做对」但被上游/下游消费方绕过或遗落的结构性能量释放点**。

以下五个方向均不是「新功能的设想」，而是 **「毛细缺口」（Capillary Gaps）**——代码中存在机制但未连通的微小缝隙。接通它们的成本极低（每方向 1–3 个 Sprint），释放的能量却贯穿整个系统。

---

## 方向一 · `internal/yamlpath`：已构建零消费的基础设施孤岛

> **类型**: 架构债务 · 基础设施利用率  
> **优先级**: P2  
> **已有分析覆盖检查**: 关键词 `yamlpath.*consum\|yamlpath.*wired\|yamlpath.*unused\|yamlpath.*dead\|yamlpath.*zero.*consumer` 在全部 120+ 份已有分析中**零命中**（FUNCTIONAL_REQUIREMENTS_AUDIT 仅在 `mode_gating` 表格行中作为附带证据提过一次，未作为独立方向展开）

### 问题

`forge-core/internal/yamlpath` 是一个完整的 YAML 路径引用解析包。它实现了：
- `Parse(ref string)`：拆解 `../policies/modes.yml#workflow_depth.reviewer` 为 `Ref{File, Path}`
- `MustParse(ref string)`：panic-on-error 版本
- `Resolve(repoRoot, ref, baseDir)`：读目标 YAML → 走 python shim 转 JSON → 沿 dot-path 走 JSON 树 → 返回值
- `ShimPath(repoRoot)`：python yaml2json shim 路径
- `mapKeys()` / `walkPath()` / `parseIndex()`：完整的 JSON 树遍历基础设施

**代码证据**：

```go
// forge-core/internal/yamlpath/yamlpath.go:31-135
// 全部公共 API：Parse, MustParse, Resolve, ShimPath, 内部 walkPath 实现
// 构建通过、测试通过（yamlpath_test.go 有完整测试）

// 但全仓非测试文件零消费：
// $ grep -rn "yamlpath\." forge-core/ --include="*.go" | grep -v "_test.go"
// → 零输出 (仅 yamlpath/yamlpath.go 自身的定义)
```

**这个包存在的理由**是 workflow 中的 `required_when: ../policies/modes.yml#workflow_depth.reviewer` 这类引用。在静态验证层面，`harness/check.py` 已经通过 Python 实现了校验（Sprint 31 新增的 `check_workflow_mode_gating`）。**但 Go 运行时从未使用它**——`asset.Workflow` 的 `ModeGating` 字段在 JSON 解码时被静默丢弃（`encoding/json` 不认识的字段直接忽略），整个 `mode_gating` 顶层块在 Go 侧是透明数据。

### 为什么这是一个「毛细缺口」而非「正常推迟」

关键区别：这个包**不是计划推迟**（如 v3 的 Firecracker/Web UI），也不是未实现（如 v2 的 full scorer），而是**已经构建完毕、测试通过、但从未接通**的代码。它产生了真实的维护成本（每次 API 变更需同步、`go vet`/`go build`/`go test` 每次都跑），却输出零运行时价值。

### 边界场景

| 场景 | 当前行为 | 如果接通的行为 |
|------|---------|--------------|
| Workflow 中 `mode_gating` 声明与 `modes.yml` 漂移 | `check_workflow_mode_gating` 在 Python 侧检测（Sprint 31），但 Go 运行时无从知晓 | Go 侧在 `asset.LoadWorkflow` 时自动 resolve 引用 → 构建解析后的 `PolicyRef` 字段 → 运行时直接消费，消除双解析器分裂 |
| 新 workflow 文件使用了未知 `required_when` 路径 | Python 校验通过（正则匹配），但 Go 解析路径时发现不存在 → 降级 | 在 asset 加载阶段就 resolve 引用 → fail-fast，而非等到 phase 执行时静默跳过 |
| 用户自定义 workflow 引用自己的 policy 文件 | 完全不支持——`internal/mode` 只有手写 Go 镜像 | `yamlpath.Resolve` 提供通用的「按路径读 YAML→取子值」原语，无需每次加新 policy 维度都改 Go 代码 |

### 产品价值

- **消除双解析器分裂**：当前 `mode_gating` 的含义由 Python 检、Go 不检。Go 侧如果能在 `asset.Load` 时自动 resolve `authority:` 引用到 `modes.yml`，就能在资产加载时就完成策略验证，而非留到 check.py 做后验。
- **为新维度的策略引用铺路**：任何 workflow 文件将来声明 `something: some.file.yml#some.path` 都不需要改 asset 包——`yamlpath.Resolve` 已经提供了通用原语。
- **减少未使用代码的维护税**：180 行 Go 代码构建、测试、lint 每次 CI 都跑，但零产出。

### 实现量级估计

1–2 sprint。核心工作：① 在 `asset.LoadWorkflow` 路径上识别 `mode_gating.authority` → 调用 `yamlpath.Resolve` → 将解析后的值注入一个可消费的字段（如 `asset.Workflow.ResolvedModeGating`）；② 在 `internal/mode` 中增加一条「如果已 resolve 则直接使用，否则 fallback 到手写镜像」的路径；③ 清理 `check.py` 中已冗余的 `check_workflow_mode_gating`（或改为 cross-check）。

---

## 方向二 · 全链路厂商耦合：从 argv 到输出解析的 Claude 依赖链

> **类型**: 架构灵活性 · 可扩展性  
> **优先级**: P1  
> **已有分析覆盖检查**: 现有多篇文档覆盖了 `cost.go` 的 Claude JSON 格式硬编码（`2026-07-11-five-structural-debt-and-product-frontiers.md` 方向二、《Agent CLI 契约最小化》方向），但**没有任何已有分析文档完整列举从 argv 构造 → 执行 → 输出解析 → 成本提取 → 过载检测 → 结果渲染的完整 6 阶段依赖链**，也未讨论将其抽象化的系统性方法。

### 问题

现有分析反复指出「`cost.go` 硬编码了 `total_cost_usd` 字段」，但这只是冰山一角。完整的有向依赖链如下：

```
阶段 1 — argv 构造 (engine_build.go:93-117)
  claudeArgv() 生成 vendor-specific flags:
    --model <tier>              ← Claude 的 --model 语义
    --permission-mode acceptEdits  ← Claude 特有
    --allowedTools "Bash(node...)"  ← Claude 特有的 allow 语法
    --disallowedTools "Edit Write"  ← Claude 特有
    --max-budget-usd <N>         ← Claude 的 --max-budget-usd
    -p <prompt>                 ← Claude 的 prompt-as-arg 模式
  └──→ 非 claude (如 echo/stub) 路径长度：~10 行

阶段 2 — 执行 (command_executor.go:40-93)
  CommandExecutor.Execute → os/exec 跑 argv[0]
  ★ 此处接口还算通用，但不通用在下游消费

阶段 3 — 输出解析 (cost.go:42-75, cost.go:310-340)
  parseClaudeCostUsd() ← 硬编码 Claude JSON 信封：
    type claudeResult struct {
      TotalCostUsd *float64 `json:"total_cost_usd"`  // Claude 专有字段
    }
  同类: parseReviewerVerdict 解析末行 "VERDICT: APPROVE" ← 文本契约
        parseExecutiveVerdict 解析末行 "VERDICT: REDESIGN" ← 文本契约
        parseConfidenceScore 解析末行 "CONFIDENCE: 85" ← 文本契约
  ★ 所有契约都是行级文本匹配，无结构化协议

阶段 4 — 成本提取 (engine_build.go:61-69)
  observeFor() 只在 isClaude==true 时接 parseClaudeCostUsd
  非 claude: cost 永远是 0/nil

阶段 5 — 过载检测 (engine_build.go:66-69)
  classifyClaudeOverload() ← Claude 529 overloaded_error 字符串匹配
  ★ 非 claude 的过载信号（如 HTTP 503/Gemini 的 ResourceExhausted）无法识别

阶段 6 — 结果渲染 (engine_build.go:64-65)
  unwrapClaudeResult() ← 剥离 Claude 的 JSON 信封，保留实际 agent 输出
  ★ 非 claude 的输出直接暴露给下游 phase

// engine_build.go:48 — isClaude 判断是一个字符串包含检查
isClaude := strings.Contains(o.agentCmd, "claude")
```

**六个阶段中五个有 Claude-specific 逻辑。** `agentCmd` 如果是 `"gemini-cli"` 或 `"opencode"`，每个阶段都会产生错误行为：
1. argv 不符合目标 CLI 的语法 → fork 失败或参数被忽略
2. 输出格式不符 → cost=0，budget guard 失效
3. 过载信号格式不符 → 所有错误被归为 KindFailed（不可重试）
4. 结果渲染错误 → agent 输出丢失或被双重包裹
5. 文本契约（VERDICT/CONFIDENCE）格式可能不同 → 收敛判断失效

### 边界场景

| 场景 | 当前行为 | 问题 |
|------|---------|------|
| 切换到 Gemini CLI | argv 全部错误（Gemini 没有 `--model` 类似语义） | 进程启动即失败 |
| 切换到 OpenCode | `--permission-mode` 无定义，`--output-format json` 不存在 | cost=0，budget guard 退化 |
| Claude CLI 升级大版本 | `--output-format json` 字段名改为 `"total_billed_usd"` | cost 静默丢失，无版本协商机制 |
| 混合使用（某些 phase 用 Claude，某些用 Gemini） | `isClaude` 是全局 bool，分支为同一个 executor | 架构上不支持混合 |

### 产品价值

这是 ForgeOS G3（多维模型路由）的架构前提。G3 承诺跨厂商路由——但当前架构只能在 CLI 启动时选择「用哪个 CLI」，无法在 phase 粒度按 (cost/latency/capability) 选择 Provider。接通完整的厂商抽象层是 G3 从叙述成为现实的唯一路径。

### 实现量级估计

3–5 sprint。核心工作：① 定义 `AgentCLI` 接口（`CommandBuilder` + `OutputParser` + `OverloadDetector` + `ResultRenderer`）；② 将 `isClaude` 分支重构为策略模式；③ 实现第二家 CLI（如 OpenCode/Codex）作为第一个跨厂商验证；④ 将三个文本契约（VERDICT/CONFIDENCE/EXECUTIVE_VERDICT）从行级正则升级为结构化协议（或至少版本化）。

---

## 方向三 · 跨会话知识生命周期：内存存储的长期增长无管理机制

> **类型**: 知识管理 · 韧性  
> **优先级**: P1  
> **已有分析覆盖检查**: 多篇已有文档覆盖了跨存储一致性（checkpoint/trace/memory 无共享 Run ID）、单会话内存 cap（`memoryCap=32`）、内存压缩（Compact/Prune），但**没有任何已有分析讨论跨会话（跨天/周）的 memory.jsonl 生命周期管理**——TTL、归档、知识合并、过期策略。所有讨论都止步于「单次 evolve 循环内」。

### 问题

`internal/memory` 使用 JSONL（append-only log）作为持久化格式。每次 `memory.Append` 写一行，`memory.Load` 读全部行。该设计在单次 evolve 循环内是正确的（"grow-only log"），但当前没有任何机制管理**跨会话**的长期增长。

```go
// forge-core/internal/memory/memory.go:106 — Load 读整个文件
func Load(path string) ([]Entry, error) {
    // ... 读所有行到 []Entry ...
    // 没有过滤条件，没有行数上限，没有时间段过滤
}

// forge-core/internal/memory/memory.go:48 — 缓存是按 session 的
var loadCaches sync.Map // key=path(string)，按 mtime 缓存
// ★ 缓存只在当前进程生命周期内有效。新 forge evolve 创建一个新进程。
```

具体问题链：

1. **跨会话增长**：一次 `forge evolve --max-iter 5` 可能产生 20-50 条 memory entries（每迭代 4-10 条）。如果用户每周跑一次 evolve，一年后 `.forge/memory.jsonl` 就有 ~2000 条。`memory.Load` 每次读全部。

2. **无 TTL/过期**：所有 entry 永久保留。一年前的一个 `finding` 和一分钟前的一样有资格被注入 agent prompt。`memory.Query` 没有时间过滤器。

3. **输入膨胀**：每个 agent phase 都执行 `appendMemoryLanes`（`prompt_memory.go:180`），从 memory store 中 `Query` 相关内容注入到 prompt。2000 条 memory 意味着每次 agent 调用都要扫描全部 2000 条、排序、过滤、注入。Agent 的上下文窗有限——注入无关旧知识会稀释真正相关的近期知识。

4. **知识陈旧**：一个 `decision` 类型的 entry 说「Module X 使用 Postgres，不要用 SQLite」——6 个月后架构升级迁移到 CockroachDB，这条旧决策仍然被注入，误导 agent。

```go
// forge-core/internal/prompt/prompt_memory.go:48 — 单会话 cap 是 32
const memoryCap = 32 // max entries injected per prompt
```
注意：`memoryCap=32` 是从 store 中选出的最多 32 条注入 prompt，但 store 本身可以无限增长。而且选择逻辑（`Query`）目前只按 `Kind+Topic` 过滤 + 分数排序，没有时间衰减。

### 边界场景

| 场景 | 当前行为 | 问题 |
|------|---------|------|
| 一年前的 `finding: "service X has high memory usage"` | 被 `Query` 选出注入 prompt（如果 topic 匹配） | agent 花 token 读过时诊断，可能误判当前状态 |
| 第 100 次 evolve 迭代 | `memory.jsonl` 有 ~800 条 | Load 读 800 条（IO 10ms+），Query 扫 800 条，注入 32 条中可能只有 3 条相关 |
| 两次 evolve 隔了 3 月，架构已变 | 旧 `decision` 仍被注入 | agent 被历史决策 anchor，可能不自觉地回退到旧方案 |
| 同一个 `topic` 有 50 条互相矛盾的 finding | 全被 Query 选中（按分数前 32 名） | 注入 32 条矛盾信息 → agent 无所适从 |

### 产品价值

ForgeOS 的核心卖点是 「24h 无人值守 × 持续演化」。演化一周、一月的系统会积累大量 memory。如果没有长期知识生命周期管理，memory 将从「助手」退化为「噪声生成器」。这是系统从「跑几天」到「跑几个月」的必经之路。

### 实现量级估计

2–3 sprint。核心工作：① 为 `Entry` 增加可选 `TTL` 字段（workflow YAML 可配置默认 TTL）；② `memory.Load` 支持时间范围过滤（或惰性加载 + 按时间衰减的 `Query` 排序）；③ 实现 `memory.Archive(path)`：将旧 entry（>TTL）移到归档文件，主文件只保留活跃 entry；④ 可选：实现跨会话的知识合并（相似 entry 自动合并，而非简单删除）。

---

## 方向四 · 运行时自动路由决策与手动 CLI 路由之间的结构性断裂

> **类型**: 功能完整性 · G3 护城河  
> **优先级**: P1  
> **已有分析覆盖检查**: FUNCTIONAL_REQUIREMENTS_AUDIT 确认了 G3 多维模型路由不驱动真实执行，但将其改判为 DEFERRED-BY-DESIGN（Sprint 30 复核后的结论），且已有分析文档主要讨论「缺什么功能」(full scorer 未接入)。**没有任何已有分析深入分析「为什么现有的 scorer 无法接入运行时」的结构性原因**——它的输入（复杂度/依赖/上下文/业务影响）目前全部是手工 CLI flag，没有任何自动计算路径。

### 问题

`forge route` CLI 实现了完整的多维评分路由：

```go
// forge-core/cmd/forge/route.go:81 — CLI 路径
score := routing.Score(dims, dimWeights)  // ← 6 维度加权评分
tier  := routing.TierForScore(score, o.taskType, effRisk, o.budget)

// forge-core/internal/routing/routing.go:177 — Score 函数是纯函数
func Score(dims map[string]float64, weights map[string]float64) float64 { ... }
```

但运行时路径完全绕过它：

```go
// forge-core/cmd/forge/engine_build.go:276-318 — 运行时路径
func phaseTierResolver(mode string, spendRatio func() float64, cards []routing.Scorecard, ...) func(p asset.Phase) string {
    // 只做了三件事：
    // 1. riskAdjustedTier ← risk.Classify(risk.FromChangedPaths(...))
    // 2. BudgetAdjustTier ← spendRatio()
    // 3. HistoryTiebreak ← cards
    // ★ 完全不调用 routing.Score()
    // ★ 完全不读 complexity / dependency / context / business_impact
}
```

**为什么不能简单地把 engine_build.go 里加一行 `routing.Score(dims, dimWeights)`？**

因为 `dims` 的输入来源不存在：

| 维度 | `forge route` CLI | 运行时路径 |
|------|------------------|-----------|
| `complexity` | `--complexity` (手动) | 无自动计算 |
| `risk` | `--risk` 或 `risk.FromChangedPaths`（路径启发式） | 仅 `risk.FromChangedPaths`（同） |
| `dependency_change` | `--dependency` (手动) | 无自动计算 |
| `security` | `--touches-payment/--touches-auth/--touches-secrets` (手动) | 仅 `risk.Signals`（路径启发式） |
| `context_size` | `--context` (手动) | 无自动计算 |
| `business_impact` | `--business` (手动) | 无自动计算 |

六个维度中，运行时只能自动算出两个（`risk` 和部分 `security`），其余四个**完全依赖手动输入**。所以不是「不愿意接通」，而是「缺少自动特征提取管道」。这正是 G3 所称的「v2+ Router service」。

### 真正的结构性难点（已有分析未展开）

1. **`complexity` 需要代码分析**：评估一个 task 的复杂度需要读相关代码的圈复杂度、调用深度、修改范围——这需要 AST 级别的代码理解。当前 `internal/risk` 只能读文件路径。

2. **`dependency_change` 需要依赖图**：评估一个改动的影响范围需要完整的依赖图。`arch-check.mjs` 有导入图但只用于执法（环/层/扇入），不输出给路由。

3. **`context_size` 需要 prompt 预估**：预估算法的上下文使用量需要 tokenize 策略 + 知识库大小。当前 `internal/prompt` 没有 token 计数能力。

4. **`business_impact` 需要语义理解**：哪些代码是核心业务逻辑、哪些是工具代码——这需要从 README/PRD 到代码的追溯。当前无机制。

### 边界场景

| 场景 | 当前行为 | 理想行为 |
|------|---------|---------|
| 一个大型重构 task（改 20 个文件） | 用中等模型（Sonnet），因为它属于 implementer | 自动提升到 Opus（复杂度高 + 影响范围大） |
| 一个改 README 的 trivial task | 用中等模型 | 自动降低到 Haiku（复杂度低、业务影响低） |
| 涉及支付模块的 3 行改动 | 用中等模型 | 自动提升到 Opus（security=critical，即使复杂度低） |

### 产品价值

这是 ForgeOS G3（多维模型路由）最后也是最大的一块拼图。全部基础设施已就位——Score 函数正确、TierForScore 正确、risk 分类器已接入、budget guard 已接入。缺的就是从「手动 CLI」到「自动运行时」的 4 条特征提取管道。接通后，ForgeOS 可以做到：**同样的 task，在 discover 阶段用 Haiku（便宜、快），在实现时自动升到 Sonnet/Opus（视复杂度/安全/影响），在评审时强制 Opus。** 无需任何人指定 `--model`。

### 实现量级估计

4–6 sprint（v1 核心 2 sprint + 每组额外 1 sprint 打磨）。核心工作：① 建立 `internal/router` 包作为自动特征提取层（从 phase、workflow、git diff、代码分析自动计算 6 维输入），将 now CLI 手动的输入逐一自动化；② `phaseTierResolver` 调用 `routing.Score` 替换当前的硬编码三路径；③ 可行性核心在于「不完美但可用」（honesty-first）：自动计算加置信度标签，不确定的维度保持保守/升级不降级。

---

## 方向五 · 输出契约的「沉默退化」风险：行级文本匹配的累积脆弱性

> **类型**: 韧性 · 安全  
> **优先级**: P2  
> **已有分析覆盖检查**: 已有分析覆盖了「度量可信度与对抗鲁棒性」（`2026-07-11-five-architect-extension-directions.md` 方向一），讨论 agent 伪造 `VERDICT: APPROVE`/`CONFIDENCE: 95` 来加速收敛的问题。**但没有任何已有分析从「解析器自身的退化脆弱性」角度展开**——不是 agent 是否说谎，而是解析器在面对格式漂移、多 agent 角色冲突、干扰文本时的行为。

### 问题

ForgeOS 的 agent→ orchestrator 回传通道完全依赖于**行级精确文本匹配**。共有四个解析器，全部在 `cmd/forge/cost.go` 中：

```go
// cost.go:310-340 — parseReviewerVerdict
// 匹配末行精确 "VERDICT: APPROVE" 或 "VERDICT: REQUEST_CHANGES"

// cost.go:350-370 — parseExecutiveVerdict
// 匹配末行精确 5 个 token 之一：
//   VERDICT: APPROVE | APPROVE_WITH_SIMPLIFICATION | REDESIGN | DELAY | REJECT

// cost.go:387-410 — parseConfidenceScore
// 匹配 "CONFIDENCE: <0-100>"（任何位置，不只是末行）

// cost.go:42-75 — parseClaudeCostUsd
// 解码 Claude JSON 信封中的 total_cost_usd 字段
```

这些解析器共享一个特征：**它们不是在结构化的协议上工作，而是在「agent 被指示在输出的某个位置写某行文本」的约定上工作。**

### 累积脆弱性

四个解析器各自独立，但有一个共同的退化模式：

1. **格式漂移**（Agent 输出格式随时间变化）：
   - Agent 在 `VERDICT: APPROVE` 后加了空行 → parseReviewerVerdict 精确末行匹配失效 → 收敛信号丢失
   - Agent 写了 `VERDICT: APPROVED`（多了 D） → 不匹配，静默退化
   - Claude 升级 JSON 格式，`total_cost_usd` 改名为 `total_billed_cost` → cost=0

2. **多角色混淆**（同一段输出被多个解析器尝试）：
   ```go
   // prompt_context.go:208-215 — observeFor 的 fallback 链
   if verdict, ok := parseReviewerVerdict(sanitized); ok { ... }
   else if score, ok := parseConfidenceScore(sanitized); ok { ... }
   // ★ reviewer 的 VERDICT 被误判为 CONFIDENCE 的风险很小（格式不同），
   //   但反过来——一个写 VERDICT 的 agent 同时产生 "CONFIDENCE: 95" 文本
   //   （比如在对话中讨论 confidence）— 就会静默污染置信度信号
   ```

3. **干扰文本**（agent 在讨论中合法使用这些关键词）：
   - Agent 在对话中说：「I think my confidence is about 80%」→ `parseConfidenceScore` 提取 `80` → 但这个 80 不是声明的 CONFIDENCE 契约
   - Review 对话中出现「If you disagree, I'll REQUEST_CHANGES」→ `parseReviewerVerdict` 可能误匹配

4. **fallback 链的无序竞争**（prompt_context.go 同前）：
   - 尝试顺序：parseReviewerVerdict → parseExecutiveVerdict → parseConfidenceScore
   - 如果 agent 在 executive review 输出中包含了关键词 `APPROVE`（例如讨论先前的循环），可能导致误匹配

### 边界场景

| 场景 | 当前行为 | 风险 |
|------|---------|------|
| Agent 在对话中写「The previous reviewer said VERDICT: APPROVE」 | 匹配到错误位置的 `VERDICT: APPROVE` | 当前循环的信号被污染为 APPROVE |
| Claude API 更新，JSON 字段 rename | cost.go 解析失败，cost=nil | Budget guard 失效，scorecard cost 为 0 |
| Agent 在末行意外多了一个空格「VERDICT: APPROVE 」| 精确匹配失败 | 收敛判定卡在 NOT MET，但 agent 实际已批准 |
| 多个 agent phase 在同一轮产生多个 VERDICT | verdictLedger 记录最后写入的那个 | 前序评审意见丢失 |

### 产品价值

这不是「某一天会出问题」的理论风险——在 Sprint 27 已经真实发生过：yaml2json 的 block-scalar 解析错误导致每个真实 workflow 文件的 `description:` 字段被注入前缀 `"> "`，而差分测试只 `t.Logf` 从不 `t.Errorf`，导致 6/7 真文件走偏却全绿。**同样的模式可能在任何文本契约上重现。**

将四个解析器统一为一个结构化的、版本化的输出信封（structured output envelope），有以下好处：
- Agent 输出始终包裹在一个机器可读的 JSON 块中（如 `---FORGE-RESULT---{"verdict":"APPROVE","confidence":85}---FORGE-RESULT-END---`）
- 解析器按版本字段分派，而非 fallback 链
- 格式变更时可通过版本协商做向后兼容
- 消除干扰文本的误匹配

### 实现量级估计

2–3 sprint。核心工作：① 定义结构化输出信封的 schema（兼容当前纯文本模式的退化路径）；② 重构 `cost.go` 中的四个解析器为统一的 `parseAgentResult`；③ 更新所有 agent card 的机器可读契约段，指定新版输出格式；④ 为旧格式保留 3–6 个月的向后兼容期（`parseAgentResult` 先试新格式 → fallback 旧格式 → fallback 无信号）。

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 对接成本 | 一句话杠杆 |
|------|--------|------|---------|----------|
| ① yamlpath 基础设施接通 | P2 | 架构债务 | 1–2 sprint | 消除已构建零消费的孤岛代码，为新策略引用铺路 |
| ② 全链路厂商抽象 | P1 | 架构灵活性 | 3–5 sprint | G3 跨厂商路由的架构前提；当前 5/6 阶段硬编码 Claude |
| ③ 跨会话知识生命周期 | P1 | 韧性 | 2–3 sprint | 长期运行的 memory 从助手退化为噪声的唯一防线 |
| ④ 运行时自动路由 | P1 | 功能完整性 | 4–6 sprint | G3 最后拼图：scorer 已对，缺 4 条自动特征提取管道 |
| ⑤ 结构化输出契约 | P2 | 韧性 | 2–3 sprint | 消除行级文本匹配的累积脆弱性，Sprint 27 bug 模式可系统防止 |

**收敛建议**：

- **若只做一件**：方向②（全链路厂商抽象）——它是 G3 的架构前提，且线性依赖方向⑤（结构化输出契约可以纳入方向②的抽象层中一起做）。

- **做前三件**：② → ③ → ④ —— 「先松绑厂商依赖，再管理知识增长，最后接通自动路由」。这三件覆盖了 ForgeOS 从「真的能跑」到「真的能长期可靠地跑」的跃升。

- **方向① 和 ⑤** 更适合在路过时顺便解决（方向① 是一个 2 天的 PR，方向⑤ 可以附在方向② 的抽象工作中一起做），而非各自独立排期。

---

> **方法论边界（诚实标注）**：
> - 本文件的分析范围是 forge-core Go 运行时 + harness Node 层 + .agent/ 声明层。Web UI、外部依赖（Firecracker/LiteLLM/OSV-DB）、Python 智能层（forge-ai）不在分析范围内。
> - 每个方向的实现量级估计基于当前代码库尺寸和架构复杂度，未经实证验证。
> - 方向② 的「切换第二家 CLI」可能需要额外的 API 认证/环境配置，不在本估算范围。
> - 本文件不是 sprint 计划或功能需求规格书——它是一份结构性能量释放点的诊断。
