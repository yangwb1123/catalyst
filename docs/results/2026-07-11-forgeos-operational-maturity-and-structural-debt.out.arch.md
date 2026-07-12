现在我有足够的上下文来生成全面的架构分析。以下是分析报告。

---

# 架构分析报告：ForgeOS `forge-core` 运行时 + 治理平面

## 1. 架构评估

### 1.1 当前架构的优势

**治理优先的设计哲学是正确的**。ForgeOS 最独特的架构决策——将带外执法（harness 闸门）作为真相之源，而非依赖宿主工具链内置的钩子——是本系统最具差异化的价值。这一决策使得：

- **宿主无关性**：`gate.mjs` / `arch-check.mjs` / `secret-scan` 可在任何 CI 中独立运行，不受 Claude Code / Codex / Gemini CLI 的行为差异影响。这符合 K8s 式控制面 / 数据面分离原则（north-star.md 原则 1）。
- **增量迁移路径**：v0（纯 Claude Code 编排）→ v2（自研 Go 运行时）的迁移是数据面替换，控制面（harness）保持不变。这是教科书级的架构演进。

**中枢旋钮（`mode × lifecycle`）是三处驱动的单一设置点**，其设计质量远超多数同类系统：

| 被驱动方 | 机制 | 状态 |
|---|---|---|
| Router 档位 | `mode` → model tier floor + safety override | ✅ 闭环 |
| Harness 严格度 | `lifecycle` → gate-set + enforce level + coverage threshold | ✅ 闭环 |
| Workflow 深度 | `mode` → discover/design/review/evolve depth | ✅ 闭环（Sprint 15/27） |
| Migration | `forge migrate --to engineering` → 5 补债任务 | ✅ 闭环 |

这比 YAML 中独立的 `environment: prod` 与 `log_level: debug` 开关更优雅，因为它**强制一致性**——没有 `mode: explorer` 却跑全闸门的矛盾状态。

**`cmd/forge` 包的「500 行阈值 + 包边界提取」纪律在实践中有效**。数据验证了这一点：当前 42 个生产文件，仅 4 个在 490–500 行区间（`main.go` 499, `engine_build.go` 498, `evolve.go` 496, `gates.go` 493），且全部处于被监控状态。对比 Sprint 27 的 `validate.go` 994 行，纪律已产生实际约束力。

**零外部依赖的 Go 运行时是一个有意识且正确的权衡**。`forge-core` 13 个内部包全部纯标准库，这意味着：无 supply-chain 风险、`go build` 在任何 Go 版本上均不会因间接依赖破损而失败、静态二进制分发的部署模型极其简单。这与 north-star 中描述的 Temporal/Postgres/NATS/OPA 等采购组件形成了清晰的「现用 vs 目标」分层——现有实现不会因为未来引入这些组件而被废弃。

### 1.2 当前架构的局限性

**`cmd/forge` 包存在结构性债务**。42 个文件中 16 个是测试，26 个生产文件。虽然 500 行纪律在强制执行，但 CLI 层的职责边界仍然不清晰：

- `cost.go`（471 行）混合了：Claude-specific JSON 解析、预算计算、成本遥测、agent 裁定的机读契约解析（`parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`）。同一个文件有三个以上不同的抽象层次。
- `gates.go`（493 行）包含：闸门裁决逻辑、N/A 豁免矩阵、信号收集、收敛报告——其中大部分逻辑已在 Sprint 29 部分提取到 `internal/gate/resolve.go`，但仍有逻辑残留在 CLI 层。
- `prompt_context.go`（454 行）混合了：prompt 组装、context 注入、任务注入、观察者模式回调注册——可以按自然缝拆出 `prompt_inject.go` / `prompt_observe.go`。

**根本原因**：`cmd/forge` 是 v0→v2 迁移的遗留物。最初它是一个单一的 CLI 入口，随着 forge-core 内部包的逐步提取，CLI 层本应变薄，但实际上它在三种力量下膨胀了：（1）新功能在 CLI 层原型化的便利性，（2）内部包提取需要多次 sprint 周期才能完成，（3）package.max_files 预算被重复谈判而非强制执行。

**第二类架构债务：`internal/orchestrator` 包的认知负荷**。该包包含 7 个生产文件 + 9 个测试文件，覆盖了：Orchestrator 引擎（`orchestrator.go` 494 行）、LoopEngine（`loop.go` 407 行）、ModeGating（`mode_gating.go`）、CommandExecutor（`command_executor.go` 349 行）、Budget（`budget.go`）、ReviewGating（`review_gating.go`）等。这是正确的包职责（编排确实是核心），但 `orchestrator.go` 的 494 行几乎达到文件上限，其内部可能容纳了多种职责。

**第三：`internal/yaml2json` 包的临时性被永久化了**。该包作为 Python `yaml2json.py` shim 的 Go 替代品而编写，但 YAML 解析不是 forge-core 的核心职责，且 Go 标准库没有 YAML 解析器——这意味着要么接受该包是手写的（`normalize.go` 264 行 + `parser.go` ?行），要么引入外部 YAML 依赖（违反零依赖原则）。当前的解决方案是手写解析器，这在实现复杂度上是可接受的（Sprint 27 已修复 block-scalar 损坏），但对于长期演进，YAML 解析的维护成本将落在 forge-core 团队身上。

**第四：收敛模型（`converge.Signals`）是一种经过精心设计的临时方案**。它通过 8 个字段覆盖了所有收敛条件，但每个信号都有自己的数据源（机读契约、git diff、gate 裁决、agent 卡声明），没有统一的信号总线或事件日志。这在当前规模下是合理的（~15 个内部包，~200 个源文件），但如果系统继续扩展，信号收集点将散布在代码库各处，缺乏可审计性。

### 1.3 关键设计决策评估

| 决策 | 评价 | 理由 |
|---|---|---|
| 带外执法为真相之源 | ✅ 正确 | 宿主无关性是系统护城河；CC hook 只是加速器 |
| 零外部 Go 依赖 | ✅ 正确（截至当前规模） | 截至 13 个内部包，标准库足够；若引入 YAML/JSON schema 验证等需求则需重新评估 |
| 中枢旋钮三处驱动 | ✅ 极佳 | 单一设置点 + 一致性 enforcement；对比 TDD 中的 "single point of truth" 原则 |
| 声明式 agent 卡 + YAML workflow | ✅ 正确 | 声明式优于指令式编排；YAML 的可读性与可审计性高于 Go 的 AST |
| fresh-context reviewer 分离 | ✅ 正确（强制） | AGENTS.md 明文规定；Sprint 27 多轮审查发现了真正的 bug |
| `cmd/forge` 包提取为内部包 | 🟡 方向正确但执行不完整 | `cost.go`/`gates.go`/`prompt_context.go` 仍混合多职责 |
| 500 行硬限 | ✅ 正确 | 数据表明它能有效约束；Sprint 27 的 994→拆分已验证 |
| Python YAML shim 作为长期方案 | ❌ 应替换 | Go 手写解析器的维护成本 > 引入一个小型 YAML 库的成本 |
| `forge run` 和 `forge evolve` 共享同一套 trace 旋转逻辑 | 🟡 中间方案 | 当前实现是正确的（同一个 `openTracer` 函数），但配额策略过于简单 |

---

## 2. 高价值架构扩展方向

### 方向 A：CLI 职责的「组织架构映射」重构（P1）

**为什么需要**：`cmd/forge` 目前是 ForgeOS 的「上帝包」反模式——它应该是一个薄 CLI 层，但包含了信号收集、prompt 组装、成本计算、机读契约解析等功能。这是一个技术债务问题，根据经验法则，架构债务在跨越 20 个生产文件的阈值后会变得昂贵（当前 26 个生产文件）。

**核心挑战**：
- 职责边界不明确：`cost.go` 中的机读契约解析是偶然复杂度（因为嵌入在 prompt 输出中的 `VERDICT:` 标记是由 JSON 响应解析器抽取的，而 JSON 解析在 `cost.go` 中是因为 `cost.go` 已经是「通用 JSON 解析器」）。
- 反向依赖：`cmd/forge` 的测试文件（16 个测试文件，~4500 行）与当前的结构耦合。提取操作需要移动测试以及生产代码。

**预期的架构变更**：

```
当前状态：
cmd/forge/
├── cost.go          (471行: JSON解析 + 预计算价 + 机读契约 + 执行环境构筑)
├── gates.go         (493行: 闸门裁决 + 信号收集 + 收敛报告)
├── prompt_context.go(454行: prompt组装 + 上下文注入 + 任务注入 + 回调)
└── ...

目标状态：
cmd/forge/            ← 仅 CLI 胶水（解析 flag + 调用内部包 + 格式化输出）
internal/parser/      ← JSON/shell 输出解析器（新包，从 cost.go 提取）
internal/signal/      ← 信号总线（从 gates.go + converge 提取）
internal/prompt/      ← prompt 组装 + 注入（从 prompt_context.go 提取）
```

**对现有系统的影响**：
- 高比特精确度要求：每个提取必须保持 `go test -race` 全绿，且 `forge accept` ACCEPTED。Sprint 29 的 `gate_resolve.go` 迁入 `internal/gate/resolve.go` 是先例。
- 影响范围大：`cost.go` 被 ~10 个文件 import（包括测试）。提取需要更新所有导入路径。

**架构权衡**：
- *方案 A*：按函数组提取（`internal/parser` 含所有 JSON/文本解析，`internal/signal` 含所有信号逻辑）。这是最快路径，但可能导致新包是略加组织的同一堆逻辑。
- *方案 B*：按数据流提取（`internal/contract` = 机读契约解析，`internal/telemetry` = 成本/延迟遥测，`internal/prompt` = prompt 组装）。这是概念上更清洁的方案，但需要更仔细地解耦当前互相交织的代码。
- *推荐*：方案 B，因为「快速重组」只会将债务从 `cmd/forge` 转移到新包，而不会消除潜在的概念耦合。

### 方向 B：信号总线抽象（P1-P2）

**为什么需要**：`converge.Signals` 目前是一个结构体，其 8 个字段由分布在 4 个不同包（`cmd/forge/gates.go`、`internal/converge`、`internal/orchestrator`）中的代码填充。这是一个「隐式耦合」问题——没有注册中心或总线来回答「哪些信号由哪些组件生产，哪些组件消费它们？」的问题。

随着 forge-core 的演进，信号的种类将会增加（`review_status`、`requirement_confidence`、`file_delta` 都是 Sprint 28-29 才接通的），每次增加都需要：（1）在 `Signals` 结构体中新增字段，（2）在 `gatherSignals` 中新增赋值，（3）在至少一个 `eval*` 函数中新增消费者。这是一个三点变更，容易出错（例如 Sprint 28 中的 `review_status`，它已在 struct 中声明但在 `gatherSignals` 中未被赋值，导致 `forge run review` 永远无法收敛）。

**核心挑战**：
- 过设计风险：信号总线可能过于通用，增加不必要的间接层，而当前 8 个信号的结构体方法已经工作。
- 性能：如果总线是同步的且所有信号在收敛评估前被收集，则没有显著的性能问题。异步总线则会增加延迟。
- 信号间的依赖关系：`review_status` 和 `requirement_confidence` 是独立的，但 `gates_status` 和 `gate_proof` 是相关的——总线必须支持信号间依赖关系的显式建模。

**预期的架构变更**：

```go
// 概念设计
type SignalRegistry struct {
    producers map[SignalKind][]Producer
    consumers map[SignalKind][]Consumer
}

signals.Register(SignalReviewStatus, reviewStatusProducer)
signals.Register(SignalReviewStatus, evalReviewStatusConsumer)
```

**对现有系统的影响**：
- 如果实现为 `internal/signals` 包，导入图低风险（叶子包）。
- 现有代码需要从 `gatherSignals` / `eval*` 迁移到生产者-消费者注册。这是一个重构，可以将 `converge.Signals` 作为适配器暂时保留（双运行模式）。

**风险**：如果实现过于复杂，可能变成「一个函数调用」的包装器。**关键约束**：信号总线不应比它所替代的显式结构体更长或更复杂。

### 方向 C：YAML 处理的「依赖审计」决策（P2）

**为什么需要**：`internal/yaml2json`（手写解析器）是 forge-core 零外部依赖原则的产物。这是一个经过权衡的决策，但它带来了维护成本。Sprint 27 的 block-scalar 错误就是一个具体例子——一个手写解析器需要覆盖 YAML 规范的全部 corner case。

**核心挑战**：
- Go 标准库没有 YAML 解析器（`encoding/json` 有，`gopkg.in/yaml.v3` 是外部依赖）。
- 引入外部依赖会破坏 forge-core 的「零依赖」承诺，这在当前是项目的一个重要差异化因素。
- YAML 在 forge-core 中的使用是有界的：仅限于解析 5 个 workflow 文件 + 配置文件。不是通用的 YAML 处理器。

**选项分析**：

| 选项 | 维护成本 | 依赖成本 | 推荐度 |
|---|---|---|---|
| 保持手写解析器 | 持续（修复 corner case） | 无 | 🟡 中间方案 |
| 引入 `gopkg.in/yaml.v3`（仅此一个依赖） | 低 | 1 个依赖 | 🟢 推荐——前提是明确声明为唯一外部依赖 |
| go-yaml v2（更稳定但更少功能） | 中 | 1 个依赖 | 🟡 可接受 |
| 继续用 Python shim（无 Go 解析器） | 中（per-call Python 进程开销） | Python 运行时依赖 | ❌ 已否决（Sprint 27 已验证 Go 解析器的必要性） |

**推荐**：在 v2 road 上明确划定一条线——forge-core 允许**恰好一个**外部依赖（YAML 解析器），将其作为 conscious 决策记录下来，而不破坏「零其他依赖」原则。

**对现有系统的影响**：
- 如果切换为 `gopkg.in/yaml.v3`，`internal/yaml2json` 的所有解析逻辑可以被 ~50 行胶水代码替代。这是 200+ 行代码的净减少。
- 但是，`internal/yaml2json` 的测试（394 行）中许多是特定于手写解析器行为的。切换到库可能需要重写这些测试。

### 方向 D：`internal/orchestrator` 的「包内包」重构（P2）

**为什么需要**：`internal/orchestrator` 包含的职责超过了单一 Go 包的合理范围。它涵盖了引擎、循环、执行器、预算、门控和模式门控。该包的认知负荷很高（7 个生产文件，~2000 行），且其平均文件大小（~285 行）是所有内部包中最高的。

**核心挑战**：
- `Internal` 包不能被该仓库外的代码导入，因此提取到更多的内部包是可行的，但命名冲突需要避免。
- 循环依赖风险：`orchestrator` 依赖于 `converge`、`asset`、`gate` 等，如果提取，新包可能会创建新的循环。

**预期的架构变更**：

```
当前：internal/orchestrator/ (7 files, ~2000 lines)
├── orchestrator.go          (494行: 引擎 + Run/RunParallel)
├── loop.go                  (407行: LoopEngine)
├── command_executor.go      (349行: 命令执行 + agent spawn)
├── mode_gating.go           (???行: 中枢旋钮集成)
├── review_gating.go         (???行: review段门控)
├── budget.go                (???行: agent调用预算)
└── *_test.go                (9 files)

目标：internal/orchestrator/ (控制面核心)
├── engine.go                ← orchestrator.go 拆出（仅引擎）
├── run.go                   ← orchestrator.go 拆出（仅 Run/RunParallel）
├── loop.go                  (保持, LoopEngine核心)
├── budget.go                (保持, 预算守卫)
├── mode_gating.go           (保持, 中枢旋钮)
└── review_gating.go         (保持, review段门控)

internal/executor/ (执行器 + agent spawn)
├── executor.go              ← command_executor.go 迁入
├── executor_unix.go
└── executor_test.go
```

**对现有系统的影响**：
- `internal/orchestrator` 目前被 `cmd/forge` 导入。如果 `command_executor` 被提取到 `internal/executor`，`cmd/forge` 需要新增一个导入——但这是安全的（叶子包，无上游依赖）。
- `internal/executor` 会是一个纯叶子包，导入 `internal/trace`、`internal/persist`，但不导入 `internal/orchestrator` 本身。这打破了一个隐式循环依赖风险（`orchestrator → executor → trace/persist ← orchestrator` 本不存在，但 `command_executor.go` 在 orchestrator 包中时，无法通过静态分析捕获此风险）。

### 方向 E：`forge maintain` 与磁盘配额管理（P3→P1 作为方向三的增量）

**为什么需要**：输入文档正确指出，方向三的部分内容已在之前的分析中被覆盖（RunID、`.forge/` 生命周期），但 `forge maintain` 命令和磁盘配额管理是真正的增量增量。目前，`.forge/` 目录（包含 trace、checkpoint 和 memory 数据）在活跃使用时可能无限制增长，且没有用户可见的维护命令。

**核心挑战**：
- `forge maintain` 的职责范围：包含 `forge gc`（垃圾回收过期 trace）、`forge doctor --fix`（修复元数据不一致）、`forge quota`（报告磁盘使用情况）和 `forge prune`（清理检查点）？
- 配额策略：目前 trace 10MB 旋转是唯一的磁盘管理。对于大型项目，单次 `forge evolve` 运行可能产生 > 10MB 数据（每个迭代有一个 trace + checkpoint + memory file）。没有全局配额。
- 用户可见性：磁盘配额是一个平台问题，而非用户特性——如果做得不好，用户会看到「磁盘空间不足」错误，而非优雅的配额拒绝。

**对现有系统的影响**：
- 新命令需要 `cmd/forge/main.go` 中的新入口点 + 新 CLI 胶水文件。
- 核心逻辑（配额计算、GC 策略）应放在 `internal/maintain` 包中。
- `forge doctor --fix` 目前是 `internal/doctor` 的一部分。应明确决定：`forge maintain` 是否包含 `doctor`（作为子命令），还是保持独立。**推荐 `forge maintain` 作为父命令，通过子命令聚合 GC、quota、doctor 和 prune**。

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则 1：接口在消费者侧定义（Go 惯例）**。ForgeOS 已遵循此惯例——`internal/orchestrator` 定义了自己的接口，实际上并不导入 `internal/converge` 的类型，而是依赖于参数传递。这是正确的；应继续保持。

**原则 2：新的内部包的导出面应 ≤ 3 个公开符号**。对于每个从 `cmd/forge` 提取到 `internal/` 的新包，应有一个纪律性的上限，防止新包成为另一个 `cmd/forge`。Sprint 29 的 `internal/gate/resolve.go` 导出 3 个符号是正确的（`GatesGreen`、`ResolveGate`、`HarnessRunner`）。

**原则 3：信号总线接口应足够通用以支持新信号，但足够具体以不需要类型断言**。当前 `converge.Signals` 结构体方法（带命名字段）比 `map[string]interface{}` 更可取，因为：（1）编译时类型安全，（2）文档化的字段名，（3）在所有 8 个信号不变时不需变化。如果信号总线被抽象化，应保持类型安全：

```go
// 不推荐：泛型信号总线
type Signal struct {
    Kind  string
    Value interface{} // 类型安全丢失
}

// 推荐：类型化信号注册 + 访问器
type Registry struct {
    signals map[SignalKind]float64 // 所有已知信号最终归一化为 float64
}
```

### 3.2 是否需要引入新的抽象层

**包级别抽象层（`internal/` 子域）**：是的，但有限制。`internal/orchestrator` 当前结构良好（没有跨越包边界的循环），其内部各文件之间的接口是隐式的（`orchestrator.go` 调用 `loop.go` 函数）。引入显式接口会不必要地增加间接层。**仅在提取新包需要时才引入抽象**。

**提供一个「CLI 中间件」层**：考虑在 `cmd/forge` 中引入一种中间件模式，适用于以下横切关注点：flag 解析、子命令调度、JSON 输出格式化和错误处理。当前，每个命令文件（`run.go`、`evolve.go`、`validate.go` 等）都独立处理这些关注点。虽然对于 12 个 CLI 入口点来说不是紧迫问题，但创建一个小型 `cmd/forge/middleware.go` 来包装通用行为可能是有益的。

### 3.3 如何保持向后兼容性

ForgeOS 有一个内置的兼容性保障：**`forge accept` 闸门**。任何提取或重构必须保持 `forge accept` 为 ACCEPTED。这是一个强大的回归保护，但需要注意：

- **测试位移**：当从 `cmd/forge/cost.go` 中提取函数到 `internal/parser/` 时，测试必须跟随。一个常见错误是只移动生产代码而不移动测试，导致覆盖率下降和代码漂移。
- **与 `forge run`/`forge evolve` 的用户可见行为**：CLI 的输出格式（flag 名、JSON 模式、错误消息）是用户可见的契约。提取不得更改 `forge run --help` 或 `forge evolve --dry-run` 的输出。
- **`.forge/` 数据格式**：trace、checkpoint 和 memory 数据的持久化格式隐式是公共契约。任何对 `internal/trace`、`internal/persist` 或 `internal/memory` 的更改必须检查其数据是否向后兼容读取。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

**不需要新的运行时框架**。ForgeOS 的演进路径（v0→v2→v3）已明确规划，建议遵循现有路径。引入一个框架（如 Cobra 用于 CLI，Wire 用于 DI）将继续推进 forge-core 的零依赖原则。

**但需要考虑一个工具化依赖**：`gopkg.in/yaml.v3` 作为 forge-core 唯一的外部 Go 依赖。理由：

- 手写解析器的维护成本已被证明（Sprint 27 block-scalar 修复）。
- YAML 不是 forge-core 的核心竞争力；决定不做自己的 YAML 解析器是可以接受的。
- 如果问题不是关于一个库的引入，而是关于「零依赖」品牌，那么更名称确切：零依赖指的是运行时依赖（数据库、消息队列），而非低级格式库。

**评估标准**：任何引入的第三方依赖应满足以下标准：

1. 稳定：SemVer 稳定 ≥ 2 年，无破坏性版本变更。
2. 范围明确：仅解决一个格式问题（YAML 解析）。
3. 零传递依赖：`gopkg.in/yaml.v3` 符合此条件（仅有 x/sys 的 go.sum 条目，但无运行时依赖）。
4. 可移除：如果将来 Go 标准库获得 YAML 支持，替换应简单到一行 import 变更。

### 4.2 第三方依赖的评估标准

| 标准 | 权重 | 说明 |
|---|---|---|
| 零传递依赖 | 强制 | 不引入间接依赖 |
| 稳定 API（SemVer） | 强制 | 防止 CI 因为传递依赖更新而意外中断 |
| 原生 Go 实现 | 高 | 避免 CGo 绑定（构建复杂性 + 交叉编译问题） |
| 维护活跃度 | 中 | 标准库的缺陷修复速度 |
| Go 版本兼容性 | 中 | 至少支持 forge-core 的 Go 版本（当前 go.mod 版本） |
| 测试覆盖 | 中 | 上游测试质量反映可靠性 |

### 4.3 自建 vs 采购

ForgeOS 当前遵循的模型很好：**核心编排自研，基础设施采购（或借用）**。

未来需要在以下方面做出决定：

| 组件 | 当前状态 | 建议 | 理由 |
|---|---|---|---|
| YAML 解析器 | 自建（手写） | 采购（gopkg.in/yaml.v3） | 非核心竞争力；用 50 行胶水替换 200+ 行维护 |
| 模型路由器 | 自研（`internal/routing`） | 自研 | 是 ForgeOS 核心技术；多维评分器与记分卡回灌是核心差异 |
| 沙箱隔离 | 未实现 | 采购（Firecracker v3） | 竞争力依赖的是隔离模型而非实现细节；Firecracker 是行业标准 |
| 跨厂商模型池 | 未实现 | 采购（LiteLLM v3） | 模型供应商网关是 commodity，自研是镀金 |
| 向量数据库 | 未实现 | 采购（Qdrant v3） | 向量搜索是附属能力，非核心编排能力 |
| Web UI | 未实现 | 自研（Next.js v3） | 用户体验是差异化因素，但 v3 优先级低 |
| 工作流引擎 | 自研（LoopEngine + converge） | 自研 | 这是 ForgeOS 核心——收敛驱动的编排是独特设计，Temporal 作为 v3 目标但当前迭代模型是自定义的 |

---

## 5. 实施路线图

### 5.1 优先级排序

| 方向 | 优先级 | 识别 | 估计投入 |
|---|---|---|---|
| **A. CLI 职责提取（`cmd/forge` → `internal/`）** | **P0** | 架构债务；当前 26 个生产文件跨越自然包边界 | 2-3 sprint |
| **E. `forge maintain` + 磁盘配额** | **P1** | 输入文档方向三的增量；处理用户可见的运维差距 | 1-2 sprint |
| **B. 信号总线抽象** | **P1-P2** | 减少增加新信号的认知负荷，提升可审计性 | 1 sprint |
| **D. `internal/orchestrator` 提取 `internal/executor`** | **P2** | 消除隐式循环依赖风险，降低认知负荷 | 1 sprint |
| **C. YAML 依赖审计** | **P2** | 将 200+ 行维护代码替换为 50+ 行胶水代码 | 0.5 sprint |
| **RunID 方案（方向三完整版）** | **P1** | 作为跨存储一致性的基础 | 已在 five-unbuilt-product-architectural-extensions.md 方向二中覆盖，此处不重复 |
| **方向四：解析失败写入 trace（v1 子集）** | **P1** | 低成本高回报（~0.1 sprint） | 0.1 sprint |

### 5.2 阶段划分和里程碑

#### 阶段 1：架构基础（Sprint N+1 至 N+3）—— 聚焦 P0

**目标**：将 `cmd/forge` 从「上帝 CLI 包」重构为「薄 CLI + 专注的内部包」。

**里程碑 M1**（Sprint N+1 末）：`cost.go` 提取
- 提取机读契约解析 → `internal/contract`（新包）
- 提取成本遥测 → `internal/telemetry`（新包，或合并到现有包）
- `cost.go` 大小从 471 行下降至 < 250 行
- `forge accept` 仍为 ACCEPTED；所有测试全绿

**里程碑 M2**（Sprint N+2 末）：`gates.go` 提取
- 完成后 Sprint 29 未完成的提取工作（`GatesGreen`、`ResolveGate`、`HarnessRunner` 已移出，但 `gatherSignals`、`reportConvergence`、`reviewStatus` 仍在 `gates.go` 中）
- 将信号收集/报告逻辑移至 `internal/signal` 或 `internal/converge`（与 `internal/converge` 保持一致）
- `gates.go` 大小从 493 行下降至 < 300 行

**里程碑 M3**（Sprint N+3 末）：`prompt_context.go` 提取
- 观察者模式回调 → `internal/observe`（新包）
- prompt 组装逻辑 → `internal/prompt`（新包）
- `prompt_context.go` 大小从 454 行下降至 < 250 行
- **总体效果**：`cmd/forge` 生产文件数从 26→22，平均文件大小从 481→~350

**风险与缓解**：
| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| 提取过程中测试被破坏 | 中 | 中等 | 每次提取步骤后运行 `forge accept`；先移动测试再生产代码 |
| 提取导致 package 间循环依赖 | 低 | 高 | 提取前建好的依赖图；Sprint 29 的 `internal/gate/resolve.go` 是先例 |
| 新创建的包被允许膨胀到与旧代码相同的尺寸 | 中 | 低 | 为新包设置 `package.max_files` × `package.avg_size` 预算作为收口条件 |
| 重构过程中合并冲突 | 中 | 低 | 按功能提取（每 sprint 一个主要提取），而非同时在多个提取上工作 |

#### 阶段 2：运维就绪（Sprint N+4 至 N+5）—— P1

**目标**：提供 `forge maintain` 命令，使运维管理成为用户可见的能力。

**里程碑 M4**（Sprint N+4 末）：`forge maintain` 命令骨架
- `internal/maintain` 包包含：GC 策略、配额计算、`doctor --fix` 集成
- CLI 子命令：`forge maintain gc`、`forge maintain quota`、`forge maintain doctor`
- `.forge/` 目录配额监控（警告阈值 + 硬限制）
- 所有代码走评审，保持 `forge accept` ACCEPTED

**里程碑 M5**（Sprint N+5 末）：RunID 一致性（方向三完整版）
- 将 RunID 作为 `trace`、`memory`、`persist` 和 `converge` 的全局标识符
- 确保跨存储的因果一致性（trace 条目引用生成它的 RunID、memory 片段引用其所基于的 trace）
- 回放/调试可按 RunID 过滤

**风险与缓解**：
| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| RunID 方案与现有 checkpoint 格式不兼容 | 中 | 高 | 向后兼容读取；新写入使用新格式；建立数据迁移路径 |
| `forge maintain` 在用户不知情时删除数据 | 中 | 中等 | 默认 dry-run（如 `forge migrate`）；添加 `--apply` 触发 |

#### 阶段 3：抽象提升（Sprint N+6 至 N+7）—— P1-P2

**目标**：引入信号总线抽象，使演进更安全。

**里程碑 M6**（Sprint N+6 末）：信号总线 v1
- `internal/signals` 包：`Registry` + `SignalKind` 枚举 + `Producer`/`Consumer` 接口
- 将现有的 8 个信号迁移到注册模式
- 添加信号注册的「自检」测试（所有声明的信号至少在迁移测试中被注册）
- 现有 `converge.Signals` 保持为兼容性层

**里程碑 M7**（Sprint N+7 末）：`internal/executor` 提取 + YAML 库评估
- `command_executor.go` 从 `internal/orchestrator` 提取到 `internal/executor`
- 评估 `gopkg.in/yaml.v3` 与手写解析器的技术权衡
- 如果决定切换，则用 50 行胶水替换 `internal/yaml2json` 的 264 行解析逻辑
- 更新 `go.mod` 以包含 yaml.v3（唯一外部依赖）

**风险与缓解**：
| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| YAML 库切换暴露了手写解析器未 cover 的 edge case | 低 | 中等 | 对 5 个真实 workflow 文件运行对比测试；准备一个维护周期 |
| 信号总线被过度工程化 | 中 | 低 | 使用 TDD 方式：先为现有信号编写测试，再实现总线 |
| `internal/executor` 提取创建了新循环 | 低 | 高 | 提取前用 `go mod graph` 和 `internal/orchestrator` 的 import 图验证循环条件 |

### 5.3 长期路线图对齐

上述三个阶段自然地映射到 ROADMAP.md 中 v2 的当前轨迹：

| 阶段 | 执行后 v2 进度 | 对应 |
|---|---|---|
| 阶段 1：架构基础 | v2「存量债务消解」— 代码库与架构文档对齐 | ROADMAP 「扩展五方向中未完全兑现的部分」 |
| 阶段 2：运维就绪 | v2「运维能力」— forge maintain + RunID | 输入文档方向三的增量扩散 |
| 阶段 3：抽象提升 | v2「稳定性基础设施」— 信号总线 + executor 隔离 | ROADMAP 中「学习闭环」的铺垫 |

v3（分布式 HA + Firecracker + LiteLLM + Web UI）虽然在此路线图中未直接处理，但从架构角度看，**阶段 1-3 创建了一个更容易迁移到分布式架构的代码库**：信号总线在 v3 中可以后挂 Temporal 的事件总线，`internal/executor` 在 v3 中可以后面挂远程 runner 池，`internal/maintain` 在 v3 中可以后面挂跨节点 GC。

### 5.4 输入文档特定点的处理建议

基于对代码库的全面验证，我对输入文档（质量评估）做出以下架构回应：

1. **关于方向一与代码证据的偏差**：严重偏差（`detect.go` 声称 338/实际 221，`migrate.go` 声称 114/实际 234）表明文档可能是在文档作者本地分支上编写的，而非针对当前 HEAD。对于架构师，这意味着**代码扫描必须基于 `git HEAD` 的真实文件系统状态，而非记忆中状态**。我建议在 `CURRENT_SPRINT.md` 中增加一条纪律：对所有代码长度声明增加 `<checked: YYYY-MM-DD>` 注解。

2. **关于方向三的 `forge run` trace 旋转错误**：这是正确的批评。实际代码确认 `openTracer` 是 `forge run` 和 `forge evolve` 共享的函数。但「共享不足」这一点（配额策略过于简单——单级 10MB 旋转，不分迭代、不分 workflow stage）是文档中问题的有效修正。架构回应：trace 配额应按 `workflow_id > iteration > phase` 层级分配，而非全局 10MB cap。

3. **关于差异化声明**：我验证了 `five-unbuilt-product-architectural-extensions.md` 方向二确实涵盖了 RunID 方案，`capillary-gaps.md` 方向五确实涵盖了 observeFor 的三级 fallback。文档对其差异化优势的评估确实存在高估。架构回应：建议在 `docs/` 中建立一个「增量覆盖记录」——一个小型 `ANALYSIS_COVERAGE.md` 文件，其中每个分析文档声明其相对于现有分析的增量贡献。

4. **关于优先级建议**：我同意文档对方向一（P1 ✓）、方向三（降为 P2，或拆分为 RunID+P2 + maintain+quota+P1）和方向四（升为 P1 v1 子集 ✓）的重新评估。与文档意见不一致的是方向二——我认为零值语义风险比文档的降级建议更值得关注，因为虽然 `seed()` 缓解了部分风险，但「静默零值 = 错误行为」的模式是 Go 的一个已知陷阱，在配置/策略代码中特别隐蔽。我建议方向二保持在 P1-P2，而非降至 P2。
