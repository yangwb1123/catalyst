现在我有了充分的代码依据。以下是我的架构分析。

---

# 架构分析报告：ForgeOS 编排运行时

## 1. 架构评估

### 当前架构的优势

**严格的零依赖策略是最正确、最被低估的设计决策。** `forge-core` 作为 24h 自治循环的控制平面，在磁盘满、网络断连、宿主 CLI 崩溃的边缘场景中必须持续运行。一个 `go.mod` 里没有 `require` 的二进制——13 个纯 stdlib 包——意味着没有 transitive dependency 的漏洞评级、没有 yaml.v2 vs v3 的冲突、没有嵌入式 vector 数据库的 OOM。这在 AI 编排的故障域里不是奢侈，是必要条件。

**原子性契约在 persistence 层是完整且诚实的。** `checkpoint.go` 的 `writeSynced` + `os.Rename` 模式在单文件系统内是 POSIX 级别的原子提交。`memory.go` 的 JSONL 追加写是同一原则的不同表达——每一行是原子的，crash 最多丢一行，不损坏已有数据。两套策略分别对应"单点可覆盖的快照"和"只增不删的日志"，选型正确。

**Layering 和文件内职责分离非常成熟。** 以 `prompt_memory.go` 和 `prompt_context.go` 的拆分为例：前者拥有 `boundMemory`(内存检索/选择逻辑)，后者拥有 `buildPrompt`(prompt 组装)，两者共享 `truncateSummary` 和 `phaseOutputSummaryCap` 常量。这在 500 行约束下做了正确的取舍——两个文件各自聚焦且不重叠，合并后又需要通过跨文件常量保持同步。

`converge.Signals` 模式是架构的亮点。它是一个**带外质量信号总线(muscle channel)**，`LoopEngine` 消费 `Signals()` 函数指针，`converge.Converge` 评估 stop condition。这意味着新的收敛信号(artifact schema 验证、知识矛盾计数、silent drift 标志)可以作为新的 `Signals` 字段加入，不改变 loop 的主路径。

### 关键架构债务与技术债

**债务 1：parallel 模式的写冲突是 current architecture 的最大缺口，且与方向一描述的不完全一致。** 审阅文档说冲突检测应基于全工作树 `git diff`——但 `forge-core` 没有 git 依赖(纯零外部依赖)，读 `git diff` 需要 `os/exec`。这意味着冲突检测要么引入一个 `os/exec` 层的 git 适配器，要么重新设计为基于临时目录的 `diff -r`。这不是小细节——它触及了"零依赖承诺 vs 实用功能"的权衡。

**债务 2：memory 层的知识一致性近乎不存在。** `KindGap/Decision/Lesson` 三类条目通过 `Topic` 字段索引，`filterSuperseded` 提供显式撤回机制(Supersedes 字段)，但没有**隐式矛盾检测**。`memory_compact.go` 的 `Compact` 做法是按 age 分界、按 kind 保留最近 N 条——这是一种**遗忘策略，而非一致性策略**。被 compact 掉的知识不会产生矛盾信号，它只是沉默地消失了。

**债务 3：phase output 是弱类型的字符串通道。** `FeedsForward` bool 触发的 output 记录以 `truncateSummary` (800 runes 的纯文本)的形式流过 `phaseOutputLedger`。没有 schema 声明、没有结构校验、没有版本映射。一个 planner 阶段声称 "feeds_forward: true" 但输出格式对下游 implementer 不可解析——这没有任何检测机制。

**债务 4：clock 和 time 的硬编码阻碍了残留可靠测试。** `loop.go` 使用 `time.Now()` 和 `time.Since()`，`memory.go` 的 `Compact` 使用 `time.Now().Unix()`。没有 clock 注入接口意味着与 time 相关的行为(NoProgress tripwire、Compact age 分界、checkpoint 的 UpdatedAtUnix)在测试中要么不可控、要么需要欺骗墙钟。`backoff.go` 同样硬编码了 `time.Since`。

**债务 5：routing 的 `recency_half_life_days` 字段存在于 code comment 和 policy.yml 的声明数据中，但从未被消费。** `routing/scorecard.go:134` 明确注释 "intentionally NOT applied here"。代码声称它属于 Eval Engine——但 Eval Engine 在 v2 路线图中未落地(见 .agent/ARCHITECTURE.md: "Gateway · Agent-Runtime · Knowledge-Engine · Sandbox · Web-UI 仍为路线图")。这意味着 `policy.yml` 中的 `history.recency_half_life_days: 30` 和 `scorecard.go` 的数据模型是一个**已声明但未执行的控制承诺**——架构中有数据、有策略声明、但没有策略执行。

---

## 2. 扩展方向

我给出五个高价值方向，前三个与审阅文档有实质差异，后两个取其精华并做深化。

### 方向一：写冲突可观测性层（不是检测，是可观测性）

**为什么需要：** 审阅文档的方向一将其定义为"检测 + 机械裁决"——但如前所述，零依赖 Go 二进制没有 `git merge-file` 的能力，机械裁决在删/改冲突场景中必然出错。正确的第一性原理是：**LLM 节点的并行输出冲突不是 bug，是信号。** 可观测性应该先于检测。

**核心挑战：**
- 零依赖约束：不能引用 git2go、libgit2、或嵌入式 diff 库
- 需要从 `os/exec` git 获取工作树 diff，但这引入了对外部 `git` 二进制的强依赖
- 冲突是指数级复杂的：N 个并行 phase 修改 M 个文件，产生 O(N²·M) 的笛卡尔积

**预期变更：**
```
internal/worktree/          # 新包
  worktree.go              # 快照/差异点接口（封装 os/exec git）
  conflict.go              # 冲突数据模型（谁写了什么，何时写的）
  report.go                # 人类可读的冲突报告，注入 LoopEngine 的日志
```

**接口设计原则：**
```go
// 新的 Signal 字段
type ConflictSignal struct {
    PhaseConflicts []PhaseFileConflict  // 谁和谁冲突了
    FileChanges    []FileChange         // 全工作树变更清单
}

// converge.Signals 增加一个字段
type Signals struct {
    // ... 现有字段
    Conflict ConflictSignal  // 零值 = 无冲突（并行模式外/无 git 时不开启）
}
```

**对现有系统的影响：** 极低。新包 `internal/worktree/` 独立于现有代码。`Signals` 字段新增是向后兼容的(零值不做任何事)。并行模式下 LoopEngine 在 `runWave` 前后各 call 一次快照比较即可。

### 方向二：知识一致性边界层（Conflict of Memory, not of Files）

**为什么需要：** 审阅文档的方向二聚焦于"toxicity detection"(矛盾检测)，但我认为应更广义地定义为**知识一致性边界(Knowledge Consistency Boundary, KCB)**。不只是一个"检测矛盾的过滤器"，而是一套策略驱动的转捩系统。

**核心挑战：**
- **silent drift** 比矛盾更难检测：旧知识"gate takes 3 min" → 新知识"gate takes 3 sec"，两者在语义上不冲突，但在实际系统行为上冲突了
- 纯 Go 实现的关键词/否定词密度检测误报率极高(`but` 没出现不代表不矛盾)
- 需要区分**临时不一致**(迭代 i 的 gap 在 i+2 解决了，但知识库里两条记录并存)和**真正矛盾**

**架构变更：**
```
internal/consistency/       # 新包
  policy.go                # 一致性策略声明（什么算矛盾、什么算 drift）
  detector.go              # 检测器组合子：句法检测器 + BM25 检索检测器 + 数值突变检测器
  reconciler.go            # 调和策略：标记矛盾、打低 confidence、自动撤回
```

**关键设计决策——三个选项：**

| 选项 | 复杂度 | 误报率 | 对上下文窗口影响 |
|---|---|---|---|
| A. 纯关键词 + 数值突变检测 | 低(≈500 行 Go) | 高 | 低(只标记) |
| B. BM25 检索 + 否定密度 + 数值突变 | 中(≈1000 行) | 中 | 中(可能增加 entry) |
| C. 外部 embedding 服务 | 高(破零依赖) | 低 | 高(增加延迟) |

**推荐：** v1 走 B 路线——复用 `internal/prompt` 已有的 BM25 `Retrieve`，加一个 `mutationDetector`(检测同一 Topic 下关键数值的 >50% 变化)，加一个 `negationDensity`(否定词/对比连词的 TF 密度 > 0.3 标记为候选)。不需要 embedding。

**接口：**
```go
// 检测器是 converge.Signal 的一部分
type ConsistencySignal struct {
    Contradictions  []Contradiction  // 检测到的矛盾对
    SilentDrifts    []Drift          // 检测到的静默漂移
    Reclassified    []memory.Entry   // 被降级/标记的 entry（修改后返回）
}
```

### 方向三：可恢复执行路径（在审阅方向三的基础上深化）

**为什么需要：** 审阅文档的方向三(selective phase execution)复杂度评估从低提到中是合理的。但更重要的是另一个维度：**checkpoint 恢复的细粒度可信度**。

**核心挑战放大：**
- 当前 checkpoint 是 per-iteration 或 per-phase：`checkpoint.go` 的 `PhaseIndex` 字段支持 per-phase resume。但当 operator 用 `--from 3 --to 5` 跳过前序阶段，checkpoint 写入时的 `PhaseIndex=3` 但前序 `Iteration` 状态是空的——未来 resume 时无法区分"我故意跳过了 0-2"和"我 crash 了只有 phase 3 的状态"。
- 审阅文档提出的 `RunKind` 设计(NormalRun / DebugRun / FlightRecorder)是必需的补充，但还需要一个**缺失的第三态：`ResumableRun`**——可以从 checkpoint 恢复但必须验证 checkpoint 的完整性链条。

**更优设计——Checkpoint 锚链：**
```go
type Checkpoint struct {
    // ... 现有字段
    RunContext RunContext `json:"run_context"` // 新增：执行上下文描述
}

type RunContext struct {
    Kind         RunKind  // normal | debug | flight_recorder | resumable
    StartPhase   int      // 实际的起始 phase（0 = 从头开始）
    EndPhase     int      // 目标终止 phase（0 = 全部）
    MissingPhases []int   // 被跳过的 phase 索引清单
}
```

**对现有行为的影响：** `RunContext` 是 `omitempty` 的——旧 checkpoint 加载后为零值，被解读为 `normal` 模式从 phase 0 开始，100% 向后兼容。

### 方向四：Phase 输出契约（在审阅方向四上深化）

**审阅文档已正确诊断核心问题**——`FeedsForward` 的纯文本输出没有格式校验。我同意复用 `converge.Signal` 模式而非新管道的建议。

**但需要补充一个设计约束：** schema 注册表不应该在 `forge-core` 内部，而应该在 `.agent/schemas/` 目录下作为声明式资产存在。原因是：
- schema 变更需要 human review——合入 `.agent/schemas/` 目录的 PR 比合入 Go 代码的 PR 更容易审查
- Go 二进制不变更即可接受新的 schema 版本
- 多个项目(被 ForgeOS 治理的仓库)可以 fork/覆盖 schema

**架构模式——Schema as Data：**
```
.agent/schemas/
  prd.md/
    v1.json              # schema 注册表条目
    v2.json
  task-plan.md/
    v1.json
```

**接口：**
```go
// 新的 internal/schema 包
type SchemaRegistry struct {
    // 从 .agent/schemas/ 加载
}

type SchemaRef struct {
    Name    string // "prd.md"
    Version string // "v1"
}

func (sr *SchemaRegistry) Validate(ref SchemaRef, data io.Reader) (Valid, Warnings, error)
```

**converge.Signal 集成：**
`SchemaRegistry` 的输出进入 `converge.Signals` 的一个新字段，`LoopEngine` 在每次迭代后评估。

### 方向五：控制面故障注入框架（优先级提前——同意审阅文档）

**完全同意审阅文档将方向五提前到第三优先级**。24h 自治运行的 checkpoint 可信度是方向一和方向三的运行前提，没有它，选择性执行和并行写检测都建立在沙滩上。

**但需要更精确的范围定义：** 审阅文档的方向五聚焦于 checkpoint 的 `ENOSPC/EIO` 和 `rotateRetain` 的目录覆盖竞态。这是正确的，但范围太窄。一个完整的控制面故障注入框架应包括：

**三阶段注入策略：**
```
Phase A: Checkpoint & Memory (当前审阅范围的超集)
  - Save: ENOSPC(磁盘满)、EIO(设备错误)、O_CREATE 父目录不存在
  - rotateRetain: 目标 .N 是目录、权限不足、跨文件系统(EXDEV)
  - Load: 截断 JSON、无效 JSON、版本不兼容
  - Memory.Append: O_APPEND 写入失效、文件被外部删除
  
Phase B: Orchestrator Engine
  - Engine.RunFrom: phase 索引越界、空 workflow、nil Engine
  - LoopEngine: MaxIter=0、NoProgress 为负、OnIteration 回调 panic
  - Parallel.RunParallel: depends_on 的循环依赖、wave 空、wave 中所有 phase 失败
  
Phase C: External Dependency Failure
  - os/exec CommandExecutor: 命令超时、stdout 超 10MiB 上限、嵌套递归达到 MaxDepth
  - 宿主 CLI (claude) 崩溃、返回非零退出码
```

**设计原则——故障注入与生产代码零耦合：**
```go
// 唯一需要的 abstract 是 clock 注入
type Clock interface {
    Now() time.Time
    Since(t time.Time) time.Duration
}

// 现有的 persist.Checkpoint 不需要变更：
// 测试直接写损坏的 .tmp 文件、目录、权限受限的路径即可。
// 不需要 Checkpoint 感知它正在被注入故障。
```

---

## 3. 接口设计建议

### 核心原则
1. **现有接口不做 breaking change。** 所有新增都是向后兼容的 additive 模式——新字段 `omitempty`、新函数签名是 `type X struct { ... OldFields ... NewField OptionalType }`。
2. **内部包的纯/IO 边界。** encode/decode 必须是纯函数(无 IO 无全局状态)，IO 包装器接受 Clock、FS、Random 等接口注入。这是测试可预测性的投资回报率最高的模式。
3. **LoopEngine 的 Signal() 函数指针模式应推广到所有带外数据。** 当前 `Signals func() converge.Signals` 是注入性的——新 signal 源头(worktree diff、consistency check、schema validation)都通过这个管道进入收敛评估。

### 具体接口化建议

**1. Clock 接口（侵入最小化的抽象）**
```go
// internal/clock/clock.go
type Clock interface {
    Now() time.Time
    Since(t time.Time) time.Duration
}

type realClock struct{}
func (realClock) Now() time.Time          { return time.Now() }
func (realClock) Since(t time.Time) time.Duration { return time.Since(t) }

// 代替：time.Now() → clock.Now()，time.Since(t) → clock.Since(t)
// 测试：clocktest.Frozen(at time.Time) — 返回固定时间
```

**受影响的包：** `internal/orchestrator`(loop.go 的 NoProgress/staleCount)、`internal/persist`(checkpoint UpdatedAtUnix)、`internal/memory`(Compact 的 ageSeconds)、`cmd/forge`(prompt_memory 的 Duration 和 Checkpoint 注入)。

**2. RunContext（选择性执行 + checkpoint 完整性）**
```go
// internal/asset/run_context.go (新文件)
type RunKind string
const (
    RunNormal     RunKind = "normal"
    RunDebug      RunKind = "debug"
    RunFlightRec  RunKind = "flight_recorder"
)

type RunContext struct {
    Kind          RunKind
    StartPhase    int  // 0 = 从头开始
    EndPhase      int  // 0 = 全部
    MissingPhases []int // 跳过的 phase 索引
    ResumeValid   bool   // resume 时作完整性检查
}
```

**3. Schema Registry（phase 输出的类型契约）**
```go
// internal/schema/package.go
type Registry struct {
    schemas map[string]Schema // schema name → version chain
    mu      sync.RWMutex
}

type Schema struct {
    Name       string
    Versions   []VersionEntry
    Latest     int // 当前活跃版本索引
}

type VersionEntry struct {
    Version string // "v1", "v2"
    Validator func(io.Reader) error // 纯校验器
}

func (r *Registry) LoadDir(dir string) error   // 从 .agent/schemas/ 加载
func (r *Registry) Validate(name, version string, data io.Reader) error
```

---

## 4. 技术选型

### 核心约束：零外部依赖不可违反

`forge-core` 是控制平面——24h 运行、自治循环、比它所编排的 agent 更可靠。任何外部依赖(包括嵌入式数据库、YAML 解析器、http 客户端)都引入了不被 ForgeOS 控制的故障域：依赖宕了、license 变更了、transitive dep 有 CVE 了——控制平面必须无视这些继续工作。

**这是非协商选择。** 审阅文档中建议的"方向一的 git diff 依赖"必须通过 `os/exec` 调用外部 `git` 二进制来解决，而非引入 `go-git` 或 `libgit2`。这意味着冲突检测是**可选功能**——当 `git` 不在 PATH 中时优雅降级，而非阻塞。

### 需要评估的第三方集成

| 候选服务 | 推荐 | 理由 |
|---|---|---|
| LLM 供应商 SDK | 不引入，通过 `os/exec` CLI | 零依赖 + 可注入任意 CLI |
| YAML 解析 | 通过 python shim 转码 | 不进入 Go 二进制 |
| 嵌入式 vector 数据库 | v3 再评估 | v1 的 BM25 足够，且 embedding 引入的 OOM 风险不适合控制平面 |
| 分布式锁(etcd/Redis) | v3 再评估 | v2 前不涉及多副本调度 |

### 自建 vs 采购的决策界限

- **故障注入框架：自建。** 这是控制平面的核心韧性属性，外部框架的注入模式与 Go `*testing.T` 的集成通常不精确，且第三方框架引入依赖。
- **Schema validation：自建/半自建。** 核心 validator(JSON schema check)用 stdlib `encoding/json` 实现即可。复杂的跨 schema 引用验证可以委托给外部工具(可选)。
- **Embedding/语义检索：采购/集成。** v3 路线图中有专门的 Embedding 服务，forge-core 通过 gRPC 或 REST 调用即可(此时外部依赖风险可接受，因为 v3 架构假设了分布式)。

---

## 5. 实施路线图

### 优先级和阶段划分

我建议的优先级与审阅文档的推荐基本一致，但有实证依据调整：

**P0 — 必须在 P1 之前完成：**
- **⑤ 控制面故障注入(Phase A)** —— 因为 checkpoint 可信度是所有其他方向的运行前提
- **② 知识一致性边界(基础 BM25 版本)** —— 因为 24h 循环中知识一致性比并行能力更基础

**P1 — 核心扩展：**
- **③ 可恢复执行路径(RunContext + selective phase)**
- **⑤ 故障注入 Phase B (Orchestrator Engine)**

**P2 — 上层扩展：**
- **① 写冲突可观测性**
- **④ Phase 输出契约 + Schema Registry**
- **⑤ 故障注入 Phase C (External Dependency)**

### 阶段路线图

**阶段 1 — 可信度夯实(当前 sprint + 1)**
- Clock 接口化(内部改动，无新包)
- 故障注入 Phase A: checkpoint + memory 的 10+ 故障场景测试
- `rotateRetain` 的目录覆盖修复 + 测试
- 为 `persist.Checkpoint` 增加 `RunContext` 字段(但暂不消费)

**阶段 2 — 知识一致性(阶段 1 后)**
- 新建 `internal/consistency/` 包(依赖 `internal/prompt` 的 BM25, 不依赖外部)
- `negationDensity` 检测器 + `mutationDetector`
- 集成到 `converge.Signals` 的新字段
- 启动 silent drift 的日志级告警(不改变 loop 行为)

**阶段 3 — 可恢复执行(阶段 2 后)**
- 实现 `--from phaseName --to phaseName` CLI 参数
- `RunContext` 在 checkpoint 中的消费：完整性验证
- DebugRun 模式跳过 checkpoint 写入
- FlightRecorder 模式(只记录不覆盖)

**阶段 4 — 冲突可观测 + 契约(阶段 3 后)**
- `internal/worktree/` 包(可选 git 依赖，优雅降级)
- `.agent/schemas/` 示范 schema v1
- `internal/schema/` registry + validator
- Schema 验证结果作为 `converge.Signals` 字段

### 风险点与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| `internal/consistency` BM25 矛盾检测误报率 > 50% | 中 | Agent 被噪声干扰开始忽略一致性警告 | 初期只做 `observation-only`(观察模式)，不自动执行。只有 confidence > 0.7 的矛盾才注入 prompt |
| 选择性执行的 checkpoint 完整性检测在非预期场景误触发 | 低 | Resume 拒绝合法 checkpoint | 在 `RunContext.ResumeValid` 设为 false 时跳过完整性检查，recovery path 留手动覆盖 |
| Schema registry 的版本映射导致 schema 爆炸 | 低 | .agent/schemas/ 目录增长 | 每个 schema 版本保留不超过 3 个活跃版本：`latest`, `latest-1`, `deprecated`。`deprecated` 只验证不拒绝 |

### 与现有路线图的交叉映射

| 本报告方向 | .agent/ROADMAP.md / .agent/ARCHITECTURE.md 映射 |
|---|---|
| ⑤ 故障注入 Phase A | 直接对应 ARCHITECTURE.md §5.3 "checkpoint 崩溃恢复安全"的落地 |
| ② 知识一致性 | 对应 ROADMAP "Memory-Based Knowledge Consistency" 条目 |
| ③ 可恢复执行 | 对应扫描发现的 `--from/--to` gap，直接解决 operator 调试痛点 |
| ① 冲突可观测 | 对应 ROADMAP "Parallel Write Conflict Detection" 但范围缩至可观测(非自动裁决) |
| ④ Phase 契约 | 对应方向四的 arch 设计，但引入 `.agent/schemas/` 目录作为 data-plane 而非 code-plane |

---

## 总结

ForgeOS 的 `forge-core` 是我审阅过的 Go 代码库中架构一致性最高的之一。零依赖策略在遇到边缘场景时证明了它的先见之明。但架构中最大的缺口不是方向一到方向五中任何一个孤立问题，而是这些问题的**交集**——checkpoint 可信度(⑤)、知识一致性(②)、并行冲突(①)之间的依赖三角。其中⑤是三角的顶点：没有可靠的 checkpoint，选择性执行(③)resume 不可信，并行检测(①)恢复不可靠。

审阅文档中第五方向优先级的提前建议是我最赞同的技术判断。我建议在阶段 1 完成 checkpoint 故障注入 + Clock 接口化之后，每交付一个阶段都回归 checkpoint 的原子性契约——这不是可选的"非功能性测试"，它是控制平面信誉的基石。
