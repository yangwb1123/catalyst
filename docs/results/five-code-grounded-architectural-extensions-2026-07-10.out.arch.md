现在我有完整的上下文。以下是架构分析。

---

# 架构分析：ForgeOS 当前架构评估与扩展方向

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 当前的架构选择体现了**务实克制**的工程文化，这是其最大优势。

**优势一：单仓 + 厚骨架的反脆弱设计**

当前架构采用「monorepo + `.agent/` 骨架 + `forge-core` Go 运行时 + Node/Python harness」的混合架构。这不是传统微服务，也不是纯 monolith——而是一种 **"可预测生长的脊髓"** 架构。其巧妙之处在于：

- `.agent/` 骨架（PROJECT / ARCHITECTURE / ROADMAP / AGENTS / DECISIONS）是**声明式的事实源**，不是代码的注释，而是代码的**宪法**。代码可以改，骨架必须先改。
- `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 更进一步：把骨架中的声明**推导为可核对的清单**，建立了从「意图声明」到「实现证明」的审计链。这在 AI 生成代码的上下文中是决定性的——没有它，LLM 会不断发明新行为而不自知。

你在 Sprint 29-31 中展现的纪律——系统性审计 `converge.Signals` 全部 8 个字段，而非只修被动撞见的——正是这一设计原则的体现。

**优势二：中枢旋钮（mode × lifecycle）作为唯一耦合点**

三处（Router 档位 · Harness 严格度 · Workflow 深度）由同一个 `mode × lifecycle` 矩阵驱动，而非三个独立旋钮。这是**高扇出、低认知负荷**的设计：

```
mode × lifecycle  ──→  Router tier floor
                  ──→  Harness gate-set + enforce + coverage threshold
                  ──→  Workflow depth (discover/design/adr/review/evolve)
```

其价值在于：**一个参数变化，全系统一致响应**。explorer → engineering 的迁移（Sprint 8 `forge migrate`）自动派生 5 个补债任务，而不是靠人脑记住所有要收紧的地方。

**优势三：带外执法（out-of-band enforcement）作为真相之源**

架构明确区分了「真相之源 = 带外 gate（harness gate.mjs / arch-check / check.py / secret-scan）」与「加速器 = in-editor hook（CC PostToolUse）」。这是对 "站在所有 CLI 之上" 这一载重墙约束的诚实回应——你不能假设宿主给你阻断能力，所以必须有一个 host-independent 的执法层。

Sprint 24-26 的真点火验证证明了这套设计的有效性：`gate.mjs` + `check.py` + `acceptance.mjs` 在真实 claude 进程下仍然独立执法，agent 不能绕过。

**优势四：零外部依赖的 forge-core 运行时**

13 个 Go 包，纯标准库，零外部依赖。这在 2026 年的 Go 生态中几乎是反潮流的决策——但在这个上下文中完全正确：

- forge-core 是**编排控制面**，不是数据面。它不需要高吞吐消息队列、不需要复杂存储引擎、不需要 ML 推理。
- **零依赖 = 零供应链攻击面**（对于控制 AI 生成代码的系统，安全是生死线，north-star 明确说）。
- **零依赖 = 零升级债务**。不需要跟踪 `go.mod` CVE，不需要处理 semver 冲突。
- **零依赖 = 跨平台编译的单二进制分发**。`forge` CLI 可以 curl 下载即用。

这也有代价（下面讨论），但在当前阶段是正确取舍。

### 1.2 当前架构的局限性

**局限一：CLI 即架构 —— 微服务能力被压缩在单进程空间**

当前 forge-core 是一个单体 CLI 二进制。所有 5 个引擎（Orchestrator / Model-Router / Context-Engine / Memory-Engine / Evaluation-Engine）在同一个进程地址空间中运行。这在以下方面构成隐患：

- **故障隔离为零**：Context-Engine 的 OOM 会拖垮整个 CLI。Memory-Engine 的死循环阻塞所有请求。
- **水平扩展不可能**：无法单独扩展 Model-Router（记分卡查询热点）而不扩展 Orchestrator。
- **durable wait 只能靠 `time.Sleep`**：north-star 中 Temporal 提供的人审等待（几小时/几天）在 CLI 架构下不可能实现——当前解决方案是 `--human-gate exit 1` 的 fail-closed 拒绝。

代码中已有诚实标注：north-star 的 Temporal 持久等待标注为 v2/v3。

**局限二：YAML 转码桥是架构的薄弱连接**

`forge-core` 通过 `python3 harness/yaml2json.py` 把 YAML workflow 定义转码为 JSON。这是**两个正确性依赖**：

- **Python 在 PATH 中**：没有 Python 就不能跑 forge-core。这违反了 "零外部依赖" 的精神。
- **转码器本身必须与 Go 运行时语义一致**：Sprint 27 的 block-scalar bug 证明这一层真实地损坏了数据——`description:` 和 `note:` 字段被注入了字面量 `"> "` 前缀，直送 agent prompt。测试本身也失效（只 `t.Logf` 不 `t.Errorf`），7 个真实文件早已跑偏却全绿。

架构上，这一层需要被吸收进 forge-core 自身。ROADMAP 标注了「属 architect/cto 的依赖决策」——这个决策不应再推迟。

**局限三：单 phase 架构限制了并行执行的价值**

当前 LoopEngine 支持 `RunParallel`，但 `RunParallel` 显式禁用了 per-phase checkpoint（`parallel.go` 注释直说 "NO per-phase checkpoint"）。这意味着：

- 并行 phase 中的任何一个崩溃，**所有**并行 phase 的工作丢失（LLM 调用费 + token 都浪费了）。
- 没有 wave → phase 映射，checkpoint schema 无法表达「wave 0 的三个 phase 中两个已完成」的场景。

方向二的评估是准确的：这不是小缺口，而是**当前架构的并行执行在实践中价值有限**的核心瓶颈。

### 1.3 关键设计决策的合理性

| 决策 | 合理性 | 评估 |
|------|--------|------|
| Go 核心 + polyglot harness 适配器 | ✅ Go 是编排语言的最佳选择（goroutine、编译速度、静态二进制）；harness 适配器是语言生态的诚实映射 | 合理，且已被 31 个 sprint 验证 |
| 零外部依赖 | ✅ 见上面分析。但 YAML 解析需要例外处理 | 当前决策正确，但 YAML 转码必须尽快吸收 |
| mode × lifecycle 中枢旋钮 | ✅ 高扇出低耦合，单一变更点驱动全系统 | 架构创新，应保持并文档化 |
| 带外执法为真相之源 | ✅ 对 host-independent 约束的诚实回应 | 已验证有效 |
| ADR-0003 submodule 全局共享暂缓 | ✅ 在「被治理项目 < 3」时推进跨仓治理是预成熟设计 | 合理，但需设定明确触发条件 |

**一个有争议的决策值得讨论**：当前 `forge-core` CLI 同时包含**编排引擎**和**CLI 界面**（`cmd/forge` 包包含了大量胶水逻辑）。虽然 Sprint 27/29/30 通过抽出 `internal/doctor`、`internal/attribution`、`internal/gate/resolve.go` 等包来消化 `cmd/forge` 的膨胀，但**架构层没有强制分层**——没有禁止 `cmd/forge` 直接 import 持久化/内存/路由等包的规则。当前靠手动审计和文件数预算来约束，这在单进程 CLI 中可行，但若未来拆微服务，需要更明确的分层契约。

### 1.4 架构债务与技术债

**架构债务（知道更好的方式，但因阶段限制选择当前方式）**：

1. **YAML 解析桥** —— 最紧迫的架构债务。Python shim 已在生产环境中被证明可以静默损坏数据（block-scalar bug）。需要用 Go YAML 库替换，或者手写 YAML 子集解析器。

2. **单进程 CLI 架构** —— 当 forge-core 的引擎数继续增长（Knowledge-Engine、Sandbox-Engine 上线），进程内隔离的必要性会指数级上升。但当前拆微服务为时过早（coordination overhead > 收益）。

3. **持久化走平面 JSON 文件** —— `persist` 包用 `write(tmp) → rename` 原子模式写 JSONL。这在单节点 CLI 中工作，但无法支持并发、无法支持事务、无法支持查询。这是有意识推迟的债务（north-star 标了 Postgres + Temporal）。

**技术债（当前做法不好且没有好的理由）**：

1. **没有独立 Router 服务** —— `internal/routing` 的 `TierFor` 自称「非完整多维评分器」，真实多维路由（complexity / dependency / context / business-impact）只喂手动 `forge route` CLI，不驱动执行。这不是架构债务，是**镀金阻抗**——在评估文档的方向四讨论中，这是正确的自我约束。

2. **`stop_condition.on_rejected` 死代码** —— Sprint 30 发现三条路径都到不了这段代码，最终加了一次性标记消费模式激活了它。但**这暴露了一个更深的架构问题**：LoopEngine 的控制流（gate FAIL → loop-back / abort / retry / human-gate reject）是**隐式**的，散布在 `run.go`、`loop.go`、`gates.go`、`converge.go` 多个文件中，没有一张状态图。

3. **测试基础设施的手工痕迹** —— `forge accept` 的测试（`test_acceptance.mjs`）需要耗费大量精力维护「copy-anywhere」不变量（Sprint 16 专门加固）。这是测试框架与代码布局耦合过紧的信号。

---

## 2. 扩展方向

基于资产负责人的评估文档和代码库的实际状态，我提出以下扩展方向，分为 **P0（当前架构内可落地）**、**P1（近期桥接）**、**P2（远期架构演进）** 三个优先级。

### 方向 A（P0）：ReplayExecutor —— 把 trace 变成 mock

**为什么需要**

评估文档方向五的论证是准确的：80% 的基础设施已存在（trace JSONL 捕获了 Status / DurationMs / CostUsdMicros / Model / Name），`replay_test.go` 和 `replay/testdata/` 已存在，但重放的目标是 `persist.Recover`（crash 恢复），不是 agent 执行重放。

缺少的 20% 是什么：**一种把真实 trace 转成 `AgentExecutor` 的适配器**，让 `DryRunExecutor`（只叙述）、`CommandExecutor`（真调 agent）和未来的 `ReplayExecutor`（从 trace 回放）三者实现同一接口。

**业务价值**

- **确定性回归测试**：每次改动后，用之前的真实 trace 回放，验证相同输入是否产生相同输出。不必花 LLM 调用费。
- **预算安全**：开发者 CI 中跑 forge-core 改动时不需要--agent-cmd claude。
- **错误分析**：生产 trace 可以下载到开发环境回放，不需要再现 LLM 调用。

**核心挑战**

1. **trace 的因果关系**：trace 记录了 agent 的输出（代码），但没有记录 agent 的**输入**（prompt + 上下文）。ReplayExecutor 需要模拟的不仅仅是「返回什么字符」，而是「返回什么文件编辑」。这需要 trace 也记录 `acceptEdits` 的编辑载荷，而不仅仅是 verdict token。
2. **非确定性分支**：如果被回放的 phase 输出文件，后续 phase 读该文件——trace 中只有最终文件内容，没有每步中间状态。

**预期架构变更**

```
AgentExecutor 接口（现有）
├── DryRunExecutor（现有 — 只叙述）
├── CommandExecutor（现有 — 调真实 agent）
└── ReplayExecutor（新增 — 从 trace 回放）
    ├── 读 trace JSONL
    ├── 按 phase 匹配事件
    └── 模拟 agent 输出（文件编辑 + verdict）
```

**对现有系统的影响**：低。`AgentExecutor` 接口只有两个实现，新增第三个不改变现有调用链。主要影响在 trace schema 和 persist 包。

**与 direction 5 评估的关系**：评估文档准确识别了缺口，但低估了 trace schema 扩展的难度——trace 需要记录的不仅仅是 `Status/DurationMs/CostUsdMicros`，还需要记录**agent 的输出载荷**。当前的 trace.Event 不具备这个能力。

---

### 方向 B（P0）：Checkpoint 的 wave-aware schema

**为什么需要**

评估文档方向二的论证是准确的，且比文档写的更彻底——`parallel.go` 不仅禁用了 per-phase checkpoint，它的注释还指出了并发 checkpoint 写入的竞态问题。

当前 `checkpoint.go` 的 `PhaseIndex` 是单 `int`，无 wave → phase 映射。这意味着：

- 并行模式（RunParallel）的崩溃恢复是**全或无**——要么全部重跑，要么全部丢弃。
- 在并行 phase 中，一个 phase 完成后的 checkpoint 写入会和其他仍在跑的 phase 产生竞态。

**业务价值**

- **硬件失效下的成本保护**：并行执行 N 个 phase，每个消耗一次 LLM 调用。如果在 phase 3/5 时崩溃，当前恢复路径从 phase 0 重跑全部 5 个——N-2 次调用费白烧。N=5 时就是 60% 的浪费。
- **更长并行管道**：如果 checkpoint 不可靠，并行执行的价值上限被压低。解决后才能设计更长的并行流水线。

**核心挑战**

1. **wave-phase 二维映射**：schema 需要从 `PhaseIndex int` 演变为 `(WaveIndex int, PhaseIndex int)` 结构体，且需要向后兼容旧 checkpoint。
2. **并发写入序列化**：多个并行 phase 同时完成时，checkpoint 写入需要串行化。可以走 per-phase 文件 + 聚合文件的两级设计，或者用 Go 的 `sync.Mutex`（但 `persist` 包现在不是并发安全的）。

**预期架构变更**

```
checkpoint.go 的 Checkpoint struct
├── PhaseIndex int  ──→  (WaveIndex, PhaseIndex)

并行 checkpoint 写入
├── per-phase 文件（无竞态写入）
└── 聚合 watcher（读取已完成 phase 生成全局 checkpoint）
```

**对现有系统的影响**：中。会触及 `persist.Recover`（需要理解新 schema）、`checkpoint.go`（写入逻辑）、`parallel.go`（完成回调）。需要迁移旧 checkpoint 格式（或接受旧格式降级为全重跑）。

**与 direction 2 评估的关系**：完全准确。评估文档指出「wave 0 的三个 phase 中两个已完成」在当前 schema 中不可表达——这是 checkpoint 恢复路径的架构瓶颈，不是性能优化问题。

---

### 方向 C（P1）：YAML 解析的 Go 内建化

**为什么需要**

这实际上是当前代码库中最直接的架构债务。Sprint 30 的 `normalize.go` 重写修复了 block-scalar 损坏，但**修复在 Python shim 中，而 shim 本身不是 forge-core 的一部分**。每次 Go 构建不需要 Python，但每次 `forge run/evolve` 需要。这产生了一个奇怪的依赖反转：

- forge-core 的**消费者**（CI、开发者、生产环境）必须装 Python。
- forge-core 的**测试**要跨语言验证 Go 行为与 Python 行为一致。

**业务价值**

- 消除运行时依赖：`forge` 静态二进制 + 零 Python。
- 消除跨语言语义差异：block-scalar bug 不会再现。
- 简化 CI：不需要 Python 环境。

**核心挑战**

1. **Go 标准库没有 YAML 解析器**。所有第三方库都有外部依赖。`gopkg.in/yaml.v3` 有 1.1k+ stars 但会引入依赖。
2. **手写解析器 vs 引入依赖**：Sprint 27 的方向五已经证明手写解析器（`internal/yaml2json`）是可行的，但 block-scalar bug 证明了手写解析器的正确性维护成本很高。ROADMAP 正确地标注了这是「属 architect/cto 的依赖决策」。
3. **安全考虑**：YAML 解析器的攻击面（`!!python/object:` 标签、锚点 Aliases、自定义 Tag 处理）——但 forge-core 消费的 YAML 来自已知工作流文件，不是用户上传内容。

**选项与权衡**

| 选项 | 优势 | 劣势 |
|------|------|------|
| A. 引入 `gopkg.in/yaml.v3` | 成熟、被广泛验证 | 打破零依赖承诺；版本升级债务 |
| B. 手写 fork `internal/yaml` 子集 | 保持零依赖；只解析 forge-core 需要的子集 | 维护成本高；新 YAML 特性需扩展 |
| C. 用 Go 的 `text/scanner` 或 `encoding/json` 的转码器 | 零依赖 | 只能吃兼容子集；无法处理多文档/锚点 |
| D. 编译 Python shim 为静态二进制（PyInstaller） | 零运行时依赖 | 二进制大小膨胀（~50MB）；非 Go 工具链 |

**我的建议**：走选项 B。理由是：

1. forge-core 消费的 YAML 是**自产的**（工作流定义在 `.agent/workflows/` 中），不是任意用户内容——不需要全 YAML 1.2 解析。
2. Sprint 27 已经证明 `internal/yaml2json` 的手写解析器是可以工作的，只是缺 block-scalar 实现——补上即可。
3. 零依赖是 forge-core 的安全承诺（north-star 反复强调），不应轻易打破。

但需要一条纪律：**手写解析器必须有差分测试**——`TestToJSON_MatchesPythonShim` 不应再是只 `t.Logf` 的幽灵测试。每次修改解析器，必须对比 Python 参考实现输出，不一致则 FAIL。

---

### 方向 D（P1）：质量评分深度的跨层反馈闭环

**为什么需要**

评估文档方向四的核心批评是有价值的，尽管具体论据有误（Scorecard 确实有 `QualityScore`，`HistoryTiebreak` 确实在用）。真正的缺口不是「没有质量数值」，而是 **质量评分的深度和反馈闭环的闭合度**：

- `quality_score` 目前只反映 gate 的「通过/不通过」二元结果。不反映：
  - Reviewer 的 `APPROVE`/`REQUEST_CHANGES`/`REDESIGN` 定性裁决（Sprint 28 刚接上 `review_status` 信号，但还没接入 scorecard）。
  - Agent 个体质量差异（同一个 phase 两个不同 agent 的表现差异不被记录）。
  - 测试覆盖率变化（`code_test_ratio` 是静态比值，不是 delta）。
  - 回归率（上次通过的测试这次是否失败）。

**业务价值**

- **更精准的模型路由**：`HistoryTiebreak` 现在只能按 `QualityScore`（gate pass/fail） + `AvgCostUsd` 做候选排序。引入 reviewer 评分后，可以区分「刚好通过 gate 的低质量实现」和「一次性通过的优质实现」。
- **agent 反馈驱动改进**：如果记分卡记录了哪个 agent（claude-code / codex / gemini-cli）产生了高 reviewer 评分，ROADMAP 可以针对性优化 agent 配置。
- **治理透明度**：「质量怎么定义」从隐式（gate pass = quality）变为显式（gate pass + reviewer score + regression check = quality）。

**核心挑战**

1. **Reviewer 评分的主观性**：reviewer 的 `APPROVE` 不等同于「高质量」。更细致的评分（1-5 星）需要机读契约扩展（当前是二元/五择一 token，不是数值）。
2. **评分的时间维度**：今天的高质量实现在三个月后可能被更好的方案取代——scorecard 有 recency 衰减（Sprint 11 实现），但衰减的是旧分权重，不是质量本身的重评估。
3. **反馈闭环的延迟**：今天 build 的 phase，reviewer 同意后进入 evolve——但质量评分要等到 evolve 结束（gate 全绿）才落入 scorecard。这个延迟对路由算法的价值打了折扣。

**预期架构变更**

```
Scorecard struct 扩展
├── QualityScore  float64 (现有)
├── Samples       int (现有)
├── PassRate      float64 (现有)
├── ReworkRate    float64 (现有)
├── AvgIterations float64 (现有)
├── AvgCostUsd    float64 (现有)
├── P95LatencyMs  float64 (现有)
└── [新增] ReviewerScore  float64  — 来自 reviewer verdict 映射
└── [新增] RegressionRate float64  — 测试回归率

反馈管线
├── review phase → parseExecutiveVerdict → verdictLedger → scorecard
├── qa phase → test pass/fail delta → scorecard
└── gate → quality_score → scorecard (现有)
```

**对现有系统的影响**：中-高。涉及 `scorecard` schema 变更（向后兼容？）、`scorecard-update.mjs` 时间窗口内的写入逻辑、`Scorecard` struct 持久化、`HistoryTiebreak` 的排序权重调整。

**与 direction 4 评估的关系**：评估文档方向四的论据多处不准确（Scorecard 有 QualityScore，HistoryTiebreak 在用），但其**深层论点**——质量维度不够深——是正确的。如果聚焦于「review scoring → scorecard → routing 的反馈闭环未闭合」而非「没有质量数值」，会是更强的论点。这个方向是我建议的 P1（有收益但非瓶颈），不是 P0。

---

### 方向 E（P2）：跨项目舰队治理的最小可行传播层

**为什么需要**

评估文档方向一是准确的：当前没有中央→项目继承机制。`harness/policies.yml` 和 `.agent/policies/modes.yml` 确认存在于单仓根目录，scorecard-update.mjs 以 `cwd` 为锚。

但方向一的根本瓶颈不在于代码实现，而在于 ADR-0003 的「待拍板」——远程位置和批准由用户决定。在决策做出前推进中央策略传播层是预成熟的设计。

**何时从 P2 升级为 P1**

当以下条件全部满足时：
1. **被治理项目 ≥ 3**（当前只有 url-shortener 和 starter 模板）。
2. **至少有一个项目由不同自然人维护**（不是同一人在同一机器上跑）。
3. **治理资产（`.agent/policies/` + harness）的变更需要跨仓同步**已被实际感受到（"我改了 modes.yml，项目 A 没生效"）。

在此之前，**forge-init 的复制策略**（每次新项目从模板复制治理资产）是正确且诚实的模式。它虽然手动，但保持了各项目的**独立演变能力**——项目 A 可以在不破坏项目 B 的情况下实验新的 gate 规则。

**核心挑战**

即使条件满足，跨仓治理也有以下设计难题：

1. **双层覆盖的解析规则**：ADR-0003 定的 submodule 机制，子仓覆盖父仓的策略文件。但「覆盖」的定义不清晰——是整文件替换，还是字段级 merge？如果是字段级 merge，深层合并（嵌套 map）还是浅层合并（顶层 key 替换）？当前 `.agent/policies/modes.yml` 有 4 层嵌套结构。
2. **传播方向**：是中央→项目（推送式），还是项目→中央（合并式）？如果是推送式，谁有推权限？如果是合并式，变更冲突如何解决？
3. **版本延迟**：中央更新 modes.yml 后，项目 A 要在多久内跟进？有没有 grace period？不跟进是 warn 还是 block？

**建议的最小可行设计**：不做字段级 merge。子仓用**独立但可被审计的配置文件**——`forge check --origin` 检查子仓策略是否与中央策略一致，不一致则告警但不阻断。这就把「强制继承」降级为「漂移可见」，给了每个项目自主呼吸的空间。

---

## 3. 接口设计原则

### 3.1 `AgentExecutor` 接口的扩展方向

当前接口（两个实现：`DryRunExecutor` / `CommandExecutor`）是 forge-core 最关键的扩展点。建议以下演进原则：

**原则一：Executor 是适配器，不是抽象**

Executor 应当保持足够薄，使之成为不同 agent CLI 的适配器，而不是 agent 行为模型。具体来说：

- 不应引入 `AgentAbstraction` 层（不要把 Claude / Codex / Gemini 的行为统一建模）。
- 每个 executor 知道如何为一个具体的 CLI 构造 argv、解析 stdout/stderr、提取 verdict。
- 共享逻辑（超时、输出 cap、budget guard）放在 `Engine` 层，不在 executor 中重复。

**原则二：ReplayExecutor 不应是 CommandExecutor 的特化**

评估文档的方向五建议把 ReplayExecutor 作为第三种 AgentExecutor。这是正确的——ReplayExecutor 的行为（从 trace 回放 mock 输出）与 CommandExecutor（调真实进程）有本质区别：

- CommandExecutor 的输入是 `(prompt, context)`，输出是 `(files, verdict, cost)`。
- ReplayExecutor 的输入是 `(phase_name)`，输出是 `(files, verdict, cost)`——从 trace 查表。

不应把两者合并为一个「带 replay 模式的 command executor」，那会产生大量的 `if replayMode { ... } else { ... }` 分支。

### 3.2 是否需要新的抽象层

**需要：状态机层**

讨论 1.4 的技术债时提到的：LoopEngine 的控制流（gate FAIL → loop-back / abort / retry / human-gate reject）目前是隐式的，散布在多个文件中。

当前代码中，`loop.go` 的 `RunFrom` 方法有 ~300 行，包含了 checkpoint resume、human-gate reject、loop-back、retry、converge 检查、mode-gating 跳过……多条控制流路径交织在一起。

建议引入**显式的 phase 状态机**，但不是用状态模式（State pattern）——那太重量级。而是用一个**可审计的决策矩阵**：

```go
// 伪代码：决策矩阵而非状态机
type Transition struct {
    Current  PhaseStatus    // running / failed / rejected / gate-fail
    Gate     GateResult     // pass / fail / n/a
    Stop     StopCondition  // roadmaps-green / gates-green / human-approved / rejected
    Next     TransitionAction // abort / loop-back(phase) / retry / next-phase / converge
}
```

这个矩阵可以用表格形式写在注释中（类似硬件状态机的状态迁移表），由 lint 规则检查枚举覆盖的完整性。不需要运行时状态机框架。

**不需要：服务发现 / RPC 层**

当前 CLI 单进程架构不引入 RPC layer。如果要拆微服务，那也是引入一个**进程边界**（IPC），而不是网络边界（gRPC）。CLI 到子进程的通信通过 `os/exec` + JSON stdout 已经足够。

### 3.3 向后兼容原则

对于 forge-core（v2 运行时），我建议以下向后兼容契约：

| 层级 | 兼容级别 | 具体约束 |
|------|---------|---------|
| CLI flags | **永远兼容** | `forge run --help` 列出的 flags 不被删除，只被 deprecated |
| checkpoint schema | **向前兼容 N-2** | 新 forge 可读旧 2 个版本的 checkpoint；旧版本读到新 checkpoint 诚实报错 |
| scorecard schema | **字段只增不删** | Scorecard 新字段可选（zero-value = N/A），不改变现有消费者 |
| trace JSONL | **只增字段** | 同 scorecard，新字段 optional |
| `.agent/` 骨架 | **按 ADR 变更** | 需要 ADR 批准才能改变骨架契约（如 modes.yml 的顶层结构） |

---

## 4. 技术选型

### 4.1 YAML 解析：手写 vs 第三方库 vs 编译 Python

如方向 C 讨论，我建议手写。但这里补充一个更细致的评估标准：

| 标准 | 引入第三方 YAML 库 | 手写 YAML 子集解析器 |
|------|-------------------|-------------------|
| 零依赖承诺 | ❌ 打破 | ✅ 保持 |
| CVE 面 | 每年 ~1-3 个 | 0（控制解析范围） |
| 功能完整度 | YAML 1.2 完整 | 仅 forge-core 子集 |
| 正确定义 | 不需要 | 需要差分测试套件 |
| 代码量 | ~0 | ~500-800 行 |
| 维护成本 | 版本升级 | bugfix（差分测试守门） |

对于 forge-core 的需求，手写解析器的子集足够：不需要 Anchor/Alias、不需要 Tag、不需要多文档、不需要 `!!str` 类型标签。只需要：mapping、sequence、scalar（含 block scalar）、注释、缩进。

### 4.2 持久化：JSONL 文件 → 嵌入式数据库？

当前 `persist` 包写平面 JSONL 文件。对于单节点 CLI 这是够用的，但有明显天花板：

- **什么时候该升级？** 当 `forge evolve` 需要同时跑多个 workflow 时（当前一次一个），或者当 `forge scorecard query` 需要按模型/agent/时间范围做聚合时。
- **用嵌入式 DB 还是文件？** 我建议**不升级，而是推迟到微服务架构**。当前 JSONL 文件虽然笨拙，但它是**无依赖、可审计、可 grep** 的——用 `jq` 就能查，不需要 `psql`。这在调试和取证中的价值大于「查询性能」的价值。
- **如果非要升级**，选择 `bbolt`（嵌入式、纯 Go、零依赖）而不是 SQLite（需要 CGO）。

### 4.3 自建 vs 采购

north-star 已经有了明确的买/建决策表。我补充一个**触发条件**而非时间表的决策框架：

| 组件 | 当前状态 | 触发自建替换的条件 |
|------|---------|-------------------|
| YAML 解析 | Python shim（临时） | 本方向 C——**当前即可触发** |
| 持久化 | JSONL 文件 | 跨 workflow 并发或按模型聚合查询出现 |
| 模型路由 | Go `internal/routing` | 跨厂商路由需要（v3 路线图） |
| 沙箱隔离 | 本机 claude 进程 | 任何非本机执行或不受信代码执行需求 |
| Web UI | 无 | 有外部用户访问需求 |
| 持久等待（Temporal） | `exit 1` fail-closed | 人审等待时间超过 CLI 进程生命周期 |

**一个通用的决策原则**：当某个组件的缺陷**被项目自身真实地感受到**（而非推测地担心），才启动自建替换。forge-core 当前很多组件（JSONL 持久化、CLI 架构、手写 YAML 解析器）都是够用的——不要去修复没有坏的东西。

---

## 5. 实施路线图

### 优先级排序

```
P0（当前 sprint 可落地，高价值，低风险）
├── 方向 A: ReplayExecutor 的最小可行版本
│   ├── 新增 ReplayExecutor 实现 AgentExecutor 接口
│   ├── 从 trace JSONL 读取 phase 事件
│   └── 模拟 agent 输出（先只支持 verdict token，后支持文件编辑）
│
├── 方向 B: Wave-aware checkpoint schema
│   ├── Checkpoint struct 增加 WaveIndex
│   ├── 并行 phase 完成时串行化 checkpoint 写入
│   └── Recover 路径理解新 schema（旧 schema 降级为全重跑）

P1（下个 sprint，高价值，中风险）
├── 方向 C: YAML 解析 Go 内建化
│   ├── internal/yaml 包实现 forge-core 子集（mapping/seq/scalar/block）
│   ├── 差分测试套件（对比 Python 参考实现输出）
│   └── 替换 python shim 调用点
│
├── 方向 D: ReviewScore 接入 scorecard
│   ├── Scorecard struct 扩展 ReviewerScore 字段
│   ├── review phase verdict → scorecard 管道
│   └── HistoryTiebreak 可选使用 ReviewerScore 权重

P2（远期，依赖阶段条件）
├── 方向 E: 跨项目舰队最小传播层
│   └── 触发条件：被治理项目 ≥ 3 且治理变更不同步被实际感受到
│
├── 单进程 → 多进程架构（IPC 而非 gRPC）
│   └── 触发条件：任意引擎成为独立性能瓶颈
│
└── JSONL → 嵌入式数据库（bbolt）
    └── 触发条件：跨 workflow 并发或复杂聚合查询出现
```

### 阶段划分

**阶段 1（当前 sprint）**：P0 两个方向并行推进

- 方向 A（ReplayExecutor）影响面窄，主要触及 `executor.go` + `trace` 包 + `replay_test.go`。
- 方向 B（Wave-aware checkpoint）影响面宽，但属于**假设没有并行 checkpoint 时不影响现有串行路径**，可以安全地以实验模式引入。

**阶段 2（下个 sprint）**：P1 两个方向

- 方向 C（YAML 内建化）必须在阶段 2 完成，不能继续推迟。Python shim 已被证明是脆弱的，且每次修改需要跨语言验证。
- 方向 D（ReviewScore → scorecard）可以与方向 C 并行，两方向几乎没有交集（YAML 在解析层、scorecard 在持久化+路由层）。

**阶段 3（远期）**：P2 方向，观测触发条件

- 方向 E 不设具体时间线，设**可测量的触发条件**。建议在 `.agent/CURRENT_SPRINT.md` 的「下一前沿」中明确写出触发条件，每次 sprint 回顾时评估。
- 微服务拆分同理。

### 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| ReplayExecutor 的 trace schema 扩展过大 | 中 | trace 从 observability 格式变成测试格式，schema drift | 保持 trace 的 observability 主体不变，ReplayExecutor 的扩展字段加在独立区块 |
| Wave-aware checkpoint 与现有 `persist.Recover` 不兼容 | 高 | 旧 checkpoint 无法恢复 | 用版本号标记新格式，旧格式降级为全重跑（诚实告知用户） |
| 手写 YAML 解析器产出与 Python shim 不一致 | 中 | 工作流解析错误 | 差分测试必须 blocking（非 `t.Logf`）；阶段 2 保留 Python shim 作为参考 |
| ReviewerScore 接入后 HistoryTiebreak 排序权重不合理 | 低-中 | 路由质量不升反降 | 默认权重为 0（不生效），由显式配置开启 |

### 里程碑定义

```
M1（阶段 1 完成）
├── forge-core 新增 ReplayExecutor，可通过 --executor replay --trace <path> 调用
├── forge replay <trace-path> 子命令
├── RunParallel 的 checkpoint 支持 WaveIndex
└── 至少一个完整端到端 trace 可被回放并产生相同 verdict

M2（阶段 2 完成）
├── forge-core 零 Python 运行时依赖（YAML 解析内建）
├── Scorecard 可记录 reviewer 评分
├── forge route --scorecard 输出 reviewer 评分维度
└── 所有 forge-core 的 Go 测试不依赖外部 Python

M3（远期条件触发后）
├── 首个非本机项目通过 forge-init 建立并被 forge check 审计
├── 存在分布式 checkpoint（非 JSONL 文件）
└── 通过 ADR 记录微服务拆分的决策理由
```

---

## 总结

评估文档的五方向分析整体质量高，四/五方向代码级引用精确。方向四的论据需要校准（Scorecard 有 QualityScore，HistoryTiebreak 在用），但其核心论点——质量维度深度不足以支撑更精细的路由决策——是有效的。

当前 ForgeOS 架构最突出的两个特征：**扎实的治理驱动设计**（.agent/ 骨架、中枢旋钮、FUNCTIONAL_REQUIREMENTS_AUDIT 的审计闭环）和**务实的阶段限制**（CLI 单进程、零依赖、不提前做微服务）。这两个特征的共同源头是同一个原则：**不做没有真实触发条件的架构投资**。

我的建议分为三层，与这个原则一致：

1. **P0**（当前可做，无需外部条件）：ReplayExecutor + Wave-aware checkpoint。两个方向都有明确缺口，修复路径清晰，不影响现有串行路径。
2. **P1**（近期桥接）：YAML 解析内建化 + ReviewScore 反馈闭环。前者的条件已触发（Python shim 已被证明可破坏生产数据）；后者是有收益但非瓶颈的深化。
3. **P2**（远期）：跨项目舰队 + 微服务拆分。依赖真实触发条件（被治理项目数、引擎性能瓶颈），不做预成熟设计。
