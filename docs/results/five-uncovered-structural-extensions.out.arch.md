# 架构分析与扩展建议：五个结构性前沿方向

> **分析对象**: `docs/requirements/2026-07-12-five-structural-architectural-frontiers-uncovered.md`  
> **分析角色**: 资深架构师  
> **分析前提**: 75+ 份已有分析已被视为已读且被尊重边界

---

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 当前的架构设计展现出若干值得肯定的品质：

**强分层治理架构**：`modes.yml` → `internal/mode/mode.go` → 全局 gate 执行 → `forge accept` 闸门的链路，体现了清晰的策略-机制分离。这也是为什么五个方向的「漂移检测」会成为问题——因为分层本身是好的，但缺乏维护层间一致性的自动化手段。

**零外部依赖原则的可持续性**：forge-core 在 18+ Go 包中坚持纯标准库，没有任何外部依赖。这在 31 个 sprint 的演进中保持了架构整洁，没有出现依赖地狱。五个方向的方案建议均**尊重这一原则**，这是架构纪律的体现。

**渐进丰富的工作流语义**：从 `feeds_forward` 到 `loop-back` 到 `converge` 到 `checkpoint`，工作流语义在持续丰富但从未破坏已有 phase 的行为。这是对未来兼容性的良好设计。

**以文件为中心的数据流**：Phase 之间通过文件系统交换数据（`emits:` → 文件 → `feeds_forward`），这个设计决策虽然简单但也有其合理性——可观测（文件系统你可以 `ls`）、可调试（vim 打开即可编辑）、零外部基础设施。

### 1.2 当前架构的局限性

五个方向暴露出的本质上是同一个架构问题的五个切面：**执行过程的可审计性与可验证性的缺失**。

**信任的单向依赖问题**：当前架构中，外层（用户/策略声明者）信任内层（forge-core 实现）的方式是隐式的——用户声明了 `modes.yml: max_function_lines: 500` 就**假设**系统在按这个阈值执行，但没有任何机制保证假设成立。这是五个方向中都存在的信任不对称：

```
用户声明 ───隐式信任──→  Go 实现
   ↓                         ?
   无法验证                 可能偏离声明
```

**语义数据的「用后即弃」模式**：`orchestrator.go` 对 phase 输出的处理方式是：提取需要的信息（裁决、成本）→ 丢弃其余。这在短期运行中没问题，但限制了 ForgeOS 向「可审计自治系统」方向发展的能力。这不是一个 bug，而是一个**架构权衡**——用减少持久化来换取实现简单性。五个方向的共同判断是：这个权衡现在需要重新评估。

**观测体系的不对称**：ForgeOS 能详细观测被治理的应用（agent 成本/延迟/质量分），但对自身的性能和行为几乎零观测。这在「谁来看守看守者」问题中相当于看守者没有被看守——在自治系统语境中，这是一个架构风险。

**契约的语义缺失**：`emits:` 声明了「应该产出什么文件」，但不声明「文件应该长什么样」。`feeds_forward` 注入「前一个 phase 的产出」，但不注入「应该如何使用这个产出」。当前架构中的契约都是**存在性契约**（有/没有），不是**语义契约**（是否符合预期）。

### 1.3 关键设计决策评估

| 决策 | 评价 | 建议调整方向 |
|------|------|------------|
| `trace.Event` 只记录指标不记录语义 | **当时合理，现在需要重新评估**。在早期 sprint 中，指标足够诊断基本问题。随着工作流复杂度增加（loop-back/converge/checkpoint），语义缺失成为审计盲区 | 引入并行语义事件流，与指标事件共存 |
| `feeds_forward` 是注入不是契约 | **架构上偏弱的决策**。注入是最简单的实现方式，但将理解责任完全推给了下游 LLM，系统层零验证 | 在注入层之上叠加轻量验证层（意图-交付一致性检查） |
| 策略值在 Go 代码中硬编码 | **短期内可接受，长期不可持续**。零依赖原则限制了配置动态加载的能力。当前`mode.go`的 baseline 表是合理的架构隔离，但缺少对账机制 | 添加对账审计命令，不改变运行时行为 |
| Phase 产出物不做存在性验证 | **合理的延迟决策**——在早期 sprint，这属于可接受的简化。在 31 个 sprint 之后，复杂度积累使这个缺口的影响变大 | 添加 WARN 级别的存在性检查，不阻断执行 |

### 1.4 架构债务（Technical Debt）

五个方向共同揭示了四类架构债务：

1. **可观测性债务**：`forge-core` 自身无埋点、无性能基准、无正确性计数器。这使性能退化只能在用户体验层面被感知（用户抱怨「变慢了」），无法在 CI 层面被捕获。
2. **契约债务**：Phase 之间的数据流依赖隐式格式约定，无版本化、无 schema、无存在性检查。多个 phase 的 pipeline 越长，这种隐式依赖的风险越高。
3. **同步债务**：配置声明层与代码实现层之间的手工值复制。这是架构层面的「重复代码」——重复的不是业务逻辑，而是策略值。
4. **审计债务**：工作流执行后不留语义轨迹。从「能用」到「可信」之间，差的不是 agent 能力，而是执行档案。

**关键判断**：这些债务在当前阶段（v2）是**非致命但具有累积风险的**。如果 ForgeOS 在 v3 或 v4 要向「完全自治」、「多仓库联邦」、「合规环境」发展，这些债务会成为瓶颈。

---

## 2. 扩展方向

以下 5 个扩展方向不是对五个方向的简单展开，而是从架构角度提出的**进一步的、更高层级的架构提议**，与五个方向互补且不重叠。

### 方向 A：执行档案（Execution Archive）—— 将「run」从瞬时过程变为可查询资产

**为什么需要**：当前每个 `forge run` 是一次性事件——stdout 输出、`trace.jsonl` 留下指标、`checkpoint` 留下阶段索引。但 `run` 本身不是一等公民——没有全局 `id`、没有语义档案、没有跨运行关联。方向一的语义日志是执行档案的基础原料，方向二的意图一致性和方向四的产出物 Schema 是其结构化填充。

**业务价值**：
- 已认证环境（审计/合规/医疗/金融）需要「每个版本从需求到交付的完整可追溯链」，执行档案直接满足这个需求
- 开发者可以 `forge log --run <id> --timeline` 查看完整故事线，而非搜索终端回滚
- 执行档案是「回放」（replay）和「对比」（diff-runs）的基础

**核心挑战**：
- 持久化量级：一个复杂 evolve run 可能产生 MB 级语义数据，需要存储策略
- 跨版本兼容：未来修改事件结构时如何处理历史档案
- 性能影响：写入语义日志不能阻塞主执行路径

**预期架构变更**：
```
当前：orchestrator → Execute → stdout + trace.jsonl(metrics)
未来：orchestrator → Execute → stdout + trace.jsonl(metrics+semantic) 
                                   → persist/archive.(run_id).ndjson
                                   → archive index (run_id → timestamp, workflow, status)
```

新增组件：
- `internal/archive` 包：管理执行档案的写入、查询、裁剪
- `cmd/forge/log.go`：`forge log` 子命令
- `cmd/forge/archive.go`：`forge archive prune/list/export` 子命令

**对现有系统的影响**：
- `orchestrator.go` 从执行路径上发射语义事件（增量添加），不改变现有控制流
- `trace.jsonl` 消费者（`scorecard-update.mjs`）需要跳过 `type: "semantic"` 的行——一行改动
- 新事件类型与已有事件是并行关系，零行为破坏

**技术难度**：低～中。架构影响集中在数据模型设计。

---

### 方向 B：策略值注册表（Policy Value Registry）—— 声明层与实现层的自动对账框架

**为什么需要**：方向五指出声明-实现漂移的三个证据（`modes.yml` vs `mode.go`、`policies.yml` vs `arch-check.mjs`、`routing.policy.yml` vs `routing.go`），但给出的方案是审计命令（`forge audit --drift`）——人对账。一个更深远的架构方向是：**从人工对账走向注册表自描述，最终走向自动同步**。

**业务价值**：
- 策略作者改 `modes.yml` 后立即知道哪些 Go 常量需要更新，而非假设「可能系统会自动用新值」
- CI 直接拦截声明-实现不匹配，而非等到生产行为异常
- 为未来「热加载策略」和「策略中心」建立数据基础

**核心挑战**：
- forge-core 零外部依赖原则：不能引入运行时 YAML 配置读取
- 声明层和实现层的生命周期不同：策略作者可能先改 YAML，Go 实现者后更新代码——审计需要容忍时间窗口
- 有些 Go 常量的值有意偏离声明（例如安全增强），需要区分「故意漂移」和「无意漂移」

**预期架构变更**：

```
当前：
  modes.yml ──手写──→ mode.go (baseline table)
  
未来-阶段1（审计）：
  forge audit --drift ──解析 YAML──→ 查找 Go 常量──→ 输出漂移报告
  
未来-阶段2（注册表）：
  internal/registry
    ├── Declared(map[string]interface{})      // 从 YAML 解析的值
    └── Implemented(map[string]interface{})   // Go 常量的自注册值
       └── Register(key string, value interface{}, source string)
```

新增组件：
- `internal/registry` 包（可选，仅阶段 2）
- `cmd/forge/audit.go`：`forge audit --drift` 子命令（阶段 1）
- 注释解析器：从 Go 源码中提取 `Source:` 注释定位对应的 YAML 值

**对现有系统的影响**：
- 阶段 1 零运行时影响，仅在显式调用时执行
- 阶段 2 需要在 `mode.go` 等文件中将常量声明改为注册表调用，影响现有代码结构
- 建议停留在阶段 1 直至分布式化（多服务策略同步）真正提上日程

**技术难度**：中。注释解析 + YAML 值匹配需要稳妥的匹配算法，避免误报。

---

### 方向 C：Phase 产物契约层（Artifact Contract Layer）—— 从隐式依赖到显式契约

**为什么需要**：方向四指出 `emits:` 只声明不验证。方向二指出 `feeds_forward` 是注入不是契约。这两个问题的共同根因是：**Phase 之间的协议没有在系统层显式表达**——协议只存在于 LLM 的 prompt 中。如果将一个复杂 pipeline（`discover → plan → implement → review → converge`）中的每一对 phase 之间的数据依赖显式化，Many 个隐式 bug 就可以被提前捕获。

**业务价值**：
- 减少「phase A 改了输出格式，phase B 默默产出垃圾」的无声数据损坏
- 多 implementer 场景下的分工边界守卫——每个 implementer 知道自己该改什么文件、不该改什么文件
- `forge validate --emits` 可以在不执行 phase 的情况下验证 pipeline 的拓扑一致性

**核心挑战**：
- 不引入外部 schema 系统（JSON Schema 虽好但增加依赖）
- Markdown 产出物的结构验证非常困难（Markdown 本身就是非结构化的）
- 向后兼容——大量现有 phase 没有声明 schema，不能因此阻断执行

**预期架构变更**：

```
当前：
  workflow.yml → emits: [file.md] → 不检查存在性 → feeds_forward(注入原始输出)

未来：
  workflow.yml → emits: [file.md] + emit_schema(可选)
                → 执行后存在性检查(WARN)
                → 如果声明 schema: 格式验证(WARN)
                → 下游 phase 注入时过滤已知结构
```

新增/修改组件：
- `internal/phase/artifact.go`：存在性检查 + 格式验证逻辑
- `internal/phase/schema.go`：轻量 schema 匹配（Markdown 标题检查、YAML 匹配模式）
- 修改 `orchestrator.go`：在每个 phase 执行后插入 artifact check

**对现有系统的影响**：
- 存在性检查对现有 workflow 零影响（WARN 级别，不断言）
- format schema 仅当声明了 `emit_schema` 才执行，现有 workflow 不受影响
- 未来 `forge validate --emits` 可以由设计者选择执行

**技术难度**：低（存在性检查）+ 中（Markdown 结构检查）

---

### 方向 D：多 Agent 执行的可观测性架构（Multi-Agent Observability Framework）

**为什么需要**：方向二触及了一个更深层的问题——planner 和 implementer 之间的意图传递是单向、不可验证的。但这不是孤例。在 ForgeOS 中，reviewer 不知道 planner 的意图（`fresh_context: true`），converger 不知道 reviewer 的裁决细节，gate 不知道 phase 执行的具体内容。这是**多 Agent 协作的可观测性缺口**——每一个体 Agent 只能看到自己的上下文，系统层没有「全局故事线」。

**业务价值**：
- 诊断复杂失败场景：一个多轮 loop-back 最终 converge fail 了，原因是 reviewer 对 implementer 的第 2 轮输出了 6 条 feedback 中的 3 条未被 address。当前需要逐轮读日志来拼凑这个故事。
- 跨 Agent 行为偏差检测：planner 规划了 P1、P2、P3，implementer 做了 P1、P2、P4——reviewer 只评审了 P1 和 P2，没注意到 P4 是计划外的。系统层应该有机制检测这种「计划外增量」。
- 收敛瓶颈定位：一个 10 次 loop-back 的 cycle，瓶颈是 reviewer 的裁决不一致（前 3 次 approve，后 7 次 request changes）。需要系统层记录每次裁决的方向变化。

**核心挑战**：
- 信息粒度平衡：记录太多 → 膨胀，记录太少 → 无用
- Agent 裁决的结构化提取：当前 `parseReviewerVerdict` 用正则从文本末尾提取 token，如果内容更广（agent feedback/具体建议），提取难度加大
- 不能影响 `fresh_context` 的设计初衷——reviewer 不看到 planner 的输出是故意的，目的是保持独立判断

**预期架构变更**：

```
当前：
  planner ──feeds_forward──→ implementer ──feeds_forward──→ reviewer
     │                           │                              │
     └─────── 各自 trace ────────┴─────────────┘              

未来：
  planner ──feeds_forward──→ implementer ──feeds_forward──→ reviewer
     │                           │                              │
     └─────── 共享事件总线 ───────┴──────────────┘
               每条事件包含：
               - role (planner/implementer/reviewer)
               - phase_id
               - verdict (如果适用)
               - intent_summary (planner 的原始规划)
               - files_changed (implementer 的实际变更)
               - convergence_criteria_state (converger 的评估状态)
```

新增组件：
- `internal/observe` 包：多 Agent 事件总线（非消息队列，是结构化日志采集器）
- `internal/observe/storyline.go`：从事件重建「故事线」——按时间戳排序，输出完整 narrative
- `cmd/forge/observe.go`：`forge observe --run <id> --storyline`

**对现有系统的影响**：
- 现有 phase 的执行路径完全不变
- 在 phase 完成后 emit 事件（同方向一的语义事件），增量添加
- 不影响 `fresh_context` 的行为

**技术难度**：中。数据结构设计比实现更难——定义正确的事件 schema 比实现写入路径更具挑战性。

---

### 方向 E：自治系统的故障检测与退化模式（Self-Diagnosis & Degradation Detection）

**为什么需要**：方向三（forge-core 内部遥测）和方向一（语义日志）的共同扩展方向是：将 ForgeOS 从一个「按需运行的工具」进化成一个「可以长时间无人值守运行并自诊断故障的自治系统」。这需要检测的不是单一的二值状态（pass/fail），而是**退化模式**——性能变差了、正确率下降了、收敛效率降低了。

**业务价值**：
- 24h+ 无人值守运行：如果某个 phase 的响应时间从 3s 逐渐退化到 30s（比如因为 LLM provider 限流），当前无法自动察觉
- 模式识别：某类 workflow 在天气环境下反复 converge fail，系统自动标记「可疑模式」
- 自我修复触发：检测到 `yaml2json` 的 error rate 从 0.01% 跳到 5%，自动触发回滚到上一个已知好的版本

**核心挑战**：
- 基线建立：需要足够的历史数据来定义「正常」
- 告警阈值：太敏感 → 告警疲劳，太宽松 → 漏报
- 与 CI 集成：在开发流程中不会因为「本机性能差」而误报退化

**预期架构变更**：

```
当前：
  无内部性能跟踪
  无退化检测
  无自我诊断

未来：
  internal/telemetry ──定期快照──→ benchmark.json (git-tracked)
       │
       ├── forge self-check --perf (当前 vs 基线比较)
       ├── forge self-check --correctness (正确性计数器比)
       └── forge self-check --degradation (趋势分析)
```

新增组件：
- `internal/telemetry` 包（方向三的基础，扩展为包含退化检测）
- `internal/telemetry/trend.go`：滑动窗口趋势分析
- `cmd/forge/self-check.go`：`forge self-check` 子命令扩展

**对现有系统的影响**：
- 零影响，全部新增代码在新包中
- 仅在显式调用 `forge self-check --degradation` 时执行趋势分析

**技术难度**：中高。退化检测的统计学方法选择（滑动窗口 / EWMA / 简单的基准偏移）需要谨慎，避免误报。

---

## 3. 接口设计建议

### 3.1 关键模块接口设计原则

基于五个方向的共同模式，提出以下接口设计原则：

**原则一：事件接口的版本化自描述**

```go
// internal/event 包的设计原则
// 每个事件包含 version 字段和 type 字段
// type 用于路由，version 用于消费者兼容性判断

type Event struct {
    Version int    `json:"v"`         // 事件结构版本号
    Type    string `json:"type"`      // "metric" | "semantic" | "diagnostic"
    Ts      int64  `json:"ts"`        // unix nano 时间戳
    Data    json.RawMessage `json:"d"` // 不同类型携带不同数据结构
}
```

**原则二：验证结果使用三态结构（而非二态）**

方向一～四中涉及的多个验证点（存在性、格式、意图一致性、性能退化）共享同一个结果模式：

```go
type CheckResult struct {
    Check   string       // "artifact-exists" | "schema-match" | "intent-delivery"
    Status  CheckStatus  // PASS | WARN | FAIL | SKIP
    Detail  string       // 人类可读描述
    Context map[string]any // 结构化上下文
}
```

**原则三：持久化接口的写入-查询分离**

所有五个方向都需要持久化新类型数据（语义事件、执行档案、内部指标）。采用**写入优先、查询可扩展**的模式：

- 写入路径：append-only 文件写入（`bufio.Writer` + 定时 flush）
- 查询路径：独立于写入路径，通过索引（`run_id` → offset 映射）或全量扫描
- 不要求实时索引，不引入外部存储

### 3.2 是否需要引入新的抽象层

**建议引入一个轻量的 `event` 抽象层**。

当前 `internal/trace` 包同时承担了指标收集和持久化两个职责。五个方向将产生大量新的事件类型，如果全部堆入 `trace` 包，会造成单包过大。建议：

```
internal/trace/    →  保持现有指标事件不变（向后兼容）
internal/event/    →  新建通用事件框架
    ├── emitter.go     (事件发射接口)
    ├── sink.go        (事件持久化接口)  
    ├── semantic.go    (语义事件类型定义)
    └── diagnostic.go  (诊断事件类型定义)
```

**不引入**的抽象层：
- 不引入消息队列或事件总线（overkill，且违反零依赖原则）
- 不引入独立的 schema registry（复杂度太高，用 JSON Schema 或轻量 DSL 即可）
- 不引入 ORM 或数据库抽象层（保持文件系统为基础存储）

### 3.3 向后兼容性策略

五个方向的向后兼容策略可以统一为「**可选增强、零行为破坏、默认不启用**」：

| 方向 | 默认行为 | 主动启用 | 兼容保证 |
|------|---------|---------|---------|
| 方向一（语义日志） | 不写入语义事件 | 设定 `FORGE_SEMANTIC_LOG=1` 或运行时调用 | `trace.jsonl` 格式不变，已有消费者跳过未知 `type` |
| 方向二（意图一致性） | 不执行意图验证 | `forge run --verify-intent` | 现有 workflow 无效应，无 INTENT 声明时退化为无验证 |
| 方向三（内部遥测） | 不收集内部指标 | `forge metrics` 子命令显式调用 | 不改变现有 CLI 行为 |
| 方向四（产出物验证） | 不做存在性/格式检查 | `emit_schema:` 声明后自动启用 | 无 schema 声明时零行为变化 |
| 方向五（漂移检测） | 不运行对账 | `forge audit --drift` 子命令显式调用 | 纯新增子命令，不影响现有子命令 |

**核心原则**：五个方向的**主动使用**不影响**不使用**的用户。他们看到的行为完全不变。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈

**基本结论：不需要引入新的外部技术栈或框架。**

五个方向可以在现有技术栈（Go 标准库 + Node.js harness 非外部依赖）内全部实现。具体分析：

| 候选技术 | 用途 | 是否必需 | 决策 |
|---------|------|---------|------|
| JSON Schema 库 | Phase 产出物验证 | 否 | 自建轻量 DSL（Markdown 标题检查 + 简单模式匹配） |
| OpenTelemetry | 内部遥测标准 | 否 | `sync/atomic` 计数器 + `time.Now()` 足够 |
| Prometheus 格式 | 指标暴露 | 否 | `forge metrics` 的文本输出已是人类+机器可读 |
| SQLite / BoltDB | 执行档案存储 | 否 | ndjson 文件 + 扫描查询在 v2 规模下足够 |
| gRPC / Protobuf | 多 Agent 事件总线 | 否 | 文件系统共享数据的设计已足够 |
| Python YAML 解析器 | 漂移检测 | 否 | forge-core 已有 `internal/yaml2json` |

**建议**：在 v2 阶段保持零外部依赖。如果未来分布式化需要，可以在 v3-v4 阶段评估 BoltDB/RocksDB/MongoDB。

### 4.2 第三方依赖的评估标准

如果未来需要在 v3+ 引入外部依赖，建议采用以下评估标准：

1. **许可证兼容性**：必须是 MIT/Apache-2.0/BSD，排除 GPL/AGPL/SSPL
2. **依赖传递的深度**：引入一个库=引入其所有传递依赖。每个依赖的依赖深度 > 3 需要单独论证
3. **构建时间影响**：新依赖不能使 forge-core 构建时间增加 > 30%
4. **二进制体积影响**：新依赖不能使 forge-core 二进制体积增加 > 20%
5. **活跃维护**：GitHub star > 100、最近 6 个月内有 commit、至少 2 个 maintainer
6. **安全性**：CVE 数据库中已知漏洞数 = 0

### 4.3 自建 vs 采购的决策依据

五个方向涉及的组件均为 ForgeOS 核心差异化的组成部分，应**全部自建**：

| 方向 | 是否可采购 | 为什么自建 |
|------|-----------|-----------|
| 语义日志系统 | 可采购（ELK/DataDog/Splunk） | ForgeOS 的语义事件模式高度特化，通用日志系统无法理解 `LoopBackTriggered` 或 `ConvergenceVerdict` 的语义；且引入外部依赖违反零依赖原则 |
| 意图一致性验证 | 不可采购 | 这是 ForgeOS 领域特定问题，外部没有针对 LLM pipeline 的计划-执行一致性验证产品 |
| 内部遥测 | 可采购（Prometheus/Datadog APM） | 但在 v2 阶段过于重，`sync/atomic` 就够用；到分布式化阶段再评估采购 |
| Schema 强制 | 可部分采购（JSON Schema 库 + 通用 schema registry） | JSON Schema 库可引入（但建议自建轻量 DSL 以保持零依赖），schema registry 自建 |
| 漂移检测 | 不可采购 | 这是 ForgeOS 特有的策略声明-代码实现双真相源问题，通用方案不存在 |

---

## 5. 实施路线图

### 5.1 优先级排序

基于五个方向的依赖关系、系统影响度和实现复杂度：

```
                  P1（立即）
                  ┌─────────────────┐
                  │   方向五（漂移      │
                  │   检测——审计命令）  │ ← 最低复杂度、最高收益
                  └────────┬────────┘
                           │ 方向一（语义日志）是方向二/四的基础
                  ┌────────▼────────┐
                  │   方向一（语义    │
                  │   日志——事件框架）│ ← 架构基础设施
                  └────────┬────────┘
                           │
         ┌─────────────────┼─────────────────┐
         ▼                 ▼                  ▼
   ┌──────────┐     ┌──────────┐     ┌──────────────┐
   │ 方向二    │     │ 方向四    │     │ 方向三         │
   │（意图验证）│     │（产出物    │     │（内部遥测）     │
   │          │     │  Schema） │     │              │
   └──────────┘     └──────────┘     └──────────────┘
   P1.5               P2               P2
   （依赖方向一/五）   （独立）          （独立）
```

**最终优先级排序**：

| 优先级 | 方向 | 阶段 | 依赖 |
|--------|------|------|------|
| **P1** | 方向五「声明-实现漂移检测」 | 阶段 1 | 无（独立子命令） |
| **P1** | 方向一「语义日志」— 事件框架 | 阶段 1 | 无（新增内部包） |
| **P1.5** | 方向二「跨 Phase 意图一致性」 | 阶段 2 | 方向一的事件框架 + 方向五的声明值注册能力 |
| **P2** | 方向四「Phase 产出物 Schema」 | 阶段 2 | 方向一的事件框架（作为验证结果输出目标） |
| **P2** | 方向三「内部遥测」 | 阶段 2 | 方向一的持久化机制 |

### 5.2 阶段划分和里程碑

#### 阶段 1（P1，优先实现，约 2-3 sprint）

**目标**：建立五个方向共用的基础设施 + 启动两个独立 P1 方向。

**里程碑 M1**（Sprint N）：`internal/event` 包落地
- 定义通用 Event 接口（versioned, typed, self-describing）
- 定义指标事件（现有 `trace.Event` 的迁移或封装）和语义事件接口
- 实现 ndjson 顺序写入（`EventSink`），支持定时 flush
- 退出前验证：新增的语义事件写入路径不影响现有 `trace.jsonl` 的写入行为

**里程碑 M2**（Sprint N+1）：`forge audit --drift` 子命令
- 实现 YAML 策略文件解析（复用 `internal/yaml2json`）
- 实现 Go 常量提取（正则 + AST 扫描）
- 实现 `Source:` 注释解析与值匹配
- 输出格式化（颜色标记漂移/同步/故意漂移）
- CI 集成：`forge.yml` 中加入 `forge audit --drift` 步骤（WARN 级别）

**里程碑 M3**（Sprint N+2）：方向一语义事件落地
- 在 `orchestrator.go` 的关键点发射语义事件：
  - `PhaseCompleted`：phase 执行完毕后
  - `ConvergenceVerdict`：converge 评估后
  - `GateResult`：gate 执行后（已有，补全细节）
  - `LoopBackTriggered`：loop-back 发生时
- `forge log` 子命令 MVP：按 run 查询、按 event type 过滤

#### 阶段 2（P1.5 + P2，约 3-4 sprint）

**目标**：实现方向二、四、三的核心功能。

**里程碑 M4**（Sprint N+3）：方向四产出物验证
- 实现存在性检查（`os.Stat(emit_path)`）
- 实现格式验证框架（Markdown 结构 DSL + 机读标记检查）
- 修改 `orchestrator.go`：phase 执行完毕后插入验证步骤（WARN 级别）
- 验证结果写为语义事件（类型：`ArtifactCheckResult`）

**里程碑 M5**（Sprint N+4）：方向二意图一致性验证
- 实现 INTENT 声明提取（从 planner 输出中提取机读段）
- 实现 `git diff --name-only` vs planner intent paths 的匹配检查
- 实现意图覆盖率报告（在 convergence report 中追加一行）
- 验证结果写为语义事件（类型：`IntentDeliveryCheckResult`）

**里程碑 M6**（Sprint N+5）：方向三内部遥测
- 实现 `internal/telemetry` 包（`sync/atomic` 计数器 + `time.Now()` 计时）
- 在关键路径埋点：`loadWorkflow`、`gatherSignals`、`buildPrompt`、`converge.Converge`、`yaml2json.Decode`
- `forge metrics` 子命令输出当前内部指标
- CI 基准快照初始化（首次运行 `forge metrics --save-baseline`）

### 5.3 风险点和缓解策略

| 风险 | 影响 | 概率 | 缓解策略 |
|------|------|------|---------|
| 语义日志文件过大导致磁盘耗尽 | 中 | 中 | 1) 默认 10MB cap + 裁剪策略（先裁剪语义事件、后裁剪指标事件）2) WARN 当日志 > 80% cap 时输出告警 3) `forge log prune` 手动清理 |
| `forge audit --drift` 误报导致 CI 噪声 | 中 | 高 | 1) `Source:` 注释的「故意漂移」注解机制 2) `drift-exceptions.json` 已知缺口声明白名单 3) 初始阶段使用 WARN 级别，确认稳定后再提升到 FAIL |
| 存在性检查 WARN 淹没正常输出 | 低 | 中 | 1) 仅在 `--verbose` 模式或 `forge log --event-type artifact-check` 时显示明细 2) convergence report 只显示聚合数（"3 artifact warnings"） |
| 意图提取的准确度受 LLM 输出格式影响 | 中 | 中 | 1) 同 verdict 契约模式：要求 INTENT 声明在最后若干行，机器可读 JSON 段 2) 提取失败时透明退化为「无意图」+ WARN 3) 不阻断执行 |
| 内部遥测的 atomic 计数器在高频调用下的性能开销 | 低 | 低 | 1) 验证 `sync/atomic` 在 Go 中的开销（纳秒级，可忽略）2) 只对 O(10^3+) 频率的操作使用采样（每 N 次记录一次） |

### 5.4 依赖注入链路

为了让五个方向的实施顺序清晰，这里给出一个简化的依赖注入关系：

```
方向五（forge audit --drift）
    │  不需要任何其他方向的基础设施
    ▼
方向一（internal/event 包）
    │  方向五的检查结果可以使用 event 包持久化（可选）
    │  方向一为方向二/四/三提供事件写入框架
    ├──► 方向二（意图验证结果写入 event 包）
    ├──► 方向四（artifact 验证结果写入 event 包）
    └──► 方向三（telemetry 数据写入 event 包，或独立文件）
```

**关键决策**：方向五和方向一没有硬依赖，可以并行开发。方向一的事件框架是方向二/四/三的共享基础设施，所以方向一的 `internal/event` 包应该率先完成。

---

## 总结

五个方向构成的不是五个孤立的功能需求，而是一个**有结构的架构升级路径**：

| 层 | 方向 | 做什么 | 为什么重要 |
|---|------|--------|-----------|
| 内层 | 方向三「内部遥测」 | 让 forge-core 自身变得可观测 | 「谁来看守看守者」——自治系统的自保能力 |
| 中层 | 方向一「语义日志」 | 记录「发生了什么」而非「花了多少」 | 执行过程的可审计性——从「能用」到「可信」 |
| 中层 | 方向二「意图一致性」 | 验证计划→执行的一致性 | 多 Agent 协作的信任基线 |
| 中层 | 方向四「产出物 Schema」 | 验证 phase 产出物的结构和存在性 | 数据完整性的契约保障 |
| 外层 | 方向五「漂移检测」 | 确保声明层和实现层同步 | 治理系统的信任根基 |

这五个方向比任何单一「加功能」或「加性能优化」的方向都更具系统性价值——它们修复的是**「系统相信自己正在做它以为自己正在做的事」**这个基础的信任链。在自治系统这个语境中，这个信任链不是锦上添花，而是必需品。
