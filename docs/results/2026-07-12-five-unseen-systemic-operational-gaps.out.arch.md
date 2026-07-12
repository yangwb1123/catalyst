现在我已经完整阅读了报告中引用的所有关键源文件，并理解了系统的架构。以下是分析。

---

# 架构分析与战略设计建议

## 1. 架构评估

### 优势

ForgeOS 的代码库在几个架构方面表现优异：

1. **严格的依赖倒置**。`orchestrator.Engine` 通过注入的回调（`RunGate`、`AgentVerdict`、`BudgetExhausted`、`OnPhase`、`Sleep`、`Observe`、`ClassifyOverload`）与所有外部系统解耦。没有硬编码的导入指向 I/O 层、供应商 SDK 或持久化——这使得该引擎可以在纯单元测试中进行测试，无需文件系统或网络。

2. **诚实优先的设计模式**。整个代码库都有明确标记的"字节精确向后兼容性"契约（"byte-for-byte the prior behavior"）。新的字段/行为默认是无效的（`MaxRetries=0`、`MaxLoopBack=0`、`MaxAgentCalls=0`）。这不是偶然的，而是一种系统的架构原则，防止回归。

3. **纯函数与 I/O 分离**。`routing.go` 中的 `DowngradeOne`、`higher`、`Score` 和 `BudgetAdjustTier` 是纯的（无 I/O、无状态）。`scorecard.go` 中的 `HistoryTiebreak` 和 `Lookup` 是纯的。`memory.go` 中的 `Query`、`encode`、`decode` 和 `filterSuperseded` 是纯的。I/O 总是在薄薄的一层封装中（`Append`、`Load`、`Save`）。这是正确的架构。

4. **原子持久化原语**。`checkpoint.go` 的 temp-+fsync-+rename 原子写入，加上历史轮转，是正确的。`memory.go` 的 O_APPEND（每条追加一个原子 `write(2)` 系统调用），加上用于重新读取性能的 mtime 键控缓存，是正确的。

### 架构性债务

**1. 五个已验证的差距可以追溯到同一个根本原因：缺乏"横切关注点"抽象。** 引擎是超内聚的——`Engine` 结构体拥有超过 15 个注入字段，全部通过 `RunFrom` 中的单个 for 循环编排。没有：
- **预算管理器抽象**（双重预算检查 `checkAgentBudget` + `checkRunBudget` 被内联在 `runAgentPhaseBudgeted` 中；`BudgetExhausted` 是一个注入的闭包，而不是一个有名称的接口）
- **重试/退避抽象**（`runAgentPhase` 中的退避逻辑与重复执行循环耦合）
- **Env 安全层**（`childEnv` 只过滤一个变量——安全范围从未被建模为可配置的策略）
- **跨存储协调器**（`checkpoint`、`trace`、`memory`、`scorecard` 各自独立存储，没有协调层）

这不是糟糕的设计——这是演化驱动的增量主义。但随着系统发展到五个持久化存储（checkpoint / trace / memory / scorecard / converge 信号）和三个成本控制维度（计数 / 计时 / 美元），缺少这些横切关注点现在是一个实际的故障模式来源。

**2. 并行执行层与序列引擎共用同一个抽象的 Engine。** `RunParallel`（`parallel.go`）绕过了 `RunFrom` 的 loop-back、每个阶段的检查点和线性重试序列——但它仍然接受一个 `Engine` 值，而 `Engine` 有 `MaxLoopBack`、`OnPhase` 和 `Sleep` 字段，这些字段在并行模式下是无效的。这是类型系统本应捕获但未捕获的配置漂移：一个设置 `MaxLoopBack=3` 的引擎在并行模式下是无声忽略的。该数据类型的忠实性变差了。

**3. 成本控制维度（`runBudgetUSD`、`maxAgentCalls`、`timeout`）通过三个独立的、非正交的机制实施**：
- `checkAgentBudget`（一个简单的计数器）
- `checkRunBudget`（一个不透明的 `BudgetExhausted() bool` 闭包）
- 每个阶段的 `Timeout`（由 `command_executor.go` 中的 `context.WithTimeout` 实施）
没有统一的数据成本模型——没有 `CostBudget` 类型来封装"我们还能花费多少"的问题。`runBudgetUSD` 由 `cmd/forge` 在外部跟踪（通过 `cost.go`，我没有完整阅读），而引擎只看到一个不透明的布尔值。这是预算降级-质量螺旋（方向二）的根本原因：预算数据无法被路由层用于前瞻性地安排降级。

### 关键的架构性权衡

| 决策 | 优势 | 成本 |
|-----------|--------|--------|
| `Engine` 作为带注入的巨型结构体 | 零依赖，完全可测试 | 没有接口契约；配置漂移（并行模式下无效的字段） |
| 纯函数 + 薄 I/O 层 | 单元可测试，确定性 | 跨存储一致性不在系统内建模 |
| O_APPEND 用于 memory | 追加 O(1)，崩溃安全 | 没有事务性跨存储写入 |
| 无外部依赖（Go stdlib 只有） | 零供应链风险，构建简单 | 每个横切关注点必须内部构建 |
| `ModePolicy` 零值 == 无过滤 | 字节精确向后兼容 | 模式必须显式设置才能生效 |

---

## 2. 扩展方向

### 方向 A：预算管理系统（P0）

**为什么需要。** 方向二（budget→degrade→low quality→rework→more spend→more degrade）是一个在代码中完全可见的破坏性螺旋。`BudgetAdjustTier` 可以在阶段层面降级 tier，但 `checkRunBudget` 的硬停止是二元的——它在耗尽时突然终止运行。两者之间没有任何东西：没有前瞻性预算规划，没有"剩余 $X → 降低 implementer 到 Haiku 以延长 5 次迭代"，没有"此阶段的预期成本"的预测。

**核心挑战。**
- 成本只有在阶段完成后（`Observe` 回调携带原始输出和延迟）才可知，而不是之前。预算管理系统需要前瞻性模型成本估算——但准确的成本取决于输出 token 计数，这在执行前是不可知的。
- 预算状态当前是 `cmd/forge` 中的一个局部变量（`cost.go` 中的 `runBudget`），对 `routing` 包不可见。需要构造一个共享的 `BudgetState` 抽象。

**预期的架构变更。**
```
// 新的横切关注点
type BudgetState interface {
    Remaining() float64          // 剩余美元
    SpendRatio() float64         // 已花费/上限
    ProjectCost(phase asset.Phase, tier string) float64  // 前瞻性估算
    Charge(costUsdMicros int64)  // 记录实际花费
    Exhausted() bool             // 硬停止
}

// Engine 获得一个 Option[BudgetState]。
// BudgetAdjustTier(routing.go) 获得 BudgetState 代替原始的 spendRatio float64。
// checkRunBudget 委托给 BudgetState.Exhausted()。
```

**对现有系统的影响。** 零——`BudgetState` 是一个可选的注入。`nil` 预算 == 今天的行为（`spendRatio=0` → 无限额）。

### 方向 B：Env 安全边界（P0）

**为什么需要。** 方向四（env 泄漏）是最紧急的攻击面。当前的 `childEnv` 过滤恰好一个变量（`FORGE_AGENT_DEPTH`）。一个 LLM agent 拥有 `子进程` 工具的完全访问权限可以读取 `GITHUB_TOKEN`、`AWS_CREDENTIALS`、`DATABASE_URL`——并且由于默认权限是 `acceptEdits`，它可以写入仓库。修复是架构性的，不仅仅是一个独立的过滤器。

**核心挑战。**
- 哪些 env 变量是"安全的"取决于部署上下文。CI runner 有更多的秘密，而不是 `forge run --executor command` 的本地开发环境。我们不能硬编码一个全局安全的 env 列表。
- POSIX 关于重复键的语义在 libc 之间未指定（`childEnv` 注释已经注意到了这一点——这为什么它使用 `strings.HasPrefix` 替换）。
- 正确的修复是建立一个具有默认允许列表和选择性拒绝的可配置的 env 策略。

**预期的架构变更。**
```
type EnvPolicy struct {
    AllowList []string  // glob 模式，例如 "PATH", "HOME", "FORGE_*"
    DenyList  []string  // 高优先级否认，例如 "GITHUB_TOKEN", "AWS_*"
}

func NewEnvPolicy(defaults ...EnvPolicy) EnvPolicy  // 合理的默认值
func (p EnvPolicy) Filter(base []string) []string   // 应用于 os.Environ()

// CommandExecutor 获得一个 Option[EnvPolicy]。
// childEnv(depth) 获得一个 envPolicy 参数。
```

**对现有系统的影响。** `nil EnvPolicy` == 今天的行为（传递所有 env）。非 nil 策略启用安全过滤。默认策略可能包括 `FORGE_*`、`PATH`、`HOME`、`LANG`、`TMPDIR`——以及 `GITHUB_TOKEN` 和 `AWS_*` 的明确拒绝。

### 方向 C：退避管理器 + 抖动（P1）

**为什么需要。** 方向三（没有抖动 + 并行执行 = 自我 DoS）在 `parallel.go` 中是立即活跃的。`backoff.go` 的注释已经承认了这一点："jitter only matters once many agents retry in parallel"——但 `runParallel` 创建了这个确切的场景。修复需要两个更改：（a）在并行退避序列中引入抖动，和（b）在并行模式下协调跨阶段退避，使所有阶段不会完美同步地重试。

**核心挑战。**
- 抖动必须是确定性的（用于可测试性）但在实践中去相关。使用 `FullJitter`（`[0, base*2^attempt)` 上的均匀随机）是标准的，但 `engine.Sleep func(time.Duration)` 没有随机源。
- 并行模式在所有阶段之间共享一个 `waveCtx`——协调退避意味着共享状态，目前通过 `sync.Mutex`（在 `parallel.go` 中）受保护，但退避发生在锁外。

**预期的架构变更。**
```
type BackoffPolicy interface {
    Duration(attempt int, jitterSeed int64) time.Duration
}

// 两种实现：
// - ExponentialBackoff { Base, Cap, Jitter bool }
// - ConstantBackoff { Interval }

// Engine.Sleep 成为 BackoffPolicy。
// runAgentPhase 将阶段索引作为种子传递给 Duration()。
```

**对现有系统的影响。** 默认的退避策略（无抖动，指数型）今天产生字节精确的相同序列。并行模式可以注入一个抖动的退避策略。

### 方向 D：跨存储一致性层（P1）

**为什么需要。** 方向五（checkpoint 说 "iteration 5, 55% done" → trace 是否记录了 iteration 5 的结束事件？→ scorecard 是否反映了在 iteration 5 中使用的模型 tier？→ memory 是否包含了该迭代的结果？）在 `doctor.go`、`persist/checkpoint.go`、`trace/trace.go` 和 `memory/memory.go` 中都没有跨文件一致性检查。在从崩溃中恢复时，我们无法知道哪些存储是同步的。

**核心挑战。**
- 四个存储（checkpoint、trace、memory、scorecard）由完全不同的机制写入：`persist.Save`（原子重命名）、`trace.Tracer.Emit`（带互斥锁的 O_APPEND）、`memory.Append`（O_APPEND）、`scorecard`（由外部的 Eval 引擎写入）。使它们保持同步需要一个两阶段提交或一个序列号类型的协调层。
- 因为持久化是每个迭代、每个阶段和每个事件的，没有单个提交点将它们全部原子化地链接起来。

**预期的架构变更（轻量级）。**
```
// 在 checkpoint 中，添加跨存储引用：
type Checkpoint struct {
    // ... 现有字段 ...
    TraceLastSeq     int   `json:"trace_last_seq"`      // 在该迭代中发出的最高 trace seq
    MemoryEntryCount int   `json:"memory_entry_count"`  // 在该迭代写入时的 memory 条目数
    ScorecardVersion  string `json:"scorecard_version"` // 上次看到的 scorecard 的 content hash
}

// doctor.go 然后可以验证：
//   checkpoint.TraceLastSeq <= trace.jsonl 中的最大 seq
//   checkpoint.MemoryEntryCount <= memory.jsonl 中的行数
//   checkpoint.ScorecardVersion == hash(scorecards.json)
```

**对现有系统的影响。** 零。写入这些字段是可选的（`omitempty`），所以旧的 checkpoint 可以读取。`doctor` 中的检查将是额外的，非失败性的警告。

### 方向 E：Tier 感知的断路器（P1→P2）

**为什么需要。** 方向二的螺旋（implementer downgrade to haiku → haiku 产生低质量代码 → reviewer REQUEST_CHANGES → implementer 以 haiku 重做 → 继续消耗预算 → 更频繁的降级）可以通过一个断路器来缓解：如果 implementer 连续 `N` 次因为 reviewer 要求变更而被循环回来，提升 tier 而不是降低它。熔断器在 `docs/requirements/expansion-production-blindspots-v36.md` 中被作为建议方向提到，但 Go 代码中没有任何实现。

**核心挑战。**
- 循环回溯可以由两种东西触发：红色 gate（门）或 reviewer REQUEST_CHANGES（建议）。断路器应该只对后者触发——gate 循环回溯可能由合法的 gate 失败引起，而 gate 失败应保持失败关闭。
- 状态必须跨阶段、跨迭代持久化。目前，循环回溯计数器（`loopBacks`）是 `RunFrom` 中的一个局部变量，在迭代之间不会持久化。

**预期的架构变更。**
```
// routing 包中的新状态
type CircuitBreaker struct {
    ConsecutiveLoopBacks map[string]int  // 按 agent 名称
    Threshold            int             // 触发阈值（例如 3）
}

func (cb *CircuitBreaker) RecordLoopBack(agent string) {
    cb.ConsecutiveLoopBacks[agent]++
}

func (cb *CircuitBreaker) ShouldUpgrade(agent string) bool {
    return cb.ConsecutiveLoopBacks[agent] >= cb.Threshold
}

// BudgetAdjustTier 在降级前咨询断路器：
func BudgetAdjustTier(base, agent string, spendRatio float64, cb *CircuitBreaker) string {
    if cb != nil && cb.ShouldUpgrade(agent) {
        return UpgradeOne(base)  // 新函数：haiku → sonnet → opus
    }
    // ... 现有的降级逻辑 ...
}
```

**对现有系统的影响。** 零。`nil` 断路器 == 今天的行为。

---

## 3. 接口设计建议

### 原则

1. **零值必须意味着"无操作"或"字节精确的旧行为"。** 这是代码库已经遵循的原则。坚持下去。
2. **接口应该是可选的注入，而不是结构占用。** 每个横切关注点（预算、env 安全、断路器、退避策略）都应该是 `Engine`（或 `CommandExecutor`）上的一个可选字段，默认是 `nil`。
3. **持久化包不应该相互引用。** `persist`、`trace`、`memory`——这些是叶包。一个横切关注点的 `ConsistencyLayer` 应该位于它们之上，可能作为 `doctor` 的一部分。

### 建议的新抽象层

```
forge-core/internal/
  budget/           ← 新的：BudgetState 接口，前瞻性成本模型
    budget.go
    cost_model.go

  security/         ← 新的：EnvPolicy，以后还有 SandboxPolicy
    envpolicy.go
    envpolicy_test.go

  backoff/          ← 新的：BackoffPolicy 接口 + 策略
    backoff.go
    jitter.go
    strategy_exponential.go

  consistency/      ← 新的：跨存储验证
    verify.go         (checkpoint ↔ trace ↔ memory ↔ scorecard)

  persist/          ← 现有的
    checkpoint.go
  trace/            ← 现有的（保持不变）
  memory/           ← 现有的（保持不变）

  routing/          ← 现有的 + 断路器
    routing.go
    circuitbreaker.go   ← 新的
```

### 向后兼容性契约

每个新的抽象都应该遵循已建立的模式：

```go
// 在 Engine 中：
BudgetState budget.BudgetState  // nil 表示"无预算限制"

// 在 CommandExecutor 中：
EnvPolicy *security.EnvPolicy  // nil 表示"传递所有 env"

// 在 runAgentPhase 中（backoff.go）：
Backoff backoff.Policy  // nil 表示"无抖动的指数退避"
```

这保留了"字节精确"的保证——一个没有设置这些字段的现有调用者会得到完全相同的旧行为。

---

## 4. 技术选型

### 新依赖评审

**结论：零新外部依赖。** `forge-core` 的 Go 部分（"零外部依赖"的宪法红线）和 `harness` 的 Node/Python 部分（也是零外部依赖）应该保持不变。所有五个方向都可以用标准库完全解决：

| 方向 | 所需内容 | 标准库支持 |
|-----------|-----------|-------------------|
| 预算管理 | 浮点运算，数学 | `math`、`sync` |
| Env 安全 | 字符串匹配，os.Environ | `strings`、`os`、`path/filepath`（用于 glob） |
| 退避 + 抖动 | 随机数 | `math/rand`（确定性种子） |
| 跨存储一致性 | JSON 解析，校验和 | `crypto/sha256`、`encoding/json` |
| 断路器 | 映射，计数器 | 内置类型 |

### 自建 vs 采购的决策

**方向 A（预算管理）：自建。** 预算边界在概念上很简单（花费与上限，带累计）。没有现成的 Go 库提供"LLM 代理运行预算管理"——任何第三方解决方案都会带来妥协，同时增加依赖关系。

**方向 B（Env 安全）：自建。** `EnvPolicy` 是一个约 50 行的过滤器，带有一个默认的允许/拒绝列表。添加一个库会为完成链增加比现有代码更多的行数。

**方向 C（退避管理器）：自建。** 指数退避 + 抖动是大约 30 行标准代码。在测试中实现确定性种子需要可注入的随机源。

**方向 D（一致性）：自建。** 跨存储验证是特定于 forge-core 的数据模型的——没有通用的"检查点一致性"库。

**方向 E（断路器）：自建。** 续循环回溯计数器 + 阈值 = 约 40 行。

**一般规则：** 当（a）领域特定于 forge-core 的数据模型，或（b）可以在 <100 行标准库中实现时，自建。对于（c）高度可配置的通用原语（HTTP 服务器、CLI 解析、速率限制器），考虑采购。这里没有什么符合条件（c）。

---

## 5. 实施路线图

### 优先级排序

| 优先级 | 方向 | 理由 |
|----------|-----------|---------|
| **P0** | B：Env 安全边界 | 最高安全影响；CI 中活跃的攻击面；组合：secret 泄漏 + `acceptEdits` = 仓库级妥协 |
| **P0** | A：预算管理系统 | 预算-降级-质量螺旋是活跃的，且跨代码库文档化；`BudgetAdjustTier` 已经存在但没有足够的上下文来正确降级 |
| **P1** | C：退避 + 抖动 | 在并行模式下活跃；自我 DoS 窗口是真实的，随着更多工作流声明 `depends_on` 而增长 |
| **P1** | D：跨存储一致性 | 防止从无声不一致的存储中恢复；通过 `doctor` 添加轻量级的、非失败的检查 |
| **P2** | E：Tier 感知的断路器 | 需要的，但预算管理系统必须首先到位以使 tier 调整不降级时无信息；取决于方向 A |

### 阶段划分

**阶段 1（P0，第 1-2 周）：Env 安全 + 预算基础。**

- 实现 `security.EnvPolicy` 及其默认允许/拒绝列表
- 注入到 `CommandExecutor` 中（可选的，向后兼容）
- 实现 `budget.BudgetState` 接口
- 迁移 `cmd/forge` 的 `runBudget` 局部变量到 `BudgetState`
- 从 `checkRunBudget`、`checkAgentBudget` 和 `BudgetAdjustTier` 中提取前瞻性成本模型

**阶段 2（P1，第 3-4 周）：退避 + 一致性。**

- 实现 `backoff.Policy` 接口，加上 Exponential + Jitter 策略
- 将 `Engine.Sleep` 迁移到 `backoff.Policy`（向后兼容的默认值）
- 在 `checkpoint.go` 中添加 `TraceLastSeq` / `MemoryEntryCount` / `ScorecardVersion` 字段，并摄取
- 在 `doctor.go` 中添加跨存储验证检查

**阶段 3（P2，第 5 周）：断路器。**

- 实现 `routing.CircuitBreaker`（连续的循环回溯计数器 + 阈值）
- 在 `agentOutcome` 路径中连接它
- 修改 `BudgetAdjustTier` 在循环回溯超过阈值时咨询断路器

### 风险与缓解措施

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|----------|--------|-------------|
| `EnvPolicy` 破坏了 CI 管道（阻塞了必要的 env） | 中等 | 高 | `nil` 默认值（今天的行为）；opt-in 策略启用过滤；在 CI 中可配置的允许列表 |
| `BudgetState` 的前瞻性成本模型不准确（输出 token 不可预测） | 高 | 低 | 使用简单的每个模型的每输入 token 常量的粗略估算。预算估计是提示性的，不是精确的；硬停止（`Exhausted()`）仍然基于实际成本 |
| 跨存储检查在现有健康部署中产生误报 | 低 | 中等 | 验证是 `doctor` 中的警告，不是失败。在阶段 2 期间观察真实世界的信号 |
| 抖动退避在重试之间引入了不可接受的延迟 | 低 | 低 | 抖动将延迟随机分布在 `[0, cap]` 范围内，而不是增加最大值。最大值由 `MaxRetries * cap` 限定 |
| 断路器在合法的困难任务上过早触发（"这个任务很难，需要多次尝试"） | 中等 | 低 | 阈值从 N=5 开始（高门槛）。断路器提升 tier 而不是阻塞（"降级经济"中的安全机制） |

### 里程碑

- **M1（第 2 周结束时）：** P0 方向已合并并启用。`forge run --executor=command` 在 CI 中默认过滤 env 秘密。预算管理在 >80% 时前瞻性降级，而不是在 100% 时突然停止。
- **M2（第 4 周结束时）：** P1 方向已合并。并行模式在重试时使用抖动的退避。`forge doctor` 报告跨存储一致性。
- **M3（第 5 周结束时）：** P2 方向已合并。断路器在连续的 REQUEST_CHANGES 循环后提升 tier。

### 治理审计

在每次添加后，`gate.mjs`（文件大小）和 `arch/arch-check.mjs`（分层）必须保持绿色。特别是：

1. **没有新的包循环：** `budget`、`security`、`backoff` 和 `consistency` 必须是叶包，不导入 `orchestrator`、`asset` 或 `cmd/forge`。
2. **`EnvPolicy` 不得导入 `orchestrator`：** 它是一个独立于编排上下文的纯数据结构。
3. **`BudgetState` 不得导入 `cmd/forge`：** 它必须位于 `internal/budget` 中，并由 `cmd/forge` 实现。
4. **功能标志：** 每个新抽象都接受 `nil` 作为"未配置"——任何非 nil 配置都会改变行为。没有全局变量，没有 init() 函数，没有隐式注册。
