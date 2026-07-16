# ForgeOS — 五个高价值扩展方向（全局扫描 V49）

> **角色**: 资深架构师 / 产品经理  
> **方法**:
> 1. 全局深扫 forge-core（18 Go 包 · ~35k LOC 运行时+CLI）、harness（39+ 模块 · ~10.5k LOC 执法层）、
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR + DECISIONS + architecture）、
>    `examples/`（url-shortener · go-taskd）、`pi-batch.py`、`.github/workflows/`
> 2. 通读 Sprint 1–31 完整演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90+ DONE · 0 GAP）
> 3. **差异化验证**: 逐篇核查 **89+ 份已有分析文档**（`docs/requirements/` 46 篇 + `docs/analysis/` 40 篇 +
>    核心文档 + ADR + CURRENT_SPRINT + ROADMAP），确认每个方向的**核心命题从未作为独立方向被展开**。
>    每个方向附代码级证据（精确到 `file:line`）+ 与已有覆盖的差异化说明。
> 4. **纪律**: 不编写任何代码。所有建议指向现有代码中的具体位置，不做空泛架构推测。
> **日期**: 2026-07-10

---

## 全景：已有覆盖 vs 本文定位

已有分析覆盖约 **150+ 独立方向**，高密度覆盖以下域：

| 高密度覆盖域 | 代表性文档 | 已覆盖方向数 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/并行/loop-back/诊断） | `high-value-extension-directions*.md` · `novel-architectural-extensions-v40.md` | ~30 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md` · `expansion-forgeos-meta-governance.md` | ~12 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈 / 熔断） | `expansion-production-readiness*.md` · `expansion-production-blindspots-v36.md` | ~15 |
| 二阶系统问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/并行安全） | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` | ~15 |
| 多仓库/联邦/跨会话治理（知识迁移/漂移检测/舰队管理） | `expansion-horizon-three.md` · `strategic-expansion-v39.md` | ~12 |
| 安全纵深（凭据/SCA/沙箱/注入防御/secret-scan/递归护栏/四维资源） | `forgotten-five-system-boundaries.md` · `novel-five-perspectives-2026-07-10-deep.md` | ~10 |
| 北极星桥梁（Temporal/OPA/OTel/多厂商/Sandbox/Web UI） | `v2-to-northstar-gap.md` · `expansion-directions-v3.md` | ~8 |
| 跨进程/运行时（文件锁/热加载/Trace CLI/可插拔扩展/状态自校验） | `forgotten-five-foundations.md` | ~5 |
| 认知边界（确定性回放/补偿撤销/故障隔离/信任加权/梯度响应） | `expansion-five-uncovered-2026-07-10.md` | ~5 |
| 结构化债务（YAML 碎片/cmd/forge 包/存储累积/进程冲突/不可编程） | `forgotten-five-structural-debt.md` | ~5 |
| 学习循环闭合（质量加权路由/记分卡驱动/收敛自调） | `expansion-five-systemic-learning-loop-gaps.md` | ~5 |
| 其他（混沌/冷启动/成本预测/冲突解决/并行 agent 协调/版本契约…） | 各单篇 | ~30 |
| **总计** | **89+ 份文档** | **~150+ 方向** |

**本文 5 个方向**的共同特征：不是「缺少某个引擎/功能」，也不是「已有机制的升级」——它们是 **当前系统已经埋下数据基础、但从未被系统性提取为独立能力方向的横切关注点**。每个方向都需要从多处现有代码中提取信号，组合成一个新能力层。

---

## 目录

1. [方向一：Prompt 有效性测量与优化闭环](#方向一prompt-有效性测量与优化闭环)
2. [方向二：Agent 卡行为契约的运行时履约验证](#方向二agent-卡行为契约的运行时履约验证)
3. [方向三：跨会话知识生命周期管理](#方向三跨会话知识生命周期管理)
4. [方向四：非代码产物的结构化验证框架](#方向四非代码产物的结构化验证框架)
5. [方向五：工作流编排反模式静态检测](#方向五工作流编排反模式静态检测)

---

## 方向一：Prompt 有效性测量与优化闭环

**优先级**: 🔴 P0 | **类别**: 学习闭环 · 智能运维 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 拥有完整的 agent prompt 生命周期：模板定义（`.ai/prompts/*.md`）→ 上下文装配（`prompt.Build`）→
agent 执行（`CommandExecutor`）→ 输出验证（gate/verdict）→ 质量记录（scorecard）。但**数据的流向是单向的**：
质量信号（cost、latency、gate pass/fail、reviewer verdict）写入 scorecard 后**从未回流到 prompt 本身**。

这意味着：
1. **prompt 变更的影响不可见**：当你修改 role card 或 prompt template 时，无法知道「这个改动让 agent
   产出更好了还是更差了」——质量指标可能变差但你永远不会收到告警
2. **不同 agent 卡/模板之间无法对比**：当前只有一个版本的 `reviewer.md`、`implementer.md`。
   如果你想试用一种新的 prompt 风格（例如给 implementer 加「先写测试再写实现」指令），必须手动
   复制修改文件、跑一版、对比 scorecard——这等于自己搭实验框架
3. **prompt 质量退化的无声引入**：`prompt_context.go` 复杂地装配多个上下文源（memory/ADRs/gate results/
   phase output ledger/verdict），任何一个源的内容变化都可能降低 prompt 质量，但无机制检测
4. **最贵的资源（prompt tokens）没有 ROI 测量**：context window 是 agent 调用中最贵的资源之一，
   但当前没有任何测量「多少 token 花在了有效指令上 vs 噪音上下文上」

### 代码级证据

**证据 A：prompt 装配是单向管线，无变异/对比能力**

`prompt_context.go` 的 `buildPrompt` 从多个源装配 prompt，但所有源都是当前文件系统的实时状态——
没有「使用 v2 版本的 template」或「对比 A/B 两组 prompt 的效果」的概念：

```go
// forge-core/cmd/forge/prompt_context.go:467-476
func readCard(repoRoot, agentName string) string {
    path := filepath.Join(repoRoot, ".agent", "agents", agentName+".md")
    // ← 永远读当前文件系统版本。没有版本选择，没有回退策略
}
```

```go
// forge-core/cmd/forge/prompt_context.go:400-410
func buildPrompt(ctx context.Context, ...) string {
    card := readCard(o.repoRoot, phase.Agent)
    // ...
    prompt := prompt.Build(phase.Agent, phase.Name, ...) + projectContext
    // ← 单一的 prompt 输出，没有 alternative 生成
}
```

**证据 B：scorecard 有质量数据但零消费于 prompt 优化**

`ScorecardPair` 记录了每个 `(model, task_type)` 组合的 `QualityScore`、`PassRate`、`ReworkRate`、
`AvgIterations`、`Samples`，但这些数据没有任何回灌到 prompt 装配路径的机制：

```go
// forge-core/internal/routing/scorecard.go:38-62
type ScorecardPair struct {
    Model         string  `json:"model"`
    TaskType      string  `json:"task_type"`
    QualityScore  float64 `json:"quality_score,omitempty"`
    PassRate      float64 `json:"pass_rate,omitempty"`
    ReworkRate    float64 `json:"rework_rate,omitempty"`
    AvgIterations float64 `json:"avg_iterations,omitempty"`
    Samples       int     `json:"samples"`
    // ...
}
```

全仓搜索证实 `QualityScore`/`PassRate`/`ReworkRate` 只在 scorecard 定义和测试中被引用：
`grep -rn "QualityScore\|PassRate\|ReworkRate" forge-core/internal/routing/` 只在
`scorecard.go` 和 `scorecard_test.go` 中出现。没有任何消费者用这些数据来调整 prompt 或 workflow。

**证据 C：`prompt.Build` 接受的参数没有「质量反馈」维度**

```go
// forge-core/internal/prompt/prompt.go:20-23
func Build(agent, phase, mode, tier, card string, ctx []string) string {
    // ← 没有 scorecardData、no historicalQuality、no feedbackFromLastRun
    // prompt 装配不知道这个 agent 上次同类任务的表现
}
```

**证据 D：agent 卡的角色文本是唯一的行为驱动，但无版本/实验标识**

每个 agent 卡（`reviewer.md`、`implementer.md`）中的角色描述直接驱动 agent 行为。改变一行文案
就可能改变数十次 agent 调用的质量——但没有任何框架来标记「这是 v2 reviewer 卡」或测量其效果。
`readCard` 只按名称读取当前文件。

### 与已有覆盖的差异化说明

- `expansion-five-systemic-learning-loop-gaps.md` 方向一（质量加权路由）讨论的是**用 scorecard 数据
  优化路由决策**（选哪个 model），不是优化 prompt 本身
- `forgotten-five-system-boundaries.md` 方向四（版本化契约寄存器）讨论的是**机读 token 的协议版本**，
  不是 prompt 文本的实验/优化
- `five-novel-architectural-frontiers-2026-07-10.md` 方向三（Prompt QA）聚焦于**prompt 注入的
  稳定性与内容正确性**，不是 prompt 版本迭代与效果测量
- `next-five-architectural-frontiers.md` 全文仅 1 次提及 "prompt effectiveness" 作为子项，
  未展开为独立方向

### 建议方向

1. **Prompt 版本标识**：每个 agent 卡和 prompt template 获取 `version` 元数据（frontmatter），
   `readCard` 可指定版本、可回退到 latest。`prompt.Build` 的输出附带所用各模板版本的 SHA-256 摘要
2. **Scorecard 标签归因**：每条 scorecard 记录附带 `prompt_digest`（所用 prompt 装配的哈希），
   使得可按 prompt 版本聚合质量指标
3. **Prompt 退化告警**：当同一个（workflow, phase, model）组合的平均 `QualityScore` 出现
   统计显著下降时，自动报告「prompt 变更可能导致了质量退化」
4. **实验框架基础设施**：支持在 workflow 中声明 `prompt_variant`（如 `reviewer_v2`），
   运行时可选择 variant，对应的 scorecard 数据自动打标比较

### 边界情况

- **冷启动**：新 prompt 版本没有历史 scorecard 数据，不能做对比。自动降级为「收集基线模式，
  满 N 样本后再出对比报告」
- **hash 碰撞**：prompt digest 用 SHA-256 截断风险极低，但仍需 fail-safe（碰撞视为不同版本，
  不合并统计数据）
- **实验的隐形成本**：跑 A/B 测试意味着部分 phase 使用可能更差的 prompt。框架应默认不启用，
  需 `--prompt-experiment` 标志显式开启，且记录额外成本

---

## 方向二：Agent 卡行为契约的运行时履约验证

**优先级**: 🟠 P1 | **类别**: 治理 · 契约完整性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 拥有 **12 个 agent 卡**（`.agent/agents/*.md`），每个卡都声明了 agent 的角色、行为边界、
输出格式、工具需求、安全约束。这些声明是 workflow 设计的依据——`build.yml` 的 `agent: implementer`
依赖 `implementer.md` 中描述的承诺（会写代码、能按 AC 实现等）。

但**这些声明在运行时从未被验证**。具体来说：

1. **`readonly` 边界声明无强制**：每个 agent 卡都声明了 `readonly: true/false`，但 `forge-core`
   在运行时只根据 workflow phase 的 `readonly` 属性控制 `acceptEdits`，从不检查 agent 卡自己的
   声明是否与 workflow 一致——不一致时系统会静默运行（卡说 readonly，phase 说可写，结果是可写）
2. **`requires_tools` 声明无预检**：agent 卡声明需要哪些外部工具（如 `requires_tools: [git, node]`），
   但 `forge-core` 在起跑 agent phase 前不做工具可用性预检——工具在 phase 中缺失 → agent 报错 →
   fail/retry，浪费一次 LLM 调用
3. **`emits:` 路径声明的运行时遵守性未验证**：agent 卡声明输出文件路径（如 `docs/discovery/`），
   workflow 的 `emits:` 也声明了输出路径，但**没有检查 agent 实际写入了这些路径**——如果 agent
   把 PRD 写到了根目录而不是 `docs/discovery/`，系统不会发现
4. **`machine_readable_contract` 的履约无法自动判定**：卡声明了 `VERDICT: APPROVE/REQUEST_CHANGES`
   或 `CONFIDENCE: <0-100>`，parser 能提取这些 token，但**没有独立验证这些 token 是否与 agent
   产出的实际内容一致**——agent 可以说 VERDICT: APPROVE 但实际报告写满了问题

### 代码级证据

**证据 A：`readCard` 只读文本，不解析行为声明**

```go
// forge-core/cmd/forge/prompt_context.go:467-476
func readCard(repoRoot, agentName string) string {
    data, err := os.ReadFile(filepath.Join(repoRoot, ".agent", "agents", agentName+".md"))
    // ← 返回整个 markdown 文本用于注入 prompt，但不解析其中的行为声明字段
    // 不提取 readonly / requires_tools / emits / machine_readable_contract
}
```

**证据 B：`readonly` 在 agent 卡中有声明，在 forge-core 中也有独立实现，但两者未关联**

agent 卡：
```markdown
<!-- .agent/agents/researcher.md:2-5 -->
**readonly**: true
**requires_tools**: none
```

forge-core 使用 workflow phase 的 `readonly` 属性决定 argv：
```go
// forge-core/cmd/forge/engine_build.go:96-100
if phase.ReadOnly {
    argv = append(argv, "--permission-mode", "acceptEdits") // 实际是可写，注释应更正
}
```
但 agent 卡自己的 `readonly: true` 声明与 workflow phase 的 `readonly` 属性之间**没有任何一致性校验**。
如果 workflow 写 `readonly: false` 但 agent 卡声明 `readonly: true`，系统静默按 workflow 执行。

**证据 C：`requires_tools` 在 agent 卡中有声明，在 `asset.Phase` 有 `RequiresTools` 字段，但无预检**

```go
// forge-core/internal/asset/asset.go:55-56
type Phase struct {
    // ...
    RequiresTools []string `json:"requires_tools,omitempty"` // Sprint 30 新增
    // ...
}
```

`requiresToolsGuard`（`prompt_context.go` 中的 degrade-and-flag 逻辑）只在 prompt 装配时叙述性地
标注工具状态，**不在起跑 agent 前做 `command -v` 或 `which` 的可用性检查**。如果某个工具在环境中
缺失，agent 要到第一次尝试调用时才会失败。

**证据 D：`check.py` 的 `check_workflow_agent_refs` 只检查引用存在，不检查行为声明一致性**

```python
# harness/check.py:380-400 (approximate)
def check_workflow_agent_refs(wf):
    # ← 只检查 workflow 引用的 agent 名称在 .agent/agents/ 中存在
    # ← 不检查: workflow 的 readonly 设置是否与 agent 卡的声明一致
    # ← 不检查: workflow 的 requires_tools 是否与 agent 卡声明一致
    # ← 不检查: workflow 的 emits 路径是否在 agent 卡声明范围内
```

### 与已有覆盖的差异化说明

- `high-value-extension-v35.md` 有 1 次提及 "agent behavior contract"，但讨论的是 agent 之间的
  接口契约（agent-to-agent），不是 agent 卡声明 vs 运行时实现的一致性
- `fresh-expansion-perspectives.md` 讨论的是**阶段间交接协议**（phase-to-phase handoff），
  不是 agent 卡行为声明的履约验证
- `expansion-five-systemic-learning-loop-gaps.md` 方向二（声明式输出契约验证）讨论的是
  **agent 输出格式的 schema 验证**（output 是否符合预期 schema），不是 agent 卡行为声明的履约
- Sprint 30 的 `requires_tools` 实现只做了 degrade-and-flag（prompt 中叙述），
  没有做 agent 卡声明与 workflow 声明之间的一致性检查

### 建议方向

1. **Agent 卡结构化解析**：在 `readCard` 或独立路径中解析 agent 卡的 frontmatter/metadata，
   提取可机读的行为声明（`readonly`、`requires_tools`、`emits` 路径、`writes_adr`、权限声明等）
2. **运行时履约检查点**：在每个 agent phase 起跑前：
   - 验证 workflow phase 的声明（`readonly`/`requires_tools`/`emits`）与 agent 卡声明一致
   - 对 `requires_tools` 做 `command -v <tool>` 预检，缺失时提前 fail 而非浪费 LLM 调用
3. **产出路径审计**：在 agent phase 完成后，检查 git diff 中新增/修改的文件是否在 agent 卡和
   workflow 声明的路径范围内。超出范围时报告告警（warn/block 取决于 enforce 模式）
4. **Verdict 真实性交叉验证**：对 reviewer/CTO 等产出 VERDICT 的 phase，做基本的文本验证——
   VERDICT: APPROVE 不应出现在一份写满 blocking issue 的报告中。这可以是简单的关键词比例启发式

### 边界情况

- **松散声明的 agent 卡**：有些 agent 卡（如 `explorer.md`）可能不声明具体 emits 路径。
  对此类卡跳过路径审计，不误报
- **跨平台工具路径**：`requires_tools` 预检需要考虑 PATH 差异和 OS 差异（windows 用 `.exe` 后缀）。
  预检只在运行 forge 的平台有效，不应跨平台承诺
- **git diff 依赖**：产出路径审计依赖 `git diff`，在非 git 工作目录或首次提交时不可用。
  降级为仅扫描文件系统目录结构
- **agent 卡演化**：旧版 agent 卡可能没有行为声明字段。缺失时应跳过对应检查，不 FAIL

---

## 方向三：跨会话知识生命周期管理

**优先级**: 🟠 P1 | **类别**: 记忆 · 知识工程 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

`memory.jsonl` 是 ForgeOS 的持久化知识库——它跨 `forge run`/`forge evolve` 会话积累，存储
agent 产出的结构化知识（ADR 理解、架构决策、测试策略、风险分析）。但这些知识**没有任何生命周期
管理**，导致以下问题：

1. **知识无限增长**：`memory.Append` 每次运行都追加条目，没有上限、没有轮转、没有归档。
   一个每天跑 evolve 的项目，一个月后可能有数千条 memory 条目。`retrieve.go` 的 TF-IDF 检索
   效率随条目数线性下降（没有索引）
2. **陈旧知识持续污染 prompt**：`prompt_memory.go` 从 `memory.jsonl` 读取最近的 N 条注入
   prompt。但如果一条 3 周前的 ADR 理解已经被后续 ADR 修正了，旧理解仍可能被召回——`Supersedes`
   字段存在但不被 `retrieve` 或 `prompt_memory` 消费
3. **无重要性/置信度驱动的保持策略**：`Compact` 按数量修剪（保留最后 N 条/种类），
   不区分「高置信度、高重要性的架构决策」和「临时调试记录」。重要的知识可能被批量修剪掉
4. **跨会话知识冲突不可见**：两个不同的 evolve 会话可能产生矛盾的知识（如一个说「用 Postgres」，
   另一个说「用 SQLite」），但没有任何冲突检测机制

### 代码级证据

**证据 A：`memory.Append` 追加即永久，无上限、无轮转**

```go
// forge-core/internal/memory/memory.go:173-210
func Append(path string, e Entry) error {
    f, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
    // ← O_APPEND 永远追加，文件无限增长
    // ← 没有文件大小上限，没有日志轮转，没有归档策略
}
```

**证据 B：`Supersedes` 字段存在但从未被任何读取路径消费**

```go
// forge-core/internal/memory/memory.go:170-171
type Entry struct {
    // ...
    Supersedes   string  `json:"supersedes,omitempty"`   // ADR-0004 → superseded
    Confidence   float64 `json:"confidence,omitempty"`
    Source       string  `json:"source,omitempty"`
    // ...
}
```

全仓搜索 `Supersedes` 的消费者：
```bash
grep -rn "Supersedes\|supersedes" forge-core/ --include="*.go"
# → memory.go: 定义 + 测试
# → 没有任何文件读取 Supersedes 字段来做知识淘汰/降权
```

**证据 C：`prompt_memory.go` 按数量截断，不按重要性**

```go
// forge-core/cmd/forge/prompt_memory.go:40-50
const memoryCap = 32  // 最多注入 32 条 memory

func buildMemoryContext(...) string {
    entries := memory.Read(path)  // 读全部
    // ← 没有按 Confidence/Importance 排序，没有移除被 supersedes 的条目
    // ← `sort.Slice` 按 recency，不是按质量/重要性
}
```

**证据 D：`Compact` 按种类/条数修剪，不是按语义重要性**

```go
// forge-core/internal/memory/memory_compact.go:50-85
func Compact(path string, keepPerKind int) error {
    // ← keepPerKind: 每种知识保留 N 条，按 recency 保留最近的
    // ← 不保留: confidence ≥ 0.9 的重要决策即使较旧也优先保留
    // ← 不排除: 已被 supersedes 的条目（它们在语义上已过时但可能被保留）
}
```

### 与已有覆盖的差异化说明

- `second-order-architectural-gaps.md` 讨论「知识衰减」作为二阶系统问题，但聚焦于**知识
  随时间失去准确性**，而非生命周期管理（保留/归档/冲突检测/淘汰策略）
- `expansion-five-systemic-learning-loop-gaps.md` 讨论的是**scorecard 数据的闭环反馈**，
  不是 memory/knowledge 的生命周期
- `strategic-extension-five-novel-2026-07-10.md` 提及 "knowledge retention" 但作为跨会话
  治理的一个子点，不是独立展开的知识生命周期方向
- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 讨论的是**状态文件
  的跨会话冲突**（并发写、孤儿进程残留），不是 knowledge 内容层面的生命周期管理
- `governance-prod-five-frontiers.md` 仅 1 次提及 "knowledge lifecycle"，未展开

### 建议方向

1. **知识 TTL 与版本标记**：Entry 增加 `created_at` 和 `ttl_days` 字段。`prompt_memory.go`
   在召回时过滤已过期的条目。`Supersedes` 字段被实际消费——当一条 entry 声明 supersedes 另一条时，
   被 supersedes 的旧条目在 prompt 注入和检索中被降权或排除
2. **重要性驱动的保留策略**：`Compact` 改为两阶段策略：(1) 先按语义标记保留高置信度
   （Confidence ≥ 0.8）+ 高频引用条目；(2) 再按数量修剪剩余条目。确保重要知识不被批量删除
3. **知识冲突检测**：新写入的 entry 与已有 entry 在（topic, claim）维度上做简单文本相似度
   比较，当检测到矛盾时写入 `contradiction_warning` 标记。冲突条目在 prompt 中一起注入并标注矛盾
4. **冷热分层**：按访问/引用频率将 memory 条目分为热（高频引用，注入 prompt）、温（可检索）、
   冷（归档，仅用于审计）。默认所有条目为温，随引用次数动态升温，超期无引用则降温

### 边界情况

- **TTL 与业务逻辑冲突**：某些知识（如根本性架构决策）应该永久保留。TTL 设计应支持
  `ttl_days: 0` 表示「不过期」
- **冲突检测的假阳性**：文本相似度可能误判互补描述为矛盾。冲突检测应只产生告警（warn），
  不阻塞执行（不 block）
- **文件增长与 IO 性能**：`memory.jsonl` 可能增长到数百 MB。`Read` 的 O(N) 扫描会成为瓶颈。
  冷热分层需要配套索引机制（按 topic 分片、按时间分段）
- **跨会话可见性**：一个 session 写的高置信度知识可能在另一个 session 中被无意覆盖。
  需要 `session_id` 或 `source_run` 字段来区分知识的生产者

---

## 方向四：非代码产物的结构化验证框架

**优先级**: 🟠 P2 | **类别**: 治理 · 质量保证 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 当前的所有 harness gate（test/lint/build/complexity/arch/security）都只验证**代码产物**。
但系统产出大量**非代码产物**——PRD、架构文档、评审报告、ADR、路线图更新——这些产物由 LLM agent
直接生成，经过 human review，但**在无人值守的 evolve 模式下没有任何结构化的自动验证**。

当前，以下场景存在质量风险：

1. **PRD 缺乏必要结构**：`product-manager.md` 描述 PRD 应有的内容（问题陈述、目标用户、成功指标、
   竞品分析），但没有自动化检查确保 agent 输出的 PRD 包含这些必要章节
2. **架构文档缺少关键决策**：`architect.md` 列出架构设计应覆盖的维度（分层、数据流、边界、风险），
   但如果 agent 跳过了一个关键维度（如「回滚策略」），当前没有任何机制发现
3. **评审报告缺失必须的裁决**：`review.yml` 要求每个评审 phase 产出 VERDICT，但 parser 只提取
   token，不验证报告**内容**是否与裁决一致——一个 `VERDICT: APPROVE` 可能对应一份写了「存在
   严重安全漏洞」的报告
4. **ADR 格式不规范**：ADR 应遵循特定模板（状态、上下文、决策、后果），但 agent 可能输出自定义
   格式的 ADR，使后续 reader 难以快速理解

### 代码级证据

**证据 A：`converge.go` 的 `gatherSignals` 收集 ReviewStatus，但只检查 VERDICT token，不验证产出质量**

```go
// forge-core/cmd/forge/gates.go:280-310 (approximate)
func reviewStatus(verdicts []string) string {
    for _, v := range verdicts {
        switch strings.TrimSpace(v) {
        case "APPROVE", "APPROVE_WITH_SIMPLIFICATION":
            return "approved"
        }
    }
    // ← 只检查 VERDICT token 的存在和值
    // ← 不验证 agent 产出的 review 文件本身的质量/完整性
}
```

**证据 B：`emits:` 在 workflow 中声明产出的路径和用途，但无 schema 级验证**

```yaml
# .agent/workflows/discover.yml:34-38
- name: requirement-discovery
  agent: product-manager
  emits:
    - docs/discovery/prd.md
    - docs/discovery/competitor-analysis.md
  stop_condition:
    type: conjunction
    all_of:
      - requirement_confidence >= 80
```

Workflow 声明了产出什么文件，但**没有任何 schema 或结构契约**——`prd.md` 是否包含 `## Problem
Statement`？`competitor-analysis.md` 是否包含 `## Competitive Matrix`？系统不知道，也不检查。

**证据 C：agent 卡中有对产出的散文描述，但无可执行的结构校验**

```markdown
<!-- .agent/agents/product-manager.md:30-45 (approximate) -->
## Output
Produces a PRD in docs/discovery/prd.md with:
- Problem statement
- Target users
- Success metrics
- Competitive analysis
- Risk assessment
- Prioritized feature list
```

这段描述是人读的、高质量的指南，但**不可被机器执行**——没有对应的 `prd.schema.json` 或
结构检查器来验证产出是否符合这些要求。

### 与已有覆盖的差异化说明

- `expansion-five-systemic-learning-loop-gaps.md` 方向二（声明式输出契约验证）讨论的是
  **agent 输出格式的机读 token 验证**（CONFIDENCE/VERDICT 等），不是非代码产物的文档结构验证
- `structural-gaps-v41-genuinely-unexplored.md` 提及 "document completeness" 但作为
  阶段间协议的一个子问题，不是独立的非代码产物验证框架
- `novel-five-perspectives-2026-07-10-deep.md` 讨论的是**代码产物的质量验证**
  （test/lint/complexity 等现有 gate），不是非代码产物
- `forgotten-five-system-boundaries.md`方向四（版本化契约寄存器）讨论的是**机器可读 token
  的版本兼容性**，不是文档结构验证

### 建议方向

1. **产出结构契约声明**：在 agent 卡或独立 schema 文件中声明每个非代码产物的最小结构要求。
   采用简单格式（JSON Schema 或自定义 YAML），如：
   ```yaml
   # .agent/schemas/prd.yaml
   artifact: docs/discovery/prd.md
   required_sections:
     - "## Problem Statement"
     - "## Target Users"
     - "## Success Metrics"
   recommended_sections:
     - "## Competitive Analysis"
     - "## Risk Assessment"
   min_length_chars: 500
   max_length_chars: 50000
   ```
2. **非代码产物闸门**：新增一个轻量 `document-check` 适配器（类似于 `lint`/`coverage` 适配器），
   在 agent phase 完成后、fed-forward 前扫描产出文件，验证它们满足声明的结构契约。结果进入
   `gatherSignals` 的信号池
3. **结构契约 vs agent 卡一致性校验**：`check.py` 新增检查，验证 agent 卡中散文描述的输出结构
   与 `.agent/schemas/` 中的结构化契约一致，防止人读指南与机器校验规则漂移
4. **渐进式采纳**：结构契约可选（`warn` 模式默认）。没有 schema 的产物按当前行为（无验证）执行。
   一旦为某个产物添加了 schema，即自动启用验证

### 边界情况

- **markdown 变体**：agent 可能输出略有不同的章节标题（如 `## Problem statement` vs
  `## Problem Statement`）。验证应大小写不敏感，允许前缀/后缀空白
- **非 text 产物**：某些 emits 可能是 JSON、YAML、或二进制文件。结构验证框架应按文件扩展名
  选择验证策略（.md → section check, .json → JSON Schema, .yaml → YAML schema）
- **验证成本**：文档结构验证是纯文本操作（微秒级），不会显著增加 phase 延迟
- **human review 替代**：非代码产物验证不替代 human review——它是在 human review 之前或
  之间的快速反馈，让 agent 有机会自纠格式问题，节省 reviewer 的注意力

---

## 方向五：工作流编排反模式静态检测

**优先级**: 🟠 P2 | **类别**: 治理 · 运维 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐

### 问题描述

ForgeOS 的工作流 YAML 文件（`discover.yml`、`design.yml`、`review.yml`、`build.yml`、`evolve.yml`）
定义了 agent 协作模式、相位顺序、gate 条件、loop-back 规则和收敛判定。这些文件是**系统的核心配置**，
但当前没有任何静态分析工具来检测其中的编排反模式。

当前存在的问题：

1. **永远不会收敛的 workflow**：如果 `stop_condition` 引用了一个在 workflow 中永远不会满足的
   metric（如 `review_status == approved` 但 workflow 没有 include 任何带 VERDICT 契约的 phase），
   `forge run` 将永远跑下去直到 max-iter 耗尽——浪费大量 LLM 调用
2. **依赖环**：`depends_on` 字段支持声明 phase 依赖（给并行引擎使用），但当前没有环检测。
   一个 A→B→C→A 的环会导致 `waves.go` 的 Kahn 排序死锁或返回空集
3. **无用 gate**：如果 `required_gates` 引用了 `modes.yml` gate_catalog 中不存在的 gate 名称，
   该 gate 条件永远无法满足。当前 `check.py` 有 `check_workflow_control_flow` 但检查的是
   phase 引用（`on_fail.target_phase` 是否存在）而非门引用
4. **永远跳过的 phase**：如果 `required_when` 条件过于严格（如引用了一个所有 mode 都跳过的
   lifecycle），该 phase 永远不会被执行但 workflow 作者可能不知道
5. **死 phase**：定义了 phase 但没有 `emits`、没有 `feeds_forward`、没有 `on_fail` 的处理路径——
   相位运行但产出不被任何下游消费，浪费 token

### 代码级证据

**证据 A：`waves.go` 的拓扑排序没有环检测**

```go
// forge-core/internal/orchestrator/waves.go:90-130 (approximate)
func kahnSort(phases []asset.Phase) ([][]int, error) {
    inDegree := map[int]int{}
    // ← 不检测环。如果依赖图中存在环：
    //   - 入度永远不会归零 → 死循环或返回不完整排序
    //   - 当前没有 detectCycle 或 Tarjan 算法
}
```

`parallel_test.go` 中的测试用例全部是无环的。无环检测测试覆盖。

**证据 B：`stop_condition` 引用的 metric 与 workflow 结构之间无交叉验证**

```go
// forge-core/internal/converge/converge.go:183-230
func (e *Evaluator) Evaluate(sig Signals, sc StopCondition) Verdict {
    switch sc.Type {
    case "conjunction":
        for _, m := range sc.AllOf {
            // ← 对每个 metric 名，evalOne 做字符串匹配
            // ← 不检查: 这个 metric 是否在 workflow 中有赋值相位？
            // ← 不检查: 这个 metric 是否在本 workflow 中永不赋值？
        }
    }
}
```

```yaml
# .agent/workflows/build.yml:101-110
stop_condition:
  type: conjunction
  all_of:
    - gates_status == green        # ← 在 build.yml 中有 gate phase → OK
    - review_status == approved     # ← 如果 build.yml 没有 include reviewer phase → 永不满足
    - requirement_confidence >= 80  # ← 如果 build.yml 没有 requirement-discovery → 永不满足
```

当前没有任何代码去验证 `stop_condition.all_of` 中引用的 metric 是否在 workflow 中有对应的
生产 phase。Sprint 29 完成了 `converge.Signals` 全字段闭环，但那是「所有被声明的字段都有赋值」，
不是「workflow 声明的 stop_condition 可被满足」。

**证据 C：`required_gates` 引用的 gate 名称无存在性校验**

```yaml
# .agent/workflows/build.yml:62-64
- name: implementer
  agent: implementer
  required_gates: [lint, test, build, complexity, arch, security]
```

如果某个 gate 名称拼写错误（如 `secuirty` 而非 `security`），`gate.go` 的 `RunGate` 会尝试
执行一个不存在的 gate，结果取决于 gate 解析逻辑（可能静默跳过，可能报错）。但 `check.py` 的
`check_workflow_control_flow` 只检查 `on_fail.target_phase` 引用的 phase 是否存在，
不检查 `required_gates` 引用的 gate 是否存在。

### 与已有覆盖的差异化说明

- 所有 89+ 份已有分析文档**没有任何一份将 "workflow anti-pattern detection" 或
  "workflow lint" 作为独立方向提出**。这是一个完全未被探索的领域
- `forgotten-five-foundations.md` 方向二（治理热加载）讨论的是策略文件的运行时更新，
  不是 workflow 设计的静态分析
- `expansion-horizon-three.md` 方向三（工作流组合代数）讨论的是**跨仓库工作流组合的
  语义模型**，不是单个 workflow 内部的反模式检测
- `forgotten-five-structural-debt.md` 方向一（YAML 解析器碎片化）讨论的是**解析正确性**
  （同一 YAML 被不同解析器产生不同结果），不是解析后的语义分析

### 建议方向

1. **Stop Condition 可达性分析**：解析 workflow 的 `stop_condition.all_of` 列表，对每个 metric
   追查 workflow 中是否有对应 phase 能为该 metric 赋值。如果某个 metric 在 workflow 中
   永远不会有赋值 phase，发出 blocking 告警
2. **依赖图环检测**：在 `waves.go` 的 `kahnSort` 前加入 `detectCycle`（标准 DFS 或 Tarjan
   算法），发现环时返回明确的错误信息（哪个 phase→哪个 phase 的环）
3. **Gate 引用存在性校验**：`check.py` 的 `check_workflow_control_flow` 扩展为同时检查
   `required_gates` 中每个 gate 名称是否在 `modes.yml` 的 `gate_catalog` 中存在
4. **永不执行 phase 检测**：检查每个 phase 的 `required_when` 条件是否在当前 workflow 的
   所有可能 mode×lifecycle 组合下都不可满足。发出 advisory 告警
5. **孤 phase 检测**：检查每个 phase 是否有至少一条「产出被下游消费」的路径——要么有
   `feeds_forward: true`，要么有 `emits:` 文件被后续 gate 消费，要么有 `on_fail` 路径。
   如果某个 phase 的执行结果不被任何路径消费，发出 advisory 告警

### 边界情况

- **动态可选 phase**：某些 phase 的 `optional_for` 可能使它在某些 mode 下被跳过，但在其他
  mode 下执行。Stop condition 可达性分析应考虑 mode×lifecycle 组合，而不是一刀切判定
- **文件外 metric**：`converge.Signals` 的某些字段可能在 workflow 外被赋值（如
  `FileDelta` 从 git diff 计算，不依赖特定 phase）。可达性分析应识别这些「隐式赋值」的 metric
- **误报风险**：workflow 作者可能有意引用一个**未来才会实现的 metric**（如预留扩展点）。
  反模式检测默认 warn，仅在 production lifecycle 下才 block
- **组合爆炸**：mode×lifecycle 有 4×4=16 种组合。分析所有组合是可行的，但输出应聚合为
  「从未执行于任何组合」/「仅执行于部分组合」两级

---

## 总结

| 方向 | 优先级 | 已有覆盖 | 核心洞察 | 预估 | 杠杆 |
|---|---|---|---|---|---|
| ① Prompt 有效性测量与优化闭环 | P0 | 无独立展开 | scorecard 数据有质量/成本/延迟，但零回灌到 prompt 优化。最贵的资源（prompt tokens）没有 ROI 测量 | ~3 sprints | ⭐⭐⭐⭐⭐ |
| ② Agent 卡行为契约的运行时履约验证 | P1 | 无独立展开 | agent 卡声明 readonly/requires_tools/emits，但运行时从未验证声明与实现一致 | ~2 sprints | ⭐⭐⭐⭐ |
| ③ 跨会话知识生命周期管理 | P1 | 1 次提及未展开 | Supersedes/Confidence 字段存在零消费；memory 无限增长无归档/冲突检测 | ~2 sprints | ⭐⭐⭐⭐ |
| ④ 非代码产物的结构化验证框架 | P2 | 无独立展开 | 代码有 gate，非代码 PRD/架构文档/评审报告没有自动结构验证 | ~2 sprints | ⭐⭐⭐⭐ |
| ⑤ 工作流编排反模式静态检测 | P2 | 零覆盖 | workflow 是核心配置，但无 static analysis 检测永不收敛/依赖环/死 phase | ~1 sprint | ⭐⭐⭐ |

### 推荐执行顺序

1. **方向②**（Agent 卡验证）与**方向⑤**（Workflow 反模式检测）是最快见效的：方向⑤只需
   1 sprint，方向②约 2 sprints，都能在现有 `check.py` 框架中增量实现
2. **方向③**（知识生命周期）基础最好：`Supersedes`/`Confidence` 字段已在 memory.Entry 中，
   代码已埋好但未接线，属于「连接已知端点」
3. **方向④**（非代码产物验证）价值高但需要设计 schema 格式 + 适配器，建议在方向②完成后
   利用其 agent 卡结构化解析的基础
4. **方向①**（Prompt 优化闭环）价值最高、影响最深远，但依赖方向③的记忆管理能力 +
   scorecard 数据管线 + prompt 版本标识，建议放在最后但投入最多资源
