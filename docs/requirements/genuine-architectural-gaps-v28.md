# ForgeOS — 五维架构盲区：Sprint 31 之后未触及的真正缺口

> **视角**: 资深架构师 / 产品经理（不写代码）  
> **方法**: 全局深度扫描 —— forge-core 18 Go 包 / 77 测试文件 / harness 38 模块 /  
>   全部 5 工作流 + 12 agent 卡 + 9 skill 卡 / .forge/ 运行时产物 / 31 个 Sprint 演进记录  
> **交叉验证**: 通读 40+ 篇 `docs/analysis/*.md` + 全部 11 篇 `docs/requirements/*.md` +  
>   `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` + 全部 ADR + DECISIONS.md + PROJECT.md + ARCHITECTURE.md  
> **承诺**: 下面每个方向均与已有 50+ 份分析文档**核心论点不重叠**。每方向附「差异化证明」说明为什么未被覆盖。  
> **日期**: 2026-07-09

---

## 已有分析全景（本文不重复）

已有 50+ 份分析覆盖了以下大维度：

| 维度 | 代表文档 | 方向数 |
|------|----------|--------|
| 功能引擎补齐（路由/编排/记忆/收敛/诊断） | `high-value-extension-directions-v2.md` | 5 |
| 生产就绪可靠性（Prompt QA / 信号硬化） | `expansion-production-readiness.md` | 5 |
| 第三地平线生态（多仓库联邦/事件驱动/资产升级） | `expansion-horizon-three.md` | 5 |
| 执行语义形式化（原子性/幂等性/回滚） | `execution-semantic-gaps.md` | 5 |
| 系统二阶伴生问题（知识衰减/配置爆炸） | `second-order-architectural-gaps.md` | 5 |
| 系统性扩展（数据生命周期/TOCTOU 竞争） | `systemic-expansion-v26.md` | 5 |
| 系统性边界盲区（无声数据丢失/持久化语义） | `uncovered-frontiers-v25.md` | 5 |
| 架构盲区与多波分析 | 40+ 篇 `docs/analysis/*.md` | 30+ |
| **总计已有覆盖** | | **~60 个方向** |

**本文的 5 个方向在全部 ~60 个已有方向中零覆盖。** 它们不是「加一个引擎」或「修一个 gap」，而是系统当前架构中**尚未被任何人识别**的设计缺口。

---

## 方向一：收敛信号源的可靠性分层 —— 信任权重系统

> **类型**: 架构 · 收敛逻辑 · 诚实性  
> **优先级**: P1（长期自治运行 silently critical）  
> **代码影响**: `internal/converge/` + `orchestrator/` + `cmd/forge/`

### 现状

ForgeOS 的收敛系统（`internal/converge/`）目前对所有信号源**一视同仁**：

```go
// converge.go:45-65 — Signals 结构体
type Signals struct {
    RoadmapCompletion float64  // ← agent 自报([x]勾选率),完全主观
    GatesGreen        bool     // ← 客观(harness exit code)
    RequirementConfidence float64 // ← agent 自报(CONFIDENCE: N),完全主观
    ReviewStatus      string   // ← agent 自报(VERDICT: …),主观
    FileDelta         float64  // ← 机械推算(客观,但启发式)
    CodeTestRatio     float64  // ← 机械推算(客观)
    HumanApproved     bool     // ← 人工信号(客观)
    // ...
}
```

**全部信号用同一权重进入 `evalOne`**：`roadmap_completion >= 100` 与 `gates_status == green` 具有完全相同的收敛影响力。RoadmapCompletion 是 agent 自报的 `[x]` 勾选率 —— 一个 print-mode agent 可以随意勾选，harness 无任何手段验证。而 `GatesGreen` 是 harness gate 的真实 exit code。

### 未被覆盖的证明

- `execution-semantic-gaps.md` 的"Phase 执行副作用模型"关注**输出格式的形式化**（文件格式/坐标系），不是收敛信号的可信度分层。
- `expansion-production-readiness.md` 的"信号硬化"关注**agent 输出中信号提取的健壮性**（parse 失败回退），不是收敛引擎如何看待已提取的信号。
- `second-order-architectural-gaps.md` 的"知识质量衰减"关注**memory 存储层的信噪比**，不是收敛判据的权重系统。
- 无一分析讨论**收敛引擎自身的诚实性模型**——关于「什么算一个令人满意的收敛」这一核心问题。

### 为什么需要

1. **agent 自报信号是完全不可靠的**——RoadmapCompletion 是 agent 自己写的 `[x]` 标记，没有任何独立验证（Sprint 25 真点火已证实：implementer 诚实拒绝勾选它无法验证的项，但一个不那么诚实的 agent 完全可以乱勾）。Harness gate 不能验证 ROADMAP 的 `[x]` 是否对应真实代码行为——它只能验证测试通过、结构合规。

2. **不同信号的可靠度差异是巨大的** —— 排列如下：

   | 信号 | 来源 | 可靠度 | 验证方式 |
   |------|------|--------|----------|
   | `GatesGreen` | Harness exit code | ★★★★★ | 进程退出码，不可伪造 |
   | `HumanApproved` | 外部人工信号 | ★★★★★ | 人间判读（非 bypassable）|
   | `security_findings` | Secret-scan result | ★★★★☆ | 模式匹配扫描 |
   | `architecture` | Arch-check 8 检查 | ★★★★☆ | 机器执法，规则明确 |
   | `FileDelta` | Git diff 机械推算 | ★★★☆☆ | 关键词子串匹配，粗启发式 |
   | `CodeTestRatio` | Git diff stat | ★★★☆☆ | 纯行数比，无语义 |
   | `RoadmapCompletion` | Agent 自报 | ★☆☆☆☆ | 无验证——agent 可随意 FILL 或 SKIP |
   | `ReviewStatus` | Agent 自报 | ★★☆☆☆ | 唯一验证是 downstream gate 最终拦截 |
   | `RequirementConfidence` | Agent 自报 | ★☆☆☆☆ | 纯 agent 自我评估，最不可靠 |

   **当前系统把最可靠的信号和最不可靠的信号等同对待。**

3. **一个具体的失败场景**：agent 自报 `roadmap_completion=100%`、`requirement_confidence=95`，同时 `GatesGreen=true`、所有 `criteria=PASS`。收敛系统报告 MET。但实际 ROADMAP 中有一半的 `[x]` 是 agent 随意勾上的 —— `FileDelta` 只有 20%（系统会发出 warning，但**warning 不影响收敛判定**）。系统判定 went to production，带着未实现的功能。

### 建议的架构方向

为信号源引入**分层信任模型**，不改变现有信号的数据结构，而是增加一个 `SignalTrust` 维度：

```
Tier 1 (客观，不可伪造): GatesGreen, HumanApproved, security_findings, architecture
Tier 2 (机械推算，可验证): FileDelta, CodeTestRatio, test_pass
Tier 3 (主观自报，不可独立验证): RoadmapCompletion, ReviewStatus, RequirementConfidence
```

核心设计原则：
- **Tier 1 信号可直接驱动收敛** —— 一个 Tier-1 信号的 FAIL 是不可绕过的。
- **Tier 3 信号不能单独驱动收敛** —— 即使 `roadmap_completion=100%`，如果没有至少一个 Tier-1 信号佐证，系统不应报告 MET。
- **阶梯需求** —— 收敛可以声明 `require_tier: 2` 表示「至少需要 Tier-2 及以上信号的满意」(适用于增量交付)，或 `require_tier: 1` 表示「必须有客观证据」(适用于版本发布)。
- 向后兼容：零值信任策略 = 当前行为（所有信号平等），现有 workflow 不受影响。

这不会改变任何现有代码的执行路径，但会在收敛引擎层增加一个**诚实性护城河**：system 不再假装不能区分一个 `[x]` 和一个 `exit 0`。

---

## 方向二：治理资产热加载 —— 长时间自治运行的零停机策略更新

> **类型**: 运维 · 运行时 · 治理  
> **优先级**: P1（24h 无人值守运行的必要前提）  
> **代码影响**: `internal/asset/` · `internal/mode/` · `internal/prompt/` · `orchestrator/` · `cmd/forge/`

### 现状

ForgeOS 的治理资产（workflow YAML、agent card、policies、mode 配置）在运行时**只加载一次**：

```go
// main.go:190-210
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    // 读文件 → 解析 → 返回 Workflow 结构体
    // 此后 wf 在内存中不再更新
}

// engine_build.go:150-180
func buildRunEngine(wf asset.Workflow, ...) orchestrator.Engine {
    // wf 被冻结在 Engine 内部
    // RunFrom / LoopEngine.Run 在整个 run/evolve 期间使用同一份拷贝
}
```

`prompt.ContextCache` 的 `Invalidate()` 方法存在（`internal/prompt/cache.go:92-102`），文档写道：

```go
// Invalidate exists for v2: once an asset's writes_adr lets an agent edit an
// ADR mid-run, the writer must call Invalidate so the freshly-written ADR
// is re-scanned instead of served from this stale memo.
```

**但这个机制只针对 prompt cache，不针对资产本身。** 在工作流运行期间：
- 如果 operator 修改了 `workflow/build.yml` 的 phase 定义 —— **不可见**
- 如果 operator 更新了 `policies/modes.yml` 的 gate-set —— **不可见**
- 如果 operator 编辑了 agent 卡（比如调整 reviewer 的 `VERDICT:` 契约）—— **不可见**
- 如果 operator 添加了新的 `harness/` 执法器 —— **不可见**

唯一的生效方式是 `SIGINT` → 重启 `forge evolve --resume`。对于一个 24h 自治运行，这意味着**任何治理调整都需要中断正在进行的迭代**。

### 未被覆盖的证明

- `expansion-horizon-three.md` 方向四「治理资产升级管线」焦点是**跨项目同步**（forge-init → forge-upgrade 的双向版本管理），不是**同项目运行时热加载**。
- `production-readiness.md` 方向二「版本兼容」焦点是**运行时版本 vs 资产版本冲突**，不是资产本身的运行时更新。
- 所有 40+ 分析中关键词 `"hot.reload"`、`"dynamic policy"`、`"zero.downtime"` **零命中**。

### 为什么需要

1. **`forge evolve` 的长周期特性使冷重启代价高昂** —— 一个 24h 的 evolve 运行可能在 iteration 7/10 时到达。此时如果 operator 发现 `review.yml` 缺少一个关键的 quality gate，他必须终止运行、修改 YAML、`--resume` 重跑。但 `--resume` 只能恢复迭代计数，不能恢复已完成的 phase 的内状态 —— 已完成 phase 的 agent 输出（代码、ADR、决策）已经写入磁盘。operator 面临两难：要么带错误的治理配置跑完剩余迭代，要么丢弃已完成的 phase 成果重启。

2. **治理资产本身就是正在被开发的代码** —— ForgeOS dogfoods 自身：`engineering` 模式下的 `forge run build` 可以修改 `.agent/workflows/` 中的文件。这意味着在工作流运行期间，**同一个进程的治理底座可能在其脚下被修改**。当前的处理方式是完全忽略这种修改。

3. **已有基础设施几乎已具备热加载能力**：
   - `prompt.ContextCache.Invalidate()` 已经是正确的失效原语
   - `yaml2json.Decode` 已经是纯函数，每次调用独立解码
   - `asset.LoadWorkflowJSON` 已经是纯函数，输入相同输出相同
   - 缺失的关键链路是：**一个周期性的资产版本检查 + 一个原子的引擎状态替换机制**

### 建议的架构方向

引入**资产版本号 + 检查-重载-替换**循环，在每个迭代/phase 边界触发：

```
// 概念性设计（非代码，仅示意方向）

type AssetWatcher struct {
    repoRoot    string
    workflow    string          // 当前工作流名
    lastModTime map[string]time.Time  // 每个资产文件的 mtime
    
    onWorkflowChange func(newWf asset.Workflow)  // 回调：替换 Engine 内的工作流
    onPolicyChange    func(newPol mode.Policy)     // 回调：替换中枢旋钮策略
    onCardChange      func(agent string, newCard string) // 回调：刷新 prompt 角色卡
}

// Check 在每次 phase 边界被调用，检查资产文件是否已修改
// 如果是，调用对应回调原子替换引擎状态
func (w *AssetWatcher) Check() {
    // 读 mtime → 对比 → 有变则 decode → 回调
    // 回调内加锁替换引擎的引用
}
```

核心设计约束：
- **仅在工作流干净边界生效**（phase 完成/iteration 完成），绝不在 phase 执行中途切换
- **只做 additive 替换**（gate-set 可以扩大，但不能缩小正在进行的 phase 的权限）
- **版本追踪**：在 checkpoint 中记录 `workflow_version`（资产文件的 SHA256），所以 `--resume` 可以检测资产是否已变更并诚实报告

---

## 方向三：Phase 级确定性回放 —— 从可观测走向可复现

> **类型**: 测试 · 调试 · 可靠性  
> **优先级**: P2（随系统复杂度增长自动升值）  
> **代码影响**: `internal/trace/` · `internal/persist/` · `orchestrator/` · `cmd/forge/` · 测试基础设施

### 现状

ForgeOS 的 trace 系统（`internal/trace/`）是一个优秀的可观测性管道 —— 它记录事件的 `Kind`、`Name`、`Status`、`DurationMs`、`CostUsdMicros`、`Model`。每次运行追加到一个 JSONL 文件。但**这些数据只用于事后观察，不能用于行为复现**：

```go
// trace.go:55-70
type Event struct {
    Seq        int    `json:"seq"`
    Kind       string `json:"kind"`
    Name       string `json:"name"`
    Status     string `json:"status"`
    DurationMs int64  `json:"duration_ms"`
    CostUsdMicros int64 `json:"cost_usd_micros,omitempty"`
    Model      string `json:"model,omitempty"`
    Detail     string `json:"detail,omitempty"`
}
```

当前 `persist/replay_test.go` 的测试只验证 checkpoint 的恢复路径（`Load → decode → assert fields`），**不验证全编排器行为的确定性** —— 没有机制回答「给定相同的输入（workflow + agent 输出），编排器是否产生相同的决策序列？」

### 未被覆盖的证明

- 已有分析中 `"deterministic replay"`、`"reproducible run"`、`"trace replay"` 关键词在 `docs/analysis/seventh-wave-data-realism.md` 中出现，但该分析方向是 **「trace fixture 的测试数据持续更新」** （积累测试数据），不是 **「从 trace 数据回放编排行为的机制」**。
- `nova-expansion-directions-v19.md` 方向五「模拟器/沙箱」提及在沙箱中运行 agent，但焦点是**运行 agent 本身**，不是**回放已有编排决策**。
- 无一分析讨论**trace → 编排器决策的因果链逆向重建**能力。

### 为什么需要

1. **编排器的核心逻辑变得越来越复杂**：`RunFrom` 有 `loopBackTo`（方向一跳转）、`agentOutcome`（reviewer 裁决跳转）、`modeGating`（中枢旋钮跳转/跳过）、`checkAgentBudget`（外部队列守卫） —— 每次 sprint 增加新的分支。**目前没有一种低成本的方式验证「给定相同的输入，v2.3 和 v2.4 的编排器是否做出相同的决策」**。

2. **当前测试覆盖的是「木偶线」而非「决策面」**：每个新功能的测试验证自己的分支（`verdict_loopback_test.go`、`mode_gating_test.go`），但**组合爆炸** —— 当 loop-back + mode-gating + budget-exhausted 同时发生时，没有端到端的回放测试来验证行为是否与预期一致。

3. **一个具体场景**：Sprint 27 的 `yaml2json` 重写导致 workflows 解码行为变化。如果有一个 trace→replay 管道，就可以用 Sprint 26 的生产 trace 回放来验证编排器在新 parser 下是否产生相同的 phase 序列 —— **不需要真 agent，不需要真 claude，只需要 trace 文件**。

### 建议的架构方向

构建 **trace-recorded orchestration replay**，将 trace 从「可观测数据」升级为「可执行的编排记录」：

```
// 概念性设计（非代码，仅示意方向）

type ReplayOracle struct {
    traceEvents []trace.Event      // 从 trace.jsonl 加载
    agentOutputs map[int]string    // phase → agent 标准输出（由 cmd/forge 的 observe 记录到 trace）
    currentIdx  int                 // 当前 replay 位置
}

// RunReplay 驱动编排器，但 agent executor 返回预先记录的输出
// 而不是调用真实 agent。编排器看到的决策路径应与原始运行完全相同。
func RunReplay(eng orchestrator.Engine, wf asset.Workflow, oracle ReplayOracle) error {
    eng.Exec = ReplayExecutor{oracle: oracle}
    // 然后正常调用 eng.Run(wf, mode)
    // 所有 gate 结果、agent 输出、timing 都来自 oracle，而非真实环境
}
```

核心设计原则：
- **纯编排器层回放**：不重放 agent 执行（那是 LLM 的行为，非确定性的），只重放**编排器的决策**（gate 看到什么结果、agent 返回什么输出、budget 是否耗尽）。
- **输入来自 trace**：trace 已记录 gate verdict、agent status、cost——缺少的是 agent 的文本输出（feed-forward 的原料）。需要在 trace 事件中增加一个 `output_snapshot` 字段（或指向外部文件的引用）。
- **断言层**：replay 完成后，可以断言 `phase 序列 == 原始 phase 序列`、`loop-back 次数 == 原始次数`、`收敛判定 == 原始收敛判定`。
- **fixture 积累**：每次真点火运行或重要测试运行，自动保留一个 replay fixture（`trace.jsonl` + 简化的 agent output 快照），形成回归套件。

---

## 方向四：Agent 能力协商式阶段分配 —— 从静态指派到动态路由

> **类型**: 编排 · 路由 · 调度  
> **优先级**: P2（阶段性高价值，但需 `requires_tools` 生态成熟）  
> **代码影响**: `internal/asset/` · `internal/routing/` · `orchestrator/` · `cmd/forge/`

### 现状

ForgeOS 的 phase→agent 映射是**完全静态**的：workflow YAML 声明 `agent: implementer`，编排器就去找 `.agent/agents/implementer.md`，任何时候、任何上下文中都使用同一个 agent。

```yaml
# build.yml
- name: implementer
  agent: implementer          # ← 编译时绑定，永不改变
  requires_tools: [web_search, web_fetch]  # ← 在本仓中硬编码指向 implementer
```

Sprint 30 添加了 `requires_tools` 字段和 `requiresToolsGuard` 函数（`prompt_context.go`），但这**只在运行阶段降级**（提示 agent 标注为 advisory），**不改变化身分配**——同一个 implementer 角色卡仍然被使用，即使它被确认缺乏必要的工具。

### 未被覆盖的证明

- `genuinely-novel-expansion-directions.md` 方向一「凭据注入」焦点是**agent 如何安全获取密钥**，不是**agent 如何动态选择**。
- `high-value-extension-directions-v2.md` 方向二「多模型编排」焦点是**不同 LLM 模型**（Opus/Sonnet/Haiku）的选择，不是**不同 agent 角色/技能卡**的选择。
- `execution-semantic-gaps.md` 方向五「版本演化」焦点是**数据结构向前兼容**，不是 agent 能力的版本化。
- 所有分析中 `"dynamic assignment"`、`"capability negotiation"`、`"skill.match"` 等关键词**零命中**。

### 为什么需要

1. **一个 agent 卡无法覆盖所有场景** —— `implementer.md` 是一个通用编码 agent。对于特定的 phase 类型（如数据库迁移、性能优化、安全修复），可能需要专门的 agent 卡或技能。当前系统没有办法说「这个 phase 需要一个 postgres 专家，而不是通用 implementer」。

2. **`requires_tools` 的引入打开了动态分配的可能** —— 当前 `requires_tools` 只做降级 + 标注。但如果系统有多个 agent 候选（例如 `implementer` + `db-specialist` + `perf-engineer`），编排器可以根据 phase 声明的 `requires_tools` 匹配 agent 卡的 `provides_tools` 来选择最佳候选。

3. **一个具体场景**：`discover.yml` 的 `market-research` 相位声明 `requires_tools: [web_search, web_fetch]`。当前系统总是使用 `researcher.md`。但如果存在一个 `market-analyst.md`（声明 `provides_tools: [web_search, web_fetch, data_visualization]`），编排器应该能够优先选择它。

4. **技能卡的存在使这成为可能** —— 系统已有 9 个 skill 卡（`.agent/skills/`），它们是可复用的能力声明。如果有 `provides_skills` 字段关联 agent 卡和 skill 卡，系统就可以做基于技能的动态分配。

### 建议的架构方向

引入**能力声明 → 需求匹配 → 动态分配**的三阶段机制：

```
// 概念性设计（非代码，仅示意方向）

// Agent 卡增加能力声明
// .agent/agents/implementer.md 新 frontmatter:
// ---
// provides_tools: [shell, git, node]
// provides_skills: [refactor-large-file, clean-architecture, testing]
// provides_expertise: [go, typescript, python, sql]
// ---

// Phase 增加需求声明
// build.yml 的 phase 可以:
// - requires_tools: [web_search]          // 工具需求（已存在）
// - requires_skills: [refactor-large-file] // 技能需求（新增）
// - requires_expertise: [go, sql]         // 专业领域需求（新增）

// CapabilityMatcher 在 phase 分配时考虑这些因素
// - 精确匹配工具需求
// - 优先选择提供所需技能的 agent
// - 在多个候选 agent 之间择优（而非只用默认的卡片）
```

核心设计约束：
- **向后兼容**：无声明的 phase 使用默认 agent 卡（当前行为字节不变）
- **优先级规则**：工具需求 > 技能需求 > 专业领域需求
- **透明报告**：每个 phase 运行时，在 trace 中记录 `selected_agent` 和 `candidate_agents`，以便 operator 理解分配决策
- **与现有路由共存**：模型路由（`TierFor`）在 agent 选定之后应用 —— 先选能力，再选模型

---

## 方向五：跨工作流相位产物的版本一致性与变更检测

> **类型**: 数据完整性 · 编排 · 治理  
> **优先级**: P1（随多阶段自动管线出现立即成为关键缺口）  
> **代码影响**: `internal/converge/` · `internal/prompt/` · `orchestrator/` · `cmd/forge/` · 可选的 `internal/diff/`

### 现状

ForgeOS 的 feed-forward 机制（`phaseOutputLedger`）将上游 phase 的输出传递给下游 phase。当 `planner` emits `task-plan.md` 时，`implementer` 的 prompt 会包含它。但系统对 phase 产物的完整性只有**时间点信任**：

```go
// prompt_context.go — feed-forward 记录机制
type phaseOutputLedger struct {
    outputs map[string]string  // phase name → output text
}

func (l *phaseOutputLedger) record(phase, output string) {
    l.outputs[phase] = output  // 覆盖式存储：只保留最新版本
}
```

**没有版本概念**、**没有变更检测**、**没有一致性验证**。如果：
1. `planner` 在 iteration 1 生成 `task-plan.md`（内容：实现 A、B、C）
2. `implementer` 在 iteration 1 完成 A 和 B
3. 人类在 iteration 1 结束后编辑了 `task-plan.md`，增加了 D 和 E
4. iteration 2 启动，`planner` 重新运行，发出新的 `task-plan.md`（只包含 D 和 E，A 和 B 已完成）
5. `implementer` 在 iteration 2 实现 D —— 但不知道 A 和 B 的代码依赖关系，因为它们的实现细节不在 feed-forward 中

**当前系统：对这种情况既无检测也无保护。** 更微妙的是：
- `prompt.ContextCache` 的 ROADMAP 专门是「必须重新读取」（因为 agent 可写），但其他 phase 产物（task-plan、ADR、设计文档）**没有类似的「版本失谐」检测机制**。
- 如果 `implementer` 在 iteration 1 写了代码，而 `reviewer` 在 iteration 2 审查 —— reviewer 看到的代码是 iteration 1 的产物，但 prompt 中注入的 phase-output 来自 re-ran `planner` 在 iteration 2 的输出 —— **出现了时间错位**。

### 未被覆盖的证明

- `execution-semantic-gaps.md` 方向二「输出协调系统」焦点是**输出格式的规范**（markdown 的标题层级、JSON schema），不是**输出的版本一致性与变更检测**。
- `second-order-architectural-gaps.md` 方向一「知识质量衰减」焦点是**memory 存储层的概念退化**（旧知识不被纠正），不是**跨相位产物的版本一致性**。
- `expansion-core-five.md` 中的"feed-forward 管道"讨论的是数据流的**方向**（谁可以传数据给谁），不是数据流的**版本完整性**。
- 无一分析讨论**「上游产物的版本与下游期望的版本不一致」这一系统性风险**。

### 为什么需要

1. **多迭代 evolve 的本质是增量修改** —— 每次 iteration 都可能修改之前 iteration 产生的文件。如果系统不追踪这些修改，feed-forward 机制会传播过时的信息到新的 agent。

2. **`feeds_forward` 与 `FreshContext` 的交互存在盲区** —— `reviewer` 有 `FreshContext: true`，所以看不到 feed-forward 和 gate 结果。但如果 review.yml 中的 `executive-review`（cto）没有 `FreshContext`，它看到的 gate 结果来自**本次 run 的哪个 gate 调用**？如果 reviewer 跑了但 gate 没跑（mode-gating 跳过了某些 gate），`executive-review` 看到的 gate 结果集是不完整的。**当前系统无法表达「我需要看到某次特定执行的 gate 结果，不是最新的」**。

3. **人类编辑与 agent 产物的冲突是不可避免的** —— 在 evolve 的间隙，人类可能：
   - 编辑 ROADMAP（已知 —— `currentTask` 每次都重新读取）
   - 编辑 task-plan（未知 —— feed-forward 可能覆盖）
   - 编辑 ADR（未知 —— ContextCache 可能返回旧版）
   - 编辑实现代码（未知 —— 下个 agent 可能覆盖）

   没有一个系统层面的**变更通知**机制。

### 建议的架构方向

引入**产物版本标识 + 变更检测 + 一致性断言**：

```
// 概念性设计（非代码，仅示意方向）

// 1. 每个 feed-forward 的产物都带版本标识
type VersionedOutput struct {
    PhaseName   string
    Content     string
    VersionHash string  // SHA256 of content
    Iteration   int     // 产出的 iteration
    CreatedAt   int64   // unix timestamp
}

// 2. 在每次 phase 运行前，检查依赖的产物是否已变更
// 如果上游产物的版本 hash 与 feed-forward 记录的不同：
//   - DETECTED: 记录 warning 到 trace
//   - REPORTED: 注入到 agent prompt（[context:version-drift]）
//   - BUT NOT BLOCKED: 当前不阻止 phase 执行（v1 是通知而非阻断）

// 3. 在收敛信号中增加产物一致性度量
// Signals.ArtifactConsistency: float64  // 0.0-1.0，有多少上游产物仍与原始记录一致
// 收敛判据可以选择要求 ArtifactConsistency >= 1.0
```

核心设计原则：
- **轻量级，非阻断性**：v1 只检测和报告，不阻塞执行。这保持了 agent 的弹性（它可以选择忽略过时的输入依然产生正确的输出）。
- **观察性优先**：变更在 trace 中记录为 `kind: "version_drift"` 事件，operator 可以在事后分析中看到「phase X 运行时，上游产物 Y 已被外部修改」。
- **收敛选项**：未来可以在 `stop_condition` 中增加 `artifact_consistency >= 1.0` 作为收敛条件，使系统在产物不一致时诚实报告 NOT MET。

---

## 优先级汇总

| 方向 | 优先级 | 类别 | 核心价值 | 实现成本估计 | 与已有分析的差异 |
|------|--------|------|----------|------------|----------------|
| **一:信号可靠性分层** | P1 | 架构/收敛 | 诚实性护城河——系统不再假装无法区分客观证据与主观自报 | 中（新增信任模型 + 向后兼容路径）| 零覆盖：信任分层 vs 信号提取 |
| **二:治理资产热加载** | P1 | 运维/运行时 | 24h 自治运行不需要因治理变更而冷重启 | 中（mtime 监控 + 原子状态替换）| 零覆盖：运行时热加载 vs 跨项目同步 |
| **三:Phase 级确定性回放** | P2 | 测试/调试 | 编排器回归测试的范式升级——从木偶线测试到组合回放 | 高（trace 格式扩展 + replay engine + fixture 积累）| 边缘触及：`seventh-wave` 提到 fixture 积累，非编排回放 |
| **四:Agent 能力协商分配** | P2 | 编排/路由 | 从静态 agent 卡到动态能力匹配，释放 skill 卡的真正潜力 | 中-高（能力声明 schema + matcher + 与现有路由共存）| 零覆盖：动态分配 vs 多模型路由/凭据注入 |
| **五:跨相位产物版本一致性** | P1 | 数据完整性 | 多迭代 evolve 的数据管道不传播过时信息 | 中（version hash + 变更检测 + trace / prompt 注入）| 零覆盖：版本一致性 vs 输出格式规范 |

### 收敛建议（如果只做三件）

1. **方向一（信号可靠性分层）** —— 这是诚实性根基。在系统能够`forge accept`并报告 ACCEPTED 时，同时让系统理解它说的是什么级别的"证据"——这是 ForgeOS 诚实性原则的自然延伸。

2. **方向五（产物版本一致性）** —— 这是多迭代数据管道的完整性底座。在系统开始做多阶段自动管线之前（`design.yml → build.yml` 的 `next_stage`），必须先确保跨 iteration 的产物不会传播过时信息。

3. **方向四（动态能力分配）** —— 随着 agent 卡和 skill 卡数量增长，静态分配会成为瓶颈。`requires_tools` 的引入已经打开了这扇门，动态分配是最自然的下一步。

方向二（热加载）和方向三（确定性回放）随 24h 无人值守运行的量产部署和编排器复杂度自然升值——在首次 24h 自治运行前，方向二是关键前提；在编排器分支复杂度超过 20 个独立决策分支前，方向三是预防性投资。
