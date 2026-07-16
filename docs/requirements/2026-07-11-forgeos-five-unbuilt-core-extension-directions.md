# ForgeOS — 全局深扫后的五个未建核心扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局逐包深扫 forge-core(18 Go 包 · ~32k LOC 生产代码 + 77 测试文件) · harness(39+ 模块) · `.agent/` 完整骨架(5 工作流 · 12 agent 卡 · 9 skill 卡 · 全部策略) · examples/ · `.forge/` 运行时产物 · `pi-batch.py`  
> **审阅范围**: Sprint 1–31 完整演进记录 · `CURRENT_SPRINT.md` · `FUNCTIONAL_REQUIREMENTS_AUDIT.md`(全量 90+ DONE,所有 GAP 已关) · ~120 篇已有 `docs/analysis/` · 157 篇已有 `docs/requirements/`  
> **去重协议**: 对每个方向的核心议题组合词,在全部已有文档中执行全文精确匹配 + 语义交叉验证,确认该方向的核心命题**从未被作为独立系统性方向展开**  
> **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据、产品价值判断、诚实边界  
> **日期**: 2026-07-11

---

## 全景概览

ForgeOS 经过 31 轮 sprint,在功能层已高度成熟。`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 确认所有已知 GAP 已关闭。但通过逐函数读码,五个**架构层级的基础空白**浮现——它们不是"少个功能",而是能力基座的结构性欠账:

| # | 方向 | 类别 | 优先级 | 一句话总结 |
|---|------|------|--------|-----------|
| 1 | **Prompt Token 预算管理与自适应上下文窗口** | 性能·可靠性 | P1 | 系统有美元/调用/时间/输出/深度五维预算,但**少了一个维度**:prompt token 无预算、无估算、无模型感知 |
| 2 | **Agent 输出语义验证管线(超越形式门禁)** | 功能 | P1 | 现有闸门全部检查**形式**(体积/架构/secret/治理),但不验证**语义**(代码是否编译/行为是否正确/契约是否兑现) |
| 3 | **Failed-Run 确定性重放与调试** | 开发者体验 | P1 | trace.jsonl 记录了"发生了什么",但无法**精确重现**一次失败迭代(内存/memory 变化使 prompt 每次不同) |
| 4 | **多厂商模型路由——超越 Claude-only 池** | 架构可扩展性 | P2 | 路由层硬编码三个 Claude tier(Haiku/Sonnet/Opus),无 Provider 抽象层、无厂商无关模型 ID |
| 5 | **Memory 价值感知生命周期管理** | 性能·质量 | P2 | memory 是纯 append-only 累积,Compact/Prune 按计数不按价值,Confidence 字段存在但不消费 |

---

## 方向一 · Prompt Token 预算管理与自适应上下文窗口

> **关键词验证**: `(token.*budget|context.*window.*manage|prompt.*budget|context.*budget|token.*cap|prompt.*size.*limit|window.*adapt|adaptive.*truncat)`  
> → 零篇文档。`forgotten-product-five-v51.md:173` 提及 `context_window: 32768` 作为配置字段,但此处讨论的是**机制层缺失**——不是少个配置,而是少一个从"估算→决策→降级"的完整子系统。独立且不同。

### 代码证据

系统已有**五维**运行时预算,但缺少第六维:

| 维度 | 存在? | 代码位置 | 机制 |
|------|-------|----------|------|
| 美元/调用 | ✅ | `orchestrator.checkAgentBudget()` `cost.go:runBudget` | `--agent-max-budget-usd` / `--run-budget-usd` |
| 调用次数 | ✅ | `orchestrator.checkAgentBudget()` | `--max-agent-calls`, per-run count |
| 墙钟时间 | ✅ | `command_executor.go:Timeout` | `--timeout`, per-phase deadline |
| 嵌套深度 | ✅ | `command_executor.go:MaxDepth` | `--max-agent-depth`, fork-bomb guard |
| 输出字节 | ✅ | `command_executor.go:MaxOutputBytes` | `--max-output-bytes`, OOM guard |
| **Prompt Token** | ❌ | **完全不存在** | **无估算、无预算、无模型感知、无降级** |

Context Engine(三个文件)当前的工作方式:

```go
// prompt.go:86-90 Gather
// 三条 lane 各自独立拼接,没有任何总大小感知:
currentTask(repoRoot)   // lane 1: taskCap=4000 runes
relevantADRs(repoRoot, query)  // lane 2: adrTopK=6
constraints(repoRoot)    // lane 3: leadingBullets, ~6 lines
```

```go
// prompt_memory.go:48
const memoryCap = 32  // memory lane: 最多 32 条,固定值
```

```go
// prompt_artifacts.go (uses_template + emits 注入)
// 模板内容全部注入,无大小限制
```

```go
// prompt_context.go:295-420 buildPromptWithEmits
// 拼接所有 lane: card + gates summary + phase outputs + memory + context + artifacts
// 没有一步检查总长度或估算 token 数
```

**关键问题**:prompt 总大小 = (card text) + (ADRs × adrTopK) + (ROADMAP up to 4000 runes) + (AGENTS.md) + (memory up to 32 entries) + (gate results across all phases) + (phase outputs across feeds_forward phases) + (template content)。**没有任何代码评估这个总和是否超过目标模型的 context window。**

### 为什么需要

1. **这不是理论问题——Claude 3 Haiku 的 context window 是 8K token,Opus 是 200K**。当前系统对使用不同 model tier(explorer→Haiku vs cto→Opus)的 phase 注入相同的 prompt 上下文。Haiku 下 prompt 可能占 6K token 导致只剩 2K 给回复;系统毫不知情。
2. **Memory 随时间单调增长**。24h evolve loop 在第 50 轮迭代时 memory 可能有 200+ 条目(每轮 2-4 条),但 `memoryCap=32` 只取最近 32 条——这不是 token 预算,而是"最近 N 条"的计数器,且没有"如果 32 条超 budget 怎么办"的降级。
3. **ADRs 数量持续增加**。`adrTopK=6` 假定 ADR 标题足够短;当每个 ADR 正文也被注入时(v2),6 个完整 ADR 可能超过 10K token。
4. **不同 lane 的优先级不同**:ROADMAP(当前任务)比 memory(历史记忆)重要,memory 比 gate results(已检查过)重要。但当前所有 lane 是**平等拼接**的,没有优先级驱动的裁剪。

### 建议方向

**Phase A — 可见性(低投入,~200 行)**:
1. 在 `internal/prompt/` 增加一个 `EstimateTokens(text string) int` 函数——简单近似:`len(s)/4`(英文 token 的典型比例),用于快速估算,不依赖外部 tokenizer
2. 在 `prompt.go:Build()` 中,在所有 lane 拼接完毕后,估算总 token 数并 **log 一条警告**(`WARNING: estimated prompt %.0f tokens may exceed %s's context window (%dK)`, value, model, limit)
3. 触发点在 `buildPromptWithEmits`(prompt_context.go:410) 返回前。不影响任何现有行为,纯可观测性。

**Phase B — 预算约束(中等投入,~400 行)**:
1. 在 `asset.Phase` 或 `mode.ModePolicy` 中增加可选 `max_prompt_tokens` 字段(默认 0=无限制)
2. 在 `prompt.go:Build()` 中,估算总 token 数后,若超过限制则执行**优先级驱动的自适应截断**:
   - 保留顺序:ROADMAP(任务) > AGENTS.md(硬约束) > ADRs(架构决策) > agent card(角色) > memory(历史) > gate results(已过信息) > phase outputs(前序输出)
   - 每条 lane 从尾部截断,直到总估算 token 在限制内
   - 截断处添加 `[…{name} truncated from {N} to {M} tokens]` 标记
3. 预算来源:由 `routing.TierFor` 返回的每个 tier 的 context window 值(`Haiku=8K, Sonnet=16K, Opus=200K`),或 workflow YAML 的显式覆盖

**Phase C — 缓存感知的 token 节省(v2,高级)**:
- 利用 claude 的 `cache_control` API:在 prompt 的稳定前缀(ADRs + AGENTS.md + card)上打缓存标记,让 API 层自动复用,不必每次重新计费
- `ContextCache`(cache.go) 已经划好了"稳定前缀"的边界,但 v1 没有利用它做 API 缓存——这是未来工作

### 边界与诚实

- Token **估算绝对不等同于准确计费**:简单 `len/4` 对代码/表格/非英文文本可能偏差 2-3 倍。Phase A 的警告应有诚实标注"estimated, not exact"。
- 自适应截断**绝不改变 prompt 的语义正确性**:截断的是 memory 和 gate results 这类辅助信息,不是 ROADMAP 或 AGENTS.md。截断标记让 agent 知悉"记忆被裁剪了",不产生幻觉。
- Phase C 依赖 claude API 的商业特性,不应在 forge-core 零外部依赖原则下引入 claude SDK。`cache_control` 标记应该是 phase C 的**prompt 文本元数据**,在 `engine_build.go` 的 `claudeArgv` 层组装,不在内部包中。

---

## 方向二 · Agent 输出语义验证管线(超越形式门禁)

> **关键词验证**: `(semantic.*verif|output.*correct|compile.*check|behavior.*verif|contract.*check|acceptance.*criterion.*auto|output.*quality.*gate)`  
> → 已有文档提及"验证"时均围绕**形式闸门**(体积/layering/secret/governance)。**零篇**把"agent 输出的语义正确性验证"作为独立方向展开。

### 代码证据

现有闸门全部分类为"形式检查"——它们检查文件的样子,不检查文件的内容是否工作:

```yaml
# build.yml:62-63
harness-gates:
  required_gates: [test, app-test, arch, complexity, lint]  # ← 这些都是形式门禁
```

当前所有 gate 的职责:
- `gate.mjs` — 文件体积(name ≤ 500 lines, root ≤ 15 files)
- `arch-check.mjs` — 架构层叠、包扇入、循环依赖、函数长度、反模式命名、drift-guard
- `check.py` — YAML schema 完整性、workflow 引用可达性
- `secret-scan.mjs` — 硬编码 secret 检测
- `sca.mjs` — CVE 扫描(依赖已知漏洞)
- `test` / `app-test` — 运行现有测试套件

**这些 gate 没有一个问**:
- ❌ 新写的代码**编译通过**了吗?(Go: `go build ./...`, Node: `node --check`, Rust: `cargo check`)
- ❌ 新写的测试**确实测试了正确的行为**?(现有 `node --test` 只跑测试,不验证测试质量)
- ❌ API 的**输入/输出契约**是否被新实现违反了?(无 OpenAPI/spec diff)
- ❌ 实现是否**满足 PRD/issue 的验收条件**?(PRD 是自由文本,无结构化验证)
- ❌ 重构是否**没有引入回归**?(无行为 diff / snapshot testing)

### 为什么需要

1. **ForgeOS 的 vision 是"AI 写的代码,AI 验证"**。如果验证层面只停留在"文件不超过 500 行"和"没有循环依赖",那 AI 可以写出漂亮的分层结构但完全不工作的代码——**gate 全绿但产品坏了**。
2. **Reviewer phase 不是语义验证的替代品**。Reviewer 是 LLM-based 的,它的 APPROVE/REQUEST_CHANGES 是**主观判断**,不是客观验证。一个 reviewer 可以 approve 一个不编译的 PR,因为 LLM 的泛化能力让它"看懂"了代码意图但漏了语法错误。
3. **真点火已验证的痛点**:在 Sprint 24-26 的真 claude 集成测试中,agent 多次写出"看起来对但实际不编译"的代码。形式 gate 全过,但 `go build` 或 `python3 -c "import X"` 失败——这些失败被 gate 的 `test` 阶段(跑现有测试)捕获,但测试可能过时或不覆盖新代码。

### 建议方向

**注入一个全新的 gate 类别:编译/语义检查闸门**——位于 `harness/` 中,作为独立的 gate 名:

| 新 Gate | 检查内容 | 适用语言 |
|---------|---------|---------|
| `compile` | `go build ./...` / `node --check` / `cargo check` / `python3 -m compileall` | 全部 |
| `api-contract` | spec 文件(OpenAPI/Protobuf/gRPC)与实现的 diff | 有 spec 时 |
| `snapshot-test` | 对关键行为做黄金文件快照,检测意外行为变化 | 可选 |
| `acceptance-auto` | 从 PRD/issue 自动提取验收条件,验证实现是否满足(LLM-as-judge) | 可选 |

架构设计:
- 这些不是 gate.mjs 的内置物——它们是**新 gate 注册项**,像 `arch` 和 `test` 一样声明在 `policies.yml`、`required_gates` 中
- `compile` gate 零配置:自动检测项目语言(复用 `cmd/forge/detect.go` 的 `detectProject` 逻辑),选择对应编译命令
- `compile` 的 fail 分类:不编译→block(不能合并不编译的代码);编译但 warning→warn(最佳实践)
- `acceptance-auto` 是可选的,成本高(需要 LLM 调用),应在 `--mode engineering` 或 `--lifecycle production` 下才启用

### 边界与诚实

- **`compile` gate 是纯客观的**:`go build` 要么过要么不过。没有假阳性。
- **`api-contract` gate 需要 spec 文件**:如果没有 spec,gate 报告 N/A(not applicable)。与现有 gate 的 tri-state(PASS/FAIL/N/A)完全兼容。
- **`acceptance-auto` gate 有 LLM 成本**:应该有自己的 `--acceptance-auto-budget-usd` 控制,且默认关闭。它的输出是"MET/NOT MET",不是 gate FAIL——不阻塞 merge,只作为收敛信号的补充。
- **不替换现有测试**:`test` gate(跑现有测试)和 `compile` gate(确认编译通过)是互补的。一个代码可以通过编译但运行时报错;可以通过测试但编译不过(import cycle 没被测试覆盖)。

---

## 方向三 · Failed-Run 确定性重放与调试

> **关键词验证**: `(deterministic.*replay|reproduce.*fail|debug.*run|replay.*trace|determin.*prompt|reproduce.*iter|replay.*iter|debug.*evolve|retro.*debug)`  
> → 已有 docs/requirements 讨论"resume"、"checkpoint"、"rollback",但**零篇**聚焦于"如何精确重放一次失败的迭代来调试"。

### 代码证据

当前 trace 系统提供了良好的可观测性,但无法精确重现:

```go
// trace.go — Event 包含: Kind、Name、Status、DurationMs、CostUsdMicros、Model、Detail
// 但它不包含: 完整的 prompt 文本、memory 快照、gate 快照
```

resume 机制只能"从 checkpoint 继续",不能"重放历史":

```go
// persist/checkpoint.go:59-63
PhaseIndex int  // 下一 phase 索引
// 不存储: 该迭代使用的完整 prompt、memory 当时的全部内容、gate 状态快照
```

```go
// evolve.go:296-302 checkpointHook
// OnIteration hook 保存: checkpoint(iteration/mode/signals) + memory(append) + trace(event)
// 没有"将该迭代的完整上下文打包为可重放单元"的概念
```

**重放一个失败迭代需要回答以下问题,但当前系统无法回答任何一个**:

| 调试问题 | 当前能回答? | 为什么不能 |
|---------|------------|-----------|
| "给 agent 的 prompt 到底是什么?" | ❌ | trace 只存 `Detail`,不存 prompt body |
| "当时 memory 里有哪些条目?" | ❌ | memory 是累积的,下一次重放 memory 更多 |
| "当时 gate 的具体状态?" | ❌ | gate 结果在迭代间变化(文件已改) |
| "当时 ADR 缓存了什么?" | ❌ | `ContextCache` 是内存中的,进程死后丢失 |
| "当时 agent 的完整输出流?" | ❌ | `MaxOutputBytes` 只截断 stdout/stderr,覆盖了历史 |

### 为什么需要

1. **无法区分"代码 bug"和"prompt 质量 bug"**:如果 evolve 在第 7 轮写出错误代码,operator 无法判断是 agent 能力问题(代码 bug)还是 prompt 上下文不足(prompt bug)。需要精确重放才能归因。
2. **Reviewer 的 REQUEST_CHANGES 反馈无法回放**:reviewer 的裁决和 Findings 依赖 implementer 当时写的 what they reviewed——implementer 再次编辑后,原始代码消失。没有"当时代码快照"就无法验证 reviewer 的裁决是否合理。
3. **这是 24h 自治运行的信任基座**:如果 operator 无法在自己时间重放和理解一次失败,就永远不敢真正信任"无人值守"模式——只能相信 trace 日志,无法复现问题。

### 建议方向

**最小可重放单元 — Phase-level Replay Bundle**:

在每轮迭代的 checkpoint hook 中,额外打包一个**轻量级 replay bundle**:

```
.forge/replay/
  iter-7/
    prompt.txt            # 该迭代的完整 prompt 文本
    memory-snapshot.jsonl  # 该迭代开始时 memory 的快照
    context-cache.json     # ContextCache 当时的 adrDocs + cardText
    gate-results.json      # 该迭代的 gate verdic 快照
    phases/
      p1-planner/
        prompt.txt         # 单个 phase 的 prompt
        output.txt         # agent output (stdout+stderr)
        output-files/      # agent 修改/创建的文件快照
      p2-implementer/
        ...
```

**约束**:
- `replay/` 是**按迭代轮次编号的目录**,只保留最近 N 轮(默认 5)。每轮迭代 ~50-200KB 文本 → 5 轮 ≤ 1MB。
- **不做全量 git 快照**:`output-files/` 只记录 agent 实际修改过的文件(增量),利用 `risk_diff.go` 的 `FromChangedPaths` 原语。
- **不阻塞迭代**:replay bundle 是**异步写入的**(goroutine + 可丢弃的 channel buffer),写失败只 warn,不 abort loop。
- **重放工具**是 `forge replay --iter 7` 命令:读取 bundle → 重建 ContextCache → 重建 memory 快照 → **dry-run 重放 prompt** 给相同的 agent CLI(`--executor=command --agent-cmd claude -p "$(cat prompt.txt)"`) → 显示输出。

### 边界与诚实

- **Replay 不能保证完全相同的 LLM 输出**:即使 prompt 完全一样,LLM 的非确定性(采样温度>0)会导致不同输出。Replay 的价值在于**给 operator 看到当时 agent 看到的是什么**,而不是精确复现 agent 的输出。
- **存储成本**:50 轮 evolve × 200KB/轮 = 10MB。可接受。更老的条目自动被 prune(保留最近 5 轮 = 1MB)。
- **与 `--resume` 的关系**:replay bundle 是**只读历史**,不影响 recover。`--resume` 仍用 checkpoint.json,不用 replay bundle。
- **不做全量回放**(re-execute all phases with same prompts)——那等于重跑整个 run,是 v2 的"时间旅行"调试。v1 只做"给我看当时给 agent 发的是什么"。

---

## 方向四 · 多厂商模型路由——超越 Claude-only 池

> **关键词验证**: `(multi.provider|model.*vendor|provider.*abstract|model.*registry|non.claude|openai.*route|anthropic.*switch|model.*agnostic|vendor.*abstraction|provider.*interface)`  
> → 已有 docs 提及"multi-model"时均围绕**Claude 内部 tier 选择**(Haiku/Sonnet/Opus)或**未来路由服务**。**零篇**将"系统需要一个厂商无关的 Provider 抽象层"作为独立架构方向展开。

### 代码证据

当前路由层硬编码三个 Claude tier,且完全不包含 Provider 概念:

```go
// routing/routing.go:20-22
const (
    TierHaiku  = "haiku"  // claude-3-haiku
    TierSonnet = "sonnet" // claude-3-sonnet
    TierOpus   = "opus"   // claude-3-opus
)
```

```go
// routing/routing.go:79-93 TierFor
func TierFor(agent, mode string) string {
    // 返回 haiku/sonnet/opus 之一,永远只返回 claude 系的 model tier
}
```

```go
// engine_build.go:85-126 claudeArgv
// --model 的值直接传 tier(opus/haiku/sonnet),硬塞给 claude CLI
// 没有 provider 参数,没有 base URL,没有 API key 选择
```

```go
// routing/routing.go:107-113 opusFloorAgents
var opusFloorAgents = map[string]bool{
    "architect": true, "reviewer": true, "cto": true,
    // 这些 agent 必须用 opus -> 但这假设 provider 是 claude
}
```

**system 各处都隐含"provider=claude"的假设**:
- `cost.go:parseClaudeCostUsd` — 解析 claude JSON 的 `total_cost_usd`,假定输出格式是 claude 的
- `cost.go:classifyClaudeOverload` — 检测 claude 的 529 HTTP 状态码
- `engine_build.go` — `claude --model` / `--permission-mode` / `--allowedTools` 全是 claude CLI 专有
- `harness/scorecard-update.mjs` — scorecard 模型名是 claude tier

### 为什么需要

1. **这不是远期的"可能用 OpenAI"问题**——这是近期的真实需求:本地模型(`ollama run llama3`)、成本敏感场景下的 GPT-4o-mini(比 Sonnet 便宜 5 倍)、特定任务上的专用微调模型。
2. **厂商锁定是架构反模式**:`opusFloorAgents` 写死了"judgement-only agent 必须用 claude Opus"。但如果部署环境没有 claude API(内网/离线/成本限制),"安全下限"退化成"不可用"。
3. **scorecard 的学习闭环不跨厂商**:`scorecard.go` 的 `HistoryTiebreak` 按 model 排名,但 model 名字是 claude tier。引入 GPT-4o 后,scorecard 无法比较"claude Sonnet 和 gpt-4o 哪个更适合 planner"——因为 schema 没有 vendor 维度。
4. **`forge route` 的多维评分(complexity/dependency/context/business)已存在但只服务 CLI**:`route.go` 的 `Score()` 做了完整的多维评分,但它产出的是 tier。没有 Provider 层,这个评分只能路由到不同的 claude model,不能路由到不同的厂商。

### 建议方向

**Provider Abstraction 层 — 最小可行(v1)**:

在 `routing` 包中引入两个原子变更,不破坏任何现有行为:

```go
// new types (routing/types.go)
type Provider string
const (
    ProviderClaude  Provider = "claude"
    ProviderOpenAI  Provider = "openai"
    ProviderOllama  Provider = "ollama"
    ProviderCustom  Provider = "custom"
)

type ModelID string  // vendor-agnostic: "claude:sonnet", "openai:gpt-4o", "ollama:llama3"

type ModelInfo struct {
    ID           ModelID
    Provider     Provider
    Tier         string    // 保留向后兼容
    CostRank     int       // 1=cheapest ...
    ContextLimit int       // token
}

// routing/registry.go
type Registry interface {
    Resolve(agent, mode string, risk string, budgetRatio float64) (ModelID, error)
    CostOf(id ModelID) float64
    ProviderFor(id ModelID) Provider
}
```

**最小集成路径**:
1. 在 `routing` 包中增加 `ModelInfo` 和 `Registry` 接口——这是 **纯类型声明**,不依赖任何外部 SDK
2. 当前默认实现 `ClaudeOnlyRegistry`:返回与 `TierFor` 相同的结果,但封装为 `ModelID("claude:opus")`——行为**逐字节不变**
3. `cmd/forge` 中 `claudeArgv` 的调用处增加一个 `<->` 适配:如果 `ModelID.Provider == "claude"`,构造现有的 `claude --model` 参数;如果为其他 provider,构造不同的 CLI 参数
4. YAML 中 `model_tier:` 字段保持不变(不变 schema),但新增可选 `model_provider:` 用于指定厂商

### 边界与诚实

- **只做抽象层,不改任何已有路径**:`ClaudeOnlyRegistry` 是默认且唯一的实现,`forge run` / `forge evolve` 行为逐字节不变。只有 operator 显式配置厂商时才走多厂商路径。
- **不做多厂商的 cost parsing**:`parseClaudeCostUsd` 保持 claude-specific。其他厂商的 cost parsing 由各自 adapter 实现。
- **不做多厂商的 overload 检测**:`classifyClaudeOverload` 保持 claude-specific。其他厂商的 retryable 失败模式各自适配。
- **scorecard schema 扩展**:在 `scorecards.json` 中增加 `vendor` 字段(默认 `"claude"`),所有现有数据向后兼容。
- `opusFloorAgents` 保留但改为默认安全策略:当 provider=claude 时依然是 opus floor;当 provider=ollama 时,fallback 到最强大的可用模型。

---

## 方向五 · Memory 价值感知生命周期管理

> **关键词验证**: `(memory.*value|confidence.*weight|entry.*score|memory.*decay|knowledge.*retire|memory.*ttl|entry.*priorit|memory.*rank|value.*based.*compact|knowledge.*evict)`  
> → 已有 docs 讨论 memory 的"append-only"特性、"Supersedes"机制、"信息密度下降"问题,但**零篇**聚焦于"memory 应该是一个有优先级的价值层级,而不是扁平的 FIFO"。已有分析(2026-07-11-codegrounded-edge-cases-and-extensions.md 方向三)讨论的是信息密度下降的现象,本文讨论的是**治理架构**:memory 应该分层管理、价值感知、自动退役。

### 代码证据

`Confidence` 字段存在但零消费:

```go
// memory.go:75-80
// Confidence is an OPTIONAL caller-supplied signal (default 1.0)
type Entry struct {
    // ...
    Confidence    float64 `json:"confidence,omitempty"` // 0.0-1.0, default 1.0
    // ...
}
```

```go
// memory.go:293 - filterSuperseded
// 只过滤 Supersedes,不涉及 confidence
```

```go
// prompt_memory.go:155-201 memoryContext
// 注入逻辑:最近 32 条(recency-based),先过滤活跃 topic,再按 recency 倒序
// 完全不使用 Confidence,完全不使用来源(source),完全不使用价值评分
```

Compact 是纯计数-based:

```go
// memory_compact.go:24-43
// Compact 把同类条目合并为摘要,只保留最近 keepPerKind 条
// 不评估合并后的信息损失,不评估保留条目的价值
```

没有自动退役机制:

```go
// 没有任何地方:删除超过 N 天的条目、删除置信度低于阈值的条目、
// 删除来源是"implementer"(自报告)而非"reviewer"(独立判断)的低价值条目
```

### 为什么需要

1. **Memory 是一维扁平列表,但知识是分层的**:不是所有记忆同等重要。一条"系统架构使用了 PostgreSQL"的决策(来自 architect,高置信度)比一条"我在第 5 轮试了加索引"的记录(来自 implementer,低置信度)重要得多。当前系统平等对待二者。
2. **置信度字段已浪费了多个 sprint**:`Confidence` 字段从 Sprint 28 就存在(SCA 框架时期),但从未有实际消费者。这是最典型的"声明了但不用的架构债务"。
3. **随着 evolve 轮次增加,高价值条目被淹没**:在第 1 轮由 reviewer 发现的关键架构问题(置信度 0.95)在第 50 轮时已经排在 32 条 cap 之外——因为新条目全部是低价值的"迭代 N 完成了 task X"记录,把高价值历史记忆挤了出去。
4. **没有 TTL/退役,memory 只有增长没有收缩**:`Prune` 只被 `forge memory-prune` 手动触发,`Compact` 每 10 轮自动触发一次但只压缩同类条目不退役旧条目。一个 100 轮 evolve 的 memory 有 400+ 条目,但其中 80% 可能是已 superseded 或低信息密度的记录。

### 建议方向

**Phase A — Confidence 消费(低投入,~150 行)**:
1. `memory.Query()` 增加 `minConfidence` 参数: `func Query(entries []Entry, kind, topic string, minConfidence float64) []Entry`——调用方传 `0.8` 只拿高置信度条目
2. `prompt_memory.go:memoryContext` 的排序改为 `(confidence DESC, recency DESC)`——高价值条目优先保留,同一置信度内按新近度
3. 在 `memoryContext` 的 `cap 32` 选择中:按置信度降序排列后,超出 cap 的部分丢弃(不再按 recency 截断)
4. confidence 赋值:reviewer agent 的 entry → `0.9-1.0`, implementer 自报告 → `0.6-0.7`, 系统自动记录的迭代轨迹 → `0.5`(无判断价值)

**Phase B — 自动价值衰减与退役(中等投入,~300 行)**:
1. 在 `memory.Compact()` 中增加 `ValueScore(entry) float64` 函数:综合考虑 `confidence * (1 - age/ttl) * sourceWeight`
   - sourceWeight: reviewer=1.0, architect=0.9, planner=0.7, implementer=0.5, evolve-system=0.3
2. Compact 不再用"保留最近 N 条同类",而用"保留价值分数最高的 N 条同类"
3. 在 `recordMemory` 中(evolve.go:368)增加 TTL 退役:memory 中的 entry 超过 `maxAgeDays`(默认 30 天)自动跳过注入,不 delete(保持 append-only 安全),只在 Compact 时真正删除
4. `forge memory-prune` 改为 `forge memory-compact [--value-threshold 0.3]`,用价值阈值驱动清理而非固定计数

**Phase C — 知识层级(v3,高级)**:
- memory 的分层存储:hot(当前迭代相关,注入 prompt) / warm(近 5 轮,压缩后注入) / cold(全部历史,仅在检索时访问)
- 这个分层是 Context Engine v3 的范畴,不在此文展开

### 边界与诚实

- **Confidence 是 agent 自报告的,不是客观真理**:reviewer 的 0.9 置信度不代表它一定正确,只是系统"trust but verify"的机制——agent 的高置信度应被后续迭代的证据挑战(Supersedes 正是为此)。
- **不改变 memory 的 append-only 安全模型**:退役不是物理删除,而是在注入时跳过(除非 operator 显式 `forge memory-prune --hard`)。这保持了崩溃安全。
- **与 Supersedes 的关系**:Supersedes 是**语义上的"这个替代了那个"**;价值管理是**注入时的"空间只给最重要的条目"**。二者互补,不冲突。
- **向后兼容**:所有新功能 omitempty / default=1.0,现有 memory 文件逐字节不变。

---

## 优先级与执行建议

| # | 方向 | 优先级 | 投入评估 | 依赖 | 收益类型 |
|---|------|--------|---------|------|---------|
| 1 | Prompt Token 预算管理 | **P1** | Phase A ~200 行,Phase B ~400 行 | 无(纯 internal/prompt) | 防止静默劣化——24h 长跑中所有 phase 迟早触发 |
| 2 | Agent 输出语义验证 | **P1** | `compile` gate ~250 行,`acceptance-auto` ~600 行 | `detect.go` 语言检测(已存在) | 填补 vision 的核心空白——形式门禁不验证代码是否工作 |
| 3 | 确定性重放与调试 | P1 | ~500 行(记录器+重放工具) | trace 系统(已存在) | 建立对 24h 自治运行的信任——operator 必须能重现问题 |
| 4 | 多厂商模型路由 | P2 | 抽象层 ~300 行,不改变任何现有路径 | 无 | 解锁未来——厂商锁定是架构反模式,越早抽象成本越低 |
| 5 | Memory 价值感知管理 | P2 | Phase A ~150 行,Phase B ~300 行 | 无(纯 internal/memory) | 提升 memory 信息密度——高价值知识应长期可用 |

**执行顺序建议**:
- **Sprint N**:方向一 Phase A(可见性) + 方向五 Phase A(Confidence 消费) + 方向二 compile gate——三个低投入、高收益的快速赢
- **Sprint N+1**:方向三 replay bundle 记录器 + 方向四 Provider 抽象层
- **Sprint N+2**:方向一 Phase B(自适应预算) + 方向五 Phase B(价值衰减) + 方向二 acceptance-auto gate(评估)

**明确排除**(反镀金):
- 不做全量 prompt history 持久化(只保留最近 N 轮的 replay bundle)
- 不做全量 git 快照(增量文件 diff 就够了)
- 不做完整的 Temporal 工作流引擎(Provider 抽象只需接口,不需要分布式调度)
- 不做 Context Engine v3 的知识图谱(价值分层是 memory 层的改进,不是新引擎)
- 不做非 claude 厂商的 cost parsing(只做 abstraction,adapter 后续分别实现)
