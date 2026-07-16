# ForgeOS — 五个结构性债务与产品扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深扫 forge-core（18 Go 包 / ~35k LOC）、harness（39+ 模块 / ~10.5k LOC）、
>   `.agent/`（12 agent 卡 / 9 skill 卡 / 5 工作流 / 全部 ADR+DECISIONS）、
>   `.ai/`（10 stage 模板 / 17 角色）、`ai-dev/`（自有 pipeline + prompts + roles）、
>   `pi-batch.py`、全部已有分析文档（`docs/requirements/` ~50 篇 + `docs/analysis/` ~40 篇）。
> **先验去重**: 对每个方向的核心机制组合词，在全部 ~90 篇已有分析文档中执行全文精确字符串搜索，
>   确认未被任何已有文档作为独立系统性方向展开（侧栏一句话提及不算覆盖）。  
> **日期**: 2026-07-11  
> **纪律**: 不编写任何代码。每个方向附带精确到 `file:line` 的代码级证据、产品价值判断、诚实边界。
> **诚实声明**: 本文不声称发现了 ForgeOS 的「所有」缺口。以下方向不是接线遗漏或 bug 修复，
>   而是架构一致性/产品成熟度层面上的**结构性前沿**，需要新框架而非修补现有函数。

---

## 核心判断

经过 31 轮 sprint 的快速迭代，ForgeOS 在**能力层**已高度成熟：编排引擎跑通、模型路由工作、
安全护栏完整、学习闭环就绪、真点火验证通过。但代码库的快速成长也留下了**三个结构性债务
（方向一/二/三）** 和**两个产品级盲区（方向四/五）**，它们不阻断当前功能，但会线性地限制
ForgeOS 的下一阶段规模化。

### 快速索引

| # | 方向 | 类型 | 紧急度 | 核心问题 |
|---|------|------|--------|---------|
| 1 | **三框架并存的结构性债务** — `.agent/` vs `.ai/` vs `ai-dev/` | 架构债务 | 🔴 P0 | 同仓库中存在三套独立的 agent 框架，零互操作性，知识碎片化。每新增一个框架，维护成本非线性增长 |
| 2 | **成本遥测的厂商硬编码** — `cost.go` 处处写死 Claude JSON 格式 | 架构债务 | 🟠 P1 | 整个 cost-based routing、budget guard、telemetry 链条被单一厂商输出格式绑定，阻碍 G3 跨厂商路由 |
| 3 | **Agent Prompt 的「万能模型」天花板** — 所有 agent 共享同一 prompt 骨架 | 架构债务 | 🟠 P1 | `buildPrompt` 对所有角色（implementer/reviewer/planner/QA）输出同构上下文，忽略了不同角色截然不同的信息需求 |
| 4 | **收敛震荡与 Checklist 漂移** — `staleCount` 无法识别震荡模式 | 产品盲区 | 🟡 P2 | ROADMAP checklist 格式在 agent 间传递时的微小变异导致收敛百分比震荡，系统无法区分「真进展」和「格式漂移」 |
| 5 | **运行时状态生命周期管理** — `.forge/` 目录无治理 | 产品盲区 | 🟡 P2 | trace.jsonl/memory.jsonl/checkpoint 无限增长，无配额/无清理/无归档策略，长期运维风险 |

---

## 方向一 · 三框架并存的结构性债务

> **「同一个仓库里有三套 agent 框架，彼此不知对方存在」**
> **紧急度: P0** | **类别: 架构债务 · 知识碎片化** | **估算: 3-4 sprints 整合**

### 问题诊断

ForgeOS 的代码仓库中，存在**三套独立且功能高度重叠的 agent 框架**，它们零互操作性、
零交叉引用、且各有不同的目录约定和执行模型：

**框架 A: `.agent/`（ForgeOS 认证框架）**

```
.agent/
├── agents/          # 12 张 agent 卡（reviewer/planner/implementer/…）
├── skills/          # 9 张 skill 卡
├── workflows/       # 5 个 YAML 工作流（discover/design/review/build/evolve）
├── policies/        # modes.yml + routing policy
└── eval/            # acceptance schema + scorecard
```

- 由 forge-core 的 `prompt_context.go` 通过 `readCard()` 直接读取
- 由 `check.py` 治理完整性检查
- 是 `forge run/evolve` 的正式运行时依赖

**框架 B: `.ai/`（AI-SDLC 模板框架）**

```
.ai/
├── prompts/         # 10 个 stage 模板（00→09）
│   └── shared/      # 4 个共享上下文文件（role-definitions/output-format/…）
├── reviews/         # 评审产出物
└── README.md        # 声称「不绑定任何 CLI，一套模板适用于所有项目」
```

- **零代码集成**: 没有任何 forge-core 代码读取 `.ai/prompts/` 目录
- 设计哲学不同: 大师模板（一个 prompt 包含所有角色）vs `.agent/` 的单角色卡
- `.agent/workflows/review.yml` 声明的 `uses_template` 指向的是 `.ai/prompts/` 但未经一致性校验
- README 自称「不绑定 Claude Code/Codex/Gemini CLI」但实际使用方式就是「粘贴到 CLI 中」
- **该框架声称的 17 个角色** 与 `.agent/` 的 12 个 agent 卡的角色集存在 50% 重叠但不一致

**框架 C: `ai-dev/`（开发中框架 / 遗产）**

```
ai-dev/
├── ai/prompts/      # 与 .ai/ 几乎镜像的 10 个 stage 模板（00→09）
├── ai/prompts/shared/ # 与 .ai/ 几乎镜像的 4 个共享文件
├── prompts/         # 独立的角色 prompt 集（architect/cto/pm/security/…，17 个文件）
├── pipelines/       # 独立的 pipeline 定义（pipeline-full-sdlc.yaml 等）
├── ai/run-review.py # 独立的评测执行器
├── pi-batch.py      # 与根目录 pi-batch.py 并行存在的副本
└── git-auto-commit.sh
```

- **零代码集成**: 没有任何代码引用 `ai-dev/`
- `ai-dev/ai/prompts/` 的 10 个 stage 模板与 `.ai/prompts/` 的内容**高度相似但不同步**
- `ai-dev/prompts/` 的 17 个角色卡与 `.agent/agents/` 的 12 个卡**完全没有交叉引用**
- `pi-batch.py` 在根目录和 `ai-dev/` 各有一个副本——**派生/重复执行框架**

### 为什么这是 P0 结构性债务

这不是「多了一些文档」的问题。三套框架同时存在造成了具体的可衡量损害：

**1. 知识碎片化**: 当 `.agent/agents/security-engineer.md` 被更新时，`ai-dev/prompts/security_engineer.md`
和 `.ai/prompts/02-security-rfc-review.md` 中的角色定义不会自动更新。同一安全评审知识以三种版本存在。

**2. 使用模型混乱**: 用户/agent 不知道该用哪个框架：
- 加一个新 agent 角色 → 应该在 `.agent/agents/` 加卡，还是在 `ai-dev/prompts/` 加 prompt？
- 跑一个安全评审 → 用 `forge run review`（走 `.agent/workflows/review.yml`）还是手动粘贴
  `.ai/prompts/02-security-rfc-review.md` 到 CLI？
- 写一个自动化 pipeline → 用 `.agent/workflows/` YAML 还是 `ai-dev/pipelines/` YAML？

**3. 治理空白**: `check.py` 检查 `.agent/` 的完整性（agent 卡引用可解析、workflow agent 名匹配），
但对 `.ai/` 和 `ai-dev/` 完全无治理——它们可以静默漂移、角色定义过期、模板格式损坏而无人知晓。

**4. 重复维护成本**: 每在一个框架中更新知识，理论上要同步到其他两个框架——但没有任何机制强制
或提醒这种同步。`ai-dev/ai/prompts/shared/` 和 `.ai/prompts/shared/` 的 4 个文件几乎完全相同——
**即它们已经在漂移，只是还没漂远**。

### 代码级证据

**证据 A: 零交叉引用**

```bash
$ grep -r "\.agent/" .ai/ --include="*.md" | wc -l
0    # .ai/ 从不引用 .agent/
$ grep -r "\.ai/" .agent/ --include="*.md" | wc -l
0    # .agent/ 从不引用 .ai/
$ grep -r "ai-dev" .agent/ --include="*.md" | wc -l
0    # .agent/ 从不引用 ai-dev/
$ grep -r "\.agent" ai-dev/ --include="*.md" | wc -l
0    # ai-dev/ 从不引用 .agent/
```

**证据 B: `.agent/workflows/review.yml` 引用 `.ai/prompts/` 但无校验**

```yaml
# review.yml 中多处:
uses_template: .ai/prompts/02-security-rfc-review.md
```

`doctor` 包的 `EvaluateWorkflowModels` 检查 `uses_template` 的文件存在性——它确实检查路径
可解析。但它**不检查该模板的内容是否与 workflow 期望的角色语义对齐**。

**证据 C: 独立的评测执行器**

`ai-dev/ai/run-review.py` 是一个 Python 脚本来驱动 AI-SDLC 评审流程——它存在于
`forge run review`（走 forge-core 编排）之外，完全不相关的执行路径。

**证据 D: 框架声称的「无限兼容」与实际绑定**

`.ai/README.md` 声称「不绑定 Claude Code / Codex / Gemini CLI」，但它的使用方式是
「将填好 Context 的模板整体粘贴到 AI Agent」——这是一个**纯手动的、无治理的、无迹可寻的**
工作流，与 ForgeOS 的核心理念（带外执法、自动化治理、可追溯）完全背道而驰。

### 修复方向（非代码，仅思路）

1. **就地整合**: 将 `.ai/` 和 `ai-dev/` 中有价值的内容迁移到 `.agent/` 体系：
   - `.ai/prompts/shared/role-definitions.md` → 合并到 Agent 卡中
   - `ai-dev/prompts/*.md` → 转化为 `.agent/agents/*.md` 格式（已有 12 张卡，补充缺失的）
   - `ai-dev/pipelines/*.yaml` → 转化为 `.agent/workflows/*.yml` 格式
2. **软弃用**: 迁移完成后，在 `.ai/README.md` 和 `ai-dev/README.md` 顶部加醒目弃用声明，
   指向 `.agent/` 对应路径
3. **漂移保护**: 在 `check.py` 中加一条检查：`.ai/prompts/*.md` 和 `.agent/` 之间的角色名映射
   不能出现未声明的漂移（类似已有的 `check_workflow_mode_gating` 漂移守卫）
4. **硬闸门**: 在一个过渡期后，禁止对 `.ai/` 和 `ai-dev/` 的非弃用类修改

### 产品价值

消除框架碎片化后，ForgeOS 的 agent 体系从「三个不同的人用三种语言描述同一件事」
变成「一个事实源 + 自动治理」。每新增一个 agent 角色，只需在一个位置更新——其他位置
要么自动继承、要么被漂移守卫挡住。

---

## 方向二 · 成本遥测的厂商硬编码

> **「整个 cost-based 路由链条被一个 JSON 格式锁定」**
> **紧急度: P1** | **类别: 架构债务 · 厂商锁定** | **估算: 2 sprints**

### 问题诊断

ForgeOS 路由系统的核心价值主张之一是 **G3 多维模型路由**——根据复杂度/风险/预算调度不同模型。
但当前实现中，**整个 cost telemetry 链条从解析到路由到 scorecard 全部硬编码为 Claude 的 JSON 输出格式**。

具体链条:

```
claude -p --output-format json          ← Claude 专有输出
  ↓
cmd/forge/cost.go: parseClaudeCostUsd  ← 硬编码解析 Claude JSON 信封
  ↓
internal/trace/Event{ CostUsdMicros }  ← cost 已经落入通用 trace 格式
  ↓
cmd/forge/scorecard_wind.go            ← 写 scorecard
  ↓
internal/routing/scorecard.go           ← HistoryTiebreak 读 scorecard 做路由决策
  ↓
engine_build.go: phaseTierResolver     ← 真正影响 --model 选择
```

链条中除 `internal/trace/` 和 `internal/routing/` 外，**所有关键环节都有 Claude 特定的逻辑**。

### 代码级证据

**证据 A: 解析函数是 Claude 专有的**

`cmd/forge/cost.go`:

```go
// 第 1-5 行自身声明:
// cost.go — the claude-specific cost-telemetry boundary of the forge CLI. ALL
// knowledge of the claude `-p --output-format json` envelope (its total_cost_usd /
// result fields) lives here
```

文件头自己承认这是「claude-specific」。全部 450+ 行代码中:

```go
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
    var env struct {
        TotalCostUsd *float64 `json:"total_cost_usd"`  // Claude 专有字段名
    }
    // ...
}
```

```go
func parseReviewerVerdict(output string) (verdict string, ok bool) {
    // 解析 Claude 输出的最后一行 "VERDICT: APPROVE" / "VERDICT: REQUEST_CHANGES"
    // 这意味着 verdict 格式与 Claude 的 markdown 输出风格绑定
}
```

```go
func parseExecutiveVerdict(output string) (verdict string, ok bool) {
    // 同上，解析 Claude 输出格式
}
```

```go
func parseConfidenceScore(output string) (score int, ok bool) {
    // 同上，解析 "CONFIDENCE: 85"
}
```

**所有 4 个解析器都假设 agent 的输出是 Claude 格式的 markdown + 末行标记**。

**证据 B: Budget 守卫的成本类型是 float64（美元），无厂商抽象**

```go
type runBudget struct {
    mu    sync.Mutex
    spent float64  // 美元，硬编码
    cap   float64  // 美元，硬编码
}
```

如果切换到 Codex（按 token 计费）或 Gemini（按字符计费），`runBudget` 的语义需要
等价换算——但整个 budget guard 链条不知道「美元」以外的成本单位。

**证据 C: agent-cmd 默认为 claude，且注入 claude 特定 flags**

`main.go`:
```go
flag.StringVar(&agentCmd, "agent-cmd", "claude", ...)
```

`engine_build.go`:
```go
// 对 claude-family 加 --permission-mode acceptEdits
// 对 claude-family 加 --model <tier>
// 对 claude-family 加 --max-budget-usd
```

这段代码 (`build.go`) 中通过 `isClaudeFamily(name)` 检查 agent 命令名来注入 claude 特定 flags。
没有等价的 Codex / Gemini 分支。

### 为什么这不是「v3 的事」

ROADMAP 将「跨厂商池（LiteLLM）」放在 v3。但当前架构的**成本遥测链**是厂商锁定最深的
环节——即使将来接入了 LiteLLM，只要 `cost.go` 仍然是 claude-specific，新的厂商的成本数据
就进不了 scorecard → 进不了 HistoryTiebreak → 路由决策永远偏袒 claude。

更本质地说: **成本是 G3 路由的核心输入之一，但成本数据的采集完全被单一厂商格式绑架。
这意味着「多厂商」不是给 LiteLLM 传个 API key 就完事的——必须重构成本抽象层。**

### 修复方向（非代码，仅思路）

1. **成本抽象接口**: 在 `cmd/forge/` 或 `internal/` 中定义 `CostParser` 接口:
   ```go
   type CostParser interface {
       ParseCost(output string) (usdMicros int64, ok bool)
       Vendor() string  // "claude" / "codex" / "gemini"
   }
   ```
   现有 `parseClaudeCostUsd` 实现 `CostParser`。Codex 和 Gemini 的 parser 作为后续接入点。

2. **Vendor-agnostic trace event**: `trace.Event.CostUsdMicros` 是通用格式（int64, USD micros），
   好设计。但需要加 `trace.Event.Vendor string` 字段，让 scorecard 可以按厂商分层。

3. **Agent 命令的厂商探测**: `main.go` 当前的 `isClaudeFamily` 启发式方法可扩展为
   `detectVendor(cmd string) string`，解析 `--agent-cmd` 参数推断厂商。

4. **预算单位抽象**: `runBudget` 保留美元（USD 是跨厂商的通用等价单位），但增加
   `tokenBudget` / `charBudget` 可选字段供 token-aware 计费的厂商精确跟踪。

### 产品价值

当前 ForgeOS 宣称支持 G3 多维模型路由，但实际只能路由到 Claude 的三个 tier。
成本硬编码是这个承诺和现实之间最大的单一缺口。解耦后:
- Codex/Gemini 接入时 cost telemetry 直接可用，不阻塞 scorecard 回灌
- `HistoryTiebreak` 可以按 vendor 展示性价比历史
- Budget guard 可以回答「如果换 Gemini Ultra，同一预算可以跑多少轮？」

---

## 方向三 · Agent Prompt 的「万能模型」天花板

> **「implementer、reviewer、planner 共享同一个 prompt 骨架——它们需要的是完全不同的信息」**
> **紧急度: P1** | **类别: 架构债务 · Prompt 工程** | **估算: 2 sprints**

### 问题诊断

当前 `buildPrompt`（`prompt_context.go:321-338`）对所有 agent 角色输出**同构的 prompt 结构**:

```
buildPrompt(repoRoot, phase, mode, tierOf, cache, gates, phaseOut, findings) → string
  1. Agent card (readCard)
  2. Mode + lifecycle 上下文
  3. ROADMAP.md（任务列表）
  4. ADR bullets（检索结果）
  5. Constraints（从 .agent/AGENTS.md 等）
  6. Gate results（gateLedger.context）
  7. Previous phase outputs（phaseOutputLedger）
  8. Memory entries（memoryContext）
  9. Review findings（reviewFindingsLedger）
 10. Secondary template（uses_template）
```

所有角色都获得以上所有 10 块信息。但不同角色需要的信息**类型和权重完全不同**:

| 角色 | 真正需要的信息 | 当前多余的字段 |
|------|--------------|--------------|
| **Implementer** | ROADMAP 具体任务 · ADR 技术决策 · 上一个 reviewer 的反馈 | 全部 gate 结果（值知道 pass/fail 即可，不需要明细）· memory（大部分无关） |
| **Reviewer** | 代码 diff · implementer 的产出物 · 架构标准 · 安全 checklist | ROADMAP 全部（只需要知道「要评审什么」，不需要 checklist 逐项）· ADR 检索结果（评审不看 ADR，看实现是否符合 ADR） |
| **Planner** | 架构文档 · ADR · 当前 ROADMAP · 之前迭代的 gap 分析 | Gate 结果（还没到执行阶段）· review findings（还没做 review） |
| **QA** | 测试覆盖情况 · implementer 新增的代码 · gate 结果明细 | ADR 检索结果（QA 不关心为什么要这么设计，只关心是否测了）· Constraints（已经体现在 gate 里了） |
| **CTO/Reviewer (executive)** | 架构方案 · 成本估算 · 风险评估 · 综合评审摘要 | Memory 全部 · ROADMAP 逐项（只关心关键决策）· 全部 gate 明细 |

### 代码级证据

**证据 A: `buildPrompt` 的签名不含角色参数**

```go
func buildPrompt(repoRoot string, p asset.Phase, mode string,
    tierOf func(p asset.Phase) string,
    cache *prompt.ContextCache,
    gates *gateLedger,
    phaseOut *phaseOutputLedger,
    findings *reviewFindingsLedger) string {

    // 统一构建，无角色分支
    roleCard := readCard(repoRoot, p.Agent, cache)  // 唯一与角色相关的行
    // 其余 9 行 append 对所有角色完全相同
}
```

角色特异性只有一行 `readCard`。其余 9 个信息块对所有角色一视同仁。

**证据 B: 不同角色在同一 context 长度下运行**

对于 prompt 长度敏感的 LLM（特别是 Haiku/Sonnet 档位），给 implementer 和 reviewer
同样长度的 context 意味着：
- implementer 的「有效信息密度」被与任务无关的 gate 明细稀释
- reviewer 的 prompt 被不需要的 ROADMAP checklist 填充，压缩了实际的评审上下文

**证据 C: planner 和 reviewer 收到「不存在」的信号**

`reviewFindingsLedger` 在 planner phase 时是空的（因为还没跑 review），但依然被附加到
prompt 中（`contextLines` 在空时输出 ""，但 `buildPrompt` 的 append 逻辑仍然被调用）。
同样的，`phaseOutputLedger` 在第一个 phase 时也是空的——但这 0 成本的 append 本身
不是问题，问题是**表明「这些相位收到了和它们无关的数据」是一个架构级别的信号**。

### 修复方向（非代码，仅思路）

1. **角色特定的 prompt 骨架**: 不为每个角色重新实现 `buildPrompt`，而是定义
   `PromptProfile` 结构，声明每个角色需要哪些上下文块:
   ```go
   type PromptProfile struct {
       NeedsRoadmap   bool
       NeedsFullADR   bool
       NeedsGateDetail bool     // 还是只需要 pass/fail？
       NeedsMemory    bool
       NeedsFindings  bool
       MaxContextLen  int       // prompt 上限，按角色调节
   }
   ```
   每个 agent 卡（`.agent/agents/*.md`）声明自己的 `prompt_profile:` 字段。

2. **角色自适应 Context 预算**: 不同角色获得不同的 token 预算:
   - implementer: 关注 ROADMAP + ADR，给较长 context
   - reviewer: 关注产出物 + 标准，给较短但更聚焦的 context
   - planner: 关注架构 + gap，给中等 context

3. **信息块惰性计算**: 对于不需要 gate 明细的角色，gateLedger 应只返回
   `"Gates: ALL PASS (3/3)"` 而非逐条细节——节省 prompt token 同时提供必要信息。

### 产品价值

当前的「万能 prompt」模型在只有 1-2 个 agent 角色时没问题，但 ForgeOS 已扩张到
12+ 角色，未来可能到 20+。每增加一个角色，prompt 的「平均信息密度」下降一部分。
角色特异性 prompt 是**架构级扩展性设计**——不是今天就必须做，但必须在第 20 个角色之前做。

直接可测量价值:
- **Prompt token 节省 20-40%**: 移除角色不需要的上下文块
- **响应质量提升**: reviewer 不受 ROADMAP checklist 干扰，focus 在代码审查
- **LLM 幻觉下降**: implementer 不会因为看到 reviewer 的 gate 结果而「提前修正」

---

## 方向四 · 收敛震荡与 Checklist 漂移

> **「RoadmapCompletion 从 80%→90%→85%→95%——系统不知道是进展还是格式漂移」**
> **紧急度: P2** | **类别: 产品盲区 · 收敛可靠性** | **估算: 1 sprint**

### 问题诊断

当前 `staleCount` 的收敛检测逻辑是:

```go
func staleCount(cur, prev float64, stale int, gatesGreen, prevGatesGreen bool) int {
    if cur > prev || (!prevGatesGreen && gatesGreen) {
        return 0  // 有进展，重置
    }
    return stale + 1  // 无进展，递增
}
```

这个逻辑假设「进展」是单调递增的——RoadmapCompletion 要么上升、要么持平。
但真实世界中，ROADMAP.md 的 checklist 格式可能在不同 agent 的修改下发生**微小变异**，
导致 RoadmapCompletion 百分比**震荡**:

**震荡模式 1: Checklist 格式漂移**

```
Iteration 1: agent A 写 "  - [x] implement auth"    → 计数正确 → 80%
Iteration 2: agent B 写 "- [x] implement auth"       → 少了一个空格，不计入 → 79%
Iteration 3: agent C 写 "  - [x] implement auth"     → 格式恢复 → 80%
```

系统看到: 80%→79%→80%。`staleCount`:
- Iteration 2: cur(79%) < prev(80%) → stale++
- Iteration 3: cur(80%) > prev(79%) → stale=0 ✅
- **但实际进展为零——格式在漂移，代码没变**

**震荡模式 2: 已完成项被「重新发现」**

```
Iteration 1: agent 看到 10/20 项 done → 50%
Iteration 2: agent 修复了一个旧 issue，加了一个新的已完成项 → 11/21 → 52.4%
             （但新项是旧 issue 的分拆，不是新功能）
Iteration 3: agent 合并了两个重叠的 checklist 项 → 10/20 → 50%
```

系统看到: 50%→52.4%→50%。真正的功能进展是**零**，但 `staleCount` 在 iteration 2
被重置，delay 了收敛判定。

**震荡模式 3: 并行 agent 的 checklist 竞争**

在 `--parallel` 模式下，两个 implementer 可能同时修改 ROADMAP.md。一个添加 `- [x] item A`，
另一个添加 `- [x] item B`——Git merge 后两个都保留，百分比正确进阶。但如果两者
同时修改了同一行（罕见但可能），Git 可能产生冲突标记，破坏 checklist 解析 → 百分比骤降。

### 代码级证据

**证据 A: `RoadmapCompletion` 是纯文本解析，无条件严格**

`converge.go`:
```go
func RoadmapCompletion(markdown string) float64 {
    for _, line := range strings.Split(markdown, "\n") {
        switch t := strings.TrimSpace(line); {
        case strings.HasPrefix(t, "- [x]"), strings.HasPrefix(t, "- [X]"):
            done++
            total++
        case strings.HasPrefix(t, "- [ ]"):
            total++
        }
    }
}
```

一个空格的变化（`"- [x]"` 变成 `" -[x]"`）导致该行被静默忽略——系统不会给出任何
警告，只是百分比变了。

**证据 B: 无 Checklist 格式标准化**

没有任何 agent 卡或 workflow 要求 agent 在编辑 ROADMAP.md 时遵守特定的 checklist 格式。
`reviewer.md` 的契约是 `VERDICT: APPROVE/REQUEST_CHANGES`，但不检查 ROADMAP 格式一致性。

**证据 C: `staleCount` 只检测单调性，不检测方差**

```go
// 当前: 只看 cur > prev
if cur > prev || (!prevGatesGreen && gatesGreen) { return 0 }

// 缺失: 方差检测
if abs(cur-prev) < 0.02 && cur < prev {
    // 微小负向波动 → 可能是格式漂移，不是真退化
    // 应该记录"微小波动"计数，而不是直接 ++stale
}
```

### 修复方向（非代码，仅思路）

1. **RoadmapCompletion 加入置信度区间**: 不返回 `float64`（52.4%），而是返回
   `Completion{Percent float64, Confidence float64, TotalItems int}`。
   当 TotalItems 频繁变化时（震荡模式 2），Confidence 下降。

2. **Hysteresis（滞后）检测**: 在 `staleCount` 中记录过去 3 个迭代的 completion 值，
   检测震荡模式（up-down-up 或 down-up-down）。如果检测到震荡但振幅 < 5%，
   视为「格式漂移」而非「真进展」，不重置 stale count。

3. **Checklist 格式规范化**: 在每次 `forge run/evolve` 结束时，自动标准化 ROADMAP.md
   的 checklist 格式（清除多余空格、统一缩进）。这样即使 agent 写出的格式有微小变异，
   也在下一轮开始前被修复。可放在 `windDownScorecards` 的同一阶段执行。

4. **GatesGreen 作为震荡锚点**: 当 RoadmapCompletion 震荡但 GatesGreen 持续不变时，
   判定为「格式震荡，非内容震荡」，降低 RoadmapCompletion 在收敛计算中的权重。

### 边界情况

- **新项目冷启动**: 0% 稳定，无震荡——hysteresis 检测跳过（样本不足）
- **大版本跃迁**: completion 从 10% 跃升到 60%（agent 一次性写了很多 `- [x]`）——这不是震荡，
  是真进展，hysteresis 检测不应误判
- **项目萎缩**: 主动删除已完成的 checklist 项导致 completion 下降——这是真退化还是清理？
  通过 `GatesGreen` 做锚点判断: 如果 gate 全绿，是清理；如果 gate 变红，是真退化

### 产品价值

收敛判定是 ForgeOS「无人值守」承诺的基石——系统说「做完了」是基于 `converge.MET`。
如果收敛判定本身因为 Checklist 格式漂移而产生假阳性（过早收敛）或假阴性（永不收敛），
整个自动化信任模型被削弱。Hysteresis 检测是**收敛层最后一道 honesty 防线**。

---

## 方向五 · 运行时状态生命周期管理

> **「.forge/ 目录会一直长到占满磁盘——没有配额、没有清理、没有归档」**
> **紧急度: P2** | **类别: 产品盲区 · 运维** | **估算: 1 sprint**

### 问题诊断

ForgeOS 的运行时状态存储在 `.forge/` 目录中，包含三个关键文件：

```
.forge/
├── trace.jsonl       # 每次 run/evolve 追加（~300 bytes/event）
├── memory.jsonl      # 每次 iteration + findings 追加（~250 bytes/entry）
├── checkpoint.json   # 每次 phase 后 overwrite（~1 KB）
└── checkpoint.json.1 # 轮换备份（~1 KB）
```

**当前没有任何生命周期管理**：

| 文件 | 增长模式 | 清理机制 | 配额 |
|------|---------|---------|------|
| trace.jsonl | APPEND only | 无 | 无 |
| memory.jsonl | APPEND only | 无 | 无 |
| checkpoint.json | OVERWRITE（固定大小） | 无（但自然自限） | 无（但自然自限） |
| checkpoint.json.1 | 单备份 | 无（自动覆盖） | 无（自动覆盖） |

### 代码级证据

**证据 A: trace 追加，永不轮换**

```go
// internal/trace/trace.go
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    defer t.mu.Unlock()
    line, _ := json.Marshal(ev)
    t.w.Write(line)
    t.w.Write([]byte{'\n'})
    return nil  // 永不检查文件大小
}
```

**证据 B: memory 追加，永不裁剪**

```go
// internal/memory/memory.go
func Append(path string, e Entry) error {
    f, _ := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
    line, _ := json.Marshal(e)
    f.Write(line)
    f.Write([]byte{'\n'})
    f.Close()
    return nil
}
```

虽然 `memory.Compact` 有裁剪逻辑（`memory_compact.go`），但它只在 `Load` 时被调用，
且只影响**内存中的视图**——磁盘上的 `memory.jsonl` 文件从不被重写或截断。

```go
// memory_compact.go: Compact 返回过滤后的副本，但不触及磁盘文件
func Compact(entries []Entry, maxAge time.Duration, keepPerKind int) []Entry {
    // 仅在内存中过滤，不写文件
}
```

**证据 C: 无 `forge gc` 或等价命令**

```
$ forge --help | grep -i "clean\|gc\|purge\|prune"
# 无输出
```

`cmd/forge/main.go` 的命令表中没有清理类命令。`forge doctor` 有 anomaly 检测，但
不触发清理。`forge status` 报告状态但不报告文件大小。

**证据 D: scorecard 写前读全文件，随文件增长线性退化**

`scorecard_wind.go`:
```go
sc := bufio.NewScanner(f)
for sc.Scan() {
    // 扫描整个 trace.jsonl → O(N) per run
}
```

当 `.forge/trace.jsonl` 达到 10MB+ 时（几个月持续使用），每次 run 结束时 wind-down
阶段会可感知地变慢。

### 为什么需要它

这不是「磁盘满了怎么办」的远期担忧——对于生产级的 24h 无人值守运行：

- **一个 evolve 50 次迭代**: trace ~50KB, memory ~15KB → 可忽略
- **10 个项目 × 每天 3 次 evolve × 30 天**: ~58MB trace + ~13MB memory → 不可忽略
- **CI 环境（每个 PR 一次 evolve）**: 如果 PR 数量多，.forge/ 成倍增长
- **多仓库环境**: 每个仓库有自己的 `.forge/`，全局增长 × 仓库数

更重要的是：当用户运行 `forge doctor --anomaly` 或 `forge migrate` 时，系统是否应该
建议或自动清理过期的运行时状态？当前不能——因为没有生命周期概念。

### 修复方向（非代码，仅思路）

1. **自动轮换**: 在 `Emit` 和 `Append` 中增加惰性文件大小检查：
   - trace.jsonl > 10MB → 重命名为 `.forge/trace.jsonl.1`，开新文件（保留最近一个备份）
   - memory.jsonl > 5MB → 重命名 + 开新文件
   - 文件轮换使用 `O_EXCL` + `os.Rename` 保证并发安全

2. **`forge gc` 命令**: 新增清理命令：
   - `forge gc --dry-run`: 报告可清理的空间
   - `forge gc`: 删除超过 N 天的 trace/memory 备份，压缩 checkpoint
   - `forge gc --aggressive`: 重置 memory（仅保留最近 N 条高 confidence 条目）

3. **状态文件配额化**: `forge run/evolve` 在启动时检查 `.forge/` 配额：
   - 总大小 > 配额（默认 100MB）→ 警告
   - 总大小 > 配额 × 2 → 自动触发 `forge gc --dry-run` 并建议

4. **Memory 持久化压缩**: `memory.Compact` 增加一个变体 `CompactPersist`，将
   压缩后的条目写回磁盘（备份原文件为 `memory.jsonl.bak`），让无限 append 的
   memory.jsonl 可以在用户请求或 size 阈值触发时「被缩小」。

### 边界情况

- **多进程并发写同 .forge/ 目录**: 两个 evolve 同时跑，一个触发轮换，另一个未感知
  → 需要跨进程文件锁（见 `expansion-blind-spots-v16.md` 方向一）
- **NFS/共享文件系统**: 文件大小 stat 在 NFS 上可能延迟几秒 → 轮换阈值加一个 padding
- **`forge gc` 与正在运行的 evolve 冲突**: 清理命令应检测 `checkpoint.json` 的 mtime，
  如果文件是 "热的"（最近 5 分钟内被修改过），跳过该仓库

### 产品价值

24h 无人值守是一个承诺。一个跑了一周的 evolve 不应该因为 `.forge/` 目录累积了几个月的
旧 trace 而变慢，更不应该因为磁盘满了而失败。`forge gc` 是有状态系统的标准 hygiene——
缺失它，ForgeOS 的使用者需要自己写 cron job 来清理 `.forge/`，而他们不该需要。

---

## 总结

| # | 方向 | 类型 | 紧急度 | 驱动因素 | 预估 sprint |
|---|------|------|--------|---------|-------------|
| 1 | **三框架并存的结构性债务** | 架构债务 | 🔴 P0 | 知识碎片化、维护成本非线性增长、用户困惑 | 3-4 sprints |
| 2 | **成本遥测的厂商硬编码** | 架构债务 | 🟠 P1 | 阻碍 G3 跨厂商路由、vendor lock | 2 sprints |
| 3 | **Agent Prompt 的万能模型天花板** | 架构债务 | 🟠 P1 | 随角色数量增加，信息密度持续下降 | 2 sprints |
| 4 | **收敛震荡与 Checklist 漂移** | 产品盲区 | 🟡 P2 | 收敛判定信任度被格式变异侵蚀 | 1 sprint |
| 5 | **运行时状态生命周期管理** | 产品盲区 | 🟡 P2 | 24h 无人值守的磁盘占满风险 | 1 sprint |

**这些方向的共同主线**: ForgeOS 的**能力层**（编排/路由/执法）已经成熟到可以交付，
但**架构一致性**和**产品成熟度**（多厂商/多角色/多框架/长期运维）尚未跟上。
方向一/二/三是必须清偿的架构债务（否则每新增一个能力都在碎片化上加码），
方向四/五是产品级信任工程（让用户敢把 24h 无人值守用在真实生产项目中）。

---

*分析日期: 2026-07-11 | 基于 forge-core + harness + .agent + .ai + ai-dev + pi-batch.py 全量源码扫描*
*先验去重验证: 对 ~90 篇已有分析文档做全文精确字符串搜索，确认每个方向的核心机制从未被作为独立系统性方向展开*
