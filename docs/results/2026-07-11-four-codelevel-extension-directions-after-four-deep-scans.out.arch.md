现在我有了完整的上下文。让我提供架构分析。

---

# 架构分析：ForgeOS 四个扩展方向

## 1. 架构评估

### 优势

ForgeOS v2 的 Go 运行时在架构纪律上表现出色，在同类系统中很少见：

- **零外部依赖**：纯 Go 标准库是刻意的架构选择。`go.mod` 无 `require` 行意味着零供应链攻击面、即时构建、无版本冲突。这是「载重墙」原则（带外执法器，host-independent）在 Go 包管理层面的贯彻。

- **显式 honesty 边界**：代码库在每个抽象层都标记了哪些能力是真实的、哪些是预估的，哪些是日志占位符。`asset.go` 里的「容错解析」注释、`openTracer` 里的 `// best-effort; ignore error`、`RunParallel` 的文件头 —— 都源自同一个原则：运行时绝不在观测性上撒谎。

- **实体很少，接口精心设计**：18 个 `internal/` 包构成清晰的依赖 DAG。最底的包（`asset`、`persist`、`trace`）没有内部导入。包边界与引擎映射对应（`orchestrator`、`routing`、`prompt`、`memory`、`converge`）。

- **渐进式复杂度**：`RunFrom`（串行）和 `RunParallel`（并发）是独立的入口点，没有条件嵌套。`Waves()` 在有向无环图解析失败时以闭锁方式失败，在第一个有错误的阶段之后才会构建并行调度。

### 局限性

一个架构师在四个方向之外还应该看到：

**1. 观测性横切关注点重复**

现在有三套独立的写入模式：
- `trace.Tracer`：追加 JSON 行，10MB 后旋转，`trace.jsonl.1` 作为单备份
- `persist.Checkpoint`：原子写入 + fsync + rename，`rotateRetain(path, retain)` 保留 5 个版本
- `internal/memory`（推测）：与 trace 和 checkpoint 不同的另一个序列化路径

这三套各自实现自己的写入器、自己的旋转和自己的恢复逻辑。真正持久的业务数据（最可贵的是 trace 行）与其他数据相比处理方式更粗糙。这是架构债务，不是特性缺口。

**2. 资产加载的容错性与运行时校验之间的差距**

`asset.go` 将自身明确界定为 "not to re-litigate schema validity"，将严格性放在 `check.py`（Python）中。这是一个合理的分层决策，但它产生了一个架构问题：Python shim（`yaml2json.py`）既是转码器又是隐式门控 —— 如果它在 `asset.go` 期望 JSON 之前未运行，缺口就会不声不响地持续存在。yaml2json 中的 `consumeBlockScalar` 破坏（Sprint 27）被一个记录但永不断言的测试掩盖了，这一事实表明额外的一层增加了测试复杂性，而真正的 bug 正是在那里被掩盖的。

**3. 并行执行缺乏预算语义**

`RunParallel` 是 `RunFrom` 的镜像实现，但预算检查（方向三）被移植了，而不是重新设计。对于串行路径，一个 phase 消耗预算，然后要么失败（预算丢失）要么继续。对于并行路径，预算是在锁下预先检查的，但已废弃的 phase 已经消耗了预算并将其标记为已用。缺少的是：wave 级别的预算预留，以便在 wave 取消时，未使用 phase 的预留可以重新声明，而不是作为一个整体丢失。

**4. `LoopEngine` 迭代模型将收敛判定与每次迭代绑定**

当前循环结构 `runIteration → checkStop → nextStartPhase` 意味着收敛判定发生在所有 phase 完成之后。对于方向二（智能跳过），改变 `nextStartPhase` 以考虑每个判据的满足状态需要在收敛和相位选择之间建立反馈通道 —— 而当前并不存在这个通道。收敛判定是单一的，而不是按判据粒度的。

### 关键设计决策评估

| 决策 | 评价 | 风险 |
|---|---|---|
| `asset.go` 容错解析 + `check.py` 严格校验 | ✅ 正确的分层；但 yaml2json shim 是单点故障 | 中等：偏离在运行时无声无息地持续存在 |
| `openTracer` 单次快速旋转 | ⚠️ 方向一正确；应与 `rotateRetain` 对齐 | 低：修复很小，风险低 |
| `nextStartPhase` 只有向前跳跃和向后跳跃 | ⚠️ 缺少「跳过这个，它已经完成了」 | 中等：循环重新运行已满足的 phase 会浪费 LLM 成本 |
| `RunParallel` 按 phase 预算检查 | ❌ 不完整的语义：废弃的预算永远丢失 | 中等：随着并行使用量增长，问题会更加突出 |
| yaml2json Python shim（零依赖 Go） | ✅ 务实之举；但 shim 是测试盲点 | 高：Sprint 27 的 block-scalar 损坏因测试惰性而持续了多个 sprint |
| `persist` 原子性（write+fsync+rename） | ✅ 教科书式的正确性 | 无 |
| `func nextStartPhase` 带 `// unreachable` 的 `OnRejected` | ⚠️ 死代码是架构债务 | 低：文档诚实，但应该提取出来 |

---

## 2. 扩展方向

### 方向 A（高）：统一持久化层——消除观测性不对称

**为何需要**：trace（审计追踪）、checkpoint（恢复）和 memory（学习）各自实现自己的 IO、自己的旋转和自己的错误处理。这会复制 bug（方向一），使恢复场景复杂化，并且随着更多数据类型的增加也无法扩展。

**核心挑战**：
- `persist` 包当前拥有有状态检测点逻辑（编码/解码、版本化格式、原子 IO）。将其泛化为处理多种记录类型，同时保持零依赖约束。
- 不同类型的保留策略不同：trace=3 个版本，checkpoint=5，memory 可能=0（仅内存）——需要为每个流配置策略。

**预期的架构变更**：
```
目前：    persist/         trace/          memory/
         checkpoint.go    tracer.go       *.go (各自独立的持久化)

未来：    persist/
         ├── store.go        ← 通用存储接口：Write/Read/List/Rotate
         ├── rotate.go       ← rotateRetain 泛化（retain 可配置、多路径）
         ├── checkpoint.go   ← 基于通用 store 的检测点
         └── trace.go        ← 基于通用 store 的 trace 写入
```

**影响**：
- `openTracer` 从 10 行缩减到 ≈3 行（`store.New(path, retain=3)`）
- `trace` 包变成纯序列化（格式、emit），不再关心 IO
- 方向一自然解决，无需额外修复

**权衡**：一个「通用存储层」可能违反 YAGNI，如果 trace+checkpoint 在 6 个月内仍然是对称的唯一两个消费者。一个更轻量的替代方案：直接导出 `rotateRetain` 并在 trace 中调用它，这样只需修改 3 行代码，而无需新建抽象层。

---

### 方向 B（高）：按判据收敛——有状态的相位跳过

**为何需要**：当前 `runIteration → checkStop → nextStartPhase` 在一次收敛判定中评估所有判据。如果 `review_status=approved` 满足了收敛条件，但 `roadmap_completion=80%` 不满足，下一次迭代重新运行整个流程，包括已经达到 approved 的评审 phase。浪费大量 LLM 成本。

**核心挑战**：
- 收敛信号（`converge.Signals`）包含每个判据的值，但 `converge.Converge` 返回的是最简单的布尔值（met/not met）。要实现跳过，需要知道**哪些判据满足了，哪些没有**。
- 相位选择（`nextStartPhase`）需要一个从判据到相位的映射：哪些判据由哪些 phase 满足？
- 与 `--max-iter` 和 `--no-progress` tripwire 的交互：跳过 phase 会更高效，但应该计入 max-iter（以避免无限循环）还是不计（以反映真实进度）？

**预期的架构变更**：
```
目前：
  checkStop(sig) → bool met
  nextStartPhase(wf) → int startIdx      (仅考虑 on_unmet/on_rejected)

未来：
  checkStop(sig) → (bool met, []CriterionStatus)
  nextStartPhase(wf, criteriaStatus) → int startIdx
    （跳过所有判据已满足的 phase；只运行未满足判据的 phase）
```

**对现有系统的影响**：
- `converge.Converge` 返回签名变化（从 `bool` 变为 `<bool, []CriterionStatus>`）
- 相位跳过必须对历史可用——`ResumePrev` 需要存储每个判据的状态，而不仅仅是一个 `float64`
- 需要在每次迭代之间持久化判据状态（通过方向 A 的统一存储）

**权衡**：
- **最优化方法（推荐）**：只跳过 `ReviewStatus=approved` 的评审 phase（最容易衡量的收益；影响明确，仅限于一个 phase）
- **最通用方法**：通用判据→相位映射器，需要 YAML 中的新模式来声明哪个 phase 满足哪个判据
- **折中方法**：一个简单的启发式方法——如果某个判据在最后一次迭代中已满足，且没有依存 phase 需要重新运行，则跳过它

---

### 方向 C（中高）：并行预算预留——wave 级别的预算分配

**为何需要**：当前，并行预算按每个 phase 独立检查（`checkAgentBudget` 原子增加）。当一个 wave 被取消时（一个 phase 失败），wave 中已废弃的 phase 所消耗的预算会永久丢失——它们已经递增了计数器，但从未产生任何输出。这不会产生不正确的行为（预算消耗是正确的），但会导致不公平：一个 wave 中有 N 个 phase，第一个失败会使其他 N-1 个 phase 的预算计入已用，但没有任何价值。

**核心挑战**：
- 预留语义应在 wave 级别（不是 per-phase）：分配整个 wave 的预算，在成功时确认，在取消时释放
- 重新声明已取消 phase 的预算需要在 wave 完成时（而不是 phase 完成时）进行一次减量
- 锁的排序（`parallel.go` 文件头中已有的锁顺序契约）——释放逻辑必须尊重已有的嵌套顺序

**预期的架构变更**：
```
目前： 每个 phase：mu.Lock → checkAgentBudget(inc) → mu.Unlock

未来： 每个 wave：
         // 预留
         reserved := len(wave)
         mu.Lock → checkAgentBudget(reserved) → mu.Unlock
         
         // phase 完成时确认
         成功 → 无操作（预留已确认）
         失败/取消 → mu.Lock → releaseAgentBudget(1) → mu.Unlock
```

**影响**：
- `runWave` 获得 `reserveAgentBudget`/`releaseAgentBudget` 辅助方法
- `checkAgentBudget` 处理批量预留（1 个 phase 或 N 个 phase）
- 串行路径不受影响（它仍然按 phase 分配）
- lock-order 契约新增一个层级

**权衡**：
- 如果并行使用量很低（`--parallel` + `depends_on` 是可选功能），这个方向可以推迟
- 但方向 C 的代码变更非常小：每个 wave 大约增加 5-10 行。这是「先做好」与「等需要时再做」的比较
- 如果 wave 被取消但重新声明预算，循环的下一次迭代可能会因新 wave 请求而超额——需要一个全局「已用 vs 已预留」账户系统

---

### 方向 D（中高）：workflow 资产的运行时守卫——warn-only 校验层

**为何需要**：`asset.go` 的容错解析意味着 `feeds_foward:`（拼写错误）静默加载为 `FeedsForward=false`，`model_tier: ops`（本应是 opus）静默加载为 `model_tier=""`（无覆盖，使用 sonnet 而非预期的 opus）。这些是静默降级——只有在 agent 使用次优模型运行后才会被发现。

`check.py`（严格校验器）在提交时运行，但不会在编辑‑运行循环中运行。运行时守卫弥补了这一差距。

**核心挑战**：
- 守卫必须**只警告，不阻断**——asset.go 的容错性是设计使然；阻止带有未知字段的 workflow 会破坏运行时。守卫应记录「警告：phase X 有未知字段 Y」并继续运行。
- 需要校验：引用完整性（`depends_on`/`target_phase` 名称存在）、已知值（`model_tier` 在识别的集合中）、拼写错误的字段名（相似字段的 Levenshtein 距离）
- 守卫应与 `asset.Phase` 中的新字段（`RequiresTools`、`ConfidenceMetric`）一起演进

**预期的架构变更**：
```
asset/
├── asset.go          ← 容错解析（不变）
├── guard.go          ← 新增：运行时校验，warn-only
│   ├── CheckWorkflow(wf) []Warning
│   ├── checkPhaseRefs  (depends_on/target_phase 名称存在性)
│   ├── checkKnownValues(model_tier/action 在集合中)
│   └── checkSpelling  (对识别的字段做 Levenshtein 检查)
└── guard_test.go     ← 针对 fixture 测试所有警告场景
```

**调用点**：`cmd/forge/evolve.go` 在 `Run` 或 `RunFrom` 之前；`cmd/forge/main.go` 在 `runWorkflow` 中。日志到 `Log` 回调（已由 CLI 注入）。

**权衡**：
- 方向 D 的方向四是这份文档的重点。我同意它的评估：一个 focused 的 ~180 行实现，只检查引用完整性和已知值，避免重新实现 `check.py`
- 警惕范围蔓延：不要添加 `check.py` 已经处理的结构化校验（YAML 模式、gate 名称存在性）。维持明确的划分：`guard.go` = 运行时引用完整性 + 拼写检查，`check.py` = 完整的治理校验。

---

### 方向 E（中）：检查点格式版本化与向前兼容

**为何需要**：`persist.Checkpoint` 有一个 `FormatVersion` 字段（目前是 `"forgeos.checkpoint.v1"`），但没有任何代码消费它——加载时忽略它，因为没有迁移逻辑。随着 forge-core 增加了 `CriterionStatus`（方向 B）和新的信号，检查点格式将演进；如果没有版本的明确升级路径，旧的检测点会静默加载为损坏或丢失字段。

**核心挑战**：
- 检查点版本的语义版本化（Major.Minor）：Major 变更需要显式迁移，Minor 变更自动兼容
- 从一个版本到另一个版本的迁移函数（在解析之前对原始 JSON 字节进行操作）
- 降级路径：如果较新的 forge-core 遇到带有未知字段的较旧检查点，它应该忽略它们（已经在做了）或发出警告

**预期的架构变更**：
```
persist/
├── checkpoint.go     ← 当前：FormatVersion 已声明但未使用
├── migrate.go        ← 新增：基于版本的迁移（v1→v2、v2→v3）
└── migrate_test.go   ← 针对旧 fixture 测试降级
```

**调用点**：`Load` 在反序列化之前检查版本并调用 `Migrate(data, fromVersion, toVersion)`。

**权衡**：
- 方向 E 是纯债务消除——它不会立即解锁任何新功能。应与方向 B 绑定（当添加 `CriterionStatus` 时，检查点格式会发生变化）
- 如果方向 B 在接下来的 2 个 sprint 内没有排上，可以单独做方向 E（≈1 天工程量）

---

## 3. 接口设计建议

### 关键设计原则

```
1. 统一错误报告，不改变调用语义
   guard.go: func CheckWorkflow(wf Workflow) []Warning
   → 接收 Workflow，返回警告。不修改 workflow。不阻断执行。

2. 在消费点提取，不在生产点提取
   rotate.go: func RotateRetain(path string, retain int) error
   → openTracer 调用它，而不是自己内联 os.Rename。

3. 为变化设计收敛判定
   converge.go: type Status struct { Met bool; Details []CriterionStatus }
   → 不再返回 bool。将「满足了没有？」替换为「满足了什么？什么没满足？」
   → 这允许 phase 跳过（方向 B）基于判据粒度的信息。

4. 将预留与消耗解耦
   budget.go: func Reserve(count int) (id BudgetID, err error)
              func Release(id BudgetID)
              func Confirm(id BudgetID)
   → parallel.go 中的原子式递增被事务式预留/确认取代。
```

### 新的抽象层？

**有一个真正的候选者**：统一存储层（方向 A）。但请参见下面的权衡。

**不推荐**：一种通用的「规则引擎」来将判据映射到相位。`asset.yaml` 中的显式声明（这个 phase 满足这个判据）更加 Go 风格，并且更不易出错。

### 向后兼容性模式

| 变更 | 兼容策略 |
|---|---|
| `converge.Converge` 返回 `<bool, []CriterionStatus>` | 旧的调用者检查 `bool`；`CriterionStatus` 是附加的。**二进制兼容**。 |
| 检查点格式 v1→v2 | `migrate.go` 返回 `(v1→v2)` 的映射。**向前兼容**：v2 读取器可以读取 v1。 |
| `nextStartPhase` 接受新的 `[]CriterionStatus` 参数 | 零值（`nil`）表示「没有状态可用」→ 回退到仅 `on_unmet`。**源兼容**。 |
| 并行预算预留 | 新的辅助方法不影响现有的 `checkAgentBudget` 签名。**零行为变更**。 |

---

## 4. 技术选型

### 需要新依赖吗？

**不需要**。所有四个方向都可以用纯 Go 标准库实现。这是 forge-core 自己强制执行的工程红线（`go.mod` 无 `require`），不应放弃。

### 可能的技术模块结构

| 提议的包 | 基础 | 理由 |
|---|---|---|
| `internal/persist/store.go` | `os`、`io`、`encoding/json` | 统一写入/旋转/恢复 |
| `internal/asset/guard.go` | `strings`、`unicode`（用于 Levenshtein） | 运行时校验，零依赖 |
| `internal/converge/status.go` | 类型定义 | 纯类型，无需新依赖 |
| `internal/orchestrator/budget.go` | `sync` | 从 `orchestrator.go` 中提取 |

### 不采用外部依赖的评估标准

forge-core 的「零外部依赖」政策不是虚荣心——它是一个安全决策（无供应链攻击面）、一个运维决策（即时构建，无损坏的 `go.sum`）和一个架构决策（强迫设计保持简单）。在添加任何依赖之前，应当：

1. **能否用 ≤50 行标准库实现？** 如果可以，就不需要依赖。
2. **依赖是否已经由 forge-core 之外的另一个组件使用？** 如果是，考虑将 forge-core 抽象为接口而不是导入。
3. **依赖解决了 forge-core 核心关注点的问题吗？** yaml2json 的 Python shim 是一个已知的临时方案——当用 Go YAML 库替换时，它应该是唯一获得依赖豁免的东西（并且只针对 parsing，不针对业务逻辑）。

### 自建 vs 采购

本仓库没有采购二进制依赖的路径。所有内容都是自建的。这不是一个权衡——这是约束条件下的设计。四个方向在标准库内部都是可实现的。

---

## 5. 实施路线图

### 优先级

| 优先级 | 方向 | 工作量估计 | 风险 | 解锁条件 |
|---|---|---|---|---|
| **P0** | 方向一：Trace 链式轮转 | ~50 行，1 个文件 | 无 | 无 |
| **P0** | 方向四：运行时守卫 | ~180 行，2 个新文件 | 低（warn-only） | 无 |
| **P1** | 方向二：收敛跳过 | ~300 行，3-4 个文件 | 中等（循环语义变更） | 方向一完成（统一存储） |
| **P1** | 方向 E：检查点版本化 | ~100 行，2 个新文件 | 低 | 方向二排上时 |
| **P2** | 方向三：并行预算预留 | ~50 行，1 个文件 | 低 | `--parallel` 使用量增长 |

### 阶段划分

**阶段 1（sprint 32-33）：快速消除不对称**

```
- [P0] 导出 persist.rotateRetain → 改名为 persist.RotateRetain
- [P0] 重构 openTracer 调用 persist.RotateRetain(tp, 3)
- [P0] guard.go：引用完整性 + 已知值 + 拼写检查
- [P0] guard 在 evolve.go + main.go 的 RunFrom/RunParallel 之前调用
```

阶段 1 的里程碑：「方向一 + 方向四已部署。trace 有 3 个备份。拼错的 workflow 字段在运行时会触发警告。」

**阶段 2（sprint 33-34）：为跳过做准备**

```
- [P1] converge.Converge 返回 `<bool, []CriterionStatus>`（旧调用者二进制兼容）
- [P1] nextStartPhase 接受 `[]CriterionStatus`
- [P1] 跳过 review_status=approved 的评审 phase（最简实现）
- [P1] 检查点版本化 + v1→v2 迁移
```

阶段 2 的里程碑：「方向二原型已部署。评审 phase 在批准后跳过。向后兼容性已验证。」

**阶段 3（sprint 34-35）：并行语义 + 统一存储**

```
- [P2] 并行预算预留（方向三）
- [方向 A 原型] 泛化 persist 以处理 trace 写入（将 trace 序列化从 IO 中提取出来）
- [可选] 泛化 guard：从 `asset` 包中提取，如果并行使用量证明是合理的
```

阶段 3 的里程碑：「所有四个方向都已部署。并行预算语义正确。trace 和检查点共享经过战斗测试的 IO 层。」

### 风险与缓解策略

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| 方向二的收敛跳过导致循环逻辑中的边界情况 | 中等 | 高（错误的跳过可能导致 LLM 看到陈旧状态） | 在 TestLoopSkip 中测试每个跳过场景，包括 edge case（0 phase 跳过、全部跳过、跳过后再恢复） |
| Guard 的拼写检查对非英语 phase 名称产生误报 | 低 | 低（warn-only） | 对已知编辑距离的字典保持宽松（Levenshtein ≥3 时不报警） |
| 并行预留引入死锁（锁顺序违规） | 低 | 高（调度依赖的 Heisenbug） | 遵循已记录的锁顺序契约（`parallel.go` 文件头）；在测试中启用 `-race`；在集成测试中始终使用 `-count=20` |
| yaml2json shim 对运行时守卫产生误报（Python shim 与 Go 的解析不同） | 中 | 中（warn-only；通过的事实比 Python 转码器更可靠） | Guard 应警告，但使用 Python 转码器作为真相来源进行检查（如果 guard 检测到问题而 yaml2json 通过了它，则向用户报告不一致） |
| 检查点迁移被遗忘（格式版本化了，但没有迁移代码） | 中 | 中（恢复静默失败） | 在 `Load` 中添加集成测试，该测试保留一个带有版本 `v1` 的 fixture 检查点并验证 `v2` 读取器正确加载它。如果格式发生变化，更新 fixture。 |

---

## 总结

四个方向覆盖了一个合理的范围——从简单修复（trace 旋转）到核心循环语义变更（收敛跳过）。按此顺序实施，增量价值最高，风险最低。

文档引用的验证是彻底的。代码差异（文件路径、行号移位）被正确识别，且不改变任何结论。`rotateRetain` 的「作为函数导出 + `openTracer` 使用它」的修复建议是正确的 —— 这是重构，不是重写。

两处细微的架构观察，文档中未提及：

1. **`persist.Checkpoint` 已有版本字段但未消费** —— 方向 E 是最小化的架构债务，应在添加 `CriterionStatus`（检查点格式变更）之前解决。

2. **Python yaml2json shim 是测试盲点** —— `consumeBlockScalar` bug（Sprint 27）因测试断言惰性而未被发现。运行时守卫（方向四）弥补了这一差距，但只能作为补充；yaml2json 测试本身应该 assert 而非 `t.Logf`。这个问题已经在 Sprint 27 中修复了。

总体架构方向是健康的。代码库以有意为之的 craftship 水平运行，而这在没有中央「上帝」架构师、只有分布式的 agent 和 fresh-context 评审的情况下 —— 很难做到。
