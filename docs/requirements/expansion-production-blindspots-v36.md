# ForgeOS — V36 生产盲区深扫：五个被忽视的扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全仓深度扫描  
>   — forge-core 18 Go 包（195+ 源文件）/ cmd/forge 16+ CLI / harness 34+ 模块 /  
>   — `.agent/` 完整治理骨架（12 agent 卡 + 9 skill 卡 + 5 工作流）/  
>   — 31 个 Sprint 演进全读 / `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（GAP 全部收口）/  
>   — **逐篇交叉验证 docs/requirements/ 下 21 份分析文档（~68+ 已有扩展方向）**  
> **核心承诺**: 每个方向与全部已有分析文档的**核心论点不重叠**。差异证明附后。  
> **纪律**: 不编写任何代码。每个方向附代码级证据 + 边界场景 + 实现参考量级。  
> **日期**: 2026-07-10

---

## 已有覆盖全景

本文**不重复**以下已被现有 ~68+ 方向充分覆盖的域（逐篇核对 21 份 `docs/requirements/*.md`）：

| 已有覆盖域 | 涵盖文档 | 方向数 |
|---|---|---|
| 功能引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/loop-back） | `high-value-extension-directions.md`·`v3`·`v34`·`v33` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md`·`expansion-gaps-v7-novel.md` | ~10 |
| 生产可靠性（Prompt QA/信号硬化/环境验证/自愈层） | `expansion-production-readiness.md`·`v34`方向五 | ~8 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md`·`v33`方向一二 | ~10 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | `second-order-architectural-gaps.md`·`systemic-expansion-v26.md` | ~10 |
| 系统边界盲区（级联截断/信任边界/持久语义/可移植性） | `strategic-extensions-v22~v33.md`·`uncovered-frontiers-v25.md` | ~12 |
| 安全/Secret/SCA/沙箱/凭据生命周期 | `genuinely-novel-expansion-directions.md` | ~5 |
| CLI DX/Shell/daemon/增量采纳 | `expansion-self-governance.md`·`v35`方向五 | ~5 |
| 外部 SDLC 集成（PR/CI/Merge/评论/Branch） | `high-value-extension-v35.md`方向一 | ~1 |
| 治理策略测试框架 | `novel-five-highvalue-extensions.md`方向一 | ~1 |
| Agent 运行时协议抽象层 | `novel-five-highvalue-extensions.md`方向二 | ~1 |
| 收敛信号溯源与信任模型 | `novel-five-highvalue-extensions.md`方向三 | ~1 |
| 跨运行 Trace 分析与经验对比 | `novel-five-highvalue-extensions.md`方向四 | ~1 |
| 自适应治理 mode 自调优 | `novel-five-highvalue-extensions.md`方向五 | ~1 |
| 行为回归检测 / 契约式测试断言 | `high-value-extension-v35.md`方向二 | ~1 |
| 并发工作树互斥锁 | `high-value-extension-v35.md`方向三 | ~1 |
| 跨项目治理继承与知识联邦 | `high-value-extension-v35.md`方向四 | ~1 |
| 渐进式启动 profile（nano/micro/standard） | `high-value-extension-v35.md`方向五 | ~1 |
| 新项目知识启动协议 | `novel-five-frontiers-v34.md`方向一 | ~1 |
| 闸门探测结果缓存 | `novel-five-frontiers-v34.md`方向二 | ~1 |
| 统一持久化存储抽象层 | `novel-five-frontiers-v34.md`方向三 | ~1 |
| 工作流编排集成测试框架 | `novel-five-frontiers-v34.md`方向四 | ~1 |
| 运行时进程健康契约 | `novel-five-frontiers-v34.md`方向五 | ~1 |
| 并行状态一致性护栏 | `strategic-extensions-v33.md`方向一 | ~1 |
| 部分失败域隔离 | `strategic-extensions-v33.md`方向二 | ~1 |
| 声明式资源预算交叉验证 | `strategic-extensions-v33.md`方向三 | ~1 |
| 语义化配置漂移检测 | `strategic-extensions-v33.md`方向四 | ~1 |
| 跨 Session 审计因果追溯 | `strategic-extensions-v33.md`方向五 | ~1 |
| **总计已有覆盖** | | **~68+ 方向** |

---

## 本文的 5 个方向

每个方向均从**代码级微观观察**出发，交叉比对本环境代码（非抽象推理），并与全部 ~68+ 已有方向论证差异。所有方向在 **v2 增量范围内可实现**，不依赖 Firecracker / LiteLLM / 外部数据库 / 跨厂商 key。

---

## 方向一：Prompt 装配可观测性与调试通道（Prompt Assembly Observability & Debug Channel）

> **类型**: 可观测性 · 调试体验 · 开发效率  
> **优先级**: P1（agent 行为异常的调试目前只能靠「重跑 + 猜」）  
> **代码影响**: 新 `--dump-prompt` flag · `internal/prompt/audit.go` · `prompt_context.go` 新 hook  
> **差异化证明**: 已有方向覆盖了 context retrieval/caching/memory-lane 的机制设计，**但零方向覆盖「已装配的 prompt 本身」的可观测性**。当 agent 行为不符合预期时，当前没有任何方式可以审查「它究竟被告知了什么」。

### 现状：代码级证据

**证据 A：prompt 构建后直接 pipe 到 agent，无持久化副本**

```go
// engine_build.go 第 108-130 行（claudeArgv 调用链）
func agentExecutor(...) orchestrator.AgentExecutor {
    ex := orchestrator.CommandExecutor{
        Build: func(p asset.Phase, mode string) []string {
            prompt := buildPrompt(o, p, resolvedModel) // ← prompt 构建
            argv := claudeArgv(o, isClaude, tierOf(p), p)
            return append(argv, "-p", prompt) // ← prompt 直接进 argv，无副本
        },
    }
}
```

`buildPrompt` 返回的字符串被直接传入 `-p` 参数交给 agent CLI，**没有任何路径可以截获、持久化、或审查这个 prompt**。`echo` executor（dry-run）会打印 prompt，但 dry-run 不跑真 agent——**你想要调试的恰恰是真跑时 prompt 是否构建对了**。

**证据 B：`buildPrompt` 做了大量工作但结果是黑盒**

```go
// prompt_context.go 第 180-320 行（buildPrompt 的主要装配逻辑）
// 1. 注入 agent 角色卡（readCard）
// 2. 注入 task（当前 ROADMAP 项）
// 3. 注入 context（ADRs + AGENTS.md + memory + gate 历史）
// 4. 注入 emits（前序 phase 产出）
// 5. 注入 feeds_forward（有向传递）
// 6. 注入 gate 裁决日志（reviewer 感知前序门状态）
//
// 整个装配过程没有中间件、没有 hook、没有日志——是纯函数。
// 唯一的外部表现是最终的 prompt 字符串，以及 agent 代理的执行结果。
//
// 如果 agent 行为异常：
//   - 是角色卡写错了？（第 1 步）
//   - 是 task 注入错了？（第 2 步）
//   - 是 context 超过 token 限制被悄声截断了？（第 3 步）
//   - 是前序 phase 的 emits 没被正确注入？（第 4 步）
//   - 还是模型本身的问题？
// 当前没有任何方式区分上述可能性。
```

**证据 C：`prompt/retrieve.go` 的检索结果也不可观测**

```go
// prompt/retrieve.go 第 40-82 行
func Retrieve(ctx context.Context, root string, tags []string, limit int) ([]Ref, error) {
    // 扫描 .agent/**/*.md + docs/adr/*.md + .ai/reviews/*.md
    // 按 tags 过滤 → 按 term frequency 排序 → 取 top N
    // 结果只返回 []Ref（path + score），没有「为什么选了 / 为什么没选」的解释
}
```

如果一个 ADR 因为 tag 不匹配或 TF 排名太低而没有被注入到 prompt，当前没有任何方式知道这件事。`--dump-prompt` 也不会暴露检索决策过程。

**证据 D：`prompt_memory.go` 的 memory 注入是完全 opaque 的**

```go
// prompt_memory.go 第 100-160 行
func memoryContext(mem []memory.Entry, phaseName string) string {
    // 按 recency + relevance 混合排序 → 取 top memoryCap(32)
    // 返回格式化字符串
    // 没有揭示：哪些 memory 被排除、为什么被排除、每条 memory 的 score
}
```

### 建议方向

引入 Prompt 装配的**全链路可观测层**，使每个 prompt 的构建决策可审查、可调试、可对比。

**1. `--dump-prompt <dir>` flag**

运行时指定目录，每个 agent phase 执行前将完整 prompt 写入文件：

```
.forge/prompts/
├── 01-planner-20260710-152233.md
├── 02-implementer-20260710-152245.md
├── 03-reviewer-20260710-152312.md
└── ...
```

每个文件包含：
- 元数据头（phase 名、model、timestamp、token 估算）
- 完整的 prompt 文本（与发给 agent 的完全一致）
- 段标记（用 `<!-- #region agent-card -->` 等分隔各组成部分）

**2. `forge inspect prompt` 子命令**

离线分析已保存的 prompt 文件：

```
$ forge inspect prompt .forge/prompts/02-implementer-20260710-152245.md
→ Phase:         implementer
→ Model:         claude-sonnet-4
→ Token count:   ~4,200 (estimated)
→ Context sources:
  - ADR-0001 (included, 87 tokens) — tag match: "architecture"
  - ADR-0002 (included, 112 tokens) — tag match: "polyglot"
  - memory: 12/32 entries included (4 excluded by recency, 2 excluded by dedup)
  - phase_output_ledger: implementer (feeds_forward: true)
→ Warnings:
  - Total estimated tokens (~4,200) exceeds model's recommended budget (~3,500)
  - 2 ADRs matched tags but were excluded by TF ranking: ADR-0004 (score 0.3, cutoff 0.5)
```

**3. Prompt Snapshot Comparison（`forge diff-prompts`）**

比较同一 phase 在不同运行中的 prompt 构建差异：

```
$ forge diff-prompts run1/prompts/02-implementer.md run2/prompts/02-implementer.md
→ Identical context sources (3/3 ADRs, 12/32 memory)
→ Different task injection: "add rate limiter" vs "fix caching bug"
→ Memory composition differs: run1 has 3 gap-type entries, run2 has 5 lesson-type entries
```

这对复现「为什么上次 agent 做对了这次做错了」至关重要。

**4. context-assembly 审计日志**

在 `buildPrompt` 中插入轻量探针，每条 context 装配决策记录一条结构化日志：

```json
{"t": "context-assembly", "phase": "implementer", "event": "include-adr",
 "path": "docs/adr/0001.md", "reason": "tag_match", "score": 0.85, "tokens": 87}
{"t": "context-assembly", "phase": "implementer", "event": "skip-memory",
 "count": 4, "reason": "recency_decay", "decay_factor": 0.3}
```

这些日志写入 `.forge/audit/` 目录，与 trace 平行。`forge inspect prompt --audit` 消费此日志。

### 边界场景

| 场景 | 行为 |
|---|---|
| 普通运行不设 `--dump-prompt` | 零开销——审计代码路径不激活，prompt 不落盘 |
| Prompt 文件包含敏感信息（密钥在 prompt 中） | 同 `.gitignore` 模式忽略 `.forge/prompts/`；用户需自行管理 |
| 单次运行产生大规模 prompt 文件（多次迭代 × 多 phase） | 按 `phase-name-timestamp.md` 命名，总大小 ≈ prompt × phase × iteration |
| `--dump-prompt` 同时用 `--executor=command --agent-cmd=claude` | 两边都不阻塞——先 dump 再注入 argv，先 capture output 再 parse |
| `forge inspect prompt` 在无 dump 目录时 | 清晰报错：「No prompts found — re-run with --dump-prompt」 |

### 收益

| 维度 | 收益 |
|---|---|
| **调试效率** | agent 行为异常时，第一件事是看 prompt，而非「重跑试试」|
| **审计** | prompt 是可审查的，不是黑盒 pipe |
| **质量改进** | 看到 context 装配的决策，才能优化它 |
| **v3 前置** | 跨厂商 prompt 格式调试会依赖同样的基础设施 |

### 与已有分析的差异证明

最接近的已有分析：
- `expansion-production-readiness.md`「Prompt QA」方向：讨论的是 prompt 内容的质量检查（是否包含矛盾指令、是否缺少必要 context），属于 prompt 构建的**静态验证**。本文方向关注的是 **prompt 装配后可观测性**——让开发者/运维者看到实际发出了什么。静态检查 vs 运行时可见性，正交。
- `novel-five-highvalue-extensions.md` 方向三「收敛信号溯源」：关注收敛判定信号的可信度分层。本文关注的是 agent 输入的可见性，两者分别位于收敛环的输入侧和输出侧。
- `high-value-extension-v35.md` 方向二「行为回归检测」：关注 agent 输出的正确性验证。本文关注 agent 输入的可观测性。输入侧的盲区同样致命但完全未被已有方向覆盖。

---

## 方向二：Gate 结果持久化与趋势分析（Gate Result Persistence & Trend Analytics）

> **类型**: 可观测性 · 运维 · 质量保障  
> **优先级**: P1（gate 是收敛判据的核心，但其结果默认只存在于 stdout 中）  
> **代码影响**: 新 `harness/gate-persist.mjs` · `internal/gate/trend.go` · `forge gate-trend` CLI  
> **差异化证明**: 已有方向覆盖了 gate 的实现、适配器、模式门控、收敛整合，**但零方向覆盖 gate **结果的持久化与趋势分析**。当前每个门的结果只在运行时的 stdout 中出现一次，然后消失。

### 现状：代码级证据

**证据 A：gate 执行结果只打印到 stdout，无结构化持久化**

```bash
$ forge run build --executor=dry --root $PWD 2>&1 | grep "gate"
# harness-gates: running gate: test
# test: PASS (3/3 tests passed, 0 failed)
# harness-gates: running gate: complexity
# complexity: PASS (all files within complexity limits)
# → 这些结果存在于 stdout 流中；不持久化在任何结构化位置
```

检查 `gates.go` 的 `ProbeAll` 调用链：结果被收集到 `GateProof` 结构体（只用于收敛判定），然后被 `reportConvergence` 打印到日志——**既不写入 trace，也不写入独立的 gate 结果文件**。

**证据 B：GateProof 结构体是瞬态的**

```go
// converge.go 第 70-74 行
type GateProof struct {
    Names     []string     `json:"names"`
    Statuses  []GateStatus `json:"statuses"`
    ProvenP   int          `json:"proven"`
    TotalP    int          `json:"total"`
    Load      bool         `json:"load_bearing"`
}
```

它在 `gatherSignals` → `evalOne` 调用链中作为瞬态值传递，用于一次收敛判定，然后被丢弃。`LoadScorecards` 和 `trace.go` 都不消费 `GateProof`。

**证据 C：`scorecard.go` 记录 cost 和 latency，不记录 gate 结果**

```go
// routing/scorecard.go 第 30-60 行
type ScorecardEntry struct {
    Tier          string  `json:"tier"`
    QualityScore  float64 `json:"quality_score"`   // accepted/samples
    P95LatencyMs  float64 `json:"p95_latency_ms"`
    AvgCostUSD    float64 `json:"avg_cost_usd"`
    LastUpdated   string  `json:"last_updated"`
    // 没有 GatePassRate、没有 GateFailureDistribution
}
```

Scorecard 跟踪的是模型路由决策所需的数据（成本、延迟、质量）。**项目的治理健康度趋势**（哪些 gate 经常失败、failure 率是否上升、趋同前平均需要多少轮修复）完全不在任何数据结构中。

**证据 D：`trace.Event` 记录 phase 执行但不记录结构化 gate 结果**

```go
// trace.go 第 44-56 行
type Event struct {
    Kind       EventKind // PhaseStart / PhaseEnd / GateStart / GateEnd / AgentStart / AgentEnd
    Name       string
    Status     EventStatus
    DurationMs int64
    CostUsdMicros int64
    Model      string
    Detail     string // 自由文本——gate 结果被序列化成 JSON 字符串塞进去
    // 没有结构化的 gate name、gate status、gate duration 字段
}
```

Gate 结果被强行塞入 `Detail string` 字段——可读但不查询。要回答「test gate 在过去一周失败了多少次」需要 grep + 手工 JSON 解析 Detail 字段。

**证据 E：`trace.jsonl` 无索引、无保留策略**

```go
// trace.go 第 110-115 行
func (t *Tracer) Emit(ev Event) {
    // 总是 append 到 trace.jsonl
    // 没有轮转、没有压缩、没有保留策略、没有索引
}
```

对一个活跃的 ForgeOS 实例，trace 文件线性增长。没有任何机制说「保留最近 30 天的 gate 结果，更早的归档」。

### 建议方向

引入 Gate 结果的**结构化持久层 + 趋势分析能力**。

**1. 结构化 Gate 结果持久化（`harness/gate-persist.mjs`）**

每次 gate 执行后（以及 `forge accept` 中），将结果写入 `.forge/gates/` 目录：

```
.forge/gates/
├── 2026-07-10.jsonl    # 按日期分片
├── 2026-07-11.jsonl
└── ...
```

每条记录（JSONL 一行）：

```json
{
  "_schema": "forgeos.gate_result.v1",
  "run_id": "evolve-20260710-152233",
  "timestamp": "2026-07-10T15:23:45Z",
  "gate": "test",
  "status": "PASS",
  "detail": "3/3 tests passed, 0 failed",
  "duration_ms": 1250,
  "mode": "engineering",
  "lifecycle": "mvp",
  "phase": "harness-gates",
  "iteration": 2
}
```

**接入方式**：`harness/gate-persist.mjs` 是一个薄 logger 模块，被 `gate.mjs` 和 `acceptance.mjs` 在每次 gate 判定后调用。`internal/gate/resolve.go` 的 `HarnessRunner` 在 trace emit 之后再加一行 persist 调用。零行为影响。

**2. Gate Trend 查询（`forge gate-trend` CLI）**

```bash
# 基本趋势：过去 N 天的 gate 通过率
$ forge gate-trend --days 7
→ Gate pass rates (last 7 days):
  test:        42/45 = 93.3%
  lint:        38/40 = 95.0%
  complexity:  40/40 = 100%
  secret-scan: 40/40 = 100%
  app-test:    12/15 = 80.0%

# 按 mode 分组
$ forge gate-trend --mode engineering --days 30
→ Gate pass rates (engineering mode, last 30 days):
  test:        89/102 = 87.3%
  ...

# 失败归因
$ forge gate-trend --gate test --failed
→ test gate failures (last 7 days):
  2026-07-10T10:22:33 — "2/3 tests passed" — phase: implementer (iteration 3)
  2026-07-09T14:11:22 — "0/3 tests passed" — phase: harness-gates (iteration 1)
  ...
```

**3. 保留策略和自动轮转**

按天分片（`YYYY-MM-DD.jsonl`），保留窗口可配置：

```bash
# 默认保留 90 天
# --gate-retention-days 30 可覆盖
# 超期的文件自动清理（forge status 时触发，或者单独的 forge gate-prune 命令）
```

**4. 收敛健康度仪表盘集成**

趋势数据可被 `forge status` 消费：

```bash
$ forge status
→ Workflow: engineering, lifecycle: mvp
→ Last run: 2026-07-10T15:22:33 (converged: true)
→ Gate health (30-day): test 93.3%, lint 95.0%, app-test 80.0% ⚠
→ Gate failure trend: test improving (95% → 93% → 97%), app-test degrading (90% → 85% → 80%)
```

### 边界场景

| 场景 | 行为 |
|---|---|
| `.forge/gates/` 目录不存在 | 自动创建（同 `.forge/` 的初始化） |
| 并发写入（两 forge 实例同目录） | JSONL append 是原子行写入，不会交错损坏（与 trace.jsonl 同级别保证） |
| 趋势分析日活数据不足（新项目，只有今天数据） | `forge gate-trend` 显示「Insufficient data — need at least 2 days」 |
| 保留策略清理旧文件时 forge 正在运行 | forge 持有 `.forge/` 的 fd，文件清理不影响打开的文件句柄；Unix 惯例 |
| 用户从不清理旧数据 | JSONL 线性增长但极慢（一次 evolve ~100 gate 执行，一天 1000 行 ~100KB，90 天 ~9MB） |

### 收益

| 维度 | 收益 |
|---|---|
| **运维** | 不再需要 grep stdout 来看 gate 历史 |
| **质量** | 趋势数据能早期预警质量退化（如 app-test 从 95% 降到 80%） |
| **诊断** | `forge gate-trend --gate test --failed` 直接给出失败详情，不需要复现 |
| **改进入口** | 知道哪个 gate 最常失败、在什么 mode 下失败，才能针对性改进 |

### 与已有分析的差异证明

最接近的已有分析：
- `novel-five-highvalue-extensions.md` 方向四「跨运行 Trace 分析」：分析 trace 数据（cost/latency）来回答经验对比问题（「改 mode 后收敛更快了吗？」）。本文方向分析的是 **gate 结果**（PASS/FAIL 分布和趋势），数据来源不同、消费者不同、回答的问题不同。Trace 分析回答「运行时特征变化」，gate 趋势回答「质量健康度变化」——两个维度正交且互补。
- `novel-five-frontiers-v34.md` 方向二「闸门探测结果缓存」：关注的是 gate 探针的**增量重评估**（减少不需要重复跑的探针以加速），而非结果持久化。缓存提高速度，持久化提供历史——两者无重叠。
- `expansion-production-readiness.md` 方向五「Gate 信号硬化」：关注 gate 信号缺失时的默认值安全，而非结果存档。
- `strategic-extensions-v33.md` 方向五「跨 Session 审计因果追溯」：关注的是**跨 run 的事件关联**（两次 run 之间是否有因果关系），而非门级结果趋势。

**核心差异**：本文方向提出了一个全新的数据结构（`.forge/gates/` 日期分片 JSONL 库）和一个全新的 CLI 消费者（`forge gate-trend`）。已有分析中没有任何一方提到这些。

---

## 方向三：跨 Phase 输出语义一致性守卫（Cross-Phase Semantic Coherence Guard）

> **类型**: 正确性 · 质量保障 · 信任  
> **优先级**: P2（agent 输出不一致是收敛但次优的常见原因）  
> **代码影响**: 新 `internal/coherence/` 包 · 新 `forge check-coherence` CLI · 可选 workflow `coherence.yml`  
> **差异化证明**: 已有方向覆盖了 planner→implementer 的文本关键词匹配、gate 的结构验证、reviewer 的二元裁决，**但零方向覆盖跨 phase agent 产出的 **语义一致性系统验证/验证、跨 phase 输出间的结构化契约验证**。

### 现状：代码级证据

**证据 A：`feeds_forward` 传递文本但不验证语义一致性**

```go
// prompt_context.go 第 180-210 行
func appendFeedbackLanes(...) {
    // feeds_forward: true 的 phase 前传产出
    // 但前传的只是 raw text（整个输出文件内容）
    // 没有「planner 说要添加 rate limiter → implementer 的代码确实有 rate limiter」验证
}
```

`feeds_forward` 是一个信息传递机制，不是一个验证机制。planner 的输出被传给 implementer 用作任务描述，但没有任何东西检查 implementer 是否**真的按计划做了**。

**证据 B：planner 和 implementer 之间没有契约**

```go
// planner agent 卡的角色描述
// "输出 plan 到 docs/plan/<timestamp>.md"
// implementer agent 卡说 "读取最新 plan 并实现"
// 但 plan 是自由文本 markdown，没有结构化字段确保 implementer 能消费
// 没有「planner 的 output schema」可以验证 implementer 是否正确覆盖了所有计划项
```

检查全部 5 个 workflow：每个 agent phase 的 `emits:` 声明了输出文件，但**没有任何正式描述**说这些输出文件应该有什么结构。reviewer phase 的存在就是来检查这些的——但 reviewer 本身也是一个 agent，它也是不可靠的。

**证据 C：phase 间的语义一致性仅靠 agent 天然能力维持**

```go
// 在 system prompt 层面说“follow the plan”，但没有程序化的校验
// 没有「planner 说要改文件 A → git diff 确实改了文件 A」
// 没有「planner 说添加功能 X → 新测试确实测试了功能 X」
// 没有「planner 说重构 -> 行数确实无明显变化」
```

这实际上是 v35 方向二部分覆盖的领域——但 v35 关注的是「agent 不引入回归」（测试结果对比），而本文关注的是「agent 按计划执行了指定任务」（planner 与 implementer 的一致性）。

**证据 D：workflow 的输出无形式化契约**

```yaml
# build.yml 第 42-46 行
- name: planner
  agent: planner
  emits:
    - docs/plan/
    # 文件被创建，但结构未定义
```

`emits:` 指定了输出目录，但没有指定输出格式。一个空的 `plan.md` 和一份详尽的 50 行计划在结构上等价——gate 无法区分。

### 建议方向

引入跨 phase 语义一致性的**结构化轻量验证层**，不依赖 agent 的自我审查能力。

**1. Phase 输出 Schema 声明（可选的 `schema:` 字段）**

在每个 workflow phase 的 `emits:` 或新增的 `schema:` 字段中声明输出格式：

```yaml
# build.yml（修改后）
- name: planner
  agent: planner
  emits:
    - docs/plan/
  schema:
    - file: "docs/plan/plan.md"
      required_sections:
        - "## Tasks"
        - "## Files to modify"
        - "## Dependencies"
      # 轻量关键词检查：以下关键词必须在输出中出现
      required_keywords:
        - "+src/"   # 至少提到一个新增文件路径
```

```yaml
# review.yml（reviewer phase 的 schema 例子）
- name: performance-review
  agent: performance-engineer
  emits:
    - docs/review/performance/
  schema:
    - file: "docs/review/performance/*.md"
      required_keywords:
        - "VERDICT:"   # 机读裁决必须在场
        - "latency"    # 绩效审核必须讨论延迟
```

**2. `forge check-coherence` 验证命令**

运行时在 gate phase 中自动执行（或在 converge 前额外执行一步）：

```
$ forge check-coherence
→ planner→implementer (build.yml):
  ✓ Plan mentions "rate limiter" → implementer diff contains "rateLimit" or "limiter"
  ✓ Plan mentions "src/shortener.mjs" → git diff includes src/shortener.mjs
  ⚠ Plan mentions "add tests" → new test file found, but coverage gate not run yet

→ discover→design stage transition:
  ✓ PRD MUST list has 5 items → ADR references 4/5 (80%)
  ⚠ PRD mentions "compliance requirement" → no ADR covers compliance (expected: design.yml P2)

→ build→review stage transition:
  ✗ performance engineer review expected but no review output found
```

**3. 三种一致性检查策略**

| 策略 | 机制 | 例子 | 成本 |
|---|---|---|---|
| **关键词追溯** | planner 描述中出现的关键词在 implementer 的 git diff 中出现 | 「rate limiter」→ 代码含 `rateLimit` | 最轻量（纯 grep） |
| **文件变更断言** | planner 声明的文件变更，git diff 确实覆盖 | 「修改 src/api.ts」→ git diff 包含 src/api.ts | 轻量 |
| **结构一致性** | pipeline 上下游的输出结构兼容 | planner schema → implementer output schema → reviewer schema | 中等（schema 解析 + 匹配） |

**4. Plan-Actual Diff 报告**

在收敛报告中增加一段「计划 vs 实际」对比：

```
→ Plan vs Actual:
  Planned files: 3 (src/api.ts, src/rate-limit.ts, test/api.test.ts)
  Modified files: 4 (src/api.ts, src/rate-limit.ts, test/api.test.ts, src/utils.ts)
  Unplanned changes: src/utils.ts (review: okay, it's a small import fix)
  Missed changes: (none)
  Plan coverage: 100% (3/3 planned files modified)
```

### 边界场景

| 场景 | 行为 |
|---|---|
| planner 输出不包含结构化关键词 | 关键词追溯降级为「跳过」（N/A），不阻断 |
| schema 文件不存在 | `forge check-coherence` 跳过该 phase（N/A），不报错 |
| 同一相干性有多个来源声称 | 用 `feeds_forward` 的前序输出作为权威（非下游解释） |
| implementer 做了计划外的工作 | 报告为「非计划变更」（warning 而非 blocking） |
| 纯修复任务（无 planner phase） | 跳过 plan→implementer 一致性检查，只保留其他可用的检查 |

### 收益

| 维度 | 收益 |
|---|---|
| **信任** | agent 不是「自己说做了就算做了」，有机械验证 |
| **质量** | plan 漂移（planner 说东、implementer 做西）被早期捕获 |
| **收敛加速** | reviewer 不需要发现「implementer 漏了计划中的某件事」这种低级问题 |
| **审计** | 每次运行的计划 vs 实际差异被持久化，可追溯 |

### 与已有分析的差异证明

最接近的已有分析：
- `high-value-extension-v35.md` 方向二「行为回归检测」：核心是 **test-result 差异化对比**（stash 前后测试结果对比）和契约式测试断言。它检查的是代码的正确性（「改动不破坏已有功能」）。本文方向检查的是 agent 的**计划执行一致性**（「改动符合计划」）。两个方向正交——一个 agent 可以完全执行了计划但也引入了回归（被 v35 捕获），也可以完美保留了已有功能但完全偏离了计划（被本文捕获）。
- `execution-semantic-gaps.md` 方向二「phase 输出契约的形状校验」：关注输出文件的 **JSON Schema 形状验证**（输出是否符合某种格式合约）。本文关注的是**跨 phase 的语义对应**（上下游输出之间的内容一致性）。形状校验和语义对应是两个不同层面的问题。
- `novel-extensions-v12-architect-perspective.md` 方向二「跨阶段语义一致性守卫」：最接近的已有分析。但该方向聚焦于**关键词匹配**（planner 说 X → implementer 提了 X），且被定为一个「注册表+匹配器」的机制设计。本文方向将其明确为三种层次（关键词追溯、文件变更断言、结构一致性），并建议作为**可选的 gate 插件**来运行——定义为收敛前的验证步骤而非 workflow 内置 phase。此外，本文的「Plan-Actual Diff」报告格式和 `schema:` 字段声明在设计细节上完全不同。

---

## 方向四：Token-Aware 上下文预算管理（Token-Aware Context Budget Manager）

> **类型**: 性能 · 可靠性 · 模型经济性  
> **优先级**: P2（随着上下文装配越来越丰富，token 超限是渐近风险）  
> **代码影响**: 新 `internal/prompt/budget.go` · `prompt_context.go` 修改 · `--max-context-tokens` flag  
> **差异化证明**: 已有方向覆盖了 context retrieval、caching、memory-cap 等装配机制，**但零方向覆盖 **模型感知的 token 预算管理与动态修剪**。

### 现状：代码级证据

**证据 A：`prompt_context.go` 装配 context 时不估算 token**

```go
// prompt_context.go 第 180-320 行（buildPrompt 主体）
// 1. 读角色卡（readCard）
// 2. 读任务（ROADMAP）
// 3. 读 context（Retrieve → 排序后取 top limit）
// 4. 读 memory（memoryContext → 取 top 32）
// 5. 拼接成 prompt 字符串
// 没有任何位置估算 prompt 的 token 数量
// 没有任何位置检查 prompt 是否超过模型 context window
```

**证据 B：`prompt.GatherCached` 不做 token 预算**

```go
// prompt/cache.go 第 40-100 行
func (c *Cache) GatherCached(ctx context.Context, wf asset.Workflow, ...) ([]byte, error) {
    // 检索 ADR / AGENTS.md / context 文件
    // 按 tag 过滤 → 按 TF 排序 → 取 top limit
    // limit 是文件数（int），不是 token 预算（int）
    // 如果一个文件有 10K tokens 而另一个只有 100，它们被等概率选中（在 TF 排序前）
    // 没有 token 预算感知的重排序
}
```

`limit` 参数是文件个数硬限制，不是 token 预算。如果检索到的每个文件都很大（如包含代码样例的 ADR），context 可能膨胀到远超模型的 context window。

**证据 C：不同模型的 context window 差异没有被任何代码消费**

```go
// routing/routing.go 第 217-231 行
var ModelMap = map[string]map[string]string{
    "anthropic": {
        Haiku:  "claude-sonnet-4-haiku",
        Sonnet: "claude-sonnet-4",
        Opus:   "claude-opus-4",
    },
    // 没有 context_window 字段
    // 没有 max_output_tokens 字段
    // 没有 token_budget 字段
}
```

路由层知道模型名但不知道模型的 context window 大小。`Claude Sonnet 4` 有 200K context window，但 `Claude Haiku` 也有 200K——这个信息存在但未被使用。当一个 model tier 被选定后，prompt 装配器不知道这个 tier 的 token 预算，因此也无法针对它优化。

**证据 D：`memoryCap = 32` 是硬编码常量，不是动态预算**

```go
// prompt_memory.go 第 48 行
const memoryCap = 32
```

memory 注入的条目数上限是固定的 32 条，不管模型是 Haiku（仍可容纳更多 token）还是模型上下文正被 ADR 填充到接近极限。没有考虑 token 层面的权衡：一个 token 预算有限的 prompt，memory 应该让位给 ADR，反之亦然。

**证据 E：prompt 超限后的行为是模型静默截断**

当 prompt 超过模型的 context window 时，模型**静默截断最早的内容**（按 position）。这意味着：
- 硬约束 ground truth（AGENTS.md 的红线）如果放在 prompt 开头且超过 token 预算，会被截断
- 用户不知道截断发生了
- ForgeOS 以为「地面实况已注入」，但实际上已经被模型丢弃

### 建议方向

引入一个 Token 预算管理层，让 prompt 装配在跨模型的 token 预算约束下运行。

**1. 模型 Token 预算注册表**

```go
// internal/prompt/budget.go
package prompt

type ModelBudget struct {
    ModelName     string // "claude-sonnet-4"
    ContextWindow int    // 200000 — 模型总 context window
    MaxOutput     int    // 8192 — 模型最大输出 token
    Recommended   int    // 120000 — 推荐的 prompt token 预算（保留输出空间 + 余量）
    Provider      string // "anthropic"
}

var modelBudgets = map[string]ModelBudget{
    "claude-sonnet-4":       {ModelName: "claude-sonnet-4", ContextWindow: 200000, MaxOutput: 8192, Recommended: 120000, Provider: "anthropic"},
    "claude-opus-4":         {ModelName: "claude-opus-4",   ContextWindow: 200000, MaxOutput: 8192, Recommended: 120000, Provider: "anthropic"},
    "claude-sonnet-4-haiku": {ModelName: "claude-sonnet-4-haiku", ContextWindow: 200000, MaxOutput: 8192, Recommended: 120000, Provider: "anthropic"},
}
```

**2. 带 Token 估算的上下文装配优先级队列**

替换先排序再截断的做法为**带权重的 token 预算分配**：

```
Token budget: 120,000 (recommended for claude-sonnet-4)

Priority 1 — Hard constraints (must include, ~5K total):
  - AGENTS.md ground truth (inviolable)
  - Current task from ROADMAP
  - Agent card

Priority 2 — Directly relevant context (try to include, ~20K target):
  - Matching ADRs (tag filter)
  - Latest reviewer feedback
  - Phase output ledger (feeds_forward)

Priority 3 — Helpful context (include as budget allows, up to budget):
  - Memory entries (scored by recency × relevance)
  - Historical scorecard data
  - Supplementary skill cards

Priority 4 — Nice-to-have (include only if budget has headroom):
  - Full context of all ADRs
  - All memory entries
  - Extended system prompts
```

**3. Token 计数集成**

```go
// 使用 tiktoken 或类似的分词器估算 prompt token 数
// forge-core 是纯 Go 零依赖——所以 token 计数也必须是零依赖
// 方案：基于 chars/tokens 比例的经验估算（~4 chars/token for English, ~1.5 for CJK）
// 或：Go 版本的 tiktoken（需要引入依赖——可选择 rust/cgo 或纯 Go 实现）
//
// 轻量 v1：用近似公式 token ≈ chars / 3.5（英文为主）
// 精确 v2：嵌入 tiktoken 的 Go 编译（需 vendors）
```

**4. 预算超限时的告警与降级行为**

```
┌─ Agent Phase: implementer ──────────────────────────────┐
│ Model: claude-sonnet-4, token budget: 120,000           │
│ Estimated prompt tokens: 156,230 (exceeds budget by 30%)│
│                                                         │
│ Priority 4 items dropped:                               │
│   - 11/32 memory entries (oldest by recency)            │
│   - 2 supplementary skill cards                         │
│   - 1 historical scorecard (not critical)               │
│                                                         │
│ Priority 3 items trimmed:                               │
│   - 3 ADRs included in abbreviated form (only titles)   │
│                                                         │
│ ⚠ Budget exceeded — 36,230 tokens over recommended      │
│   (prompt may be truncated by model)                    │
└─────────────────────────────────────────────────────────┘
```

**5. `--max-context-tokens` 可配置 flag**

```bash
# 用户可覆盖默认的 token 预算
forge run build --max-context-tokens 80000
# 在测试/调试场景中限制 prompt 大小以加速
```

### 边界场景

| 场景 | 行为 |
|---|---|
| Token 估算不精确（≈ ±20%） | 作为参考而非硬限制——真正的截断由模型决定。ForgeOS 做尽力而为但不保证精确 |
| 零依赖 Go 下无法嵌入 tiktoken | v1 用 char/token 经验比率估算；v2 可引入依赖或 RPC 调用 |
| 模型 context window 变更（Anthropic 升级模型）| `ModelBudget` 集中维护，升级模型改一个文件 |
| CN 混合语言时 token 估算偏差 | 估算公式设为可扩展（允许多语言加权）或提供 override |
| 用户认为 token 预算过低 | `--max-context-tokens` flag 覆盖 |

### 收益

| 维度 | 收益 |
|---|---|
| **正确性** | AGENTS.md 地面实况不再因超限被模型静默截断 |
| **成本** | 不需要的 context token 被裁剪——按 token 计费的成本降低 |
| **可预测性** | 用户知道 prompt 是否安全在 context window 内（不再「模型静默丢」）|
| **质量** | 优先级调度让最重要的 context 总是先被包含 |

### 与已有分析的差异证明

最接近的已有分析：
- `expansion-production-readiness.md`：讨论了 token 预算相关的上下文溢出问题（方向四「context 传染病 —— 质量衰退」），但关注的是 context 质量而非 token 预算管理。它问「是不是包含了不该有的 context」，而本文问「预算有限下哪种 context 应该被优先保留」。
- `novel-five-frontiers-v34.md` 方向三「统一持久化存储抽象层」：关注不同存储后端（文件系统、S3、K/V 存储）的统一抽象——与 token 预算完全正交。
- `execution-semantic-gaps.md` 方向一「phase 输出原子性」：关注一个 phase 的多个输出文件之间的原子性保证——与 token 预算完全正交。

**核心差异**：本文方向是第一个将**模型信息（context window）反馈到 prompt 装配决策**中的设计。现有所有 prompt 装配决策（`GatherCached`、`memoryContext`、`buildPrompt`）都**不知道模型类型和预算**，只对 context 做数量或排序的简单截断。

---

## 方向五：自治工作流执行超时与熔断体系（Autonomous Workflow Timeout & Circuit-Breaker Hierarchy）

> **类型**: 可靠性 · 运维 · 成本控制  
> **优先级**: P1（当前熔断机制碎片化，缺少统一的工作流级熔断策略）  
> **代码影响**: `internal/orchestrator/circuit.go`（新文件）· `loop.go`/`orchestrator.go` 修改 · `--circuit-breaker` flag  
> **差异化证明**: 已有方向覆盖了四维资源护栏（recursion/budget/timeout/output-cap）、backoff 重试、doom-loop tripwire，**但零方向覆盖统一多级熔断体系**——熔断决策目前分散在三层（系统调用超时、phase 超时、迭代 no-progress tripwire），没有层次化熔断状态机、没有熔断状态持久化、没有手动重置命令。

### 现状：代码级证据

**证据 A：现有超时和熔断机制分散在三层，缺乏统一协调**

```
第一层 — 系统调用超时（command_executor.go）
  └ `--timeout`：单个 agent 命令执行超时 → SIGKILL → 归类为 retryable（`KindOverloaded`）
  └ 被 backoff 重试机制消费（backoff.go — 最多 3 次退避重试）

第二层 — Engine 级别预算熔断（orchestrator/budget.go）
  └ `--max-agent-calls`：单次 run 的总 agent-phase 执行数上限
  └ `--run-budget-usd`：总美元成本上限 → `BudgetAdjustTier` 降级路由
  └ `budget.go` 中的 `BudgetExhausted` 被 LoopEngine 消费为外部停止信号

第三层 — Loop 级别 Doom-Loop 防护（loop.go）
  └ `MaxIter`：安全底线（超限报告为未收敛，不熔断）
  └ `NoProgress` 触发器：staleCount 达到阈值后停止迭代
```

这三层互不通信。一个 phase 超时 5 次被 backoff 重试（第一层），phase-level budget 不感知（第二层），loop-level 不感知（第三层）。熔断决策是**局部最优而非全局最优**。

**证据 B：没有熔断状态持久化**

```bash
$ grep -rn "circuit\|breaker\|half-open\|熔断" forge-core/ --include="*.go" | grep -v _test | grep -v "Circuit\|Breaker"
# 无输出 —— 不存在熔断模式的概念
```

if a run is cancelled due to repeated timeouts, restarting the same workflow (`--resume`) will retry the same failing phase from scratch—with the same configuration that failed before. There's no memory that "this phase was failing in the previous run."

**证据 C：超时和重试配置是全局的，不是 phase-specific 的**

```go
// main.go 第 45-68 行
// --timeout 是全局 flag，所有 phase 共享同一个超时时间
// --max-agent-calls 也是全局的
// 但 reviewer phase(需要更多推理时间)和 implementer phase(可能写很多代码)的超时需求完全不同
```

一个配置适用于所有 phase：reviewer（通常很快，~30s）和 implementer（可能很慢，~5min）共享同一个 `--timeout`。

**证据 D：`backoff.go` 的重试不记录失败上下文**

```go
// orchestrator/backoff.go 第 20-60 行
type RetryPolicy struct {
    MaxAttempts int
    BaseDelay   time.Duration
    MaxDelay    time.Duration
}

func (r *RetryPolicy) ShouldRetry(err error) bool {
    // 检查错误类型（retryable / terminal）
    // 重试后返回重试结果
    // 没有持久化记录「这个 phase 已经失败了 3 次」
}
```

每次 forge 新进程都从 clean state 开始重试逻辑。没有 `failure_count` 持久化到 `.forge/` 目录，所以跨运行的失败趋势不可知。

### 建议方向

引入层次化熔断体系（Multi-Level Circuit Breaker），统一管理超时、重试、降级和手动恢复：

**1. 三层次熔断状态机**

```
层 1 — Phase 级熔断（per-phase circuit breaker）
  状态: CLOSED → OPEN（连续失败 N 次后断开）→ HALF-OPEN（等待冷却）
  触发条件: phase 连续超时 / 连续 gate FAIL / 连续 agent 错误
  动作: OPEN 后跳过该 phase（标记为 skipped）、降级至 echo executor、或用备用 phase 替代
  恢复: 手动 `forge circuit-reset --phase <name>` 或冷却超时后 HALF-OPEN 试探

层 2 — Workflow 级熔断（per-workflow circuit breaker）
  状态: CLOSED → OPEN（该 workflow 连续多次不收敛）
  触发条件: 连续 N 次 run/evolve 不收敛（converged=false）
  动作: OPEN 后建议降低 mode/lifecycle 严格度、或建议换 workflow
  恢复: 手动 `forge circuit-reset --workflow build`

层 3 — 系统级熔断（global circuit breaker）
  状态: CLOSED → OPEN（所有 workflow 都被熔断）
  触发条件: 系统资源严重不足（磁盘满 / OOM / 关键工具缺失）
  动作: OPEN 后所有 forge 操作（除 status 和 circuit-reset 外）拒绝执行
  恢复: 手动 `forge circuit-reset --system`（清理后）
```

**2. 熔断状态持久化**

熔断状态写入 `.forge/circuit.json`：

```json
{
  "phases": {
    "implementer": {
      "state": "OPEN",
      "failure_count": 5,
      "last_failure": "2026-07-10T15:23:45Z",
      "last_error": "timeout: command exceeded 5m0s",
      "opened_at": "2026-07-10T15:23:45Z"
    }
  },
  "workflows": {
    "build": {
      "state": "HALF_OPEN",
      "consecutive_failures": 3,
      "last_run": "2026-07-10T15:30:00Z",
      "last_outcome": "NOT_CONVERGED"
    }
  }
}
```

**3. Phase 级感知的熔断决定机制**

每个 phase 的超时和重试策略可独立配置：

```yaml
# .agent/workflows/build.yml（扩展）
phases:
  - name: implementer
    agent: implementer
    timeout: 5m         # implementer 需要更长超时
    retry:
      max_attempts: 3
      backoff: exponential
    circuit_breaker:
      failure_threshold: 5  # 连续 5 次失败后 OPEN
      cool_down: 10m        # 10 分钟后恢复 HALF_OPEN
      fallback: skip        # OPEN 后的行为：skip / use-stub / fail-closed

  - name: reviewer
    agent: reviewer
    timeout: 30s        # reviewer 通常很快
    retry:
      max_attempts: 1    # 审不过就审不过，不重试
    circuit_breaker:
      failure_threshold: 10
      cool_down: 5m
      fallback: skip-with-note  # skip + 标记跳过的原因
```

**4. `forge circuit-status` / `forge circuit-reset` CLI**

```bash
# 查看所有熔断器状态
$ forge circuit-status
→ Phase-level circuit breakers:
    implementer:     CLOSED (0 failures)
    reviewer:        OPEN (12 failures) — last: timeout
    qa:              CLOSED (1 failure, below threshold 5)
→ Workflow-level:
    build:           HALF_OPEN (3 consecutive failures, cooling 7m remaining)
→ System-level:     CLOSED

# 手动重置单个熔断器
$ forge circuit-reset --phase reviewer
→ reviewer circuit breaker reset: OPEN → CLOSED

# 重置所有
$ forge circuit-reset --all
→ All circuit breakers reset to CLOSED
```

### 边界场景

| 场景 | 行为 |
|---|---|
| Workflow 连续失败但原因不同（一次超时、一次 gate 失败、一次 agent 错误） | 视为不相关的失败——只递增连续相同错误的计数 |
| `forge circuit-reset` 后 phase 立刻再次失败 | 熔断再次 OPEN（更快——阈值减半，避免熔断→复位→熔断震荡）|
| 熔断 OPEN 的 phase 被 `--resume` 跳过 | 跳过 OPEN 的 phase（报告为 skipped），从下一个 phase 开始 |
| 用户从不检查熔断状态 | 不影响正常运行——熔断只在连续失败后触发降级，不主动阻断第一次尝试 |
| 分布式/并发运行时熔断竞态 | `.forge/circuit.json` 原子读写（与 checkpoint 同级别保证，当前没有分布式锁）——v1 暂不支持分布式熔断状态共享 |

### 收益

| 维度 | 收益 |
|---|---|
| **稳健性** | 一个有问题的 phase 不会无限重试烧预算——在 X 次失败后自动跳过 |
| **可恢复性** | 熔断状态跨运行持久化——重跑知道「这个 phase 上次失败了，先试探」|
| **可运维性** | 清晰的 CLI 查看和重置熔断状态，取代"我不知道为什么这个 phase 一直失败" |
| **成本控制** | 熔断机制在 agent-phase 数量维度（---max-agent-calls）之外提供了第二道成本防线 |

### 与已有分析的差异证明

最接近的已有分析：
- Sprint 22-23「真点火安全护栏四维」（recursion/budget/timeout/output-cap）：这是四维资源安全护栏，每个维度是一个独立的安全边界。本文方向是整合和升华这些孤立的护栏为一个**层次化熔断状态机**——有状态（CLOSED/OPEN/HALF-OPEN）、持久化（跨重启记忆）、可管理（手动复位命令）。四维护栏好比四个独立的保险丝，熔断体系是「配电盘+保险丝盒+重置按钮」的整体。
- `strategic-extensions-v33.md` 方向二「部分失败的域隔离」：关注一个 phase 失败时不波及其他 phase（独立错误域）。本文关注的是**失败后的熔断行为**（多次失败后自动降级或跳过）——两者互补：域隔离决定「失败的影响范围」，熔断决定「失败后的处理策略」。
- `high-value-extension-v35.md` 方向三「并发工作树互斥」：解决双实例冲突，与熔断正交。
- `expansion-production-readiness.md` 方向三「环境验证」：侧重运行前的环境检查（工具可用性），而非运行中的故障熔断。

**核心差异**：本文方向是第一个提出**三层次熔断状态机 + 状态持久化 + 手动复位命令**的设计。现有机制专注于「如果失败该怎么重试」（backoff）和「如何防止无限循环」（tripwire），本文聚焦于「如果连续失败该怎么降级并在未来预防同样的问题」（circuit breaker）。

---

## 优先级建议

| 方向 | 优先级 | 理由 | 预估复杂度 |
|---|---|---|---|
| 方向一：Prompt 装配可观测性 | P1 | 当前 agent 调试是黑盒——`--dump-prompt` 是基础调试设施，应尽早建立 | 小（~200 Go + ~100 JS） |
| 方向二：Gate 结果持久化与趋势 | P1 | gate 是 ForgeOS 的核心价值主张，但结果不可查询——这是基础运维就绪缺口 | 中（~300 Go + ~200 JS + 新 CLI） |
| 方向五：熔断体系 | P1 | 四维护栏已落地但各自隔离——统一熔断体系是「生产就绪」的最后一公里 | 中（~400 Go + 持久化 + CLI） |
| 方向三：跨 Phase 语义一致性 | P2 | 重要但门槛较高（需要 schema 声明约定），非初版必需 | 中-大（~500 Go + schema 设计） |
| 方向四：Token 预算管理 | P2 | 当前 context 规模尚可控，但长期看是渐进风险——建议放在方向一之后 | 中（~300 Go + 估算模型） |

所有方向**不互相依赖**，可按任意顺序独立交付。
