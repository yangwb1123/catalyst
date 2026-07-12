# 架构师分析报告：ForgeOS 五个未覆盖结构性扩展方向

## 1. 架构评估

### 1.1 当前架构优势

ForgeOS 的当前架构在以下方面表现出色，这也是五个方向能成立的**先决条件**：

| 优势 | 对应分析中的体现 |
|---|---|
| **极简核心主义** | forge-core 零外部依赖，`sync/atomic` + Go 标准库即可支撑完整编排循环 |
| **声明式治理骨架** | `.agent/` + `modes.yml` + `policies.yml` 形成清晰的配置面，使得声明-实现漂移成为可检测的问题（而非设计缺陷） |
| **清晰的 Phase 抽象** | `Phase{Agent, Emits, FeedsForward, FreshContext}` 的原子粒度让跨 phase 契约缺失成为可观察的缺口 |
| **Trace 系统已就位** | 虽然 trace 当前是指标级，但它证明团队已经认可「Observability as a feature」——从 0 到 1 已完成，本文讨论的是从 1 到 N |
| **Checkpoint 机制** | 存在基本的持久化层，语义日志可以在其肩头建立，而非平地起楼 |

### 1.2 当前架构局限性

五个方向本质上暴露了同一个架构问题——**ForgeOS 在治理纬度上存在「观测不对称」**：

```
被治理的 workflow (agent 行为) → 丰富的可观测性 (trace, cost, quality)
治理者自身 (forge-core)        → 黑盒
phase 间传递 (意图信息)        → 静默管道
配置声明层 (YAML)              → 无自动化对账
```

具体而言：

1. **可观测性单向性**：所有观测设施面向 agent 行为，观测者（forge-core、phase pipeline、配置系统）自身不被观测。这是一个**递归信任问题**——谁 guard 了 guard 者？

2. **隐式契约**：`feeds_forward`、`emits`、`task-plan.md` 的格式都是隐式约定。没有契约强制层，系统只能依赖设计者/维护者的纪律性。这在单人项目中可行，在多人协作中不可持续。

3. **信息销毁惯例**：phase 执行输出被解析后即丢弃、convergence 详情不被持久化、loop-back 原因非结构化——信息流的设计哲学是「用完即焚」。这在可重现环境中是可接受的，但在 LLM 非确定性环境中是**结构性风险**（无法回溯不可复现的行为）。

### 1.3 架构债务评估

| 债务类型 | 严重程度 | 来源 |
|---|---|---|
| **双真相源债务** | 高（P1） | `modes.yml` ↔ `mode.go`、`policies.yml` ↔ `gate.mjs/arch-check.mjs` 的手工镜像 |
| **信息丢失债务** | 高（P1） | trace 记录「花了多少」而非「发生了什么」；phase 输出解析后遗弃；loop-back 原因字符串化 |
| **隐式契约债务** | 中（P2） | `emits` 声明但不验证；跨 phase 产出物格式无版本关联；Intent 无数据模型 |
| **自观测债务** | 中（P2） | forge-core 自身操作（loadWorkflow, gatherSignals, converge）无埋点 |

其中「双真相源债务」和「信息丢失债务」是**结构性债务**——它们不是某个代码模块的质量问题，而是整个系统设计的决策遗产。不处理的话，随着系统规模增长，债务利息呈指数增长（更多的模式/策略/phase → 更多的双真相源）。

---

## 2. 扩展方向

### 2.1 方向 A：结构化执行档案（Semantic Event Log + Intent Tracking）

> 对应文档方向一+方向二的融合升级

#### 为什么需要（业务价值）

这是从**实验性工具**到**受信自治系统**的核心一跳。没有执行档案：

- 用户无法信任 24h 自治运行的结果（「我怎么知道它做了什么才得出这个结论？」）
- 无法做 root cause analysis（「第三次迭代为什么失败了？——我们不知道，因为 trace 只有时间戳」）
- 无法做合规审计（「这个版本经过什么评审流程？——我们有 scorecard 显示成本 $0.84」）

**一句话**：没有执行档案的自治系统是魔术盒——用户要么接受魔术，要么不信任。

#### 核心挑战和技术难点

1. **信息量与存储成本的平衡**：语义日志比指标日志大 2-3 个数量级。一个 evolve 运行可能产生 MB 级语义事件。裁剪策略不能破坏审计链的完整性。
2. **非结构化到结构化的提取**：从 LLM 产出的自由文本中提取结构化 intent（planner 的 `INTENT: [...]`），依赖 prompt 工程可靠性，不是 100% 确定。
3. **跨版本兼容**：trace.ndjson 格式演化时必须保证旧 consumer 不崩溃。每行自描述 + 版本号是必要的，但版本跃迁策略需要明确。
4. **敏感内容风险**：agent 输出可能包含凭据、密钥、PII。默认采集 = 安全风险。

#### 预期的架构变更

```
forge-core
├── internal/trace/
│   ├── event.go          ← 现有：Event{Type, DurationMs, CostUsdMicros,...}
│   ├── semantic.go       ← 新增：SemanticEvent{
│   │                         PhaseOutput, Verdict, FilesChanged,
│   │                         ConvergenceDetail, LoopBackReason
│   │                      }
│   ├── intent.go         ← 新增：Intent{Domain, Object, Files}
│   └── writer.go         ← 修改：支持多类型事件混合写入 ndjson 流
```

`orchestrator.go` 的执行循环中插入语义捕获点：

```
runAgentPhase → Execute → parseVerdict → parseCost → [新增] captureSemanticPayload
                                                            ↓
                                                    emit SemanticEvent → trace.ndjson
```

#### 对现有系统的影响

- **向后兼容**：行格式从 `trace.jsonl` 变为 `trace.ndjson`，但 `{"type":"metric"}` 事件不变。`scorecard-update.mjs` 添加 `type != "semantic"` 过滤即可。
- **性能开销**：每 phase 增加 1 次 JSON 序列化 + 1 次文件追加写（批量 flush 后可忽略）。不变的热路径（`Execute`、`GatherSignals`）不受影响。
- **存储增长**：估算每 phase ~5KB 语义事件。100 phase 运行 ≈ 500KB。10MB cap 可覆盖 ~20,000 phase。

#### 选项比较

| 方案 | 优点 | 缺点 |
|---|---|---|
| **A1. 同文件混合流**（文档建议） | 时间序完整；单文件管理；consumer 按 type 过滤 | 文件格式迁移（jsonl→ndjson）；需要 schema evolution 策略 |
| **A2. 分离文件**（`trace.ndjson` + `semantic.ndjson`） | 互不影响；schema 独立演化；可按需裁剪 | 时间关联需要外部 join；多文件管理复杂度 |
| **A3. SQLite 嵌入式存储** | 结构化查询；ACID 保证；forge log 直接 SQL | 引入 CGo 依赖（违反零外部依赖） |

**推荐 A1**——折中复杂度与功能。文件格式变迁是工具链的演进，不是架构颠覆。A2 增加不必要的协调复杂度。A3 违反核心设计原则。

---

### 2.2 方向 B：配置-代码声明一致性护栏（Declaration-to-Implementation Drift Guard）

> 对应文档方向五

#### 为什么需要（技术价值）

ForgeOS 的核心价值主张之一是**声明式治理**。但如果声明（YAML）和实现（Go/JS）之间的对账是人工的，这个价值主张就只在审计当天成立。

配置-代码漂移的深层风险不是技术上的，而是**信任上的**：

- 当一个新开发者看到 `modes.yml` 的 `explorer.gates: [lint, build]`，他相信系统行为符合此声明。但实际上行为可能因 `mode.go` 未同步而不同。这种信任侵蚀是缓慢但致命的。
- 假设未来 forge-core 拆分为多个服务，每个服务都有自己的声明-实现映射，漂移问题从 1 处变为 N 处。治理复杂度呈线性增长。

#### 核心挑战和技术难点

1. **语义匹配**：YAML 值（`max_function_lines: 60`）和 Go 常量（`MAX_FUNCTION_LINES = 50`）的「含义」不完全等价。Go 可能做了安全增强（比 YAML 更严格），但普通的文本 diff 会标记为漂移。
2. **引用路径解析**：哪个 Go 常量对应哪个 YAML 路径？当前靠注释维护（文档建议的 `Source: path:key`），未来需要更自动化的方式。
3. **非二进制漂移**：不是所有漂移都同等严重。`gate_set` 漂移是 P0（安全影响），`max_file_lines` 漂移是 P2（无安全影响，仅代码风格）。需要优先级分级。

#### 预期的架构变更

```
forge-core/
├── cmd/forge/
│   └── audit.go          ← 新增：forge audit --drift 子命令
│
harness/
├── drift/
│   ├── discover.go       ← 新增：扫描 YAML + 代码中的声明值
│   ├── match.go          ← 新增：按 Source 注释匹配声明-实现
│   └── report.go         ← 新增：输出结构化漂移报告
│
.github/workflows/
└── forge.yml             ← 修改：CI 添加 forge audit --drift 步骤
```

不需要修改 `internal/mode/mode.go` 或 `harness/gate.mjs` 的实现逻辑——只添加**审计面**。

#### 对现有系统的影响

- **零侵入**：`forge audit --drift` 是只读审计命令，不修改任何现有代码的执行路径。
- **需约定 Source 注释**：在 Go 常量/JS 变量旁添加 `// Source: harness/policies.yml:max_function_lines`。这是轻量约定，不引入新抽象。
- **CI 集成风险**：如果漂移检测是 `fail-on-drift`，可能导致 CI 因非关键漂移（如注释格式）而红。建议初始阶段用 `fail-on-critical` 模式，只报告 `gate_set`/`阈值`/`路由策略` 等关键漂移。

#### 实现路径

```
Phase 1（当前 sprint）: 
  - forge audit --drift 扫描所有已知 YAML→Go 映射点
  - 输出 JSON: [{source, code_value, yaml_value, status}]
  - CI 中仅 WARN（非 FAIL）

Phase 2（安全验证后）:
  - 添加 gate_set / routing thresholds 的关键漂移 FAIL
  - 引入 drift-exceptions.json 允许声明已知漂移

Phase 3（成熟期）:
  - 自动发现新的声明-实现映射点（而非仅扫描预注册表）
```

---

### 2.3 方向 C：Phase 产出物契约系统（Artifact Contract Enforcement）

> 对应文档方向四

#### 为什么需要（技术价值）

当前 `emits` 声明的核心矛盾是：**系统用它们生成 prompt 上下文，但不在执行后验证它们**。这意味着：

- 一个 phase 的 `emits` 声明了 3 个文件，但实际只写了 2 个 → 静默失败
- 下游 phase 依赖的输入文件不存在 → 静默跳过 → 收敛判定在没有关键输入的情况下完成

这不是边缘 case——在 24h 自治运行中，每个 agent phase 都以 LLM 行为为基础，而 LLM 是**非确定性的**。它可能因为 prompt 长度的变化、token seed 的漂移、或者简单的不稳定而决定「今天不输出 requirement-draft.md」。当前系统对此毫无防御。

#### 核心挑战和技术难点

1. **延迟产出 vs 立即验证**：有些 phase 的产出物是逐步写出的（长时间执行的 agent 一边思考一边写文件）。当前 design 是 phase 结束后的同步检查。无法检查「还没有产出但正在产出中」的状态。
2. **Markdown 的结构验证**：不引入 Markdown AST 解析器的话，只能用正则/关键词匹配，误报率较高。
3. **可选的 schema 定义**：如果 schema 定义是可选的，就可能没人定义。这是治理工具的常见困境——「可选=永远不会被采用」。

#### 预期的架构变更

```
internal/
├── artifact/
│   ├── checker.go        ← 新增：存在性检查 + schema 验证
│   └── schema.go         ← 新增：Markdown 结构规则 DSL 解析
│
orchestrator.go           ← 修改：phase 结束后调用 artifact.Checker.Check()
```

核心变更点只有一处：`runAgentPhase` 末尾，插入一道 artifact check。

#### 对现有系统的影响

- **向后兼容**：artifact check 失败时行为是 WARN（默认）→ 不影响 convergence 判定。现有 workflow 不受影响。
- **无新依赖**：存在性检查用 `os.Stat`；schema 验证用简单规则引擎（50 行内实现），不引入 JSON Schema 库。
- **影响 execution time**：每 phase 增加 1-5ms（file stat + 可选内容扫描）。可忽略。

---

### 2.4 方向 D：Core 内部遥测基础设施（Self-Observability Layer）

> 对应文档方向三

#### 为什么需要（技术价值）

ForgeOS 的可观测性有一个讽刺的不对称：它能回答「这次 evolve 花了 agent 多少 token」，但不能回答「loadWorkflow 花了多少毫秒」。产品级 agent 的编排引擎自身没有性能/正确性数据，这在任何生产系统中都是不可接受的。

#### 核心挑战和技术难点

1. **对「零外部依赖」的挑战**：引入 telemetry 注册表（即使是 `sync/atomic` 计数器）也增加维护负担。每个新操作需要手动注册指标。
2. **基准比较的环境一致性**：GitHub Actions runner 之间的性能差异可能使基准比较产生噪音。需要 runner 标签对齐或归一化。
3. **决定 what to measure**：测量所有操作会导致指标爆炸。需要精确定义哪些操作是「关键路径」引入埋点，哪些是「辅助路径」不引入。

#### 对现有系统的影响

- **最小影响**：仅新增 `internal/telemetry` 包，所有指标点用 `sync/atomic` 实现，零锁、零阻塞。
- **完全不修改现有执行逻辑**：指标点以 decorator 方式注入——不是修改函数内部，而是包裹调用点：

```go
// 不修改 loadWorkflow 内部
// 而是：
func (e Engine) Run(ctx context.Context, ...) error {
    start := time.Now()
    err := e.loadWorkflow(ctx, ...)
    telemetry.Record("load_workflow.duration_ms", time.Since(start).Milliseconds())
    return err
}
```

#### 实施建议

不是所有操作都需要永久埋点。建议**先建立基础设施，再逐步添加指标点**：

| 迭代 | 新增指标点 | 优先级 |
|---|---|---|
| 1 | `forge accept` 总耗时、arch-check 各检查耗时 | P1（用户体验影响最大） |
| 2 | `loadWorkflow`, `gatherSignals`, `buildPrompt` 耗时 | P2（开发者体验） |
| 3 | yaml2json decode count/error count | P2（正确性门控） |
| 4 | converge.Evaluate 各 Criteria 耗时 | P3（极端场景分析） |

---

### 2.5 方向 E：声明层契约运行时验证（Runtime Contract Verification Layer）

> 该方向在文档中未单独列出，而是作为方向二的子问题和方向四的扩展。我将其作为一个独立的方向提出。

#### 为什么需要

当前 `feeds_forward` 和 `fresh_context` 是设计期声明，运行时无验证。一个定义为 `fresh_context: true` 的 reviewer phase 如果因为代码 bug 意外接收了 upstream context，当前架构无法检测。同理，一个规划了「3 项任务」的 planner，如果 implementer 只交付了 2 项，系统无感知。

这是一个**运行时契约验证**的缺失——不是验证 agent 的产出质量，而是验证 phase 间的通信承诺是否兑现。

#### 核心挑战

1. **对 phase 边界的动态理解**：planner 的 INTENT 是自由文本提取，不是静态结构化数据。验证的可靠性受 LLM 行为影响。
2. **验证的时机**：是在 implementer phase 结束后立即验证？还是在整个 workflow 结束时统一验证？前者增加延迟，后者延迟发现。
3. **不阻塞 vs 阻塞的权衡**：发现 intent 不匹配时，是只记录（不中断自治运行）还是触发 human escalation？前者降低信任价值，后者降低自动化率。

#### 架构变更

```
engine/
├── contract/
│   ├── intent.go         ← 新增：Intent 数据模型 + 解析
│   ├── verifier.go       ← 新增：intent → delivery 对比逻辑
│   └── registry.go       ← 新增：注册哪些 phase 对需要契约验证
```

关键设计点：契约验证默认是**只读**模式（记录+告警），不阻断。只有当 workflow 显式声明 `enforce_contract: true` 时才阻断。

---

## 3. 接口设计建议

### 3.1 关键模块接口设计原则

#### 原则一：隔离可观测性与执行逻辑

```go
// 不推荐——耦合
func (e *Engine) Run(ctx context.Context) error {
    telemetry.RecordStart()      // ← 可观测性混入执行路径
    err := e.runInner(ctx)
    telemetry.RecordEnd()        // ←
    return err
}

// 推荐——Decorator 模式
type TrackedEngine struct {
    inner *Engine
    telemetry *Telemetry
}

func (t *TrackedEngine) Run(ctx context.Context) error {
    start := time.Now()
    err := t.inner.Run(ctx)
    t.telemetry.Record("run.duration_ms", time.Since(start).Milliseconds())
    return err
}
```

**理由**：可观测性是可选的增强层，不是核心执行逻辑的一部分。`TrackedEngine` 可以用 build tag 控制编译是否包含。

#### 原则二：Semantic Event 作为一等类型

```go
// SemanticEvent 是数据的规范表示，非自由文本
type SemanticEvent struct {
    Version   int       `json:"v"`       // schema version
    Timestamp time.Time `json:"ts"`
    SessionID string    `json:"sid"`     // run identity

    Type    SemanticType `json:"type"`    // phase_completed | loopback | convergence | gate_result
    Subtype string       `json:"subtype,omitempty"`

    // 只放一个 payload 字段，避免类型爆炸
    Payload json.RawMessage `json:"payload"`
}
```

**理由**：用 `Payload` 承载具体类型比每个事件类型一个结构体更灵活。消费者根据 `Type` 决定如何解析 `Payload`。

#### 原则三：契约验证是插拔式的

```go
// PhaseContractVerifier — 每个 phase 可以声明要验证哪些契约
type PhaseContractVerifier interface {
    // Name 返回验证器名称，用于日志/告警标识
    Name() string
    // Verify 在 phase 执行后调用；返回违反的契约详情（空 = 全部通过）
    Verify(ctx context.Context, phase Phase, input, output *PhaseOutput) []ContractViolation
}
```

**理由**：不同的契约验证器（intent checker、artifact checker、schema checker）各自独立，可组合、可热插拔。Future-proof。

### 3.2 是否需要引入新的抽象层

**需要的抽象层**：

| 抽象层 | 位置 | 职责 | 说明 |
|---|---|---|---|
| **遥测注册表** | `internal/telemetry` | 提供命名计数器+定时器 | 轻量、`sync/atomic` 实现、无外部依赖 |
| **契约验证器 registry** | `internal/contract` | 管理 PhaseContractVerifier 列表 | 对 Engine 透明，Engine 只负责调用 `verifyAll` |
| **语义事件管道** | `internal/trace/semantic.go` | 定义结构化事件 + 写入 | 与现有指标事件隔离但共存 |

**不需要的抽象层**：

| 建议但实际不需要 | 理由 |
|---|---|
| 新的事件总线/消息队列 | ForgeOS 是单体二进制，in-process channel + 批量写文件足够 |
| 独立的 schema registry 服务 | 当前 schema 验证是文件级，用 JSON Schema 文件太大且不够灵活 |
| 配置参数验证框架 | 声明-实现漂移的 Source 注释约定足够，引入验证框架增加复杂度 |

### 3.3 向后兼容性

| 变更 | 兼容策略 |
|---|---|
| `trace.jsonl` → `trace.ndjson` | 旧的只写 metrics 事件的 consumer 继续工作（忽略 `type:"semantic"` 行） |
| `emits` 新增 emit_schema 字段 | 可选：缺省时行为不变（存在性检查 + WARN） |
| `forge audit --drift` 新增 | 新命令，不影响现有行为 |
| `forge log` 新增 | 新命令，不影响现有行为 |
| `forge metrics` 新增 | 新命令，不影响现有行为 |
| 核心执行循环插入捕获点 | 只在 module 版本号 >= 2 时激活；legacy mode 通过 env `FORGE_LEGACY_TRACE=1` 切换 |

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈

**不需要引入外部框架。** 五个方向的实现都不需要超出当前技术栈：

| 方向 | 所用技术 | 是否新依赖 |
|---|---|---|
| 语义日志 | Go 标准库 `encoding/json` + `os.File` | 否 |
| 意图验证 | Go 标准库 `strings` + `regexp` (`git diff --name-only` 输出解析) | 否 |
| 性能遥测 | Go 标准库 `sync/atomic` + `time` | 否 |
| 产出物验证 | Go 标准库 `os.Stat` + `strings.Contains` | 否 |
| 漂移检测 | Go yaml2json（已有）+ Node.js `js-yaml`（已有）| 否 |

**唯一可能引入的新依赖**是 JSON Schema 验证库——但文档建议不使用（保持零依赖）。Markdown 结构验证用关键词匹配代替。

### 4.2 第三方依赖评估标准

如果未来（v3+）引入第三方依赖，评估标准应是：

1. **零 CGo 要求**：CGo 破坏跨平台编译和静态构建。依赖必须 `CGO_ENABLED=0` 兼容。
2. **许可证兼容**：仅 MIT / Apache 2.0 / BSD。禁止 GPL/LGPL/AGPL（传染性许可证）。
3. **上游健康度**：GitHub stars > 500、最近更新 < 6 个月、open issues ratio < 5%。
4. **API 稳定性承诺**：go.mod 中声明 `v1` 或更高版本，避免 `v0.x` 依赖。

### 4.3 自建 vs 采购/引进

这些方向的决策全部倾向**自建**：

| 方向 | 自建理由 | 风险 |
|---|---|---|
| 语义日志 | 存储格式与 ForgeOS 的 trace 系统深度绑定，通用日志库对语义结构理解不匹配 | 需要 schema evolution 设计 |
| 意图验证 | ForgeOS 的 phase 概念是领域特定的，通用 agent 验证框架不存在 | 需要 prompt 工程配合 |
| 内部遥测 | 数据只用于 ForgeOS 自身运维，外部可观测系统（OpenTelemetry）太重 | 需要自己维护基准快照；可使用 OTEL 的指标命名约定但不用其 SDK |
| 产出物验证 | ForgeOS 的 `emits` 概念独特（文件声明 → prompt context），通用文件验证工具不适用 | 需要自己维护 schema 定义 |
| 漂移检测 | 完全依赖于 ForgeOS 的 YAML-Go 映射约定，无通用工具 | 需要约定 Source 注释语法 |

**唯一可能考虑引进**的是 OpenTelemetry 的指标命名规范（Metric naming conventions）——不是用其 SDK，而是借鉴其命名模式（`forge_internal_*`、`forge_accept_duration_ms`），避免未来迁移到 OTEL 时的映射成本。

---

## 5. 实施路线图

### 5.1 优先级排序

```
P0: 立即需要（当前 sprint）
P1: 下一 sprint
P2: 未来 2-3 sprints
P3: 远期
```

| 优先级 | 方向 | 序号 | 理由 |
|---|---|---|---|
| **P0** | 配置-代码声明漂移检测 | 方向 B | 最低成本、最高风险收益比。当前已有 `check.py` 基础设施，扩展即可；`modes.yml↔mode.go` 的静默偏差是正在发生的风险 |
| **P1** | 语义日志 + forge log | 方向 A (部分) | 调试 agent 行为异常是日常堵点。没有执行档案，每次 agent 行为异常都需要重跑。重跑成本随 LLM 调用次数累积 |
| **P1** | 跨 phase 意图一致性验证 | 方向 A (部分) | 与语义日志可以共享 intent 提取逻辑，增量成本低 |
| **P2** | Phase 产出物 Schema 强制 | 方向 C | 不影响现有 workflow 行为，WARN-only 模式可以逐步建立 |
| **P2** | Core 内部遥测 | 方向 D | 价值需要时间累积（基准快照需要多次运行才有对比意义），且对当前用户无直接影响 |

### 5.2 阶段划分和里程碑

#### Phase 1（Sprint 32-33）—— 基础治理增强

**目标**：建立声明-实现一致性护栏 + 语义日志数据模型

| 里程碑 | 交付物 | 验收标准 |
|---|---|---|
| M1.1 | `forge audit --drift` 命令 | 能扫描所有已知 YAML-Go 映射点，输出结构化漂移报告 |
| M1.2 | CI 集成 WARN 模式 | CI 运行 `forge audit --drift`，漂移仅 WARN 不 FAIL |
| M1.3 | SemanticEvent 数据结构定义 | 在 `internal/trace` 中定义 PhaseCompleted、LoopBackTriggered、ConvergenceVerdict 事件类型 |
| M1.4 | `orchestrator.go` 插入捕获点 | 每个 agent phase 执行后产生一条 PhaseCompleted 语义事件 |

**风险**：Source 注释约定的采用率可能不高。**缓解**：在 `CONTRIBUTING.md` 中约定，并在 CR 中强制执行。

#### Phase 2（Sprint 34-35）—— 执行档案建立

**目标**：语义日志持久化 + `forge log` 查询 + Intent 提取

| 里程碑 | 交付物 | 验收标准 |
|---|---|---|
| M2.1 | trace.ndjson 混合流格式 | 指标事件和语义事件写入同一 ndjson 文件，按 ts 交叉排列 |
| M2.2 | `forge log` 查询命令 | 支持 `--run`、`--phase`、`--event-type` 过滤 |
| M2.3 | INTENT 提取 prompt 模板 | planner phase prompt 结尾添加 `INTENT: ...` 机读段模板 |
| M2.4 | Intent→Delivery 验证 | implementer phase 后检查 git diff 文件列表 vs INTENT 声明 |

**风险**：INTENT 提取依赖 prompt 工程的可靠性。**缓解**：INTENT 段标记为 `optional`，缺失时回退到无验证。同时在 CI 跑 `prompt_template_test.go` 断言 INTENT 段被正确解析。

#### Phase 3（Sprint 36-37）—— 产出物契约 + 自观测

**目标**：emits 验证 + Core 遥测

| 里程碑 | 交付物 | 验收标准 |
|---|---|---|
| M3.1 | Phase 产出物存在性检查 | 所有 `emits` 文件在 phase 结束后检查是否存在，缺失时 WARN |
| M3.2 | Markdown 结构验证规则 | 可配置的关键词/标题检查，规则定义在 workflow YAML 中可选注入 |
| M3.3 | `forge metrics` 命令 | 输出 `loadWorkflow`、`gatherSignals`、`forge accept` 等内部操作耗时 |
| M3.4 | CI 基准快照门控 | benchmark 结果写入 git-tracked 快照，`>20%` 退化告警 |

#### Phase 4（远期）

- 声明契约运行时验证（方向 E）
- 语义日志的 OOM 保护和裁剪策略
- forge log 的 `--sanitize` 脱敏选项
- 多 implementer phase 的意图边界守卫

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| **Source 注释约定不被采用** | 中 | 中 | CR 自动化检查新加常量是否带 `Source:` 注释；CI 中 `forge audit --drift` 扫描未知映射点时告警「未注册声明来源」 |
| **Intent 提取准确率低** | 高 | 低 | INTENT 是 optional 的；低准确率时降级到无验证；通过 `forge accept` 的 ACCEPT/REJECT 驱动 prompt 模板迭代 |
| **trace.ndjson 格式演化导致旧数据不可读** | 中 | 中 | 每行自带 `v` 版本号；consumer 用 version-aware 解析；建立数据迁移路径（`forge migrate trace`） |
| **语义日志 OOM 影响磁盘使用** | 低 | 高 | 10MB cap + 裁剪策略；裁剪优先保留指标事件；提供 `forge trace prune` 和自动 GC |
| **漂移检测告警疲劳** | 高 | 中 | 默认 `--relaxed` 模式只报告关键漂移（gate_set/阈值/路由策略）；`--strict` 模式需主动开启 |
| **相位契约验证与 workflow 现有行为冲突** | 低 | 高 | 契约验证默认 `WARN-only`；`enforce_contract: true` 才阻断；所有现有 workflow 不受影响 |

---

## 6. 综合架构建议

### 6.1 核心洞察

五个方向的共同模式是：**ForgeOS 需要一个「元治理层」**——不是治理 agent 行为，而是治理治理系统自身的完整性。具体而言：

```
当前架构
┌─────────────────────────────────────────────────────────────┐
│  治理层 (modes.yml, policies.yml, check.py, gate.mjs, etc.) │──→ 治理 agent 行为
│  被治理层 (forge-core, workflow exec, agent output)          │──→ 被观测
└─────────────────────────────────────────────────────────────┘

目标架构
┌─────────────────────────────────────────────────────────────┐
│  元治理层 (本文五个方向)                                      │──→ 持续验证治理层自身
│      ├─ 声明-实现漂移检测 (方向五)                              │      是否如实运行
│      ├─ 语义执行档案 (方向一+二)                                │      是否产生预期效果
│      ├─ 产出物契约验证 (方向四)                                 │      是否兑现承诺
│      └─ 内部遥测 (方向三)                                      │      是否有性能恶化
├─────────────────────────────────────────────────────────────┤
│  治理层 (modes.yml, policies.yml, check.py, gate.mjs, etc.) │──→ 治理 agent 行为
│  被治理层 (forge-core, workflow exec, agent output)          │──→ 被观测
└─────────────────────────────────────────────────────────────┘
```

### 6.2 架构决策记录（ADR）

需要记录的架构决策：

| 决策 | 选项 | 选择 | 理由 |
|---|---|---|---|
| 语义日志格式 | 混合流 vs 分离流 vs SQLite | **混合流** | 时间序完整 + 单文件管理；consumer 按 type 过滤即可 |
| INTENT 验证强度 | 阻断 vs WARN-only | **WARN-only** | LLM 提取不可靠时阻断会挫败失败用户体验 |
| 声明-实现对账模式 | 自动发现 vs 预注册表 | **预注册表 + Source 注释** | 自动发现需要 AST 解析，复杂度高且不精确 |
| 自观测实现 | sync/atomic vs channel-based | **sync/atomic** | 零阻塞、零锁、最小开销 |
| schema 验证引擎 | JSON Schema 库 vs 自建 DSL | **自建 DSL** | 保持零外部依赖；Markdown 验证不适合 JSON Schema |

### 6.3 最终建议路线

**按成本/收益排序的最优路径**：

1. **本周**：实现 `forge audit --drift`（方向五的 MVP）——无新依赖、无运行时影响、可立即发现已知缺口。这是成本最低但收益最直接的方向。
2. **Sprint 32**：定义 SemanticEvent 数据结构 + orchestrator 捕获点（方向一的 MVP）——数据模型一旦定义，后续查询命令的实现是增量工作。
3. **Sprint 33**：`forge log` 查询命令 + Intent 提取 prompt 模板（方向一+二）——需要 prompt 工程配合，但与 phase 执行逻辑解耦。
4. **Sprint 34**：Phase 产出物存在性检查（方向四）——简单、低风险、为后续 schema 验证铺路。
5. **Sprint 35+**：Core 内部遥测（方向三）——建立基础设施 + 选关键路径埋点，逐步添加。

五个方向的共同特征决定了一个重要的架构原则：**元治理层的操作成本必须低于被治理层的收益**。也就是说，`forge audit --drift` 必须在 1 秒内完成（否则开发者不愿意运行），语义日志的写入必须在 1ms 内（否则影响执行循环）。元治理不能成为性能瓶颈。

---

*本分析基于 `docs/requirements/five-uncovered-structural-extensions.md`（2026-07-10）及全仓代码扫描结果。所有建议均在 v2 增量范围内实现，不依赖 Firecracker/LiteLLM/外部数据库。*
