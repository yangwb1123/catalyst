# 架构分析：ForgeOS 五个结构性缺口交叉验证

> **分析范围**：基于交叉验证文档 `docs/requirements/2026-07-12-five-uncovered-architectural-gaps-scan.out.md`（以下简称"验证文档"），结合 BOOTSTRAP.md、ARCHITECTURE.md、ROADMAP.md、CURRENT_SPRINT.md、north-star.md 及 31 个 Sprint 的演进记录。
>
> **分析角色**：资深架构师
>
> **核心发现前置**：验证文档本身的"方向五"事实错误（3/4 字段已被消费）暴露了一个**元架构问题**：架构知识的唯一事实源散布在代码注释、agent 卡口述、散落文档三处，无单一来源可用于验证"声明 vs 实现"的一致性。这个问题远大于方向五本身。

---

## 1. 架构评估

### 1.1 当前架构的优势

**分层清晰度** — ForgeOS 的 v2 Go 运行时已成功建立了一个**低耦合、高内聚**的核心骨架。5 引擎（Orchestrator、Model-Router、Context-Engine、Memory-Engine、Evaluation-Engine）+ 18 个纯标准库 Go 包 + 零外部依赖，在"自研 vs 采购"之间选了一条极端的**自研最小主义**路径。这不是技术幼稚，而是一个深思熟虑的架构决策：在外部依赖未确定之前（LiteLLM 厂商尚未接入、Temporal 服务尚未部署、Qdrant 尚未选型），用零依赖的核心逻辑验证编排味道是否正确。

**增量交付纪律** — 从 v0（声明式 `.agent/` 骨架）到 v1（闭环 + Claude 路由）到 v2（Go 运行时落地），每个版本都可独立验证。north-star 微服务架构存在但不绑架当前决策。这是《Building Evolutionary Architectures》的实践范本。

**闸门系统（Harness）** — `gate.mjs`（快速体积）→ `arch-check.mjs`（8 项架构检查）→ `check.py`（治理完整性）→ `secret-scan` → `forge accept`（聚合裁定），形成了渐进式的质量过滤管网。每个闸门 fail-closed（出问题保守阻断）、honest（不可测项 N/A 不伪造）。这在 AI 编排系统中极其重要——因为 AI 输出天然不可预测，闸门是唯一可信的护栏。

**中枢旋钮（mode × lifecycle）** — 用两个维度同时驱动 Router 档位、Harness 严格度、Workflow 深度、migration，是一个出色的**复用与一致性的统一模型**。一个设置改变三处行为，避免了三套独立配置之间的漂移。

**"先拆分，再继续"** — 这不是一句口号，而是 31 个 Sprint 中反复执行的真实纪律。`cmd/forge` 包超过文件数限制后，不是简单抬上限，而是将逻辑拆入 `internal/doctor`、`internal/attribution`、`internal/gate/resolve.go` 等独立包。**每一次"超限"都触发了一次架构重构**，而非权宜放宽。

### 1.2 局限性

**Go 运行时缺乏 YAML 原生支持** — `forge-core` 的"零外部依赖"红线意味着 Go 标准库不包含 YAML 解析器。当前的 Python shim 桥接（`yaml2json.py`）是务实的脚手架，但它引入了**双解析器语义漂移**（验证文档方向四的风险确认真实）。Go 的 `yaml2json.Decode → any → json.Marshal` 往返在 int vs float64、block scalar 处理、序列项空值语义上可能与 Python 的 PyYAML 有差异。方向四的风险**已由 Sprint 27 的 block-scalar 损坏 bug 实际坐实**——那次 bug 导致每个 workflow 文件的 `description:` 字段被注入字面量 `"> "` 前缀，6/7 个真实 YAML 文件跑了全局测试却全绿，因为差分测试使用了 `t.Logf` 而非 `t.Errorf`。

**AgentExecutor 生命周期真空** — 验证文档方向二的判断准确。当前只有一个 `DryRunExecutor` 实现，接口签名为 `Execute(ctx, p, mode) error`，缺少 `Init`、`Shutdown`、`Rollback`、`Health`。这在当前阶段（dry-run 为主）不是阻塞问题，但**当多个 executor 类型共存时**（CommandExecutor、ClaudeExecutor、CodexExecutor、DockerExecutor），缺少生命周期契约会导致：
- executor 资源泄露（未关闭的进程、未清理的临时目录）
- 部分初始化的 executor 被用于 phase 执行
- executor 故障不影响整个 engine 的健康报告

**Agent 输出契约的隐式脆弱性** — 验证文档方向三（P1🥇）判断正确。`parseReviewerVerdict`、`parseConfidenceScore`、`parseClaudeCostUsd` 三个解析器都基于**精确文本匹配**，无 schema 验证、无 fallback 链、无版本协商。更深的架构问题不在解析器实现——而在于**这些契约从未被显式声明为"契约"**。它们散布在 `cost.go`、agent 卡 markdown 段、prompt 模板中，没有一个集中的 `agent-output-contract` 定义文件。这意味着：
- LLM 供应商升级输出格式 → 静默数据丢失
- 新的 agent 角色（如 security-engineer、distributed-engineer）需要重新发明相同的契约机制
- 契约变更无法版本化或协商

### 1.3 架构债务

**声明与实现的时差** — 验证文档方向五的事实错误（3/4 的 ADDED HERE ONLY 注释已过期）揭示了一个**系统性的文档-代码同步缺失**。这不是单个字段的问题——`asset.go` 中的注释是 Sprint 30 的中间状态，而消费代码在 Sprint 31 中已完成，但注释从未更新。在 31 个 Sprint 的快速迭代中，这种时差几乎不可避免。但没有自动化机制（如 `forge validate --consumed-fields`）来检测它。

**Sprint 30 的"4 处仅文档标注"误判** — Sprint 30 最初将 5 个 GAP 标注为"仅文档标注"，Sprint 31 复审后纠正了其中 4 处。这本身就是**架构审查盲点**的具体实例——实现 agent 和 review agent 都在同一个 sprint 内工作，对同一代码库的同一判断可能出错。方向五的验证错误和 Sprint 30→31 的复审纠正，在时间上高度相关。

**`internal/routing` 多维评分器未接入执行路径** — Sprint 30 复核后从 GAP 改为 DEFERRED-BY-DESIGN，但这是一个有代价的推迟：`forge route` CLI 可以显示复杂度/依赖/上下文/业务影响的评分，但 `forge run` 和 `forge evolve` 的实际模型选择不使用这些维度。这意味着路由决策（当前使用分类→评分→tier→下限→预算→历史择优）的**多维信号已经采集但未被消费**——这是数据面与控制面之间的架构断层。

**converge 信号完整性 vs 实际驱动力的不匹配** — `converge.Signals` 目前有 8 个字段全部闭环（经过 Sprint 29 的系统审计），但**并不是所有信号都参与了收敛决策**。`evalRoadmap` 仍然是最主要的驱动力，`evalReviewStatus` 在 REVIEW 段，而 `evalRequirementConfidence`、`FileDelta` 等信号的加权聚合并未形式化。`signals` 更接近"可观察的报告"而非"决策引擎的输入"。

---

## 2. 扩展方向

基于以上评估，以下是 5 个高价值的架构扩展方向，按技术债程度和业务影响排序。

### 方向 A：Agent 输出契约 → 契约注册表与版本化 schema（P1）

**为什么需要**：方向三的脆弱解析器是症状，病因是**契约隐性**。当前有三个隐式契约（reviewer verdict、confidence score、claude cost），每个都是用 `unwrapClaudeResult`+末行精确匹配的临时代码。随着 agent 角色扩展（security-engineer、distributed-engineer、performance-engineer、CTO——north-star 中已声明），每个都需要自己的输出契约。没有注册表，每个新角色都要重写解析器、重写测试、重写 fallback 逻辑。更危险的是——LLM 供应商更改输出格式时，**没有版本协商**来确保向后兼容。

**核心挑战**：
1. **版本协商**——Engine 和 Agent 需要在 phase 开始时交换契约版本（类似 HTTP 的 Accept header），而不是硬编码格式
2. **多级 fallback**——精确匹配 → 结构化解析（JSON/XML）→ 启发式提取 → 安全默认值，需要可配置的 fallback 链
3. **跨供应商格式差异**——Claude 的 JSON envelope 不同于 Codex 的纯文本输出，契约注册表需要能在供应商之间转换

**预期的架构变更**：
```
internal/contract/
  registry.go       ← 注册 AgentOutputContract（verdict/confidence/cost 等）
  version.go        ← 版本协商逻辑
  resolver.go       ← 多级 fallback 链引擎
  builtin/
    verdict.go      ← 内置二元/五择一裁决契约
    confidence.go   ← 置信度评分契约
    cost.go         ← 成本数据契约
```

`cost.go` 中现有的解析器需要抽取为契约实现，保留 `classifyClaudeOverload` 的 envelope-first 稳健策略（已验证比文档描述更优）。

**对现有系统的影响**：低。当前解析器全部在 `cost.go` 中，可以逐步迁移。新契约注册后，旧解析器作为内置契约的默认 fallback 保留。向后兼容通过 `Registry.Lookup("reviewer-verdict")` → 找不到 → 回退到现有内联解析实现。

### 方向 B：Executor 生命周期契约 → 多 executor 运行时（P1→P2）

**为什么需要**：当前 `DryRunExecutor` 是唯一实现。但路线图中 `--executor command --agent-cmd claude` 已在 Sprint 24-26 端到端坐实，`CommandExecutor` 虽然存在但不是 `AgentExecutor` 实现（验证文档方向二确认）。随着 executor 类型增加（ClaudeCodexExecutor、DockerExecutor、RemoteSSHExecutor），缺少生命周期契约会导致：

| Executor 类型 | 需要 Init | 需要 Shutdown | 需要 Rollback | 需要 Health |
|---|---|---|---|---|
| DryRunExecutor | ❌ | ❌ | ❌ | ❌ |
| CommandExecutor | 环境检查 | 子进程清理 | N/A | 进程存活 |
| ClaudeCodexExecutor | OAuth 令牌刷新 | Agent session 关闭 | phase 回滚 | API 可达性 |
| RemoteExecutor | SSH 连接池 | 连接关闭 | 远程工作区清理 | 心跳 |

**核心挑战**：
1. **Init 阶段的语义**——环境检查应该在 Init 时 fail-fast，还是在 Execute 时 lazy-check？当前 CommandExecutor 是后者。每种 executor 的 Init 语义需要形式化。
2. **Rollback 的粒度**——phase 级别 vs step 级别。设计决策需要明确。
3. **Health 检查的契约**——Executor.Health(ctx) 应该返回什么信息（宕机、过载、认证过期）？当前的 Engine 没有集成健康信息。

**预期的架构变更**：
```go
// internal/executor/lifecycle.go
type Lifecycle interface {
    Init(ctx context.Context) error               // fail-fast 环境检查
    Shutdown(ctx context.Context) error           // 资源释放
    Rollback(ctx context.Context, phase Phase) error // phase 级回滚
    Health(ctx context.Context) HealthStatus      // 健康报告
}
type Executor interface {
    Lifecycle
    Execute(ctx context.Context, p Phase, mode string) error
}
```

**对现有系统的影响**：中等。`DryRunExecutor` 需要实现 `Lifecycle`（全部 no-op），`CommandExecutor` 需要从"裸子进程管理"升级为完整的生命周期管理。但 `orchestrator.Engine` 中的调用路径（`runAgentPhase`）可以保持现有签名，只在 Engine 启动/停止时额外调用 Lifecycle。

### 方向 C：双解析器语义漂移 → golden-file 回归测试 + 规范参考解析器（P1）

**为什么需要**：验证文档方向四已证明双 YAML 解析器的静默语义漂移是真实风险。Sprint 27 的 block-scalar bug（description 字段被注入 `"> "` 前缀）曾真实地在 6/7 个真实文件中跑偏而测试全绿。当前只有"差分测试"（Go vs Python 输出对比），但差分测试本身可能失效（上次就是由于使用了 `t.Logf` 而非 `t.Errorf`）。需要的不是修补当前差分测试，而是建立一个**规范参考解析器**。

**核心挑战**：
1. **规范参考的选择**——Go yaml2json vs Python PyYAML vs 手动实现 `internal/yaml2json`（Sprint 27 重写过）。三者中哪一个是"正确"的？需要选择一个作为规范参考，其他与之对齐。
2. **regression 测试的粒度**——需要不是"整体 YAML 文件通过"的端到端测试，而是**针对每个 YAML 构造类型**（block scalar、flow sequence、anchor/alias、tag 等）的独立测试用例。上次 block-scalar bug 就是因为在整体对比中被平均掉了。
3. **golden-file 的维护成本**——每次 YAML 构造变更（如支持新字段）都需要更新 golden file。需要自动化工具一次生成全部。

**预期的架构变更**：
```
forge-core/internal/yaml2json/
  testdata/
    golden/           ← 规范输出（golden file）
      mappings.json
      sequences.json
      scalars.json
      block-scalars.json
      anchors.json
      aliases.json
      tags.json
    fixtures/         ← 输入 YAML
  validate_test.go    ← golden-file 回归测试（真断言，非 t.Logf）
```

**对现有系统的影响**：低。纯测试新增，不动生产代码。Sprint 27 的 `normalize.go` 重写已经修复了 block-scalar bug，golden-file 测试是加固。

### 方向 D：Gate 增量缓存 → 墙钟优化引擎（P2）

**为什么需要**：验证文档方向一确认了 `RunFrom` 的 loop-back 无条件重跑所有 gate。这在当前阶段（gate 执行 ~300ms）不是问题，但随着 gate 数量增加（当前 8 项架构检查 + secret-scan + test，未来可能增加 mutation/coverage/SCA——north-star 中声明了更多 Harness Workers）和 gate 复杂度提升（SCA 的 OSV 解析、mutation 测试运行），每次 loop-back 重跑全部 gate 的墙钟成本会线性增长。

**核心挑战**：
1. **缓存失效的粒度**——"哪个 gate 的哪个输入变了" vs "重跑全部"。细粒度缓存（per-gate per-input）的失效逻辑复杂度远超收益；粗粒度缓存（per-iteration 快照）实现简单但命中率低。
2. **确定性 gate 的判定**——哪些 gate 是纯确定性（相同输入→相同输出）。`gate.mjs`（体积检查）和 `arch-check.mjs`（结构分析）是确定性的；`test` 不是（时间戳/随机性/外部状态）。
3. **缓存存储的位置**——内存（Engine 生命周期内）vs 磁盘（跨进程持久化）。当前 `CommandExecutor` 是子进程，所以缓存需要在进程间共享或放在磁盘。

**设计选项对比**：

| 方案 | 粒度 | 复杂度 | 收益 | 风险 |
|---|---|---|---|---|
| per-phase gate 快照 | 全 gate 集 | 低 | 中等（一次 loop-back 省一次全量） | 缓存和实际结果可能不同 |
| per-gate content-hash | 单个 gate | 高 | 高 | 失效逻辑复杂，可能漏缓存 |
| 不做，先等墙钟证实痛点 | N/A | 0 | 0 | 可能错过设计窗口 |

**推荐**：选择"per-phase gate 快照"方案。在 `Engine` 内维护一个 `map[PhaseIndex][]GateResult` 作为内存缓存，loop-back 到之前跑过的 phase 时直接返回缓存结果。这不完美（同一个 phase 在不同 iteration 输入可能不同），但实现简单、收益确定、风险低。当墙钟确实成为瓶颈时，再升级为 content-hash 方案。

**架构变更**：
```go
// internal/orchestrator/cache.go
type gateCache struct {
    mu    sync.RWMutex
    cache map[string][]gate.Result // key: phaseIndex-iteration
}
```

### 方向 E：`forge validate --consumed-fields` → 声明-实现一致性自动化（P3）

**为什么需要**：验证文档方向五的事实错误（3/4 字段已被消费但文档声称"未被消费"）暴露了一个元架构问题——没有一个命令能回答"当前 forge-core 版本消费了 workflow YAML 中的哪些字段"。Sprint 30→31 的"4 处仅文档标注→真实现"的复审纠正表明，即使是同一团队的实现者和审查者也可能对消费状态产生分歧。需要自动化检测。

**核心挑战**：
1. **静态分析的精度**——Go struct field → JSON tag → 实际消费代码的路径追踪。`asset.Phase.SecondaryTemplate` 在 `prompt_artifacts.go` 中被读取，`asset.Phase.Readonly` 在 `engine_build.go` 中被读取——这些消费点与 `asset.go` 中的字段定义分布在不同的包中。静态分析需要跨包追踪。
2. **注解格式**——需要一种标准化的注解格式来标记"消费点"，而不是依赖 README 注释或 `ADDED HERE ONLY` 约定。
3. **避免过度设计**——这应该是一个 hygiene 检查，而不是完整的依赖分析器。Pareto 原则：用 20% 的精度覆盖 80% 的消费关系。

**架构变更**：
```
forge validate --consumed-fields     # 输出每个字段的消费状态
forge validate --workflow-schema     # 验证 workflow YAML 与当前版本的兼容性
```

**对现有系统的影响**：低。纯工具链新增，不影响运行时。可以作为 `harness/check.py` 的补充检查。

---

## 3. 接口设计原则

### 3.1 契约第一（Contract-First）

当前架构中，agent 输出契约、executor 生命周期、converge 信号——这些关键的跨模块边界都是**隐式契约**。隐式契约的问题是：它们只在代码被阅读时才存在，无法被工具验证，无法版本化，无法协商。

**建议**：所有跨模块边界（engine↔executor、orchestrator↔agent、harness↔gate）都应该以`interface`或`struct`的形式在独立的 `contract/` 包中声明。这不是为了 Java 式的过度工程——而是因为**在 AI 编排系统中，契约是唯一可信的协调机制**。AI 输出不可预测，闸门不可 bypass，契约不可模糊。

**具体建议**：
- 将 `cost.go` 中的三个解析器抽取为 `internal/contract` 下的命名契约
- 将 `converge.Signals` 定义为收敛契约（已有，但需要文档化其加权聚合逻辑）
- 为每个 agent 角色定义输出契约（reviewer-verdict、executive-verdict、confidence-score）

### 3.2 显式版本化

当前没有一个模块版本化。这在单库 monolith 阶段是可接受的，但向 microservice（north-star）演进时，**每个服务交付的契约版本必须明确**。

**建议**：
- 每个 `internal/` 包顶层加 `const Version = "v1"` 和 `SupportedVersions` 声明
- `forge validate --interfaces` 检查所有内部接口的版本一致性
- 不要在 build 时引入 semver 解析库（违反零外部依赖），用 `string` 比较 + `major.minor` 简化版本号

### 3.3 注册表而非硬编码

当前架构中有多处 "if claude" / "if command executor" 的分支逻辑：

```go
// cost.go:classifyClaudeOverload — 先尝试 JSON envelope，回退到文本
// engine_build.go — 根据 agent 类型构建不同的 argv
// prompt_context.go — 不同的 provider 有不同的 prompt 拼装
```

随着供应商增加（Codex、Gemini CLI、OpenHands），这种 if-else 模式会线性增长。

**建议**：引入注册表模式——`executor.Register("claude", factory)`、`contract.Register("reviewer-verdict", parser)`、`prompt.Register("claude", builder)`。这不需要框架，只需要一个 `sync.Map` + `init()` 函数。

### 3.4 向后兼容原则

当前的"纯 Go 标准库，零外部依赖"红线在 v2 阶段是正确的。但向 v3 演进时，这个约束需要**有选择地放松**。建议采用以下分层兼容策略：

| 层 | 依赖规则 | 适用范围 |
|---|---|---|
| `forge-core/internal/*` | 零外部依赖 | 核心编排逻辑、路由决策、契约 |
| `forge-core/cmd/forge` | 零外部依赖 | CLI 胶水层 |
| `harness/` | Node.js 生态 | 闸门（已在使用 node:test） |
| 未来: 外部集成 | 允许有限依赖 | LiteLLM、Qdrant、Temporal 的轻量客户端 |

关键原则：**核心编排逻辑绝不依赖外部库**。外部集成通过接口适配器接入，适配器位于独立的 `forge-integrations/` 路径。

---

## 4. 技术选型

### 4.1 "零外部依赖"的决策评估

**当前状态**：`go.mod` 无 `require` 行。这是极端的自研最小主义。

**评估**：正确。在 v2 阶段（编排味道验证期），零外部依赖降低了风险：
- 无传递依赖的 license 合规问题
- 无上游 breaking change 的中断
- 无 CVE 追踪负担（Sprint 19 的 SCA 框架 ready，但无 SQLite/Qdrant 库需要监控）
- CI 不依赖外部模块代理

**何时需要放松**：当以下条件全部满足时：
1. YAML 解析：当前 Python shim 桥接已成为错误来源（方向四已验证）→ 引入 go-yaml v3
2. 嵌入式数据库：持久化需求超出 JSON 文件 → 引入 SQLite（通过 CGo 或 pure-Go 实现）
3. HTTP 客户端：外部 API 集成 > 3 个（当前仅有 CLI 子进程）

**go-yaml v3 的引入时间**：建议在 Sprint 32 或下一个"治理深化"阶段。当前 Python shim 虽然可靠（Sprint 27 的 block-scalar bug 已修），但双解析器的维护成本和漂移风险已超过 go-yaml 的 1 个外部依赖成本。具体的架构判决点：**当第五个 YAML 构造类型触发错误时**。

### 4.2 Temporal 的引入时机

**north-star 状态**：Orchestrator 使用 Temporal（Go）作为 durable workflow 引擎。

**当前状态**：Orchestrator 是纯 Go 内存状态机。Sprint 14 的 `LoopEngine`、Sprint 13 的 loop-back 状态机、Sprint 31 的 `on_rejected` 持久标记——所有这些"持久化信号"目前都是**文件标记**（`.forge/*.approved` / `.forge/*.rejected`）。

**评估**：过早引入 Temporal 将是架构过度。当前的 in-process 编排器（`LoopEngine` + `RunFrom` + 文件标记）足够支撑 v2 的全部需求。Temporal 的引入应该在以下信号触发时：

1. **跨机器编排需求**——编排器需要多个物理节点协调（当前单进程）
2. **人审等待超过 1 小时**——当前 `durable_wait` 标注为 v2/v3，当人审成为瓶颈
3. **编排器重启丢失状态成为问题**——当前 checkpoint/resume 在 Sprint 5 已实现

### 4.3 Firecracker Sandbox 的架构决策

north-star 的 Firecracker microVM 是目前最远的架构目标。但在引入之前，有一个重要的**中间架构形态**需要设计：

**中间方案：Docker 容器化 executor**

在 Firecracker 之前，Docker container 作为 executor 的隔离边界，可以解决当前最紧迫的架构限制：
- `CommandExecutor` 在宿主机上直接 fork 子进程，没有文件系统隔离
- 无网络隔离（agent 可以访问宿主机的全部网络）
- 无资源限制（CPU/memory 无 cgroup 保护）

Docker executor 可以复用当前的 `AgentExecutor` 接口，只需要一个新的实现：

```go
type DockerExecutor struct {
    image    string    // 预构建的 agent 宿主镜像
    mounts   []Mount   // 工作区挂载
    resource  ResourceSpec // CPU/memory/disk 上限
    network  NetworkPolicy // egress 白名单
}
```

**引入时机**：当第一次发生 agent 逃逸事件或资源滥用事件时。在此之前，`CommandExecutor` 的可审计性（Sprint 22 的 output-size cap + Sprint 20 的 recursion guard + Sprint 21 的 budget guard）提供了足够的安全基线。

### 4.4 技术选型决策框架

基于当前项目和 31 个 Sprint 的经验，建议以下技术选型的决策流程：

```
1. 这是核心编排逻辑吗？                    → 是 → 自研（零外部依赖）
                                           → 否 → 2
2. 这是一个外部集成适配器（LiteLLM/Qdrant）？ → 是 → 采购引擎，自研薄适配器
                                           → 否 → 3
3. 这是基础设施/部署（CI/SCM/Observability）？→ 是 → 采购
                                           → 否 → 4
4. 这是一个广泛使用、稳定、小接口的库？
   例：go-yaml、mattn/go-sqlite3           → 是 → 允许，但放在独立的 internal/ 包中隔离
                                           → 否 → 自研
```

---

## 5. 实施路线图

### 优先级排序

| 优先级 | 方向 | 当前状态 | 架构影响 | 预估工作量 |
|--------|------|---------|---------|-----------|
| **P0** | 方向三：Agent 输出契约 → 契约注册表 | 验证通过，风险真实 | 中（抽取 + 注册表） | 0.5-1 sprint |
| **P0** | 方向四：双解析器 → golden-file 回归 | 验证通过，风险真实 | 低（纯测试） | 0.25 sprint |
| **P1** | 方向 B：Executor 生命周期 | 验证通过，非阻塞 | 中（接口变更 + 适配） | 1 sprint |
| **P1** | 方向 A：契约注册表 → 版本化 | 方向三的自然延伸 | 中（contract 包） | 1 sprint |
| **P2** | 方向 D：Gate 增量缓存 | 墙钟优化，非安全 | 低（内存缓存） | 0.5 sprint |
| **P2** | 引入 go-yaml v3 替代 Python shim | 双解析器问题的根本解 | 中（依赖变更） | 1 sprint |
| **P2** | 方向 E：`forge validate --consumed-fields` | 治理 hygiene | 低（工具链） | 0.5 sprint |
| **P3** | Docker 容器化 executor | 安全隔离增强 | 高（新 executor 实现） | 2 sprints |
| **P3** | `forge scan` → 架构知识图谱 | 元架构知识管理 | 高（全仓分析） | 2-3 sprints |

### 阶段划分

#### 阶段 1：契约加固（Sprint N — N+1）

目标：解决验证文档中确认的最高风险（方向三 + 方向四）。

**交付物**：
- `internal/contract/` 包，内含 `verdict`、`confidence`、`cost` 三个内置契约
- 契约注册表 + 版本协商支持
- golden-file 回归测试覆盖全部 YAML 构造类型（block scalar、flow sequence、anchor/alias 等）
- 差分测试从 `t.Logf` 升级为真断言

**验证标准**：
- `forge validate --contracts` 输出全部已注册契约及其版本
- 所有 7 个真实 workflow YAML 在 Go 和 Python 解析器下 byte-identical
- Sprint 27 的 block-scalar bug 加入 regression test

**关键风险**：
- 契约注册表的设计可能过度工程（需要 Review 闸门把关）
- golden-file 在每次 YAML 支持变更时需要更新

#### 阶段 2：运行时生命周期（Sprint N+1 — N+3）

目标：弥补 Executor 生命周期缺口 + 引入 go-yaml v3 替代 Python shim。

**交付物**：
- `AgentExecutor` 接口增加 `Lifecycle`（Init/Shutdown/Rollback/Health）
- `DryRunExecutor` 实现 Lifecycle（全部 no-op）
- `CommandExecutor` 实现 Lifecycle（环境检查 Init、子进程清理 Shutdown）
- go-yaml v3 引入 `forge-core/internal/yaml2json` 作为主要解析器
- Python shim 保留为 fallback + golden-file 参考

**验证标准**：
- `forge run --executor command` 在 Init 失败时 fail-fast（而非在 Execute 时）
- Engine.Shutdown 确实调用所有 executor 的 Shutdown
- Go YAML 解析器与 Python shim 在所有已知输入上 byte-identical

**关键风险**：
- go-yaml v3 的引入打破"零外部依赖"红线——需要架构决策层通过
- `CommandExecutor` 的 Init 环境检查可能过于严格（如 claude CLI 存在但未认证 → fail？）

#### 阶段 3：性能优化 + 治理工具（Sprint N+2 — N+4）

目标：解决墙钟优化 + 声明-实现一致性自动化。

**交付物**：
- per-phase gate 快照缓存（内存）
- `forge validate --consumed-fields` 子命令
- `asset.go` 的 ADDED HERE ONLY 注释更新

**验证标准**：
- loop-back 到已跑 phase 时 gate 返回时间 < 1ms（而非重新执行）
- `forge validate --consumed-fields` 正确报告 `SecondaryTemplate`/`Readonly`/`RequiresTools` 已被消费

#### 阶段 4：安全隔离（Sprint N+3 — N+5）

目标：Docker 容器化 executor（Firecracker 的前置步骤）。

**交付物**：
- `DockerExecutor` 实现（复用 CommandExecutor 之子进程通信模式）
- 工作区挂载 + 网络策略 + 资源限制
- `forge run --executor docker` 端到端坐实

**验证标准**：
- Docker executor 跑通最小 phase（与 CommandExecutor 相同输入→相同输出）
- 资源限制生效（CPU quota、memory limit）
- 网络限制生效（egress 白名单拒绝未授权访问）

### 风险矩阵

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| 契约注册表过度工程 | 中 | 低 | Review 闸门强制"最小可行"原则；拒绝任何"可能有用"的抽象 |
| go-yaml v3 引入破坏零依赖红线 | 高 | 中 | 先做架构决策审查；如拒绝，保留 Python shim + 增强 golden-file 测试 |
| Docker executor 的 CVE 面 | 中 | 高 | 仅使用官方镜像；最小权限容器；无 host network |
| 方向三修复过程中改变现有行为 | 低 | 高 | 逐个契约迁移，每步跑通现有测试套件 |
| 墙钟优化收益不足以证明投入 | 高 | 低 | 先测量再优化——如果 loop-back 墙钟 < 1s，暂缓缓存实现 |

---

## 总结

验证文档的五个方向交叉验证整体质量高（方向一至四论证链完整，方向五的事实错误是方法论的警示）。核心结论：

1. **方向三（P1🥇）和方向四（P1🥇）风险真实且已验证**——契约隐式性和双解析器漂移是当前架构中两个最危险的隐式错误来源。建议在下一 sprint 立即修复。

2. **方向五的事实错误是一个有价值的"元发现"**——它揭示了 ForgeOS 缺少"声明 vs 实现"的自动化一致性检查。sprint 30 的 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 是弥补这一缺口的第一步，但需要工具化（`forge validate --consumed-fields`）。

3. **零外部依赖红线的代价已显现**——Python shim YAML 桥接是最大的架构债务之一。这不是 Python 的问题（Python 是优秀的语言），而是**跨语言桥接的语义映射**引入了无法静态验证的"灰色地带"。block-scalar bug 是前车之鉴。

4. **ForgeOS 的增量交付纪律是真正的架构护城河**——31 个 Sprint 中，"先拆分再继续"被反复执行，`cmd/forge` 包的三次架构重构、Sprint 30→31 的 4 处纠正、Sprint 29 的系统信号审计——这些不是偶然，是工程纪律的产物。任何扩展方向都不能损害这个纪律。

最终建议：**先修 P1 方向（三+四），再做 P2 的 Executor 生命周期和 go-yaml 引入，墙钟优化推迟到测量后决策。** 在每一个变更上，保持 ForgeOS 自己的闸门标准：单人审自己的代码 = 无效审，fresh-context reviewer = 唯一可接受的审查方式。
