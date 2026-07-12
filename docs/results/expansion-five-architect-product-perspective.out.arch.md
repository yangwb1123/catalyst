# 架构师评审：ForgeOS 结构债与扩展方向分析

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 当前架构在同类系统中具有显著的**设计成熟度**：

**优势一：中枢旋钮（`mode × lifecycle`）是极好的架构抽象。**
一个设置同时驱动 Router 档位、Harness 严格度、Workflow 深度三处——这是 Kubernetes 式「声明式控制面」思想的正确落地。从 Sprint 5 的初始实现到 Sprint 15 的完整七维度，它经受住了 26 个 sprint 的演进而不崩塌，证明抽象边界选得对。

**优势二：带外执法（out-of-band enforcement）是最正确的载重墙决策。**
`BOOTSTRAP.md` 明确说「真相之源 = 带外执法层，host-independent」——这避免了 ForgeOS 被宿主 CLI 的能力天花板绑架。gate.mjs → arch-check → acceptance.mjs 的分层执法链，与 north-star 架构的 PDP/PEP 分离理念一致，v0→v2 不返工。

**优势三：纯标准库零依赖的决心在 v2 阶段正确。**
Go 运行时 17 个包、`go.mod` 无 `require`——这在 v2 阶段（核心循环验证期）是对的：每引入一个外部依赖都是需要论证的架构决策，而非默认行为。YAML 解析用 python shim 是诚实的临时方案，没有为「看起来专业」而仓促引入 `go-yaml`。

**优势四：31 轮 sprint 积累的架构纪律已经制度化。**
`FUNCTIONAL_REQUIREMENTS_AUDIT.md`、arch-check 8 检查、函数≤50 行/循环依赖=0 的机器执法——这些不是一次性审计，而是持续的架构治理。Sprint 27 的多 agent 并行拆分表明架构能自我纠正。

### 1.2 当前架构的关键局限性

**局限一：`asset.Phase` 的单体膨胀是架构债务的典型信号。**
一个 329 行、~30 字段、67% 注释的 struct 承载全部工作流语义——这正是 `AGENTS.md` 红线试图避免的「God Object」的雏形。更严重的是，同样的 schema 碎片在 5 个地点各自维护（YAML / Go struct / mode gating / gate resolution / convergence signals），每新增一个字段需要 6 处同步。这不是「需要重构」，这是**架构抽象层缺失**——缺少一个「phase schema 的权威定义 + 代码生成器」。

**局限二：Context Engine 的实际实现与架构文档的声明之间存在断层。**
架构文档说「Context Engine as a Service: 装配 + RAG + token 预算」——而代码层 `prompt_context.go` 的实际实现是 `fmt.Fprintf` + `strings.Join(ctx, "\n\n")` + 手动 `len(text)/4` token 估算。这不是「v0 可以接受的简化」，而是一个**架构承诺未被跟踪兑现**的案例。Context Engine 是 five-north-star 的命名引擎之一，但 sprint 记录中它从未被作为独立方向治理过——它只是随着需求逐轮被「顺便加一点」。

**局限三：进程外 Agent 执行架构的契约脆弱性是运行时最危险的单点故障。**
当前 forge-core 与 Claude Code 的交互完全依赖 fork/exec + 5 个独立的文本解析函数（`parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`、`unwrapClaudeResult`、cost.go 中 claude JSON 的解析）。没有一个结构化的输出协议——Claude Code 的某个版本升级改了输出格式，全线崩溃且完全静默（没有 schema 校验）。这不是「可改进的」，这是**架构层级的缺陷**：for-cli 交互应该有结构化输出模式（`--output-format json` 已被部分使用但未被规范化为契约）。

**局限四：Trace 系统具备 emit 但无 readback 聚合 API。**
`internal/trace` 写 `trace.jsonl`、`internal/persist` 写 checkpoint、`internal/memory` 写经验——但三者之间没有任何跨运行的聚合查询层。每次 `forge run`/`forge evolve` 产生的数据是孤岛。Learning loop 从 scorecard 学，但不从 trace 学。这是**数据飞轮的断链**——架构文档声明的「随使用增长的记分卡/模板/策略数据飞轮」的「飞轮」部分尚未闭环。

### 1.3 关键设计决策评估

| 决策 | 评估 | 建议 |
|---|---|---|
| **v2 起步用纯 Go 零依赖** | ✅ 正确——在核心循环验证期避免依赖决策的沉没成本 | 现在 v2 已稳定（31 sprints），可以开始有选择地引入依赖（如 `go-yaml`），每一笔需要 ADR 论证 |
| **进程外 Agent 执行（fork/exec）** | ⚠️ 正确但应升级——v0→v2 的第一正确选择（隔离性），但输出解析层的脆弱性已被 31 轮 sprint 反复暴露 | 参见第三部分建议 |
| **中枢旋钮（mode×lifecycle）作为唯一配置入口** | ✅ 极其正确——避免了配置爆炸（Sprint 31 的 `blocking: false` 案例证明其他配置路径容易变成死字段） | 坚持这一原则，新配置必须先过「能否融入中枢旋钮」的检查 |
| **YAML 经 python shim 转码** | ⚠️ 正确的临时方案，但已成为长期依赖 | 建议在 v2 中期（当前）正式引入 Go YAML 库。python shim 是 sprint 中真实损坏过的组件（Sprint 27 的 `block-scalar 损坏` bug） |
| **CLI 命令直接 shell 出 harness 工具** | ⚠️ 正确但不可持续——`forge run` 直接 shell 出 `gate.mjs`/`check.py`/`acceptance.mjs` 意味着 Node/Python 是 forge-core 的运行时依赖 | 中长期应编译 harness 为 Go 静态二进制（north-star 已规划），短期确认是否将 `harness/` 的 Node/Python 代码作为 forge-core 的正式依赖 |

### 1.4 架构债务与技术债

**已识别的结构债（按严重性排序）：**

1. **🔴 Phase Schema 碎片化**：~30 字段 × 6 处维护 — 这是最高优先级的结构债，因为它每次新增字段都产生 GAP 型 bug
2. **🔴 Context Engine 实现与声明的断层**：架构文档有一个 Context Engine，代码层是一堆 `strings.Join` — 需要决定是升级实现还是降级文档
3. **🟠 进程外契约脆弱性**：5 个独立解析函数，无 schema 验证，无版本协商
4. **🟠 Trace 数据孤岛**：emit 有，聚合无 — 数据飞轮缺后半段
5. **🟠 cmd/forge 包文件数持续承压**：从 14→16→17，每个 sprint 都在顶预算 — 暗示包边界可能不合理
6. **🟡 Python YAML shim 成为隐含依赖**：没有它在 forge-core 无法解析自己的配置文件
7. **🟡 `internal/doctor` 的测试覆盖率缺口**：Sprint 27 才补的测试，说明「拆出新包后补测试」不是自动流程

---

## 2. 扩展方向

基于架构评估，我识别出以下 5 个高价值扩展方向。与已有分析文档的差异化在最后附注。

### 方向一：Phase Schema 权威化与代码生成（P0）

**为什么需要（业务价值/技术价值）：**
这是**结构债的源头**。每次新增 Phase 字段需要 6 处同步修改，`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 14 个 GAP 中超过一半源于此。这不是「重构」，而是**消除一类 bug 的根源**。业务上：每次快加特性时「忘了同步一处」导致静默功能缺失，侵蚀信任。

**核心挑战和技术难点：**
- 需要从 YAML schema 生成 Go struct + mode gating + gate resolution + convergence 的同步代码
- YAML schema 是当前唯一权威源（`.agent/workflows/*.yml`），需要将其格式化为机器可读的 schema 定义
- 不能破坏既有的手动同步模式——需要一个过渡期（code generation + 手动可覆盖）
- Go 标准库无 YAML 支持，code generation 需要先解决 YAML 解析问题

**预期的架构变更：**
- 新增 `internal/phaseschema/` 包：Phase schema 的权威定义 + 代码生成器
- YAML 中 Phase 声明变为引用 schema 定义而非内联全部字段
- `harness/check.py` 新增 schema drift 检测（vs 已有 `check_mode_priorities` 模式）

**对现有系统的影响：**
- **低中断**——code gen 在 Build 时运行，不影响运行时路径
- **高增益**——消除一类已知的 bug 模式
- 需要 1-2 个 sprint 完成 schema 定义 + code gen + 迁移过渡

### 方向二：Context Engine 结构化（P1）

**为什么需要：**
当前 Context Engine 是 `prompt_context.go` 中 `strings.Join(ctx, "\n\n")` 的线性拼接——没有结构化格式、没有 token 优先级预算、没有内容分片。这是架构文档声明的「Context Engine」和实际实现之间最大的断层。随着 Agent 需要注入的内容增多（多个 workflow phases、scorecard、memory、gate 裁决、前 phase 产物），无结构拼接会导致：

1. Token 预算不可控 — 你不知道哪些内容被截断了
2. 优先级不可表达 — 人审裁决和 gate 结果同等权重
3. 无法调试 — 无法回答「为什么这次 injection 没有包含某条信息」

**核心挑战：**
- 需要一个分层 Token 预算模型（必需层 / 高价值层 / 尽力而为层）
- 需要在保持向后兼容的前提下引入「Context 装配器」（Context Assembler）——不能一次性重写全部 prompt 构建路径
- `buildPromptWithEmits`（`prompt_context.go:338`）有 9 条独立的注入路径——它们各自有不同的优先级语义，需要统一

**预期的架构变更：**
- 新增 `internal/context/` 包（与 `internal/prompt` 共存，不替代）：Context Assembler
- 定义 `ContentClass` 枚举 + `Priority` 层级 + `TokenBudget`
- `buildPromptWithEmits` 逐步迁移到新 API
- 接入 `internal/trace` 的 context telemetry：每次 injection 记录 注入层数 / token 消耗 / 截断内容

**对现有系统的影响：**
- **中中断**——需要逐步迁移 9 条注入路径，每条需测试验证行为和 token 消耗不变
- 预计 2-3 个 sprint（非连续，可与其他方向并行）

### 方向三：Agent 输出契约形式化（P1）

**为什么需要：**
当前 forge-core 与任何 agent CLI（当前是 Claude Code）的交互是 fork/exec + 5 个独立文本解析函数。每次 Claude Code 升级输出格式，整个管线静默断裂。当前已有部分使用 `--output-format json`（cost.go），但：

1. 不是所有输出都走 JSON 路径（reviewer verdict 是纯文本末行匹配）
2. JSON 输出也没有 schema 校验（只是 map[string]any 的泛型解析）
3. 没有版本协商机制——forge-core 无法说「我理解的协议版本是 X」

**核心挑战：**
- Claude Code 的输出格式不在 forge-core 控制范围内，所以不能假设「JSON 一定可用」
- 需要同时支持「结构化回退到文本解析」的 graceful degradation
- 版本协商需要 agent CLI 支持——这是跨组织边界的问题
- 现有 5 个解析器各有边界情况（Sprint 27 修复的 block-scalar bug 是教训）

**预期的架构变更：**
- 新增 `internal/agentprotocol/` 包：定义 AgentOutput 接口 + 多实现（JSON / TextFallback）
- 现有 `cost.go`/`prompt_context.go` 中的 inline 解析函数统一迁移到此包
- 引入输出 schema 定义（Go struct tags / JSON Schema）+ 校验
- 版本协商：`forge run` 注入 `Forge-Protocol-Version` header 到 agent CLI（如果 CLI 支持）

**对现有系统的影响：**
- **中高中断**——需要重构 5 个现有解析器，且需要保持每个的向后兼容
- 建议 2 个 sprint，并行于方向二
- 关键风险：text parsing 的 regression——每个解析器需要有 fixture-based 测试（真实 claude 输出样本）

### 方向四：跨运行 Trace 聚合与操作智能（P1→P2）

**为什么需要：**
这是唯一被审查文档指出「有增量价值」的方向。当前 trace 系统是 emit-only 结构——写 `trace.jsonl` 但不提供任何 readback 聚合 API。Learning loop 从 scorecard 学但不从 trace 学。这意味着：

- 无法回答「哪个 phase 的失败率在上升？」
- 无法回答「我的平均每次运行成本是上升还是下降？」
- 无法做运行间比较（diff 两次 `forge evolve`）

**核心挑战：**
- trace.jsonl 是文件系统存储，不是数据库——没有索引没有查询
- 跨运行聚合需要 schema 稳定 —— 但 trace 的 Event kinds 还在增长（Sprint 26 加了 cost telemetry）
- 聚合本身是一个需要权衡的问题：在每次 CLI 调用时做聚合（响应延迟增加） vs 后台聚合（需要 daemon/temporal worker）

**预期的架构变更：**
- 新增 `internal/trace/aggregate.go`：纯内存/文件聚合查询 API
- trace.jsonl 增加索引文件（`.forge/trace.ndx`）：按运行 ID + 时间戳 + event kind 做 B-tree 索引
- `forge trace --summary` CLI 子命令：聚合摘要 + 趋势线
- 与 scorecard 的数据关系需要明确定义：scorecard 是「学习信号」，trace aggregate 是「操作智能」

**对现有系统的影响：**
- **低中断**——纯增量，不改变已有 trace emit 路径
- 1-1.5 个 sprint 可完成核心 API
- 建议优先于 SDK 提取，因为它解锁的能力更直接（运维决策）

### 方向五：Monorepo 工作区（P2，但应提前）

**为什么需要：**
审查文档的正确判断：方向五（Monorepo 工作区）是五个方向中唯一**真正未被已有分析展开**的。当前 forge-core 假设每次 `forge run` 工作在一个单项目根目录——没有 workspace 概念、没有子项目 scope 继承、没有跨子项目的 gate 链式触发。这是企业采用的关键壁垒：没有一个真实企业项目是单仓库的。

**核心挑战：**
- 需要重新设计项目模型：从单根目录 → 多 workspace + 根/子覆盖
- `forge-init` 已能做脚手架继承，但运行时模型还是单仓库
- `internal/migrate` 和 `internal/doctor` 的 scope 需要理解子项目边界
- Gate 链式触发：子项目 A 的共享库改了，需要触发子项目 B 的 gate 运行

**预期的架构变更：**
- 重构 `internal/asset`：新增 `Workspace` 类型（根 workspace + N 子 workspace）
- 每个 CLI 子命令需要理解 `--workspace` / `--all` flag
- `internal/gate/resolve.go` 跨 workspace gate 聚合
- 配置继承模型：子 workspace 继承根，可覆盖

**对现有系统的影响：**
- **高中断**——需要重构项目模型的核心假设。建议在方向一（Phase Schema 权威化）之后再开始
- 预计 3-4 个 sprint
- 关键前提：方向一的 Phase Schema 做完后，workflow 定义才稳定，workspace 扩展才不至于在沙上建塔

---

## 3. 接口设计建议

### 3.1 关键模块接口原则

**原则一：每个内部包只有一个公认的「权限入口」。**
当前 `internal/prompt` / `internal/gate` / `internal/converge` 等包是正确模式——它们导出的符号集合小、职责单一。但 `internal/asset` 正在滑向 God Package，因为它承载 Phase/Workflow/Run 等核心类型和对应的解析/校验/序列化逻辑。建议：

```
internal/asset/          ← 只放类型定义（Phase, Workflow, Run, Workspace）
internal/asset/validate  ← 校验逻辑
internal/asset/codegen   ← 代码生成（方向一）
```

**原则二：Agent 输出解析应统一在一个协议包下。**
当前 5 个解析函数散布在 `cost.go`（1 个）、`gates.go`（2 个）、`prompt_context.go`（1 个）、`converge/gates.go`（1 个）。建议统一到 `internal/agentprotocol` 包下，暴露单一接口：

```go
type Parser interface {
    ParseVerdict(output string) (Verdict, error)
    ParseConfidence(output string) (int, error)
    ParseCost(output string) (*CostReport, error)
}
```

**这不仅仅是代码组织——它为方向二（多厂商支持）提供自然的扩展点。**

**原则三：Trace 应有 emit-side 和 read-side 两个接口，而非一个。**
当前 `Tracer` 只有 Write 接口。如果要加跨运行聚合（方向四），需要一个独立的 `TraceStore` 接口：

```go
type TraceStore interface {
    // Write
    Append(runID string, events []Event) error
    // Read
    Query(filter TraceFilter) ([]RunSummary, error)
    Aggregate(query AggregateQuery) (*AggregateResult, error)
}
```

文件系统实现（当前）和未来 SQLite/嵌入式 DB 实现共用同一接口。

### 3.2 是否需要引入新的抽象层

**需要：Phase Schema 抽象层（方向一）。**
这不是一个新的「层」，而是把当前碎片化的 schema 定义收敛到一个权威源。具体方案有两个选项：

| 选项 | 方案 | 优点 | 缺点 |
|---|---|---|---|
| A. Go 代码生成 | 用 Go 源文件定义 schema，生成 YAML schema + 校验代码 | Go 类型检查，编译期发现漂移 | 需要 code gen 工具链；与既有 YAML-first 的工作流方向相反 |
| B. YAML schema 先行 | 定义 JSON Schema / CUE schema，生成 Go 代码 | 与既有 YAML 声明式方向一致；schema 可被非 Go 工具消费 | 需要引入 CUE 或 JSON Schema 解析器；对 forge-core 零依赖原则有影响 |

**建议：选项 B，但用 CUE 而非 JSON Schema。**
CUE 比 JSON Schema 更简洁，且支持 Go 原生 code gen（`cue get Go`）。`go-yaml` 作为首个正式外部依赖引入（论证见第 4 节）。

### 3.3 向后兼容性策略

**策略一：schema 演进用三个阶段的引入周期。**
- 阶段 1（过渡期）：code gen 产出与手动维护并存，check.py drift guard 告警但不阻断
- 阶段 2（默认期）：code gen 为首要路径，手动修改为「允许但需要 override flag」
- 阶段 3（强制期）：手动路径移除，check.py 阻断 drift

**策略二：所有新解析路径必须回退到旧路径。**
特别是方向三的 Agent 输出契约——JSON 路径必须先试，失败后静默回退到文本解析，不中断已有管线。回退路径必须有 metric（`forge trace` 可观察到回退率）。

**策略三：Monorepo workspace 以 `--workspace` flag 起步，不改变默认行为。**
根 workspace 的行为与当前完全一致。子 workspace 支持通过 `forge run --workspace frontend` 递进式采用，不强制全仓迁移。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈/框架

经过 31 轮 sprint，以下是当前适合引入依赖的时机判断：

| 依赖 | 建议 | 理由 | 反对方案 |
|---|---|---|---|
| **Go YAML 库** (goccy/go-yaml) | ✅ **推荐引入** | Python shim 在 Sprint 27 真实损坏过（block-scalar bug）；每次都 shell out 到 Node/Python 增加延迟和故障面 | 继续用 python shim（成本可控但架构债务增加） |
| **CUE** (cue-lang/cue) | ✅ **推荐引入** | 解决 Phase Schema 碎片化的最佳工具；Go 原生集成；用于 schema + validation + code gen | 自研 schema 语言（不划算）；JSON Schema（但 forge 已有 YAML 生态） |
| **SQLite** (modernc.org/sqlite) | ⚠️ **暂缓，v3 考虑** | 适合 trace 聚合方向，但引入 C 依赖与现代c.org 的 CGo-free 方案仍在演进 | 文件系统 + 索引文件（当前方案，足够再撑 1-2 个方向） |
| **Temporal** | **v3 目标** | north-star 已定，v2 不用 | forge-core 的 LoopEngine + checkpoint 在 v2 足够 |
| **结构化日志库** | ❌ **不需要** | Go 标准库 `log/slog` 在 Go 1.21+，forge-core 只需要结构化 + 级别，slog 足够 | `log/slog` 是标准库，零外部依赖，零引入成本 |

### 4.2 第三方依赖的评估标准

对于 forge-core 的依赖决策，建议建立明确的门槛：

```
forge-core 依赖引入审计清单（每项必须通过）:
□ 依赖是否已解决「是否有纯 Go 实现」（CGo 倾向否决）
□ 依赖的编译时间增量 < 2s（CI 中的感知延迟）
□ 依赖的二进制大小增量 < 2MB（vs 当前 ~12MB 静态二进制）
□ 依赖的 API 稳定性承诺（Go 1.x 兼容 / semver）
□ 是否有被主流项目采用的先例
□ 是否有 ADR 记录「为什么选它不选 X」
□ 是否可被 forge-init 继承（copy-anywhere）
□ 引入后能否消除一个既有的临时方案（python shim / 手动 parser）
```

### 4.3 自建 vs 采购的决策依据

ForgeOS 在自研/采购边界上的判断一直良好。以下是对当前缺口的具体建议：

| 能力 | 自研/采购 | 现有资产 | 建议 |
|---|---|---|---|
| YAML 解析 | **采购**（go-yaml） | python shim（自研，脆弱） | 立即切换——这是消除一个已证实的故障点 |
| Schema 系统 | **采购**（CUE） | 6 处手动同步（自研，高维护） | 方向一引入 |
| Agent 输出协议 | **自研** | 5 个独立解析函数 | 自研方向三，不采购——这是核心差异化 |
| Trace 存储 | **自研**（文件系统过渡） | trace.jsonl | 方向四开始前决定是否升到 SQLite |
| 模型路由 | **自研** | routing.go | 保持自研——这是护城河 |

**关键原则**：ForgeOS 的护城河是治理逻辑/路由决策/记分卡/角色体系——这些必须自研。技术性/工具性的东西（YAML 解析器 / Schema 引擎）应该采购。

---

## 5. 实施路线图

### 5.1 优先级排序

基于架构债务的紧急程度 + 对后续方向的依赖关系：

| 优先级 | 方向 | 阶段 | Sprint 估算 | 依赖 |
|---|---|---|---|---|
| **P0** | **方向一：Phase Schema 权威化** | v2.1 | 1.5 | 引入 Go YAML 库（前置依赖决策） |
| **P1** | **方向三：Agent 输出契约形式化** | v2.1 | 2 | 方向一的部分产出（schema 定义经验） |
| **P1** | **方向二：Context Engine 结构化** | v2.2 | 2-3 | 方向一（稳定 schema 后做 injection 才稳妥） |
| **P2** | **方向四：跨运行 Trace 聚合** | v2.3 | 1.5 | 方向二（Context Assembler 可消费 trace telemetry） |
| **P2** | **方向五：Monorepo 工作区** | v2.4 | 3-4 | 方向一（稳定 Phase 后 workspace 扩展才安全） |

### 5.2 阶段划分和里程碑

```
v2.1（核心架构债务清偿）
├── 引入 Go YAML 库 + 移除 python shim
├── Phase Schema 权威化（CUE codegen → 6→1 维护点）
├── Agent 输出契约形式化（internal/agentprotocol）
└── 里程碑：forge accept 不依赖 python3；schema drift 被自动检测

v2.2（核心引擎成熟）
├── Context Engine 结构化（internal/context Assembler）
├── Token 预算 + 优先级模型
├── Agent 协议版本协商（forge-core 与 agent CLI 互认版本）
└── 里程碑：context injection 可观测/可预算/可调试

v2.3（数据飞轮闭环）
├── Trace 聚合 API（internal/trace/aggregate）
├── forge trace --summary
├── Scorecard + Trace 联合分析（跨运行 cost/latency 趋势）
└── 里程碑：Learning loop 同时从 scorecard 和操作模式学习

v2.4（企业采纳）
├── Workspace 模型（internal/asset/workspace）
├── 子项目 scope 继承 + override
├── 跨子项目 gate 链式触发
├── forge run/migrate --workspace
└── 里程碑：一个真实 monorepo 被测通过
```

### 5.3 风险点和缓解策略

**风险 R1：Phase Schema 权威化遭遇既有工作流格式的「隐藏语义」。**
当前 YAML 文件中的某些 Phase 字段可能存在未显式声明的行为契约（例如 `secondary_template` 的某些使用模式依赖了字符串拼接的具体行为）。Codegen 可能丢失这些隐式假设。
**缓解**：在迁移过渡期保持 codegen 输出与手动路径并行运行 2 个 sprint，falsification 测试验证行为一致。

**风险 R2：引入外部依赖（go-yaml / CUE）后，forge-core 的「零外部依赖」承诺被打破的心理冲击。**
团队已经以零依赖为荣 31 个 sprint。这会是一个文化上的阻力。
**缓解**：改变表述从「零外部依赖」到「核心层零非必要依赖」。每笔依赖必须通过第 4.2 节的审计清单。在 README 中诚实标注「forge-core 核心包（internal/orchestrator / routing / converge）零外部依赖；工具函数层（yaml / schema）使用经过审计的库」。

**风险 R3：Agent 输出契约形式化（方向三）与 Claude Code 的版本冲突。**
如果 Claude Code 改了 JSON 输出格式，而 forge-core 的新 parser 太严格，可能比现在的宽容式 `map[string]any` 解析更容易断裂。
**缓解**：parser 实现应分层——严格的 schema 验证层 + 宽容的 fallback 层。schema 验证失败不抛错，降级到宽容解析并记录 telemetry（forge trace 可观察到 schema drift 率）。这比当前的全线静默解析更安全，而不是更不安全。

**风险 R4：Monorepo 工作区（方向五）的 scope 蔓延。**
「加 workspace 支持」听起来像一个功能，但实际触及 CLI 模型、配置继承、gate 聚合、migrate/doctor scope 等几乎所有子系统。很容易变成 6+ sprint 的 mega-feature。
**缓解**：严格定义 v1 scope——只做「多项目的单一 forge run 选择」+「根→子配置继承」+「gate 聚合汇报」。不做跨 workspace 的 grow/evolve/cycle 检测、不做 workspace 间的 trace 关联分析。v1 稳定后才扩。

---

## 附注：与已有分析文档的关系

根据审查文档的评估，我补充说明本分析与已有文档的关系：

| 已有文档 | 覆盖域 | 本分析的关系 |
|---|---|---|
| `four-structural-gaps.md` (2026-07-10) | Phase 膨胀 / Context Engine / 结构化日志 / 契约脆弱性 | ✅ 方向一/二/三直接对齐，但本文增加了：技术选型评估（CUE/go-yaml）+ 实施分级（P0/P1）+ 向后兼容策略 |
| `five-architect-product-perspective-2026-07-10.md` | 跨运行 Trace 聚合 / 部分失败完整性 / 多厂商归一化 / 模板漂移 / 自限流 | ✅ 方向四有重合，但本文增加了：Trace 聚合的接口设计建议（TraceStore）+ 与 Scorecard 的关系定义（操作智能 vs 学习信号） |
| `five-structural-extension-directions-2026-07-11.md` | 路由自校准 / 预测估算 / 多运行生命周期语义 / 依赖注入框架 / Agent 产出合约 | ⚠️ 方向五（Monorepo）与本文方向五重合但角度不同——本文聚焦项目模型重构而非「合约验证」 |
| `expansion-horizon-three.md` | 第三地平线：多仓库联邦 / Web UI / 管道组合 | ✅ 本文方向五（Monorepo）是其「多仓库联邦」的前置步骤，而非替代 |

本文的**核心增量**不在提出「新的功能方向」，而在：
1. 将 Phase Schema 碎片化升为 **P0 架构债务**而非「待优化项」
2. 对五大方向给出了**具体的接口设计建议 + 技术选型 + 向后兼容策略**
3. 明确了方向之间的**依赖关系**（方向一 → 方向二/五、方向二 → 方向四）
4. 为 forge-core 的「零依赖」转向「有选择的依赖」建立了一套可审计的决策流程
