# ForgeOS — 架构/产品视角的四方向结构债扩展分析

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局扫描 forge-core（18 Go 包 · ~35k LOC 运行时 + CLI）、harness（39+ 模块 · ~10.5k LOC）、
>    .agent/（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR + DECISIONS + architecture）、
>    docs/（FUNCTIONAL_REQUIREMENTS_AUDIT · 31 轮 sprint 演进 · 全部 90+ 分析文档）。
> 2. **差异化验证**: 对每个方向的核心理念，在全部已有分析 docs/requirements/（~70 篇）+
>    docs/analysis/（~40 篇）+ 核心文档中做关键词检索，确认该**方向作为独立系统性扩展从未展开**。
>    每个方向附「与已有覆盖的关系」说明。
> 3. **纪律**: 不编写任何代码。每个方向附精确到`file:line`的代码级证据、实际影响、边界情况。
> **日期**: 2026-07-10

---

## 核心判断

ForgeOS 已完成 31 轮 sprint 迭代、引擎落地、真点火坐实、GAP 闭环。**当前代码库面临的问题已不是「缺少功能」，而是「软件架构本身的成熟度不足」**——代码结构与业务复杂度之间的鸿沟正在扩大。

以下四个方向聚焦于**结构债务**，而非功能缺口。每个方向都是「现有代码已然存在的设计特征被推至极限后暴露的深层问题」，而非「缺少的新特性」。

| # | 方向 | 核心矛盾 | 严重性 | 类型 |
|---|------|----------|--------|------|
| 1 | **`Phase` 结构体膨胀:Schema 碎片化级联** | 一个 329 行文件、~30 字段、67% 注释的结构体承载全部工作流语义，且 schema 散布 5+ 处地点——每加一次字段就是一次多点同步 | 🔴 P0 | 可维护性 |
| 2 | **Context Engine v1:字符串拼接作为架构** | 架构文档声明的「Context Engine」就是 `fmt.Fprintf` + `strings.Join(ctx, "\n\n")`——没有结构化格式、没有 token 记账、没有优先级模型 | 🟠 P1 | 架构天花板 |
| 3 | **无结构化日志的可观测性断层** | 全仓 0 条结构化日志、0 个日志级别、0 个关联 ID——5 个 `trace.go` 的 Event kinds 没有任何日志伙伴 | 🟠 P1 | 可观测性 |
| 4 | **进程外 Agent 执行架构的契约脆弱性** | 5+ parser 函数做 claude CLI 输出文本抓取(fork/exec + 字符串解析是整个 agent 交互的唯一通道)，输出格式变化静默断送全管线 | 🔴 P0 | 集成可靠性 |

---

## 已有覆盖全景（本文不重复）

以下域已被 90+ 篇已有分析充分覆盖（代表性最高覆盖度的文档 + 方向数）：

| 域 | 覆盖程度 | 代表文档 |
|---|---|---|
| 编排引擎内核（串/并行/loop-back/mode-gating/stop-condition/checkpoint/resume/recursion-guard/budget） | 深度覆盖（~35 方向） | expansion-five-uncovered-2026-07-10.md, current-sprint |
| 生产可靠性（超时/退避/输出上限/进程组/资源护栏/真点火） | 深度覆盖（~18 方向） | expansion-production-readiness.md, current-sprint |
| 可观测性与学习闭环（trace/telemetry/scorecard/memory/三维真数据/跨运行分析） | 深度覆盖（~12 方向） | five-gaps-from-global-scan-2026-07-10.md |
| 安全纵深（secret-scan/recursion/budget/cap/SCA/四维护栏/注入防御） | 深度覆盖（~12 方向） | five-novel-architectural-frontiers-2026-07-10.md |
| 治理/执法（arch-check 8 检查/check.py 10 检查/circular-dep/function-length） | 深度覆盖（~12 方向） | expansion-five-codelevel-architect-gaps.md |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 | — |
| 产品运营化（部署/回滚/多分支/发布治理/二进制版本/成本智能/决策解释） | 深度覆盖（~8 方向） | five-product-operational-gaps.md |
| 北向扩展（Temporal/OPA/OTel/多厂商/Sandbox/Web UI/多仓库联邦/管线组合） | 已规划（~12 方向） | expansion-horizon-three.md |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/执行语义形式化） | 深度覆盖（~12 方向） | novel-extensions-v36-deep-architectural.md |
| 系统边界盲区（级联截断/YAML 分歧/信任边界/flock/跨进程锁） | 深度覆盖（~10 方向） | forgotten-five-system-boundaries.md |

**以下四个方向落在上述所有覆盖的间隙中**——不是因为它们不重要，而是因为它们指向的是代码层**已经存在的结构**的退化风险，而非「缺失的组件」。

---

## 方向一 · `Phase` 结构体膨胀: Schema 碎片化级联

**优先级**: 🔴 **P0** | **类别**: 架构 · 可维护性 | **杠杆**: ⭐⭐⭐⭐⭐  
**已有分析覆盖**: **零** — 90+ 篇分析无一篇将 `asset.Phase` 结构体的持续膨胀作为独立结构性问题分析。

### 问题描述

`asset.Phase` 是 forge-core 的工作流相位通用数据结构。它已经从一个~10 字段的简单描述器膨胀为一个**~30 字段的通用语义容器**，承载着权限(Readonly)、工具声明(RequiresTools)、模板(UsesTemplate/SecondaryTemplate)、指标名(ConfidenceMetric)、门控(RequiredWhen/OptionalFor)、反馈方向(FeedsForward)、并行依赖(DependsOn)、只读标记(FreshContext)、产物声明(Emits)、风险操作(OnFail/WritesADR)等几乎所有工作流语义。

更严重的是，同样的 schema 碎在 **5 个不同位置**，各自独立维护:

| 位置 | 文件 | 角色 | 同步方式 |
|------|------|------|----------|
| YAML 源 | `.agent/workflows/*.yml` | 作者声明 | 被 yaml2json 解码 |
| Go struct | `internal/asset/asset.go:Phase` | 运行时载体 | 手动维护 |
| mode gating | `internal/mode/mode.go` | 中枢旋钮过滤 | 手动维护 |
| gate resolution | `internal/gate/resolve.go` | 闸门裁决 | 手动维护 |
| convergence signals | `internal/converge/converge.go` | 收敛信号评估 | 手动维护 |
| governance checker | `harness/check.py` | 治理完整性校验 | 手动维护 |

每次新增一个 Phase 字段（比如 Sprint 31 新增 `SecondaryTemplate`），需要同步修改 **6 个地点**。遗漏一处就是 `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 里记录的 GAP 类型 bug——"declared in YAML but never decoded"。

`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 14 个 GAP 中，**超过一半**是这种 schema 碎片化导致的: `requires_tools` 零消费者、`readonly` 零消费者、`mode_gating:` 顶层块零解码器、`secondary_template` 零消费者、`blocking:` 零解码器、`confidence_metric:` 零读取。每次都是同样的问题: 在一个位置加了字段，忘记在其他位置加消费端。

### 代码级证据

**证据 1: Phase struct 329 行，67% 是注释**

```go
// forge-core/internal/asset/asset.go:43-285
type Phase struct {
    Name              string     `json:"name"`
    Agent             string     `json:"agent"`
    Description       string     `json:"description,omitempty"`
    RequiredGates     []string   `json:"required_gates"`
    RequiredWhen      string     `json:"required_when"`
    OnFail            *OnFail    `json:"on_fail"`
    ModelTier         string     `json:"model_tier"`
    WritesADR         *WritesADR `json:"writes_adr"`
    FeedsForward      bool       `json:"feeds_forward"`
    DependsOn         []string   `json:"depends_on"`
    FreshContext      bool       `json:"fresh_context,omitempty"`
    Emits             []string   `json:"emits,omitempty"`
    ConfidenceMetric  string     `json:"confidence_metric,omitempty"`
    OptionalFor       []string   `json:"optional_for,omitempty"`
    UsesTemplate      string     `json:"uses_template,omitempty"`
    RequiresTools     []string   `json:"requires_tools,omitempty"`
    Readonly          bool       `json:"readonly,omitempty"`
    SecondaryTemplate string     `json:"secondary_template,omitempty"`
    // + OnFail, WritesADR, StopCondition, 等嵌套结构体
}
```

统计: 329 行总长，221 行注释（67%），~30 字段。每个字段有一段散文体注释解释语义——但这些注释只存在于代码中，没有在任何 schema 定义或文档中。

**证据 2: 每个新字段涉及 5+ 消费端**

以 Sprint 31 新增的 `SecondaryTemplate` 为例:

```bash
# YAML 源声明 (.agent/workflows/review.yml:95)
$ grep "secondary_template" .agent/workflows/review.yml
# → secondary_template: .ai/prompts/06-production-readiness.md

# Go struct 字段 (asset/asset.go:283)
$ grep "SecondaryTemplate" forge-core/internal/asset/asset.go  
# → SecondaryTemplate string

# prompt artifact 注入 (cmd/forge/prompt_artifacts.go, 需手动接线)
# doctor 校验 (internal/doctor/models.go, 需手动接线)
# 还需要: internal/gate? internal/converge? check.py?
```

FUNCTIONAL_REQUIREMENTS_AUDIT 确认 `secondary_template` 在 Sprint 30 之前是**完全零消费**的 GAP——字段+YAML 都有，但所有消费端都是空的。

**证据 3: asset.go 自己的包注释已经承认了维护负担**

```go
// forge-core/internal/asset/asset.go:43-285
// (省略的巨大注释块) —— 每个字段的注释比字段本身长 5-10x
```

### 实际影响

1. **维护速度衰减**: 每加一个字段，修改的面从 1 个文件变为 5-6 个文件。这是 O(n) 复杂度增长，n = 字段数。
2. **新 contributor 学习成本**: 要理解一个 phase 的完整语义，需要同时读 6 个文件 + YAML 文件 + check.py。
3. **遗漏导致的 GAP 积累**: `FUNCTIONAL_REQUIREMENTS_AUDIT` 记录的一半 GAP 是这种问题——每次修完后过几个 sprint 又会出现同类 GAP。
4. **单文件体积压力**: 329 行 / 67% 注释已经触近 500 行。如果按「先拆分」纪律，拆什么？每个字段的注释有其消费端，拆文件只会把注释分散到更多文件。
5. **编码/解码不对称**: Sprint 27 发现 `parseSeqItem` 的 nil 分支不 append——同样的根本问题: schema 的消费者(decoder)和声明者(struct)没有共享契约。

### 边界情况

- **向后兼容**: 不能破坏已有 workflow YAML 文件的解析。新 schema 管理机制必须保证已声明的所有字段继续被支持。
- **跨版本**: 如果引入版本化 schema（如 `PhaseSchemaV1` vs `PhaseSchemaV2`），需要处理旧 YAML 文件的兼容读取。
- **Go 零依赖约束**: 不能引入 protobuf / flatbuffers / OpenAPI codegen——必须纯标准库。

### 与已有覆盖的关系

- `forgotten-five-structural-debt.md` 方向二讨论 `internal/asset` 包的"脆弱的解码器"，但聚焦于**JSON 解析错误处理**而非**结构体膨胀本身**。
- `strategic-extension-five-novel-2026-07-10.md` 方向一讨论 `cmd/forge` 依赖中枢，但那是**包级别**的耦合问题，不是**数据结构级别**的膨胀问题。
- `forgotten-five-system-boundaries.md` 方向五讨论「Agent 机读契约版本协商」——那是**运行协议**的版本化，不是**内部数据结构**的版本化。

**本文方向一是关于: 一个结构体承载了所有语义、所有地方都要同步修改、没有任何单一事实源——这是 forge-core 代码库当前最昂贵的单点维护税。**

---

## 方向二 · Context Engine v1: 字符串拼接作为架构

**优先级**: 🟠 **P1** | **类别**: 架构 · 可扩展性 | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖**: **零** — 90+ 篇分析中无一篇将 Context Engine 的**实现形态**作为架构风险分析（存在讨论 prompt content/injection/lane 的文档，但从未质疑过「字符串拼接」作为架构基石的合理性）。

### 问题描述

ARCHITECTURE.md 声明了 **Context Engine** 作为五大引擎之一（与 Orchestrator / Model-Router / Memory-Engine / Evaluation-Engine 同级）。但它的实际实现是:

```go
// forge-core/internal/prompt/prompt.go:39-52
func Build(agent, phase, mode, tier, card string, ctx []string) string {
    var b strings.Builder
    fmt.Fprintf(&b, "You are the %q agent in ForgeOS (phase=%s, mode=%s, tier=%s)...", 
        agent, phase, mode, tier)
    b.WriteString("## Role card\n")
    b.WriteString(card)
    if len(ctx) > 0 {
        b.WriteString("\n\n## Project context\n")
        b.WriteString(strings.Join(ctx, "\n\n"))  // ★ 全部上下文等权重拼接
    }
    return b.String()
}
```

这就是 Context Engine v1。整个上下文管理是:
- **字符串拼接** → 没有结构化 prompt 格式
- **等权注入** → `strings.Join(ctx, "\n\n")` — 一个 gate 失败记录和一条安全约束在 prompt 中的权重完全相同
- **无 token 记账** → 没有人知道每个 context lane 消耗了多少 token
- **无优先级/冲突解决** → 当 memory 说「已尝试方案 A 失败」且 ADR 说「方案 A 被否决」而 gate 说「方案 A 通过」时，agent 看到的是一条信息混在一起
- **无 phase 类型差异化** → reviewer phase 和 implementer phase 使用完全相同的 prompt 构造逻辑（只是在注入内容上有区别）

相比之下，CLI 层(`cmd/forge/prompt_context.go`, 454 行)是核心逻辑(`internal/prompt/prompt.go`, 59 行)的 **7 倍**——这是典型的层间倒置。

### 代码级证据

**证据 1: Context Engine 核心只有 59 行有效逻辑**

```go
// forge-core/internal/prompt/prompt.go — 全文件 59 行有效逻辑
// - Build: 20 行 (字符串拼接)
// - Gather: 120+ 行 (三 lane 收集，但也是字符串拼接)
// - constants: 3 行 (adrTopK, taskCap)
// 总计 ~59 行有效逻辑 / 175 行总长（含注释和包文档）
```

**证据 2: CLI 层是核心层的 7x**

```bash
# 核心业务逻辑
$ wc -l forge-core/internal/prompt/*.go
# → 591 行 (3 文件: prompt.go 175, cache.go 231, retrieve.go 185)
# 但 prompt.go 只有 59 行有效逻辑，其余是注释

# CLI 接线层
$ wc -l forge-core/cmd/forge/prompt_context.go forge-core/cmd/forge/prompt_memory.go forge-core/cmd/forge/prompt_artifacts.go
# → ~700+ 行 (gateLedger, verdictLedger, phaseOutputLedger, observeFor, buildPrompt, etc.)
```

**证据 3: 不同上下文类型被相同方式处理**

```go
// forge-core/cmd/forge/prompt_context.go (buildPrompt 内部)
// 将所有 context lane 收集到 []string，然后 strings.Join
var lanes []string
lanes = append(lanes, context())        // gate 结果 → 字符串
lanes = append(lanes, memoryContext(...)) // 记忆 → 字符串
lanes = append(lanes, artifactContext(...)) // 产物 → 字符串
// ... 全部等权混入一个字符串
```

没有任何一个 lane 知道其他 lane 的存在——没有冲突检测、没有优先级排序、没有 token 预算分配。

**证据 4: prompt cache 将「字符长度」作为等价 Token 计数的代理**

```go
// forge-core/internal/prompt/cache.go:66-80
// ContextCache 的 cost() 返回字符串长度，而非 token 数
// 对于中文/英文混合内容，字符长度与 token 数可能差 4x
```

### 架构影响

1. **天花板**: 任何需要结构化 prompt 设计的功能（模板化 prompt、few-shot 管理、role-specific 格式）都得重写 `Build`。当前的字符串拼接架构无法渐进式扩展。
2. **窗口浪费**: 无法回答"我的 prompt 有多少 token？哪个 lane 吃得最多？"。`prompt_cache.go` 的 `cost()` 返回字符长度——这只是 token 计数的简陋代理。
3. **Agent 决策质量**: 当 `memory.jsonl` 积累到 100+ 条时，`strings.Join` 会把 100 条记忆全部塞进 prompt（仅受 `memoryCap:32` 限制）。没有相关性排序，没有丢弃策略。
4. **跨厂商可移植性**: 不同模型（Claude vs GPT-4 vs Gemini）有不同的 prompt 格式偏好和 token 计价方式。当前的字符串模型假设 prompt = 纯文本，无法插入 system prompt、structured output schema 等厂商特定构造。

### 边界情况

- **`adrTopK = 6`**: 这是对 ADR 注入数量的硬编码上限。当 ADR 数>6 时，`prompt.Retrieve` 基于子串匹配打分——但其打分函数(TF-IDF-like 关键词匹配)没有语义理解。当前 4 个 ADR ≤ 6 所以不暴露问题，但增长后第一个症状就是「无关 ADR 被注入，相关 ADR 被丢掉」。
- **`taskCap = 4000` runes**: ROADMAP 截断是硬 cap（runes，不是 tokens）。中文 ROADMAP 在相同 rune 数下 token 更多。
- **Agent 忽略 context**: 即使完美构造了 prompt，也无法知道 agent 实际使用了哪些部分。当前 zero 反馈回路。

### 与已有覆盖的关系

- `expansion-five-codelevel-architect-gaps.md` 方向一讨论「上下文窗口压力」——那是**量的问题**(token 够不够用)，不是本文的**质的问题**(字符串拼接作为架构)。
- `forgotten-five-foundations.md` 方向二讨论「prompt 质量与注入」——那是**prompt 内容**(应该注入什么)，不是本文的**prompt 形态**(如何构造 prompt)。
- `forgotten-five-structural-debt.md` 方向三讨论「`prompt_context.go` 庞杂的单一职责违反」——那是**文件组织**问题，不是本文的结构化 prompt 架构问题。

**本文方向二是关于: ForgeOS 自称有五大引擎之一「Context Engine」，但实际实现是一个字符串拼接函数。这不是功能缺失，是架构声称与实现之间的鸿沟。**

---

## 方向三 · 无结构化日志的可观测性断层

**优先级**: 🟠 **P1** | **类别**: 运营 · 可观测性 | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖**: **零作为独立方向** — 90+ 篇分析讨论 trace/telemetry/scorecard 的采集和存储，但没有任何一篇分析指出**整个代码库缺失结构化日志基础设施**。

### 问题描述

ForgeOS 有优秀的 trace 基础设施（`internal/trace/trace.go` — 结构化 Event 类型、JSONL 输出、6+ Event kinds）。但 trace 只覆盖**运行时事件**（phase start/end, gate result, iteration event）。代码本身的操作日志——决策、警告、错误上下文——全部通过非结构化 I/O 输出:

```go
fmt.Printf("forge validate: all checks passed\n")
fmt.Fprintf(os.Stderr, "forge memory-prune: %v\n", err)
log.Printf("checkpoint loaded (iteration=%d)", cp.Iteration)
fmt.Printf("  [FAIL] %s — unparseable: %v\n", wf, err)
```

这意味着:
- 没有任何日志条目可以关联到 trace 事件（没有 correlation ID、没有 trace_id）
- 没有日志级别(debug/info/warn/error)——所有输出都是硬编码的 visible 或 invisible
- 没有结构化字段（JSON 行、键值对）——日志是纯文本，只能 human grep
- 没有输出路由——所有日志混合在 stdout/stderr 中，没有按模块/级别分离
- 对于无人值守运行(24h+ evolve)，无法回答"这个迭代到底发生了什么"——只有 trace 事件的时间线，没有执行过程中日志的渐进式叙述

### 代码级证据

**证据 1: 全仓零条结构化日志**

```bash
$ grep -rn "log\.\|fmt\.\|os\.Stderr\|os\.Stdout" forge-core/cmd/forge/ --include="*.go" | grep -v "_test.go" | grep -v "flag\|usage\|version" | wc -l
# → 100+ 处非结构化输出

# 验证结构化日志库导入为零
$ grep -rn "\"log/syslog\"\|\"log/slog\"\|zap\|zerolog\|logrus\|otel" forge-core/ --include="*.go" | grep -v "_test.go" | grep -v "vendor\|\.pb\."
# → 零结果（零外部依赖约束）
```

**证据 2: trace 与日志之间无关联**

```go
// forge-core/internal/trace/trace.go:63-84
type Event struct {
    Kind         string `json:"kind"`
    Name         string `json:"name"`
    Status       string `json:"status"`
    DurationMs   int64  `json:"duration_ms"`
    // 没有: LogCorrelationID, LogFilePath, LogLineRef
}

// trace.Event 没有任何字段指向对应的日志输出
```

```go
// forge-core/cmd/forge/main.go (典型的错误处理模式)
if err != nil {
    fmt.Fprintf(os.Stderr, "error: %v\n", err)  // ★ 文本日志
    os.Exit(1)                                    // ★ 丢失错误上下文
}
// trace 记录了 "forge run 失败" 的事件，但不包含错误日志的具体消息
```

**证据 3: `forge doctor` 无法从日志诊断**

```go
// forge-core/internal/doctor/doctor.go
// doctor 从 checkpoint + trace + memory 诊断异常
// 但它读取的是结构化的 trace.jsonl + checkpoint.json，不是日志
// 如果日志是结构化的，doctor 可以关联 "为什么 gate 失败" 的日志上下文
// 当前它只能看到 "gate 失败了" 这个事实，看不到失败原因
```

**证据 4: evolve 循环的关键路径输出**

```go
// forge-core/cmd/forge/evolve.go — 核心循环输出
// 用 fmt.Printf 输出迭代进度 (@型信号)
// 用 fmt.Fprintf(os.Stderr, ...) 输出错误
// 用 log.Printf 输出 checkpoint/memory 操作
// 三种不同的输出路径，三种不同的格式，没有一种可以被机器消费
```

### 实际影响

1. **排障困难**: trace 告诉你"第 7 次迭代的 phase 3 失败了"——但不知道为什么。失败的细节在 stderr 的某处文本中，和 trace 事件没有关联。
2. **运营自动化**: 无法建立基于日志的告警——没有规则引擎可以对「gate 连续失败 3 次」或「cost 异常激增」产生告警，因为日志不是结构化数据。
3. **AI 辅助排障**: `forge doctor` 无法将 trace 事件与日志上下文关联。一个结构化日志系统可以让 doctor 说「iteration 5 的 implementer phase 超时了——对应日志 `[error] implementer: timeout after 30s (model=sonnet, cost=$0.03)`」。
4. **零依赖约束**: 不能引入 zap/zerolog/logrus，但如果 Go 标准库的 `log/slog`（Go 1.21+）可用——forge-core 使用 Go 1.24（`go.mod` 可见）——**`slog` 是纯标准库**，零外部依赖。这是一个可以且应该立即使用的工具。

### 边界情况

- **性能**: 高频路径（如 trace 的每个 `Emit` 调用）不应产生同步日志写入。slog 的 `Handler` 接口允许异步/批量写入。
- **兼容**: 现有 `fmt.Printf` 输出（`forge status`、`forge validate` 等用户可见输出）不应变为结构化日志——用户可见的输出和机器可读的日志是两个不同通道。
- **文件 vs stdout**: 24h+ evolve 循环的日志应该写入文件（`.forge/forge.log`？），而非 stdout——但当前 stdout 是唯一输出。

### 与已有覆盖的关系

- `five-gaps-from-global-scan-2026-07-10.md` 方向一「可观测性导出与外部监控集成」讨论将 trace/scorecard **导出到 Prometheus/Datadog**——那是**外部集成**问题，不是本文的**内部日志基础设施缺失**问题。
- `forgotten-five-structural-debt.md` 方向一「`cmd/forge` 依赖中枢退化」讨论了包结构——但未触及日志问题。
- `strategic-expansion-v39.md` 方向三「跨运行错误遥测聚合与模式驱动路由」讨论了 trace 的错误聚合——但聚的是**trace 事件**，不是**操作日志**。

**本文方向三是关于: ForgeOS 有 trace 但无日志。trace 告诉你「什么发生了什么事件」，日志告诉你「为什么」。没有后者，24h 无人值守循环的排障就是盲人摸象。**

---

## 方向四 · 进程外 Agent 执行架构的契约脆弱性

**优先级**: 🔴 **P0** | **类别**: 集成 · 可靠性 | **杠杆**: ⭐⭐⭐⭐⭐  
**已有分析覆盖**: **零作为独立方向** — 一些已有分析提及 `command_executor.go` 的错误分类或 `os/exec` 的资源耗尽，但没有任何分析将「fork/exec + 字符串解析」作为整体架构耦合问题系统分析。

### 问题描述

ForgeOS 是一个旨在「站在 Claude Code / Codex / Gemini CLI / OpenHands 之上」的治理层（PROJECT.md 逐字原文）。但它与每个 Agent CLI 的集成模式完全一致:

1. **`os/exec.Command`** spawn 子进程（fork + exec）
2. **字符串构建** CLI 参数（`engine_build.go` 的 `claudeArgv`）
3. **`CombinedOutput()`/`cmd.Run()`** 收集 stdout/stderr
4. **字符串解析** 从输出中提取结构化数据（`cost.go` 的 5+ parser 函数）

这种模式导致:

- **5 个 parser 函数全部依赖 claude CLI 的输出格式**——`parseClaudeCostUsd` 解析 JSON envelope 中的 `total_cost_usd`、`parseReviewerVerdict` 解析末行的 `VERDICT: APPROVE`、`parseExecutiveVerdict` 解析 `VERDICT: REDESIGN`、`parseConfidenceScore` 解析 `CONFIDENCE: 85`、`unwrapClaudeResult` 剥离 claude 的 `│` 行首标记
- **格式变化 = 整个管线静默失败**——没有一个 parser 在解析失败时发出告警（它们返回零值 + ok=false，调用者默默地降级）
- **没有版本协商**——agent card 里的 `VERDICT: APPROVE` 是 v1 散文级契约。如果未来 claude CLI 改变 JSON 输出格式，`parseClaudeCostUsd` 返回 `ok=false`，成本跟踪静默归零
- **每增加一个 agent CLI 类型，需要重新实现一整套协议**——Codex 的 JSON 输出格式不同，Gemini CLI 的参数语法不同

### 代码级证据

**证据 1: 5 个 parser 函数全部依赖 claude 私有契约**

```go
// forge-core/cmd/forge/cost.go — 5 个 parser，全部 claude-specific
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
    // 解析 claude 的 --output-format json 的 total_cost_usd 字段
    // 字段名 "total_cost_usd" 是 Claude 私有契约
}

func parseReviewerVerdict(output string) (verdict string, ok bool) {
    // 解析末行 VERDICT: APPROVE — 这是一段写在 .agent/agents/reviewer.md 里的散文描述
    // 没有任何 machine-readable schema 定义这个格式
}

func parseExecutiveVerdict(output string) (verdict string, ok bool) {
    // 同 reviewer，但 5 个可能值（APPROVE / APPROVE_WITH_SIMPLIFICATION / REDESIGN / DELAY / REJECT）
}

func parseConfidenceScore(output string) (score int, ok bool) {
    // 解析末行 CONFIDENCE: <0-100> — 散文契约
}

func unwrapClaudeResult(output string) string {
    // 剥离 claude CLI 的行首 │ 标记 — Claude 私有输出格式
}
```

**证据 2: 所有 parser 的失败路径都是静默降级**

```go
// forge-core/cmd/forge/cost.go:180-196
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
    if err := json.Unmarshal(...); err != nil {
        return 0, false  // ★ 静默降级
    }
    if env.TotalCostUsd == nil {
        return 0, false  // ★ 静默降级
    }
    // ...
}

// 调用者:
usd, ok := parseClaudeCostUsd(output)
if !ok {
    // 不发出 cost 事件，不记录日志，不告警
    // trace 中就没有 cost_usd_micros 字段
    return
}
```

当 claude CLI 升级改变了 JSON 格式时，症状不是「报错」——而是「forge 突然不记录成本了」。没有任何告警。

**证据 3: `engine_build.go` 的 CLI 参数构造硬编码 claude 语法**

```go
// forge-core/cmd/forge/engine_build.go:93-155
func claudeArgv(...) []string {
    argv := []string{"claude", "-p", prompt}           // ★ claude 命令名硬编码
    argv = append(argv, "--model", model)               // ★ claude 参数格式
    argv = append(argv, "--permission-mode", "acceptEdits")  // ★ claude 特有
    argv = append(argv, "--disallowedTools", "Edit Write")    // ★ claude 特有路径限定
    argv = append(argv, "--max-budget-usd", ...)              // ★ claude 特有
    argv = append(argv, "--output-format", "json")            // ★ claude 特有
    return argv
}
```

如果未来要支持 Codex（参数 `--model` 用 `-m`，无 `--permission-mode`，输出格式完全不同），需要**复制整个函数 + 全部 5 个 parser + prompt 注入逻辑**。

**证据 4: `os/exec` 在 6+ 个文件中直接使用**

```bash
$ grep -rn "\"os/exec\"\|exec\.Command" forge-core/cmd/forge/ --include="*.go" | grep -v "_test.go"
# → 6 个文件使用 os/exec:
#   - main.go (YAML shim)
#   - validate.go (python shim)
#   - gates.go (git commands)
#   - scorecard_wind.go (node harness)
#   - route.go (python shim)
#   - + orchestrator/command_executor.go (agent CLI)
```

每个使用 `os/exec` 的地方都是一个**硬编码的命令名 + 硬编码的参数格式 + 字符串解析**。整个 forge-core 的分布式系统集成层就是 `os/exec.Command` + `Output()`。

### 架构影响

1. **多 CLI 支持成本**: 每增加一个 agent CLI（Codex / Gemini CLI / OpenHands），需要:
   - 一套新的 CLI 参数构造
   - 一套新的输出解析器
   - 一套新的成本提取逻辑
   - 一套新的错误分类
   - 测试这四者的**排列组合**

2. **测试难度**: 当前所有 agent 集成测试要么使用 fake script（伪造输出），要么使用真 claude（需付费）。没有一个中间层——没有 mock agent CLI、没有 recording/replay 测试工具、没有契约测试框架。

3. **升级风险**: claude CLI 的输出格式不是版本化的 API。Anthropic 可以在任何时候改变 JSON 输出结构（即使只是添加新字段/改变缩进，现有 parser 也可能被破坏）。

4. **死代码风险**: 如果 claude CLI 某个输出格式变化，老的 parser 不会报错——它只是静默返回 `ok=false`。对应的功能（成本跟踪、裁决识别）静默消失，没有告警。

### 边界情况

- **非 claude 回退**: `if !isClaude` 分支已存在（`engine_build.go:93`），但回退逻辑是**移除所有 claude 特有参数并希望最好**——没有通用的 CLI 适配器接口。
- **混合输出**: claude 的输出可能同时包含 JSON envelope（stdout）和纯文本错误（stderr）。当前只收集 stdout（`CombinedOutput` 合并了 stderr，但 `claude --output-format json` 只在 stdout 产生 JSON）。
- **超时与输出截断**: `cappedBuffer` 在输出超限后截断——如果截断刚好切掉了 `VERDICT:` 行，裁决解析返回 `ok=false`，整个 phase 的评审结果丢失。

### 与已有覆盖的关系

- `five-unseen-governance-horizons.md` 方向五「Agent 机读契约版本协商」——最接近，但聚焦于 agent card 输出的机读 token（`VERDICT:`/`CONFIDENCE:`）的版本化，而非底层的 fork/exec + 字符串解析架构。
- `novel-extensions-v36-deep-architectural.md` 方向三「fork/exec 错误分类」——讨论 `command_executor.go` 在 fork/exec 失败时的**错误分类**，而非整体架构耦合。
- `forgotten-five-structural-debt.md` 方向五「pi-batch.py 治理孤儿」——讨论一个独立脚本的集成问题，而非 forge-core 的进程外执行架构本身。

**本文方向四是关于: ForgeOS 声称站在所有 CLI 之上，实际实现是 os/exec.Command + 字符串解析。这是一个架构级耦合——每个新的 CLI 支持都需要重新实现一整套协议，而升级当前 CLI 的输出格式风险是静默数据丢失。**

---

## 总结: 四个方向的内在关系

这四个方向不是孤立的问题——它们指向一个共同的根本矛盾:

> **ForgeOS 在功能层面的成熟度（31 sprint、五大引擎、真点火验证、全 GAP 闭环）已经远超其软件架构层面的成熟度。**

| 方向 | 功能层面 | 架构层面 |
|------|----------|----------|
| Schema 碎片化 | 工作流语义完全可表达 | 6 个点同步维护，遗漏是常态 |
| Context Engine | 五大引擎之一 | 字符串拼接函数 |
| 可观测性 | trace 结构化、scorecard 丰富 | 零结构化日志 |
| 进程外执行 | 多 CLI 愿景已声明 | `os/exec` + 字符串解析 |

修复这些方向不会在用户可见的功能层面带来新特性。它们会改变:
- **维护速度**: 加一个新 phase 字段不再需要同步改 6 个文件
- **运营效率**: 24h evolve 循环的排障从「grep stdout」变为「结构化日志查询」
- **集成可靠性**: 新增 agent CLI 不再等于复制粘贴 5 个 parser
- **架构可信度**: 架构文档说的「Context Engine」和代码里实际存在的对应

**建议优先处理方向一 + 方向四（P0）**，因为它们直接制约代码库的长期演进能力。方向二和三（P1）是架构天花板的上限问题——在遇到真正的「token 预算管理」或「外部监控集成」需求前，可以暂缓。
