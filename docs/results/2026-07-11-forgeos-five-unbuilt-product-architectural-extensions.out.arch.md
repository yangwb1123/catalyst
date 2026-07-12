现在我已全面了解项目上下文——包括 `.agent/` 下的全部架构文档、全部 Sprint 历史（31 个 sprint 的详尽记录）、两个分析文档（五个结构扩展方向 + 五个系统性缺口分析），以及有效的代码库结构。以下是我的架构分析。

---

# ForgeOS 架构分析：基于 Five Structural Extension Directions 审计反馈

## 1. 架构评估

### 优势

**1.1 执行纪律与架构自省的耦合强度是该项目最突出的架构属性。**

ForgeOS 已经构建了一个自我执行的架构治理循环——其中 `harness`（gate.mjs / arch-check.mjs / check.py / secret-scan.mjs）既规定了代码库规范（函数 ≤ 50 行、循环依赖 = 0、分层规则），又由 `forge accept` 施加聚合阻断门控。该循环有其自身的反馈回路，由 `CURRENT_SPRINT.md` 中的「架构自我纠正」片段证明——作者检测到 arch-check 关于 fan-in 的误报（将测试文件计入耦合），并在进一步 root-caused 和修复的情况下进行修复。这类元级架构智能在大多数组织中并不存在。

**1.2 带外执行（运行无关）是多年来 Kubernetes 控制平面设计选择之中一个被低估的架构洞察。**

通过强制执行严格分离——控制平面 API（`forge-core` Go 运行时）将治理委托给 shell-out 到 `harness/*.mjs`（Node.js 进程）——ForgeOS 确保闸门结果无法被受管制的代理程序操纵。Sprint 24-26 的真实点火运行已实证证明：即使 `claude` 代理程序以 `acceptEdits` 运行且完全不受限制，闸门裁决仍由独立的 Node.js 进程产生，该进程与正在被评估的 Go 运行时的主机无关。这是一个安全关键的架构决策，是正确的选择。

**1.3 模块化演进：从 CLI-over-Claude 到自研 Go 运行时，零破坏性过渡。**

v0→v1→v2 的路径——在未重写系统的前提下，从纯粹的声明式文件加上 Claude Code 原生编排，演进到完整的自研 Go 运行时——是增量架构迁移的典范。`architecture/north-star.md` 定义了目标分布式目标，而当前代码库则忠实地实现了该路线图的子集，没有过度设计或金矿。

### 局限性

**1.4 进程生命周期管理存在一个架构空白，从 Sprint 27 中至今尚未完全解决。**

`main.go` 注册 `signal.Notify` 是由 `docs/ROADMAP.md`（方向 1：韧性运行时）定义的，但 `internal/orchestrator/command_executor.go` 仍然会产生孤儿孙进程——这些孙进程通过 `setsid()` 脱离进程组——已由审计报告确认。`WaitDelay = 2s` 可减轻管道阻塞，但不能减轻端口 / 文件锁 / 内存泄漏。这是一个持续存在的架构债务，会损害核心价值主张「24h 无人值守自治」。

**架构影响**：如果 ForgeOS 要运行 24 小时，在启动时无法清理泄漏的子进程，在足够多的迭代后会消耗所有 PID、文件描述符和内存。其核心前提（自治运行）及其架构现实（遗漏的进程组清理）之间存在不匹配。

**1.5 `cmd/forge` 包的重复分解模式表明存在边界问题。**

`cmd/forge` 文件上限（`package.max_files`）已被多次破坏（Sprint 27、29、30），每次都是通过从 CLI 层提取纯逻辑转移到新的 `internal/` 包来解决的（`internal/doctor`、`internal/attribution`、`internal/gate/resolve.go`）。这种模式本身是健康的——它证明了架构纪律正在发挥作用——但这也意味着 `cmd/forge` 仍然承载了太多不属于 CLI 编排层的逻辑。

**诊断**：`cmd/forge/cost.go`（471 行）包含：
- `parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`——协议解析器
- `observeFor`——prompt 构建逻辑
- 成本核算和 claude-JSON 解析逻辑

这些跨越三个架构层（解析、prompt 构建、遥测）。正确的归属应是：`internal/contract/` 用于协议解析器，`internal/telemetry/` 用于成本消耗。

**1.6 「方向 0」Phase Contract 抽象缺失（由跨方向评审文档正确识别）。**

`asset.Phase` 定义了名称、代理、Emits——但它没有定义「此阶段的输入是什么、成功标准是什么、副作用是什么」。`converge.Signals` 拥有 `RoadmapCompletion` 和 `GatesGreen`，但没有「此阶段是否完成其承诺输出」的概念。Phase Contract 抽象在架构文档中无处不在（方向 1 的工作流组合、方向 3 的冲突检测、方向 4 的产物验证），但它并未作为代码中的一阶构造存在。

### 架构债务

1.  **孤儿进程组泄漏**——`setsid()` 逃逸未处理（中等影响，低修复成本）。
2.  **`cmd/forge` 层的横切关注点**——`cost.go` 承载协议解析 + prompt 构建 + 遥测（高影响，中等修复成本）。
3.  **跨存储关联缺失**——`checkpoint` / `trace` / `memory` 均没有 `RunID`——正向因果关系不可追溯（高影响，低修复成本）。Sprint 29 修复了 `converge.Signals`，但这个特定的关联缺口仍然存在。
4.  **`.forge/` 生命周期不完整**——`DefaultCompactThreshold` 确实被消费了（如审计反馈正确指出的那样），但 trace 的自动清理完全缺失，且 trace 文件旋转（10MB 滚动）会创建 `trace.jsonl.1`，而该文件从未被清理（中等影响，低修复成本）。

---

## 2. 扩展方向

我从审计反馈和交叉验证中分析了五个方向，外加一个我发现的基础方向。每个方向都包含业务价值、核心挑战和架构影响。

---

### 方向 0（基础）· Phase Contract 抽象

**为什么需要**：当前 `asset.Phase` 结构体拥有名称、代理、Emits——但没有合同化的输入/输出契约。五个扩展方向中的三个（工作流组合、冲突检测、产物验证）都依赖于此抽象。如果没有它，每个方向都会重复发明「这个阶段承诺了什么」的概念，并在不同的方向中产生不兼容的实现。

**核心挑战**：
- 合同必须是类型安全的，但不能为每阶段使用做类型膨胀（一个 `discover` 阶段的合同与一个 `implementer` 阶段的合同具有不同的结构）。
- 向后兼容性：现有的 YAML 工作流文件（5 个）必须在不重写的情况下原地使用。
- 合同验证必须容忍合理的格式差异（来自 `cost.go` 的 `VERDICT:` 协议解析是一个教训——严格匹配是脆弱的）。

**预期架构变更**：
```
asset.Phase  + ContractRef string  // path to .contract.yaml in .agent/contracts/
             + Inputs     []ContractInput
             + Outputs    []ContractOutput

contract.Contract — 新包 internal/contract/
  Validate(phase *asset.Phase, outputs map[string][]byte) Result
  InferEmits(phase *asset.Phase) []string    // 从合同派生 emits，而非硬编码
```

下一个问题如 `check.py` 将在 `forge validate` 期间检查合同引用是否解析（如同对代理/角色卡所做的那样）。

**对现有系统的影响**：低。所有当前工作流将继续使用零合同正常运行——`ContractRef == ""` → 跳过验证。这是自愿选择的严格性，而非强制性的。

---

### 方向 1 · 路由阈值自校准引擎

**为什么需要**：来自审计反馈的交叉验证确认 `HaikuMax = 0.34` 和 `SonnetMax = 0.69` 是硬编码常量。Scorecard 的数据结构包含 `QualityScore`、`PassRate` 和 `AvgIterations`，但只用于 `HistoryTiebreak`（同档最优），从未用于校准档位边界。当前系统无法处理模型性能漂移（例如，Sonnet-4 变得和旧的 Opus 一样好，但仍在阈值上承担成本损失）或项目特定漂移（1000 个 ADR 使 `context_size` 膨胀，进而将一切向上推档）。

**核心挑战**：
- 信号稀疏性：如果一个项目每天只跑 5 次任务，则需要数周才能获得足够的数据用于校准。
- 对抗性环路：如果校准降低了阈值，预算变得紧张，且回馈使阈值进一步降低——需要存在的防振荡阻尼。
- 新模型冷启动：用 Opus-5 替换 Opus-4 需要重置该校准的历史数据。

**预期架构变更**：
```
internal/routing/
  calibrate.go — 新文件
    CalibratedThresholds { SonnetMax, HaikuMax, MinSamples, DriftWindow }
    Recalibrate(scorecard map[Key]ScorecardPair) CalibratedThresholds
    WithDeadband(current, suggested Thresholds, band float64) Thresholds  // 阻尼

routing.go
    BandForScore 读取 calib 而非常量
    const 降级为默认值（值 = fallback）
```

**对现有系统的影响**：低。v1 只生成建议（`forge route --calibrate`），不改变运行时行为。v2 使校准具有副作用。

---

### 方向 2 · 预测性运行估算引擎

**为什么需要**：ForgeOS 拥有反应性预算护栏（`budget.go`、`--max-agent-calls`、`--run-budget-usd`），但零预测能力。在 `forge evolve` 之前，操作员无法知道「这要花多少钱？」或「这要跑多久？」。`preflight.go` 确实提供了基于硬编码的每阶段成本（Sonnet 0.08 美元，Opus 0.35 美元）的成本估算——但正如交叉验证所指出的，这并非「零覆盖」；它只是不使用已就位的真实评分卡数据。

**核心挑战**：
- 冷启动：首次运行的仓库没有历史数据。预测必须回退到来自模式 × 生命周期 × 工作流的聚合基线。
- 离群值剔除：一次评级者 API 故障（导致 10 次重试）不应使后续预测产生偏差。需要使用百分位数而非均值。
- 模型转换：当路由切换到新模型时，旧数据对于新模型来说价值为零。`model` 维度必须作为分桶键的一部分。

**预期架构变更**：
```
forge-core/cmd/forge/
  predict.go — 新命令  forge predict <workflow>
    读取 .forge/trace.jsonl + .agent/routing/scorecards.json
    聚合 (workflow, model, mode, lifecycle)
    输出预期成本 / 持续时间 / 迭代次数的预测区间

internal/predict/
  predictor.go — 新包
    Predict(workflow, mode, lifecycle) Estimate
    WithConfidence(data []TraceEvent) Estimate  // p50 / p90 / p95
  
internal/persist/
  checkpoint.go + RunID 字段（支持跨运行聚合）
```

**对现有系统的影响**：低。纯附加命令；不与现有行为交互。v1 只有 `forge predict` CLI 命令。

---

### 方向 3 · 静态分析驱动的风险提取

**为什么需要**：审计反馈确认了 `internal/risk/risk_diff.go` 是一个纯路径启发的分类器——它只做 `strings.Contains(basename, "payment")`，零文件内容读取。`routing.Score()` 的 `complexity`、`dependency_change`、`context_size` 和 `business_impact` 维度都硬编码为 0.5——没有信号来源。虚构风险下限（`risk=critical → Opus`）是空的；它在等待一个永远不会以当前形式到来的输入。

**核心挑战**：
- 多语言：AST 需要语言感知。Go vs TypeScript vs Python vs Rust 都有非常不同的 AST 结构。
- 生成代码：`forge evolve` 中代理编写的代码在被 gate 检查之前不会有持久的 diff。风险分析必须在代理完成到 gate 运行之间运行。
- 删除很困难：删除支付代码的 PR 应该降低风险，但路径启发式贪婪地看到 `payment/` 并标记为 risky。

**预期架构变更**：
```
internal/risk/
  risk_diff.go — 从路径 → 路径 + 轻量级内容嗅探扩展
    sniffContent(path string) FeatureVector
    detectImports(content []byte) []string      // "import payment/v2"
    detectCriticalFuncs(content []byte) []string // "func.*[Pp]rocess[Pp]ayment"
    
internal/routing/
  scoring.go — Score() 新信号来源
    computeComplexity(changedPaths) float64     // 从 gocyclo radon 适配器
    computeDependencyDelta(changedPaths) float64 // lockfile diff
```

**对现有系统的影响**：中等。v1 轻量级内容嗅探不会破坏任何东西，因为当前风险提取完全缺失。v2 AST 集成需要一个 harness 适配器（例如 `lizard` 或 `gocyclo`），这可能会增加 CI 依赖项。

---

### 方向 4 · 跨运行失效模式分类

**为什么需要**：`trace.Event` 拥有丰富的字段（Kind、Status、DurationMs、CostUsdMicros、Model），但全库消费它们的唯一地方是 scorecard（在模型 × 任务类型层面）。没有人回答「过去一周最常见的失效模式是什么？」或「哪些工作流阶段的失败率在上升？」。`doctor` 包只做单快照健康检查。

**核心挑战**：
- Trace 文件旋转：`trace.jsonl` 每 10MB 旋转。跨运行分析需要一个稳定的 trace 归档，而非仅基于位置的文件。
- 运行边界：由于缺少 `RunID` 或 `SessionID`，来自不同 evolve 运行的 trace 事件仅通过 `seq=1` 重置来区分。恢复运行边界需要启发式（>N 秒无事件 → 新运行）。
- 隐私：Trace 包含阶段名称和门裁决等可能敏感的字段。跨运行聚合需要匿名化或访问控制。

**预期架构变更**：
```
cmd/forge/
  trace.go — 新子命令 forge trace
    --summary: 按 (Kind, Status) 聚合计数 + 按 (model, Status) 的失败率
    --trend:   ┊最近 N 次运行的失败率趋势

internal/trace/
  archive.go — 新文件
    Rotate(root string) error
    ArchiveReader(path string) ([]Event, error)  // 读取活跃 .jsonl + .jsonl.N

internal/doctor/
  anomaly.go — 扩展
    FailureTrend(traces []Event, window time.Duration) TrendReport
```

**对现有系统的影响**：低。v1 `forge trace --summary` 是一个纯本地、零依赖的聚合——只是一个仅追加的命令，就像 `forge predict` 一样。

---

### 方向 5 · 代理产出合同验证框架

**为什么需要**：交叉验证确认 `cost.go` 中 `VERDICT:` / `CONFIDENCE:` 的字符串协议解析是脆弱的（Sprint 27 确认为 bug——YAML 代码块内的 `VERDICT:` 被静默丢弃）。每个工作流阶段的 `emits:` 是纯文档；没有架构验证输出产物。当前系统覆盖 L0（退出码）到 L1（尾词令牌），但缺少 L2（产物存在）到 L3（产物结构）。

**核心挑战**：
- 合同版本化：代理卡升级后，产出格式发生变化。旧阶段的新代理版本可能会生成旧格式。`schema_version` 字段是必要的。
- 部分合规性：一个阶段产生 5 个 sprint 条目，其中 3 个合法、2 个字段缺失——应该拒绝全部还是部分接受？
- 多语言代理：Claude 的输出格式可能与 Gemini 不同。合同验证需要绑定到提供商/层级。

**预期架构变更**：
```
internal/contract/ — 新包（与方向 0 复用）
  validator.go
    ValidateEmits(phase *asset.Phase, outputDir string) EmitResult
    ParseVerdict(output string) Verdict  // 替换 cost.go 中的 ad-hoc

asset/
  asset.go + Emit 结构体（从字符串 → 结构化引用）
    type Emit struct {
        Name     string
        Path     string   // glob
        Schema   string   // 引用 schema 注册表中的某个条目
        Required bool
    }

internal/gate/
  resolve.go + contractGate(phase *asset.Phase) GateResult  // 在下一个阶段之前作为可选门控运行
```

**对现有系统的影响**：中等。v1（定义架构、`forge validate --contracts` 检查解析）是纯附加的且安全的。v2（`contract_check` 门控）需要工作流 YAML 中的新选项，但默认情况下是禁用的，以保证向后兼容性。

---

## 3. 接口设计建议

### 3.1 Package 分拆原则

当前模式——向 `internal/` 包提取纯逻辑以解决 `cmd/forge` 文件计数问题——工作正常，但应被形式化：

| 当前 `cmd/forge` | 应归属位置 | 理由 |
|---|---|---|
| `cost.go:parseReviewerVerdict` | `internal/contract/parse.go` | 协议解析，非 CLI 逻辑 |
| `cost.go:observeFor` | `internal/telemetry/observe.go` | Telemetry 消耗 |
| `cost.go:parseConfidenceScore` | `internal/contract/parse.go` | 与 Reviewer 相同的协议解析 |
| `gates.go:requirementConfidence` | `internal/converge/signals.go` | 收敛信号，非门控 |
| `prompt_context.go`（454 行） | `internal/prompt/build.go` | Prompt 构建复用 |

**接口契约**：每个 `internal/` 包应导出 1-3 个符号，且应由现有消费点存在性驱动。不应有预建包。

### 3.2 Phase Contract 接口

```
// internal/contract/contract.go
package contract

type PhaseContract struct {
    Name       string           // 人类可读名称
    Emits      []ArtifactSpec   // 阶段承诺产生的产物
    Verdict    *VerdictSpec     // 可选：阶段输出裁决（APPROVE / REJECT / ...）
    SchemaVer  string           // 单向递增
}

type ArtifactSpec struct {
    Path     string   // glob 模式，相对于仓库根目录
    Schema   string   // 架构定义名称
    Required bool
}

// 不带验证的容错 Parse
func ParseVerdict(output string) Verdict  // 替换 cost.go 的 ad-hoc 解析
func ValidateEmits(contract *PhaseContract, outputDir string) EmitResult
```

**设计原理**：`ParseVerdict` 必须容错（大小写不敏感，忽略代码围栏），而 `ValidateEmits` 必须严格（文件存在性 + 架构合规性）。这种不同的严格度反映了一次性解析（由 `forge-core` 在运行时调用）与缓存验证（由 `forge validate` 在构建时调用）之间的差异。

### 3.3 Predict 接口

```
// internal/predict/predictor.go
package predict

type Estimate struct {
    CostUsd         Range    // 最小值、中位数、最大值
    WallClockMin    float64  // 预期底线墙钟时间
    Iterations      Range
    Confidence      float64  // 0.0–1.0（可获得的样本数量）
    ByPhase         []PhaseEstimate  // 每个阶段的细分
}

type Range struct { Min, P50, P95, Max float64 }

// ForgePredict 使用 scorecard + trace 历史数据来预测工作流成本
func ForgePredict(wf *asset.Workflow, mode string, lc string, history []trace.Event) Estimate

// 回退到硬编码默认值（如果 history < MinSamples 个样本）
func BaselineEstimate(wf *asset.Workflow, mode string, lc string) Estimate
```

**设计原理**：`BaselineEstimate` 是 `preflight.go:checkCostEstimate` 中现有硬编码逻辑的继承者。它提供了与当前完全相同的行为，但被提取到一个显式的回退路径中。

### 3.4 向后兼容性契约

所有五个方向必须满足以下兼容性规则：

1.  **零结构再平衡**：现有 YAML 工作流文件（`.agent/workflows/*.yml`）在未显式更新之前必须保持原样解析。
2.  **默认停用**：所有新门控（合同验证、校准阈值调整）默认情况下为 off，直至用户在工作流定义中显式选择加入。
3.  **仅附加命令**：`forge predict`、`forge trace --summary`、`forge route --calibrate` 不得修改任何现有文件。它们只读。
4.  **可修复回退**：`ParseVerdict` 的新实现必须回退到旧的 ad-hoc 逻辑（当前是 `cost.go:273-350`），以确保过渡路径。

---

## 4. 技术选型

### 评估标准

鉴于当前零外部依赖承诺（Go 标准库仅用于 `forge-core`），任何新依赖必须满足：

| 标准 | 权重 | 注释 |
|---|---|---|
| 零运行时依赖 | 必须 | Go 标准库是 `forge-core` 在 `go.mod` 中的唯一内容 |
| 可 vendor | 最好 | 如果依赖是纯 Go，则可 vendor；CGo 则禁止 |
| build 大小影响 | 考虑 | `forge-core` 目前 < 4MB 静态二进制——新依赖不应将其推至 > 8MB |
| 许可证兼容性 | 必须 | 与现有 MIT 兼容 |
| 已灭活 | 考虑 | 获得上游支持且至少存在 1 年以上 |

### 每个方向的技术选型

| 方向 | 推荐方法 | 理由 | 与零依赖契约的偏差 |
|---|---|---|---|
| Phase Contract | 纯 Go 标准库：`encoding/json`、`strings`、`path/filepath` | 无解析需求——合同是 YAML，但由现有的 yaml2json 转码处理 | 零偏差 |
| 路由校准 | 纯 Go 标准库：`sort`、`math`、`container/heap` | 百分位数排名和阻尼是简单的算法 | 零偏差 |
| 预测引擎 | 纯 Go 标准库：`math`、`sort`、`time` | 百分位数统计 + 指数移动平均 | 零偏差 |
| 风险提取 v1 | 纯 Go 标准库：`bufio`、`strings`、`path/filepath` | 轻量级内容嗅探不需要 AST 库 | 零偏差 |
| 风险提取 v2 | Harness 适配器 shell-out：`gocyclo` / `lizard` / `radon` | AST 分析是语言特定的；保持与现有适配器框架一致 | 零偏差（是通过继承运行无关的 shell-out，而非依赖注入） |
| 失效分类 | 纯 Go 标准库：`sort`、`container/heap`、`time` | JSONL 聚合是纯 I/O——无需数据结构库 | 零偏差 |
| 代理产出合同验证 | 纯 Go 标准库：`encoding/json`、`strings`、`path/filepath` | 合同架构是 YAML，通过现有转码处理 | 零偏差 |

**结论**：所有方向都可以通过 Go 标准库 + 现有 harness 适配器框架来实现。零外部依赖契约在 v2 中继续成立。对于 WASM gate 引擎（扩展路线图中的方向 3）——则需要 `wazero`——这将打破契约，但这是 v3 的问题。

### 自建 vs 采购决策

所有五个方向都倾向于**自建**：

| 维度 | 评估 |
|---|---|
| 差异化 | 阈值校准和合同验证是 ForgeOS 的核心差异化因素——购买它们将意味着商品化 |
| 数据本地性 | 所有方向都消耗本地 trace / scorecard 数据——无外部服务可以访问它 |
| 复杂度 | 所有 v1 实现都 < 200 行纯 Go，使用标准库 |
| 维护负担 | 低——这些是新包、单文件逻辑路径，不是长期维护的重负载 |

**例外**：如果（且仅当）v2+ AST 风险提取需求增长超出正则级别模式匹配，则使用 `tree-sitter-go`（已经是一个稳定、纯 C 的 AST 解析器）作为 vendor 的 Go 绑定可能是有意义的。但这不是 v1 的考虑因素。

---

## 5. 实施路线图

### 最终优先级（基于审计反馈的交叉验证 + 我自己的分析）

| 排名 | 方向 | 优先级（已更新） | 类别 | 短期价值 | 风险 | 估计（精短 Sprint） |
|---|---|---|---|---|---|---|
| **0** | Phase Contract 抽象 | **P0** | 基础 | 为方向 1、3、4 实现提供基础架构 | 低（零行为变化；纯附加） | 1 |
| **1** | 代理产出合同验证（产品验证） | **P0** | 数据完整性 | 修复目前正在产生静默降级的实际 bug（Sprint 27） | 低（仅附加类型定义 + 验证器） | 1–2 |
| **2** | 静态分析风险提取（v1 内容嗅探） | **P0** | 路由安全 | 关闭当前空的安全下限（`risk=critical → Opus` 等待输入） | 低（仅正则级模式；无新依赖） | 1–2 |
| **3** | 预测性运行估算 | **P1** | 成本可观测 | 利用已就位的丰富数据；解锁「这个要花多少钱？」 | 低（仅追加命令；零行为改变） | 1 |
| **4** | 跨运行失效分类 | **P1** | 运维智能 | 利用已就位的 trace 数据；提供 `forge trace --summary` | 低（仅追加命令） | 1 |
| **5** | 路由阈值自校准 | **P1** | 学习闭环 | 从同档最优 → 档位合理；长期成本节约 | 中等（需要阻尼来防振荡） | 2–3（v1 仅建议） |

### 阶段划分

#### 阶段 1：「基础架构」（1–2 个 Sprint）

专注方向 0 + 方向 1（代理产出合同验证）。

```
Sprint A:
  新增 internal/contract/ 包
    - PhaseContract / ArtifactSpec / VerdictSpec 类型定义
    - ParseVerdict（从 cost.go 提取，保留回退）
    - ParseConfidenceScore（从 cost.go 提取，保留回退）
  新增 asset.Phase.ContractRef 字段（零值 = 跳过验证）
  新增 forge validate --contracts（检查合约解析）

Sprint B:
  新增 internal/contract/validator.go
    - ValidateEmits（产物存在性 + 架构 compliance）
  将 contractGate 添加到 internal/gate/resolve.go（可选 v2 门控）
  将 cost.go:parseReviewerVerdict 弃用为 internal/contract.ParseVerdict 的包装器
```

**里程碑**：`forge validate --contracts` 针对全部 5 个工作流运行且零失败。`ParseVerdict` 产生与旧版 `parseReviewerVerdict` 相同的输出（差异可验证）。

#### 阶段 2：「路由安全性」（1–2 个 Sprint）

方向 3（风险提取 v1） + 开始方向 2（预测引擎）。

```
Sprint C:
  扩展 internal/risk/risk_diff.go：
    - 新增 sniffContent(path) — 轻量级文件内容检测
    - 新增 detectCriticalFuncs(content, lang) — 基于模式的关键函数检测
    - 新增 detectPaymentImports(content, lang) — 关键导入检测
  将 internal/routing/scoring.go 添加到 computeComplexity（基于文件的启发式）
  对内部文件测试所有新函数（forge-accept 闸门自动触发）

Sprint D:
  新增 internal/predict/（forge predict CLI 命令）
    - ForgePredict（基于评分卡历史）
    - BaselineEstimate（回退到硬编码默认值）
    - 从 trace.jsonl 聚合 percentiles
```

**里程碑**：`forge route --diff-files risk_diff.go` 报告内容级别的风险特征（不仅是路径）。`forge predict build` 输出预期成本范围。

#### 阶段 3：「运维智能 + 自校准」（2–3 个 Sprint）

方向 4（失效分类） + 方向 5（阈值校准）。

```
Sprint E:
  新增 cmd/forge/trace.go：forge trace --summary
  新增 internal/trace/archive.go：旋转 + ArchiveReader
  新增 internal/doctor/anomaly.go：FailureTrend

Sprint F:
  新增 internal/routing/calibrate.go
    - CalibratedThresholds 类型
    - Recalibrate（以评分卡为输入，输出建议的阈值）
    - WithDeadband（防振荡阻尼）
  forge route --calibrate 输出建议的阈值调整
```

**里程碑**：`forge trace --summary` 报告跨运行聚合数据。`forge route --calibrate` 输出与当前硬编码阈值有差异的建议。

#### 阶段 4：「可选严格性」（选择加入，非默认开启）

方向 2（独立验证） + 方向 5（合同门控）。

```
Sprint G（可选）：
  添加 contract_check 门控：在完成该阶段后，在运行下一阶段之前由可选门控运行
  添加 N/A 与 PASS 在 prompt 中的差异化（建议自 Sprint 30：prompt_context 中的 `[N/A]` vs `[PASS]`）
```

**里程碑**：`forge run --strict` 执行合同验证。门控失败会记录 trace 事件并延迟收敛。

### 风险与缓解策略

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| **Contract YAML 格式膨胀** | 中等 | 每个新阶段都需要新合同——如果设计过于严格，用户会弃用该机制 | 保持合同可选；默认跳过验证；使用「最少意外」的宽松模式 |
| **校准阻尼振荡** | 中等 | 如果阈值调整与评分卡反馈形成对抗性循环，系统可能会自我破坏 | 仅在每次评分卡更新时重算的限速器；需要 `min_samples` 才能生效；死区带宽 |
| **内容嗅探误报** | 低 | 注释中的 `import payment` 触发 `TouchesPayment = true`，导致路由不必要地升级 | 将 `sniffContent` 评估记录为「启发式」而非「确定式」；允许手动覆盖（现有 `--touches-*` 标志） |
| **Trace 归档磁盘使用率** | 低 | 旋转的 `.jsonl.N` 文件可能会堆积 | 添加 `forge trace prune --keep N`（类似 `forge memory-prune`）；默认保留 10 个文件 |
| **`cmd/forge` 文件计数再次爆发** | 高（模式） | 每个新命令都会向 `cmd/forge` 添加一个新的 `.go` 文件，该文件已处于 16/17 预算之内 | 严格使用先例：新命令小于 50 行 → `cmd/forge` 中的内联；大于 50 行 → 需要新的 `internal/` 包 |

---

## 总结

这五个方向代表着一次质量极高的架构审计——代码证据精确到行数，优先级有很好的合理性，且交叉验证揭示了严格诚实的事实（例如，`DefaultCompactThreshold` 确实被消费了；`forge status --json` 已实现），这些事实是值得信赖的分析的特征。

我的主要架构建议是：

1.  **将 Phase Contract 抽象添加为方向 0**。它是其他三个方向的基础，可在单个 sprint 中完成，且零行为影响。没有它，方向 1（工作流组合）、3（冲突检测）和 4（产物验证）最终要么重复工作，要么构建互不兼容的合同概念。

2.  **在方向 1（合同验证）中，将 `cost.go` 的 ad-hoc 解析器提取到 `internal/contract/` 中**。Sprint 27 已经证明了脆弱的字符串协议解析的成本——一个 `VERDICT:` 放在代码块内部就会静默降级为一个空值，从而导致 agent 的白白花费没有被注意到。正式化该协议解析应该与提取它同时进行，因为两者都需要相同的数据流分析。

3.  **将阶段 1 的范围缩小到仅方向 0 + 方向 1**。审计反馈正确地指出了两处事实校正：`DefaultCompactThreshold` 被消费，`forge status --json` 已实现。将这些纳入方向优先级意味着方向 3 和 4（依赖于这些的）的紧急程度略低于最初评估的水平。将阶段 1 的范围缩小到两个 P0 方向——合同（修复活跃的静默降级 bug）+ 基础方向（Phase Contract 抽象）——可实现 2 个 sprint 内可验证的交付。

4.  **接受 `cmd/forge` 分解模式并将其形式化**。将其从一次性的、逐个 sprint 的损害控制（每次文件计数超过预算就创建新包）转变为明确的包提取检查表：每添加一个超过 50 行的新 CLI 命令，都需要先创建一个 `internal/<domain>/` 包，然后才能将 CLI 胶水写入 `cmd/forge`。这比现在每次接近文件计数上限后都提高 `package.max_files` 要好。
