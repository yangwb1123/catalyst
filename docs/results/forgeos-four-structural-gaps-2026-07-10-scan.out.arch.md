以下是我基于综合评估报告与 `forge-core/` 实际代码库交叉验证后的架构分析。

---

# 架构分析：ForgeOS `forge-core` 运行时

## 1. 架构评估

### 1.1 当前架构的优势

**1.1.1 零依赖的设计纪律**

`forge-core/` **13 个 Go 包零外部依赖**（`go.mod` 无 `require`），这是当前代码库中最被低估的架构优势。这不是特立独行——它是**经过深思熟虑的载重墙**（见 `north-star.md` 原则第 4 条：能力契约/适配器模式）。在编排层（控制面）不引入任何第三方，意味着：

- **无传递依赖风险**：没有左移依赖版本、没有 CVE 供应链攻击面、没有许可证漂移。
- **编译/构建恒为零配置**：`go build` 在任何 Go 安装上立即可用。
- **架构清晰强制**：不能靠外部框架做依赖注入、ORM、或 RPC 框架来掩盖设计缺陷——每一个抽象都必须是显式的 Go 接口。

**1.1.2 声明式 Workflow 与命令式 Engine 分离**

`asset.Phase` 是纯数据（JSON 承载的声明式描述），`orchestrator.Engine` 是纯行为（状态机执行器）。这种分离是**架构脊柱**的正确选择：

- 资产层（`asset/`）对 Workflow 语义零知识——`Emits`、`FeedsForward`、`DependsOn` 都是扁平的 `[]string`/`bool`，由 engine/CLI 层解释。
- Engine 对资产载体零知识——它接收 `asset.Workflow` 和 `asset.Phase`，不知道 JSON vs YAML vs Protobuf。
- 未来替换 YAML shim（`yaml2json/` Python 脚手架）为原生 Go YAML 库时，资产层接口不变。

这是**依赖方向：interfaces → application → domain 单向向内**红线（`AGENTS.md`）的实际执行。

**1.1.3 注入式回调的测试可测性设计**

`Engine` 结构体的几乎每一个可观察行为都通过注入点暴露：

```go
type Engine struct {
    Exec           AgentExecutor    // 可替换的 Agent 执行器
    RunGate        func(name string) gate.Result  // 可 fake 的闸门
    Log            func(string)     // 可捕获的日志
    OnGateResult   func(name, status string)  // 可断言的回调
    AgentVerdict   func(phase string) (string, bool)  // 可注入的裁决
    BudgetExhausted func() bool     // 可模拟的预算耗尽
    Sleep          func(time.Duration)  // 可 fake 的时钟
    Ctx            context.Context  // 可控制的生命周期
}
```

每一个字段的 `nil` 语义都有明确的向后兼容合约（"byte-for-byte unchanged"），这是**演进式架构**的典范：新功能以 opt-in 接口添加，不改变既有路径的二进制输出。测试文件（`orchestrator_test.go`、`loopback_test.go`、`verdict_loopback_test.go`、`budget_test.go`、`loop_test.go`）共 15+ 文件证明了这套模式的可测试性。

**1.1.4 并发安全的前置设计**

`parallel.go` 的锁顺序合约（8 级锁）从第一天就写进代码注释，而不是"等 race detector 发现再加锁"。`prompt/cache.go` 的 `ContextCache.mu` 也是并行执行器的前瞻防护。这是罕见的设计质量——大部分项目在 v1 的单线程阶段会跳过锁，然后在 v2 并行化时陷入 Heisenbug 调试地狱。

### 1.2 当前架构的局限性

**1.2.1 文件系统级契约的缺失（方向一、二的本质）**

这是评估报告中**最一致的跨方向主题**：Engine 层对文件系统的认知极薄。

| 层面 | 有保护 | 无保护 |
|------|--------|--------|
| 进程内数据结构 | 8 级锁合约 + mutex | — |
| Agent 调用预算 | `MaxAgentCalls` + `MaxLoopBack` + `BudgetExhausted` | — |
| 命令输出大小 | `cappedBuffer` + `MaxOutputBytes` | — |
| 文件写入 | — | 无 pre-snapshot、无冲突检测、无 per-phase 命名空间 |
| 文件产出声明 | `Phase.Emits` 声明存在 | `emitsContext` 只有 read-time WARNING，无 write-time 验证 |
| 总 Prompt 大小 | 各 lane 独立 cap | 无聚合后总量检查 |

这是架构中一个**结构性的不对称**：内存状态有精细的锁合约和预算守卫，文件系统状态却只有"读取时尽量宽容"的被动策略。对于以文件产物为核心（代码、文档、ADR、ROADMAP）的 AI 软件工厂，这是一个逐渐扩大的技术债。

**1.2.2 零值特例的认知负荷（方向三）**

`Engine` 结构体的 10 个字段中，约 8 个有零值特例语义。在 Go 中，零值是合法的默认值，但 Engine 的零值合约有两种本质不同的模式：

| 模式 | 字段 | 语义 |
|------|------|------|
| **哨兵-禁用** | `MaxRetries=0` | 零重试（原始行为） |
| **哨兵-无上限** | `MaxAgentCalls=0` | 无上限（原始行为） |
| **哨兵-跳过** | `ModePolicy=zero` | 不过滤（原始行为） |
| **哨兵-NOP** | `OnGateResult=nil` | 无回调（原始行为） |
| **默认值** | `MaxDepth=0` | 安全默认 2（*非哨兵*） |
| **默认值** | `MaxOutputBytes=0` | 安全默认 10MiB（*非哨兵*） |

关键问题不在于数量（~8 个），而在于**一致性缺失**：同为整数零，`maxRetries=0` 是"禁用"，`maxDepth=0` 是"选默认"。这种不一致对新手贡献者是不透明的，且只能靠注释（"BACK-COMPAT: zero means unbounded"）来维护——注释不编译。

不过，我不同意评估报告中将其降级为 P2 的判断。零值问题的影响面是广泛的（每个新字段都要决定自己属于哪种零值模式），且修复成本极低（`Engine.Validate()` 集中声明）。**我认为这是应该在当前 sprint 中解决的 P1 质量债**，不是因为它在生产中有破坏性，而是因为它降低了整个类型系统的可预测性——是新贡献者犯零值相关 bug 的第一来源。

**1.2.3 配置与代码的耦合**

`ModePolicy` 的 Workflow-depth 输出、闸门过滤、阶段跳过逻辑分布在三个地方：

- `internal/mode/mode_policy.go` — 策略定义
- `orchestrator/mode_gating.go` — 策略消费
- `harness/policies.yml` — 策略数据（YAML）

虽然这在 v1 是可工作的，但策略数据的最终事实源未明确定义。当前只有 `mode.Effective(mode, lifecycle)` 在 Go 代码中硬编码了 mode×lifecycle 矩阵。当需要让策略数据可配置（YAML 驱动）时，会需要一次中等规模的重构。

### 1.3 关键设计决策评价

| 决策 | 评价 | 备注 |
|------|------|------|
| Engine 使用结构体（非接口）+ 注入回调 | ✅ 正确 | Go 惯用、零分配、线程安全（不可变配置，可变状态通过回调） |
| 零外部依赖 | ✅ 正确 | 控制面层的正确选择；数据面可依赖外部工具 |
| 声明式 Workflow + 命令式 Engine | ✅ 正确 | 脊柱分离，两端可独立演进 |
| `RunFrom` 为串行主路径，`RunParallel` 为 opt-in 并行 | ✅ 正确 | 缺省安全、不影响既有 worklow |
| `phaseOutputSummaryCap=800` runes 作 prompt 保护 | ✅ 正确但不足 | 各 lane 独立 cap 到了，但聚合 cap 缺失 |
| Python YAML shim 作 JSON 转码 | ⚠️ 可接受 | 明确的临时方案（`ROADMAP.md` 标注），未来可替换 |

---

## 2. 扩展方向

### 2.1 方向 A：文件系统契约层（File Contract Layer）—— **P1**

**为什么需要**

当前架构对文件系统的认知是对称缺失的：Engine 读出文件（`emitsContext`、`currentTask`、`constraints`）时有宽容的 fault tolerance（文件不存在 = 空字符串），但对写入**没有任何期望管理**。Paralell 模式下多个 agent 写入同一文件的冲突风险是真实的——而且在项目从串行迁移到并行 process 时只会放大。

评估报告将 Parallel 写入冲突列为 P1 + 最强缺口，我同意。但我的修复方案与评估报告不同：**不是加一个"写入冲突检测器"，而是建立一个通用的文件契约层**。

**核心挑战**

1. **无副作用侵入**：Engine 当前零文件写入（只读）。引入写入契约不应让 Engine 突然产生副作用。
2. **与现有声明兼容**：既有 worklow 无 `emits` 声明，必须继续字节不变地工作。
3. **并行下的原子性**：多个 agent 同时写入时的冲突检测需要 pre/post snapshot 比较，这在 Go 中不平凡。

**预期的架构变更**

```
forge-core/internal/
  filecontract/       ← NEW PACKAGE
    contract.go       — 契约定义 (Emits 检查器 + 产物验证器)
    snapshot.go       — pre/post snapshot 比较
    namespace.go      — per-phase 临时命名空间

cmd/forge/
  build_contract.go   — 契约构建（从 Engine 配置注入）
```

- `filecontract.Contract` 是一个注入到 `Engine` 的可选字段（nil = 不检查，向后兼容）。
- `Engine.runAgentPhase` 之后检查 `Contract.Verify(phase, preSnapshot)`。
- 并行模式下每个 phase 获得 per-phase 临时命名空间（git worktree 或临时目录），产物在 wave 完成后移入共享目录。

**对现有系统的影响**

- 纯新增，无既有路径变更。
- `Phase.Emits` 从"声明"变为"可验证的契约"。
- Per-phase 临时命名空间为未来 Sandbox 隔离（`north-star.md` 的 Firecracker 方向）预铺了路径。

### 2.2 方向 B：Prompt 总大小预算器（Aggregate Prompt Budget）—— **P2**

**为什么需要**

评估报告方向四的核心真实缺口是：各 lane 有独立 cap（`memoryCap=32`、`adrTopK=6`、`taskCap=4000`、`phaseOutputSummaryCap=800`），但无聚合后的总量检查。这意味着即使每个 lane 都在限制内，五者累加 + role card + system prompt 仍可能超过模型上下文窗口。对于当前的 Claude 模型（200K tokens）这可能不紧急，但随着（a）workflow 增加更多 context lane、（b）跨厂商池引入不同上下文窗口的模型，这是一个潜在的静默失败。

**核心挑战**

1. **预算模型的选择**：基于 token（需要 tokenizer）/ 基于字符 / 基于 runes？token 最准确但需要依赖；runes 最轻量但过度近似。
2. **超过预算时的行为**：硬裁切（信息丢失）vs WARNING-only（继续可能超窗口）vs 降级模型（换到更大窗口模型）。
3. **与各 lane cap 的关系**：聚合预算不是替代各 lane cap，而是叠加层。

**预期的架构变更**

```go
// internal/prompt/budget.go — NEW
type Budget struct {
    MaxRunes int  // 总 prompt 最大 rune 数；0 = 不检查
    OnOversize func(lanes map[string]int) // 可选的回调（WARNING / 裁切 / 拒绝）
}
```

- 在 `Build()` 或 `GatherCached()` 的末尾注入。
- 各 lane 报告自己的 rune 数，预算器累计后决定行为。

**对现有系统的影响**

- 纯新增，既有 path 不受影响（`Budget{nil}` 或不设置 `OnOversize`）。
- 需要确定 `MaxRunes` 的合理默认值（例如 `maxContextWindow - safetyMargin`）。

### 2.3 方向 C：策略数据外移（Policy as Data）—— **P2**

**为什么需要**

当前 `internal/mode` 的 `Effective()` 硬编码了 mode×lifecycle 矩阵。这在一段时间内是可以工作的，但违背了 `north-star.md` 的第 5 条原则："策略即数据，治理为独立平面（PDP/PEP 分离，OPA式）"。当系统需要：
- 用户在项目级别自定义 mode×lifecycle 矩阵
- 不同项目有不同的闸门集
- 跨租户的策略隔离

时，硬编码的策略矩阵会成为阻塞点。

**核心挑战**

1. **保持零依赖**：Go 标准库没有 YAML 解析器或 policy engine。选择：引入 OPA/Rego（外部依赖）vs 自定义 DSL（自研）vs 策略数据为 JSON（继续用 python shim）。
2. **与既有 `mode.Effective()` 的迁移路径**：不能一次性重构所有消费者。
3. **策略验证**：数据驱动的策略需要自己的 schema 验证（不能等运行时暴露错误）。

**预期的架构变更**

```
.agent/policies/
  routing.yml        — 原 routing/policy.yml（数据化）
  modes.yml          — 原 modes.yml（数据化）
  lifecycle.yml      — lifecycle 矩阵

internal/mode/
  effective.go       — 改为从数据目录加载，回退到硬编码默认
```

- 引入 `PolicyLoader` 接口：`Load() (Policy, error)`，默认实现是硬编码的，可选实现读取 `.agent/policies/`。
- `mode.Effective()` 在 `PolicyLoader` 返回 nil 时回退到当前硬编码。
- 数据文件的 schema 由 `harness/check.py` 验证。

**对现有系统的影响**

- 中间变更：既有代码的 `mode.Effective()` 签名不变。
- 新增 `PolicyLoader` 注入到 `cmd/forge` 的 `buildRunEngine`。
- 迁移路线：硬编码 → 双读取（硬编码 + 文件，WARNING on diff）→ 仅文件。

### 2.4 方向 D：零值合约集中声明（Zero-Value Contract Declaration）—— **P1**

**为什么需要**

评估报告正确识别了零值蔓延问题，但我认为其建议（"step 0: 在 `Engine.Validate()` 集中声明"）**低估了这个方向的价值**。实际上，零值问题不仅影响 `Engine`，还影响 `CommandExecutor`、`LoopEngine`、`asset.Phase` 等多个结构体。集中声明零值合约是一种**架构文档即代码**的做法，它比注释更可测试、比文档更可见。

**核心挑战**

1. **零值语义的治理标准**：什么字段应该用 `0=nil` 哨兵 vs 什么字段应该用 `pointer` + `nil` 跳过？需要确立类型级标准。
2. **Validate() 的幂等性**：`Validate()` 应只报告，不修改状态。
3. **测试兼容性**：零值字段在千百个既有测试中是隐式假设的，`Validate()` 必须与这些假设兼容。

**预期的架构变更**

```go
// Engine.Validate() 新增方法
func (e Engine) Validate() []ContractNote {
    // 返回所有零值特例的声明列表，每个包含：
    // {Field, ZeroMeaning, RiskLevel}
}
```

- 新增 `ContractNote` 类型：字段名、零值语义、风险等级（info/warning/error）。
- `cmd/forge` 在 `--verbose` 模式下打印合约说明。
- 跨包一致：`CommandExecutor.Validate()`、`LoopEngine.Validate()` 同理。

**对现有系统的影响**

- 纯新增，零既有路径变更。
- 为未来的 `forge doctor` 集成提供基础信号。

### 2.5 方向 E：分布式 Trace 与 Cost Attribution 管道 —— **P3**

**为什么需要**

`internal/trace` 已经有一个结构化的 JSONL 事件系统，但 `cost_usd_micros` 和 `model` 字段是"不透明"的——`trace` 不解释它们，只是携带。这意味着：
- 成本归因依赖上层（`cmd/forge`）正确解析 claude JSON 中的 `total_cost_usd`。
- 没有跨迭代的成本聚合。
- 没有一个直接可用于财务审计的成本视图。

**核心挑战**

1. **分离格式与解释**：`trace.Event` 应该保持通用的键值对结构，而不是在结构体上硬编码 `CostUsdMicros` 和 `Model` 字段。
2. **成本归因的精确性**：当使用多模型路由时，每次 LLM 调用的成本需要能被归因到正确的 phase/iteration/agent。
3. **与 `north-star.md` 目标架构的语义对齐**：当前 trace 格式是 v1 的平坦格式，未来需要与 OTel 语义约定对齐。

**预期的架构变更**

```
forge-core/internal/trace/
  event.go      — 通用事件结构（保留）
  model.go      — 模型/成本归因类型（新增或替代硬编码字段）
```

- 将 `CostUsdMicros` 和 `Model` 从 `Event` 结构体中的硬编码字段，转化为 `Attributes map[string]any` 中的通用键。
- 新增 `AttributeKeys` 常量：`AttrCostUsdMicros`、`AttrModel`、`AttrTier`、`AttrProvider`。
- `cmd/forge` 在 `Observe` 回调中填充归因数据。

**对现有系统的影响**

- 结构性变更，但向后兼容（`omitempty` 使新事件格式兼容旧的 JSON 消费者）。
- 需要更新既有 test assertions（约 10+ 个测试文件涉及 trace 输出）。

---

## 3. 接口设计建议

### 3.1 关键模块接口设计原则

**3.1.1 Engine 接口冻结原则**

`Engine` 结构体当前是一个"超级结构体"（16 个字段），但这不是上帝对象——每个字段是一个独立的注入点。这个模式应该保持，但有两个改进：

1. **分组接口**：将关联的注入点分组为子接口，例如：

```go
type GateHooks struct {
    RunGate  func(name string) gate.Result
    OnResult func(name, status string)
}

type BudgetHooks struct {
    MaxAgentCalls   int
    MaxLoopBack     int
    BudgetExhausted func() bool
}

type Engine struct {
    Exec          AgentExecutor
    Log           func(string)
    Gate          GateHooks      // nil-safe
    Budget        BudgetHooks    // nil-safe
    AgentReview   AgentReviewHooks // nil-safe
    Ctx           context.Context
    // ...
}
```

这不是纯语法糖——它让 `NewEngine` 构造函数的参数列表从 16 个可选字段变为 1 个必需字段 + 3 个可选分组，显著改善了可读性。同时，每个分组的零值合约在自己的类型上声明，而不是全部在 `Engine` 上。

2. **所有零值语义必须在该字段/分组的 `Validate()` 中有声明**。

**3.1.2 AgentExecutor 接口的演进**

当前 `AgentExecutor` 接口只有两个实现：`DryRunExecutor` 和 `CommandExecutor`。随着 Sandbox 隔离（`SandboxConfig` 在 `command_executor.go:37-50` 已有骨架）和远程 agent 执行（`north-star.md` 的 Agent Registry & Scheduler），这是需要演进的：

```go
type AgentExecutor interface {
    Execute(ctx context.Context, p asset.Phase, mode string) error
    // 未来可能扩展：
    // Cancel(phase string) error
    // Capabilities() CapabilitySet
}
```

**但不要过早抽象**：当前两个实现之间的差异已经很大（一个零输出，一个有输出大小限制、重试、超时），再加一个远程执行器可能会导致接口泄漏实现细节。等到第三个实现实际上需要时再抽象。

### 3.2 是否需要新的抽象层

**3.2.1 需要：FileSystem 抽象层**

当前文件读取分散在 4 个包中：

| 包 | 文件读取 | 路径 |
|-----|-----------|------|
| `prompt/prompt.go` | `os.ReadFile` | `.agent/ROADMAP.md`、`.agent/AGENTS.md`、`docs/adr/*.md` |
| `prompt/cache.go` | `os.ReadFile` | `.agent/agents/*.md` |
| `prompt/retrieve.go` | `os.ReadFile` | ADR 文件 |
| `asset/asset.go` | `os.ReadFile` | workflow JSON |

引入 `fsys.FS` 接口（Go 1.16+ 的 `io/fs`，或自定义的轻量接口）将为测试注入 fake filesystem 提供路径，并且为未来的 Sandbox 文件系统隔离铺路：

```go
// 在 internal/fsys 或已有 internal/persist 中
type FS interface {
    ReadFile(name string) ([]byte, error)
    WriteFile(name string, data []byte, perm os.FileMode) error
    ReadDir(name string) ([]os.DirEntry, error)
    // Stat 用于 emits 验证
    Stat(name string) (os.FileInfo, error)
}
```

**这不需要成为当前 sprint 的工作**，但应该在设计文档中作为已知方向记录，使得未来引入时不会与既有代码冲突。

**3.2.2 不需要：网关模式（Gateway Pattern）**

不要把 "File Contract Layer" 设计成一个"文件读写网关"。当前 Engine 的文件系统接触点是点状的、只读的，且目前不需要集中的读写仲裁。Per-phase 临时命名空间 + post-execution 产物验证就足够了，不需要全局文件网关。

### 3.3 向后兼容性策略

当前代码库对向后兼容有极好的纪律（每个新字段的文档都包含"byte-for-byte unchanged"保证）。以下原则应继续：

1. **永远使用指针语义作 optional 行为**：`*OnFail`、`*WritesADR`、`*SandboxConfig`。这使 nil = 跳过 = 旧行为。
2. **非指针零值用作哨兵时，必须附加 doc 注释**：且 `Validate()` 在 `--verbose` 下打印该注释。
3. **新接口默认 nil-safe**：如 `OnGateResult`、`AgentVerdict`、`BudgetExhausted`的模式。
4. **JSON `omitempty` 标记必须谨慎使用**：`Phase.FeedsForward` 使用 `bool` + 无 `omitempty`，因为 false 是有语义的（"不 feed forward"）。但 `Phase.Emits` 使用了 `omitempty`，因为 nil 和 empty 在语义上等价（"不 emit"）。

---

## 4. 技术选型

### 4.1 需要引入的新技术栈

**谨慎推荐：`go-yaml` 仅在 yaml2json shim 替换时**

`forge-core/` 当前通过 `yaml2json/` Python shim 将 YAML workflow 转码为 JSON。这明确标记为临时方案（`ROADMAP.md`："未来可换 Go YAML 库——属 architect/cto 的依赖决策"）。

- **时机**：当前不应替换。零依赖是 v2 的正确状态。Python shim 的可维护性风险是低的（约 500 行 Python，12 个测试）。
- **候选**：`gopkg.in/yaml.v3`（最广泛使用，稳定性高）vs `go-yaml/yaml` v2（数据结构支持更原生）。
- **权衡**：引入 go-yaml 将 forge-core 从"零依赖"变成"一依赖"——虽然只有一个，但这是架构原则的改变。在 v3（跨厂商、完整 TAD）阶段引入是合理的；在 v2 中期引入需要明确理由。

**不推荐：OPA/Rego**

`north-star.md` 列出了 OPA/Rego 作为 PDP 引擎的候选。对于 v2，这是过早抽象。当前硬编码的策略矩阵（`mode.Effective`）加上文件数据化（方向 C）足以覆盖未来 2-3 个 sprint 的需求。OPA 应在策略需要：
- 跨租户的动态加载
- 用户自定义策略 DSL
- 实时策略变更

时再引入。这不是 v2 的目标。

### 4.2 第三方依赖评估标准

如果未来必须引入依赖，以下是 ForgeOS 的具体评估标准（继承自零依赖纪律）：

| 标准 | 要求 | 原因 |
|------|------|------|
| 传递依赖数 | ≤ 3 | 控制面不允许左移依赖树 |
| Go 版本要求 | 当前 Go 版本 + 1（保守） | 避免强迫用户升级 Go |
| 许可证 | MIT / BSD / Apache-2.0 | GPL 禁止（治理层不能污染用户项目）|
| API 稳定性 | Go 1 兼容承诺 | 不得有 breaking change 在 minor 版本 |
| 维护活跃度 | 最近 12 个月内有提交 | 不能引入"搁置依赖"到控制面 |
| 测试覆盖 | 依赖自身的测试覆盖 ≥ 80% | 低测试覆盖的依赖是风险传递 |

当前的 YAML shim 是一个特例：它不是 `go.mod` 中的依赖，所以不适用这些标准。但如果要替换为 Go YAML 库，以上标准适用。

### 4.3 自建 vs 采购

| 组件 | 建议 | 理由 |
|------|------|------|
| File Contract Layer | 自建 | 领域特定（AI agent 产物的语义契约），无现成库 |
| 策略引擎（PDP） | 暂不自建 | v2 用文件数据化；v3 再评估 OPA |
| Sandbox 隔离 | 采购（Firecracker/Docker） | `north-star.md` 已定：采购隔离层，自研编排 |
| YAML 解析 | 自建 shim（保持） | Python shim 已在 ROADMAP 标注为临时；Go YAML 库是唯一可接受的替代，但 v2 不应急于引入 |
| 分布式 trace 后端 | 采购（OTel + Grafana） | `north-star.md` 已定；自研事件格式（`trace.Event`）用于本地调试，OTel 管道用于生产 |

---

## 5. 实施路线图

### 5.1 优先级矩阵

| 方向 | 优先级 | 评估 | 影响范围 | 风险 | sprint 估算 |
|------|--------|------|----------|------|------------|
| D：零值合约集中声明 | **P0** | 修复成本极低，收益广泛，为其他方向铺路 | `internal/orchestrator`、`internal/asset`、cmd/forge | 无（纯新增） | 0.25 |
| A：文件系统契约层 | **P1** | 最强缺口，填补架构对称性缺失 | `internal/filecontract`（新）+ `internal/orchestrator` | 中等（并行模式的原子性保证复杂） | 1.5 |
| C：策略数据外移 | **P2** | 治理合规方向，但当前硬编码可工作 | `internal/mode`、`.agent/policies/`、`cmd/forge` | 低（有回退路径） | 1.0 |
| B：Prompt 总大小预算 | **P2** | 代码质量方向，防止未来静默失败 | `internal/prompt` | 低（纯新增） | 0.5 |
| E：分布式 Trace 管道 | **P3** | 观察性方向，在 v3 之前完成即可 | `internal/trace`、`cmd/forge` | 中（需要更新既有 test） | 1.0 |

### 5.2 阶段划分

**阶段 1 — 地基（当前 sprint，0.5 sprint）**

目标：为所有后续方向建立制度基础。

1. **零值合约集中声明**（方向 D）
   - `Engine.Validate()` → `[]ContractNote`（~15 行）
   - `CommandExecutor.Validate()` + `LoopEngine.Validate()`
   - `cmd/forge --verbose` 打印合约说明
   - 为 `ContractNote` 类型编写 2-3 个测试
   - **满足 P0：改动最小、收效最直接**

2. **新增 `fsys.FS` 接口声明**（不作为使用，仅作为设计文档 + Go 接口定义）
   - 在 `internal/persist/` 中新增 `fs.go`，包含 `FS` 接口 + 文件注释记录为何需要它
   - **不改动任何既有消费者**

**阶段 2 — 文件契约层（下一 sprint，1.5 sprint）**

目标：填补架构中最严重的文件系统缺口。

1. **`filecontract` 包骨架**
   - `Contract` 结构体（nil = 不检查）
   - `Snapshot(repoRoot)` → `pre-state`（记录 emits 声明的文件是否存在、大小、修改时间）
   - `Verify(phase, preState)` → `[]ContractViolation`（缺失的文件、大小不匹配、意外创建的文件）
   - `utils_test.go` 中基于临时目录的测试

2. **`Engine` 集成**
   - `Engine.FileContract *filecontract.Contract` 字段（nil-safe）
   - `RunFrom`: 在 `runAgentPhase` 前 `Snapshot`，后 `Verify`
   - `RunParallel`: 为每个 phase 创建 per-wave 临时命名空间（`mktemp -d` + 软链产物）

3. **Per-phase 临时命名空间的成本与安全考虑**
   - 临时目录在 wave 完成后清理（`defer os.RemoveAll`）
   - 产物通过原子重命名（`os.Rename`）移入共享目录
   - 记录在 `trace.Event` 中以便调试

**阶段 3 — 策略数据化（再下一 sprint，1.0 sprint）**

目标：使策略数据可配置，为多项目复用铺路。

1. **`.agent/policies/modes.yml` 结构设计**
   - 从 `internal/mode` 中已有的测试数据（`mode_test.go` 中的示例值）导出 YAML schema
   - `harness/check.py` 中新增 schema 验证规则

2. **`PolicyLoader` 接口 + 默认硬编码实现**
   - `mode.Effective()` 接收可选的 `PolicyLoader`
   - 文件加载失败时回退到硬编码（日志 WARNING）
   - 双运行模式下（文件 + 硬编码）diff 检测

3. **数据迁移验证**
   - 从 `mode_test.go` 的值反向生成 `.agent/policies/modes.yml`
   - 验证 `mode.Effective` 在文件加载模式下输出与硬编码一致

**阶段 4 — Prompt 预算 + Trace 管道（持续，1.5 sprint）**

1. **Prompt 总大小预算器**（0.5 sprint）
   - `budget.go`：累加各 lane rune 数
   - `Build()` 末尾的警告（超预算但继续）/ 硬裁切（按比例）选择
   - 连接到 `trace` 事件系统

2. **Trace Event 属性泛化**（1.0 sprint）
   - 将 `CostUsdMicros`、`Model` 从 `Event` 结构体移到 `Attributes map[string]any`
   - 保留兼容的 getter 方法
   - 更新 10+ 个 test 文件

### 5.3 风险与缓解

| 风险 | 可能性 | 影响 | 缓解策略 |
|------|--------|------|----------|
| 文件契约层与既有 worklow（无 emits 声明）不兼容 | 低 | 中 | `Contract{nil}` 是缺省值，既有 worklow 完全不被检查 |
| Per-phase 临时命名空间带来性能开销 | 中 | 低 | 当前 phase 执行时间以秒/分钟计，文件复制/移动的毫秒级开销可忽略；仅在 `--parallel` 模式下启用 |
| 策略数据化后，`mode.Effective()` 的输出与文件加载不同 | 中 | 高 | 双运行模式：文件加载 + 硬编码同时计算，diff 检测输出 WARNING 但不阻断（fail-open）；CI 中应 fail-closed |
| 评估报告与代码的差异（方向四已部分修复）导致路线图基于过时信息 | 低 | 中 | 本分析已逐一交叉验证；方向四已从"无防护"调整为"聚合预算缺失"，sprint 估算从 0.5 降至 0.25 |
| Zero-Value Validate() 成为维护负担 | 低 | 低 | Validate() 是纯声明式的（return `[]ContractNote`），不需要解析或分析；新增字段时只需增加一行声明 |

### 5.4 关键决策点一览

| 决策 | 推荐 | 备选 | 决策时间点 |
|------|------|------|-----------|
| 文件契约层放在 `internal/filecontract/` vs 嵌入 `internal/persist/` | `internal/filecontract/` 独立包 | `internal/persist/`（但该包现在的职责是 checkpoint/fault/replay，与契约语义正交） | 实施前 |
| Per-phase 命名空间：git worktree vs `mktemp -d` | `mktemp -d` + 软链 vs 原子重命名 | Git worktree 原子性更好但慢，且在非 git 仓库中不可用 | 实施时评估 |
| 策略数据格式：YAML vs JSON vs TOML | YAML（与既有 workflow 资产一致）但通过 python shim 加载 | JSON（Go 原生解析但人工可读性差）| 阶段 3 开始时 |
| Prompt 预算器超过预算时的行为：WARNING vs HARD CUT | **WARNING**（仅日志，不阻断） | HARD CUT（拒绝构建 prompt）对于 v2 过于激进，agent 阶段已消耗了预算 | 阶段 4 设计时 |
| Trace 属性泛化的向后兼容：getter 方法 vs 消费者直接读 map | **Getter 方法**（`Event.CostUsdMicros() int64`），内部用 map 实现 | 直接暴露 map 并冻结 map key 常量 | 阶段 4 设计时 |

---

## 总结

ForgeOS `forge-core` 是一个**异常高质量**的代码库。零依赖纪律、注入式回调节构、声明式/命令式分离，以及"byte-for-byte unchanged"向后兼容文化的结合，是 Go 微服务架构中的罕见水平。四个方向的技术债——文件系统契约缺失、零值语义蔓延、prompt 聚合预算缺失、策略数据耦合——都在可管理范围内，且修复路径明确。

我最推荐立即执行的行动是：

1. **零值合约集中声明**（P0，0.25 sprint）——这是改动最小、收益最高的地方，并且为所有后续方向建立制度基础。
2. **文件契约层**（P1，1.5 sprint）——填补架构中最严重的对称性缺口。这是独立且有可见价值的交付物。
3. **策略数据化**（P2，1.0 sprint）——治理合规的中间里程碑，使得 ForgeOS 可以真正治理非自身项目。

这三个方向按顺序执行将使 `forge-core` 从"为自身治理而设计"演进到"为治理任意项目而设计"——这是从 v2 到 v3 的关键转型。
