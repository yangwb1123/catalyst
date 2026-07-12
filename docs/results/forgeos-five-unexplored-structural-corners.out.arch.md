现在我已掌握项目的完整上下文。让我基于验证文档 + 项目自身架构文档，撰写完整的架构分析报告。

---

# 架构深度分析报告：ForgeOS

## 1. 架构评估

### 1.1 当前架构的核心优势

ForgeOS 的架构是我见过的**少数在 v0-v2 阶段就具备清晰演化纪律**的项目。以下几点值得单独强调：

**① 控制面/数据面分离的萌芽已到位。** 虽然当前是单体二进制 (`forge-core`)，但 `harness/` 作为带外执法层已经独立于主逻辑运行——这恰好是 north-star 架构中「PDP/PEP 分离」原则的最小可行实现。`gate.mjs` `arch-check.mjs` `secret-scan.mjs` 都是通过 subprocess 独立执行的，不嵌入 Go 进程内部。

**② 中枢旋钮 (mode×lifecycle) 的设计杠杆率极高。** 一个设置同时驱动 Router 档位、Harness 严格度、Workflow 深度——这是在 monolith 阶段为未来分布式架构做的关键「抽象预留」。当未来拆出独立 `Policy/Gov PDP` 服务时，这一旋钮的语义可以原样迁移。

**③ 「零外部依赖」不是教条，是有意识的架构约束。** `forge-core` 的 `go.mod` 无 `require`——这意味着所有跨包通信都必须通过 Go 的标准库接口完成。这强制团队避免「加个库来绕过设计问题」的坏习惯，也使得未来编译为静态二进制嵌入任何宿主都极简单。

**④ 文件/函数/扇入的「代际护栏」已经制度化。** 这不是传统的 lint 规则——它是通过 `harness/adapters/go.yml` + `arch-check.mjs` 组成的一条**不可绕过的 CI 闸门**。任何 PR 违反 500 行 / 50 行 / 循环依赖 / 包扇入上限都会被 `forge accept` 拒绝。这是在 monolith 阶段对抗熵增的最有效机制。

### 1.2 当前架构的关键局限性

验证文档的 5 个方向可以归纳为 **3 个结构性问题**：

**局限性 A：包间契约隐性化（方向 4 的深层含义）。** 整个 `internal/` 下只有 1 个接口定义 (`AgentExecutor`)。`converge` `gate` `memory` `persist` `trace` 之间的跨包调用全部通过 concrete types 完成。这在 13 个包、每个包 ≤500 行的规模下尚可管理，但当包数增长到 20-30 时，concrete type 的直接引用将导致：
- 无法独立测试（需要 import 完整的包依赖链）
- 无法并行开发（接口变更影响所有调用方）
- 无法替换实现（测试替身、mock 都需要接口）

**局限性 B：职责判定机制缺乏连续轴（方向 3 + 方向 5 的深层含义）。** 当前 `converge.Converge` 只返回 `(met bool)`——二值。但真实的软件演化是渐进的：40% ROADMAP 完成 ≠ 0%，只是不到 100%。缺乏连续轴导致：

- LoopEngine 无法做「partial credit」决策（如：虽然未收敛但已从 20% 跑到 80%，趋势健康，继续；从 80% 降到 30%，应报警）
- `QuickDoctorCheck` 只跑一次、不在循环内运行——无法做趋势检测
- 健康诊断只有 2 个维度（roadmap completion + gates green），缺少**存储一致性 / 收敛趋势 / 成本效率**等多维信号

**局限性 C：单二进制 CLI 的共享状态耦合（方向 1）。** `cmdRun` 和 `cmdEvolve` 共享 16 个 flag bindings + `subcommands` map + `loadWorkflow` + `maxLoopBack`。这不是并发安全问题（Go CLI 天然串行），但这是**概念耦合**——`cmdEvolve` 继承了大量只在 `cmdRun` 中有意义的 flags（如 `--parallel`），新开发者很难理解「为什么 evolve 需要 artifacts 路径」。

### 1.3 架构债务 vs 技术债

| 类型 | 具体表现 | 严重度 | 修复成本 |
|---|---|---|---|
| **架构债务** | 包间无接口契约（concrete type 跨包引用） | 中 | 中（定义接口 + 适配器，不涉及逻辑重写） |
| **架构债务** | `cmdRun`/`cmdEvolve` 共享 CLI flags 导致概念泄漏 | 低 | 低（拆分 `runOptsShared` + `runOptsExtras`） |
| **架构债务** | 收敛判定二值化，无法感知连续进度 | 高 | 高（涉及 `converge` 返回类型变更 + LoopEngine 逻辑重写） |
| **设计债务** | `Phase` 有 `Emits` 但无 `Expects`，依赖语义不对称 | 低 | 低（纯 additive 字段 + validate 检查） |
| **测试债务** | 包间无 contract test（只有集成测试覆盖跨包路径） | 中 | 中（每个新接口加 `*_contract_test.go`） |
| **文档债务** | `internal/routing.TierFor` 的「非完整多维评分器」自我标注需对齐实际行为 | 低 | 低（只需文档修正） |

**判断**：当前代码库不存在「需要重写才能修」的架构债。所有问题都是增量可修复的。这与 ROADMAP.md 的 31 个 sprint 的输出质量一致——纪律严格。

---

## 2. 扩展方向

### 方向 A：引入包间契约层（P0 — 架构安全基线）

**为什么需要：** 13 个包的时代，concrete type 引用尚可。但这个项目正处于从 monolith 向分布式演化的关键转折点（ROADMAP 显示 v3 会拆出独立服务）。如果在 monolith 阶段不建立接口契约，拆服务时会面临「被引用的 concrete type 遍布 13 个包，拆不动」的困境。此外，`converge.Signals` `gate.Result` `persist.Store` 这些核心类型需要 contract test 来保证实现者不偏离契约。

**核心挑战：**
1. 不能过度设计——接口应该从真实使用点提取（consumer-driven contract），而非预先发明抽象
2. 契约测试需要独立于实现测试运行——意味着需要额外的 CI 步骤
3. 部分类型（如 `trace.Event`）是纯数据结构，接口化的收益有限

**预期架构变更：**
- `internal/converge`：定义 `Probe` 接口（`Evaluate(ctx, Signals) (Results, bool)`），当前 `Converge` 作为默认实现
- `internal/gate`：定义 `Consumer` 接口（`Consume(ctx, Result) error`），当前直接调用作为默认实现
- `internal/persist`：定义 `Store` 和 `Loader` 接口
- 新增 `internal/contracts/` 包（或散列到各包下的 `*_contract_test.go`）

**对现有系统的影响：**
- 零行为变更——只需要加接口定义 + 让现有 concrete type 实现它们
- 不会触发任何重构（只是加 abstraction）

**两个选项：**

| 选项 | 做法 | 收益 | 成本 |
|---|---|---|---|
| **保守** | 只对 `converge.Signals` 和 `gate.Result` 加接口（它们被跨包引用最多） | 覆盖 80% 耦合面 | 低，2-3 接口 |
| **激进** | 对所有跨包 public type 加接口 | 架构弹性最大 | 中，5-7 接口，需要 contract test 基础设施 |

**建议**：选保守路线起步，优先做 `converge.Probe`+`persist.Store` 两个接口，它们在 north-star 架构中会演化为独立服务。

---

### 方向 B：连续收敛判定 + 内循环健康自检（P0 — 循环智能的基础设施）

**为什么需要：** 当前 `Converge` 返回 `(met bool)` 二值，LoopEngine 对状态的理解只有「到了/没到」。这导致：
- 无法区分「刚起步（0%）」vs 「即将完成（90%）」——两者的 Loop 行为应该不同
- `QuickDoctorCheck` 只跑一次——无法做趋势检测
- 无法检测「收敛停滞」（多次迭代分数不变）或「收敛倒退」（分数下降）
- 这是验证文档确认的 P0 问题

**核心挑战：**
1. 连续收敛分数需要定义清晰的计算逻辑——不能是黑盒 ML 分数
2. 健康检查的多维信号需要权衡——哪些维度报告就应终止，哪些只是告警
3. 趋势检测需要历史数据——当前 `trace` 已有 iteration-level 数据，但 LoopEngine 不消费

**预期架构变更：**
- `Converge` 返回类型从 `(Results, bool)` 改为 `(Results, ConvergenceScore)`——`ConvergenceScore` 包含百分比 + 子维度得分
- LoopEngine 新增 `[]HealthCheck` 注入点——每次迭代后运行，报告健康状态
- `HealthCheck` 接口：`Check(ctx, LoopState) (HealthStatus, error)`
- 内置 health checks：收敛趋势（线性回归斜率）、gate pass rate 变化、成本效率比、存储一致性
- `staleCount` 从 2 维（roadmap + gates）扩展为多维指标加权

**对现有系统的影响：**
- **破坏性变更**：`Converge` 返回类型改变——所有调用方需更新
- `checkStop` 的逻辑需要从 `met == true` 改为 `score >= threshold`（threshold 可配）
- 向下兼容路径：提供 `Met() bool` 辅助方法，阈值默认 100%

**风险点：**
- 分数计算可能引入主观性——需要通过 `docs/adr/` 明确定义分数算法
- 多维度分数加权可能导致调试困难——需要提供 `--explain-scores` flag

---

### 方向 C：Phase 工件依赖图显式化（P1 — 全流程数据流的完整性）

**为什么需要：** 验证文档确认 `Phase` 有 `Emits []string` 但无 `Expects`。这意味着：
- 无法自动推导 phase 之间的数据依赖
- `forge validate --artifact-flow` 不存在——开发者只能人肉检查「上一个 phase 是否产出了我需要的东西」
- 与已有的 `DependsOn`（执行顺序依赖）是正交关系：A 在 B 之前跑 ≠ B 需要 A 的产出

**核心挑战：**
1. 工件路径是动态的（由 agent 实现决定），与声明式的 `expects:` 匹配需要「模式匹配」而非「字面量匹配」
2. 跨 phase 的工件依赖可以在循环中形成「数据流环」（A emits → B expects → B emits → A expects）——需要静态检测
3. 与已有的 `FeedsForward` 语义需要对齐——`FeedsForward` 是「prompt 上下文注入」而非「文件依赖」

**预期架构变更：**
- `Phase.Expects []string`（glob 模式，匹配 `emits:` 的路径）
- `forge validate --artifact-flow`：构建 DAG，检测未满足的 `expects:` 和环
- `RunFrom` 的 phase 排序：结合 `DependsOn`（执行序）和 `Expects`（数据流）做拓扑排序
- `gatherSignals` 可选加入 `artifact_coverage`（已满足的 expects 比例）

**对现有系统的影响：**
- 纯 additive——现有 workflow 文件不声明 `expects:` 时行为完全不变
- `forge validate` 新增一个检查项，不影响 `forge run` 运行时
- 为未来方向 A 的「数据流可视化」和方向 E 的「增量构建」奠定基础

---

### 方向 D：CLI 子命令的责任边界显式化（P1 — 开发者体验提升）

**为什么需要：** 验证文档确认 `cmdEvolve` 继承了 `cmdRun` 的 16 个 flags，即使其中部分对其语义不适用。这已经导致了一次修复（`cmdEvolve` 需要加自己的 `--max-iter` 和 `--resume` flags）。随着命令行增长，这种「继承式膨胀」会导致：
- 新开发者难以理解「为什么这个 flag 在这里」
- `forge run` 和 `forge evolve` 的 flag 文档混杂在一起
- 未来第三个子命令（如 `forge watch`）会继承一样的问题

**核心挑战：**
1. 共享 flags 中有一部分确实是共同的（`--workflow` `--mode` `--executor`），合理的共享可以接受
2. 拆分可能导致 flag 行为不一致——需要保证 `runOptsShared` 在两边行为完全一样
3. `loadWorkflow` `subcommands` map `maxLoopBack` 等代码共享需要更细致的分析——有些共享是合理的，有些是偶然的

**预期架构变更：**
- 将 `runOpts` 拆为 `runOptsShared`（两边都需要）+ `runOptsRunExtra`（仅 run）+ `runOptsEvolveExtra`（仅 evolve）
- 在 flag 注册/文档中加入 `applies_to: run, evolve` 元信息
- 将绑定的 flag 从「按变量名分类」改为「按子命令职责分类」

**对现有系统的影响：**
- 零运行时行为变更——纯代码组织重构
- 不会触发 CI 闸门（不涉及逻辑变更）
- 为 CLI 自动化文档生成（`forge --help` 按子命令输出）奠定基础

---

### 方向 E：健康检查框架 + 自诊断循环（P0 — 续方向 B，运行时韧性）

**为什么需要：** 验证文档确认 `QuickDoctorCheck` 跑一次（evolve 启动时 + engine build），从不进循环。`staleCount` 只有 2 个维度。这意味着一个常见的故障模式无法被检测：存储损坏 → 连续 10 次迭代都安全地跑步不前 → budget 烧完 → 用户发现时已经晚了。

结合方向 B 的连续收敛分数，方向 E 聚焦于**健康信号本身的设计**。

**核心挑战：**
1. 健康检查需要有明确的「严重度分级」——什么情况下 Stop，什么情况下 Warn
2. 趋势检测需要滑动窗口——引入「最近 N 次迭代的收敛分数序列」
3. 误报风险——健康检查本身需要自检（防止「诊断器本身坏了」）

**预期架构变更：**
- `LoopEngine.HealthChecks []HealthCheck`（注入式，可配置）
- 内置 HealthCheck 实现：
  - `ConvergeTrendCheck`：最近 5 次迭代的收敛分数斜率，如果连续下降则告警
  - `GateRegressionsCheck`：gate pass rate 是否在下降
  - `StorageConsistencyCheck`：`memory`/`persist` 中的数据是否自洽
  - `BudgetHealthCheck`：剩余 budget 是否足够完成剩余 roadmap
- `LoopOutcome` 扩展为包含健康状态详细报告

**对现有系统的影响：**
- additive——现有 `LoopEngine.Run` 行为不变
- 健康检查默认可关闭（`--skip-health-checks`）
- 为 v3 的带外 Sandbox 监控（Firecracker 内的 agent 健康状况）提供接口原型

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

基于当前架构的特点，我建议以下 3 条接口设计原则：

**原则 1：从消费者提取，不从生产者发明。** 观察 `internal/orchestrator` 当前是如何引用 `converge.Signals` `gate.Result` `persist.Store` 的——那些引用点就是接口方法的自然定义。例如 `orchestrator` 调用 `converge.Signals` 的方式是：

```go
// 当前：直接引用 concrete type
signals := converge.GatherSignals(...)
results := converge.Converge(signals)
```

→ 接口就是从 `GatherSignals` 和 `Converge` 这两个消费者角度提取的：

```go
type Probe interface {
    Gather(ctx context.Context, ...) (Signals, error)
    Evaluate(ctx context.Context, sig Signals) (Results, bool)
}
```

**原则 2：接口应保持 package 本地化。** 不要建一个 `internal/interfaces.go` 放所有接口。每个 package 应定义自己期望的「服务接口」，让提供者 package 去适配。这是 Go 的「接受接口，返回结构体」哲学。例如 `internal/orchestrator` 应定义 `converge.Probe`、`persist.Store`、`gate.Checker` 接口——即使 `internal/converge` 还不知道这些接口存在。

**原则 3：契约测试与接口定义同时创建。** 每定义一个接口，立即在其消费者 package 中写一个 `*_contract_test.go`，它在测试时通过一个注入的 mock 来验证接口契约。这样：
- 实现者可以独立测试（通过 mock）
- 接口的演化会触发契约测试失败
- 未来替换实现时，契约测试是 first-line 验证

### 3.2 是否需要引入新的抽象层

**需要，但要克制。** 以下是具体的抽象层决策：

| 抽象层 | 是否引入 | 理由 |
|---|---|---|
| `orchestrator/clocks` （时间抽象） | 否 | Go 标准库 `time` 已经足够，out-of-process 测试可以通过真实时间 |
| `persist.Store` / `persist.Loader` 接口 | **是** | 这是拆出独立存储服务的前提。当前是本地文件系统，未来可能是 S3/Postgres |
| `converge.Probe` 接口 | **是** | 这是「连续收敛分数」方向 B 的前提。当前 `Converge` 作为默认实现 |
| `gate.Consumer` 接口 | **是** | 这是「harness 可插拔」的前提。当前是 subprocess 调用 harness 脚本 |
| `trace.Sink` 接口 | **否** | `trace.Event` 是纯数据结构，序列化方式已经通过 `encoding/json` 抽象了 |
| `memory.Memory` 接口 | **延迟引入** | 当前 memory 包还不稳定，接口设计可能会频繁变动——等稳定后再提取 |

### 3.3 如何保持向后兼容性

对于方向 B 这种涉及核心类型变更的，需要**分三步走**：

1. **步 1（backward-compatible additive）**：给 `Converge` 返回类型加 `Score()` 方法，保留 `Met()`。当前 `met == bool` 的内部逻辑不变。
   ```go
   type Result struct { ... } // 不变
   type ConvergenceScore struct {
       Percentage float64
       Details    map[string]float64
   }
   type ConvergenceOutcome struct {
       Results []Result
       Score   ConvergenceScore
   }
   func (o ConvergenceOutcome) Met() bool { return o.Score.Percentage >= 1.0 }
   ```

2. **步 2（deprecation window）**：所有调用方从 `Converge` 返回值取 `Met()` 改为取 `Score().Percentage >= threshold`

3. **步 3（break change）**：移除旧的 `(Results, bool)` 签名，只保留 `ConvergenceOutcome`

这种模式已在 Go stdlib 中被多处验证（`context` 包的演化、`io` 包的接口添加）。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

**基于当前阶段和架构纪律，不建议引入新的核心依赖。** 理由：

1. **Go 标准库已覆盖当前所有需求。** `forge-core` 13 个包全部使用纯 stdlib——`flag` `os/exec` `encoding/json` `net/http` `sync` `context`——在这些包之上再加框架不会带来额外收益，反而引入版本管理和安全扫描的开销。

2. **唯一例外：YAML 解析。** ROADMAP 诚实声明了 Go 标准库无 YAML 解析器，当前通过 `python3 harness/yaml2json.py` shim 转码。这是合理的临时方案。对于 YAML 库的引入，建议：
   - 不引入 `gopkg.in/yaml.v3` 等外部库——违反「零外部依赖」纪律
   - 等 YAML 解析需求稳定后（workflow schema 不再频繁变动），**用 Go 标准库自行实现最小子集**——只解析 forge-core 真正使用的 YAML 结构（mapping + sequence + scalar + block scalars），不需要完整 YAML 规范兼容
   - 或者：等 Go 标准库原生支持 YAML（Go 2 或 proposal）

3. **需要引入的是「接口」而非「框架」。** 方向 A 和方向 B 的技术选型决策不是「选什么库」，而是「定义什么接口」：

| 需求 | 实现方案 | 为什么不是框架 |
|---|---|---|
| 包间契约 | Go interface + contract test | interface 是语言内置特性，零依赖 |
| 健康检查 | `[]HealthCheck` 切片 + 策略模式 | 不需要规则引擎，每个 check 是一个 struct |
| 连续收敛分数 | struct + 权重组合 | 计算逻辑固定，不需要规则引擎 |
| 数据流验证 | DAG 拓扑排序 | Go 标准库的 `sort` 包足够实现 Kahn 算法 |
| 测试替身 | `*_contract_test.go` + mock struct | Go testing 包原生支持 |

### 4.2 第三方依赖的评估标准

当未来 v3 确实需要引入外部依赖时，建议采用以下评估标准（作为 `.agent/DECISIONS.md` 的新条目）：

**硬性红线（不满足则否决）：**
1. 必须与 Go 标准库不冲突（不重复实现 net/http / encoding/json 等）
2. license 必须兼容（MIT / Apache 2.0 / BSD，排除 GPL/AGPL）
3. 必须被 arch-check 纳入扇入计算（依赖算进被依赖包的扇入）
4. 必须提供 contract test 接口（不能是黑盒二进制）

**评分维度（5 分制，总分 ≥ 20 才引入）：**
1. **必要性**：是否无法用 Go 标准库 + 自研实现？（5 = stdlib 完全不行，1 = stdlib 可以但麻烦）
2. **稳定性**：v1.0 以上 + 向后兼容承诺（5 = 已 v2.0+，1 = < v1.0）
3. **安全记录**：过去 12 个月 CVE 数（5 = 0 CVE，1 = ≥5 CVE）
4. **社区活跃**：commit 频率 + issue 响应（5 = 周更 + 24h 响应，1 = 年更）
5. **移植成本**：如果替换，需要改多少代码（5 = 接口替换 1 个文件，1 = 散落 50+ 文件）
6. **构建影响**：二进制体积增加（5 = < 1MB，1 = > 50MB）
7. **传递依赖**：是否引入自己的依赖（5 = 零传递，1 = ≥10 传递）

**当前建议**：**零外部依赖保持到 v3 边界条件触发。** 触发条件建议为：「forge-core 需要与外部服务通信（Postgres / Temporal / NATS）且 Go 标准库的 net/http + protobuf 最简实现确实不够用」时，才引入最小化的客户端库。

### 4.3 自建 vs 采购的决策依据

North-star 架构已经做了一个很好的分层决策：

| 品类 | 决策 | 逻辑 |
|---|---|---|
| **编排引擎**（Temporal） | **采购** | 分布式 durable execution 是极其困难的问题，自研不划算 |
| **策略引擎**（OPA/Rego） | **采购** | PDP/PEP 分离模式已被大量生产验证 |
| **沙箱**（Firecracker） | **采购** | 安全隔离不是核心竞争力，KVM 级隔离自研不起 |
| **模型路由决策** | **自研** | 这是 ForgeOS 的核心算法护城河 |
| **Context 装配** | **自研** | 与 ForgeOS 的角色体系 / workflow 深度绑定 |
| **记分卡 / Eval** | **自研** | 这是与使用量共同增长的数据飞轮 |
| **网关**（Envoy） | **采购** | 标准的 7 层网关，不需要自研 |
| **向量数据库**（Qdrant） | **采购** | 专业引擎替换自研的 TF-IDF，是合理的演化路径 |

我的补充建议：**将「自研/采购」决策与包间接口提前对应。** 例如：
- `persist.Store` 接口当前由本地文件系统实现 → 当需要 Postgres 时，新增 `internal/persist/postgres.go` 实现同一接口
- `trace.Sink` 当前输出 JSON 文件 → 当需要 OTel 时，新增 `internal/trace/otel.go` 实现
- 接口设计的正确性是未来采购/自研替换的关键基础设施

---

## 5. 实施路线图

### 5.1 优先级排序

基于验证文档的分析和项目当前阶段（v2 已落地，趋势向好，但存在 3 个 P0 的架构安全隐患）：

| 方向 | 优先级 | 理由 | 估算工作量 |
|---|---|---|---|
| **B: 连续收敛 + 健康检查** | **P0** | 这是 LoopEngine 的核心智能。当前二值判定导致「无法做趋势判断、无法做 partial credit、无法做停滞检测」。project.yml 中已有 `stop_condition` 的结构定义，接线只需 2-3 个 sprint | 2-3 sprint |
| **E: 健康检查框架** | **P0** | 接 B 之后立即做。健康检查是 LoopEngine 从「哑巴循环」进化到「智能循环」的关键。当前 `QuickDoctorCheck` 跑一次就走的模式不可持续 | 1-2 sprint |
| **A: 包间接口契约** | **P1** | 当前 13 个包还能 manage，但项目正在增长。在当前规模下加接口是最低成本的时机。越晚做，改的调用方越多 | 2-3 sprint |
| **C: Phase 工件依赖** | **P1** | 低风险高收益。纯 additive，不涉及运行时行为变更。适合「带薪休假回来第一个 sprint」的轻松任务 | 1 sprint |
| **D: CLI 子命令界限** | **P2** | 纯开发者体验改进，不影响用户可见行为。可在其他 sprint 的间隙做 | 0.5 sprint |

### 5.2 阶段划分和里程碑

**阶段 0（前序条件 — 已满足）：** forge-core 已落地、全绿、13 包已跑通 `forge accept`。✓

**阶段 1（Sprint N ~ N+2，预计 3 sprint）：收敛智能**

- 里程碑 1.1：`Converge` 返回类型从 `(Results, bool)` 改为 `(Results, ConvergenceScore)`，保留 `Met()` 向后兼容
- 里程碑 1.2：`LoopEngine` 新增 `HealthCheck` 注入点 + `[]HealthCheck` 切片
- 里程碑 1.3：两个内置健康检查就绪：`ConvergeTrendCheck`（滑动窗口斜率）+ `GateRegressionsCheck`
- 里程碑 1.4：`staleCount` 从 2 维扩展为多维指标加权
- 里程碑 1.5：`forge run --explain-scores` 输出迭代健康报告
- 闸门：`forge accept` 全绿 + fresh-context reviewer APPROVE

**阶段 2（Sprint N+3 ~ N+5，预计 3 sprint）：结构硬化**

- 里程碑 2.1：`converge.Probe` 接口定义 + `Converge` 作为默认实现
- 里程碑 2.2：`persist.Store` 接口定义 + `FileStore` 作为默认实现
- 里程碑 2.3：两个接口的 contract test 就绪（`orchestrator/converge_contract_test.go` + `orchestrator/persist_contract_test.go`）
- 里程碑 2.4：`forge validate --artifact-flow`（Phase 依赖图检测）
- 里程碑 2.5：`Phase.Expects` 字段 + workflow 文档更新
- 里程碑 2.6：CLI flags 按 `runOptsShared` / `runOptsRunExtra` / `runOptsEvolveExtra` 拆分
- 闸门：`forge accept` 全绿 + `go test -race` 全绿 + fresh-context reviewer APPROVE

**阶段 3（Sprint N+6 ~ N+8，预计 3 sprint）：闭环强化**

- 里程碑 3.1：健康检查输出接入 `trace` 事件系统（每次迭代的健康状态被持久化）
- 里程碑 3.2：`BudgetHealthCheck`（剩余 budget vs 剩余 roadmap 的预估消耗对比）
- 里程碑 3.3：`StorageConsistencyCheck`（`memory` + `persist` 数据自洽验证）
- 里程碑 3.4：健康检查策略化——`modes.yml` 中声明 `health_check` 的阈值和严格度
- 里程碑 3.5：`forge diagnose` 命令——输出当前项目的健康报告
- 闸门：`forge accept` 全绿 + fresh-context reviewer APPROVE + 可选：真 claude 端到端验证一次

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| **收敛分数算法引发争议** | 中 | 高 | 在 `.agent/DECISIONS.md` 中明确记录算法设计决策，提供 `--explain-scores` flag 让开发者 inspect 分数计算过程 |
| **接口过度设计**（在 13 包阶段加了 10 个接口，结果都是单实现） | 中 | 低 | 遵循「从消费者提取」原则——不预先发明接口，从 `orchestrator` 的 import 图中提取。如果包 A 当前只有 1 个调用方，不要为「未来可能的调用方」加接口 |
| **健康检查误报导致正常迭代被终止** | 低 | 高 | 每个健康检查必须有明确的 False Positive Rate 控制。默认只在 `warn` 级别输出（不打断循环），`commit` 级别才会 Stop。v1 阶段所有 HealthCheck 默认 open（只报告不阻断） |
| **Contract test 变成「虚假安全」（测试通过但生产行为不同）** | 中 | 中 | Contract test 必须包含**反例测试**——验证 mock 返回 error 时消费者正确处理。不只要测「接口返回正确结果」，还要测「接口返回错误时消费者不崩溃」 |
| **连续收敛分数导致开发者对「为什么没达到 100%」的认知负担增加** | 中 | 低 | `forge run --verbose` 输出每个子维度的分数和权重，让开发者可以精确理解「差在哪里」。这是从「黑盒通过/不通过」到「白盒每个维度的进度」的必然成本 |
| **Phase.Expects 的 glob 匹配与真实 agent 产出路径不一致** | 中 | 低 | `Expects` 采用 glob 模式匹配（而非精确路径匹配），与 `Emits` 的路径模式对称。`forge validate` 阶段就做匹配验证，不在运行时做 |
| **架构拆分的 Team 承接力**（团队是否适应同时维护接口 + 实现） | 低 | 中 | 接口契约的压力其实是好事——迫使开发者更清晰地思考「我的包对外的承诺是什么」。建议在 `.agent/AGENTS.md` 中新增一项红线：「新增跨包 public type 必须同步定义接口或做出不定义接口的 ADR 决策」 |

---

## 总结

ForgeOS 当前处在**架构健康度较高的 monolith 阶段**——有清晰的中枢旋钮模式、严格的工程纪律、真实的 dogfood 验证。验证文档确认的 5 个方向都很精确，其中 P0 的两个方向（连续收敛判定 + 自诊断循环）是 LoopEngine 从「哑巴循环」进化为「智能循环」的关键。

我的核心建议总结为三点：

1. **先做收敛分数（方向 B），再做接口（方向 A）。** 收敛分数是核心行为变更，需要更多思考和验证时间。接口是纯 additive abstraction，可以在连续收敛分数稳定之后再提取。当前 13 包不会在 2-3 个 sprint 内膨胀到不可管理。

2. **保持零外部依赖纪律。** 方向 A~E 的所有变更都可以通过 Go 标准库 + 接口 + contract test 完成。YAML 解析是唯一合理的例外，建议用 minimal parser 代替 python shim——但这不是影响架构的方向性决策。

3. **在拆出独立服务前，先在 monolith 中建立接口契约和健康检查框架。** 这是 north-star 分布式架构成功的 precondition——如果 monolith 内部已经是「接口清晰、契约可测、健康可检」的架构，拆服务时只需要把接口实现从「本地调用」改为「gRPC 调用」。否则拆服务时会陷入「先做架构重构、再做服务拆分」的双重风险。
