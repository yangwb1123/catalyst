现在我已深入理解项目，以下是我作为资深架构师给出的评审。

---

# 架构评审：ForgeOS forge-core 运行时

**评审人**：资深架构师  
**日期**：2026-07-12  
**范围**：forge-core Go 运行时（18 个内部包 + CLI `cmd/forge`）+ 外部 Harness 层（gate.mjs/check.py/acceptance.mjs）

---

## 1. 架构评估

### 1.1 当前架构的优势

**1.1.1 层级分离清晰，依赖单向**

`forge-core` 的包图严格遵守单向依赖：

```
cmd/forge (编排+CLI)
  ↓
internal/orchestrator (引擎: 代码无LLM)
  ↓
internal/{asset,gate,mode,prompt,converge,routing,memory,trace,persist,...}
```

没有循环依赖，这是通过 arch-check 强制执行的。该属性使单个包可以由新人独立研究，无需理解整个图。

**1.1.2 通过接口反向控制实现合理的执行器抽象**

`Engine.Exec` 作为 `AgentExecutor` 接口（`DryRunExecutor`/`CommandExecutor`），`Engine.RunGate` 作为注入的 gate 运行函数，`Engine.OnPhase`/`OnIteration`/`OnBeforeIteration` 作为生命周期回调——所有这些都遵循**依赖注入**模式。引擎本身不导入 `exec`、`os` 或任何 LLM SDK。这使得引擎可以：

- 进行单元测试，无需真实进程或 API 密钥
- 通过 `CommandExecutor` 扩展到真实的 agent CLI
- 通过 `DryRunExecutor` 保持安全默认值

这已通过 Sprint 24-26 的真实 Claude 执行验证——八个真实 gap 被找出、修复，引擎代码不变。

**1.1.3 诚实（Honesty）作为架构原则**

诚实是一种已执行的架构属性，而非虚无缥缈的说法：

- **无伪造 N/A**：缺失工具的闸门报告 `N/A`，而非假装通过
- **已知缺口的诚实标注**：`ROADMAP.md` 和 `README.md` 系统性地标注了哪些功能是实际的，哪些是方位性/路线图上的
- **收敛是计算得出的，而非声明的**：`converge.Converge` 根据实际情况评估停止条件，而非轮数
- **YAML 解析通过临时 shim 完成**：`yaml2json.py` 被诚实地称为临时方案，不声称原生 Go 支持
- **mode gating fail-safe**：未知输入→全开，绝不静默放松

**1.1.4 四维资源安全已内置**

在四次 sprint 中（Sprint 20-23），运行时获得了完整的资源围栏：

| 维度 | 实现 | 类型 |
|---|---|---|
| 深度 | `FORGE_AGENT_DEPTH` + `MaxDepth` | 递归 fork-bomb 防护 |
| 数量 | `MaxAgentCalls` | 每次运行的 agent 调用硬上限 |
| 时间 | `context.Context` + `CommandExecutor` 超时 | 执行墙钟上限 |
| 内存 | `cappedBuffer` | 子进程输出缓冲区上限 |

这些是**成对**的：递归防护 + 预算防护；超时 + 输出上限。此处没有单点故障。

### 1.2 当前架构的限制

**1.2.1 跨进程状态完全无防护**

这是分析文档中已指出的最严重问题。`memory.jsonl` 是一个全局的、追加写入的文件，位于项目级 `.forge/` 目录下。没有进程级文件锁，没有 `run_id` 隔离。两个并发运行的 `forge evolve` 进程会相互泄漏发现：

> 进程 A 的 `costly_gap` → 加载到进程 B 的 prompt 中 → 进程 B 花费数千美元处理不在其范围内的 gap

`sync.Map` 缓存（`memory.go:39-42`）的设计注释说明了该问题——但文件层没有修复方案。`persist.Checkpoint` 也存在类似的问题，因为它是单文件快照，每次写入都会覆盖，但不会跨进程隔离。

**1.2.2 并行执行是已部署但从未调用的代码**

`parallel.go` 实现了完整的 wave 感知并行执行器，带有：
- 依赖图分析（`waves.go`）
- 按 wave 上下文取消（fail-fast）
- 并发安全状态收集（8 个有锁的 ledger）
- LOCK ORDER 契约文档

但它在运行时**完全未被使用**，因为没有任何工作流 YAML 声明 `depends_on`。这意味着：
- 锁层级有序契约未经实战检验
- wave 调度器未覆盖真实工作负载
- 多线程竞态条件（race condition）仅由 `-race` 测试覆盖，而非生产压力

这是一项可观的技术债务——大量的复杂性，潜在的竞态条件，且无实际收益。

**1.2.3 错误传播虽然正确，但用户不透明**

gate 失败的确切语义很清晰——第一个非 OK 的 gate 中止。但 `orchestrator.go` 中的错误传播模式依赖于 Go 的 `error` 类型。特别是当 `CommandExecutor` 包装有意义的退出码（KindRecursionLimit、KindTimeout、KindFailed）时，用户看到的只是类似这样的内容：

```
agent phase "implementer" failed: exec: "claude": exit status 1
```

exit-1 丢失了语义：是模型路由错误？预算超限？输出截断？`ExecError` 类型携带了原因，但该信息未呈现给用户、写入 trace 或通过 scorecard 上传。这是**可观察性差距**——诊断 agent 失败需要阅读 `trace.jsonl`。

**1.2.4 通过 shell 转码进行 YAML 解析易碎**

`yaml2json.py` 是一个 Python shim，它：

- 在 `exec.Command("python3", "harness/yaml2json.py")` 下 shell 出去
- 引入 Python 运行时依赖（在 `go.mod` 中未声明）
- 为运行时增加了进程生成开销（每次 `forge run` 调用一次）
- 在工作流 YAML 出现解析错误时，在 Go/Python 边界处产生不透明的错误消息

虽然被诚实地称为临时方案，但这是一个吞吐量瓶颈和监控空白——Go 原生 YAML 库将消除这种进程生成，并提供更好的错误处理。

### 1.3 关键设计决策评估

**决策 1：纯标准库，零外部依赖** ✅  

这是正确的“启动期”决策。它消除了可传递的依赖风险、构建时供应链攻击和 `go.sum` 漂移。但这是一个有意的**限制**：没有 YAML 库（临时 shim），没有结构化日志记录（`log.Println` + `fmt.Sprintf`），没有分布式锁（`sync.Mutex` + 文件系统），没有 HTTP 框架（`net/http`）。每个限制都有对应的成本。在某个时刻——可能是在引入 `Temporal` 或消息传递时——该策略需要放宽。决策应该是**什么时候**引入依赖，而不是**是否**引入。

**决策 2：带外执法作为真实来源** ✅  

与依赖宿主（Claude Code 的 `PostToolUse` hook）相比，将真实来源置于 CI runner / Sandbox 中是正确的。这确保了：
- 执法与宿主无关——没有宿主知道如何绕过
- 加速器是可选的——如果宿主没有 hook 能力，仅靠 harness 就能强制执行
- 适应性强——只需编写新的适配器，就能将新宿主引入同样的标准

**决策 3：中枢旋钮（mode × lifecycle）作为三点驱动的单一设置** ✅  

这是架构中最高杠杆的设计决策。单一设置同时驱动三处：**Router 档位 · Harness 严格度 · Workflow 深度**。这意味着：
- 用户理解一个概念（“我是在探索还是工程化？”）
- 语义在各子系统间一致——没有偏宽松的配置
- 迁移变得简单——`forge migrate` 是一个明确定义的状态转换

Sprint 15 完成 work 后，三处驱动全部就位。

**决策 4：Go 作为编排运行时，而非智能层** ✅  

Go 运行时（`forge-core`）有意不做**智能决策**——它不调用 LLM，不编写代码，不制定架构。它编排、调度、强制执行门控并跟踪状态。智能层在 Python（`forge-ai`，路线图中的未来）。这种正交性意味着：
- Go 层保持可测试性，无需 LLM 成本
- 智能层可以独立发展
- 可以利用 Go 的并发模型处理多个 agent
- 单元测试不需要 LLM 凭证

### 1.4 架构债务（技术债）

| 债项 | 位置 | 影响 | 偿还条件 |
|---|---|---|---|
| 零并行采用 | `parallel.go` (150+ 行) | 已部署但未调用的代码路径，锁层级 | 第一个 `depends_on` workflow |
| Python YAML shim | `yaml2json.py` | 进程生成 + 运行时依赖 | Go YAML 库已评估 |
| `main.go` 接近 500 行 | `cmd/forge/main.go` | 违反单一职责原则 | 按子命令拆分（`evolve.go` 已完成，`main.go` 仍需提取） |
| `orchestrator.go` 接近 500 行 | `internal/orchestrator/orchestrator.go` | 800+ 行的包注释表明责任过多 | 将 RunFrom/RunParallel/runPhaseParallel 拆分为单独的文件 |
| 可观察性差距 | `ExecError` → 用户界面 | 诊断 agent 失败困难 | 结构化的 trace 事件聚合 |
| 无跨进程隔离 | `memory.jsonl`, `persist/checkpoint.json` | 静默知识污染 | `run_id` + 文件锁 |

---

## 2. 扩展方向

### 方向 F1：跨进程状态隔离（合并原始方向③ + 部分方向②）

**为什么需要**：这是代码库中最昂贵的静默错误。两个并发的 `forge evolve` 进程（同一仓库上的 CI 作业 + 开发者手动运行）会通过共享的 `memory.jsonl` 相互泄漏发现。修复成本极低（加 `run_id` 字段 + Load 时过滤）。

**业务价值**：防止因将无关 Gap 纳入 prompt 上下文而导致的 LLM 预算静默燃烧。对于 run_id 可能为 0 的初期用户，向后兼容。

**技术难点**：
- 现有 `memory.jsonl` 无 `run_id` 字段 → 需要迁移策略或兼容 mode
- 跨平台文件锁（`flock` 在 Linux 上，`LockFileEx` 在 Windows 上）
- `persist.Checkpoint` 需要类似的隔离逻辑

**架构变更**：
```
internal/memory/
  memory.go → +ReadIsolated(filter func(Entry) bool) / +WriteWithContext(runID, entry)
  filelock.go (新) → 适配 OS 的文件锁抽象
internal/persist/
  checkpoint.go → +RunID 后缀或目录
```

**对现有系统的影响**：隔离到 `internal/memory/memory.go` + `internal/persist/persist.go`。现有 `Append`/`Load` 函数用 `run_id=""` 保持向后兼容。cmd 注入 `run_id`。

### 方向 F2：阶段 emits 后置条件执行

**为什么需要**：目前没有工件验证。agent phase 会写入文件，但无人检查输出是否存在、非空且有效。当 agent 应当写入 `docs/adr/ADR-001.md` 但静默失败（token 耗尽、上下文冲突、模型拒绝）时，所有下游 phase 都会收到空白输入。

**业务价值**：为每个 phase 的输出建立信任基线。`emits:` 声明在 workflow YAML 中被解析，但函数体内未被消费——允许声明后不兑现。

**技术难点**：
- 需要区分 overwrite mode vs append mode（独占 vs 共享文件）
- 对大型目录（如 `src/` 包含多个文件的 phase）的模式匹配
- agent phase 运行中产生文件，但 gate phase 之前可验证——顺序正确吗？是的，gate 在 agent 之后。

**架构变更**：
```
internal/orchestrator/
  verify.go (新) → 针对 emits 声明的后置条件检查器
  orchestrator.go → 在 runAgentPhase 返回后调用 verifyEmits

internal/asset/
  asset.go → Phase.Emits 添加 Mode 字段 (append|overwrite)
```

**对现有系统的影响**：纯增量。执行在 agent 成功之后（无 agent → 无验证）。未声明 emits 的 workflow 不受影响。

### 方向 F3：计算阶段池化和资源感知调度

**为什么需要**：当前的 wave 调度器（`waves.go`）仅基于依赖关系创建 wave。它没有考虑到：
- agent phase 会生成整个进程（`CommandExecutor`）
- 某些 phase 需要大量内存（大型 prompt 上下文、大型 repo 分析）
- 「宽」workflow（4+ 个并行 phase）会同时耗尽系统资源

**业务价值**：防止并行 execution 在高并发场景下导致的 OOM。允许 workflow 设计者表达资源约束（“此 phase 需要 8GB RAM”），且运行时自动节流。

**技术难点**：
- 可跨平台运行的可用内存检测（`unix` sysctl vs `windows` 全局内存状态 API）
- 为 phase 分配精确资源的上限估算
- 与现有 wave 调度程序集成，不破坏当前行为

**架构变更**：
```
internal/orchestrator/
  pool.go (新) → 资源感知的执行器（semaphore-guarded 并发度）
  waves.go → 波构建阶段合并资源约束

internal/asset/
  asset.go → Phase.Resources（可选结构体，带 CPU/mem/disk 字段）
```

**对现有系统的影响**：纯增量。`MaxConcurrency` 默认为 `len(wave)`（当前行为）。只有显式设置资源约束的 workflow 才会被节流。建议先搁置（P3），直到真实的资源压力报告出现，或发现第一个宽 `depends_on` workflow。

### 方向 F4：workflow 静态验证系统（合并原始方向⑤ + 方向②的模式方面）

**为什么需要**：目前，workflow 验证仅限于组成层面的检查（`check.py` 检查）和模式-gating 一致性。workflow 无法表达跨阶段约束（“如果 phase A 设置了 `on_fail: loop_back`，则 phase B 不能声明 `mode: explorer`”）或资源不变量（“此 workflow 要求 memory.jsonl 已初始化”）。

**业务价值**：在运行时之前捕捉编排错误。将“先写后跑”的开发体验升级为“先验证后写”，与 TypeScript 的 `tsc` 做类型检查而非在运行时发现错误的方式相同。

**技术难点**：
- 需要描述跨 phase 约束的 schema 语言（YAML 之上的一层）
- 可达性分析（“gate phase loop-back 到 `reviewer`，但 reviewer 被 mode skip”）需要遍历状态图
- 向后兼容性：现有 workflow 在验证器放松时不应报错

**架构变更**：
```
internal/validation/
  workflow.go (新) → 静态约束检查器
  reachability.go (新) → phase 图的可达性 + 检查 visited

internal/asset/
  schema.go → 扩展 workflow 模式以包含跨 phase 约束
```

**对现有系统的影响**：纯增量。当前 5 个 workflow 要通过验证，无需修改。新约束以可选的 workflow 级 `invariants` 块形式添加。

### 方向 F5：trace 驱动的可观察性管线

**为什么需要**：目前，trace 事件（`internal/trace/trace.go`）会写入一个平面 `trace.jsonl` 文件。没有聚合、没有可视化、没有跨进程关联。Sprint 26 的 Telemetry 已证明通过 `Observe` hook 收集延迟/成本/数值的可行性——但数据流向是单向的（文件 → scorecard → 人读）。缺少的是：

- trace 事件的流式传输（`tail -f` + 紧凑 UI）
- 跨进程 trace 关联（`run_id` 作为 trace_id 传播）
- 用于自动 root-cause 分析的结构化错误轨迹

**业务价值**：使“调试 agent 行为”从读取原始 JSON 行转变为使用结构化仪表板。降低故障平均解决时间（MTTR），当生产 workflow 在无人值守的 CI 上静默失败时。

**技术难点**：
- 需要增量后端设计（append-only → 可查询索引）
- `trace.jsonl` 格式的向后兼容性
- 开销——trace emission 绝不能阻塞 phase 执行

**架构变更**：
```
internal/trace/
  trace.go → +索引层（按 run_id 分段）
  sink.go (新) → 抽象输出目标（本地文件 / websocket / 轮询 HTTP）
  aggregator.go (新) → 按窗口聚合（与 scorecard 集成）

internal/orchestrator/runner.go (新) → 通用的 phase lifecycle 编排体+可观察性 hook
```

**对现有系统的影响**：纯增量。现有 `trace.jsonl` 继续写入。新的接收器需要显式配置。

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则 1：引擎保持无 LLM**

`internal/orchestrator` 中的 `Engine` 结构体必须永远不导入 Claude SDK、OpenAI 客户端或任何模型——这是架构基石。`Engine.Exec` 作为 `AgentExecutor` 接口，`Engine.RunGate` 作为函数值——两者都通过主函数注入。这意味着：

- 引擎可以在不烧钱的情况下进行单元测试
- 可以添加新的执行器（例如 `OllamaExecutor`、`OpenAIExecutor`）而无需接触引擎
- 引擎保持对执行时延和失败模式的职责明确

**原则 2：运行时库与 CLI 分离**

`cmd/forge` 是 CLI 入口点。所有共享逻辑都在 `internal/` 包中。`internal/orchestrator.LoopEngine` 不应导入 `flag` 或 `os.Args`。生命周期钩子（`OnIteration`、`OnPhase`、`OnGateResult`）是连接引擎与 CLI 代码的正确接口——它们允许引擎保持通用性，同时 CLI 处理检查点 I/O、trace 写入和日志记录。

**原则 3：诚实作为接口契约**

每个公共函数的值要么在计算（计算停止条件、解析分数），要么在明确委托（`RunGate`、`Exec`）。不存在“可能真实也可能不真实”的状态。特别是：

- `converge.Evaluate` 对未知指标报告 `NOT_MET`——从不说谎
- `gate.Result` 有明确的 `OK`/`FAIL`，通过 `N/A`（缺失工具）与 `ERROR`（工具崩溃）区分
- `memory.Load` 缺失文件时返回 `(nil, nil)`——从未声称有数据

### 3.2 新抽象层的引入

**当前缺失：Phase 生命周期包装器**

当前，phase 执行逻辑分布在 `orchestrator.go`（`runPhase`、`runAgentPhase`、`runGatePhase`）和 `loop.go`（迭代循环）中。随着并行执行、emits 验证和可观察性的增加，该生命周期将变得复杂。建议：

```
internal/orchestrator/
  runner.go (新) → PhaseRunner 结构体，封装通用生命周期：
                   1. 模式门控检查
                   2. 预算检查
                   3. 执行（串行或并行，通过 Executor）
                   4. emits 后置条件验证
                   5. trace 事件发射（通过 Observable hook）
                   6. 结果聚合
```

`Runner` 将被 `RunFrom` 和 `RunParallel` 共同使用，消除两个代码路径之间的重复。

**当前缺失：跨进程文件锁定**

`memory.go` 和 `persist.go` 需要一个平台感知的文件锁定层。Go 标准库不提供 `flock`——需要 `golang.org/x/sys/unix` 或低级 syscall 包。或者，可以创建一个 `internal/filelock` 包，封装：

```go
type Lock struct { ... }
func LockFile(path string) (*Lock, error)  // 非阻塞获取
func (l *Lock) Unlock() error
```

Windows 通过 `LockFileEx` 实现，Linux 通过 `flock` 实现。这应该保持简单——没有分布式锁定，没有租赁，只有进程级互斥。

### 3.3 向后兼容性

**已建立契约**：
1. 零值 `ModePolicy` = 无过滤（所有 gate，无 phase skip）
2. 零值 `MaxLoopBack` = 无 loop-back（红色 gate 立即中止）
3. 零值 `MaxAgentCalls` = 无限（从不因预算计数拒绝）
4. 零值 `MaxDepth` = 0 不阻断合法调用
5. `memory.Load` 在缺失文件时返回 `(nil, nil)`

**新组件指南**：

每个添加到现有结构体的新字段必须是：
- 可选（零值表示“无操作”或“默认行为”）
- 单一下降（不是结构体切片，使 JSON 向后兼容性复杂化）
- 接口实现（不是更改现有接口）

对于新的抽象层（`Runner`、`filelock`、`verifier`），必须通过构造函数注入，绝不能全局初始化。

---

## 4. 技术选型

### 4.1 新依赖引入标准

在核心运行时中，每个新依赖必须满足以下所有条件：

| 标准 | 原因 |
|---|---|
| 构建时非必需（通过构建标签可选） | 主分支保持零依赖 |
| 必须解决至少一个 P1+ 需求 | 没有“以防万一”的依赖 |
| CI 中必须可缓存 | 构建不应因上游停机而中断 |
| 许可证必须兼容 | MIT/BSD/Apache 2.0；无 SSPL/AGPL |
| 必须由活跃的维护者维护 | 非由已归档组织维护的单人项目 |

**当前候选**：
- `gopkg.in/yaml.v3`：消除 Python shim。满足所有标准。推荐在 P1+ 需求出现时采用。
- `golang.org/x/sys/unix`：用于 `flock`。已经隐式依赖（通过 `os/exec` 间接使用）。标准扩展，符合标准。

### 4.2 自建 vs 采用

| 组件 | 决策 | 理由 |
|---|---|---|
| YAML 解析 | **采用第三方库** | 规格稳定；YAML 库是标准基础设施。自建 YAML 解析器是重复造轮子。 |
| 文件锁定 | **自建**（~30 行） | 包装 2 个 syscall；无需第三方库的复杂性。 |
| 分布式编排 | **采购**（Temporal） | 分布式执行复杂；Temporal 作为路线图中的 v3。不要过早自建。 |
| LLM 路由 | **自建** | 模型路由是竞争优势；特定于 ForgeOS 的评分维度。没有第三方库能理解“mode × lifecycle × risk”。 |
| 评估引擎 | **自建** | `converge.Evaluate` 特定于 workflow 停止条件。没有现成的库支持基于 roadmap 完成度的收敛。 |
| agent-CLI 集成 | **自建** | 每个 CLI（Claude Code、Codex、Gemini CLI）都有不同的 SDLC。薄适配器层对于跨宿主支持是必要的。 |

### 4.3 技术栈演进

```
v2 现状（纯 Go 标准库）：
  forge-core/           ← Go stdlib 仅
  harness/*.mjs         ← Node.js（gate.mjs, acceptance.mjs, check.mjs 自测）
  harness/*.py          ← Python（yaml2json.py, check.py）

v2 中期（+ 最小依赖）：
  forge-core/           ← Go stdlib + gopkg.in/yaml.v3 + golang.org/x/sys
  harness/*.mjs         ← 同 v2
  harness/*.py          ← 仅 check.py（迁移到 YAML 后删除 yaml2json.py）

v3 目标（完全 polyglot）：
  forge-core/           ← Go（编排 + 调度 + 路由）
  forge-ai/             ← Python（发现 + 设计 + 审查 智能层）
  forge-runtime/        ← Rust（Sandbox：Firecracker 编排）
  forge-web/            ← TS/Next（Web UI + 仪表板）
```

**建议**：v2 中期（+YAML + sys）是下个 sprint 的正确目标。不要在 v3 之前急于引入 Rust/Python 层。核心运行时需要达到架构稳定性，然后才能可靠的智能层在其上构建。

---

## 5. 实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 方向名称 | 理由 |
|---|---|---|---|
| **P0** | F1 | 跨进程状态隔离 | 最昂贵静默错误；修复成本最低 |
| **P1** | F2 | 阶段 emits 后置条件 | 信任基线；每个 phase 两次 `os.Stat` 即可 |
| **P2** | F4 | workflow 静态验证 | 工作流数量超过 10+ 时自动升级 |
| **P2** | F5 | trace 驱动的可观察性 | 操作效率；无紧急安全影响 |
| **P3** | F3 | 资源感知调度 | wave 天然窄；先等真实用户撞墙 |

### 5.2 阶段和里程碑

#### 阶段 1：跨进程基础（Sprint N）

**目标**：消除静默知识污染

```
M1.1  memory.go：+FileLock 类型
M1.2  memory.go：Append/Load 获取锁
M1.3  memory.go：可选的 run_id 过滤（新函数 LoadRun(id)）
M1.4  persist.go：检查点获取锁（move → write → release）
M1.5  cmd/forge：forge run 生成 run_id，在 memory 和 persistence 中传播
M1.6  测试：并发的 evolve 进程使用共享 memory.jsonl，隔离通过
```

**风险**：文件锁定在 NFS 或 FUSE 文件系统上可能为死锁或静默无操作。缓解措施：锁定失败退化为无操作，且诚实记录（“file locking unavailable on this filesystem”）。

#### 阶段 2：工件验证（Sprint N+1）

**目标**：确保每个 phase 产出其声明的 `emits:`

```
M2.1  internal/asset：Phase.EmitMode（append|overwrite）
M2.2  internal/orchestrator/verify.go：后置条件检查器
M2.3  orchestrator.go：在 runAgentPhase 后插入 verifyEmits
M2.4  支持 emits 模式的 workflow YAML 更新
M2.5  测试：产生文件 → PASS；不产生文件 → FAIL
```

**风险**：大型目录的模式匹配（`src/**/*.ts`）增加了复杂性。缓解措施：从精确文件路径开始，稍后添加 glob。

#### 阶段 3：静态验证 + 可观察性（Sprint N+2 到 N+3）

```
M3.1  internal/validation/workflow.go：跨 phase 约束检查器
M3.2  internal/validation/reachability.go：phase 图可达性
M3.3  check.py：集成 workflow 验证
M3.4  internal/trace/sink.go：可配置输出目标
M3.5  internal/trace/aggregator.go：窗口汇总
```

**风险**：可达性分析增长的状态空间（n 个 phase 的指数级路径）。缓解措施：限制检查到最大深度为 phase 数量的两倍——失败意味着无法静态分析。

#### 阶段 4：资源调度（Sprint N+4，触发后）

**目标**：当第一个宽 `depends_on` workflow 真实触发时解锁

```
M4.1  internal/orchestrator/pool.go：受信号量保护的并发
M4.2  internal/asset：Phase.Resources（可选）
M4.3  waves.go：在波构建时合并资源约束
M4.4  测试：可用内存 > 0，但受信号量节流的宽波
```

**风险**：跨平台宿主内存检测不精确。缓解措施：使用不精确但安全的近似值（1/4 的系统内存作为默认值）。允许用户通过 worklow 覆盖。

### 5.3 风险及缓解措施

| 风险 | 可能性 | 影响 | 缓解措施 |
|---|---|---|---|
| 文件锁定在 NFS/FUSE 上效果不佳 | 中等 | 高（静默降级） | Lock fail → 记录“不可用” + 不获取锁继续运行 |
| 宽并行 wave 真实触发 OOM | 低 | 严重（进程被杀） | 在第一个宽 `depends_on` workflow 出现前保持 P3 |
| Python 用户缺少 `python3` | 低 | 中等（不能 run） | 阶段 2 中迁移到 Go YAML 库 |
| 审计 gap 被快速上报（LLM 幻觉） | 中等 | 中等（紧急缺口） | 保持 checkpoint 基础设施——可以回溯应用修复 |
| 新接口需要打破向后兼容性 | 低 | 中等（用户工作流中断） | 默认零值规则确保现有 workflow 不变 |

### 5.4 一句话总结

**跨进程隔离（P0）修复成本最低且防止最昂贵的错误；emits 验证（P1）建立信任基线；workflow 验证（P2）和可观察性（P2）以正确的顺序解锁操作规模；资源调度（P3）等待真实世界信号。架构核心理由是正确的——Go 引擎保持无 LLM 和可测试性，且诚实原则贯穿所有级别。**
