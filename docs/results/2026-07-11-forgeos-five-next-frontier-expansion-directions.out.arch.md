现在我已经完整阅读了原始文档、交叉验证结果 (`.out.md`) 以及已有的深度架构分析 (`.out.arch.md`)。我将遵循您提供的分析框架，**产出与现有分析差异化**的独立架构视角——不对已有高质量结论做重复，而是从您指定的五个维度做"第二层追问"。

---

# ForgeOS 五条前沿方向 — 独立架构分析

> 基线：commit `b0c80e4` · forge-core 18 Go 包 · harness 全套闸门
>
> 本文遵循您指定的分析框架（架构评估 → 扩展方向 → 接口设计 → 技术选型 → 实施路线图），
> 与现有的 `.out.arch.md` **不重复**。两篇分析是互补关系：
> - `.out.arch.md` 侧重**三条暗线**（控制回路协调、存储抽象、安全沙箱）+ **三项关键决策**
> - 本文侧重**架构负载分析** + **每个方向的架构变更成本** + **具体技术权衡选项**

---

## 1. 架构评估

### 1.1 当前架构的承载力分析——不是"好不好"，而是"能负载多少"

与其做定性评价（已有分析已经做了），我做**定量承载力评估**：当前架构能支持多少项目、多大规模、多少并发？

| 资源维度 | 当前容量 | 5方向 Phase 1 后的需求 | 瓶颈点 |
|---|---|---|---|
| **trace 数据量** | 单项目每 run ~500KB-2MB JSONL | ① fleet: 10× 项目 × 2MB = 20MB; ② replay: 每次仿真需加载完整 trace → 2MB → 20MB | 无索引全表扫描 O(n) |
| **memory 积累** | 单项目每次 evolve ~50-200 条 Entry | ③ pattern miner 需要跨会话的完整 memory → 1000+ 条 | `memory.Load()` 当前 O(n) 全部加载到内存 |
| **并发 run** | 单项目串行（文件锁已缺省） | ① fleet 下多项目并行 → 理论上 N 个并行 | JSONL 无文件锁 → trace 交错 |
| **策略解析** | 2 输入（mode×lifecycle），O(1) | ① fleet: 3 输入 + 继承链 → O(depth) | `mode.Effective` 当前无缓存 → 每次调用重算 |
| **架构验证** | 静态 8 check，<1s | ⑤ drift: runtime 验证 + 持续测量 → 异步周期任务 | 当前 `arch-check` 同步调用阻塞 workflow |

**关键结论**：当前架构不是"有缺陷"，而是**设计承载力方向与 5 方向的需求方向不同**——当前优化的是"单项目正确性"，5 方向需要的是"多项目可扩展性"。这个转变是架构风格（style）的转变，不是修补。

### 1.2 关键设计决策的合理性

我选取三个做重新评估——分析方式不是"对/错"，而是"在什么假设下成立/不成立"。

**决策 1：trace 采用 append-only JSONL 而非结构化存储**

| 分类 | 内容 |
|---|---|
| **原始决策假设** | trace 是审计日志，主要写入、极少读取；每次 run 覆盖前次 trace |
| **决策时的预期负载** | 单项目、单次 run、<1000 events |
| **事实变了吗？** | 变了。方向② replay 要求频繁读取；方向⑤ drift 要求持续聚合统计；方向① fleet 要求跨项目读取 |
| **重新评估** | v0 决策正确（最大 simplicity），但**当前负载已超出该决策的舒适区**。不是"该换数据库"，而是"该加读优化层"（见第 3 节） |

**决策 2：`mode.Effective(mode, lifecycle)` 为纯函数 + 2 参数**

| 分类 | 内容 |
|---|---|
| **原始决策假设** | 策略是全局的、运行时不改变；调用者只需要知道"当前模式" |
| **决策时的预期负载** | 一次 run 中调用 ≤10 次 |
| **事实变了吗？** | 变了。方向① fleet 引入层级策略后，"策略"变成动态的、可继承的、运行时可能变更的 |
| **重新评估** | 2 参数签名仍是正确的核心抽象，但**需要增加一个"策略上下文"参数**（可以是 `nil` 表示使用默认值）。不能扩参数到 3 个；应该用接口注入（见第 3 节） |

**决策 3：所有包零外部依赖**

| 分类 | 内容 |
|---|---|
| **原始决策假设** | 依赖管理的复杂度 > 手写实现成本 |
| **决策时的预期负载** | 18 个包，每个 200-500 行 |
| **事实变了吗？** | 部分变了。包的规模翻倍后，手写 YAML/HTTP/JSON 流解析的维护成本变成**净负值** |
| **重新评估** | 零依赖纪律仍然有价值，但**允许个别经过严格筛选的纯 Go 零传递依赖包**（如 `gopkg.in/yaml.v3`、`modernc.org/sqlite`）——见第 4 节技术选型 |

### 1.3 架构债务

已有分析正确识别了"事件模型扁平化"和"阶段定义侵入"两个债务。我补充两个**未曾在任何文档中讨论的债务**：

**债务 A（新增）：`internal/asset` 的 Workflow 定义与 `internal/mode` 的模式定义之间有隐含耦合**

```
internal/asset.Phase 定义了 lifecycle_stage (design/review/build/evolve)
internal/mode.Mode 定义了 mode×lifecycle 的允许组合
```

两者共享"lifecycle"的概念，但**各自的 `lifecycle` 定义是字符串枚举，没有共享类型**。`asset.Phase.LifecycleStage` 是 `string`，`mode.Lifecycle` 也是 `string`。如果有人在 `asset` 中新增一个 lifecycle stage 但 `mode` 中不更新——没有编译时保护。

**影响**：方向② replay 需要精确重建"当时有效的策略配置"，如果 asset 和 mode 对 lifecycle 的理解不同步，仿真结果可能不准确。

**修复成本**：低。提取 `internal/core/lifecycle.go` 共享 `Lifecycle` 类型（约 20 行 + 测试）。

**债务 B（新增）：`internal/risk.Level` 的优先级模型（Critical/High/Medium/Low）与 mode 的策略模型之间没有正式的交互契约**

```
risk.Level(risk.Classify(changedPaths)) → 影响 routing decision
mode.Mode + lifecycle → 影响 routing decision
```

两者分别决定"该多小心"和"该多严格"，但它们产生冲突时（例如：risk=Critical 但 mode=explorer，explorer 默认不开启所有 gate），**没有明确的优先级规则**——当前是 `orchestrator.go` 中隐式的 if-else 顺序决定。

**影响**：方向① fleet 的策略继承需要明确定义"fleet 全局策略 vs 项目级 risk"的优先级。如果这个优先级不先定义，跨项目策略下推的语义就不确定。

**修复成本**：中。需要一个设计文档（ADR-0006）明确定义四个策略输入（mode/lifecycle/risk/fleet policy）的优先级顺序。

---

## 2. 扩展方向（更深层分析）

已有分析对 5 个方向已有充分分解。我在此为每个方向补充**架构变更成本估算**和**两个实现路径的权衡**。

### 方向①：multi-project fleet orchestration

**架构变更成本**：

| 项 | 估算（agent-sprints） | 说明 |
|---|---|---|
| 核心类型 `Fleet`, `FleetMember` | 0.5 | 纯新包，无既有依赖 |
| `PolicyOverride` 三输入扩展 | 0.5-1 | 需要改 `mode.Effective` 签名或加接口适配器 |
| `forge fleet` CLI 子命令 | 1 | 至少 4 个子命令（list/policy/status/aggregate） |
| 跨项目 telemetry 聚合 | 1-1.5 | 依赖 TraceStore 接口（如未预提取则成本翻倍） |
| 继承优先级模型设计 | 0.5（文档） | ADR + 实现 |
| **合计** | **3.5-4.5** | 若 Phase 0 预提取 TraceStore 接口可节约 0.5-1 |

**两个路径的权衡**：

| 维度 | 路径 A：先建独立 fleet 控制面 | 路径 B：先扩展 `forge run` 支持多项目 |
|---|---|---|
| **实现方式** | `internal/fleet` 新包 + `forge fleet` CLI，与现有 `forge run` 独立 | 在 `forge run --project=PATH` 中增加 `--all` 或 `--selector` 参数 |
| **优点** | 完全隔离，不影响既有单项目工作流 | 更快交付（复用现有 run 管线），用户无需学新命令 |
| **缺点** | 用户需要学一个新子命令树；前期零交付感 | 策略继承需要改既有 `Effective` 签名，影响现有调用点 |
| **适合阶段** | v1（产品化阶段） | v0（快速验证阶段） |
| **架构建议** | **推荐 v0 走路径 B**，v1 再拆独立控制面 | 路径 B 的改动是可逆的 |

### 方向②：replay & simulation sandbox

**架构变更成本**：

| 项 | 估算（agent-sprints） | 说明 |
|---|---|---|
| `internal/sim` 核心引擎 | 1-1.5 | 重放 trace + 预测路由决策 |
| `Event.Detail` 策略指纹（v0） | 0.3 | 不改 schema |
| 结构化 DecisionContext 字段（v1） | 0.5 | 需扩展 trace Event 结构 + 迁移旧 JSONL |
| `forge simulate` CLI | 0.5 | 输出表格 |
| CLI UX 设计（表格/对比报告） | 0.5 | 被低估的工作量 |
| **合计** | **2.8-3.3** | 取决于是否先做 TraceStore 接口 |

**两个路径的权衡**：

| 维度 | 路径 A：v0 利用 Detail 字段快速出 MVP | 路径 B：先扩展 trace schema 再做仿真 |
|---|---|---|
| **实现方式** | `Event.Detail` JSON 指纹 → `sim.Simulate()` 对比 | 先加 `DecisionContext` 结构化字段 → 迁移旧数据 → 仿真 |
| **优点** | 2-3 天可见 `forge simulate` 输出 | 长期数据质量高，不受 Detail 150 字限制 |
| **缺点** | Detail 字段被滥用（非设计用途），schema 版本管理弱 | 前期投入大（加字段 + 迁移），2 周看不到用户交付 |
| **架构建议** | **强烈推荐路径 A**（与已有分析一致） | 路径 B 作为 v1 计划，但不用阻塞 v0 |

### 方向③：knowledge mining & cross-session learning

**架构变更成本**：

| 项 | 估算（agent-sprints） | 说明 |
|---|---|---|
| `internal/learn` PatternMiner | 1-1.5 | 统计频率 + 趋势 + 简单相关 |
| `forge learn` CLI | 0.5 | 输出报告 |
| `Supersedes` 去重消费者 | 0.3 | 写入 `MemoryEntry` + 验证 |
| correlate 引擎（trace × scorecard） | 1-1.5 | 跨数据源关联，需要 TraceStoreReader 接口 |
| **合计** | **2.8-3.8** | correlate 依赖方向②的前置条件 |

**核心挑战**：不是技术难度，而是**信号质量**。模式挖掘的典型问题是 90% 的输出是噪音。如果 v0 版本输出 5 条"发现"但 4 条是误报，用户信任度会被破坏。

**缓解策略**：
1. v0 只输出**频率趋势**（"topic X 出现频率 +20%"），不做语义推断（"说明 X 问题严重"）
2. 结果标注 `confidence: "statistical"` 且附上原始数据引用
3. 不做自动 action（如自动改策略）——保持"建议→人审"最高杠杆

### 方向④：graduated self-healing

**架构变更成本**：

| 项 | 估算（agent-sprints） | 说明 |
|---|---|---|
| `internal/heal` RemediationPlan 类型 | 0.5 | 策略链注册表 |
| Tier-2 模型升级实现 | 0.5-1 | `phaseTierResolver` 的"尝试下一 tier"逻辑 |
| Tier-3 角色升级实现 | 0.5 | 重解析 Phase 配置 |
| Tier-4 prompt 增强 | 0.3 | 复用 `phaseOutputLedger` |
| 安全阀（recursion limit × 升级阶梯解耦） | 0.3 | 区分 agent 深度 vs 升级阶梯深度 |
| **合计** | **2.1-2.6** | 不含 Tier-5（已放弃） |

**已有分析 vs 我的判断**：
- 已有分析：建议 Tier 3-5 全部放弃
- **我的判断**：Tier-3 和 Tier-4 可以在 v1 零架构变更做（配置重解析 + 现有 ledger 复用）
- **分歧原因**：已有分析假设 Tier 3-4 需要 mid-flight phase mutation，实际上不需要——它们只是"重试时换参数"
- **但我不主张在 v0 做**——不是因为架构限制，而是因为**行为可预测性**：Tier 3-4 的失败模式（升级角色后 gateway 配置不匹配、prompt 增强后 token 暴增）尚未被充分研究

### 方向⑤：runtime architecture drift detection

**架构变更成本**：

| 项 | 估算（agent-sprints） | 说明 |
|---|---|---|
| struct diff drift detector（v0） | 0.5 | `arch-check --json` 输出比较 |
| `contracts.yaml` 格式设计 | 0.3（文档） | 机器可读架构声明格式 |
| latency budget verifier（v1） | 1-1.5 | P99 滑动窗口 + trace 数据积累 |
| 与 `forge evolve` scan phase 集成 | 0.5 | 输出 `drift-report.md` |
| **合计** | **2.3-2.8** | 前提：trace 数据已有 |

**关键决策点**：先做哪个 detector？

| 选项 | 前置依赖 | 交付时间 | 业务价值 | 建议 |
|---|---|---|---|---|
| struct diff | 无（arch-check 已产出 JSON） | 0.5 sprint | 低（静态已有 arch-check）但可做技术验证 | ✅ **v0 立即做** |
| latency budget | 需 >3 trace 文件 + DurationMs 字段 | 1-1.5 sprint（数据积累+代码） | 高（第一行真的运行时偏离） | ⚠️ **v1** |
| API contract | contracts.yaml 格式 + 幂等性插桩 | 2+ sprint | 高（安全合规） | ❌ **v2** |

---

## 3. 接口设计建议

### 3.1 三组核心接口的设计原则

已有分析提出了 TraceStore / Policy / HealingStrategy 三个接口契约。我在此基础上补充**设计约束条件**：

**原则 1：接口属于消费者，不属于生产者**

```go
// 错误做法：internal/trace 定义一个大的 TraceStorer 接口
// 正确做法：internal/sim 定义它需要的 Reader，internal/fleet 定义它需要的 Reader
// 不同消费者可以有不同的 Reader 接口

// internal/sim/sim.go
type TraceReader interface {
    GetTrace(id string) (*Trace, error)
}

// internal/fleet/telemetry.go
type TraceSummarizer interface {
    ListTraces(opts TraceFilter) ([]TraceSummary, error)
}
```

**为什么**：生产方（`internal/trace`）如果定义"完整"接口，容易过度设计（加入未来可能需要的所有方法）。消费方定义自己需要的接口，自然实现最小依赖。

**原则 2：只读接口和写接口分离**

```go
type TraceReader interface { ... }  // 方向②⑤①的共同依赖
type TraceWriter interface { ... }  // 方向②（仿真写结果）和现有 run 管线的共享
```

**为什么**：方向② replay 不应该有写权限——它只能读历史 trace，不能修改。如果 replay 的 `TraceReader` 嵌入了 `WriteEvent` 方法，它就有了非预期的能力。Go 的接口分离让这成为编译时保证。

**原则 3：所有接口应该可以在 5 分钟内用 mock 实现**

如果接口定义有 >5 个方法，或者参数类型是包外不可见的内部类型，那么它不能用于单元测试。

### 3.2 具体接口提案——替代 "DecisionContext 结构体" 的两种方案

已有分析提出了 `ReadonlyDecisionContext` 接口。我提供两种替代方案（不是更好，而是不同权衡）：

**方案 A：接口（已有分析建议）**

```go
type ReadonlyDecisionContext interface {
    Mode() mode.Mode
    Lifecycle() mode.Lifecycle
    Risk() risk.Level
    GateSet() []string
}
```

| 优点 | 缺点 |
|---|---|
| 扩展灵活，加方法不破坏既有实现 | 值语义被打破（接口是指针语义） |
| 可 mock，可替换 | 每次调用都虚拟方法调用（微开销） |
| 天然不可变（接口无 setter） | 不能用在 map key 或 == 比较中 |

**方案 B：值类型 + 构建器模式（我的推荐）**

```go
type DecisionContext struct {
    mode      mode.Mode      // 小值类型
    lifecycle mode.Lifecycle // 小值类型
    risk      risk.Level     // uint8
    gates     gateSet        // 内部封装
    // 未来扩展通过 embedded 策略集
    fleetPolicy atomic.Value // 懒加载，不可变 snapshot
}

func NewDecisionContext(m mode.Mode, l lifecycle.Lifecycle) DecisionContext { ... }
func (d DecisionContext) WithFleetPolicy(p FleetPolicy) DecisionContext { ... }
```

| 优点 | 缺点 |
|---|---|
| 值语义，可做 map key，可 == 比较 | 扩展时需要改结构体（但 With 方法保持 backward compat） |
| 零间接（无接口的虚拟调用开销） | 不能 mock（但 DecisionContext 是数据类，不应 mock） |
| 调试友好（可直接打印所有字段） | embedding 策略集可能被滥用 |

**我的建议**：场景决定。如果 DecisionContext 被频繁传递（方向② replay 可能每秒解析数百个决策点），值类型方案 B 更优。如果 DecisionContext 主要在 CLI/API 边界使用（方向① fleet 的策略匹配），接口方案 A 更优。**我推荐两阶段：v0 用方案 B（简单值类型），v1 需要 mock 测试时提取接口**——Go 的接口是隐式满足的，从值类型提取接口是零成本重构。

### 3.3 向后兼容的检查清单

在任何接口扩展时，必须通过以下检查：

```
□ 旧调用点是否需要改代码？— 目标：零改动
□ 旧数据文件是否能被新代码读取？— 目标：无需迁移脚本
□ 新数据的默认值是否与旧行为一致？— 目标：新字段=零值=旧行为
□ 接口的新方法是否提供默认实现（Go 1.18+ 的接口默认方法）？— 目标：旧实现无需新增方法
□ 新的 CLI 子命令/参数是否与现有子命令/参数互斥或兼容？— 目标：不改变既有命令行为
```

这个检查清单应该放入 `forge accept` 的验证流程（作为 `gate.mjs` 中的一个规则）。

---

## 4. 技术选型

### 4.1 需要重新审视的四个技术决策

已有分析提出了三个假设（零外部依赖、JSONL 足够、统计方法足够、Go 侧实现）。我在此基础上深入**替代方案的全面分析**。

**决策 1：trace 存储引擎**

| 选项 | 优点 | 缺点 | 成本 |
|---|---|---|---|
| **当前：纯 JSONL 文件** | 零依赖、对人类可读、可用 grep/sed 分析 | 无法索引、无并发保护、无 schema 版本 | 0 |
| **JSONL + 索引文件**（如 `.forge/trace.idx`） | 保留 JSONL 的可读性，增加按 traceID 的 O(1) 查找 | 索引文件需与 JSONL 同步；索引构建有开销 | ~100 行索引逻辑 |
| **bbolt（嵌入式 Key-Value）** | 纯 Go（零 CGO）、事务性、简单 | 不可 grep、数据目录增长不可控 | ~300 行适配器 |
| **modernc.org/sqlite（纯 Go SQLite）** | 可查询（SELECT）、CGO-free、事务性 | 重量级、编译慢、runtime OOM 风险 | ~500 行适配器 |
| **Badger（LSM-tree KV）** | 写优化、适合大量 trace 写入 | CGO 依赖、包大（~50MB） | ❌ 违反零依赖 |

**我的建议**：**v0 保留 JSONL，但加一个轻量索引文件**。索引文件格式为 `map[traceID]offset` 的 JSON（读时 mmap，写时 append 后重建）。这是"维持 JSONL 的简单性 + 获取 O(1) 查找"的最小成本路径。

**决策 2：YAML 解析器**

| 选项 | 类型 | CGO | 传递依赖 | 稳定性 | 建议 |
|---|---|---|---|---|---|
| **手写解析器（当前）** | 自建 | 无 | 0 | ⚠️ 已出现 2 次 bug | ❌ 已有架构债务 |
| **gopkg.in/yaml.v3** | 纯 Go | 无 | 0 | ✅ Go 团队推荐，3+ 年稳定 | ✅ **推荐** |
| **goccy/go-yaml** | 纯 Go | 无 | 0 | ⚠️ 较新（2022），API 不稳定 | ❌ v1 后考虑 |
| **ghodss/yaml** | 纯 Go | 无 | 0 | ⚠️ 维护模式（archived） | ❌ |

**决策依据框架**（适用于所有第三方依赖评估）：

```
┌────────────────────────────────────────────────────┐
│  1. 是标准库实现不了的？            → 是            │
│  2. 是纯 Go，零 CGO？              → 是 (yaml.v3)  │
│  3. 是零传递依赖？                  → 是            │
│  4. 有 ≥3 年的实际生产使用？       → 是 (2018-)    │
│  5. 社区活跃（非 archived）？       → 是            │
│  6. API 稳定（非 v0.x）？          → 是 (v3)       │
│  7. 不会引入 runtime 不确定行为？   → 是            │
└────────────────────────────────────────────────────┘
    通过 ≥6/7 → 可依赖 | 通过 ≤3/7 → 不可依赖
```

根据此框架，`gopkg.in/yaml.v3` 通过 7/7，**可依赖**。`goccy/go-yaml` 通过 5/7（API 不稳定 + 社区较新），**建议等待**。

**决策 3：方向③的"学习"是否需要 ML/NLP 依赖**

| 选项 | 能力 | 复杂度 | 可解释性 | 建议 |
|---|---|---|---|---|
| **纯统计（BM25 + 频率 + 相关系数）** | 模式识别、趋势检测 | 低 | ✅ 完全可解释 | ✅ **v0** |
| **统计 + TF-IDF 加权** | 主题聚类 | 低-中 | ✅ 词级可解释 | ✅ **v0-v1** |
| **统计 + embedding（text-embedding-3-small 等）** | 语义聚类 | 中（需外部 API） | ❌ 黑箱 | ❌ v2+ |
| **小模型微调（LoRA）** | 项目特定模式 | 高（训练集 + MLOps） | ❌ 黑箱 | ❌ 不适合 |

**关键约束**：ForgeOS 的"数据飞轮"产生的数据量（每次 evolve ~50-200 条，每周可能 1-2 次）不足以支撑任何需要训练的 ML 方法。**统计方法不是限制，而是诚实的选择**。

**决策 4：方向① fleet 的项目间通信方式**

| 选项 | 优点 | 缺点 | 建议 |
|---|---|---|---|
| **共享文件系统（NFS/git submodule）** | 零依赖，复用现有 fs 操作 | 跨主机不适用、性能瓶颈 | ✅ **v0（单机场景）** |
| **本地 HTTP/gRPC（fleet daemon 监听 localhost）** | 进程隔离，独立生命周期 | 需管理 daemon 进程、端口冲突 | ⚠️ **v1** |
| **Kubernetes CRD + Operator** | 符合 K8s 生态，可扩展 | 太重，违反零依赖 | ❌ 北极星阶段 |

### 4.2 自建 vs 采购/复用

在 5 方向的上下文中，这个决策主要出现在方向③（learn）中：

| 功能 | 自建成本 | 第三方方案 | 建议 |
|---|---|---|---|
| 模式挖掘（频率/趋势） | ~1.5 sprints | —（无可用的第三方库实现 ForgeOS 特定模式） | ✅ 自建 |
| 语义聚类（v2+） | ~3 sprints | 调用外部 LLM API（如 `claude-sonnet-4-20260507` summarize） | ✅ **复用 LLM**，不自建 embedding |
| 审计仪表盘（v1 fleet） | ~3 sprints | Grafana + SQLite 后端 | ✅ **复用 Grafana**，不自建 |
| 策略变更审批工作流 | ~2 sprints | —（需定制） | ✅ 自建（GitOps PR 模式） |

---

## 5. 实施路线图

### 5.1 优先级排序

基于**对"ForgeOS 从个人工具跃升为平台产品"的杠杆率**进行排序：

| 优先级 | 方向 | 杠杆率判断 | 原因 |
|---|---|---|---|
| **P0** | ② replay & simulation | 最高 | 它是 ForgeOS 自身安全迭代的基础设施；没有它，改策略 = 烧钱验证 |
| **P0** | ⑤ struct diff drift（v0 subset） | 高 | 0.5 sprint，零前置依赖，验证方向⑤的可行性 |
| **P1** | ① fleet orchestration | 高 | 产品跃升关键，但范围大，需要分步 |
| **P1** | ④ self-healing T1-2 | 中-高 | 24h 自治的可靠性前提，但范围可控制 |
| **P2** | ③ knowledge mining | 中 | 价值高但依赖数据积累，可等 2-3 月后再做 |
| **P3** | ⑤ latency budget + API contract | 低 | 前置条件多（trace 数据量 + YAML 解析器） |

**与已有分析的差异**：已有分析将方向②和①并列为 P1。我倾向于将方向②升级为 **P0**——因为②是①的前置条件（fleet telemetry 聚合需要可靠的 trace 读取，而这正是②要解决的）。如果把①先做、②后做，①的 telemetry 聚合会因为缺乏 trace 索引而性能受限。

### 5.2 阶段划分

```
Sprint A（Weeks 1-2）— 基础设施
├─ 方向② v0: Detail 策略指纹 + sim 引擎 + forge simulate（MVP 输出对比表）
├─ 方向⑤ v0: struct diff drift detector（复用的 arch-check JSON 输出）
├─ Phase 0: TraceStore 接口提取 + memory.Supersedes 验证
└─ 护栏: 文件互斥 + 循环依赖 CI guard

Sprint B（Weeks 3-5）— 核心交付
├─ 方向④ v0: 自愈 T1-2（重试 + 模型升级）+ 安全阀
├─ 方向② v1: DecisionContext 结构化字段 + 仿真结果与真实 trace 的对齐验证
└─ 方向① v0: 路径 B（扩展 forge run 支持多项目 + PolicyOverride）

Sprint C（Weeks 6-8）— 产品化
├─ 方向① v1: forge fleet CLI + ADR-0005 策略继承模型
├─ 方向③ v0: PatternMiner + forge learn（频率趋势 + correlate 表）
├─ 方向④ v1: 自愈 T3-4（角色升级 + prompt 增强）
└─ 方向⑤ v1: latency budget verifier（P99 滑动窗口）

Sprint D（Weeks 9-11）— 深化
├─ 方向① v2: PolicyResolver 三层继承 + fleet policy set 跨项目下推
├─ 方向③ v1: Supersedes 去重 + 跨会话知识压缩
├─ 方向② v2: 仿真引擎升级为结构化 DecisionContext 驱动
└─ forge accept 全方向回归
```

### 5.3 风险与缓解

已有分析覆盖了：model escalation budget burn、replay credibility、pattern miner noise、反馈放大、不完整项目目录、显式模型不可升级。我补充三个**架构层面的风险**：

**风险 1：方向②和方向④的交互——自愈升级后的 trace 数据包含自愈操作，导致 replay 失真**

如果一次 run 中触发了模型升级（方向④ T2），trace 中会记录"先在 Sonnet 上 failed，后在 Opus 上 succeeded"。方向② replay 仿真"如果在 engineering 模式运行会怎样"时，如果 replay 没有区分"原始失败"和"自愈后的重试"，可能会错误地认为"Sonnet 也跑成了"。

**严重性**：中-高。**缓解**：trace Event 中新增一个 `retry_of_seq` 字段（指向原始失败事件的 sequence number）。replay 引擎遇到带 `retry_of_seq` 的事件时，跳过原始失败事件，只重放眼原始失败。或者反过来——replay 可以选择"不模拟自愈"（只使用原始失败事件），以评估"如果没有自愈会怎样"。

**风险 2：方向① fleet 的策略继承链可能引入不可见的策略冲突**

```
Fleet 级策略: gate_set = [arch, security, test, performance]
Project 级策略（override）: gate_set = [arch, security]
```

如果 merge 策略是"项目级覆盖 fleet 级"，那么如上的配置会导致 performance gate 被意外禁用。但如果 merge 策略是"fleet 级扩展项目级"，那么 performance gate 被强制加入但项目没有对应的 gate script——导致 `forge validate` 报错。

**严重性**：高（安全合规风险）。**缓解**：引入**策略冲突检测**——在 `PolicyResolver` 中做三件事：
1. Merge 后的策略集必须是**合法组合**（所有 gate 有对应实现）
2. Conflict 时输出**可解释的差异报告**（"Fleet 策略要求 gate=performance，但 project 未实现"）
3. 不支持"项目降低 fleet 安全水位"——`mode.Effective` 中 fleet 策略设置最低安全水位，项目只能提升不能降低

**风险 3：5 方向并行推进时，`internal/` 包的边界污染**

5 方向新增 5 个包（`fleet`, `sim`, `learn`, `heal`, `drift`）。如果它们之间出现**不必要的双向引用**（例如 `sim` 引用了 `heal` 的升级策略类型、`heal` 引用了 `drift` 的检测结果），循环依赖会迅速爆发。

**严重性**：中。**缓解**：在 Phase 0 的 CI guard 中，加入 `internal/` 包的**层级依赖约束**：

```
internal/
├── fleet (可引用: trace, mode, routing, converge, asset, risk)
├── sim   (可引用: trace, routing, mode, converge)
├── learn (可引用: memory, trace)
├── heal  (可引用: exec_error, backoff, asset, prompt_context)
├── drift (可引用: trace, converge, arch-check 快照)
└── core/ (共享类型: Lifecycle, EventKind 等)
```

约束规则：
- 无包可引用 `cmd/forge/`
- `sim` 不能引用 `heal`
- `fleet` 不能引用 `sim` 或 `learn`
- `drift` 不能引用 `heal`
- 所有包可以向 `core` 方向引用，不能反向

这个约束可以用 `go mod verify` 的自定义脚本（或 `golang.org/x/tools/go/analysis` 的 lint 规则）实现。

---

## 6. 总结：五个方向的架构本质

最后，我想将 5 方向抽象为**更本质的架构术语**，帮助理解它们对 ForgeOS 整体架构的意义：

| 方向 | 架构本质 | 一句话描述 |
|---|---|---|
| **① fleet** | **控制面缩放**（Control Plane Scaling） | 从单机单项目 → 多项目多租户的控制面架构 |
| **② replay** | **确定性地回放**（Deterministic Replay） | 给非确定性 LLM 系统加上确定性测试基础设施 |
| **③ knowledge** | **跨会话推理**（Cross-Session Inference） | 从无状态事件流 → 有状态知识积累 |
| **④ healing** | **分级恢复**（Graceful Degradation） | 从二元 retry/abort → 多级自治恢复 |
| **⑤ drift** | **运行时契约验证**（Runtime Contract Checking） | 从静态结构验证 → 动态行为验证 |

这五个方向的共同本质是：**ForgeOS 从"一个人使用的脚本编排器"向"一个平台运行的操作系统"演化的架构路径**。方向①是控制面缩放，方向②是测试基础设施，方向③是状态管理，方向④是可靠性与自治，方向⑤是治理深度。五者组合起来，就是 ForgeOS 的"OS-like"能力集。

---

以上分析可供下一阶段 ADR 编写、技术选型评审、Sprint 规划参考。如果您希望对其中某个方向展开更详细的 ADR 草案或技术方案，请告知。
