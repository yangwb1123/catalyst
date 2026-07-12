---

# 架构全景分析：ForgeOS 五个结构性扩展方向

> **分析基准**：`docs/requirements/2026-07-11-five-structural-extension-directions-architect-pm-combined.md`
> **交叉参考**：`docs/analysis/2026-07-12-tech-lead-analysis.md`（一组不同的方向，但提供了互补的视角）
> **角色**：系统架构师
> **日期**：2026-07-12

---

## 1. 架构评估

### 1.1 当前架构的核心优势

ForgeOS 当前的架构设计有几个清醒且坚定的决策值得肯定：

| 优势 | 体现在 | 架构意义 |
|------|--------|---------|
| **零外部依赖策略** | `go.mod` 无 `require` | 供应链攻击面为零；构建可复现；适当时不需要依赖审计 |
| **控制面与执行面分离** | forge-core（Go）↔ harness（Node/Python） | 执法层可独立演进而不会引入运行时回归 |
| **可观测性基础设施前置** | `.forge/trace.jsonl` schema 在设计阶段就包含 `DurationMs`、`CostUsdMicros`、`Kind`、`Status` | 数据底座就绪——本题分析的多数方向之所以可行，是因为这块地基已经铺好了 |
| **基于 scorecard 的闭环信号** | `HistoryTiebreak` 消费历史质量信号做同档择优 | 证明团队理解「路由不是静态分配，而是持续优化」 |
| **架构执法的工具化** | `arch-check` 的 8 项检查 + `gate.mjs` + `check.py` | 架构规范有人执法，不是写在文档里的纸面规定 |

### 1.2 关键设计决策的合理性评估

**决策 1：风险分类器用路径启发式而非内容分析**

> `internal/risk/risk_diff.go:15-20` — "it reads only the basename and directory prefix of each path, never the file CONTENT"

这是 **合理的有意简化**。在 v1 阶段，路径启发式提供了 80% 的价值（支付模块的改动 90% 在 `payment/` 目录下）而成本极低。但该决策在代码库规模增长、目录结构与职责不再一一对应时，回报递减。当前（~32K LOC、18 包）已经进入了这个递减区间。这不再是一个设计缺陷——而是**一个需要重新审视的到期决策**。

**决策 2：路由阈值硬编码为 const**

> `routing.go:37-40` — `HaikuMax = 0.34`, `SonnetMax = 0.69`

这是 **合理的初始默认**，但现在是 **架构债务**。`const` 的选择意味着「这些值永远不会变」——编译器保证了这一点。如果要让它们可变，必须改为 `var` 或引入配置层。这不是一个简单的常数提取重构，而是语义变更：从「编译期确定」到「运行时可调」。

**决策 3：Agent 输出用 ad-hoc 字符串解析而非结构化协议**

> `cost.go:330-410` — `parseReviewerVerdict`, `parseConfidenceScore`, `parseClaudeCostUsd`

这是 **最值得质疑的设计决策**。在 agent 卡（YAML frontmatter）已经有 `emits:` 声明的情况下，选择用 `strings.HasSuffix` 和 `strconv.Atoi` 提取结构化数据，而没有定义明确的 schema，是一个架构短路。原因可能是 pragmatism（快速出 MVP），但三个 load-bearing 路径（verdict、confidence、cost）全都用这种方式，说明这不是"临时方案"而是"默认方案"。这是一个需要结构性纠正的架构债。

**决策 4：trace 数据只写不读**

> `.forge/trace.jsonl` 记录了丰富的结构化事件，但没有任何消费者

这在 v1 是合理的（"先记录，后分析"），但持续了多个 sprint 后，变成了一个 **信息熵损失** 的模式——数据在积累，但洞察在蒸发。评分卡系统（Scorecard）是当前唯一的聚合消费者，但它只读模型性能数据，不读运行健康数据。

### 1.3 架构债务清单

| 债务 | 严重程度 | 首次引入 | 影响范围 |
|------|---------|---------|---------|
| 路由阈值硬编码为 `const` | 🟡 中 | v1 | 模型路由效率持续偏离最优 |
| 风险分类只有路径启发式 | 🟠 高 | v1 | 安全下限 Opus 守卫等待一个不会来的信号 |
| Agent 输出无合约验证 | 🔴 严重 | v1 | Sprint 27 已验证的静默降级 bug，真实下游影响 |
| trace 数据只写不读 | 🟡 中 | v1 | 运维盲区——系统在黑暗中运行 |
| 维度评分声明 6 维实仅 1 维有信号 | 🟠 高 | policy.yml v1 | 路由系统对用户做出虚假的维度承诺 |
| YAML 解析无交叉验证 | 🟡 中 | Sprint 27 | 双解析器差异可能已引入隐蔽的 workflow 语义漂移 |
| AgentExecutor 无生命周期 | 🟢 低 | v1 | 当前无触发条件，但限制了未来扩展（daemon 模式） |
| Emits 声明是纯文档不执行 | 🟠 高 | v1 | 下游阶段对上游产出格式无结构性保障 |

---

## 2. 扩展方向

结合文档的 5 个方向和 tech-lead 的 5 个方向，我从架构层面识别出 **4 个高价值的架构扩展方向**，它们以不同的方式覆盖了原文档的发现，并补充了缺失的视角。

### 方向 A：契约化 phase 间接口（Contract-First Phase Interface）

> 覆盖：方向五（Agent 产出合约验证）+ tech-lead 方向三（Agent 输出契约验证）

#### 为什么需要

当前架构中 phase 间数据传输是 **隐式、无类型、无契约** 的。Phase A 写入文件，Phase B 读取文件——但 B 对文件格式的依赖从未显式声明。这带来了两个问题：

1. **系统内脆弱性**：A 的输出格式轻微变化 → B 的解析器静默退化为默认值 → 下游决策基于错误数据
2. **演化阻力**：要修改 A 的输出格式来引入新能力，必须人工检查所有下游 B 能否兼容——无自动化辅助

当前已经暴露的三个解析器（`parseReviewerVerdict`, `parseConfidenceScore`, `parseClaudeCostUsd`）只是冰山一角。每个 agent 卡中 `emits:` 的每个产出物，理论上都可能被下游依赖。

#### 核心挑战

- **合约的粒度**：应该细到 JSON Schema（强类型）还是粗到"Markdown 文件必须包含 `## Decision` 标题"（结构断言）？前者精确但昂贵，后者便宜但模糊。
- **版本兼容**：当 agent 卡升级后 `emits` 格式变化，旧 run 的 trace 数据和产出物与新合约可能不匹配。
- **解析器 vs 合约的关系**：如果合约定义了"verdict 必须是 APPROVE/REQUEST_CHANGES/REJECT 之一"，当前 ad-hoc 解析器的 exact-match 应该被合约驱动生成，而不是手写。

#### 建议的架构变更

```
当前：
  agent output (raw text) → ad-hoc parser → structured value
                                ↓
                      strings.Contains fallback
                                ↓
                         default value (silent)

建议：
  agent output (raw text) → contract validator → structured value
                                ↓
                    [pass] → exact match (from schema)
                    [fail-hard] → error + trace event
                    [fail-soft] → relaxed match + trace.warning
```

关键设计决策：**合约定义在 agent 卡 frontmatter 中**（而非单独文件），使用轻量级 schema（YAML 内嵌断言或 JSON Schema 子集）。新 agent 卡创建时自带模板。

#### 对现有系统的影响

- `asset.Phase` 新增 `Contract` 字段（optional, schema 引用）
- `cost.go` 的三个解析器重构为合约驱动的生成解析器（输入是 schema + 原始文本 → 输出结构化值）
- 现有 `emits:` 声明保持兼容——无合约的 phase 行为不变（fallback 到当前 ad-hoc 解析）
- `forge validate --contracts` 新增验证通道

#### 与 tech-lead 方案的关系

Tech-lead 的 TASK-001~004 是可行的工程实现路径。但架构层面的建议是：**合约 schema 应定义在 agent 卡 frontmatter 中，不单独成文件**。这降低采用壁垒，让合约成为 agent 卡定义的一等公民，而非附加的治理文档。

---

### 方向 B：从反应式预算到主动式资源管理

> 覆盖：方向一（路由阈值自校准）+ 方向二（预测性运行估算）

#### 为什么需要

当前 ForgeOS 的资源管理（模型选型、预算分配）完全是 **反应式** 的：

- 路由：基于 18 个月前设定的静态阈值
- 预算：只有硬上限 `--run-budget-usd`
- 成本：只有 run 完成后的后验计数

架构上，这是 **开环控制 vs 闭环控制** 的区别。当前是开环——预设阈值 → 运行 → 观察结果（但不回灌）。目标是闭环——历史 + 当前 → 预测 → 调整阈值 → 运行 → 观察 → 回灌校准。

这两个方向（阈值自校准 + 预测估算）共享同一个数据管道瓶颈：**Scorecard 的聚合粒度**。

#### 核心挑战

**数据粒度的结构性限制**：

当前 `Scorecard` 的 key 是 `(model, task_type)`。但阈值校准需要的粒度是 `(model, task_type, complexity_bucket)` 或 `(model, task_type, mode)`。预测需要的粒度是 `(mode, lifecycle, workflow)`。

这意味着在触及这两个方向的业务逻辑之前，必须先解决数据模型的问题。否则：
- 阈值校准的输入有噪声（同一个 model 在简单任务和复杂任务上的表现不同，但被聚合在一起）
- 预测的输入过于粗略（按 mode 聚合时简单与复杂 workflow 平均在一起）

**冷启动**：

一个新仓库的 scorecard 是空的。新仓库的 trace 是空的。阈值校准和预测在冷仓中的第一次运行都是盲跑。

架构建议：**ForgeOS 内置一份从自身开发仓库聚合的通用基线**，在新仓库无本地历史时作为 fallback。这不是一个 hack——它反映了 ForgeOS 自身是其设计目标的第一用户的事实。

#### 建议的架构变更

```
Scorecard 聚合粒度扩展：
  当前键空间: (model, task_type)
  建议键空间: (model, task_type, mode) + 可选 complexity_bucket

阈值管理提取为独立的 Calibrator：
  当前: const HaikuMax, SonnetMax (hardcoded in routing.go)
  建议: Calibrator 接口 + BuiltinCalibrator 实现
        calibrator.CalibratedThresholds{ HaikuMax, SonnetMax }
        从 scorecard 数据驱动，min_samples 守卫

预测引擎：
  当前: 无
  建议: Predictor 接口 + HistoryPredictor 实现
        输入: (mode, lifecycle, workflow_name)
        输出: EstimatedRun{ Cost, Duration, Iterations, Confidence }
        冷启动: 内置基线 + 按 (mode, lifecycle) 的行业经验值
```

关键设计决策：**Calibrator 和 Predictor 应该是只读顾问（advisory），不直接修改运行时的路由阈值**。v1 只输出报告和建议，v2 引入自动调整开关。这防止了校准噪声污染路由决策。

#### 对现有系统的影响

- `routing.go` 的 `const` 改为 `var` 或注入 `CalibratedThresholds`（**这是侵入性改动，需要仔细处理向后兼容**）
- `Scorecard` 结构体新增维度字段（非破坏性扩展）
- 新增 `internal/predict/` 包（零依赖，纯数据聚合）
- `forge run --dry-run` 新增预测输出（CLI 层不变）
- 内置基线编译进二进制（~几百字节的 JSON 数据）

---

### 方向 C：静态分析驱动的多维评分基础设施

> 覆盖：方向三（静态分析风险提取）

#### 为什么需要

这是技术层面**最紧迫**的架构扩展。当前 `policy.yml` 声明了 6 维评分：

```yaml
scoring:
  dimensions:
    - name: complexity
      weight: 0.25
    - name: dependency_change
      weight: 0.25
    - name: security
      weight: 0.15
    - name: risk
      weight: 0.25
    - name: context_size
      weight: 0.05
    - name: business_impact
      weight: 0.05
```

但运行时的 `Score()` 函数实际只从 `risk` 维度得到真实信号（来自路径启发式 `FromChangedPaths`），其余 5 维全部硬编码为 0.5。这意味着：

1. **路由的排序权重偏差**：`complexity` 权重 0.25 和 `context_size` 权重 0.05 在输出结果上无区别——因为它们都输出 0.5，乘以任何权重都不变。
2. **安全下限的虚假保障**：`risk=critical` → Opus 硬下限。但 `risk` 来自路径启发式，不读代码内容。改动 payment 包里的注释也触发 Opus（浪费钱）；在 `utils/` 包中新增支付核心逻辑不会触发 Opus（安全风险）。

#### 核心挑战

- **多语言支持**：ForgeOS 的仓库是多语言的（Go frontend、Python harness、TypeScript）。一个单一的内容分析框架需要语言感知。这意味着要么引入一个多语言 AST 库（增加依赖），要么为每种语言写特定的正则/模式分析器（增加维护成本）。
- **分析时机**：静态分析必须在 gate 阶段执行，但 `forge evolve` 中 agent 写的代码在被 gate 检查前没有持久化 diff。时间窗口极窄。
- **深度 vs 速度权衡**：全文件 AST 分析（~100ms/文件）在 CI 场景可行，在本地实时交互场景不可行。需要可配置的分析深度。

#### 建议的架构变更

```
当前:
  FromChangedPaths(paths) → RiskScore { 路径启发式降级 }

建议:
  ContentAnalyzer {
    // 可组合的分析器链
    analyzers: []Analyzer{
      &PathHeuristic{},       // v1 的路径启发式（保留，向后兼容）
      &ImportPatternDetector{}, // v1.5: import 路径检测
      &CyclomaticComplexity{},  // v2: 圈复杂度（需 harness adapter）
      &FunctionLevelDiff{},     // v3: 函数级 diff 分析
      &DependencyGraph{},       // v3: 跨模块依赖拓扑变化
    }
  }

  每个 Analyzer 产出:
  {
    dimension: string    // "complexity" | "risk" | "dependency_change" | ...
    score:     float64   // [0, 1]
    evidence:  string    // 人类可读的决策理由
    depth:     AnalyzerDepth // HEURISTIC | PATTERN | AST_FULL
  }
```

关键设计决策：**signal production function（信号生产函数）与 scoring dimension（评分维度）解耦**。一个维度可以有多个分析器贡献信号；一个分析器可以贡献多个维度的信号。`policy.yml` 只声明权重和 each 维度的最低信号要求（`min_signal_depth: PATTERN`），不固定信号来源。

#### 对现有系统的影响

- `risk_diff.go` 的 `FromChangedPaths` 保留为 `PathHeuristic` 分析器，保持完全向后兼容
- 新增 `internal/content/` 包（零外部依赖，仅使用标准库的 `go/parser` 和正则）
- `policy.yml` 扩展 `scoring.signals` 段，允许指定每个维度的 `min_signal_depth`
- `routing.Score()` 的调用者从硬编码的 `0.5` 改为从 ContentAnalyzer 获取信号
- 没有现有功能被破坏——当前路径启发式作为默认 Analyzer 链的第一个元素

#### 与 tech-lead 方向的关系

Tech-lead 的交叉验证建议 v1 只做「Go 文件的 `import` 路径检测」，这是正确的精简策略——它复用 `arch-check` 已有的 `extractJsImports` 模式，~50 行代码，立即提升路由准确率。我赞同这个裁剪，但架构设计上应为未来的 Analyzer 扩展保留插入点，避免 v2 时再次重构。

---

### 方向 D：运维智能与自治循环

> 覆盖：方向四（跨运行失效分类）+ tech-lead 方向二（AgentExecutor 生命周期真空）+ 方向一（Gate loop-back 重跑）

#### 为什么需要

三个独立的方向汇聚为一个更高阶的架构需求：**ForgeOS 需要能够观察自身并自动响应**。

- trace 数据只写不读（方向四观察到）
- executor 没有生命周期管理（tech-lead 方向二观察到）
- loop-back 时无条件重跑所有 gate（tech-lead 方向一观察到）

这三者共享一个根因：**系统的运行时没有「自我感知」抽象层**。当前架构把运行时看作一个一次性执行器（run → exit），而不是一个有状态、可观察、可自愈的服务。

#### 核心挑战

- **运行边界标识**：trace 事件没有 run_id 或 session_id 标签。两份连续 `forge evolve` 的 trace 行在同一文件中无法区分。这是所有 trace 消费的前提条件，必须先修复。
- **数据生命周期**：`.forge/trace.jsonl` 无旋转/归档。跨运行分析需要一个稳定的数据管理机制。
- **告警阈值**：statistical anomaly detection（如 3σ）需要在冷启动阶段有足够的基线数据。

#### 建议的架构变更

```
Tracer 增强：
  当前: { Seq, Kind, Status, DurationMs, CostUsdMicros, Model }
  建议: + RunID (UUID, 每次 forge evolve 生成)
        + PhaseIdx (phase 在 workflow 中的索引)
        + Iteration (loop-back 计数器)

Trace 文件管理：
  当前: .forge/trace.jsonl (无限增长)
  建议: .forge/trace/     (目录)
        active.jsonl      (当前运行)
        archived/         (按日期旋转的 gzip 归档)

Executor 生命周期契约：
  当前: { Execute }
  建议: { Init, Execute, Shutdown, Rollback, Health }
  Rollback 在 phase 失败时调用（反向顺序清理已成功完成的 phase 的产物）
  Health 在每个 Execute 前调用（检查磁盘空间、临时目录可写）

RunStatus 抽象：
  当前: 无运行级状态跟踪
  建议: 每个 run 从 Init() 开始记录状态到 .forge/run.json
        包含 { RunID, Status, Phases[], StartTime, HealthEvents[] }
        loop-back 时增量更新而非覆盖
```

#### 对现有系统的影响

- `Tracer` 新增 `RunID` 字段（非破坏性扩展，现有 JSONL 行无此字段时 seq 作为 fallback 标识符）
- `.forge/trace/` 目录结构变更（需要迁移逻辑处理旧 `trace.jsonl`）
- `AgentExecutor` 接口扩展（影响 `DryRunExecutor` 和未来的 `CommandExecutor`）
- `Engine.RunFrom` 生命周期调用链调整（Init → for-loop → Shutdown）
- 没有向后兼容问题——所有新增都是 optional，旧行为保持不变

---

## 3. 接口设计建议

### 3.1 核心原则

| 原则 | 理由 | 具体体现 |
|------|------|---------|
| **接口最小化** | 避免过度抽象——v1/v2 需要的是"够用"而非"完美"的接口 | `Calibrator` 接口只暴露 `Calibrate(Scorecard) → Thresholds`，不暴露训练/推理分离 |
| **只读优先（advisory-first）** | 自动调整路由阈值可能引入灾难，校准器先出报告再出行为 | `Calibrator.Calibrate()` 返回 `Report + RecommendedThresholds`，不修改全局状态 |
| **渐进式 fallback** | 解析/分析/预测都不应破坏正常运行流程 | 每一层 fallback 都有明确的置信度标注，消费者选择是否使用 |
| **先测后改** | route 的 `const` 改 `var` 前必须有 golden-file 锁住当前行为 | 所有解析器重构前必须用 fixture 锁定当前输出 |

### 3.2 新增核心接口

#### `internal/calibrator/calibrator.go`

```go
// Calibrator 读取历史数据，输出阈值建议。
// v1: 只读，不修改路由状态。
type Calibrator interface {
    // Calibrate 基于 Scorecard 数据计算推荐阈值。
    // 返回的 Report 包含当前阈值 vs 推荐阈值的对比 + 样本量信息。
    Calibrate(ctx context.Context, sc *Scorecard) (*CalibrationReport, error)
}

type CalibrationReport struct {
    CurrentThresholds  TierThresholds
    RecommendedThresholds TierThresholds
    SampleCount        int
    MinSamplesRequired int
    Dimensions         []DimensionReport  // 每个维度的区分度分析
}
```

#### `internal/predict/predictor.go`

```go
// Predictor 基于历史数据预测一次运行的资源消耗。
// v1: 仅 CLI 输出，不影响运行时。
type Predictor interface {
    Predict(ctx context.Context, input PredictionInput) (*Prediction, error)
}

type PredictionInput struct {
    Mode       string
    Lifecycle  string
    Workflow   string
    ChangedFiles int
}

type Prediction struct {
    TotalCostUsd     EstimatedValue
    TotalDurationMs  EstimatedValue
    ExpectedIterations EstimatedValue
    Confidence       PredictionConfidence  // HIGH / MEDIUM / LOW / COLD_START
    Breakdown        []PhasePrediction      // 每个 phase 的预测
}
```

#### `internal/content/analyzer.go`

```go
// Analyzer 从文件 diff 中提取一个维度的评分信号。
type Analyzer interface {
    // Analyze 对变更文件进行分析，输出指定维度的评分。
    Analyze(ctx context.Context, diff *FileDiff) (*Signal, error)
    // Dimension 返回此 Analyzer 生产的维度名称。
    Dimension() string
    // Depth 返回此 Analyzer 的分析深度。
    Depth() AnalyzerDepth
}

type Signal struct {
    Dimension string        // "complexity"
    Score     float64       // [0, 1]
    Evidence  string        // "cyclomatic complexity of payment.go:45 increased from 3 to 8"
    Depth     AnalyzerDepth // PATTERN
}
```

#### `internal/trace/lifecycle.go`（扩展）

```go
// RunBoundary 标记一次完整运行的开始和结束。
type RunBoundary struct {
    RunID    string    `json:"run_id"`
    Action   string    `json:"action"` // "START" | "END"
    Mode     string    `json:"mode"`
    Workflow string    `json:"workflow"`
}
```

这个简单的结构解决了 tech-lead 提到的阻塞点——trace 事件没有运行边界标识。写入一个 `RunBoundary{Action:"START"}` 事件即可分割不同运行的 trace 行。

### 3.3 保持向后兼容性的策略

| 变更 | 兼容策略 |
|------|---------|
| `routing.go` 的 `const` 改为 `var` | 初始值与当前 const 相同；`Calibrator` 不启用时行为完全不变 |
| `Tracer` 新增 `RunID` 字段 | JSONL 序列化 optional；旧行在读取时 `RunID` 为空字符串 |
| `AgentExecutor` 新增生命周期方法 | 默认实现（no-op）嵌入 `DryRunExecutor`；调用者检查接口断言 `if lc, ok := exec.(LifecycleExecutor); ok { ... }` |
| `Scorecard` 新增维度字段 | Go struct 新增字段 zero value 兼容；旧数据读入时维度字段为 0 |
| `.forge/trace.jsonl` → `.forge/trace/` 目录 | 迁移逻辑：如果文件存在则读全部，如果是目录则读 active.jsonl |
| 合约 schema 引入 | 无合约的 phase 行为完全不变（当前 ad-hoc 解析器作为 fallback 保留） |

---

## 4. 技术选型

### 4.1 核心原则

**forge-core 零外部依赖的铁律不应打破**。这是该项目的核心竞争力之一——一个 Go 运行时可以在任何环境中编译和部署，无需 `go mod download`、无需 CGO、无需运行时依赖。这条红线约束了所有的技术选型。

但区分 forge-core 和非 core 组件（如 harness）的依赖政策是必要的。核心运行时内置的分析器用标准库 `go/parser` 和 `regexp`；非核心的 harness adapter 可以有选择地引入离线分析工具（如 `gocyclo`、`lizard`）。

### 4.2 是否需要引入新技术栈

| 方向 | 所需能力 | 建议方案 | 依赖影响 |
|------|---------|---------|---------|
| 内容分析 - Go AST | 函数级 diff、import 分析 | `go/parser` + `go/ast`（标准库） | ✅ 零依赖 |
| 内容分析 - 圈复杂度 | 函数复杂度计算 | v1: `internal/content` 自实现 Walk；v2: harness adapter 对接 `gocyclo` | ✅ v1 零依赖；v2 引入外部工具但运行在 harness 层 |
| 合约 schema 定义 | 轻量级断言式 schema | 自定义 YAML-based schema DSL（~10 个断言类型） | ✅ 零依赖，YAML 已有 `yaml2json` 包 |
| 预测引擎 | 简单统计聚合（均值/中位数/p90） | 自实现统计函数（~50 行） | ✅ 零依赖 |
| 趋势分析 | anomaly detection | 3σ 阈值 + 移动均值（自实现~80 行） | ✅ 零依赖 |
| trace 归档 | gzip 压缩 | `compress/gzip`（标准库） | ✅ 零依赖 |

**结论**：在当前规划的 v1/v2 范围内，**不需要引入任何新的外部依赖**。所有需要的数学计算（均值、中位数、百分位数、标准差）和文件格式（YAML、JSON、gzip）都在 Go 标准库的范围内。这是 forge-core 零外部依赖策略的一次验证。

### 4.3 自建 vs 采购/集成的决策框架

| 决策 | 倾向 | 理由 |
|------|------|------|
| 模型成本预测 | **自建** | 领域特定（基于 ForgeOS 的 mode/lifecycle/workflow 分桶），无现成 SaaS 可做 |
| 圈复杂度计算 | **集成外部工具**（v2） | 不 reinvent the wheel；`gocyclo`/`lizard` 已成熟；通过 harness adapter 集成而非直接引入核心 |
| YAML 解析 | **使用现有**（已存在 Go yaml2json） | 交叉验证需要，不替换已有解析器，只添加验证层 |
| 趋势告警 | **自建** | 逻辑简单（3σ / 移动均值），无必要引入外部监控系统 |
| 合约 schema | **自建 DSL** | 与 agent 卡 frontmatter 深度绑定；JSON Schema 太重（不适用于 "markdown 必须包含某标题" 这类断言） |

### 4.4 第三方依赖评估标准（未来需要时的准则）

| 标准 | 最低要求 | 红线 |
|------|---------|------|
| 许可证 | MIT / Apache 2.0 / BSD | ❌ AGPL / SSPL / BUSL |
| go.mod 的传递依赖 | 0 或 1 层 | ❌ > 3 层传递依赖 |
| CGO | 无 CGO | ❌ 任何 CGO |
| 最近维护 | 过去 12 个月有发布 | ❌ 超过 2 年未更新 |
| 代码质量 | 有测试、有 lint | ❌ 无测试或覆盖率 < 50% |
| 二进制大小 | 增加 < 500KB | ❌ > 5MB |

---

## 5. 实施路线图

### 5.1 优先级综合排序

综合原文档的 P0/P1/P2 分级和 tech-lead 的交叉验证，我的架构优先级排序如下：

| 优先级 | 方向 | 理由 |
|--------|------|------|
| **P0** | **方向 C：静态分析驱动的多维评分基础设施**（原方向三） | 安全下限 Opus 守卫当前是空壳；维度虚假承诺是路由系统的架构性不诚实 |
| **P0** | **方向 A：契约化 phase 间接口**（原方向五 + tech-lead 方向三） | Sprint 27 已有真实 bug；三个 load-bearing 解析器的静默降级已在产生实际影响 |
| **P1** | **方向 D·v1：运行边界标识**（阻塞点解除） | Tracer 增加 RunID + RunBoundary；这不是一个方向，而是所有 trace 消费方向的先决条件 |
| **P1** | **方向 B：从反应式预算到主动式管理**（原方向一 + 二） | 阈值校准和预测共享数据管道瓶颈 Scorecard 的粒度修复；可以并行做 v1 |
| **P2** | **方向 D·v2：运维智能循环**（原方向四 + tech-lead 方向一 + 二） | trace 消费 + executor 生命周期 + gate 缓存；依赖方向 D·v1 的基础 |

### 5.2 三个阶段

#### 阶段 1（当前 Sprint）— 根基加固

| 项目 | 估算 | 依赖 |
|------|------|------|
| **P0** 方向 C v1: Go import 路径内容嗅探 | ~50 行，~1 天 | 无 |
| **P0** 方向 A v1: agent 卡 frontmatter 合约 schema + 验证器 | ~200 行，~3 天 | 无 |
| **P1** 方向 D·v1: Tracer 增加 RunID + RunBoundary 事件 | ~30 行，~0.5 天 | 无 |
| 技术债: `routing.go` 的 `const` 改为 `var`（仅声明改动，无行为变化） | ~5 行，< 1 小时 | 无 |

**里程碑 M1**："两 P0 一 P1 一债"完成，`forge accept: ACCEPTED`

#### 阶段 2（下一 Sprint）— 能力建设

| 项目 | 估算 | 依赖 |
|------|------|------|
| **P1** 方向 B v1: Scorecard 聚合粒度扩展 `(model, task_type, mode)` | ~100 行，~1 天 | 阶段 1 完成 |
| **P1** 方向 B v1: `forge run --dry-run` 预测报告 | ~250 行，~3 天 | Scorecard 扩展 |
| **P1** 方向 B v1: `forge scorecard --calibrate` 阈值建议 | ~150 行，~2 天 | Scorecard 扩展 |
| **P2** 方向 D·v2: `forge trace --summary` | ~220 行，~2 天 | 阶段 1 RunID |
| **P0** 方向 A v2: `contract_check` gate 插入 phase 间 | ~100 行，~1.5 天 | 阶段 1 合约 schema |

**里程碑 M2**：全部五个方向 v1 功能可用，进入 `forge accept`

#### 阶段 3（未来 Sprint）— 进化与自治

| 项目 | 估算 | 依赖 |
|------|------|------|
| **P2** tech-lead 方向一: Gate loop-back 增量缓存 | ~150 行，~3 天 | 阶段 2 |
| **P2** tech-lead 方向二: AgentExecutor 生命周期 | ~200 行，~3 天 | 阶段 2 |
| **P1** 方向 B v2: 阈值自动调整（有 min_samples 守卫） | ~100 行，~2 天 | 阶段 2 calibration report |
| **P2** 方向 D·v3: trace rotate + archive 机制 | ~200 行，~3 天 | 阶段 2 |
| **P0** 方向 C v2: 圈复杂度接入（harness adapter） | ~150 行，~2 天 | 阶段 1 ContentAnalyzer 框架 |
| **P0** 方向 A v3: 合约学习（失败 N 次自动强调 prompt） | ~80 行，~1 天 | 阶段 2 contract_check |

### 5.3 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Scorecard 粒度扩展后发现历史数据不兼容 | 🟡 中 | 阈值校准推迟 1 sprint | 兼容策略：旧数据以 mode="" 填充，Calibrator 感知数据缺失度 |
| Tracer RunID 字段引入后，旧 trace 文件无法关联运行边界 | 🟢 低 | trace summary 仅统计到新数据 | 旧文件保持原样；新数据从第一个含 RunID 的行开始统计 |
| 合约 schema DSL 设计过复杂 | 🟡 中 | 采用壁垒高 | v1 只支持 3 种断言：`field_exists`、`field_equals`、`field_range`；其它延后 |
| Path heuristic 和 import analyzer 对同一个文件输出冲突信号 | 🟢 低 | 路由行为不确定 | ContentAnalyzer 按 `max(heuristic, import)` 聚合——更保守的信号胜出 |
| 零外部依赖策略使 v2 圈复杂度接入困难 | 🟡 中 | 需要自实现 Walk | 自实现 `internal/analysis/cyclo.go` 基于 `go/ast` 的复杂度 Walk（~100 行） |

### 5.4 不做清单

每个方向提案都需要明确「不做什么」来控制范围蔓延：

| 方向 | 明确不包含（v1/v2 范围外） |
|------|--------------------------|
| 方向 A | 不引入外部 JSON Schema 库；不做跨语言合约验证；不做合约版本化自动迁移 |
| 方向 B | 不使用 ML 模型做预测（纯统计聚合）；不做场景模拟；不做动态预算重分配（v2 仍只出报告） |
| 方向 C | 不做全文件 AST 分析（v1）；不做跨语言统一分析框架；不做被删代码的风险降级检测 |
| 方向 D | 不做分布式 trace 收集；不做远程告警后端；不做趋势自动响应（只出报告） |

---

## 6. 综合结论

这份文档 + tech-lead 交叉验证的组合，揭示了 ForgeOS 当前架构的一个深层模式：**早期 v1 的 pragmatism 决策已跨过回报递减的临界点**。

路径启发式风险分类、ad-hoc 字符串解析、硬编码路由阈值、trace 只写不读——这些都是 v1 的合理选型。但它们同时失效，说明系统已经成长到了需要结构性升级的阶段。这不是任何一个方向的失败，而是一个架构 maturity transition 的征兆：

> ForgeOS 正在从「**能够运行**」（run-oriented）向「**能够管理自身**」（observe-learn-adapt）进化。

这五个方向——尤其是方向 C（静态分析风险提取）和方向 A（合约验证）——不是增量的功能添加，而是这个进化转型的架构基础设施。当前 sprint 的 P0 选型（方向三 + 五）是正确的：它们修复的是 **系统的诚信问题**（routing 说自己是多维的但其实不是；agent 输出了错误格式但系统不报告），然后才是 **系统的智能问题**（阈值自校准、预测、模式识别）。

建议立刻执行的三个优先事项：

1. **Tracer 加 RunID**（~30 行，0.5 天）—— 解锁所有 trace 消费方向
2. **合约 schema 进入 agent 卡 frontmatter**（~200 行，3 天）—— 开始修复系统诚信
3. **Go import 路径内容嗅探**（~50 行，1 天）—— 以最小成本升级风险分类器

这三件事无依赖冲突，可并行推进。完成后，ForgeOS 的架构底座就准备好了承接后续四个方向的增量演化。

---

*以上分析基于文档提供的 5 个方向 + tech-lead 交叉验证的 5 个方向，以及 forge-core ~32K LOC 的架构理解。不包含任何实现代码，仅提供架构层面的设计和建议。*
