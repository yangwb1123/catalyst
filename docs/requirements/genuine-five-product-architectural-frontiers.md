# ForgeOS — 五个人视角下仍有结构性缺口的扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局逐文件扫描代码库：forge-core（18+ Go 包 / `cmd/forge` 17+ 子命令 / ~33k LOC）、  
>    harness（39+ 模块 / ~10.5k LOC 执法层）、`.agent/` 完整治理骨架（12 agent 卡、9 skill 卡、5 工作流）、  
>    examples/（url-shortener、go-taskd）、CI pipeline、pi-batch.py（499 行）、全部 ADR+DECISIONS  
> 2. 通读现有 59 份 `docs/requirements/*.md` + 40+ 份 `docs/analysis/*.md` +  
>    `FUNCTIONAL_REQUIREMENTS_AUDIT.md` + 全部 sprint 记录（1–31），逐方向交叉验证。  
> 3. **差异化验证**: 对每个方向在全部已有文档中做关键词 + 语义搜索，确认核心论点  
>    **从未作为独立扩展方向被系统性展开**。  
> 4. **纪律**: 不编写任何代码。每个方向附代码级证据。  
> **日期**: 2026-07-10  

---

## 全景定位

59 份 `docs/requirements/*.md` 已覆盖极广的表面。以下方向落在所有已有覆盖的间隙中：

| 已有高密度覆盖 | 本文方向（未被已有文档作为独立方向展开） |
|---|---|
| Gate 执行经济学 / 记忆去重 / 墙钟预算 / 编排器 Hook / 并行执行 | **① ForgeOS Self-Hosting Bootstrap** |
| 生产可靠性 / 环境验证 / 健康契约 / 进程健康守护 | **② Agent 输出信心归因**（超过二元裁决） |
| 执行语义形式化 / 因果一致性 / 版本演化 / 状态恢复 | **③ 知识策展管线**（语义质量，非存储优化） |
| 二阶系统问题 / 知识衰减 / 配置爆炸 / 无声数据丢失 | **④ 无人值守信任曲线**（超越二进制 crash barrier） |
| 多仓库联邦 / 跨会话治理 / 可插拔扩展 / Schema 版本化 | **⑤ 自举治理资产版本收敛**（非方案版本，是演进路径版本） |

---

## 方向一 · ForgeOS Self-Hosting Bootstrap

**优先级**: 🔴 P0 | **类别**: 产品 · 元治理 | **预估**: 3+ sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 宣称「AI 24h 自治完成 Idea→Production」，但其自身的代码演进是**手动 sprint-by-sprint** 的：
- 31 轮 sprint 每轮由人类（或 Claude Code agent 手动）逐个修改文件
- CI 虽然跑 `forge accept`，但从没有 CI job 运行 `forge evolve` 来管理 ForgeOS 自身的功能演进
- 两个示例（url-shortener、go-taskd）被端到端管线建造，但 ForgeOS 自身从未被自己的工具建造

这不是「缺某个功能」，而是**产品哲学的自指一致性缺口**：一个声称「让 AI 自治建造软件」的工具，
不能自治建造自己——这可能是产品信任的最大短板。

### 为什么之前做不到

Self-hosting 对 ForgeOS 有独特的自举困难：

1. **递归闸门问题**: ForgeOS 的 `harness/policies.yml` 设 `enforce: block`，`arch-check.mjs` 读 `max_function_lines: 50`。如果 `forge evolve` 修改 ForgeOS 自身代码，中间状态的违规（例如重构中的超大函数、临时测试文件突破扇入上限）会被自己的闸门**立即阻断**，而不会有「等我改完再检查」的宽容窗口。

2. **文件数预算僵局**: `.arch/rules.yaml` 设 `package.max_files: 17`（`cmd/forge`）。如果 evolve 需要在 `cmd/forge` 加一个新子命令，它必须同时在其他地方进行合并才能不突破预算——但合并本身也是 evolve 的一部分，形成「先有鸡还是先有蛋」的自锁。

3. **零外部依赖约束**: forge-core 的 `go.mod` 无 `require`。evolve 不能引入任何外部 Go 库。这大幅限制了 evolve 能自动执行的修复策略。

4. **CI → evolve 循环未接通**: `.github/workflows/forge.yml` 只跑 `forge accept` + `go build/test`，从没跑过 `forge evolve`。即使本地能够 self-host，CI 也无法验证它持续工作。

### 代码级证据

```yaml
# .github/workflows/forge.yml (当前 CI)
steps:
  - name: forge accept
    run: node harness/acceptance.mjs
  - name: go build
    run: go -C forge-core build ./...
  - name: forge-core tests
    run: go -C forge-core test ./...
  - name: forge run build --executor dry
    run: /tmp/forge-test run build --executor dry --root $PWD
    # ← `forge evolve` 从未在 CI 中出现
    # ← ForgeOS 从不尝试用自己管理自己
```

```yaml
# harness/policies.yml
enforce: block
# ← 全仓 block 模式。任何 `forge evolve` 修改 ForgeOS 自身的中间状态都会被拦截。
# ← 没有 `forge evolve --bypass-gates` 或宽容模式
```

```go
// forge-core/go.mod
// ← 无 require 行
// ← forge-core 零外部依赖。evolve 不能引入第三方库来帮助重构。
```

### 具体建议

**阶段 1（~1 sprint）: CI 验证自举可行性**
- 在 CI 中加一个 `forge evolve run build --executor command --agent-cmd echo --max-agent-calls 3` job
（非 claude，仅验证编排路径），确保自举管线至少是可调用的
- 此阶段 zero 预算开销（echo 不调 LLM），纯管线验证

**阶段 2（~1 sprint）: 引入递归闸门的豁免机制**
- `harness/policies.yml` 加 `enforce: self_hosting_relaxed` 模式（或 `--allow-intermediate-violations`）：允许 `forge evolve` 在**同一个提交内**存在中间状态的违规，只要最终提交通过全部闸门
- 实现：`acceptance.mjs` 在检测到 `FORGE_SELF_HOSTING` 环境变量时，对工作树中的 **uncommitted 改动** 使用宽松阈值（如 max_function_lines 翻倍），仅在最终 `forge accept` 时使用真实阈值

**阶段 3（~1 sprint）: 最小自举演示**
- 选择一个超小范围的任务作为首次自举尝试（例如：为 forge-core 增加一个新 `internal/xxx` 包的骨架），
用 `forge evolve` 和 `--agent-cmd=claude` 真跑，验证管线能完成且输出被 `forge accept` 接受
- 目标不是「替代所有开发」，而是**证明路径可行**

### 边界情况

| 情况 | 处理 |
|---|---|
| evolve 修改了闸门配置自身 | 配置变更应使用**两阶段提交**: 先改配置 → `forge accept`（新闸门生效） → 再用新闸门 |
| evolve 创建了文件但闸门超限 | 允许多次 evolve 迭代逐步收敛，而非 single-pass 必须完美 |
| 自举 CI job 失败 | 自举是**信息性的**（informational），不阻断主 CI 绿色——它只报告「self-hosting 管线今日状态」 |

---

## 方向二 · Agent 输出信心归因（超过二元裁决）

**优先级**: 🟠 P1 | **类别**: 路由 · 可观测性 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 当前对 agent 输出的所有裁决都是**二元的**：

| Phase | 裁决类型 | 输出 |
|---|---|---|
| `requirement-discovery` (product-manager) | 连续值 | `CONFIDENCE: 0-100` |
| `implementer` (build) | 无裁决 | gate 过/不过 |
| `code-review` (reviewer) | 二元 | `APPROVE` / `REQUEST_CHANGES` |
| `executive-review` (cto) | 五择一 | `APPROVE` / `APPROVE_WITH_SIMPLIFICATION` / `REDESIGN` / `DELAY` / `REJECT` |
| `security-review` | 二元 | `VERDICT: APPROVE` / `VERDICT: REQUEST_CHANGES` |
| qa | 二元 | gate 过/不过 |

但「批准但不确定」和「批准且确信」在系统中看起来完全一样。一个 security engineer 说
「看起来没问题，但我没检查 auth 层」（confidence ≈ 60%）和「逐行审计了全部代码路径，无风险」（confidence ≈ 98%）都输出同样的 `VERDICT: APPROVE`。

这意味着：
- **Loop-back 决策盲**: 60% 置信度的 APPROVE 应该引发额外 review，但当前不能
- **路由错配**: Opus 对一个简单任务输出 99% 置信度，Haiku 对同样任务输出 60%——但它们的 `VERDICT: APPROVE` 完全一样
- **长期趋势不可见**: 如果某个 agent card 的 prompt 变更导致它的输出置信度从 90% 降到 70%，当前没有任何指标能捕获

### 代码级证据

```go
// forge-core/cmd/forge/cost.go
// parseConfidenceScore 只适用于 discover 阶段:
func parseConfidenceScore(output string) (int, bool) {
    // ← 只搜索 "CONFIDENCE: <N>"
    // ← 只在 product-manager 的机读契约中声明
    // ← 其他所有 agent card 不声明这个契约
}
```

```go
// forge-core/cmd/forge/cost.go
// observeFor 的三路 fallback:
// 1. 二元 reviewer 契约 (VERDICT: APPROVE/REQUEST_CHANGES)
// 2. 五择一 executive 契约
// 3. confidence 契约 (第三级 fallback)
// ← 但第三级 fallback 只在 discover 阶段真正工作
// ← 所有 review/build 阶段的 agent 卡没有声明 CONFIDENCE 契约
```

```go
// forge-core/internal/orchestrator/orchestrator.go
// loopBackTo 的触发条件:
func (e *Engine) AgentVerdict(wf asset.Workflow, phaseIdx int) (string, bool) {
    // ← 基于 agent 输出的 VERDICT 文本
    // ← 没有 confidence 维度
    // ← 60% 置信度的 REQUEST_CHANGES 和 98% 置信度的处理方式完全一样
}
```

```go
// forge-core/internal/converge/converge.go
// Signals 结构体:
type Signals struct {
    RoadmapCompletion float64
    GatesGreen        bool
    // ...
    // ← 没有 per-agent Confidence 信号
    // ← 没有 AggregateConfidence 字段
}
```

### 差异化证明与已有覆盖

| 已有文档 | 覆盖内容 | 与本文区别 |
|---|---|---|
| `architectural-expansion-perspectives.md` | 讨论 confidence metric 在 discover phase 的消费 | 不讨论将其扩展到 review/build 阶段 |
| `fresh-expansion-perspectives.md` | 讨论「内容复杂度适配」（给不同模型不同深度任务） | 那是**任务分解深度**，不是**输出信心归因** |
| `cross-cutting-systemic-gaps.md` | 讨论版本追踪（prompt 内容版本化） | 那是**版本归因**（why quality changed），不是**信心归因**（how sure is the agent） |
| `forgotten-five-system-boundaries.md` | 讨论 agent 输出闸门和真实性验证 | 那是**输出真伪**，不是**输出信心** |

已有文档中 `strategic-production-gaps.md` 最接近——它提到「不确定性的表达」，但只作为 route 优化的一个子段落，不是独立方向。

### 具体建议

**做法**: 将 CONFIDENCE 契约从 discover 阶段的独有机制，推广为所有 agent 卡的**可选第二契约**（与 VERDICT 并列，但放在同一行或下一行）：

```
VERDICT: APPROVE
CONFIDENCE: 85
```

- `cost.go` 的 `parseConfidenceScore` 改为通用解析器（不绑定 phase 名）
- `observeFor` 每个阶段都尝试解析 CONFIDENCE 行（失败 = 默认 0 = unmetered）
- `converge.Signals` 增加 `AgentConfidence float64` 字段（加权平均或取各 phase 最小值）
- scorecard 增加 confidence 维度（`avg_confidence`）
- `forge route` 消费 confidence 信号：低置信度输出 → 强制升档 Opus 或增加 review 轮次

### 边界情况

| 情况 | 处理 |
|---|---|
| agent 不输出 CONFIDENCE | 默认 confidence=0（≈「未测量」），不与「低信心」混淆 |
| agent 输出明显不合理的值（>100 或 <0） | clamp 到 [0,100]，trace 中记录指标异常 |
| 同一 phase 多个 agent 输出不同 confidence | 取最低（最保守）——安全优先 |
| 模型升级后 confidence 普遍提升 | 这是好现象，scorecard 趋势可自动检测 |

---

## 方向三 · 知识策展管线（语义质量，非存储优化）

**优先级**: 🟠 P1 | **类别**: 记忆 · 知识管理 | **预估**: 2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的 Memory Engine 是一个**纯追加日志**（append-only JSONL）。知识只有三种操作：
- `Append`（写入）
- `Load`（全量读取）
- `Prune`（按 kind 保留最近 N 条——纯计数，不涉及语义）
- `Query`（按 topic/kind 过滤）
- `Supersedes`（手动标注取代关系）

没有自动策展（curation）机制的后果：

**1. 知识冗余积累**。同一个 gap 可能在多个迭代中被多次发现和记录：

```jsonl
{"seq":1,  "kind":"gap", "topic":"api-testing", "content":"no integration tests for payment endpoint", "source":"explorer/iteration-1"}
{"seq":42, "kind":"gap", "topic":"api-testing", "content":"missing payment integration test coverage",    "source":"explorer/iteration-5"}
// ← 两条实质相同，但无机制合并
```

**2. 矛盾知识无声共存**。两个 agent 做出不同决策，都写入 memory：

```jsonl
{"seq":5,  "kind":"decision", "topic":"database", "content":"use PostgreSQL for transactional data"}
{"seq":37, "kind":"decision", "topic":"database", "content":"use SQLite for simplicity (YAGNI)"}
// ← 两条矛盾，但 Query 返回两条，前者优先级高（seq 小）但不一定正确
```

**3. 知识没有置信度衰减**。Sprint 1 的一条知识（`confidence=1.0`）和 Sprint 30 的一条新知识（`confidence=0.8`）在 Query 中权重一样。没有基于「时间」或「被后续事件矛盾」的自动衰减。

**4. `Prune` 是纯计数，不评估质量**：

```go
// forge-core/internal/memory/memory_compact.go
func Prune(path string, keepPerKind int) (int, error) {
    // ← 按 kind 保留最近 N 条
    // ← 不评估条目的质量、置信度、相关度
    // ← 可能删除高价值旧知识，保留低价值新知识
}
```

### 差异化证明与已有覆盖

| 已有文档 | 覆盖内容 | 与本文区别 |
|---|---|---|
| `systemic-expansion-v26.md` 方向四「记忆压缩」 | 性能优化：减少 memory 加载与注入的 token 开销 | **性能维度**。本文讨论**语义质量维度**：去重、矛盾检测、置信度评估 |
| `second-order-architectural-gaps.md`「知识衰减」 | 知识时效性（过时知识自动降权） | 那是**衰减**（stale → lower weight）。本文是**策展**（主动去重、矛盾解决、质量评分） |
| `expansion-horizon-three.md`「跨 session 记忆传递」 | Session 间的知识分享与纠错 | 那是**分发**（knowledge sharing between sessions）。本文是**单一 session 内部的知识洁癖** |
| `expansion-five-product-blindspots.md` 方向二「跨会话记忆传递与学习继承」 | 新 session 继承旧 session 的学习成果 | 那是**继承机制**（session→session）。本文是**在单个 memory store 内的质量保证** |

已有文档中没有任何一份将知识策展（curation）作为独立的质量维度处理——所有相关讨论要么是性能优化（减小体积），要么是分发机制（跨 session 共享），要么是结构扩展（增加字段）。None of them treat knowledge quality as a first-class, continuously-maintained property.

### 具体建议

**第一层（~0.5 sprint）: 冗余检测与合并**
- `memory.Query` 增加 `Deduplicate()` 方法：对同一 `kind+topic` 的条目，保留 confidence 最高者，合并 content（append 或取最新）
- 检测条件：`kind` + `topic` + 文本相似度（编辑距离或最长公共子串，零外部依赖可用）

**第二层（~0.5 sprint）: 矛盾检测**
- `memory.Entry` 增加 `Contradicts string` 字段（可选，指向被矛盾的 entry seq）
- `Append` 时自动扫描已有条目中相同 `kind+topic` 的内容，检测语义反转（基于 sign 词：use/don't use, should/shouldn't, yes/no）
- 检测到矛盾时：新条目 confidence 保留，旧条目 confidence 减半，双方都 `Contradicts` 指向对方

**第三层（~0.5 sprint）: 时间衰减整合**
- `memory.Query` 增加 `DecayByAge(halfLife time.Duration)` 选项：按条目的写入时间指数衰减 confidence
- 默认 halfLife = 30 天（对齐已有 `policy.yml` 的 `recency_half_life_days: 30`）
- 在 `observeFor` 或 `buildPrompt` 中混合记忆注入时应用衰减

**第四层（~0.5 sprint）: 知识质量看板**
- `forge memory-status` 子命令：显示 memory 统计（总条目数、去重后可合并数、矛盾数、按 kind 分布、平均 confidence、最旧/最新条目时间）
- 与 `forge doctor` 集成：当记忆冗余率 > 20% 或矛盾条目 > 5 时发 WARN

### 边界情况

| 情况 | 处理 |
|---|---|
| 两条不同 topic 的 entry 实际上冗余 | 文本相似度是启发式，不可能 100% 准确。诚实标注：`Deduplicate` 是 advisory，从不自动删除——它报告「可合并」候选项，由后续 `Prune` 或用户确认后执行 |
| 矛盾检测的假阳性（两条 entry 看起来矛盾但实际互补） | 检测结果是置信度减半而非删除——即使假阳性，也只是轻微降低权重而非丢失信息 |
| 时间衰减导致高频 session 的知识被不公平降级 | halfLife 从条目的 `AppendedAt` 算起，与 session 频率无关 |

---

## 方向四 · 无人值守信任曲线

**优先级**: 🟢 P2 | **类别**: 产品 · 运维 | **预估**: 1.5–2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 的核心产品承诺是「24h 无人值守运行」。但当前的安全护栏全是**二进制 crash barrier**——
要么 pass，要么 fail，缺少中间的**信任建立机制**：

| 安全维度 | 当前机制 | 缺点 |
|---|---|---|
| 深度 | `--max-agent-depth` 达到上限拒绝 spawn | 用户不知道「这次 evolve 预计会 spawn 多少层」 |
| 预算 | `--max-agent-calls` 超限 fail-closed | 用户不知道「标准 evolve 一次迭代约花费多少」 |
| 时间 | `--timeout` 超时 kill | 用户不知道「这个 phase 比正常慢了 3x」 |
| 成本 | `--agent-max-budget-usd` 封顶 | 用户不知道「一次完整的 evolve 管线大约要烧多少钱」 |
| 输出 | `--max-output-bytes` 截断 | 用户不知道「agent 输出了异常的 10MB 日志」 |

真实场景：用户想跑一个 24h 的 `forge evolve` 但不确定它会花多少钱、需要多久、会改什么文件。
当前回答只能是「设置上限然后祈祷」——没有一个模式能说「好的，我先仿真一次（dry-run with reasoning），
这是预计路线图：将修改约 5 个文件、增加 ~200 行代码、预计成本 $0.30–0.50、预计 8–12 个迭代。
确认继续吗？」

这不是缺某个功能——这是**产品的信任基础**。没有这个，用户永远不放心让 ForgeOS 无人值守运行。

### 代码级证据

```go
// forge-core/cmd/forge/evolve.go — cmdEvolve
func cmdEvolve(args []string) int {
    // ← 直接开始 evolve
    // ← 没有 "--dry-run" 的扩展版本输出预估
    // ← 没有 "先计划，后执行" 的两阶段模式
    // ← 没有 "--confirm" 标志
}
```

```go
// forge-core/cmd/forge/preflight.go — cmdPreflight
func cmdPreflight(args []string) int {
    // ← 检查环境是否就绪（python3、claude CLI、workflow 文件可解析等）
    // ← 但不做任何运行时的成本/范围预估
    // ← 不说 "this evolve will cost approximately $X"
}
```

```go
// forge-core/internal/trace/trace.go
type Event struct {
    // ← 记录了历史 duration 和 cost
    // ← 但没有任何函数用历史数据对未来的 run 做预估
    // ← 没有 "avg cost per phase by task_type" 的查询
}
```

```go
// forge-core/internal/routing/scorecard.go
// ScorecardPair 有 AvgCostUsd、AvgLatencyMs、SampleCount
// ← 存在历史数据，但只用于模型选择（HistoryTiebreak）
// ← 不用于成本/时间预估
```

### 差异化证明与已有覆盖

| 已有文档 | 覆盖内容 | 与本文区别 |
|---|---|---|
| `five-systemic-oversights-v45.md` 方向三「成本预估与预算协商」 | Cost forecasting 作为**预算管理功能** | 那是**经济维度**（防止超支），本文是**信任维度**（让用户有信心让系统无人值守） |
| `production-hardening-five-v42.md` 方向二「多边资源合约」 | 给 **gate 执行**加资源声明和预算调度 | 那是**内部 gate 调度**，不是**用户可见的信任建立** |
| `expansion-production-readiness.md` 方向四「模式系统熔断与降级」 | 运行时自保 | 那是**系统自保**，不是**用户心理安全感** |
| `forgotten-five-foundations.md` 方向一「跨进程守护」 | 运行时状态锁和 PID 文件 | 那是**并发安全**，不是**运行前信任建立** |
| `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向四「多会话协调」 | Session 管理和 daemon 模式 | 那是**运行时协调**，不是**运行前透明** |

最重要的是：所有已有分析都在讨论**系统如何保护自己**（crash barrier、预算隔离、状态锁），
没有任何分析讨论**系统如何赢得用户的信任**让用户愿意打开这些保护并允许无人值守运行。
这是一个产品视角的缺口，不是技术视角的。

### 具体建议

**第一层（~0.5 sprint）: `forge plan` —— 运行前仿真**

新增 `forge plan` 子命令（与 `forge run`/`forge evolve` 同级），它：
1. 读取 workflow，列举所有 phase 及其 agent 类型
2. 查询 scorecard 历史：相同 `(workflow, model, task_type)` 组合的 `AvgCostUsd` + `AvgLatencyMs`
3. 如果没有历史数据，用默认估算值（写在 `modes.yml` 或 hardcoded fallback）
4. 输出结构化预估：

```
$ forge plan evolve build --mode balanced --max-iter 5
forge plan: evolve build (balanced, mvp)
  phases per iteration: planner(haiku) implementer(haiku) reviewer(sonnet) qa(haiku)
  estimated cost per iteration: $0.08-$0.15
  estimated duration per iteration: 45s-90s
  estimated total for 5 iterations: $0.40-$0.75 / 3.5min-7.5min
  safety bounds: --max-agent-calls 20 --max-agent-depth 2 --timeout 5m
  based on: 7 historical runs (scorecard N=42 samples)
  confidence: MEDIUM (3 of 4 phase types have historic data)
```

**第二层（~0.5 sprint）: `--confirm` 标志**

`forge run/evolve --confirm`：在执行第一个 agent phase 前打印 plan 并等待用户输入 `y/yes`。
目的是防止「不小心在 production 仓库上跑了个昂贵的 evolve」。

```
$ forge evolve build --mode engineering --confirm
ForgeOS Plan:
  ...(同上)...
  WARNING: mode=engineering will run all gates including coverage+lint
  Enter 'yes' to continue (or Ctrl-C to abort): _
```

**第三层（~0.5 sprint）: 梯度告警（非二进制）**

当前 `--timeout` 超时直接 kill。改为多级告警：

```
--timeout-warn 60s   # phase 超过 60s → [WARN] phase X is taking longer than usual
--timeout 300s       # phase 超过 300s → kill
```

同样逻辑用于 output-size：
```
--max-output-warn 1MiB   # agent 输出超过 1MiB → [WARN] agent output unusually large
--max-output-bytes 10MiB # agent 输出超过 10MiB → truncate
```

### 边界情况

| 情况 | 处理 |
|---|---|
| 无历史数据（第一次跑） | `forge plan` 使用 `modes.yml` 中的 `default_cost_estimate` 字段（新增），或 `unknown — no historic data` |
| 历史数据来自不同 lifecycle | 按 lifecycle 分桶查询，不同 lifecycle 的成本差异大（mvp vs production） |
| `forge plan` 输出与真实成本偏差大 | 诚实标注：`confidence: LOW (based on N=2 samples)`。每次实际成本写入 scorecard，逐步提高信任精度 |

---

## 方向五 · 自举治理资产版本收敛

**优先级**: 🟢 P2 | **类别**: 治理 · 运营 | **预估**: 1 sprint | **杠杆**: ⭐⭐⭐

### 问题描述

ForgeOS 治理资产（`.agent/agents/*.md`、`.agent/workflows/*.yml`、`harness/*.mjs`、`.ai/prompts/*.md`）
的数量和内容随 sprint 持续增长。当前状态：

- 12 个 agent 卡
- 5 个工作流（discover、design、review、build、evolve）
- 9 个 skill 卡
- 39+ 个 harness 模块
- 4 个 ADR
- 多个 `.ai/prompts/*.md` 评审模板

这是 ForgeOS 自身的治理资产栈。当一个项目通过 `forge-init` 继承这些资产时，它得到的是
**当前时间点的快照**。`forge-upgrade` 可以重新同步**文件内容**，但它无法回答：

1. **哪些治理资产对当前项目是必需的？** — 12 个 agent 卡中是否每个都是当前 lifecycle 和 mode 
下需要的？如果 lifecycle=idea，可能需要 product-manager 和 architect，但不需要 security-engineer
和 performance-engineer。

2. **治理资产自身的「收敛」状态是什么？** — 项目启动后，`.agent/` 中的治理资产是否被手工修改过？
修改次数 vs 上游版本号？有没有「此项目自定义了很多治理配置，但核心 workflow 与上游一致」的意识？

3. **治理资产升级的冲突声明是什么？** — `forge-upgrade --apply` 会覆盖文件。如果项目自定义了
`build.yml` 的一个 gate 阈值，上游也改了同一条——自动覆盖会丢失项目配置。当前没有版本感知的
merge 策略。

这是「治理资产自举后的版本漂移」问题——是 Sprint 30 的 FUNCTIONAL_REQUIREMENTS_AUDIT 中
「内容版本化」缺口的剩余部分。横向对比治理资产的版本是「快照 vs 上游」的关系，没有收敛的概念。

### 代码级证据

```go
// forge-core/cmd/forge/validate.go — cmdValidate
func cmdValidate(args []string) int {
    // ← 验证 workflow YAML 可解析，agent 引用可解析
    // ← 不检查治理资产与项目 lifecycle/mode 的匹配度
    // ← 不说 "this project is lifecycle=mvp but has production-level review phases"
}
```

```go
// forge-core/internal/doctor/governance.go — Governance
func Governance(root string) GovernanceReport {
    // ← 检查治理目录的文件存在性和最后修改时间
    // ← 但不计算「治理资产总数 vs 当前 lifecycle 建议数」的比率
    // ← 不检测「过时治理资产」（如 idea 项目仍持有 security-engineer 卡）
}
```

```go
// harness/scaffold/forge-upgrade.mjs — manifestProjection
export function manifestProjection(sourceRoot) {
    // ← 机械地同步文件（byte-identical overwrite）
    // ← 不处理本地自定义 vs 上游的冲突
    // ← 没有 "three-way merge" 概念
}
```

### 差异化证明与已有覆盖

| 已有文档 | 覆盖内容 | 与本文区别 |
|---|---|---|
| `cross-cutting-systemic-gaps.md` 方向二「治理资产版本化」 | 治理文件内容的 git hash 记录到 checkpoint | 那是**版本标记**（record which version ran）。本文是**版本收敛**（which versions are needed by this project） |
| `architectural-expansion-perspectives.md` 方向三「治理资产 Schema 版本化」 | YAML schema 字段增删的版本演进 | 那是**格式版本**（format evolution）。本文是**内容策略版本**（content relevance） |
| `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 中 ADR-0003 相关条目 | Submodule 共享治理资产的机制 | 那是**分发拓扑**。本文是分发后**每个项目自身的资产集是否最优** |
| `expansion-five-product-blindspots.md` 方向四「组织级多租户与策略继承」 | 多项目之间的治理分层继承 | 那是**组织拓扑**（org hierarchy）。本文是**单项目的资产集精简性** |

核心区别：已有分析讨论「治理资产如何分发给项目」，本文讨论**分发后每个项目持有的治理资产集
是否过时/冗余/过剩**——这是「治理 Hygiene」的未覆盖角度。

### 具体建议

**第一层（~0.3 sprint）: 治理资产 → lifecycle 映射表**
- 在 `modes.yml` 或新增的 `governance.yml` 中声明每个治理资产所属的 lifecycle 阶段：

```yaml
# .agent/policies/governance.yml（新增）
governance_assets:
  agent_cards:
    product-manager: [idea]
    architect: [idea, mvp, growth]
    implementer: [mvp, growth, production]
    reviewer: [mvp, growth, production]
    security-engineer: [production]
    performance-engineer: [production]
    distributed-engineer: [production]
    cto: [mvp, growth, production]
    explorer: [growth, production]
    researcher: [idea, growth]
    planner: [mvp, growth, production]
    qa: [mvp, growth, production]
  workflows:
    discover: [idea]
    design: [idea, mvp]
    review: [mvp, growth, production]
    build: [mvp, growth, production]
    evolve: [growth, production]
```

**第二层（~0.3 sprint）: `forge validate --governance-fit`**
- 新增检查：对比当前 `project.yml` 的 `lifecycle` 与资产声明中的生命周期范围
- 输出示例：

```
$ forge validate --governance-fit
forge validate: governance fit for forgeos (mode=engineering, lifecycle=mvp)

unnecessary assets (lifecycle=mvp doesn't need):
  ⚠ agent card: security-engineer (requires production)
  ⚠ skill card: ai-sdlc-review (requires production)
  ⚠ workflow: review (requires production, but your mode=engineering may still use it)

missing assets (lifecycle=mvp typically needs):
  ℹ agent card: architect is present ✓
  ℹ agent card: implementer is present ✓
  ℹ workflow: build is present ✓

summary: 3 unnecessary, 0 missing. governance coverage 89%.
```

**第三层（~0.4 sprint）: `forge prune --governance`**
- 安全移除当前 lifecycle 不需要的资产（只移除文件，不改变 workflow 引用）
- 默认 dry-run（只列出），`--apply` 才真正删除
- 移除前验证：`forge validate` 绑定该项资产的所有 workflow 仍能解析且 agent 引用不报错

### 边界情况

| 情况 | 处理 |
|---|---|
| 项目自定义了非标准 lifecycle 映射 | `pragma: skip_governance_fit_check` 在 agent card 头部声明，跳过该资产的检查 |
| lifecycle=production 但需要 researcher 卡 | 映射表是**建议值**（recommended range），不是**白名单**。项目可以保留超出范围的资产，只是打标提醒 |
| 用户对默认映射有异议 | 映射表是项目可编辑的 YAML 文件，project.yml 可配置 `governance_asset_overrides:` 覆盖默认映射 |

---

## 优先级总结

| 优先级 | 方向 | 杠杆 | Sprint 预估 | 前置依赖 |
|---|---|---|---|---|
| 🔴 P0 | ① Self-Hosting Bootstrap | ⭐⭐⭐⭐⭐ | 3+ | 自举豁免机制（阶段 2） |
| 🟠 P1 | ② Agent 输出信心归因 | ⭐⭐⭐⭐ | 1.5 | 机读契约推广（agent 卡版本化） |
| 🟠 P1 | ③ 知识策展管线 | ⭐⭐⭐⭐ | 2 | 记忆结构扩展（Contradicts 字段） |
| 🟢 P2 | ④ 无人值守信任曲线 | ⭐⭐⭐⭐⭐ | 1.5–2 | Scorecard 历史查询能力 |
| 🟢 P2 | ⑤ 治理资产版本收敛 | ⭐⭐⭐ | 1 | 治理→lifecycle 映射文件 |

**做前三件**: ① + ③ + ④。Self-hosting 验证产品愿景，知识策展解决真实用户痛点（记忆膨胀），
信任曲线打开无人值守的采纳漏斗。三件合起来建立一个自洽的叙事：ForgeOS 能管理自己（①）、
能管理自己的知识（③）、能让用户放心让它管理（④）。

**其中方向②（信心归因）** 虽为 P1 但可推迟：它依赖 agent 卡的机读契约扩展，而这些契约
在 Sprint 28–30 刚刚稳定。建议等 agent 卡契约模式再运行 1–2 个 sprint 后再扩展。

**方向⑤（治理版本收敛）** 是低风险、高可见性的「治理 Hygiene」改进：它不需要新增运行时
状态或 agent 卡修改，只需要新增一个 YAML 映射文件和验证逻辑，适合与新 sprint 并行执行。
