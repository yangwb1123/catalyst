现在我已掌握完整背景。以下是我的架构分析。

---

# 架构评估报告：ForgeOS 扩展方向验证

> **角色**：资深架构师  
> **输入**：代码级验证报告（v52 方向审计）+ 现有架构文档（BOOTSTRAP、ARCHITECTURE、ROADMAP、CURRENT_SPRINT、north-star）+ ~343 份历史需求文档交叉引用  
> **范围**：体系评估 · 扩展方向 · 接口设计 · 技术选型 · 实施路线图

---

## 一、架构评估

### 1.1 当前架构的优势

**设计纪律是真正的护城河。** ForgeOS 在处理一个极易腐化的领域时，表现出了与其规模不匹配的架构成熟度：

- **层叠式执法**：`gate.mjs`（编辑时即时信号）→ `check.py`（治理完整性）→ `acceptance.mjs`（完整 Stop 闸门，聚合 8 检查），三者形成信任链而非单点。每一层诚实标注 N/A、不伪造通过——这在 AI-native 工具链中极罕见。
- **回路完整性**：Sprint 24–26 用真实 Claude 跑通了 multi-agent 端到端闭环，暴露并修复了 8 个真实 gap（任务注入、写权限、模型路由、工作目录、成本封顶、trace-latency、cost-telemetry、reviewer-缺-gate-信号）。这不是概念验证，是承受了真实资金成本的实证。
- **超时/预算/递归/输出四维护栏**：`Timeout` + `MaxAgentCalls`（总量上限）+ `MaxAgentDepth`（递归深度）+ `cappedBuffer`（输出大小）构成了 AI 执行的安全网——这是生产级系统的必备设施，很多商业产品在 v2 阶段尚未做到。
- **零依赖承诺的真正履约**：`go.mod` 无 `require` 这一事实，意味着 18 个 Go 包、~32k LOC 的生产代码不依赖任何第三方库。这对审计合规和离线部署有深远意义。
- **中枢旋钮的 7 个维度完整性**：一个 `mode × lifecycle` 设置驱动 Router 档位 + Harness gate-set/严格度 + Workflow 深度（discover/design/adr/reviewer/evolve）+ migration。从「声明式配置」到「运行时行为」的全链路一致，这在 k8s 类系统中都要花多个版本才能收敛。

### 1.2 当前架构的局限性

以下限制并非债务，而是有意识的设计取舍（`north-star.md` 已标注目标态与现状的差距）：

| 限制 | 本质 | 影响 |
|------|------|------|
| **单进程、单目录假设** | `root` flag + `asset.Load` 单一路径 + `persist/checkpoint` 单目录 | 无法表达 monorepo 多模块、无法跨进程协同。方向五 monorepo 和方向四舰队治理都撞此墙 |
| **编排逻辑驻留 CLI 层** | 17 个子命令的逻辑大量在 `cmd/forge/` 中，而非 `internal/` 包 | SDK 化困难；`internal/` 包有纯函数形态但无稳定 API surface；`cmd/forge` 包文件数反复逼近上限（已抬到 17） |
| **日志系统为 `Log func(string)` 签名** | 无级别/无结构/无上下文传播 | 调试复杂 evolve 循环只能靠人读全部行；无法做机器筛选；CI 解析困难 |
| **trace 只写不查** | `trace.go` 只有 Emit，没有 Query/Replay/Aggregate | 跨运行趋势分析、故障复盘、审计回放全部需要手动 jq；方向三和五的调试器价值被基础设施限制 |
| **YAML shim 是临时脚手架** | `harness/yaml2json.py` 作为桥梁 | Go 零依赖承诺在此产生摩擦；Sprint 27 发现 block-scalar 损坏且差分测试失效——虽已修，但中间件本身是薄弱环节 |
| **收敛语义验证缺位** | `converge.Signals` 的 8 个信号中无「行为/语义匹配」 | RoadmapCompletion 是自报、GatesGreen 是语法、FileDelta 是文件名启发式——三者均不回答「实现真的符合需求吗？」这一根本问题 |
| **人类审批是二元闸门而非异步工作流** | `human_gate` 通过 `converge.Converge` 实现——批准/不批准，无条件批准、带反馈重做、拆分建议等 | 方向一（半自治 co-pilot）中用户设想的 4 级自治框架与现有的二元审批之间存在根本性的交互模型差异 |

### 1.3 架构债务 vs 设计取舍

根据 `AGENTS.md` 的纪律和 Sprint 记录，我区分以下两类：

**真正的结构性债务（需偿付）**：
1. **`cmd/forge` 包文件数持续告警**：Sprint 27 抬到 17、Sprint 30 抬到 18、Sprint 31 回调到 16。每次超过阈值后再回调的「先拆后抽」模式表明：`cmd/forge` 承担了过多 CLI 编排 + 领域逻辑的双重职责。根治方案是方向二（SDK 提取）或至少将 `internal/gate/resolve.go` 的拆出模式推广到 `cost.go`、`prompt_context.go`、`evolve.go`。
2. **`Log func(string)` 的设计**：Sprint 31 的 31 轮迭代中，没有一轮系统性地处理日志。不是因为它不重要，而是因为它不阻塞功能交付——但作为调试复杂 async 工作流的唯一窗口，它的缺失是运维杠杆缺失。
3. **YAML shim 的隐性依赖**：`python3 harness/yaml2json.py` 不是 Go 依赖，但它是运行时依赖。`forge-init` 复制的项目中如果没有 Python 3，`forge run` 会静默失败（因为 `exec.Command` 的 error 处理在 shim 路径上）。

**经过论证的设计取舍（不应与债务混为一谈）**：
- 单目录假设在 v2 内合理——`north-star.md` 明确将跨仓库治理标记为 v3/v4
- `trace` 只写不查——Sprint 25–26 的重点是数据采集的生产化而非消费；方向五和方向三的 trace 消费确实是正确的前沿
- 无 semantic convergence——是 `honest-but-trusting` 模式的诚实边界，方向二（语义收敛验证）虽然高价值，但不应在 `risk.FromChangedPaths` 的廉价启发式上构建伪语义保证

---

## 二、扩展方向

基于验证报告的真实差异化分析和当前架构局限性，我识别以下 **4 个高价值架构扩展方向**（而非 5 个——在真实的架构评估中，4 个比凑 5 个更有说服力）：

### 方向 A：运行时多项目舰队控制平面（取代被证伪的方向 1 和 3）

**为什么需要**：验证报告指出方向 1（半自治 co-pilot）的"0 篇覆盖"声明被证伪——现有方向 4（异步协作人审界面）已覆盖其核心功能。方向 3（agent 漂移检测）被 2 个现有方向覆盖。真正的缺口在**跨项目治理**和**可信执行环境**的交汇处。

方向 A 整合了两个真正未覆盖的方向（本地 LLM 离线部署 + 治理审计追踪）和一个已验证的缺口（跨项目策略一致性），形成一个统一的前沿：

**核心挑战**：
1. **`TierFor` 现在纯字符串映射 `(agent, mode) → tier`**——多项目场景需要一个组织级策略引擎，在项目级决策之上叠加全局约束（production 项目强制 `router_floor: opus`、跨项目预算池）
2. **本地 LLM executor 的上下文窗口适配**：`prompt_context.go` 当前无 `MaxTokens` 敏感度——本地模型（Llama/Gemma）的上下文窗口与 cloud API 差异巨大，需要基于 model profile 做 token 预算裁剪
3. **`policies.sum` 锁文件的根信任问题**：方向 4 已识别但未解决的中心难题——锁文件自身在 evolve 中被修改谁来锁？需要引入一个新的信任根（硬件 TPM 或 `--policy-signing-key` 外部参数）

**预期架构变更**：
- 新包 `internal/fleet/`（多项目注册 + 策略传播 + 预算聚合）
- `internal/mode/mode.go` 新增 `Effective(mode, lifecycle, orgPolicy)` 签名重载
- `internal/routing/routing.go` 新增 `ModelProfile` 类型（`MaxTokens`/`CostPerToken`/`Provider` 等），`TierFor` 重载为 `TierFor(agent, mode, profile)`
- `internal/persist/checkpoint.go` 新增 `SignedMutation`（策略变更的签名验证）
- 新包 `internal/localexec/`（`LocalModelExecutor` 实现 `AgentExecutor`）

**对现有系统的影响**：中等。`AgentExecutor` 接口（`executor.go`）是天然扩展点，`LocalModelExecutor` 是新增实现而非修改。`TierFor` 的签名重载可通过 overload 保持向后兼容。关键风险在 `mode.Effective` 的签名变化——所有调用点需审计。

### 方向 B：结构化可观测基础设施（替代方向 5 中被证伪的"全新调试器"声称）

**为什么需要**：验证报告证实方向 5（trace 可视化调试器）在 2 个现有文档中有实质覆盖——五产品架构扩展方向.md 的"自治运行可观测性"和 2026-07-11-five-product-architectural-expansion-directions-scanned.md 的"可重放调试引擎"。这些方向已经提出了 `forge trace ls|show|replay|query|diff`、时间线可视化、甘特图、运行比较。但**没有一个被实现**。方向 B 不是"全新"而是"将累积的架构共识兑现为代码"。

**核心挑战**：
1. **`trace.Event` 当前是扁平 `Op string`**——无法表达决策链。`routing.TierFor` → `BudgetAdjustTier` → `safetyFloor` 的每一跳都需要 trace。需要 `DecisionChain []DecisionStep` 字段
2. **trace 存储升级**：纯 JSONL 在单个 evolve 可能产生数十万 event 时查询性能不可接受。选项：嵌入 SQLite（CGo-free driver，破坏零依赖承诺？）vs. 按 iteration 分片 JSONL + 构建文件内倒排索引
3. **回放保真度**：重放需使用当时的 forge 版本代码，否则 evaluator 语义漂移。这意味着 trace 必须记录 forge 版本 hash，可能还需记录 evaluator 函数引用

**技术选型决策**：

| 选项 | 优点 | 缺点 | 建议 |
|------|------|------|------|
| **SQLite 嵌入**（`modernc.org/sqlite` 纯 Go） | 结构化查询、聚合、排序 | 破坏零依赖承诺；~5MB 二进制膨胀 | **推荐 v3**，v2 用分片 JSONL + 索引 |
| **分片 JSONL + 索引文件** | 保持零依赖；增量写入 | 查询能力弱；跨分片聚合需全扫描 | **推荐 v2**，与当前 `trace.go` 的 append-only 模式一致 |
| **`trace/query.go` 独立文件索引** | 加载时建内存索引；中等查询能力 | 大 trace 建索引慢；内存消耗 | 作为 v2 的折中方案 |

**建议**：v2 用分片 JSONL（每 10k event 一个文件） + `query.go` 加载时建倒排索引。零依赖承诺保持到 v3，届时引入 SQLite。

**对现有系统的影响**：小。`trace.Event` 加 `DecisionChain` 字段是向后兼容的添加（现有 event 反序列化后为 nil）。`internal/replay` 是新包，不修改既有运行时路径。

### 方向 C：产物完整性治理平面（将数个"声明但未接线"的缺口收敛为统一机制）

**为什么需要**：验证报告和现有文档反复触及同一个根本问题——ForgeOS 对 agent 产物的信任是盲信的：`emits:` 声明了文件但无人验证存在性；`VERDICT:` 契约被硬编码解析但无版本协商；`CONFIDENCE:` 被 agent 自报但无交叉验证。方向 C 不是解决这些问题中的某一个，而是**建设一个统一的产物治理平面**。

这个方向整合了：
- 方向 3 中验证报告认可的核心差异化价值（运行时行为漂移的趋势检测）
- 方向 5 中重构建议的"验证 agent 产出而非仅验证语法正确性"
- 方向 4 的「非代码产物质量门」提案
- 现有文档中碎片化的 `emits` 验证需求

**核心架构变更**：
1. **产物契约注册表**（`internal/contract/`）：将机读契约从 `cost.go` 的硬编码字符串提取为中心化注册表
   ```go
   type Contract struct {
       Token    string     // "VERDICT: APPROVE"
       Since    string     // "v1"
       Parser   func(string) (any, bool)
       Schema   string     // 可选 JSON Schema
   }
   var Registry = map[string]Contract{...}
   ```
2. **运行时产物验证钩子**（`internal/artifact/`）：在 `runAgentPhase` 完成后、进入下一 phase 前，对 `emits:` 中的每个文件名执行验证——存在性检查 + schema 匹配（如果注册了）
3. **趋势检测器**（`internal/trend/`）：跨运行分析合约合规率随时间变化（100%→80%→60%），当趋势越过阈值时触发 `forge doctor` 告警

**对现有系统的影响**：中等偏高。`cost.go` 的三个解析器需要从 switch-case 提取为注册表条目，这是重构而非重新实现。`runAgentPhase` 需要插入验证钩子——不影响既有 gate 逻辑，但与 `on_fail` 的耦合关系需要谨慎处理（产物验证失败是灰色区域：phase exit 0 但内容不满足契约）。

### 方向 D：语义收敛验证（架构层面回答"实现真的符合需求吗"）

**为什么需要**：这是验证报告和所有现有文档共同指向但从未解决的根本缺口。当前 converge 的 8 个信号都是语法/机械信号：RoadmapCompletion（自报）、GatesGreen（语法）、FileDelta（文件名启发式）。方向 D 是**行为级验证**的第一个版本。

**关键限制**：不要假装能做深层语义理解。方向 D 的 v1 必须是诚实的、范围受限的：
- 不做 NL 理解（不试图解析 PRD 描述与实现代码的语义）
- 做**跨产物断言检查**：PRD 说了"support OAuth login" → 检查 design doc 是否包含 OAuth 章节 → 检查 ADR 是否记录了 OAuth 决策 → 检查 test 是否覆盖了 OAuth 流程
- 做**影响范围推理**：git diff 对 `internal/auth/` 的改动 → 检查相关 test 是否被修改、ADRs 是否被更新、review doc 是否讨论了此变化
- 做**架构约束检查**：`arch-check.mjs` 的 8 检查扩展为可配置的多 package 规则集（payment 包不得 import 非允许列表中的包）

**架构变更**：
- `internal/contract/` 的从方向 C 引入后，方向 D 在其上建设断言引擎
- `converge.Signals` 新增 `ContractCompliance` 字段（0.0–1.0）
- `forge validate --semantic` 子命令，但诚实标注范围
- 新包 `internal/inference/` 或 `internal/traceable/`（专注于廉价启发式，如 `risk.FromChangedPaths` 同类）

**对现有系统的影响**：小。所有新增都是可选的——不影响现有 converge 判定逻辑。`ContractCompliance` 作为附加信号注入，不改变现有 8 信号的权重。

---

## 三、接口设计建议

### 3.1 关键模块的接口设计原则

**原则 1：所有跨包边界的数据交换必须通过契约接口，而非通过 `exec.Command` + stdout 解析**

当前状态：SDK 提取的愿景（方向二 in `expansion-five-architect-product-perspective.md`）与实际代码之间存在差距——大量逻辑在 `cmd/forge/` 中而非 `internal/` 中，外部项目只能 `exec.Command("forge", "route", "--diff-files")` 获取路由决策。这是 SPI（Service Provider Interface）的缺失，不是 API 的缺失。

**建议**：对方向 C 的产物契约和方向 D 的断言引擎，接口设计在第一天就面向库消费者，而非面向 CLI 命令：

```go
// 好：返回结构化结果
type ContractResult struct {
    PhaseName   string
    EmitFile    string
    ContractID  string
    Valid       bool
    Errors      []string
}
func VerifyEmit(phase asset.Phase, wfRoot string) []ContractResult

// 避免：返回 string 让调用者解析
func VerifyEmit(phase asset.Phase) string  // ❌
```

**原则 2：用 `Logger` 接口取代 `Log func(string)`**

当前签名 `Log func(string)` 是 Go 标准库中 `io.Writer` 级别的抽象——它已经是抽象了，但抽象程度不够。升级为：

```go
type Logger interface {
    Debug(msg string, keysAndValues ...any)
    Info(msg string, keysAndValues ...any)
    Warn(msg string, keysAndValues ...any)
    Error(msg string, keysAndValues ...any)
}
```

这为方向 B（结构化可观测性）提供基础设施，也服务方向 A（多项目舰队中日志聚合需要结构化字段）。

**原则 3：`trace.Event` 的扩展性问题**

当前 `Event` 结构体定义了固定字段集。当方向 B 需要 `DecisionChain`、方向 C 需要 `ContractResult`、方向 D 需要 `AssertionResult` 时，有三种策略：

| 策略 | 优缺点 | 推荐场景 |
|------|--------|----------|
| **固定字段 + optional**：Event 加所有可能的字段（大部分为 nil） | 简单；不用版本化 | ❌ 字段集膨胀不可控 |
| **`Detail json.RawMessage`**：Event 保留 Detail 字段，由消费者自行 decode | 灵活；字段集独立演进 | ✅ 推荐——`trace.go:57` 已有 `Detail string`，升级为 `json.RawMessage` |
| **Event 接口 + 具体实现**：`TraceEvent` 接口 + `PhaseEvent`/`GateEvent`/`DecisionEvent` | 类型安全；Go 风格 | ⚠️ 适合 SDK API，不适合序列化（JSON unmarshal 需要 type discriminator） |

**建议**：`Detail string` → `Detail json.RawMessage`，保持扁平 Event 结构不变。对 JSON 消费者最友好，对 Go 消费者通过 `WithDetail(T)` 泛型包装函数提供类型安全。

### 3.2 抽象层评估：是否需要新的抽象层？

**需要**：
1. **`Logger` 接口**：上文已论证。不是方向 B 的可选项，是基础设施的必要升级。
2. **`ModelProfile` 类型**：方向 A 的本地 LLM 执行需要。当前模型选择隐含在 `TierFor` 的字符串输出中。方向 A 需要一个显式的 `ModelProfile` 类型携带 `MaxTokens`/`CostPerToken`/`Provider`/`Capabilities` 等信息，使 `prompt_context.go` 的 token 预算裁剪成为可能。
3. **`ContractVerifier` 接口**：方向 C 需要。`Verify(ctx, emitPath, contractDef) → Result`。

**不需要**：
1. **`Engine` 接口 vs 当前 struct**：Sprint 24–26 的端到端验证表明 `Engine` 结构体作为编排核心工作良好。抽象为接口在当前阶段弊大于利——会为尚未出现的第二个实现（如 Temporal 化）增加间接层。
2. **`Event` 接口**：如上分析，固定结构体 + `Detail json.RawMessage` 的混合策略更优。

### 3.3 向后兼容性策略

| 变更 | 兼容性策略 | 迁移期 |
|------|-----------|--------|
| `Log func(string)` → `Logger` 接口 | 保留 `Log` 字段作为 deprecated alias；`Logger` 默认实现回退到 `Log` | 2 sprints |
| `trace.Event.Detail` string → `json.RawMessage` | 新 reader 优先尝试 Unmarshal Detail；退化则当做原始字符串 | 读时兼容，写时在新代码改用结构化 Detail |
| `TierFor(agent, mode)` → `TierFor(agent, mode, profile)` | 保留旧签名（调用 profile nil 则退回到当前行为） | 永久（nil 安全） |
| `mode.Effective(mode, lifecycle)` → `Effective(mode, lifecycle, orgPolicy)` | 同上 | 永久 |
| 新包引入 | 新 package 目录，不修改现有 import 路径 | 无迁移成本 |

---

## 四、技术选型

### 4.1 本地 LLM 执行（方向 A 组件）

**选项分析**：

| 选项 | 优势 | 劣势 | 建议 |
|------|------|------|------|
| **`llama.cpp` + server mode** | 最成熟的开源 LLM serving；支持量化；Go client 通过 REST API | 外部进程依赖；GPU 显存管理需额外编排 | **推荐**——与 `CommandExecutor` 的模式完全一致 |
| **Ollama API** | 更简单的安装体验；自带 model management | 增加一层抽象；社区版无认证/多租户 | 可作为 llama.cpp 之上的补充选项 |
| **纯 Go 推理（如 `llama.go`）** | 零外部依赖承诺保持 | 性能和模型支持远不如 llama.cpp；工程量大 | ❌ 不适合生产 |
| **LiteLLM proxy** | 已列入路线图（v3 跨厂商池）| 面向 cloud API，不是本地推理 | ❌ 方向不匹配 |

**决策**：本地 LLM 执行器通过 `CommandExecutor` 的扩展点实现——`LocalModelExecutor` 包装 `llama.cpp` 的子进程管理，与 claude executor 共享 `AgentExecutor` 接口。零依赖承诺保持（`llama.cpp` 不是 Go 依赖，是运行时依赖）。

### 4.2 Trace 存储引擎（方向 B）

**选项分析**（与上文接口设计部分对应）：

| 选项 | 优势 | 劣势 | 建议 |
|------|------|------|------|
| **分片 JSONL + 索引文件** | 保持零依赖；append-only 写入性能好；增量 | 查询能力有限；跨分片聚合成本高 | **v2 推荐** |
| **SQLite（`modernc.org/sqlite` 纯 Go）** | 完整 SQL 查询；索引；聚合 | 破坏零依赖；二进制膨胀 ~5MB | **v3 推荐**——当查询成为瓶颈时 |
| **embedded NATS/Redis** | 原生 pub/sub；流式 | 太重；运维复杂度 | ❌ 不适合单进程场景 |

### 4.3 产物契约 Schema 格式（方向 C）

| 选项 | 优势 | 劣势 | 建议 |
|------|------|------|------|
| **CUE** | 类型检查 + 验证合一；Claude 生态熟悉 JSON | 学习曲线；Go 集成需 CUE 库 | ❌ 价值与学习成本不匹配 |
| **JSON Schema（`gojsonschema`）** | 广泛采用；可直接在 prompt 中注入 schema | 有外部依赖但可 self-contained | **推荐 v2**——与 trace JSONL 的格式一致 |
| **Go 结构体断言** | 零依赖；编译时类型安全 | schema 变更需重新编译 | **推荐 v1**——与方向 C 的注册表模式一致 |

**决策**：v1 用 Go 结构体断言（`contract.go` 中的 `verifyEmitMarkdown(headings []string) error`），v2 可选接入 JSON Schema。

### 4.4 自建 vs 采购

基于 `north-star.md` 的 buy vs build 表和方向 B/C 的技术选型：

| 组件 | 自建原因 | 反例 |
|------|---------|------|
| **Trace 查询引擎** | 自建（方向 B）| 领域特定（event kind 多、需要决策链重建）——通用 TSDB 不适合 |
| **合约注册表** | 自建（方向 C）| 契约格式（`VERDICT: APPROVE`）太简单，OPA Rego 会增加不必要的间接层 |
| **产物 schema 验证** | 采购 JSON Schema 标准，薄自研 wrapper | 不重新发明验证语义 |
| **本地 LLM 执行** | 包装 llama.cpp（采购推理引擎、自研编排）| 与 `AgentExecutor` 接口对齐 |
| **策略引擎（多项目）** | 自建薄层，不引入 OPA | 当前策略模型（YAML + 代码内联）足够表达项目级策略；组织级策略只需叠加层，不需要 Rego 的全能力 |

---

## 五、实施路线图

### 优先级总览

```
P0（本次路线图必须） ─── 方向 C（产物治理）：最短路径到最大信任提升
│                         方向 B（可观测 v2）：基础设施级依赖，解锁方向 A 和 D 的调试能力
P1（此路线图核心） ─── 方向 A（多项目 + 本地 LLM）：两个真正未被发现的方向，市场差异化
│                         方向 D（语义收敛 v1）：架构层面解决「实现 vs 需求」的根本缺口
P2（后续路线图）  ─── 方向 A 的 fleet 控制平面扩展
│                    方向 B 的 SQLite 升级（v3）
│                    方向 D 的合约学习自动化（v3）
```

### 阶段划分

**Phase 1：基础设施层（~2 sprints）**

| 工作项 | 方向 | 产出 | 依赖 |
|--------|------|------|------|
| `Logger` 接口 + `--log-level`/`--log-format` | B | 结构化日志基础设施 | 无 |
| `trace.Event.Detail` → `json.RawMessage` | B | 可扩展的 trace 格式 | 无 |
| 合约注册表 + `VerifyEmit` 存在性检查 | C | `internal/contract/` 包 | 无 |
| `forge validate --contracts` | C | CLI 入口 | 合约注册表 |

**Phase 2：产物治理与可观测（~2–3 sprints）**

| 工作项 | 方向 | 产出 | 依赖 |
|--------|------|------|------|
| `cost.go` 三个解析器迁移到注册表 | C | 统一的合约解析路径 | Phase 1 合约注册表 |
| 运行时产物验证钩子（`runAgentPhase` 后） | C | 自动 `emits` 存在性验证 + 契约格式检查 | Phase 1 |
| `forge trace query` + 分片 JSONL | B | trace 查询能力 | Phase 1 trace 格式升级 |
| `forge trace replay`（v1：dry-run 回放） | B | 基础的 trace 回放 | Phase 1 |
| `internal/trace/query.go` | B | 可编程的 trace 查询 API | Phase 1 |

**Phase 3：本地 LLM + 语义收敛（~3–4 sprints）**

| 工作项 | 方向 | 产出 | 依赖 |
|--------|------|------|------|
| `ModelProfile` 类型 + `TierFor` 签名扩展 | A | 模型属性感知的路由 | 无 |
| `LocalModelExecutor` | A | `llama.cpp` 子进程包装 | `ModelProfile` |
| `prompt_context.go` token 预算裁剪 | A | 按模型 MaxTokens 适配 prompt | `ModelProfile` |
| `ContractCompliance` 信号 | D | 收敛系统的语义维度 | Phase 2 合约验证 |
| `forge validate --semantic`（v1：跨产物断言）| D | 行为级验证入口 | Phase 2 |

**Phase 4：多项目舰队（~2–3 sprints）**

| 工作项 | 方向 | 产出 | 依赖 |
|--------|------|------|------|
| `internal/fleet/` 包 + 组织级策略模型 | A | 多项目基础 | Phase 3 `ModelProfile` |
| `forge org init/register/status` | A | CLI 入口 | `internal/fleet` |
| `project.yml` `extends:` 消费 | A | 策略继承链 | Phase 1 `Logger` 用于调试 |
| 趋势检测器 + `forge doctor --trends` | C | 跨运行合规趋势 | Phase 3 合约验证 |

### 风险点和缓解策略

| 风险 | 影响 | 概率 | 缓解 |
|------|------|------|------|
| **零依赖承诺被破坏**（如 SQLite 嵌入） | 架构原则违反 | 中 | Phase 2 明确使用分片 JSONL，SQLite 推迟到 v3 作为明确架构决策 |
| `runAgentPhase` 产物验证钩子与 `on_fail` 交互错误 | 编排逻辑 bug | 高 | 在 Phase 2 的设计中，产物验证钩子必须**不调用** `on_fail`（那是 gate 的职责），只记录 `trace.Event` 和 `ContractCompliance` 信号 |
| **本地 LLM 质量低于 Cloud API，用户误判系统能力** | 用户体验受损 | 中 | `LocalModelExecutor` 在轨迹中诚实标注 `provider: local` 和 `model: llama-3.1-8b`，`scorecard.go` 按 model+provider 分别聚合质量分，用户可跨模型对比 |
| **多项目策略继承中「安全下限被 override」** | 安全漏洞 | 低 | 遵循 `production` lifecycle 一票否决的既有模式：组织级 `production_floor` 在 `mode.Effective` 中做 max() 而非 override |
| **方向 D 的语义验证过度承诺** | 信誉损失 | 中 | 严格遵循 `risk.FromChangedPaths` 的诚实模式——v1 只做结构检查，不在文档或 prompt 中暗示能理解语义 |
| `cmd/forge` 包文件数在 Phase 2–3 中再次告警 | 工程债务积累 | 高 | Phase 1 中就启动 `cost.go`/`prompt_context.go` 的域逻辑向 `internal/` 迁移，作为 Phase 2 的前提条件 |

---

## 推荐行动序列（各角色视角）

**对于 CTO**：方向 C（产物治理）是信任基础设施——让自治系统的产出可审计、可验证、可趋势追踪。不做这个，自治运行永远是"相信它会做好"而非"看到它确实做好了"。

**对于产品经理**：方向 A（本地 LLM + 多项目舰队）是市场差异化——本地部署解锁企业销售（金融/政务/医疗的数据主权需求），多项目治理解锁组织级采用（从"开发者的个人工具"到"工程组织的 OS"）。

**对于技术负责人**：方向 B（可观测基础设施）是投资——Log 接口和 trace 查询是所有后续方向（A、C、D）的依赖。先做它。Phase 1 的 2 sprints 产生的 ROI 会贯穿后续所有阶段。

**诚实标注**：以上分析基于 343 份现有需求文档 + 代码级验证报告 + 31 轮 sprint 的演进记录。方向 D（语义收敛 v1）的能力边界在第一天就必须诚实——v1 只做跨产物断言（PRD→design→ADR→test 的存在性链），不做 NL 理解，不做功能正确性判定。任何声称能「验证实现是否符合需求」的 v1 产品，如果不是在说有限范围内的结构匹配，就是在过度承诺。
