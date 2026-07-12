Now I have a thorough understanding of the system. Let me synthesize the architectural analysis.

---

# 架构分析：ForgeOS 五个扩展方向的元评估与技术审阅

> **分析基线**: 原始分析文档（方向一→五）、审计报告（逐方向事实核查）、ForgeOS 架构声明（`ARCHITECTURE.md` / `north-star.md` / `ROADMAP.md` / 31 轮 sprint 记录）

---

## 一、架构评估

### 1.1 当前架构的核心优势

ForgeOS 的架构在经过 31 轮 sprint 迭代后，展示出**罕见的架构成熟度**，尤其是在以下几个方面：

**1. 控制面/数据面分离的清晰性**
`north-star.md` 明确遵循 k8s 式控制面/数据面分离。当前 `forge-core`（Go 运行时）处于控制面位置，harness 闸门处于数据面/执法位置。中间通过 `yaml2json.py` shim + shell-out 桥接。这个分离是**真实的而非纸面的**——见证如 `agentExecutor` 通过回调而非耦合方式接入 `costSink`/`gateLedger`/`OnGateResult`。

**2. 带外执法（out-of-band enforcement）作为真相之源**
与依赖 CLI hook（CC PostToolUse）作为加速器的策略正确地将「强制」与「加速」分离。`harness/gate.mjs`、`arch-check.mjs`、`check.py` 全部是 host-independent 的独立可执行体，不依赖宿主 CLI 的合作。这是整个工程的红线保障。

**3. 自我治理的元一致性**
ForgeOS 对自己的代码库执行相同的 8 项架构检查（layering / package / fan-in / cognitive / naming / function-length / circular-dependency / drift-guard）。这不仅仅是一种纪律展示——它在 Sprint 27-31 中反复捕捉到真实问题（`cmd/forge` 文件预算超标、`gates.go` 500 行逼近、test_acceptance `copy-anywhere` 漂移）。**架构师自己遵守自己定的规则，这是架构治理的最高形态。**

**4. 中枢旋钮（mode × lifecycle）的一致性**
一个 `project.yml` 的 `mode + lifecycle` 二元组同时驱动三处 Router 档位、Harness 严格度（gate-set + enforce + coverage 阈值）、Workflow 深度（discover/design/adr/reviewer/evolve）。Sprint 15 完成全维度接线，Sprint 18 完成 enforce 严格度，Sprint 31 完成 mode_gating 漂移守卫。这是**真正的正交设计**——一个轴转动，多个维度响应。

**5. 诚实机制贯穿架构**
不是 meta 层面的修辞，而是嵌入代码的行为：trace 中 `N/A` 与 `0` 的区分、coverage 工具不可用时诚实降级、`honesty` 注释标注已知限制而非假装完整。这种「反镀金」决策文化在架构层面的体现是：承认哪些事 **故意不做** 并给出理由（如 `blocking: false` 不实现、多维路由评分自我推迟至 v2+）。

### 1.2 当前架构的关键约束与局限

**1. 单体 CLI 架构的上限**
尽管 `forge-core` 是 13 个 Go 包的模块化设计，但所有操作通过单一 CLI 二进制 `forge` 暴露。这意味着：
- 无法独立升级/部署子服务（所有功能耦合在同一个发布周期）
- 无法水平扩展（一个 `forge evolve` 进程独占编排权）
- 长期运行时的状态管理（checkpoint/trace/memory）全部在本地文件系统，无法跨机器迁移
这是 **v0→v1 架构决策的合理取舍**（简化部署、零依赖），但也是明确的技术债——`north-star.md` 的目标架构是 Temporal 驱动的微服务拓扑。

**2. YAML 解释的 shim 依赖**
`yaml2json.py` 是唯一的 Python 依赖，且是通往 Go 完整解析的临时脚手架。虽然 Sprint 27 曾尝试用纯 Go YAML 解析器替代，但该尝试被自身的架构纪律约束住了（Go 零依赖）。目前在 `go.mod` 仍保持 `require` 为零。这个 shim 意味着：
- 每次 `forge run/evolve` 多了一个进程 fork 和 IPC 步骤
- YAML 行为取决于系统 Python 环境（潜在的漂移源）
- 不可能在 Go 端做编译时 YAML 校验

**3. Trace 与 Scorecard 的持久化范式不统一**
当前 `trace.jsonl` 是 append-only 事件流，`memory.jsonl` 是键值积累（有 `Compact/Prune`），`checkpoint.json` 是状态快照（有 `rotateRetain`）。三种存储范式各有不同的生命周期管理策略——但 trace 的审计级持久性要求与它的无界增长之间存在根本张力。这个张力**不是接线问题，是存储模型设计问题**：trace 是 append-only event log，天然不适合用文件轮转管理，正确的归宿是外部日志系统（ELK/Loki/Temporal 的事件历史）。

**4. 跨语言政策消费者之间的碎片化**
Go (`internal/mode`)、Node.js (`acceptance-quality.mjs`)、Python (`check.py`) 三个运行时各自解析 `modes.yml`/`policies.yml` 的子集，没有共享的解析核心或代码生成。目前靠手动同步和测试覆盖维持一致性，这随扩展集会越来越脆弱——审计报告指出 Go 端实际上没有 coverage 阈值解析（只有 Node.js 有），说明跨语言政策消费的「事实边界」与「假设边界」已经出现偏差。

**5. 反馈回路的数据质量差距**
虽然 Sprint 26 已通过真 `claude` 跑出三维真数据（quality + latency + cost），`scorecard` 已收集、`HistoryTiebreak` 已实现，但从 scorecard 到路由决策的完整闭环**虽然已接线（审计纠正了原始分析的方向三错误）**，但其行为过于保守——只做冷启动 fallback 和薄数据择优，不做主动的质量驱动的动态路由调整。这是一个「基础设施就绪，但策略未开」的状态。

### 1.3 架构债务评估

| 债务类型 | 严重程度 | 影响范围 | 偿付时序 |
|---------|--------|--------|---------|
| YAML shim 依赖 | 中 | 每次 `forge run/evolve` 的 5ms+ 延迟 + 环境漂移 | v2 中期（可随 Go YAML 库引入解决） |
| CLI 单体二进制 | 高 | 无法独立部署/升级/水平扩展 | v3（依赖 Temporal 引入） |
| Trace 存储模型不匹配 | 中-高 | 长运行时存储不可控 | 当前（P2）应至少 retain-N + 归档 |
| 跨语言政策消费者碎片化 | 中 | 新增模式/策略时的同步成本和漏检风险 | 当前（P3）应加对账 |
| `forge run --summary --json` 缺失 | 低-中 | CI 集成需 parse 文本 | 当前（P2）高价值增量 |

---

## 二、高价值扩展方向

基于前述评估和对原始分析 + 审计的反思，我提出以下**优先级的架构扩展方向**。与原始分析不同的是，我聚焦在**架构模式层面的扩展**而非单个功能点。

### 方向 A（优先级 P0）：Prompt 装配预算 —— 运行时总量守卫

**原始分析编号**: 方向一（P1）
**我的修正**: ⬆ **P0** — 这是正确性问题，不是优化问题

**为什么需要（从架构层面）**：
当前 `buildPromptWithEmits` 将 7+ 上下文 lane 拼接后直接送入 LLM。Token 窗口是 LLM 交互中最硬的上限——超限后模型静默截断输入，导致关键上下文丢失（role card / ADRs / gate results）。这不是性能退化，是**正确性丧失**。审计确认 6/6 主张正确（仅 `memoryCap` 为 32 而非 ~15 的细节偏差），且 `gateLedger.context()` 和 `findingsContext` 虽然受硬编码限制不增长，但 role card 和 emits 文件确实无界。

**核心挑战**：
1. 预算超限后的降级策略需要定义「lane 优先级」——这本质上是**声明式 prompt 配置**的架构问题，而非运行时的一个 if 判断
2. Token 的估算（rune→token 映射）在 Go 端不是精确的——需要与模型分词器对齐
3. `buildPromptWithEmits` 和 `Gather` 两个拼装入口需要统一守卫点（当前各自独立）

**架构变更**：
- 在 `prompt_context.go` 引入 `PromptBudget` 结构体，集中管理所有 lane 的预算上限和优先级
- 将 lane 优先级声明化（从硬编码改为 role card 可覆盖的配置）
- 拼装后统一 `checkPromptBudget` 检查，超限时按优先级逐级降级

**对现有系统影响**：影响面局限在 `internal/prompt` 包，不对 orchestrator/loop/routing 等外部 API 造成任何变动。

### 方向 B（优先级 P1）：结构化 Run 完成摘要 — CI/CD 集成接缝

**原始分析编号**: 方向五（P3 → 审计建议 P2）
**我的修正**: ⬆ **P1** — 架构上这是「可观测性最后一公里」

**为什么需要**：
ForgeOS 的核心价值之一是「自治软件工厂」。但一个黑箱工厂是不可运营的——CI 系统、Dashboard、成本追踪系统、审计日志都需要**机器可消费**的完成摘要。当前 `LoopEngine.RunMany` 返回的有 `LoopOutcome`（iteration/cause/convergence），但 `cmdEvolve` 和 `cmdRun` 都将其格式化为一行人类文本后丢弃。结构化输出是 CI 集成的**必要条件**，不是锦上添花。

**核心挑战**：
1. 摘要的数据源是 trace 事件——但 trace 是 append-only 的实时流，摘要需要聚合（sum/avg/count）而非简单转储
2. 摘要的 schema 需要向前兼容——CI 流水线可能依赖字段名
3. 摘要时机：`forge run --summary --json`（执行后即时聚合）vs `forge trace query --summary`（事后从已有 trace 查询）

**架构变更**：
- 在 `internal/trace` 包新增 `Summarize(events []Event) RunSummary`，纯函数式聚合（无副作用）
- `loop.go` 的 `RunMany` 结束时持有了所有 iteration 的 events，可在返回前调用 `Summarize`
- CLI 层新增 `--summary --json` flag，不走 stderr 而写 stdout（方便 CI 捕获）
- `Summary` 结构体按 JSON schema 版本化

**对现有系统影响**：零侵入——`loop.go` 的 `events []Event` 已存在于 `RunMany` 作用域内；`trace.Event` 的 `Kind/DurationMs/CostUsdMicros/Status` 字段全部已有。唯一新增的是聚合过程和 JSON 序列化。

**两个选项对比**：

| 选项 | 方案 | 优点 | 缺点 |
|------|------|------|------|
| A（推荐） | `LoopEngine` 内建 `Summarize` 后吐 JSON 到独立文件 `.forge/summary.json` | 与执行生命周期耦合，确保数据完整 | 需在 engine 中新增文件写入职责 |
| B | `forge trace query --summary` 独立命令，事后聚合 | 职责分离，不修改 engine | 需要持久化的 events buffer 或重新解析 trace.jsonl |

**推荐**：先用 **选项 A**（engine 内建输出 `summary.json`），`forge trace query --summary` 作为独立路径后续补充。

### 方向 C（优先级 P1）：Trace 生命周期管理 — retain-N + 归档

**原始分析编号**: 方向二（P1 → 审计建议 P2）
**我的修正**: **P1** —— 审计发现 10MB rotation 已实现，但只存一份 `.1` 备份。对于 24h 无人值守场景（已真点火验证）这是不够的。

**为什么需要**：
ForgeOS 已跑通 24h+ autonomouos evolve（Sprint 24-26）。trace 是事后审计和调试的唯一原始数据源。当前只保留当前文件 + 一个 `.1` 备份，意味着：
- 一次长跑覆盖所有旧 trace 数据
- 用户无法回溯 N 次迭代前的 trace
- 磁盘无预警地持续增长

**核心挑战**：
1. Trace 是 append-only event log，与 memory（键值积累）和 checkpoint（状态快照）的生命周期模型完全不同——不能 compact（事件序不可破坏），不能 prune（审计需要历史）
2. 归档需要压缩、标记、可查询——这是「存储管理」子系统的职责，不是 `trace.Tracer` 的职责

**架构变更**：
- `trace.go` 的 `Tracer` 增加 `rotateRetain(N int)` 模式（镜像 `checkpoint.rotateRetain`）
- 新文件 `trace_archive.go`：将旧 trace 文件移至 `.forge/archive/` 子目录，`.tar.gz` 压缩
- `forge status` 展示 trace 统计（当前大小、估计可回溯迭代数）
- `forge trace list` 显示归档清单，`forge trace cat --archive N` 读取旧归档

**对现有系统影响**：`trace.go` 的 API 兼容（新增方法而非修改签名）；`evolve.go` 的 `openTracer` 需传递 retain 参数；CLI 新增子命令。

### 方向 D（优先级 P2）：声明式 Policy 一致性校验框架

**原始分析编号**: 方向四（P2 → 审计建议 P3）
**我的修正**: **P2** —— 虽然审计纠正了三端 coverage 的假想问题（实际仅一端存在），但 gate 词表和 depth 枚举值的跨语言漂移风险是真实的。

**为什么需要**：
当前三个运行时（Go / Node.js / Python）各自维护对 `modes.yml` 和 `policies.yml` 的解析逻辑。没有中心化的「权威值」声明和对账机制。虽然 `check.py` 的 `check_workflow_mode_gating`（Sprint 31 新增）已检查 workflow 与 modes.yml 的 mode_gating 漂移，但以下尚未被覆盖：
- Gate 名注册表（Go 的 `fullGates` 数组 vs Node.js 的 gate list vs Python 的 gate 检查）
- workflow_depth 枚举（Go 的 `EvolveDepth` 常量 vs modes.yml 的字符串字面量）
- model_tier 枚举

**核心挑战**：
1. 对账框架不能成为第四个运行时——应轻量、声明式
2. 不能强制所有消费者从同一个源自动生成代码（Go/Nodes.js/Python 各有独立的构建周期和部署单元）
3. 对账应该是 **pre-commit 门**而非运行时门

**架构变更**：
- 新增 `forge validate --policies` 命令，聚合当前三个运行时的值并输出差异
- 在 `modes.yml` 或 `policies.yml` 中声明**权威 gate 注册表**（JSON Schema 格式），所有消费者从此生成
- `check.py` 新增 `check_gate_catalog` 和 `check_depth_enums`，与 Go 常量的硬编码值交叉校验
- 这个对账是**观测性而非阻断性**（报告不一致但不阻止执行）

**对现有系统影响**：零侵入——不修改任何已有行为，只新增校验路径。

### 方向 E（优先级 P2/P3）：Scorecard 驱动的动态路由策略开放

**原始分析编号**: 方向三（P2 → 审计建议不提交）
**我的修正**: ⬆ **P2（增量优化）** —— 审计正确纠正了原始分析的「接线断开」事实错误（`phaseTierResolver` 已接 HistoryTiebreak），但原始分析提出的增量方向仍有价值。

**为什么需要**：
当前 `HistoryTiebreak` 已接线但行为保守——它只在 `minSamples` 阈值以上时才影响路由，且仅在同 tier 候选人间做择优。如果历史上 Opus 对某 task_type 的平均质量低于 Sonnet，当前路由不会主动降级。对于**成本优化**和**质量优化**而言，当前策略是「只有数据，没有决策力」。

**核心挑战**：
1. 跨 session 历史积累——当前 scorecard 数据只在一个 evolve session 内有效，被 `forge route` 命令行消费。Sprint 26 证明了 trace 的 `cost_usd_micros` 和 `duration_ms` 落盘真实，但 scorecard 的跨会话持久化尚未实现路线图
2. 路由策略的安全下限（risk ≥ critical 强制 Opus）必须始终高于历史数据驱动的优选的优先级——history-aware 路由只能在同一 risk 级别内做优化，不能越级降级
3. 用户预期管理：当前静态路由可预测（agent X → tier Y），history-aware 路由需要可解释性

**架构变更**：
- `PhaseTier` 或 `phaseTierResolver` 增加 `--history-aware` flag（默认 off，向后兼容）
- 启用时：`HistoryTiebreak` 的优选从 advisory 升级为对 `agentTier` 静态值的 override（但仍受 risk 下限压制）
- 跨 session 历史：`forge route --scorecard` 写入 `.forge/scorecard-history.json`

**对现有系统影响**：`internal/routing` 包的 API 为零变化（`HistoryTiebreak` 已存在），`phaseTierResolver` 的行为通过 flag 控制。影响面最底。

---

## 三、接口设计建议

### 3.1 关键原则

**1. 一切状态可序列化的结构化格式**
`trace.Event` 已正确使用结构化字段（`Kind/Name/Status/DurationMs/CostUsdMicros/Model/Seq`），这是一个好模式的延续。以下接口同样应输出结构化格式：
- `LoopOutcome` → 新增 `ToJSON()` 方法（当前只有 `String()`）
- `forge run --summary` → 输出 `Summary` 结构体
- `forge status` → 支持 `--json` flag

**2. 外部消费者（CI/CD）的契约应通过文件而非 stdout 达成**
- `forge run --summary --json` 应同时写 `.forge/summary.json`（结构化）和 `stdout`（人类可读）
- 这样 CI 可以挂载 `.forge/` 目录读取结构化结果，而非依赖 stdout 解析
- 这也是 trace/checkpoint/memory 的既有模式——它们全部写入 `.forge/` 目录

**3. Policy 消费者应共享一个单源事实（SSOT）**
当前三个运行时的政策解析各自独立。虽然在 Go 中引入 YAML 库会破坏零依赖纪律，但可以：
- 用 JSON Schema 定义 `modes.yml` 和 `policies.yml` 的权威结构（语言无关）
- 每个运行时从 Schema 生成类型定义（Go 用 `go generate`，Node.js 从 JSON 推导，Python 用 `datamodel-code-generator`）
- 这不是强制所有消费者自动同步，而是让类型定义与 Schema 的漂移**可以被自动检测**

### 3.2 是否需要新的抽象层

**是的，在以下两个领域需要引入抽象层：**

**1. Prompt 装配的「预算分配器」（Budget Allocator）**
当前 `buildPromptWithEmits` 和 `Gather` 是「拼装工」而非「分配器」。当 token 预算超限时没有任何降级策略。建议引入 `PromptBudget` 抽象：
- 每个 lane 声明 `priority` 和 `maxTokens`
- 总预算超过 window 80% 时，按优先级从低到高截断
- Role card 可声明其 lane 的 `priority`（默认为中间）

这个抽象不是新包，而是 `internal/prompt` 内部的补充类型，不对外暴露。

**2. Policy 一致性校验的「对账器」（Policy Reconciler）**
不是新的运行时，而是**一个独立于所有运行时之外的（轻量）校验模式**：
- 读取 modes.yml 的权威值
- 分别运行三个消费端的分析模式（非完整运行时）收集各自的值
- 输出对账矩阵

### 3.3 向后兼容性策略

| 变更 | 兼容策略 |
|------|---------|
| `buildPromptWithEmits` 加总量检查 | 纯新增行为——超限前不会告警；不会改变输出内容（只截断不扩增） |
| `LoopOutcome` 加 `ToJSON()` | 纯新增方法——不改变 `String()` 行为 |
| `trace.Tracer` 加 `rotateRetain` | 加参数默认值 `0 = disabled`（与当前行为逐位兼容） |
| `PhaseTier` 加 history-aware flag | `--history-aware` 默认为 false |
| `forge run --summary --json` | 新增 flag，不修改 `--help` 现有输出 |
| Policy Schema 定义 | 纯文档 + 校验工具层，不修改运行时行为 |

---

## 四、技术选型

### 4.1 是否引入新的技术栈或框架

**不建议在本阶段引入新的外部依赖。** ForgeOS 的零依赖纪律是架构的优势而非妨碍。三个方向可以在零外部依赖下实现：

| 方向 | 所需技术 | 外部依赖 | 理由 |
|------|---------|---------|------|
| Prompt 总量守卫 | Token 估算 + lane 优先级 | **零** | 用 rune 计数近似（与 `taskCap=4000` 同模式），不引入分词器 |
| 结构化摘要 | JSON 序列化 | **零** | Go `encoding/json` 已就绪 |
| Trace retain-N | 文件轮转 + gzip | **零** | Go `archive/tar` + `compress/gzip` + `os.Rename` 全在标准库 |
| Policy 对账 | JSON Schema 校验 | **零** | 用 Go `encoding/json` 做 schema 验证（子集，不完整 JSON Schema） |
| History-aware 路由 | 条件分支 + flag | **零** | 纯逻辑变更，不依赖新框架 |

**唯一可能打破零依赖的是跨 session scorecard 持久化**——如果选择用嵌入式数据库（如 SQLite via CGO）替代 JSON 文件。但我**建议不这么做**：用 `.forge/scorecard-history.json` 足够容纳 1000+ session 的聚合分，性能完全在线性搜索范围内。

### 4.2 自建 vs 采购的原则

延续 `north-star.md` 的 Buy vs Build 原则：

| 能力 | 决策 | 理由 |
|------|------|------|
| Token 分词 | **自建 rune 近似** | 精确 token 计数需引入模型对应分词器（如 `tiktoken`），这会引入 CGO/Python 依赖且需要随模型更新同步。rune 近似（如当前 `taskCap=4000 runes`）已跑通 200+ iteration，精度足够。 |
| 结构化摘要格式 | **自建 JSON schema** | 无采购选项——这是 ForgeOS 自有的输出格式 |
| Trace 归档 | **自建 tar.gz** | Go 标准库直接支持，零额外成本 |
| Policy Schema 对账 | **自建校验器** | 无现成的「跨运行时政策对账」产品——这是 ForgeOS 治理模型的独特性需求 |
| SCA 漏洞库 | **框架已就绪，差 DB 供应商** | Sprint 19 已实现 OSV-format 解析引擎 + semver 匹配，只差接入 OSV/NVD 数据源。这是正确的 Buy 决策——不造漏洞库轮子 |

### 4.3 关于 Go YAML 库的审慎建议

当前 `yaml2json.py` shim 是架构中最明显的临时依赖。引入 Go YAML 库（如 `gopkg.in/yaml.v3`）会打破零依赖纪律，但有正反两面：

**反对引入的理由**：
- 零依赖是 forge-core 的独特卖点和架构优势——`go.mod` 零 `require` 意味着 `go build` 永远不失败、无 CVE 传播、无 license 合规问题
- YAML 解析在 forge-core 的热路径上（每次 `forge run/evolve` 启动时解析 workflow），但整个关机时间 < 50ms，引入 Go YAML 库不会显著改善
- Sprint 27 的手写解析器尝试证明了准确实现 YAML 规范的高成本——但那是为了 100% 兼容，现在 99% 场景已经覆盖

**支持引入的理由**：
- 消除 Python 环境依赖——CI/CD runner 不必安装 Python + PyYAML
- 编译时类型安全——Go 结构体直接反序列化，而非 JSON 转手的中间表示
- 本仓的零依赖是可打破的（DECISIONS.md D6 的 `go.mod 无 require` 是对 v2 启动时的状态描述，非红线承诺）

**推荐（折中）**：**不在本 sprint 引入**。在 YAML shim 造成可测量的问题时（如 Python 升级破坏兼容、CI runner 环境不一致），再引入 Go YAML 库。届时引入时应选择 `gopkg.io/yaml.v3`（最成熟、最接近规范）。这个决策已写在本仓的 ADR/DECISIONS.md 语境中。

---

## 五、实施路线图

### 5.1 总优先级排序

基于审计报告的修正和架构分析，我的优先级排序如下（与原始分析的 P1→P3 排序不同）：

| 优先级 | 方向 | 原始优先级 | 预估 | 杠杆 | 依赖 |
|--------|------|-----------|------|------|------|
| **P0** | Prompt 总量预算守卫 | P1 | 0.5 sprint | ⭐⭐⭐⭐⭐ | 无 |
| **P1** | 结构化完成摘要 | P3 → P2↑ | 1 sprint | ⭐⭐⭐⭐⭐ | `trace.Event` 已就绪 |
| **P1** | Trace retain-N + 归档 | P1 | 1 sprint | ⭐⭐⭐⭐ | 10MB rotation 已就绪 |
| **P2** | Policy 一致性校验 | P2 → P3 → P2 | 1 sprint | ⭐⭐⭐ | `check.py` 模式已就绪 |
| **P2/P3** | History-aware 路由增量 | P2 → 建议不提交 | 1.5 sprints | ⭐⭐⭐ | `HistoryTiebreak` 已就绪 |

### 5.2 阶段划分

**阶段 1（0.5 sprint）—— P0 快速止血**
- `PromptBudget` 结构体 + `checkPromptBudget` 检查
- `memoryCap` 修正为 32（审计发现的偏差）
- Role card 截断规则（>100 行时保留头部 + 机读契约）
- Emits 文件 rune cap（`emitCap=4000`）
- 修正 findings/gate boundary 分析的描述

**阶段 2（1 sprint）—— P1 高杠杆增量**
- `LoopOutcome.ToJSON()` 方法 + `Summary` 结构体
- `forge run --summary --json` CLI flag
- 聚合逻辑在 `loop.go` 内完成（不新增服务）
- CI consumer 示例（GitHub Actions action 配置）

**阶段 3（1 sprint）—— P1 运营韧性**
- `trace.Tracer.rotateRetain(N)` + 归档至 `.forge/archive/`
- `forge status` trace 统计展示
- `forge trace list/archive` 子命令
- `--trace-retain` flag for `forge evolve`

**阶段 4（1 sprint）—— P2 治理深度**
- `forge validate --policies` 命令
- Gate 注册表在 `policies.yml` 中声明（JSON Schema）
- `check.py` 新增 gate 词表 + depth 枚举对账
- 实现对账结果输出（非阻断）

**阶段 5（1.5 sprints）—— P2/P3 路由优化增量**
- `forge run --history-aware` flag
- `agentTier` 可选降级逻辑（受 risk 下限压制）
- 跨 session scorecard 持久化 `.forge/scorecard-history.json`
- `forge route --summary` 展示历史路由统计

### 5.3 风险点和缓解策略

| 风险 | 影响方向 | 概率 | 缓解策略 |
|------|---------|------|---------|
| Token 估算不精确导致误判 | Prompt 总量检查 | 中 | 使用 rune 近似（保守估算），超限告警而非阻断，让用户调整 lane 优先级 |
| 结构化摘要增加 loop.go 的执行后延迟 | 结构化摘要 | 低 | `Summarize` 是纯 O(n) 聚合，1000 iteration 的 trace < 5ms |
| trace 归档导致异步 I/O 阻塞热路径 | Trace retain-N | 中 | 归档操作异步化（goroutine + 文件锁），不阻塞 `Emit` |
| 跨语言 Policy 对账的假阳性 | Policy 一致性 | 低 | 对账模式默认 warn 而非 block，用 `--strict` 显式开启阻断模式 |
| History-aware 路由的用户预期管理 | 路由增量 | 中 | 默认 off，`forge run --history-aware` 需显式启用，输出解释路由决策（如 `tier: sonnet (history: 85% pass on cost-optimized)` |
| 回归测试对已修正方向三接线判断的依赖 | 整体 | 低 | 保持当前 `phaseTierResolver` 测试覆盖，新增 history-aware flag 场景的独立测试 |

### 5.4 与当前架构轨迹的关系

上述路线图与 ForgeOS 当前的演进轨迹高度一致：

- **Sprint 30-31 的「GAP 收口」模式**延续到阶段 1-4（接线型、小增量、快速验证）
- **Sprint 24-26 的真点火验证范式**覆盖阶段 5（history-aware 路由需要在真 agent 下验证降级行为）
- **Sprint 27 的「先拆分再继续」纪律**适用于所有阶段——如果某一方向导致文件超限或函数过长，必须先重构再生产
- **Sprint 29 的信号完整性模式**（逐字段核对声明→消费者→赋值）适用于阶段 4 的 Policy 对账设计

**建议阶段 1 可以在当前 sprint 的上下文内直接开始**——它是 0.5 sprint 的纯补充，无新包、无新文件（如果符合体积闸门）、无外部依赖。

---

## 总结

本分析的独特贡献在于：**不是重复原始分析的五个方向，而是从审计报告发现的事实错误出发，分析了这些错误反映的更深层系统性问题**：

1. 原始分析的核心主张（方向三：接线断开）在当前 HEAD 已不成立，这说明 **ForgeOS 的 v1.5 演进速度超过了分析者的代码扫描频率**。建议未来的架构分析先 fetch + checkout 最新 HEAD，而非基于记忆或前一次分析。

2. 审计报告指出的「三个方向存在实质性事实错误」本身是一个**方法论的架构问题**：如何保证架构分析的准确性和时效性？答案可能是将 `forge validate --policies` 扩展到包括 **`forge validate --analysis`**（让自动化工具辅助人工分析，而非全凭人力）。

3. 存在一条清晰的**架构演化路径**：从「单体 CLI + 本地存储」（当前）→「独立服务 + 结构化输出」（阶段 2-3 的增量）→「Temporal 驱动 + 外部存储」（north-star 目标）。五个方向中的四个可以在这个路径上简单叠加，不需要任何「路径偏移」。
