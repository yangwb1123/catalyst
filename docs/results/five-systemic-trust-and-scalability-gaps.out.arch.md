现在我对代码库有了透彻的理解。以下是架构分析。

---

# 架构分析：ForgeOS — 从 5 个已验证盲区出发

## 1. 架构评估

### 总体架构风格：脊柱 + 循环混合体

当前架构（v2，已落地的 `forge-core` Go 运行时）是一个**脊柱编排器，辅以演化循环**：
- **脊柱**：`DISCOVER → DESIGN → REVIEW → BUILD`，通过 `forge run` 执行的单次线性传递。
- **循环**：`EVOLVE`（通过 `forge evolve`），迭代地 `Scan → Gap → Roadmap → Implement → Review → Evaluate → Loop`，直到收敛。

引擎位于 `internal/orchestrator/`，核心原语是 `LoopEngine`，它在当前工作流上迭代，在每次迭代后通过 `converge.Converge` 测量收敛信号，并由 `risk.Classify` 通过模型路由辅助。持久化（checkpoint/trace/memory）已建立且版本化。闸门执行通过 `gate` 包实现了非公司化。

### 优势

| 层面 | 优势 |
|---|---|
| **引擎架构** | 循环终止是**基于信号的**（`converge.Signals`），而非基于轮数。`LoopOutcome` 总是说明*为什么*停止。这是正确的根基属性。 |
| **安全/失效关闭** | `evalOne` 中未知指标自动视为“未满足”。`GateProof` 使得豁免可见，而非静默隐藏。`risk.Classify` 是不可逆信号的硬性底线。 |
| **关注点分离** | `LoopEngine`（编排）不知道 `persist`（IO）或 `trace`（可观测性）。通过回调（`OnIteration`、`OnPhase`）注入钩子。这是恰当的分层。 |
| **持久化格式版本控制** | 大部分已经做对：checkpoint（`forgeos.checkpoint.v1`）、trace（`forgeos.trace.v1`）和 memory（`forgeos.memory.v1`）都携带 `_format` 标记，带有向后兼容的 `omitempty` 回退。在 v1 之前就思考版本控制是成熟的。 |
| **零外部依赖** | `go.mod` 中 `require` 为零。对于安全审计和长期维护来说，这是一个巨大的胜利。 |

### 局限性（架构债务）

| # | 债务 | 严重程度 | 描述 |
|---|---|---|---|
| **D1** | **输出验证真空** | **高** — 信任基线 | Agent 输出（`sanitizeAgentOutput`、`VERDICT`、`RoadmapCompletion`、`FileDelta`、`ConfidenceScore`）被解析但从未被独立验证。`computeFileDelta` 做的是关键词子串匹配，而非实际代码 diff 分析。存在输入信任，没有「反驳」层。 |
| **D2** | **质量衰减 ⇢ 停滞循环脱钩** | **高** — 长期运行风险 | `staleCount` 只测量进展停滞（`RoadmapCompletion` 是否前进？`GatesGreen` 是否变绿？）。它不感知质量趋势——测试覆盖率下降、复杂度增加、评审严格性松懈——因此一个循环可以在产出具腐化的代码时仍然算作「有进展」。 |
| **D3** | **`FromChangedPaths`⇢ 执行策略的缺失链路** | **中** — 效率损失 | `risk.FromChangedPaths` 通过路径启发式方法派生出受影响的表面，但其输出仅流向阶段层级解析器。它不驱动执行策略——无跳跃式门控、无差异化模型选择、无预算调整。有信号，但信号是断开的。 |
| **D4** | **全量执行，无稀疏模式** | **低-中** — 规模成本 | 每一次 `forge accept` 运行所有门控。每一次 `arch-check walk()` 扫描所有 190+ 文件。没有任何机制可以根据变更影响重新计算门控或审计步骤。对于小型变更来说成本很高；在规模下不可持续。 |
| **D5** | **Workflow YAML 定义，无格式版本控制** | **中** — 演进风险 | Checkpoint/trace/memory 已版本化，但 `build.yml`、`design.yml` 等缺少 `format_version`。`GatesGreen` 是一个无法追踪门控组成变化的 `bool`。跨版本迁移没有 `forge migrate workflow` 命令。 |

### 架构未识别出的结构性盲区

审查报告正确地指出了审查报告中的五个方向，但我发现还有**第六个系统性缺失**，它横跨多个方向：

**D6 — 缺少「证据链」抽象**：每一个输出验证问题（方向一）、质量问题（方向二）和版本信任问题（方向四）都可以追溯到一个核心缺失：没有**证据**（`Evidence`）作为一等运行时概念。今天：

- Agent 做出声明（`Verdict: "approved"`、`ConfidenceScore: 85`、`RoadmapCompletion: 0.7`）。
- 存在没有证据的断言。
- 没有一层来将断言与支持证据（文件路径、门控输出、测量结果）绑定，并由审计层进行抗篡改链接。

引入 `Evidence` 将把方向一、二和四联系到一个统一的原语下。

---

## 2. 扩展方向（根据审核修正）

### 方向 A：证据锚定的输出框架（P1 — 信任基线）

#### 为什么需要

当前，ForgeOS 信任 agent 的自报告。没有独立验证层意味着以下方面不可抵赖：幻觉审查、投毒记忆条目或虚假的路由图完成度。对于任何非演示用途的部署来说，这都是*首要*信任问题。

#### 核心挑战

1.  **系统验证**：验证「VERDICT = approved」意味着重放评审过程或验证其产物。两者成本都很高。你需要一个*分层*模型——一些断言是廉价的（模式检查），一些是昂贵的（完整审查重放）。
2.  **证明 vs. 暗示**：证据必须可链接到其主张，同时保持紧凑。一个「路由图完成度证据」可能是一个指向完成了 `[x]` 项的 git blob 的指针，而非内联内容。
3.  **跨会话持久化**：证据必须与检查点和追踪一起持久保存，这样在恢复时，不会忘记先前迭代的验证状态。

#### 预期的架构变更

```
Current:  agent output → parse → trust → use
Target:   agent output → parse → VERIFY (light) → evidence record → use
                                          ↓
                                     VERIFY (heavy) on demand
                                          ↓
                                   audit log (tamper-evident)
```

具体来说：
- **新包**：`internal/evidence/`——`Evidence` 类型（主张 + 证明 + 时间戳 + 验证者身份）、`Verifier` 接口（廉价的 + 昂贵的验证器）、`Chain`（用于抗篡改检测的链接哈希）。
- **扩展现有信号**：`converge.Signals` 获得可选的 `EvidenceRef`，因此收敛报告可以链接到其支持证据。
- **审计层**：`internal/trace/` 扩展为包含证据事件，使事后调查成为可能。

#### 对现有系统的影响

- 对不连接证据的现有工作流的**向后兼容性**：旧信号以 `EvidenceRef = nil` 的形式进入，并保持现有行为。
- `evalOne` 中引入可选的证据检查：如果提供了证据，则验证；如果未提供，则照常根据代理的声明进行评估。
- **存储成本**：每个循环迭代多出数百字节。不重要。

---

### 方向 B：质量趋势检测与手术干预（P2 — 长期可靠性）

#### 为什么需要

`staleCount` 是*停滞*检测器，不是*质量*检测器。一个循环可以产生技术债务不断的代码，每次迭代都声称完成度从 30% → 35% → 40%，而质量指标却在恶化。对于计划运行 24 小时以上的自治系统来说，这是必要的。

#### 核心挑战

1.  **信号选取**：哪些信号构成「质量趋势」？测试覆盖率 delta（`mode.go:47` 已提到 `coverage_delta`）、复杂度 delta、评审严格性（评审是否发现更多问题？）、lint/安全警告 delta。这些信号带有不同的收集成本。
2.  **非平稳性**：一个项目开始时覆盖率低，然后提高。基线会发生变化。趋势检测必须区分*噪声*、*改进*和*衰减*。
3.  **干预触发器**：如果检测到衰减，会发生什么？模型升级？强制重构阶段？人类上报？干预必须与触发信号的严重程度成比例。

#### 预期的架构变更

- **新包**：`internal/trend/`——环形缓冲区（例如，过去 10 次迭代）、趋势函数（斜率、EWMA、阈值比较）、`TrendSignal` 枚举。
- **扩展现有信号**：`converge.Signals` 获得 `QualityTrend TrendSignal`（`steady` / `improving` / `declining`）。
- **循环扩展**：`LoopEngine` 获得 `QualityTripwire`——如果质量在 N 次迭代中持续下降，则触发手术阶段（重构、增加测试覆盖率的任务）。
- **CLI 暴露**：`forge evolve --quality-tripwire=3`（连续 3 次迭代下降则触发）。

#### 对现有系统的影响

- 所有趋势信号都是可选的（零值 = `steady`，无触发器）。
- 手术干预是作为*同一循环内*的阶段注入的，而非作为独立的执行。循环不会为重构产生子进程；它会重新规划并继续执行。这避免了调度复杂性。
- **风险**：错误警报可能中断有效进展。缓解措施：仅当迭代也显示*停滞*或*通过*时才触发干预，而非仅仅在衰减期间。调整需随经验而来。

---

### 方向 C：依赖感知的稀疏执行（P3 — 规模性能）

#### 为什么需要

每一次 `forge accept` 运行所有 6 个门控。每一次 `arch-check walk()` 扫描所有文件。对于小型变更，这在达到规模时变得昂贵。门控子集 DAG（`test` → `build` → `lint`，但 `secret-scan` 如果更改不触及敏感文件，可能不需要运行）是未使用的。

#### 核心挑战

1.  **影响图构建**：你需要一个从文件路径到门控/检查的静态映射。某些映射是一一对应的（`secret-scan` → 包含凭据的文件）。其他映射是传递的（更改测试文件 → 运行测试门控）。这需要声明性元数据。
2.  **最小的与最大的**：最小集合（仅直接受影响的检查）可能存在覆盖漏洞。最大集合（基于可达性的传递闭包）可能是完整的，但成本高昂。你需要一个*分层*的稀疏化模型。
3.  **并行编排**：`RunParallel`（基于 wave 的）已经注意到选择性执行与波分组之间存在冲突。稀疏执行必须产生相位 DAG，而非线性序列。

#### 预期的架构变更

- **新元数据**：`policies/gate-deps.yml` 或每个 `.agent/workflows/*.yml` 中的 `depends_on` 字段——声明「如果路径与 `X/**` 匹配，则门控 G 是必需的」。
- **影响滤波器**：一个新的 `internal/impact/filter.go`——给定 `git diff --name-only` 返回*需要*哪些门控/阶段。纯函数，无 I/O。
- **门控执行器扩展**：`gate.Gate()` 获得一个可选的 `--filter` 标志，该标志传递路径子集。如果所有路径都是「不影响，跳过」，则门控退出，状态为 NA（而非 PASS/FAIL）。
- **生成式审计**：*实际*运行的门控集被记录，因此审计可以验证稀疏性是否合法（即，没有门控因格式错误的影响定义而被跳过）。

#### 对现有系统的影响

- 零影响，除非提供了影响滤波器。`--filter` 的缺失意味着「运行所有门控」，完全向后兼容。
- `impact.Filter` 产生的 NA 状态**不得**满足 `GatesGreen`，除非所有必需的门控都显式运行并通过。这意味着需要一个新的聚合器：`GatesGreenSparse`，它理解「门控 X 被跳过，因为影响声明说其覆盖路径未更改」。这是门控的 `Converge` 等价物。
- **风险**：过窄的影响声明可能漏掉问题。缓解措施：稀疏化是*加速*，而非*替代*。运行 `forge accept --full`（非稀疏）是一个可选的确认步骤。

---

### 方向 D：Workflow YAML 版本化 + Gate 格式契约（P2 — 演进安全）

**根据审核修正**：基础已存在（checkpoint/trace/memory 已版本化）。缺失项是 Workflow YAML 和 Gate 聚合信号。

#### 为什么需要

`build.yml` 中没有 `format_version` 意味着未来的格式更改无法被早期检测到。当门控集合演变时，`GatesGreen` 的 `bool` 性质会丢失语义：今天的「门控都是绿色的」与明天的门控集含义不同。没有 `forge migrate` 命令来处理跨版本转换。

#### 核心挑战

1.  **YAML 格式演化**：与严格类型化的 JSON 模式（checkpoint、trace）不同，YAML 是松散类型化的，且通过 Go 的 `encoding/json` 解析（通过 python 桥接）。在 YAML 中引入 `format_version` 需要与迁移工具协调。
2.  **`GatesGreen` 是 bool，不是集合**：将其更改为集合（`{"lint": "PASS", "test": "NA", ...}`）会破坏每个使用 `bool` 的现有 `converge.Signals` 消费者。一个*并行的* `GatesVerdict` 字段，保留了 `GatesGreen` 的原始逻辑，是正确的演进路径。
3.  **迁移工具链**：`forge migrate workflow` 必须处理路径：工作流 YAML → 转码后的 JSON → 运行时结构。这是三条需要同步的路径。

#### 预期的架构变更

- **工作流 YAML**：`format_version: "forgeos.workflow.v1"` 添加到每个 `.agent/workflows/*.yml`。如果缺失，则默认为 `"forgeos.workflow.v1"` 以实现向后兼容。
- **`converge.Signals`**：添加 `GatesVerdict map[string]string`（在 `GatesGreen bool` 之外）。`evalOne` 获得一个理解门控聚合规则的可选 `gates_verdict` 分支。
- **`internal/migrate/`**：扩展以处理 `workflow` 资源类型，对于仅版本升级的模式，可能会回退到 `adr` 和 `checkpoint` 迁移器。
- **`asset.Workflow`**：获得一个 `FormatVersion string` 字段，在加载时填充。

#### 对现有系统的影响

- `GatesGreen` 保持为 `bool` 且完全向后兼容。`GatesVerdict` 是并行添加的，最初为 `nil`。
- 通过 `yaml2json` 桥传递版本信息——需要桥能够感知 `format_version`。这不是重大更改；目前的桥只是盲目地复制顶层键。
- **最大风险**：YAML → Go 通过 JSON 的转码引入了额外的移动部件。一个原生的 Go YAML 解析器（由于 `go.mod` 中的零依赖而被避免）最终将需要用于生产部署。建议：v3 开始的外部依赖是 `gopkg.in/yaml.v3`，仅用于 `asset` 包，以消除桥。

---

### 方向 E：从变更到执行策略的信号路径（P3 — 架构短路修复）

#### 为什么需要

`risk.FromChangedPaths` 存在，产生结构化风险数据供消费，但它被浪费了。它只用于阶段层级解析器（`resolveAutoRisk`），从未用于驱动执行策略——跳过门控、提升模型、调整预算、针对变更定制提示。信号已产生但未被路由。

#### 核心挑战

1.  **将影响映射到预算**：`FromChangedPaths` 输出（BlastRadius、敏感表面标志）应注入 `run_budget.go`，以便高风险变更获得更大的预算，低风险变更获得更小的预算。这需要从 `risk.Signals` 到预算分配的线性映射。
2.  **将影响映射到提示策略**：一个触及支付代码的变更必须包含安全上下文。一个纯测试变更不需要。Agent 提示应根据影响范围进行扩充。
3.  **与门控稀疏化（方向 C）的协调**：方向 E 决定了「*如何*运行此变更」，方向 C 决定了「*哪些*门控运行此变更」。它们必须引用相同的 `risk.Signals` 输入。

#### 预期的架构变更

- **`internal/risk/` 中的新生产者**：一个可选的 `Strategy` 输出源自 `FromChangedPaths`：
  ```go
  type Strategy struct {
      BudgetMultiplier float64           // 1.0 = 正常, 2.0 = 高影响的双倍预算
      GateWhitelist    []string          // 要运行的门控子集（如果为 nil，则全部运行）
      ContextInject    []string          // 要注入到代理提示中的上下文主题
      ModelFloor       string            // 如果影响较高，则覆盖模型层级底线
  }
  ```
- **`internal/budget/` 中的新消费者**：一个考虑影响乘数的 `ImpactBudget` 函数。
- **`internal/orchestrator/` 中的新路由**：`phaseTier` 和 `RunFrom` 接收可选的 `Strategy`，并应用其指令。

#### 对现有系统的影响

- 策略是可选输出。如果 `Strategy` 为 nil，则所有执行路径与之前完全相同。
- 预算乘数必须尊重硬性上限（预算乘数不得超过 3.0 或类似限制），以降低资金意外耗尽的风险。
- **风险**：不正确的路径影响分析可能导致测试不充分。缓解措施：策略建议在默认情况下是*可选的*；用户必须通过 `--apply-risk-strategy` 显式启用。

---

## 3. 接口设计原则

### 核心原则

1.  **证据作为一等值**：任何有副作用的操作（Agent 调用、门控执行、人类批准）都会产生一个可选的 `Evidence` 记录。证据是惰性验证的——廉价检查立即运行，昂贵检查在请求时运行。
2.  **所有东西都是可选的**：新字段为 nil/零值，并且完全向后兼容。没有破坏性更改。
3.  **信号是单调的**：`converge.Signals` 新字段必须从零开始，只会增长（从不收缩）。这使得版本升级安全。

### 新的抽象层

提出了两个新的抽象层：

| 抽象层 | 包 | 职责 |
|---|---|---|
| **证据链** | `internal/evidence/` | `Evidence` 记录、`Verifier` 接口、可选的链哈希 |
| **趋势线** | `internal/trend/` | `RingBuffer`、`Slope` 函数、`Tripwire` 评估 |

这些服务于明确的目的，运行时可选择加入，并且不会将自身注入到核心循环路径中，除非被调用。

### 向后兼容性策略

```
API / 结构体更改协议：
1. 新字段始终为指针/可选 → nil = 旧行为
2. 旧字段永远不更改类型
3. 新信号字段对收敛是可选的（零值 = 无数据 = 未满足）
4. 持久化格式：新的 JSON 字段 + `omitempty` = 旧文件原生加载
```

---

## 4. 技术选型

### 新依赖项评估

当前 `go.mod` 具有零依赖项（无 `require`）。这是当前架构的一个核心租户，维护良好。方向 A–E 不需要新的 Go 依赖项。

| 方向 | 需要的 Go 依赖项 | 理由 |
|---|---|---|
| A（证据） | 无 | 纯标准库。SHA-256 用于链哈希，JSON 用于序列化。 |
| B（趋势） | 无 | 纯数学（EWMA、斜率）。环形缓冲区是简单的切片。 |
| C（稀疏执行） | 无 | 纯 DAG 遍历。`impact.Filter` 是纯路径匹配。 |
| D（YAML 版本化） | **可能：** `gopkg.in/yaml.v3` | 用于直接 YAML 解析以替代 python 桥。这是一个架构决策——推迟到 YAML 版本化的*第二次*迭代，届时 `yaml2json` 桥最终被替换。 |
| E（风险策略） | 无 | 纯派生。`Strategy` 是 `FromChangedPaths` 输出上的纯函数。 |

### 自建 vs. 采购决策

| 功能 | 决策 | 理由 |
|---|---|---|
| 证据验证 | **自建** | 领域特定（ForgeOS Agent 输出验证）。没有现成的库。 |
| 趋势检测 | **自建** | 简单的数学（EWMA、斜率）。标准库就足够了。 |
| 影响分析 | **自建** | 文件路径 → DAG 映射特定于 ForgeOS 的工作流结构。 |
| YAML 解析 | **购买**（`gopkg.in/yaml.v3`） | 成熟的、经过战斗考验的库。自建意味着重新实现 10k LOC 的复杂性。仅适用于生产部署（v3+）。 |
| 审计日志 | **自建** | 证据链格式特定于 ForgeOS。 |

### 版本化策略

方向 D 隐含了一个跨整个系统版本化策略：

| 工件 | 当前状态 | 格式版本 | 行动 |
|---|---|---|---|
| checkpoint | ✅ 已版本化 | `forgeos.checkpoint.v1` | 无变化 |
| trace | ✅ 已版本化 | `forgeos.trace.v1` | 无变化 |
| memory | ✅ 已版本化 | `forgeos.memory.v1` | 无变化 |
| workflow YAML | ❌ 未版本化 | — | **新增：** `format_version: "forgeos.workflow.v1"` |
| gate verdict | ❌ 未结构化 | `bool GatesGreen` | **新增：** `GatesVerdict map[string]string` 并行字段 |
| adr (ADR-0001) | ❌ 未版本化 | — | 未来考虑 |

---

## 5. 实施路线图

### 优先级

| 优先级 | 方向 | 理由 |
|---|---|---|
| **P1** | A — 证据锚定的输出 | 信任基线。没有它，所有其他方向都建立在无法审计的输出之上。 |
| **P2** | B — 质量趋势 | 长期运行需要它；24 小时会话已在路线图中。 |
| **P2** | D — 工作流 YAML 版本化 | 演进安全。格式风味之前的未来工作流更改的封锁。 |
| **P3** | C — 稀疏执行 | 性能增益。重要但不阻止功能。 |
| **P3** | E — 从影响到策略的路径 | 效率增益。前提是稀疏执行（方向 C）的某些部分。 |

### 阶段划分

#### 阶段 1：「信任」（P1 — 方向 A）

*里程碑：ForgeOS 可以证明其代理输出的真实性。*

- [ ] 定义 `internal/evidence/` 中的 `Evidence` 类型
- [ ] 实现 `Verifier` 接口（轻量级：模式检查；重量级：可选的完整重放）
- [ ] 向 `converge.Signals` 添加可选的 `EvidenceRef` 字段
- [ ] 将证据记录挂接到 `trace.Event`
- [ ] 更新 `reportConvergence` 以显示证据状态
- [ ] 添加 `forge verify <signal>` CLI 命令以按需运行重型验证器

**风险与缓解**：

- **性能**：重型验证器可能代价高昂。缓解措施：重型验证是异步的，并由 `forge verify` 按需触发。从不阻塞循环。
- **存储**：证据记录可能会累积。缓解措施：证据是检查点和追踪的*部分*；它们受相同的轮换政策约束。

#### 阶段 2：「韧性」（P2 — 方向 B + D）

*里程碑：循环检测质量下降；工作流 YAML 版本化。*

**子阶段 2a：质量趋势（方向 B）**

- [ ] 在 `internal/trend/` 中定义 `RingBuffer` 和 `Slope` 函数
- [ ] 向 `converge.Signals` 添加 `QualityTrend` 字段（无副作用——仅可观测）
- [ ] 将 `QualityTripwire` 添加到 `LoopEngine`（触发手术阶段）
- [ ] 实现手术阶段注入器（当触发时，追加一个 `refactor` 阶段到当前迭代）
- [ ] 添加 `forge evolve --quality-tripwire=N` CLI 标志

**子阶段 2b：工作流版本化（方向 D）**

- [ ] 将 `format_version: "forgeos.workflow.v1"` 添加到所有 `.agent/workflows/*.yml`
- [ ] 将 `GatesVerdict map[string]string` 添加到 `converge.Signals`（与 `GatesGreen` 并行）
- [ ] 更新 `evalOne` 以理解可选的 `gates_verdict` 分支
- [ ] 实现 `forge migrate workflow`（最初是 `adr` 迁移器的直接包装器）
- [ ] 将 `FormatVersion string` 添加到 `asset.Workflow`

**风险与缓解**：

- **方向 B 的错误警报**：早期迭代中的假阳性可能会将循环送入无用的重构周期。缓解措施：质量触发在默认情况下是*关闭的*（`--quality-tripwire=0`），并且默认记录但不干预。干预在需要时显式选择加入。
- **方向 D 的 YAML 桥依赖**：`yaml2json` 桥必须传递 `format_version`。风险较低——这只是一个额外的顶级关键字。

#### 阶段 3：「效率」（P3 — 方向 C + E）

*里程碑：ForgeOS 只运行必要的门控，并根据影响调整其执行策略。*

**子阶段 3a：稀疏门控（方向 C）**

- [ ] 定义 `policies/gate-deps.yml` 或内联到工作流 YAML 中（`depends_on` 已存在于 `asset.Phase` 中，但用于相位排序，而非门控选择）
- [ ] 实现 `internal/impact/Filter`（纯函数：`(paths, deps) → gate_subset`）
- [ ] 向 `gate.Gate()` 添加 `--filter` 标志
- [ ] 添加 `forge accept --sparse` 标志（或在与影响配置文件匹配时默认自动检测）

**子阶段 3b：风险驱动的策略（方向 E）**

- [ ] 定义 `internal/risk/Strategy` 输出类型
- [ ] 从 `FromChangedPaths` 实现 `Strategy` 派生
- [ ] 将策略注入预算分配（`internal/orchestrator/budget.go`）
- [ ] 将策略注入提示上下文（如果影响映射到安全，则添加安全指导）
- [ ] 添加 `forge run --apply-risk-strategy` CLI 标志

**风险与缓解**：

- **稀疏门控可能会漏掉问题**：过窄的影响声明可能是错误的。缓解措施：稀疏化是*选择加入的*，完整的 `forge accept --full` 始终可用。`--sparse` 运行也会生成一个「跳过的门控」审计轨迹。
- **风险策略可能会过度分配预算**：如果 `BlastRadius` 被高估（例如，大型但安全的重命名），预算乘数可能会浪费资金。缓解措施：预算乘数上限为 3.0 倍，并且可以通过 `--max-budget-usd` 被硬性预算上限覆盖。

---

## 总结：决策矩阵

| 决策 | 选项 A（选择） | 选项 B（拒绝） | 理由 |
|---|---|---|---|
| 证据应具有惰性验证措施 | **分层验证**：轻量级 + 重量级按需 | 所有验证始终运行 | 两条原则都成立，但按需重型验证避免了循环延迟的代价。 |
| 质量趋势是新信号还是现有信号的扩展 | **新 `trend.Signal` 类型** | 向 `Signals` 添加更多浮动字段 | 趋势是一个跨领域的问题——它消费多个信号并产生一个聚合。复杂的信号更好地封装。 |
| 门控稀疏化应使用声明性影响映射还是学习到的映射 | **声明性**（受控、可审计） | 学习到的（自动、不透明） | 声明性映射对于审计和调试是必要的。学习到的映射是 v4+ 的候选。 |
| Workflow YAML 版本控制应内联在 YAML 中还是作为元数据 | **内联** `format_version` | 单独的 `version.yml` 或目录清单 | 内联是自我描述的，并且对于工具来说是显而易见的。单独的元数据文件增加了分布式的复杂性。 |
| 风险策略应默认选择加入还是选择退出 | **选择加入**（`--apply-risk-strategy`） | 默认开启 | 策略改变了 ForgeOS 核心的执行语义。在证明稳定性之前，选择加入是更安全的。 |

总体评估：该架构稳健、诚实，且工程红线（零外部依赖、基于信号的终止、失效关闭）保持完好。五个已识别的盲区是真实的，并且本文中提出的扩展方向在运行的权衡范围内是可解决的，且不会损害现有系统。最大的一次性杠杆是**方向 A（证据锚定的输出）**，它建立了所有其他方向所依赖的信任基线。
