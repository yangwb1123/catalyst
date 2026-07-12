已通读项目架构文档(ARCHITECTURE.md / BOOTSTRAP.md / ROADMAP.md / AGENTS.md / DECISIONS.md / north-star.md)和验证反馈文档。以下是我的架构分析报告。

---

# 架构分析报告：ForgeOS `forge-core` 代码库

## 1. 架构评估

### 1.1 当前架构的优势

**A. 精准的渐进式演进策略**
项目的 v0→v1→v2→v3 路线图是近期所见最自律的增量交付案例之一。它不是"先造一个 K8s 再跑"，而是：
- **v0**：只做 Context + Harness（止血层，无运行时）
- **v1**：Claude Code 原生编排 + 声明式 agent 卡 + 治理闸门
- **v2**（当前）：Go 运行时脚手架就位（13→17 包），零外部依赖
- **v3**（目标）：分布式高可用架构（Temporal / Firecracker / LiteLLM / 跨厂商池）

每一版可独立验证，**不写进未来才能证明现在的假设**。这是架构债务管理最高级的形式——增量范围与学习同步。

**B. 带外执法载重墙（Harness）**
`harness/gate.mjs` + `arch-check.mjs` + `check.py` + `secret-scan` 构成一套 **host-independent 的治理层**。这解决了 AI 编码代理最经典的"幽灵合规"问题——代理在 prompt 中被教导规则，但可以（且会）绕过。带外闸门是不可逃避的事实源。

再细分三层执法（来自验证反馈的启示）：
1. **Edit-time 加速器**（CC PostToolUse hook）：快速失败，非阻断
2. **Stop 闸门**（`forge accept`）：聚合 8 检查，阻断性
3. **CI**（`.github/workflows/forge.yml`）：跨 session 的最终防线

**C. 中枢旋钮（mode × lifecycle）**
一个设置同时驱动三处（Router 档位 / Harness 严格度 / Workflow 深度），是典型的**策略即数据**（Policy as Data）实践。`production` lifecycle 一票否决的设计保证了安全底线不会被松散 mode 绕过——这正是 north-star 中 OPA 式 PDP/PEP 分离的 v0 表现形式。

**D. 文件/函数级结构预算**
500 行 / 50 行 / 15 根目录文件这些红线本身不是独创，但**对 AI 生成代码施加硬性结构预算**是一个被低估的架构选择。AI 生成的代码天然倾向于单块放大（"上帝文件"），这些阈值充当了**抗熵屏障**。

验证反馈中 cmd/forge 包反复触达 16 文件 / 499 行的边界并触发拆分的模式，证明该预算在真实工作负载下有效。

### 1.2 当前架构的局限性与债务

**A. `cmd/forge` 的"假 CLI 层"问题（原分析方向②，验证后成立）**
这是最严重的**架构债务**。16 个文件、12,513 行中，`cost.go`(471行)、`prompt_context.go`(454行)、`gates.go`(493行) 承载了大量本应属于 `internal/*` 的纯业务逻辑。

从架构角度看，根本问题不是文件行数本身，而是：

- **分层被文件边界掩盖**：`arch-check` 只检查"包间"依赖，不检查"包内文件间"的职责混合。`cmd/forge` 是一个包，但内部存在多层职责混装。
- **包名承诺了接口但未兑现**：`cmd/forge` 的名字暗示它只是 CLI 胶水，实际承载了：编排生命周期（`evolve.go`/`engine_build.go`）、prompt 装配（`prompt_context.go`/`prompt_memory.go`/`prompt_artifacts.go`）、预算追踪（`cost.go`）、收敛判定（`gates.go`）、校验（`validate.go`）等多种职责。

**好消息**：项目已有一套成功的拆分模式（`internal/doctor`、`internal/attribution`、`internal/gate/resolve.go`），问题不在于"能否拆"，而在于**需要持续当心不要新加逻辑进 CLI 层**。

**B. YAML 解析器三重碎片化（原分析方向①，完全成立）**
三条路径（`internal/yamlpath` → Python shim → `internal/yaml2json` Go 手写解析器）加上差分测试 `TestToJSON_MatchesPythonShim` 使用 `t.Errorf` 而非 `t.Logf`（验证者纠正——这实际上强化了论点），是**有意识的临时方案但久未治理**的典型案例。

从架构角度的三个层次问题：

1. **运行时碎片**：Go 原生解析器（`yaml2json.Decode`）不支持 anchor/alias/tag，但 Python shim 支持。这意味着同一 workflow 文件在两条路径下可能产生不同 JSON。
2. **Python 硬依赖**：`internal/yamlpath` 的 `Resolve` 函数硬编码 `exec.Command("python3", ...)`，让一条本应是纯 Go 的路径绑定了运行时 Python 依赖。
3. **测试死角**：差分测试只覆盖 7 个已知 workflow 文件，不覆盖所有 YAML 消费路径。Block scalar 损坏（Sprint 27 发现）7/7 真文件全部跑偏却被 `t.Logf` 静默掩盖，直到 fresh reviewer 发现。

**综合判断**：这是**P1 级架构债务**。不是"需要更多测试"的问题，而是三个解析器之间的语义不一致本身就是静默数据损坏源。

**C. 持久化的非原子性漏洞（原分析方向④，验证者称"最干净的新方向"）**
跨进程文件冲突的完全正确性已被验证证实（零 `flock/LOCK_EX` 匹配）。但架构层面更深的关切是：

- **`persist.SaveCheckpoint` 使用 `os.WriteFile` 后已改为原子重命名**（验证补充确认），这是一个已修的具体漏洞。
- **更广泛的模式问题**：`.forge/` 目录下多种状态文件（trace.jsonl / checkpoint.json / memory files / stage approval markers）各自独立持久化，**没有事务边界**。写 checkpoint 成功但写 trace 失败（或反之）导致跨文件状态不一致。
- **无目录级存储预算**：缺 `du -sh .forge/` 自动预警。

验证者提议重写方向③的叙事是正确的——"trace.jsonl 无限增长 + 缺总存储预算 + memory compaction 存在但仅 evolve 触发"比原分析更准确。

**D. 所有 `internal/` 包不可外部集成（原分析方向⑤，验证者确认为设计决策，建议 P2）**
当前 `go.mod` 是 `module forgeos/forge-core` 无 `go.mod` 版本。要把任何包提升到 `pkg/` 需要：
1. API 稳定性承诺
2. `go.mod` 版本依赖路径
3. 处理 `cmd/forge` 引用 11+ 个 `internal/` 包的耦合

验证者给出的"可能 >3 sprints"判断是合理的。

### 1.3 关键设计决策合理性评估

| 决策 | 合理性 | 评价 |
|---|---|---|
| 零外部依赖（纯 Go stdlib） | ✅ 完全合理 | v2 阶段，一个编排运行时不需要外部依赖来验证核心假设 |
| 带外执法而非依赖 agent 诚实 | ✅ 完全合理 | AI 代理不可信是架构假设，不是 bug |
| Python shim 做 YAML 转码 | ⚠️ 临时合理 | 脚手架，但已演变为半永久状态，需要 P1 关闭 |
| `cmd/forge` 做单体 CLI | ⚠️ 历史合理 | 演进而来，随包增长阈值自动拆分，但有持续性债务 |
| mode×lifecycle 三驱旋钮 | ✅ 高度合理 | 最优雅的设计之一，单个配置驱动 Router/Harness/Workflow |
| Fresh-context reviewer 独立审 | ✅ 完全合理 | 单一职责在流程层面的体现 |
| `go.mod` 无版本 | ⚠️ 合理但不可持续 | v2 可接受，v3 前必须解决 |

---

## 2. 扩展方向

以下 5 个方向基于验证反馈的 5 个方向，但融入了架构层面的评价和深化。

### 方向 A：YAML 解析统一 —— 关闭三重碎片化（P1，杠杆：⭐⭐⭐⭐⭐）

**为什么需要**：三个解析器产生不同输出是一个静默数据损坏源。Go 原生解析器的 block scalar bug（Sprint 27）让所有 workflow 文件的 description/note 字段携带 `> ` 前缀直达 agent prompt——已产生真实影响。同时 Python 运行时依赖阻碍了纯 Go 静态二进制的交付目标。

**核心挑战**：
1. **anchor/alias 支持**：Go 标准库无 YAML 库。要么引入外部依赖（违反当前零外部依赖纪律），要么加深手写解析器支持 anchor/alias（工程量大）。
2. **语义一致性**：需要和 PyYAML（事实上的参考实现）逐语义等价。
3. **迁移策略**：不能"切掉"Python 路径而不破坏现有的 `internal/yamlpath` 消费者。

**预期的架构变更**：
- 选项 A1（推荐）：将 `internal/yaml2json` 提升为 `internal/yaml`，覆盖全部 YAML 消费需求，合入 `internal/yamlpath` 的功能，关闭 Python shim 路径。
- 选项 A2（低保真）：保留三条路径，但将差分测试从"7 个已知文件"扩展为**属性式测试**（property-based testing）覆盖全部 YAML 语义边界（anchor/alias/tag/block scalars/multi-doc）。
- 选项 A3（长期）：引入一个外部 YAML v3 库（如 `goccy/go-yaml`），接受代价。

**对现有系统的影响**：
- `harness/yaml2json.py` 可从运行时依赖降级为"仅用于测试交叉验证"（甚至删除）
- `internal/yamlpath` 的 Python 硬依赖需重写
- `preflight.go:112` 的 "python3 missing" 警告可移除

**我的推荐**：在当前 Sprint 直接走选项 A2（属性式测试）+ 修复 block scalar + 明确决定 Python shim 的去留。走 A3 需要 architect/CTO 会议讨论依赖决策。

---

### 方向 B：`cmd/forge` 职责分离与包重构（P1，杠杆：⭐⭐⭐⭐⭐）

**为什么需要**：12,513 行的 CLI 层混杂编排/提示装配/预算跟踪/收敛判定等职责，与分层架构原则矛盾。它让我们无法独立测试编排逻辑而不拉起整个 CLI；也无法让编排逻辑被另一个入口（如未来的 gRPC 服务）复用。

**核心挑战**：
1. **包文件数上限**（当前 16/17）提供持续拆分压力，但只解决了"横向切文件"，不解决"纵向切职责"。需要让拆分方向是**按领域包**而非按文件数。
2. **`cmd/forge` 依赖图谱**：11+ 个 `internal/` 包被引用，拆分时需要理清哪些依赖是真实的，哪些是仅通过 `cmd/forge` 的"中转层"间接依赖。
3. **`prompt_context.go`** 是最大的单一职责问题：管理 4 种 ledger + 互斥锁 + prompt 装配。这与 `internal/prompt` 的界限模糊。

**预期的架构变更**：
- **Phase 1**（快速获胜）：`cost.go` 中的预算追踪逻辑（`parseClaudeCostUsd`、`classifyClaudeOverload`、`costEmitter`）移入已有 `internal/orchestrator/budget.go` + 新 `internal/cost` 包。
- **Phase 2**：`prompt_context.go` 的 4 种 ledger（verdict/phaseOutput/gate/cost）抽象成 `internal/ledger` 包。
- **Phase 3**：`gates.go` 的收敛判定逻辑（`GatesGreen`/`ResolveGate`）已完成迁入 `internal/gate/resolve.go`，需持续防止回渗。
- **长期**：`cmd/forge` 只留 CLI 路由 + flag 解析 + 主控流程胶水，目标规模 <5000 行。

**对现有系统的影响**：
- 每个提取步骤都是一个安全的"移动文件不改变行为"的操作（当前多次拆分已证明可行）
- 需要更新 `.arch/rules.yaml` 的 `package.max_files` 预算
- 零消费者破坏——`cmd/forge` 的包内函数在同一 binary 中，包外消费者通过已导出接口

---

### 方向 C：持久化层统一与存储治理（P2 → P1-for-production，杠杆：⭐⭐⭐）

**为什么需要**：跨进程无锁、trace.jsonl 无限增长、checkpoint 与 memory/trace 之间无事务边界——这些在单进程开发场景下可接受，但 CI/CD 场景或 `forge evolve` 长期运行场景下会产生真实的数据损坏风险。

**核心挑战**：
1. **跨进程锁定**：不引入外部依赖（如 PostgreSQL）的前提下，基于文件系统的跨进程锁（`flock`）是唯一选项，但需要确保文件锁在进程崩溃时的清理。
2. **`trace.jsonl` 轮换**：`Compact` 只压缩 `memory` 不压缩 `trace`。需要决定轮换策略（按大小/按时间/按迭代数）和保留策略。
3. **事务边界**：checkpoint + trace + memory 三者的写入顺序和原子性保证。

**预期的架构变更**：
- 新 `internal/storage` 包（或 `internal/persist` 的扩展）：提供目录级存储配额检测、trace 轮换、统一写前日志（WAL）或至少写入顺序契约。
- 跨进程锁层：`flock` 封装 + 超时 + 崩溃清理。
- 存储预警接入 `--max-storage-bytes` flag（四维资源护栏的第五维）。

**对现有系统的影响**：
- `internal/persist` 需要扩展（而不是重写）
- `internal/trace` 需要添加轮换触发机制
- `cmd/forge/evolve.go` 的 `compactMemoryIfDue` 需要扩展为 `storageMaintenanceIfDue`
- 向后兼容：无锁的单进程使用不变，仅多进程场景下自动获取锁

**我的推荐**：P2 处理存储治理（trace 轮换 + 总配额检测），P1-for-CI/CD 处理跨进程锁。不应在 v2 引入 WAL——那是 v3 分布式架构时 Temporal 做的事情。

---

### 方向 D：`pkg/` 可编程 API 的渐进式准备（P2，杠杆：⭐⭐）

**为什么需要**：当前所有 `internal/` 包不可外部导入，使 forge-core 作为库/框架的集成场景完全不可能。验证反馈提到"等社区需求触发"是务实的判断，但可以从现在开始**为这个方向做准备而不承诺交付**。

**核心挑战**（验证者已概述）：
1. API 稳定性承诺等级
2. `go.mod` 版本依赖路径
3. `cmd/forge` 与 11+ `internal/` 包的耦合解绑

**预期的架构变更**：
- **不创建 `pkg/` 目录**——不做逆向工程。
- **做**：对每个 `internal/` 包，确保其公共接口是**合理的、有注释的**。当前大量 `internal/` 包的工具人函数缺乏文档注释（`godoc`）。
- **做**：将 `internal/` 包间的依赖图中**真正需要跨包共享的接口**记录下来（ADR）。
- **不做**：任何接口稳定性承诺。

**对现有系统的影响**：接近零。这是准备性工作，不是交付性工作。

**我的推荐**：当前 Sprint 末尾用 1 个 skill（`document-interfaces`）为所有 `internal/` 导出符号加 godoc 注释。这不是 API 设计，但为未来的 API 设计保留可追溯性。

---

### 方向 E：`trace.jsonl` 格式升级为结构化事件日志（P3，杠杆：⭐⭐⭐）

**为什么需要**（这是验证反馈中未展开但值得关注的方向）：当前 `trace.jsonl` 是 append-only JSON 行，被 `--resume` 完整重读。随着运行时间增长：
- 重读时间线性增长（`forge run --resume` 需要扫描全部历史 trace）
- 无法按阶段/agent/时间范围过滤重放
- 数据结构没有 schema 版本控制

**核心挑战**：
1. 设计一个向前兼容的事件模型（可以支持未来字段而不破坏旧消费者）
2. `--resume` 的效率：需要索引（按 iteration/phase）而非全扫描
3. 不应引入外部存储

**预期的架构变更**：
- `internal/trace` 扩展：事件 schema 版本 + 索引文件 + 选择性重放
- 或采用已有格式的最佳实践（如 NDJSON + sidecar index）

**对现有系统的影响**：
- 写入路径变成写事件 + 更新索引
- 读取路径变成按索引跳跃而非全扫描
- 需要迁移器（`forge migrate trace --to-v2`）

**我的推荐**：P3。在 `trace.jsonl` 达到性能瓶颈前不做。当前 Sprint 31 的 checkpoint/resume 已经足够好。

---

## 3. 接口设计建议

### 3.1 关键模块接口原则

**3.1.1 包间接口的 "honesty-first" 模式**
项目已有很好的模式——`internal/converge.Signals` 的诚实性注释（"无数据 omit 不伪造 0"）。应正式化为接口设计原则：

```
每个接口的返回值必须诚实反映其计算能力：
  - 有数据 → 返回数据
  - 无数据 → 明确标记"无数据"，不返回零值/默认值
  - 不确定 → 明确标记置信度
```

这在 AI-native 系统中至关重要——因为消费者（LLM agent）无法区分"数据为零"和"无数据被编码为零"。

**3.1.2 回调而非继承**
项目已正确选择了回调模式（`Engine.OnIteration`/`OnGateResult`/`Observe` hook）。应保持并扩展：

```
当前模式 ✅:
  engine.OnGateResult → cost.go callback → trace event

避免的模式 ❌:
  class CostEngine(Orchestrator): ...  # 继承导致紧耦合
```

**3.1.3 在包边界定义错误类型，而非传递字符串**
当前做法不一致：有些包导出 `ErrKind` 常量（如 `internal/trace`），有些包传递字符串错误。应统一。

### 3.2 需要引入的抽象层

**A. 存储接口层**（`internal/storage`）

这是当前最缺的抽象。当前：
- `internal/persist` 负责 checkpoint
- `internal/trace` 负责 event 日志
- `internal/memory` 负责记忆存储
- 三者各自调用 `os.WriteFile`/`os.OpenFile`/`os.Rename`

引入统一存储接口：

```go
type Store interface {
    Save(ctx context.Context, kind string, data []byte) error  // 原子写 + 自动索引
    Load(ctx context.Context, kind string) ([]byte, error)
    List(ctx context.Context, kind string) ([]string, error) 
    EstimateUsage(ctx context.Context) (bytes, limit uint64, err error)
}
```

这不是为了引入新功能，而是让**三个存储消费者共享同一个"存储治理"适配器**——配额追踪、跨进程锁、原子性保证各一份实现。

**B. 命令总线 / 事件总线**（v3 准备）

当前 `cmd/forge` 中的所有操作都是直接函数调用。随着引擎独立化，需要一个薄事件总线：
- 这不是 Temporal（v3）
- 这是一个进程内 pub/sub，用于解耦"事件产生者"（orchestrator）和"事件消费者"（cost tracker / trace writer / memory）
- 当前 `OnGateResult` 式的回调模式已经是一个雏形，可以正式化为 `EventBus` 接口

**C. 锁接口**（跨进程安全）

```go
type FileLock interface {
    Lock(ctx context.Context) error
    Unlock() error
}
```

这是从方向④（跨进程文件冲突）提取的最小化抽象。不应引入大而全的分布式锁框架。

### 3.3 向后兼容性保持

- **采用兼容的扩展字段模式**：`Checkpoint` 有 `FormatVersion` 字段，`trace` event 有 schema——这是正确模式。坚持下去。
- **`--resume` 兼容**：旧 trace 格式仍可读，新格式加 version header。
- **CLI flag 弃用策略**：如需改名 flag，保留原名作为 alias + 打出警告，一个版本后再移除。当前 `forge run/evolve` 的 flag 集已有 15+ 参数，需避免 flag 名冲突。

---

## 4. 技术选型

### 4.1 当前零外部依赖是正确的，但需要明确退出条件

`go.mod` 无 `require` 是 **v2 阶段的正确选择**。它验证了核心编排假设而不需要处理依赖管理。但这个约束会变成枷锁，需要明确退出条件：

| 条件 | 触发时机 | 建议动作 |
|---|---|---|
| YAML 解析器需要 anchor/alias | P1（当前） | 允许一个 YAML v3 库（如 `goccy/go-yaml`） |
| Schema 验证（workflow YAML） | P2 | JSON Schema 库（纯 Go 实现） |
| 分布式协调 | v3 | Temporal（采购）+ NATS（采购） |
| 多厂商 AI 路由 | v3 | LiteLLM（采购网关） |
| 沙箱隔离 | v3 | Firecracker（采购隔离引擎） |

**决策框架**：
1. 这个库是否解决一个**在零外部依赖架构下无法准确解决**的问题？
2. 这个库是否是**领域标准**（如 PyYAML for YAML）而非临时流行的库？
3. 依赖引入的**增量**（vendor 大小、构建时间、漏洞面）是否在可接受范围内？

### 4.2 自建 vs 采购的边界

项目已有清晰的边界（north-star.md 的服务目录表已标记）。需要强调的是：

**核心差异化——自研**：
- 编排逻辑（`internal/orchestrator`）
- 治理模型（`harness/` 全系列）
- 路由决策 + 记分卡（`internal/routing`）
- 角色/上下文体系（`internal/prompt`）
- 评估/收敛引擎（`internal/converge`）

**通用基础设施——采购**：
- YAML 解析（标准库或社区库，非自研手写解析器）
- 工作流引擎（v3: Temporal）
- 沙箱（v3: Firecracker）
- 模型路由网关（v3: LiteLLM）
- 认证（现有: OIDC 集成）
- 可观测性（OTel 标准）

**当前自研过度的地方**：
- `internal/yaml2json`（手写 Go YAML 解析器）——**明确的自研过度**。Go 社区有多个成熟的 YAML v3 库。当前解析器不做 anchor/alias，block scalar 曾损坏，但对项目自身的 7 个 YAML 文件有效。可接受作为临时方案，但应评估替换成本。

### 4.3 不需要引入的技术栈

验证反馈中隐式暗示的几个方向，我建议**暂不引入**：

- **嵌入式数据库（bbolt/etcd）**：当前文件系统持久化足够，引入数据库增加复杂度而无收益。v3 分布式架构过渡到外部存储（Postgres/NATS）时不经过嵌入式数据库阶段。
- **依赖注入框架（wire/uber-fx）**：当前包依赖图是静态可推导的，A 包 import B 包是明确的。DI 容器在包数 <30 时是做加法而非做减法。
- **代码生成器**：当前 Go 代码的接口数量有限（每个包 1-3 个导出类型），手动维护足够。代码生成引入了构建依赖和认知负载。

---

## 5. 实施路线图

### 5.1 优先级排序

| 方向 | 优先级 | 时间 | 杠杆 | 依赖 |
|---|---|---|---|---|
| **A. YAML 统一** | P1 | 当前 Sprint | ⭐⭐⭐⭐⭐ | 依赖决策（外部库 vs 加深自研 vs 保留分叉） |
| **B. cmd/forge 拆分** | P1 | 当前 Sprint（与 A 并行） | ⭐⭐⭐⭐⭐ | 无外部依赖 |
| **C1. trace 轮换 + 配额检测** | P2 | 下个 Sprint | ⭐⭐⭐ | 无 |
| **C2. 跨进程锁** | P1-for-CI/CD | CI/CD 启用前 | ⭐⭐⭐ | 无 |
| **D. 接口文档化** | P2 | 低负载 sprint | ⭐⭐ | 无 |
| **E. trace 格式升级** | P3 | 性能瓶颈时 | ⭐⭐⭐ | 需要当前 trace 体积数据 |

### 5.2 阶段划分

**Phase 1 — 止血（1-2 sprints，YAML + CLI 分离）**

| 里程碑 | 产出 | 验证标准 |
|---|---|---|
| YAML 解析器碎片关闭 | 1 个解析路径（而非 3 个），block scalar + 属性测试通过 | 差分测试覆盖全部 YAML 语义边界；Python shim 不再是运行时依赖 |
| `cmd/forge` 到 8000-9000 行 | cost/budget → `internal/cost`；ledger → `internal/ledger`；gates 逻辑已在 `internal/gate` | `cmd/forge` ≤ 9,000 行；`arch-check` 无新增违规 |
| 跨进程锁 | `internal/persist` 的 `SaveCheckpoint` 使用 `flock` | 两个并行 `forge run --resume` 通过（而非截断） |

**Phase 1.5 — 治理深化（1 sprint）**

| 里程碑 | 产出 | 验证标准 |
|---|---|---|
| trace 轮换 | `trace.jsonl` 达到阈值（如 50MB）时自动轮换，保留最近 N 个 | `forge evolve` 运行长时间后 `.forge/` 目录大小可控 |
| 存储配额检测 | `--max-storage-bytes` 默认 1GB | 超限时 `forge evolve` 提前 fail-closed |
| `internal/*` 文档化 | 全部导出符号有 godoc | `go doc ./...` 无 missing doc 警告 |

**Phase 2 — 可集成性准备（2-3 sprints，v3 前期）**

| 里程碑 | 产出 | 验证标准 |
|---|---|---|
| 包接口审计 | 确定哪些可以提升到 `pkg/` | ADR 文档 + architect 审阅 |
| 进程内事件总线 | Engine 事件通过 EventBus 分发，cost/trace/memory 为独立消费者 | 单测可 mock EventBus 验证事件传递 |
| `go.mod` 版本化 | `v0.x.y` 语义版本 | 按 semver 发布 |

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| **YAML 统一时引入外部库破坏零外部依赖纪律** | 高 | 中 | 允许有条件的例外——先做属性式测试覆盖现有缺陷，再做外部库决策 |
| **`cmd/forge` 拆分导致循环依赖** | 中 | 高 | 已三次证明"可拆分且不产生循环"（到 `internal/doctor`/`attribution`/`gate/resolve`），复用既有模式；**从已有消费者最少的函数开始拆** |
| **跨进程锁在 CI 环境不可用**（某些 CI runner 文件系统不支持 `flock`） | 中 | 中 | 回退方案：每进程写唯一标记文件，靠冲突检测（非锁机制） |
| **存储配额触发误断** | 低 | 中 | 默认阈值（1GB）对开发场景远大于实际需要；用户可通过 `--max-storage-bytes 0` 禁用 |
| **"先拆分再继续"与 P1 并行开发冲突** | 中 | 中 | 拆分为两个并行 agent 分别处理 YAML 和 CLI 分离，各自有自己的 struct 预算 |

### 5.4 明确不做的事项

与验证反馈一致，以下事项应明确标记为 "not now"：

- **将 `forge-core` 包提升到 `pkg/`**——等社区需求触发，不做提前设计
- **引入 Temporal/LiteLLM/Firecracker**——v3 范围
- **Web UI**——架构外（偏离 CLI/声明式核心）
- **Embedding 语义检索**——当前 TF-IDF 已工作，镀金

---

## 总结

ForgeOS 的 `forge-core` 代码库在架构自律方面远超同类项目：带外执法、中枢旋钮、结构预算、fresh-reviewer 纪律——这些都是**处理 AI 生成代码熵增的有效架构决策**。

五个分析方向中，验证反馈确认了四个方向的事实基础，修正了一个方向（存储累积）的叙事但保留了核心关切。作为架构师，我的最大建议不是"做更多"，而是**"关闭半开的门"**：

1️⃣ **YAML 三重碎片**是半开的门——三条路径都有道理，但合在一起产生了未知的语义差距。关闭它（统一解析器）是在一个 sprint 内可完成的最高杠杆动作。

2️⃣ **CLI 层膨胀**是半开的门——每次拆分都正确，但未形成"新代码放对包"的习惯。需要一个清晰的"职责地图"（哪个 `internal/*` 包管什么），而非每次触达文件数上限再拆。

3️⃣ **trace 无界增长**是半开的门——memory 已有 compaction，trace 没有。在 memory compaction 的基础上对称加 trace 轮换是一个 P2 动作，但优先级不应该低于"让 evolve 跑过夜"。

4️⃣ **跨进程无锁**是半开的门——非 CI/CD 场景无害，但 CI/CD 是项目的明确目标（`.github/workflows/forge.yml` 已存在）。在 CI/CD 全面启用前处理这个漏洞比之后补救代价低一个数量级。

这扇门意象概括了当前 forge-core 的架构状态：**每个方向都已经写到够好以至于能正常工作，但尚未写到够好以至于可以忘记**。下个 Sprint 的目标应该是把这四扇门中的前两扇（YAML + CLI 职责）关紧。
