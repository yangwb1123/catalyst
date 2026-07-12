现在我已充分掌握项目脉络。以下是我的完整架构分析。

---

# 架构分析报告：ForgeOS 五方向评估与扩展

## 一、架构评估

### 1.1 当前架构的优势

**分层清晰，有北极星指引。** v2 的 Go 运行时已经落地了 5 引擎（Orchestrator / Model-Router / Context-Engine / Memory-Engine / Evaluation-Engine），每层有明确的职责边界和文档化的「诚实边界」。代码库中随处可见形如 `// HONESTY:` / `// Trade-off (stated, not hidden):` 的注释——这不是偶然，而是刻意植入的**架构透明度契约**，让接手的 agent 理解设计取舍而非盲目修改。

**三大结构亮点：**

| 亮点 | 体现 | 架构价值 |
|------|------|---------|
| **中枢旋钮** (Central Knob) | `mode × lifecycle` 同时驱动 Router 档位、Harness 严格度、Workflow 深度 | 一个参数变更即可全局调整系统行为，避免配置散落各处 |
| **带外执法** (Out-of-band Enforcement) | `harness/` 是真相之源，CC hook 只是加速器 | 不绑定任何宿主，可插拔——是系统架构层面最关键的"载重墙" |
| **诚实标注体系** | 每处已知缺口都有文字标注，不假装全功能 | 降低技术债隐藏风险，使架构决策可追溯 |

### 1.2 当前架构的局限性

**从验证文档的结论反推，五个方向映射出三类架构债务：**

#### 类型 A：声明-实现缝隙（方向四已验证但需修正认知）

`routing` 包已经有 `ModelMap` / `ResolveModel` / `TierFor` / `Score` / `TierForScore` / `BudgetAdjustTier`——一个完备的厂商无关模型路由抽象层。但 `engine_build.go` 的 `claudeArgv` 仍然硬编码 `--model`，`opusFloorAgents` 假设 provider=claude。

**本质问题不是"缺少抽象"，而是抽象层建在内部包（`internal/routing`），集成点却在高层 CLI 胶水（`cmd/forge`），两者之间存在接线断点。** 这类似于六层 OSI 模型各层协议都定义了，但层间 SAP（服务访问点）没实现。

#### 类型 B：数据模型丰富但消费不足（方向一 + 方向五）

- `memory.Entry` 有 `Confidence`（0-1）、`Source`（provenance 标记）、`Supersedes`（显式撤回）三个字段，但只有 `[unverified]` / `[low-confidence]` 两个前缀消费者
- `boundMemory` 的排序策略是「recency floor + relevance」，完全不看 confidence
- `Compact` 按 `keepPerKind` 计数截断，不看价值

**这本质上是数据飞轮（data flywheel）的初始阶段：采集层已经建好，但消费/反馈环路未闭环。** Confidence 是 agent 自报告的天然不可靠信号，需要交叉验证机制才能成为可信的排序信号。

#### 类型 C：运行时韧性缺口（方向三）

`trace.Event` 没有 `PromptText` 字段，`checkpoint.go` 只存 `PhaseIndex`，不含 memory/prompt 快照。

**这是一个"假设 100% 可靠"的架构假设：系统假设 checkpoint/resume 场景只需要恢复执行位置，不需要恢复执行上下文。** 在单次 session 内这是合理的（memory 可以重读），但在跨 session 重放或事后审计场景中，没有 prompt 快照意味着"你知道 agent 在第几轮做了什么，但不知道是什么 prompt 让它做的"。

### 1.3 关键设计决策评审

| 决策 | 评价 | 建议 |
|------|------|------|
| Go 纯标准库零依赖 | ✅ 正确。减少供应链风险，编译产物单一二进制 | 保持，不引入外部依赖 |
| YAML 经 python shim 转码 | ⚠️ 临时方案合理，但已存在 30+ sprints | 应考虑 Go 原生 YAML 解析（属依赖决策，需 architect/cto 层级拍板） |
| Memory JSONL 追加写 | ✅ 正确。append-only 避免 overwrite 数据损坏 | 保持 |
| Checkpoint 原子写（temp+rename） | ✅ 正确。防止崩溃留下损坏文件 | 保持 |
| Fresh-context reviewer 不读前序输出 | ✅ 正确。保护独立性（AGENTS.md 红线） | 保持 |
| Gate 强制顺序执行 | ⚠️ 当前正确（避免竞态），但可预见 v3 需要并行 | gateLedger 已预埋 mutex（但未启用），架构前瞻性好 |
| Agent 阶段默认 dry-run | ✅ 正确。防误烧预算 | 保持 |

---

## 二、扩展方向

基于以上分析，我提出 5 个高价值架构扩展方向，与验证文档的五方向有所交叉但视角不同：

### 方向 A：Context 预算控制器（对标验证文档方向一，扩展为完整预算管理子系统）

**为什么需要：** 当前系统在拼接各 lane（task plan / ADRs / AGENTS.md / multi-phase outputs / memory / gate results）后没有任何 token 预算检查。在 Haiku（8K token window）上静默溢出的路径是真实且可预测的——不是理论风险。

**核心挑战：**
- Token 估算需要模型感知（不同模型 tokenizer 不同）
- 降级策略需一致性（不是随机丢弃，而是按优先级逐层降级）
- 需要处理多阶段 prompt 装配的依赖关系（某个 lane 可能是另一个 lane 的前提）

**预期架构变更：**

```
当前: buildPrompt() → 拼接各lane → 直接输出
未来: buildPrompt() → assembleLanes() → budgetController.Check(lanes, model) 
                                    → if over cap: downgrade(降级 lane/截断/提醒)
                                    → finalize()
```

**关键设计决策：** 预算控制器应该是一个**可插拔策略链**（类似 Go 的 `http.Handler` 链）而非单个硬编码函数。不同 mode 可用不同策略：explorer 可接受更激进的截断，engineering 应优先降级 memory lane 而非 gate results。

**对现有系统的影响：** 低。接口隔离在 `buildPrompt` 内部，contract 不变——输出仍然是 `string`。唯一变化是当预算超限时注入降级标记（如 `[memory truncated: 24→8 entries to fit window]`），保证诚实可见。

---

### 方向 B：语义验证管线（对标验证文档方向二，扩展为多层次验证体系）

**为什么需要：** 当前所有 gate 检查的是"代码的样子"而非"代码是否工作"——函数长度、循环依赖、文件行数等。没有 compile gate、没有测试覆盖 gate。验证文档已指出：新代码不编译但 `test` 门仍可通过（因为没有测试调用它）。

**核心挑战：**
- 语言多样性（Go / Python / JS / Rust）需要 polyglot adapter
- 编译检查必须被视为 load-bearing gate，而当前 adapter 框架下的 lint/coverage/SCA 都被标记为 N/A-ready（诚实降级）
- `build` gate 必须避免「CI 上才能 catch」的延迟反馈

**预期架构变更：**

```
当前: harness gates = [lint, test, build(不存在), complexity, arch, security]
未来: harness gates = [lint, test, compile(t新建), coverage(已有框架但常N/A), complexity, arch, security]
                         ↓                              ↓
                    load-bearing — 无编译不通过      load-bearing — tool exists but may N/A
```

**关键设计决策：** `compile` gate 应作为**独立 gate**（而非并入 `build`）加入 `required_gates` 列表，以便 mode-gating 可以独立控制它。工程 mode 下 `compile` 应为 `required`，explorer 下可为 `optional`。

**对现有系统的影响：** 中。新 gate 需要 `adapter` 框架的扩展（已有 `adapters/` 目录与 `probeLint` 模式可复用），但 `build` gate 需要新实现而不是复用现有工具。工作量约 200 行适配器 + 每个目标语言约 100 行探测逻辑。

---

### 方向 C：Deterministic Replay Bundle（对标验证文档方向三，但定位为审计基础设施而非运行时韧性）

**为什么需要：** 当前 `trace.Event` 无 `PromptText`，`checkpoint` 不含 memory/prompt 快照。这意味着事故调查时你只能看到"agent 做了什么"（trace event），看不到"是什么 prompt 让它这么做的"（trace cause）。这是审计盲区。

**核心挑战：**
- Prompt 体积大（一次 prompt 可能 5-10KB），全部存下来会增加 trace 体积
- 仅存最近 N 轮 vs 全部存——存储成本与审计完备性的权衡
- 需要明确的隐私边界（prompt 可能含敏感信息）

**预期架构变更：**

```
trace.Event{
    + PromptText string `json:"prompt_text,omitempty"`  // 默认不存，--full-trace 启用
    + MemorySnapshot []memory.Entry `json:"memory_snapshot,omitempty"`  // 注入时的 memory 快照
}
```

**关键设计决策：** 不应把 replay bundle 做成全量无差别记录。建议设计两级策略：
- **Level 1（默认）**：仅存 `PromptText` 的 SHA256 hash + `PromptSizeBytes`，用于检测 prompt 漂移而不暴露内容
- **Level 2（`--full-trace` 显式开启）**：存完整 prompt + memory 快照

**对现有系统的影响：** 低。`Event` 结构体已用 `omitempty` 模式，新字段加入不影响既有 trace 的向后兼容性。`saveCheckpoint` 追加 snapshot 字段——同样 `omitempty`，旧 checkpoint 无缝兼容。

---

### 方向 D：价值感知 Memory Pipeline（对标验证文档方向五，但聚焦在数据飞轮闭环而非仅管理策略）

**为什么需要：** 当前体系已有一个完备的 memory 存储层（JSONL append-only）和相对丰富的数据模型（Confidence / Source / Supersedes / Kind / Topic），但**消费端只有两个前缀消费者**。这意味着：
1. Memory 的排序完全不看 confidence——低质量 entries 可能淹没高质量 entry
2. Agent 自报告的 confidence 天然不可靠，没有交叉验证
3. `Supersedes` 提供了显式撤回机制，但没有任何自动的"价值衰减"策略

**核心挑战：**
- Confidence 是 agent 自报告的——需要 source-based weight 方案（implementer=0.5, reviewer=1.0）
- 价值衰减需要时间维度，当前 `Entry` 有 `CreatedAtUnix` 但衰减逻辑在 `decayWeight` 中仅用于 scorecard，未用于 memory
- 引入价值排序后，需要保持与现有 `boundMemory` 的向后兼容

**预期架构变更：**

```
当前: boundMemory = recencyFloor(8) + relevance(N - 8)  // 排序不看 confidence
未来: boundMemory = recencyFloor(8) + valueScored(N - 8) // 按 confidence * sourceWeight * decayFactor 排序
                              ↓
                    现有 consumers:
                    - [unverified] prefix (<0.3)
                    - [low-confidence] prefix (<0.7)
                    新增 consumers:
                    - sorting weight in boundMemory
                    - compact retention priority (high-value entries survive compaction)
                    - prompt lane priority (高 confidence 放在更前)
```

**关键设计决策：** Confidence 交叉验证应该是对称的（bidirectional），而非单方向 reviewer→implementer。即 implementer 产出的 knowledge statement，如果 reviewer 未反对（silent approval），应该提升 confidence。这可以通过 `Supersedes + Source` 联合推断实现。

**对现有系统的影响：** 中。核心变更在 `boundMemory` 和 `Compact` 的排序/筛选逻辑，不影响 `memory.Entry` 结构体，不影响 append/load/query 原语。约 300 行变更。

---

### 方向 E：Provider 抽象层集成（对标验证文档方向四，但聚焦在"已声明但未接线"的集成点）

**为什么需要：** 验证文档第 4 方向的修正指出：`routing.ModelMap` / `ResolveModel` / `Providers()` 已经是一个完整的厂商无关路由抽象，但 `engine_build.go` 的 `claudeArgv` 仍然硬编码 `--model` 参数，未从 provider 配置读取。`opusFloorAgents` 仍假设 provider=claude。

**核心挑战：**
- `claudeArgv` 需要知道 provider-specific flags（`--model` vs 其他厂商的 `--model-id`）
- Provider 配置需要接入 project.yml（v1 可走环境变量，v3 可走配置中心）
- 多厂商意味着 `--max-budget-usd` 等成本控制参数的语义因 provider 而异

**预期架构变更：**

```
当前: claudeArgv(o, isClaude, tier, phase) → 硬编码 --model <tier>
未来: claudeArgv(o, provider, tier, phase) → providerRegistry.Lookup(provider).ModelFlag(tier) → "--model <model-name>"
                           ↓
         providerRegistry (现有: ModelMap → ResolveModel)
                           ↓
         新增接口:
         type Provider interface {
             Name() string
             ModelFlag(tier string) (string, string)  // flag name, value
             CostParser() func(string) float64         // 各厂商 JSON 格式不同
             OverloadClassifier() func(error) bool     // 529 等价物
         }
```

**关键设计决策：** 不要在 v2 做全量 provider 抽象——这属于 v3 的"跨厂商池 LiteLLM 集成"。v2 只需把现有的 `ModelMap` 消费端从 `claudeArgv` 的硬编码改为 `ResolveModel` 查询，让后续 provider 扩展只需往 `ModelMap` 追加条目、而不需要修改 engine 逻辑。

**对现有系统的影响：** 低-中。核心变更在 `claudeArgv` 和 `agentExecutor` 的接线，不改变 `routing` 包、不改变 `Engine` 接口、不改变 `buildPrompt`。工作量约 150 行集成代码 + 现有 `ResolveModel` 测试的扩展。

---

## 三、接口设计建议

### 3.1 核心原则

1. **Context 层不感知 Provider 层，Provider 层不感知 Context 层。** 当前架构中 `prompt_context.go`（Context 层）知道 `cost.go`（Provider 层）的存在——`observeFor` 闭包里有 `isClaude` 布尔值。这破坏了层间隔离。建议通过**回调接口**解耦：

```
// 当前: observeFor 内部 if isClaude { ... parseClaudeCost }
// 未来: observeFor 接受 ObserveSink interface { Consume(phase, output) }
//       claudeObserveSink / echoObserveSink / dryRunObserveSink 各自实现
```

2. **数据飞轮的接口应该与存储层正交。** `memory.Entry` 的 `Confidence` / `Source` 是跨层数据，不应只被 prompt 层消费。应该有一个 `ScoreWeighter` 接口（读 Confidence + Source + recency 出排序权重），可被 `boundMemory` + `Compact` + prompt lane 排秩三方复用。

3. **所有扩展点都应该可枚举，而非可插拔的无限扩展。** 当前架构中有两个模式已经做对：`mode` 是有限枚举（explorer / balanced / engineering / cto），`tier` 是有限枚举（haiku / sonnet / opus）。同样，`provider` 也应该是一个有限枚举（当前只有 anthropic，v3 才有 cross-vendor pool）。不要一开始就做 SPI 注册机制。

### 3.2 具体建议

| 模块 | 当前接口形态 | 建议的接口演进 |
|------|------------|--------------|
| Model Router | `ResolveModel(provider, tier)` + `ModelMap` map | 增加 `Provider` interface（但保持为包内私有，不导出 SPI）|
| Token Budget | 无 | 增加 `budgetController` interface：`func Check(lanes []Lane, model string) ([]Lane, error)` |
| Memory Confidence | 字段无消费者 | 增加 `entryWeight(e) float64` 纯函数（可由 Confidence × SourceWeight × decayFactor 组合）|
| Harness Gates | `required_gates: [lint, test, ...]` 是静态列表 | 增加 `compile` gate（已有 adapter 框架可复用）|

### 3.3 向后兼容策略

所有新接口字段都使用 `omitempty` + 零值语义：
- `Event.PromptText` 为空 = 不记录 trace，等价于当前行为
- `Checkpoint.MemorySnapshot` 为 nil = 不恢复 memory，等价于当前行为
- 新增 provider 配置字段时，空值默认退回到 `anthropic` + `claude-*` 系列
- `boundMemory` 新增 confidence-aware 排序时，如果所有 entry 的 confidence=0（无标注），退化到当前 recency+relevance 逻辑

---

## 四、技术选型

### 4.1 需要引入的新技术/框架

| 组件 | 建议 | 理由 | 时机 |
|------|------|------|------|
| Go YAML 库（`gopkg.in/yaml.v3`） | 引入 | 当前 YAML→JSON 经 python shim，是系统唯一的外部运行时依赖 | v2.5（非紧急但持续债务） |
| LiteLLM | 不引入 | 属于 v3 的跨厂商池方向，v2 只有 anthropic（已验证） | v3 |
| Temporal | 不引入 | 属于 v3 的 HA 分布式方向，v2 是单进程 | v3 |
| Embedding/RAG 库 | 不引入 | 已有 BM25-lite（`prompt.Retrieve`），对 v2 够用；语义检索是 v3+ | v3+ |
| 原生 `testing` 包 | 维持 | 已有 `go test` 全绿接入 | 维持 |

### 4.2 第三方依赖评估标准

ForgeOS 的零依赖纪律（`go.mod` 无 `require`）应作为**硬约束**，设立四道防线：

1. **来源审计**：依赖必须 OpenSSF Scorecard ≥ 7.0（防供应链攻击）
2. **编译确定性**：Go module checksum database 验证（防篡改）
3. **许可证兼容**：仅允许 MIT / Apache-2.0 / BSD（禁止 GPL/LGPL/AGPL）
4. **版本锁定**：`go mod vendor` + `go mod verify` 确保可重现构建

当前唯一需要突破这条纪律的是 YAML 库，因为 python shim 才是真正的"运行时外部依赖"——它假设 Python 3 + PyYAML 在 PATH 上。这是一个隐性依赖，比显式 Go 依赖更不可控。

### 4.3 自建 vs 采购决策矩阵

| 组件 | 决策 | 依据 |
|------|------|------|
| Provider Registry | **自建**（已存在，需接线） | 当前只有 anthropic，v3 才有 cross-vendor pool |
| Token Budget Controller | **自建** | 领域特异性强，无现成库 |
| Confidence Cross-validator | **自建** | 领域特异性强 |
| Replay Bundle | **自建**（在 trace + persist 上扩展） | 已有基础设施 |
| YAML Parser | **采购**（`gopkg.in/yaml.v3`） | 标准库缺位，不属核心创新 |

---

## 五、实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 估算工作量 | 独立程度 | 立即可见收益 |
|--------|------|-----------|---------|------------|
| **P0** | A - Context Budget Controller Phase A（token 估算 + 超限降级） | ~200 行 | ✅ 独立 | Haiku phase 静默溢出阻断 |
| **P0** | B - Semantic Validation Pipeline Phase A（`compile` gate） | ~200 行 | ✅ 独立 | 新代码不编译即在 gate 层 FAIL |
| **P1** | D - Value-Aware Memory Pipeline Phase A（confidence 进入排序权重） | ~300 行 | ✅ 独立 | Memory 不再被低质 entries 淹没 |
| **P1** | E - Provider 抽象集成 | ~150 行 | ⚠️ 低依赖 | routing 包不再有悬空接口 |
| **P2** | C - Deterministic Replay Bundle | ~250 行 | ✅ 独立 | 审计能力提升但非当前紧迫 |

**与验证文档的优先级建议一致：同时做方向 A Phase A + 方向 B compile gate，再加方向 D Phase A——三个加起来 ~700 行，各自独立，收益立即可见。**

### 5.2 阶段划分

#### Phase 1（Sprint N）—— 止血：~700 行

| 子任务 | 产出 | 验收标准 |
|--------|------|---------|
| Token 估算器 | `internal/budget/tokenizer.go` 纯函数 | 对已知文本估算误差 < 15%（与 claude tokenizer 比） |
| Prompt 装配→预算门 | `buildPrompt` 内嵌预算检查 | assemble 超限时诚实注入截断标记 |
| `compile` gate | `harness/adapters/<lang>/compile.yml` + probe | 编译失败 → harness FAIL，编译通过 → PASS |
| Confidence 排序 | `boundMemory` 置信度加权策略 | 高 confidence entries 优先保留，向后兼容 |

#### Phase 2（Sprint N+1）—— 增强：~400 行

| 子任务 | 产出 | 验收标准 |
|--------|------|---------|
| Provider 集成 | `claudeArgv` 走 `ResolveModel` | `--model` 值从 ModelMap 查询而非硬编码 |
| Confidence 交叉验证 | `sourceWeight(phase string) float64` | implementer=0.5, reviewer=1.0, planner=0.7 |
| Memory compact 价值感知 | `Compact` 按 weight 排序保留 | 高价值 entries 优先保留 |

#### Phase 3（Sprint N+2+）—— 完备：~250 行

| 子任务 | 产出 | 验收标准 |
|--------|------|---------|
| Replay Bundle Level 1 | `trace.Event.PromptHash` | 可检测 prompt 漂移 |
| Replay Bundle Level 2 | `--full-trace` prompt 快照 | 可完整复现 agent prompt |

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| Token 估算不准导致过度截断 | 中 | 中 | 按模型分 tokenizer（Haiku/Sonnet/Opus 不同），提供 `--no-truncate` 逃生门 |
| Confidence 排序改变现有 prompt 输出 | 低 | 低 | `boundMemory` 零 confidence 退化到当前行为；回归测试验证 |
| Provider 抽象引入跨厂商 leaky abstraction | 低 | 中 | 范围限定在当前 `ModelMap` 的消费端接线，不引入 SPI/插件机制 |
| `compile` gate 与 adapter 框架交互复杂 | 中-低 | 低 | 已有 `probeLint` 模式可完全复用，`compile` gate 只是 adapter 框架的新实例 |

### 5.4 不放过的架构观察

以上分析揭示了一个更深的模式：**ForgeOS 在数据采集层已经过度建设（over-instrumented），在数据消费层严重欠建设（under-utilized）。**

- `memory.Confidence` → 只有 2 个消费者（前缀标记），0 个用于排序/筛选
- `routing.ModelMap` / `ResolveModel` / `Score` / `TierForScore` / `BudgetAdjustTier` → 只有 1 个消费者（`forge route` CLI），0 个用于 engine 自动执行
- `converge.Signals` 的 8 个字段已经全部闭环（Sprint 29 已审计），但这是**人工审计保障**，不是 architecture 层面的自闭环设计

这不是偶然——它源于项目的演化顺序（先建底层基础设施，再在上层接线）。现在基础设施已经就位，**架构工作的重心应从"新建基础设施"转向"接线消费已存数据"**。五个方向的本质都是接线任务。

**最高杠杆建议：** 在每个 sprint 的计划阶段，明确标记哪些工作是"建新数据管道"（新的引擎/存储/抽象），哪些是"接已有数据消费"（给已经存在的字段找第二个消费者）。后者应该优先于前者，直到数据采集/消费比降到 ~1:2。
