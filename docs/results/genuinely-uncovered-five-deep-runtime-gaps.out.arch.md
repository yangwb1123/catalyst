# 架构师分析：ForgeOS 运行时边界盲区与演进方向

> **基于**: `2026-07-12-five-runtime-boundary-blindspots.md`（原始分析）+ `2026-07-12-five-runtime-boundary-blindspots.out.md`（审阅响应）
> **视角**: 跨层架构评估，不限于五个方向本身，而是审视其揭示的系统级结构性命题

---

## 一、架构评估

### 1.1 当前架构的本质优势

ForgeOS 的核心架构决策——**治理层（harness）与执行层（agent CLI）的分离**——在当前代码库中得到了比文档表面更彻底的贯彻。五个盲区的共同特征是「机制存在但边界未覆盖」，这恰恰是**架构分层成功**的副作用：多数维度已在正确层级完成隔离，唯有少数几个点因历史原因被穿透。

具体而言，以下几点是值得长期守护的架构决策：

| 决策 | 体现 | 价值 |
|------|------|------|
| 四维资源护栏（深度/数量/时间/内存）在 forge-core 层独立实现 | `executor.go`, `budget.go` 中的 `AgentExecutor` 和 `checkRunBudget` | 治理层不依赖 vendor，可测试可审计 |
| 存储分离（checkpoint/trace/memory）的写入隔离 | 三个独立模块各自管理单一文件格式 | 单文件一致性可独立保证，故障范围受控 |
| Kahn 拓扑排序的 wave 抽象 | `waves.go:45-70` 的 `depends_on` + 拓扑排序 | 声明式计算依赖顺序，执行引擎对 phase 拓扑零假设 |
| 锁排序合约的显式文档化 | `parallel.go:25-50` 自声明覆盖范围 | 将「已知未覆盖区」写入代码注释，是架构诚实的最佳实践 |

### 1.2 架构局限——从五个盲区看本质问题

五个盲区虽然各自独立，但揭示了三个更为根本的结构性短板：

#### 短板 A：资源维度的一等公民不均等

当前五个资源维度（深度/数量/时间/内存/成本）中，前四个在 forge-core 层有**原生类型/接口**来表达和强制，而成本维度仅有 `--agent-max-budget-usd` 字符串参数透传。这不是偶然的——成本维度的挑战在于：

- **它不是 forge-core 自有资源**：深度和数量是 forge-core 内部计数的，时间是 context 传递的，内存是 output bytes 自有的。但成本是 agent CLI 执行的结果，forge-core 需要**事前预测**和**事后测量**两种能力
- **它需要调用链路的双向信息**：forge-core 需要知道「这个 phase 预计花多少钱」才能做 pre-Execute 拦截，但 forge-core 目前没有 phase 级成本估算模型
- **它涉及领域外知识**：不同模型（Opus vs Sonnet vs 自定义）的单位成本不同，forge-core 需要知道这些映射关系

**架构含义**: 这意味着需要引入一个新的抽象——`CostModel`——将成本从「透传参数」提升为「一等公民资源类型」。

#### 短板 B：phase 边界只有「进入」契约，没有「退出」契约

ForgeOS 对 phase 生命周期有精确的进入控制（权限注入、prompt 构建、budget check），但 phase 退出时**没有任何结构化的产出声明验证**。这不是 `emits` 的孤立问题——phase 退出时产生的所有副作用（文件创建、git 变更、cost 消耗、trace 记录）都没有被系统地检核是否符合 phase 执行前的声明。

对比典型的 actor 模型：actor 发送消息前可以校验消息 schema，但 ForgeOS 的 phase 在「发送」自己的产出前不做任何 schema 校验。

**架构含义**: 这意味着需要引入**Phase Lifecycle Hook 系统**——在 phase 进入（`prePhase`）和退出（`postPhase`）时执行可注册的校验链。

#### 短板 C：三个存储之间的「信任差」

三存储的写入隔离本身不是问题——单一职责是好设计。但缺乏**交叉引用机制**使得在故障恢复后无法回答一个基本问题：「这三个存储是否代表同一个时间点的一致视图？」

这其实是一个**分布式系统领域常见的「快照隔离」问题**——三个独立的 append/overwrite 流之间没有 `sync` 屏障。当前架构假设每个存储独立就是够的，但对于故障恢复场景，这个假设不成立。

**架构含义**: 需要引入轻量级的**一致性标记**（epoch/seq number）作为三存储的隐式事务 ID，不破坏现有的写入隔离。

### 1.3 架构债务与技术债

按严重程度排序：

1. **成本维度的供应商耦合**（方向一）——**P0 债务**。架构承诺的多厂商治理在此维度被单 vendor 实现穿透。虽然 claude 是主要目标，但架构文档承诺的「治理层独立于底层 CLI」在此处不真。

2. **emits 语义的二象性**（方向二）——**P1 债务**。`emits` 同时被用作「声明」（告诉下游会产出什么）和「权限」（告诉 agent 可以写哪些目录），但两者都没有验证。这使 phase 的「契约面」只有声明没有校验，是典型的语义欠载（semantic underloading）。

3. **Budget 数据模型存在但接口未暴露**——**P1 债务**。审阅发现的 `PhaseBudget` 结构体已存在于 `internal/budget/budget.go`，但 `AgentExecutor` 接口未引用它。这是「未使用的抽象」代码债务——要么移除，要么提升到接口层。

4. **并行路径的 maxParallel 使用 GOMAXPROCS**——**P2 债务**。`parallel.go:96-98` 用 CPU 维度约束 IO-bound 并行，是维度错配。虽因 `depends_on` 未启用而休眠，但一旦启用就会暴露。

5. **checkpoint 备份文件名无时间戳**——**P2 债务**。`checkpoint.go:~125` 的 `.bak1`/`.bak2` 滚动备份无法与 trace/memory 的时间锚点关联，限制了运维恢复场景。

---

## 二、扩展方向

以下列出四个高价值架构扩展方向，其中**方向 A 和 B 是五个盲区的自然收敛**，方向 C 和 D 是更上层的结构演进。

### 方向 A：资源维度的一等公民化（五盲区收敛）

**为什么需要**：五个盲区的根因之一是资源维度表达力的不均等。将成本、存储一致性、输出契约都提升为与深度/数量/时间/内存同等级别的「系统资源」，就能天然消除盲区。

**核心挑战**：

- 资源维度的类型系统设计——需要定义 `ResourceDimension` 接口，且不同维度的强制策略可组合
- 成本维度的预估需要外部知识（模型定价），引入领域知识到 forge-core
- 向后兼容——现有 `--agent-max-budget-usd` CLI 参数必须继续工作

**预期架构变更**：

```
AgentExecutor (现有接口)
├── Execute(ctx, phase, mode) → error       # 已有
├── SetCostCap(usd float64)                  # 新增 — 将成本提升到接口层
├── SetOutputContract(declared []string)     # 新增 — 将 emits 验证纳入契约
└── BudgetWatermark() → (level string)       # 新增 — 预算水位感知
```

同时引入 `ResourceGuard` 接口：

```go
type ResourceGuard interface {
    Check(ctx context.Context, phase Phase, state RunState) error
}
```

目前的 `checkRunBudget` 是隐式的 ResourceGuard——提升为显式接口后，5 个维度的检查器可独立注册，且可逐个启用/禁用。

**对现有系统的影响**：

- 低侵入——`AgentExecutor` 接口的扩展是**加法**，不影响已有实现
- `CommandExecutor` 的 `Build` 函数字段需要新增成本估算方法签名——这是主要变更点
- `routing.go` 的 `BudgetAdjustTier` 可作为成本维度的第一个 ResourceGuard 实现

---

### 方向 B：Phase 生命周期契约（Phase Contract Protocol）

**为什么需要**：当前 phase 的生命周期只有「进入时有权限声明」，退出时没有任何结构化校验。引入退出契约后，emits 验证、成本审计、文件系统影响测量都可以以统一的「Phase Contract」框架表达。

**核心挑战**：

- 退出阶段的校验不能阻塞关键路径——失败如何降级（fail-open vs fail-closed）需要配置
- emit 文件的存在性检查必须考虑延迟（agent 可能异步写文件），引入时间窗口
- 与 converge 信号的关系——哪些契约违反应触发收敛信号

**预期架构变更**：

```
执行流程变更: prePhase → Execute → postPhase (新增)

postPhase(ctx, phase, execResult) → ContractReport
  ├── EmitsVerifier: 检查声明的 emits 路径是否存在
  ├── CostVerifier: 对比实际成本与预算上限
  ├── FileChangeVerifier: git diff 检测非声明文件的创建
  └── TraceVerifier: 确认 phase 的 trace 事件已正确记录

ContractReport → converge 信号转换 (当 report.MissingEmits > 0 时触发)
```

**对现有系统的影响**：

- 中等侵入——需要修改 `loop.go` 和 `parallel.go` 的 phase 执行流程，增加 post-execution hook
- `prompt_artifacts.go:37-55` 的 WARNING 逻辑可迁移到 EmitsVerifier 中，使其不再是旁路日志
- 对 `engine_build.go:197-201` 的 readonly narration 中的 emits 路径，可在 postPhase 中校验这些路径是否实际被写入

---

### 方向 C：分段式快照一致性（Snapshot Isolation for Three Stores）

**为什么需要**：三存储一致性问题本质上是分布式系统中「无协调的写入流之间的视图一致性问题」。引入轻量级一致性标记可以在不破坏写入隔离的前提下提供故障恢复时的信任基础。

**核心挑战**：

- 一致性标记的生成点选择——必须与 phase 边界对齐（iteration start/end、converge point），不能每次 trace 写入都同步 checkpoint
- 标记的跨存储传播——checkpoint 写入时读取 trace 和 memory 的当前 seq，但 trace/memory 是仅追加的，不存在「回滚」概念——标记的含义是「截至此 seq 的数据是 checkpoint 的有效视图」
- 恢复阶段的校验算法——如何高效比对 checkpoint seq 与 trace/memory 的实际行数

**预期架构变更**：

```go
// checkpoint.json 新增字段
type Checkpoint struct {
    // ... 现有字段 ...
    Epoch struct {
        Iteration uint64    `json:"iteration"`
        TraceSeq  uint64    `json:"trace_seq"`    // trace.jsonl 行数
        MemorySeq uint64    `json:"memory_seq"`   // memory.jsonl 行数
        Timestamp time.Time `json:"timestamp"`
    } `json:"epoch"`
}
```

`forge doctor` 扩展为三存储交叉审计命令：

```bash
forge doctor --cross-check
# CHECK:  checkpoint epoch at trace_seq=472, trace.jsonl has 475 lines (3 trailing events)
# WARN:   3 trace events after last checkpoint epoch — normal if crash occurred
# CHECK:  memory_seq=89 matches memory.jsonl (89 lines)
```

**对现有系统的影响**：

- 低侵入——仅 checkpoint 结构体增加字段，`persist.Save` 在写入前收集 `trace.Count()` 和 `memory.Count()`（两者都已有近似实现）
- 无新依赖——`trace.go` 和 `memory.go` 不需要修改写入逻辑，只需暴露 `Count() int` 方法
- `forge doctor` 的扩展是正交的——不影响核心执行路径

---

### 方向 D：自适应并行资源调度（Adaptive Wave Scheduling）

**为什么需要**：当前 wave 调度只考虑 `depends_on` 拓扑约束，不感知任何资源维度。当并行真实启用（未来有 workflow 使用 `depends_on`），连 CPU 维度也用 GOMAXPROCS 近似处理。需要一个显式的资源调度层。

**核心挑战**：

- 资源维度的选择——进程数、文件描述符、API rate limit、磁盘 I/O 带宽，哪些是 wave 调度的核心约束？
- 动态适应性——wave 开始时无法精确预知其子进程的资源消耗，需要运行时反馈（如 `ulimit -n` 的实时剩余值）
- 与传统调度器的关系——Go 运行时有自己的 goroutine 调度器，wave 调度应聚焦于**外部资源**（进程、FD、API 配额）

**预期架构变更**：

```go
type WaveResourceConstraint struct {
    MaxProcesses     int     // ulimit -u 扣除
    MaxFileHandles   int     // ulimit -n 扣除
    MaxAPIRate       float64 // requests/min, 由 AgentExecutor 报告
    MaxConcurrentWrites int  // 文件系统写冲突预算
}
```

`parallel.go` 的 `maxParallel` 计算从 `GOMAXPROCS` 替换为：

```go
func (e *Engine) waveConcurrency(wave Wave, constraints WaveResourceConstraint) int {
    limiters := []int{
        constraints.MaxProcesses,
        constraints.MaxFileHandles / avgFilePerPhase,
        constraints.MaxConcurrentWrites,
        runtime.GOMAXPROCS(0), // CPU 作为最低保底
    }
    return min(limiters...)
}
```

**对现有系统的影响**：

- 中等侵入——主要影响 `parallel.go` 的 `runWave` 逻辑和 `command_executor.go` 子进程生成
- 需要引入新的系统级查询（`ulimit` 读数、文件描述符占用量），这些在 `command_executor_unix.go` 已有部分实现可扩展
- `waves.go` 的拓扑排序逻辑不变——这是正交的

---

## 三、接口设计建议

### 3.1 关键接口设计原则

基于五个盲区的分析，以下接口原则应该是架构级约束：

**原则一：每个资源维度必须有对应的接口级抽象**

当前：`MaxDepth` → `Executor` 的深度检查 ✓
当前：`--agent-max-budget-usd` → 字符串透传 ✗

修复方向：引入 `CostCapper` 接口：

```go
type CostCapper interface {
    CostCap() float64        // 返回当前执行的成本上限（USD）
    SetCostCap(cap float64)  // 设置该上限
    CostEstimate() float64   // 返回对本次执行的成本预估
}
```

`AgentExecutor` 组合此接口，使得成本维度与其他四个维度处于同等抽象层级。

**原则二：phase 必须是「双面契约」——有进入卡也有退出卡**

当前 phase 的声明（`Emits`, `Permissions`, `Timeout`）只有进入阶段被读取，退出阶段不做校验。

修复方向：`PhaseContract` 作为 phase 的完整契约描述：

```go
type PhaseContract struct {
    Entry  PhaseEntryContract  // 执行前——已有（权限、超时、保留）
    Exit   PhaseExitContract   // 执行后——新增（emits 验证、成本范围、文件范围）
}
```

**原则三：故障恢复应提供「一致性视图」而非「尽力恢复」**

当前 `resume` 机制只读 checkpoint，不验证 trace 和 memory 的一致性。

修复方向：在 `Engine.Resume` 中增加轻量级校验步骤——不阻止 resume，但记录差异到 trace，使得 operator 有数据辅助判断。

### 3.2 是否需要引入新的抽象层

**需要引入一个「轻量级」中间层：Phase Lifecycle Manager**

当前的生命周期逻辑散布在 `loop.go`（串行路径）、`parallel.go`（并行路径）、`executor.go`（执行接口）中。五个盲区中有三个（方向一、二、四）涉及「在 phase 执行前后插入校验逻辑」，当前架构要求每个新校验都直接修改 `loop.go` 或 `parallel.go`。

引入 **Phase Lifecycle Manager** 作为「phase 执行中间件链」：

```
现状:  loop.go → executor.Execute → loop.go（无扩展点）
目标:  loop.go → PhaseLifecycleManager(preExecHooks → Execute → postExecHooks) → loop.go
```

hooks 可独立注册：

```go
type PhaseLifecycleManager struct {
    preHooks  []PhasePreHook    // budget check, cost cap config, etc.
    executor  PhaseExecutor
    postHooks []PhasePostHook   // emits verification, cost audit, etc.
}
```

**但建议审慎**——这是有成本的抽象。如果 only one or two hooks，直接在 `loop.go` 添加校验调用比引入中间件更务实。**建议条件是≥3 个不同的 pre/post hook 时引入此抽象层**，在此之前保持扁平。

### 3.3 向后兼容性

五个方向的变更全部是**加法**：

| 方向 | 变更类型 | 向后兼容策略 |
|------|---------|-------------|
| 方向一（成本护栏） | `AgentExecutor` 接口扩展 | 新增方法有缺省实现（`SetCostCap` 默认 nocap），不要求已有实现变更 |
| 方向二（emits 验证） | phase 生命周期钩子 | Post-exec 验证的结果默认只记录 trace/warn，不阻断执行流（可配置 opt-in fail-closed） |
| 方向三（一致性标记） | checkpoint 结构体扩展 | 新字段为 `omitempty`，旧 checkpoint 读取时字段为零值，逻辑回退到当前行为 |
| 方向四（梯度决策） | Engine 配置扩展 | 水位阈值默认关闭（disabled），用户 opt-in 启用 |
| 方向五（资源调度） | parallel 计算逻辑 | 默认行为回退到当前 GOMAXPROCS 逻辑，新调度模型在启用约束时才激活 |

**核心原则**：所有新行为默认不改变现有用户的运行结果（no behavioral change by default）。

---

## 四、技术选型

### 4.1 是否需要引入新的技术栈或框架

**不需要**。五个方向所涉及的技术栈全部在现有 ForgeOS 生态内：

| 方向 | 所需技术 | 已有覆盖 | 是否需新依赖 |
|------|---------|---------|-------------|
| 成本护栏 | float64 预算 + 字符比较 | `PhaseBudget`, `checkRunBudget` | ❌ 纯 Go 标准库 |
| emits 验证 | 文件存在性 + git diff | `os.Stat`, `os/exec git diff` | ❌ 纯 Go 标准库 |
| 一致性标记 | JSON 序列化 + uint64 计数 | `checkpoint.go`, `trace.go`, `memory.go` | ❌ 纯 Go 标准库 |
| 梯度决策 | 阈值比较 + 回调注册 | `BudgetExhausted` 回调模式可复用 | ❌ 纯 Go 标准库 |
| 资源调度 | ulimit 系统调用 + 资源估算 | `command_executor_unix.go` | ❌ 纯 Go 标准库 |

**这是一个值得骄傲的事实**——五个盲区的修复不需要任何外部依赖。这不仅降低了交付风险，也维持了 forge-core「零外部依赖」的架构红线。

### 4.2 第三方依赖的评估标准（未来场景）

虽然当前不需要新依赖，但成本维度揭示了未来可能需要的外部信息——**模型定价数据**。

如果 forge-core 需要内置每模型每 token 成本表（Opus vs Sonnet vs Haiku vs 自定义模型），有三种路线的权衡：

| 选项 | 方案 | 优势 | 劣势 |
|------|------|------|------|
| **A. 硬编码成本表** | 在代码中维护 `ModelCostMap` | 零依赖、可离线工作 | 价格变动需代码更新 |
| **B. 配置文件映射** | `.forge/cost-mapping.yaml` 中用户维护 | 灵活、用户自定义 | 维护负担转移给用户 |
| **C. 运行时查询接口** | agent CLI 返回自己的成本估算（`--response-cost-json`） | 最精确、vendor 知道自己的定价 | 需要 agent CLI 支持，回到 vendor 耦合问题 |

**架构师建议**：采用 **A + C 两阶段**。先硬编码常见模型的公开定价表（选项 A），同时在 `AgentExecutor` 接口上预留 `CostEstimate()` 方法供未来 vendor 提供精确值（选项 C 的接口预留），用配置文件的成本倍率因子（选项 B 的轻量版）覆盖自定义模型。

### 4.3 自建 vs 采购的决策依据

五个方向全是自建——这是正确的决策，原因：

- 每个方向都**高度耦合于 ForgeOS 内核的 phase/engine/budget/parallel 概念**，外部库不可能理解这些领域边界
- 复杂度低（每个方向 ~100-300 行核心逻辑），不值得外部依赖带来的版本协商和 API 映射成本
- forgo-core 的「零外部依赖」红线是经过验证的正确决策——它使 forge-core 的构建、测试、部署一步到位

唯一的「采购豁免」场景是：**如果未来引入非 LLM agent（如 bash agent、API agent），其成本模型与 LLM 完全不同，可能需要一个 `CostModelProvider` 接口**，允许外部系统注入成本估算逻辑。但这时也是接口设计问题，不是采购问题。

---

## 五、实施路线图

### 5.1 优先级排序（P0/P1/P2）

基于审阅调整后的排序：

| 层级 | 方向 | 工作量 | 风险 | 收益 | 工期 |
|------|------|--------|------|------|------|
| **P0** | 方向三 · 一致性标记 | ~0.5 sprint | 低——纯加法，不破坏现有路径 | 高——24h+ 长跑的数据可信度底线 | 3-5 天 |
| **P0** | 方向一 · 厂商无关成本护栏 | ~1 sprint | 中——需要接口扩展 + 拦截点定位 | 高——五个资源维度中唯一依赖 vendor 的缺口 | 5-8 天 |
| **P1** | 方向二 · emits 验证 | ~1 sprint | 低——纯加法，先 warn-only 再 fail-closed | 中高——治理完整性的重要补充 | 5-7 天 |
| **P2** | 方向四 · 预算水位告警（降级版①） | ~0.5 sprint | 低——trace event at 20%/10% | 中——大幅改进 UX 但非安全缺口 | 3-4 天 |
| **P3** | 方向五 · 资源调度 + 方向四完整梯度 | ~2 sprints | 高——依赖并行启用 + 可能的设计迭代 | 中——取决于并行真实使用量 | 待并行启用 |

① 方向四推荐按两阶段交付：第一阶段只加水位告警 trace event（低成本 UX 改进）；第二阶段再实现完整的梯度降档级联（需要更多设计验证）。

### 5.2 阶段划分和里程碑

#### Phase 1：「数据信任基础」（Sprint 32 — 当前 sprint）

| 任务 | 里程碑 | 验收标准 |
|------|--------|---------|
| 方向三：checkpoint 增加 `TraceSeq` + `MemorySeq` | Day 3 | 旧 checkpoint 可读（`omitempty`），新 checkpoint 含 seq |
| 方向三：`trace.Count()` + `memory.Count()` | Day 4 | 两存储暴露行数计数，不改变写入路径 |
| 方向三：`forge doctor --cross-check` | Day 5 | 启动时可检测三存储的分歧并报告 |

**交付物**：
- `internal/persist` 的 checkpoint 结构体变更
- `internal/trace` / `internal/memory` 的 `Count() int` 方法
- `cmd/forge` 的 `doctor` 子命令扩展

#### Phase 2：「成本治理独立化」（Sprint 32-33）

| 任务 | 里程碑 | 验收标准 |
|------|--------|---------|
| 方向一：`AgentExecutor` 接口新增 `CostCap()` / `SetCostCap()` | Day 3 | 接口变更，默认 nop 实现 |
| 方向一：`runPhase` 中增加成本预检（Build → pre-exec cost check → spawn） | Day 5 | 非 claude CLI 也受 `--agent-max-budget-usd` 约束 |
| 方向一：`PhaseBudget` 结构体提升到接口实现中 | Day 7 | `CommandExecutor` 的预算字段从 CLI 参数变为 `CostCapper` 调用 |
| 方向一：集成测试覆盖 echo CLI + 预算上限 | Day 8 | 测试验证非 claude CLI 的预算拦截 |

**交付物**：
- `AgentExecutor` 接口变更
- `orchestrator` 或 `executor` 的 cost pre-check 函数
- 集成测试（`engine_build_test.go`/`executor_test.go`）

#### Phase 3：「输出契约治理」（Sprint 33 或后续）

| 任务 | 里程碑 | 验收标准 |
|------|--------|---------|
| 方向二：`postPhase` 可配置钩子注册 | Day 3 | lifecycle manager 支持 pre/post hooks（或 loop.go 直接调用） |
| 方向二：`EmitsVerifier` 实现（warn-only 模式） | Day 5 | phase 完成后检查 emits 路径存在性，写 trace |
| 方向二：emits 缺失的收敛信号注入 | Day 6 | `converge` 信号可以感知 emits 验证结果 |
| 方向二：历史 workflow 的回溯兼容 | Day 7 | 所有现有 workflow 的 emits 验证在 warn 模式通过 |

**交付物**：
- Phase Lifecycle Hook 系统（或 loop.go 的 post-exec 校验路径）
- `EmitsVerifier` 模块
- converge 信号扩展（可选字段）

#### Phase 4：「预算感知 UX 增强」（漂移至并行启用后）

| 任务 | 优先级 |
|------|--------|
| 方向四 Phase 1：`BudgetWatermark` trace event at 20%/10%/5% | P2 — 可独立交付 |
| 方向四 Phase 2：梯度降档级联 | P3 — 需更多 UX 验证 |
| 方向五：`WaveResourceConstraint` 模型 + adaptive scheduling | P3 — 待 `depends_on` 启用 |

### 5.3 风险点和缓解策略

| 风险 | 涉及方向 | 概率 | 影响 | 缓解 |
|------|---------|------|------|------|
| **R1**: `AgentExecutor` 接口扩展破坏自定义 executor | 方向一 | 低 | 高 | 使用 default implementation pattern，接口扩展全部可选方法 |
| **R2**: 一致性标记的 seq 在 crash 后漂移 | 方向三 | 中 | 中 | `forge doctor` 检测到差异时只 warn 不 block，留给 operator 判断 |
| **R3**: emits 验证的 false positive（文件写入延迟） | 方向二 | 中 | 低 | post-phase hook 引入可配置 delay（如 500ms）再检查；或者将验证设为异步 |
| **R4**: 预算水位告警触发过于频繁（震荡） | 方向四 | 低 | 中 | 引入冷静期（cooldown）：水位触发后在 X 秒不再重复告警 |
| **R5**: 并行资源约束导致 wave 退化为串行 | 方向五 | 中 | 低 | 这是正确行为——约束应当有上限，退化到串行比过载更安全。记录到 trace 供优化参考 |
| **R6**: 多个方向同时修改 loop.go 导致冲突 | 方向一/二/四 | 中 | 低 | Phase 1/2/3 的顺序已按变更隔离规划——方向三只改 checkpoint，方向一只改 executor，方向二改 loop 但排在之后 |

### 5.4 推荐的实施顺序（最终建议）

```
Sprint 32 ─┬─ 方向三（一致性标记）── 3-5 天
           │
           └─ 方向一 Phase 1（CostCap 接口 + pre-exec 拦截）── 5-8 天
                              ↓
Sprint 33 ─┬─ 方向一 Phase 2（集成测试 + 文档）── 2 天
           │
           ├─ 方向二（emits 验证，warn-only 模式）── 5-7 天
           │
           └─ 方向四 Phase 1（水位告警 trace）── 3-4 天
                              ↓
     backlog ── 方向四 Phase 2（梯度降档） + 方向五（资源调度）
```

**收敛后的执行策略**：

1. **本周（Sprint 32）启动方向三和方向一**——两者不冲突、改动正交、都只需要新加代码不重构旧逻辑
2. **Sprint 33 承接方向二和方向四 Phase 1**——方向二的 post-phase hook 可以作为方向四预算水位 trace 事件的携带者，一次 hook 系统建设滋养两个方向
3. **方向四 Phase 2 和方向五漂移**——不是因为不重要，而是因为它们依赖前置条件（方向四依赖方向一的成本数据积累，方向五依赖并行启用），强行提前做会导致设计在被真实需求验证之前「猜测」需求

---

## 六、补充架构观察

### 6.1 关于「架构完整性」的元思考

五个盲区揭示了一个有趣的现象：**ForgeOS 的架构完整性不是「缺少什么层」，而是「层之间的边界未完全界定」**。

这不是负面发现——恰恰相反。在一个有 18+ Go 包、~35k 生产代码的系统中，五个边界盲区在跨层扫描后才暴露，说明核心层的隔离是成功的。盲区都在**层间接口的「传递」部分**（成本参数的透传、emits 的声明但不验证、checkpoint 写但不关联 trace），而不是在各层内部。

这意味着架构策略应该专注于**接口契约的形式化**（formalization of interface contracts）而不是新增架构层。

### 6.2 关于「杠杆率最高的一步」

如果只做一步就走：

**不是方向三（审阅建议的「改动最小」），而是方向一（厂商无关成本护栏）**。

理由：方向三虽改动小（checkpoint 加两个字段），但一致性标记只有在该 checkpoint 用于恢复时才产生价值——它的杠杆在故障时。方向一的杠杆在**每一次执行**——无论使用什么 agent CLI，成本上限都在 forge-core 层强制。而且方向一的修复为方向四（预算梯度）提供了前置依赖（成本模型接口），有「开启更多可能性」的增值效果。

**实际建议：Sprint 32 同时推进方向三和方向一**——两者改动正交，3-5 天（方向三）+ 5-8 天（方向一）可以在两周内完成两个 P1/P0 方向，覆盖数据信任 + 成本治理。

### 6.3 需要关注的二阶风险

1. **方向一的审计模式可能带来性能开销**：如果每调用都做成本估算，可能增加 phase 进入的延迟。建议成本估算走「快速通道」——只做简单的 `if estimatedCost > costCap` 算术，不做重 API 调用。

2. **方向三的一致性标记在 evolve 场景的语义**：跨迭代 evolve 时，memory 的 Compact 操作会改变 memory 行数，使 seq 不再唯一。设计方案时需要明确 seq 是「原始条目计数」还是「当前条目计数」，建议是后者并在 Compact 时更新 seq。

3. **方向二的 emits 验证与 converge 耦合**：如果 emits 验证结果被注入 converge 信号，可能导致「因 emits 验证失败而持续 evolve」——这个循环可能在 emits 文件永远无法被 agent 生成时陷入死循环。需要在 converge 信号中区分「硬性」和「软性」验证失败，只有硬性（如 core artifacts missing）才触发 evolve 循环。
