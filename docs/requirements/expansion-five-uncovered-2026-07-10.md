# ForgeOS — 全局扫描后的五方向高价值扩展：聚焦编排运行时盲区

> **角色**: 资深架构师 / 产品经理  
> **方法**: 
> 1. 全局深扫 forge-core（140+ Go 源文件 · 18 Go 包 · ~33k LOC 运行时 + CLI）、
>    harness（39+ 模块 · ~10.5k LOC 执法层）、.agent/（12 agent 卡 · 9 skill 卡 · 
>    5 工作流 · 全部 ADR+DECISIONS+architecture）、examples/、pi-batch.py
> 2. **差异化验证**: 逐篇通读 docs/requirements/（45 篇）+ docs/analysis/（40 篇）+ 
>    FUNCTIONAL_REQUIREMENTS_AUDIT + 核心文档，确认每个方向未被已有分析作为独立方向展开。
>    每个方向下方附「与已有分析的核心区别」。
> 3. **纪律**: 不编写任何代码，所有建议附代码级证据。
> **日期**: 2026-07-10

---

## 全景概览

已有 85+ 篇分析文档覆盖了 ForgeOS 几乎全部功能域——执行语义形式化、生产可靠性、第三地平线生态、
二阶伴生问题、系统边界盲区、北极星桥梁等等。但以下五个方向落在所有已有分析的**覆盖间隙**中，
每个都代表一个**已在代码中部分埋点但未作为系统性方向展开**的深层缺口。

| # | 方向 | 代码已存在的基础 | 已有分析覆盖状态 |
|---|---|---|---|
| 1 | **确定性回放: Hermetic Agent Replay** | trace.jsonl + checkpoint + memory 三维数据已落盘，但无回放机制 | seventh-wave-data-realism.md 讨论 test fixture 的数据积累，非运行时回放机制 |
| 2 | **相位级原子性与补偿撤销** | on_fail/on_unmet loop-back + rejection marker 已存在，但无撤销原语 | five-high-value-extensions-v44.md 讨论 `forge rollback` 作为独立命令，非编排原语 |
| 3 | **故障隔离与级联阻断** | parallel.go 有锁顺序合约 + fail-fast wave 取消，但无隔离/熔断 | **零覆盖** — bulkhead/circuit-breaker/resource-isolation 均未在任何分析中出现 |
| 4 | **知识完整性: 信任加权与主动腐化检测** | memory.Supersedes + Confidence + Source 字段已存在但零消费 | **零覆盖** — memory trust/staleness/provenance weighting 均未展开 |
| 5 | **无人值守渐变安全: 从硬截止到梯度响应** | MaxAgentCalls/MaxDepth/MaxOutputBytes/Timeout 四个硬护栏已就位 | **零覆盖** — 以 graceful degradation 为独立方向的系统分析不存在 |

---

## 方向一 · 确定性回放: Agent 执行的 Hermetic Replay

**优先级**: 🟠 P1 | **类别**: 运行时 · 可观测性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 在 Sprint 24-26 已验证了自治闭环: `forge evolve` 驱动的多 agent 无人值守运行
可以持续数小时到数天，产出 trace.jsonl（~100 events/iteration）、checkpoint.json、memory.jsonl。
但**从 trace 数据无法重新生成相同的运行**。如果有人问「三天前的那个 evolve 运行到底为什么
做了那个架构决策？如果换个 prompt 会不会不同？」，答案只能是「再跑一次——但结果可能完全不同，
因为 LLM 输出是非确定性的」。

当前，以下场景全部依赖不可复现的现场:
- **审计**: 合规要求证明某个决策是经过指定 gate 流程产生的
- **Debug**: trace 显示 gate 失败/loop-back 但无法重现失败时的精确 prompt→response
- **调优**: 改变 routing tier 或 prompt 后，无法与历史运行在相同输入条件下对比
- **训练**: 没有「黄金运行」作为测试管线正确性的标定基准

### 代码级证据

1. **trace.go 记录但不存储 agent 输入/输出** — `trace.Event` 有 Status/DurationMs/CostUsdMicros，
   但不包含 agent phase 的 prompt 文本或 agent 响应原文:
   ```go
   // forge-core/internal/trace/trace.go:63-84
   type Event struct {
       Kind         string `json:"kind"`
       Name         string `json:"name"`
       Status       string `json:"status"`
       DurationMs   int64  `json:"duration_ms"`
       CostUsdMicros int64 `json:"cost_usd_micros,omitempty"`
       // 无 Prompt string, 无 Response string
   }
   ```

2. **Tracer.Emit 不可重放** — `trace.Emit` 调用后事件不可撤销，没有 `Replay(r io.Reader)`
   接口来从已存事件流还原状态:
   ```go
   // forge-core/internal/trace/trace.go:89-95
   type Tracer struct {
       mu     sync.Mutex
       seq    int
       writer io.Writer // 只写不回读
       Now    func() time.Time
   }
   ```

3. **CommandExecutor 无 record/replay 模式** — 当前要么调用真实 CLI（`claude -p`），
   要么 dry-run（只叙述）。不存在第三种「从已记录响应回放」的模式:
   ```go
   // forge-core/internal/orchestrator/command_executor.go:45-70
   type CommandExecutor struct {
       Build func(p asset.Phase, mode string) []string
       // 无 Replay func(p asset.Phase, trace []trace.Event) []string
   }
   ```

4. **memory.go 无确定性 replay 模式** — `memory.Append` 追加到 JSONL，但无法从某个
   已存的知识快照「重放」到相同状态（Append 是 IO 副作用、不是纯函数）:
   ```go
   // forge-core/internal/memory/memory.go:173-210
   func Append(path string, e Entry) error {
       // 总是写当前时间，不可重放
   }
   ```
   `CreatedAtUnix` 是调用者注入的，但 Append 总是设置 `Format = "forgeos.memory.v1"`，
   无法被非侵入地「冻住」。

5. **checkpoint.go 是崩溃恢复快照，不是重现点** — `Checkpoint` 存储 `RoadmapCompletion`、
   `Iteration`、`PhaseIndex`，设计目标是「从哪继续」而非「从相同的状态复现」:
   ```go
   // forge-core/internal/persist/checkpoint.go:42-63
   type Checkpoint struct {
       Workflow          string  // 要恢复的工作流
       Mode              string  // 执行模式
       Iteration         int     // 已完成的迭代数
       PhaseIndex        int     // 下一个要执行的相位索引
       // 无 PromptDigest, 无 RoutingSnapshot, 无 MemoryDigest
   }
   ```

6. **forge detect + autoSelectWorkflow 首次将「项目特征→工作流选择」自动化**，
   但一旦 trace 被记录，无法将其作为未来自动回放的匹配条件。

### 与已有分析的核心区别

- `docs/analysis/seventh-wave-data-realism.md` 讨论的是**将真实 trace 数据作为测试 fixture**
  积累，以提升测试的真实度。本文方向一是关于**把 replay 作为运行时原生能力**——不只是测试，
  而是 audit、debug、调优、合规的通用基础设施。两者的产物相似（trace fixture），
  但目标机制完全不同（test-only fixture accumulation vs runtime replay engine）。
- `genuine-architectural-gaps-v28.md` 方向三提到 "deterministic replay" 关键词，
  但该分析的焦点是**测试数据的持续更新**，不是运行时回放引擎。
- 本方向的本质问题是: **forge-core 拥有完整的可观测性数据管线，但只有单向（记录→外部工具），
  没有闭环（记录→回放→验证→改进）。**

### 建议方向

1. **Record mode**: 在 trace.jsonl 外增加 `trace.full.jsonl`，存储每个 agent phase 的
   `{phase, agent, model, prompt_hash, prompt_text, response_text, routing_snapshot}`。
   Omitempty 处理保持轻量，默认不开启（`--record-full` flag）。
2. **Replay executor**: 新增 `ReplayExecutor` 实现 `AgentExecutor` 接口，
   从录制的 trace 响应中回复，而非调用真实 LLM。与 `DryRunExecutor`、`CommandExecutor`
   平级:
   ```go
   type ReplayExecutor struct {
       TracePath  string      // 指向录制好的 trace.full.jsonl
       PhaseIndex map[int]int // phase -> trace entry index
   }
   ```
3. **Trace digest certification**: 运行终结时计算 trace log 的 SHA-256 摘要，
   输出到 `.forge/trace.<run-id>.digest`，使得事后可验证 trace 未被篡改。
4. **Semantic diff between runs**: 两个 trace.full 文件可对比 prompt 差异、
   routing 差异、gate 结果差异——现在完全靠人读。

---

## 方向二 · 相位级原子性与补偿撤销

**优先级**: 🟠 P1 | **类别**: 编排 · 韧性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的编排模型假定**前向推进**。当前，一个 agent phase 写文件、改代码、调用 API，
发生的变更不会自动撤销。当一个 phase FAIL:
- loop-back 重新运行实现 phase，但**错误的中间状态保留在文件系统中**
- checkpoint/resume 跳过已完成的 phase，但**从不撤销已完成的副作用**
- `forge migrate --apply` 修改 project.yml + 注入 ROADMAP 项——但 `--dry` 与 `--apply`
  之间不提供快捷的 `--undo`

这不是 git revert（事后人工清理），而是编排运行时缺乏**相位级补偿原语**。

### 代码级证据

1. **无补偿动作字段** — `asset.Phase` 没有任何 `Compensate` 或 `OnRollback` 字段:
   ```go
   // forge-core/internal/asset/asset.go:40-105
   type Phase struct {
       Name         string
       Agent        string
       RequiredGates []string
       OnFail       *OnFail     // 只有前向动作
       ModelTier    string
       FeedsForward bool
       // 无 CompensatePhase, 无 RollbackAction
   }
   ```

2. **on_fail 只有 loop_back，没有 undo** — `on_fail` 动作词汇表只包含 `loop_back`:
   ```go
   // 仅在 orchestrator.go 中被读取
   // on_fail: {action: loop_back, target_phase: implementer}
   // 不存在 action: compensate 或 action: undo
   ```

3. **`forge migrate --apply` 的变更不可逆** — `internal/migrate/migrate.go` 的 `Apply`
   修改 `project.yml` 和 `ROADMAP.md`，但没有对应的 `Rollback()` 方法:
   ```go
   // forge-core/internal/migrate/migrate.go:86-90
   func (p Plan) Apply(...) error {
       // 只前向，无 undo
   }
   ```

4. **`forge run`/`forge evolve` 在 phase 完成后不做 git snapshot** —
   即使有 checkpoint，但没有运行前的 `git stash` 或 `git commit --allow-empty` 标记，
   所以无法在完成后判断「这个 phase 改了什么」。

5. **computeCodeTestRatio / computeFileDelta 只读 git diff，从不改变 git 状态**:
   ```go
   // forge-core/cmd/forge/gates.go:340
   exec.Command("git", "-C", root, "diff", "--stat", "HEAD").Output()
   // 只读
   ```

6. **`.forge/<stage>.rejected` 标记存在但只触发 phase 重跑，不触发撤销**:
   ```go
   // forge-core/cmd/forge/gates.go:225-265
   // resolveRejectionStartPhase: 仅返回起始 phase 索引
   // 不调用 git checkout 或 undo
   ```

### 与已有分析的核心区别

- `five-high-value-extensions-v44.md` 方向五提到 `forge rollback`，但那是**一个独立 CLI
  子命令**，用于事后（post-merge）按 trace event 撤销先前的代码变更。它不涉及编排层的内建
  补偿原语。
- 本方向不是添加一个新 CLI 命令，而是在**编排状态机层面引入补偿动作**——每个 phase 可以
  声明一个可选的撤销 phase，在 loop-back / rejection / rollback 场景中被编排引擎自动执行。
  这是与 `for`/`while` 平级的语言特性，不是一个事后工具。

### 建议方向

1. **Phase.CompensatePhase 字段**: 声明一个补偿 phase，在 phase FAIL 且 loop-back budget
   耗尽时自动执行（而不是直接 abort）。补偿 phase 运行在原名下但有 `compensating: true` 标记。
2. **Pre-phase git tag/snapshot**: 每个 agent phase 运行前，`forge run` 自动打
   `git tag forge/pre/<phase>`，运行成功后推进到 `forge/post/<phase>`，失败时可
   `git diff forge/pre/<phase> forge/post/<phase>` 精确看出改动。
3. **`forge rollback` 编排原语**: 不要作为独立 CLI 命令，而是作为一种 stop condition
   （`stop_condition.type: rollback`），使用与 `HumanApproved` 相同的标记机制
   （`.forge/<stage>.rollback`），触发编排引擎反向执行。
4. **Compensation workflow** (与 five-high-value-extensions-v44.md 不同):
   不是用户手动调用 `forge rollback`，而是编排器在检测到不可恢复的 gate 失败时，
   自动生成并执行一个 compensation workflow——这是编排层的自愈行为，不是用户的诊断工具。

---

## 方向三 · 故障隔离与级联阻断: Phase 级 Bulkhead + Circuit Breaker

**优先级**: 🟠 P1 | **类别**: 编排 · 韧性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 当前在并行模式下需要一个精细的锁顺序合约（parallel.go:28-54）来避免死锁。
但这个合约只约束了**数据竞争**，没有约束**故障传播**。一个 phase 的失败：
- 通过共享的 memory store 污染后续 phase 的知识
- 通过共享的文件系统留下错误状态
- 通过锁顺序合约中的任意一个 mutex 传播延迟

更关键的是，没有**熔断**机制。如果一个 reviewer phase 连续 3 次 REQUEST_CHANGES，
没有「连续失败计数→跳过该 phase→降级路由」的响应。

### 代码级证据

1. **memory store 全局共享** — 所有 phase 读写同一个 JSONL 文件:
   ```go
   // forge-core/internal/memory/memory.go:173
   func Append(path string, e Entry) error {
       // 单一路径全局存储
   }
   // forge-core/internal/memory/memory.go:230
   func Load(path string) ([]Entry, error) {
       // 所有 phase 看到相同的数据
   }
   ```
   一个受污染的 phase 写入错误 Decision 后，所有后续 phase 的 prompt 都会包含它。

2. **parallel.go 锁顺序合约只防死锁，不防故障传播** — 8 层锁顺序保证了不会 A 等 B、
   B 等 A，但如果一个 phase 持锁太久（IO 阻塞），并行中的所有 phase 都会排队:
   ```go
   // forge-core/internal/orchestrator/parallel.go:28-54
   // 只约定锁获取顺序，不约定锁持有时间
   ```

3. **CommandExecutor 无熔断计数器** — 每个 phase 独立重试 (`MaxRetries`)，但失败计数
   不跨 phase 种类累积:
   ```go
   // forge-core/internal/orchestrator/orchestrator.go:55-60
   type Engine struct {
       MaxRetries   int   // 全局，不按 phase/agent 种类区分
       MaxLoopBack  int   // 全局，不按电路状态区分
   }
   ```

4. **running agent process 不做 cgroup/resource 隔离** — `exec.Cmd` 直接运行在 forge
   进程组中:
   ```go
   // forge-core/internal/orchestrator/command_executor_unix.go:33-50
   func (e *CommandExecutor) start(ctx context.Context, argv []string) error {
       cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
       // 无 Cgroup, 无 rlimit, 无 OOM score adj
   }
   ```

5. **错误分类（KindTimeout / KindOverloaded / KindFailed）存在但被平等对待** —
   `exec_error.go` 定义了三类错误，但 `orchestrator.go` 中所有非重试型错误都被
   同等视为 abort:
   ```go
   // forge-core/internal/orchestrator/exec_error.go:14-22
   const (
       KindTimeout    = "timeout"
       KindOverloaded = "overloaded"
       KindFailed     = "failed"
       KindConfig     = "config"
       // 没有 KindDegraded / KindCircuitOpen
   )
   ```

### 与已有分析的核心区别

- `execution-semantic-gaps.md` 聚焦于时序语义（因果一致性、版本化副作用），不讨论
  phase 间的故障隔离。
- `second-order-architectural-gaps.md` 讨论配置爆炸、TOCTOU、无声数据丢失，
  不讨论编排层的级联故障。
- `forgotten-five-system-boundaries.md` 讨论跨进程边界，不讨论同进程内的 phase 隔离。
- **本方向是 ForgeOS 编排模型的一个结构性缺口**: 作为「软件工厂 OS」，
  它没有进程级（phase 间）的最小特权/故障隔离，同一个进程空间内的任意 phase 可以
  影响所有其他 phase。

### 建议方向

1. **Phase 级 Memory Namespace**: 每个 agent phase 在运行前收到一份 memory 快照，
   运行期间的 Append 写入隔离的分片，phase 成功后 merge 回主 store。失败的分片被丢弃
   （已存在的已有知识不受污染）。这已在 memory.go 中有 `loadCache` 的分片能力基础。
2. **Circuit Breaker per Agent Kind**: 按 agent 角色（implementer/reviewer/planner）
   统计连续失败次数。连续 N 次 gate FAIL 或 REQUEST_CHANGES → open circuit → 跳过该
   phase 的 agent 执行，直接记录「circuit open: 原因」到 trace。
3. **Phase 级 Resource Budget**: 不仅 `MaxOutputBytes` 全局，而是每个 phase 有自己的
   `PhaseMaxCalls int` 和 `PhaseMaxDuration time.Duration`，防止单个 phase 耗尽全局预算。
4. **故障注入接口**: 为测试编排韧性，暴露一个 `FaultInjector` 接口（如阶段性地让
   gate 返回 FAIL），让系统可以非侵入式验证熔断/隔离的正确行为。
5. **Progressive failure escalation**: 不是 binary pass/fail，而是
   `PASS → WARN → DEGRADED → FAIL` 四档，编排器可以据此做出不同响应:
   - WARN: 记录告警，继续
   - DEGRADED: 降级路由（haiku 替代 sonnet），继续
   - FAIL: 正常熔断/loop-back

---

## 方向四 · 知识完整性: 信任加权记忆与主动腐化检测

**优先级**: 🟡 P2 | **类别**: 记忆 · 智能 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐

### 问题描述

memory store 是 ForgeOS 跨会话知识的核心。但当前的设计是**全信任、无质疑**的:
- 任何 phase 写入的 `Entry` 与任何其他 phase 写入的具有相同权重
- `Supersedes` 字段存在但需要写入者主动设置——没有人检查「这条 Decision 是否正确」
- `Confidence` 字段存在但当前以默认 1.0 被全接受——只在 prompt 层添加 `[unverified]` 前缀，
  没有系统级的信任模型
- 没有机制检测记忆腐化: 一个早期 phase 写入的错误知识会无限期地毒害所有后续 phase

### 代码级证据

1. **`memory.Entry.Confidence` 被定义但零消费** — 代码中有 `Confidence float64` 字段，
   但 `prompt_memory.go` 的 `appendMemoryLane` 和 `buildMemoryBlock` 从不读取它:
   ```go
   // forge-core/internal/memory/memory.go:125-130
   type Entry struct {
       Confidence float64 `json:"confidence,omitempty"` // 0.0-1.0, default 1.0
       // 无消费者
   }
   ```
   `grep -rn "\.Confidence\b" forge-core/cmd/forge/` 证实零引用。

2. **`memory.Entry.Source` 被定义但零消费** — `Source string` 记录 phase/agent 来源，
   但 `Query` 函数不按来源过滤或加权:
   ```go  
   // forge-core/internal/memory/memory.go:155-159
   type Entry struct {
       Source string `json:"source,omitempty"` // phase/agent 来源
   }
   // Query 只按 Kind+Topic 过滤，不区分来源
   // forge-core/internal/memory/memory.go:286-310
   func Query(entries []Entry, kind, topic string) []Entry {
       // 无信任权重
   }
   ```

3. **`memory.Entry.Supersedes` 有机制但无主动触发** — 只依赖写入者主动设置。
   没有人检测「一条 3 天前的 Decision 与最新 gate 结果矛盾」并自动 supersede:
   ```go
   // forge-core/internal/memory/memory.go:133-143
   type Entry struct {
       Supersedes string `json:"supersedes,omitempty"` // 手动设置才有用
   }
   ```

4. **memory.Compact 保留摘要但只按时间裁剪** — 摘要合成时不做内容验证:
   ```go
   // forge-core/internal/memory/memory_compact.go:81-206
   func Compact(path string, threshold, keepPerKind, ageSeconds int) {
       // 按时间排序，保留最近 keepPerKind 条，更早的合成摘要
       // 不检查知识是否正确
   }
   ```

5. **无「跨 phase 矛盾检测」** — reviewer 发现 implementer 的代码实现与架构决策矛盾，
   memory 中保留两者。没有人检测并标记「决策 A 与事实 B 矛盾」。

### 与已有分析的核心区别

- `forgotten-five-meta-governance-and-blindspots.md` 讨论治理元数据的一致性。
- `expansion-self-governance-and-hygiene.md` 讨论 agent 行为的自我约束。
- `second-order-architectural-gaps.md` 讨论知识衰减作为「二阶伴生问题」，
  但视角是形式化的配置和依赖管理，不是运行时记忆的信任模型。
- **本方向是唯一聚焦于「记忆层自身的完整性」的分析**——不是记忆的存储格式（已解决）、
  不是记忆的检索（已解决）、而是「记忆是否可信」这个更根本的问题。

### 建议方向

1. **Source-trusted weighting**: 按 agent 角色赋予信任权重。reviewer 的 Lesson 置信度高于
   implementer 的 self-report。planner 的 Decision 置信度高于相同条件下 harvester 的发现。
   权重可在 `.agent/policies/memory.yml` 中声明。
2. **Cross-phase contradiction detection**: `memory.Load` 后遍历 Entry，检测同一 Topic
   下 Kind=Decision 且 Detail 可被当前 gate 结果（GatesGreen=false）否定——自动赋予
   Supersedes。
3. **Active staleness annotation**: 如果一条知识超过 TTL（由 `memory.TTL` 声明，默认 7 天
   但 agent 卡可覆盖），在注入 prompt 前自动添加 `[STALE: created N days ago]` 前缀。
4. **Gate-backed 知识验证 phase**: 新增工作流类型 `verify-knowledge.yml`，其唯一职责是:
   读取 memory store 中未经验证的条目，执行最小验证（检查引用文件是否存在、配置值是否一致），
   置信度不足则标记 `confidence: 0.3`。
5. **Memory hygiene score**: 记忆存储的「健康分」——`memory.Prune` 和 `memory.Compact`
   的触发改为按健康分（总条目数 × 未验证比例 × 平均年龄）动态决策，而非固定阈值。

---

## 方向五 · 无人值守渐变安全: 从硬截止到梯度响应

**优先级**: 🟡 P2 | **类别**: 安全 · 运营 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

Sprint 22-23 为真点火了创建了四个硬护栏: 递归深度上限（MaxDepth）、agent 调用总数上限
（MaxAgentCalls）、输出大小上限（MaxOutputBytes）、超时（Timeout）。这四个护栏是
**二值开关**: 要么运行正常，要么被切断。没有中间状态。

在一个 24h 自治运行的场景中，硬截止是最差的选择:
- 一个 implementer phase 逐渐变慢（每个迭代多花 30s）→到达全局 timeout → 整个运行 abort
- memory store 逐渐增长（每次迭代多 5 条 entry）→ 达到 $budget 上限 → abort
- reviewer 连续 2 次 REQUEST_CHANGES（但第三次可能 APPROVE）→ 没有「再给一次机会?
   但降级到 haiku」的选项

### 代码级证据

1. **所有护栏都是单阈值硬截止** — 没有分级告警或降级:
   ```go
   // forge-core/internal/orchestrator/command_executor.go:55-60
   type CommandExecutor struct {
       MaxDepth       int   // 达到→拒绝
       MaxOutputBytes int   // 达到→截断
       Timeout        time.Duration // 达到→SIGKILL
   }
   // forge-core/internal/orchestrator/orchestrator.go:120-130
   type Engine struct {
       MaxAgentCalls int // 达到→拒绝
   }
   ```

2. **Engine.executeAgent 对预算超限只有拒绝，无降级** — `checkAgentBudget` 返回 true/false，
   没有中间档:
   ```go
   // forge-core/internal/orchestrator/orchestrator.go:260-270
   func (e *Engine) checkAgentBudget() bool {
       // return count < MaxAgentCalls || MaxAgentCalls == 0
       // 二值
   }
   ```

3. **error 分类不分级** — `exec_error.go` 的 `KindTimeout`、`KindOverloaded` 等同对待，
   没有「WARN → 记录但继续」:
   ```go
   // forge-core/internal/orchestrator/exec_error.go:14-22
   type Kind string // 常量，无严重度字段
   ```

4. **`cost.go` 有 `overloadDetected` 检测 529 但只做单次 backoff**，
   没有「连续 N 次 overload → 降级路由 tier」的逻辑:
   ```go
   // forge-core/internal/orchestrator/backoff.go:35-55
   func backoff(ctx context.Context, attempt int) bool {
       // 指数退避，但从不改变 agent 配置
   }
   ```

5. **loop.go 对 no-progress tripwire 只做 abort** — 当 `RoadmapCompletion` 多个迭代
   未进步时，loop 终止。没有尝试降级（如切换到 explorer mode 完成剩余 work）:
   ```go
   // forge-core/internal/orchestrator/loop.go:386-410
   // noProgress 递增 → tripwire → abort
   // 无 graceful degradation 路径
   ```

6. **memory.Prune 是写入后的清理，不是写入前的预防** — 内存增长是发现问题，
   没有写入前的预算检查:
   ```go
   // forge-core/internal/memory/memory.go:313-350
   func Prune(path string, keepLast int) error {
       // 事后裁剪
   }
   ```

### 与已有分析的核心区别

- `expansion-production-blindspots-v36.md` 讨论生产环境盲区（监控缺失、故障恢复不完整），
  但不聚焦于护栏本身的**响应梯度**。
- `expansion-production-perspectives.md` 讨论生产就绪度，但缺少对「硬截止是最危险的
  护栏形态」这一认知。
- `expansion-production-readiness.md` 更广泛地覆盖生产就绪，但「梯度响应」作为一个
  独立的设计模式未被识别。
- `forgotten-five-foundations.md` 方向一（跨进程守护）涉及进程级监控，不是编排级梯度。
- **本方向的核心洞察**: 安全护栏在 24h 自治场景中，二值截止不是最安全的——它把系统从
  「部分降级但仍在工作」推向「完全不可用」。梯度响应既是安全增强，也是可用性增强。

### 建议方向

1. **三档告警阈值**: 每个资源维度（calls/depth/output/time/memory）配置 `warn/critical/block`
   三档:
   - warn（70% 阈值）: 记录告警到 trace，继续运行
   - critical（90%）: 记录告警 + 降级当前 phase tier + 触发 memory prune
   - block（100%）: 正常拒绝/spawn（当前行为）
2. **Graceful degradation response table**: 按阈值档位定义系统响应:
   ```
   output_bytes > 70% → 截断当前 phase 输出为摘要 + 通知
   output_bytes > 90% → 降级所有后续 phase 到 haiku
   max_calls > 70%   → 跳过非关键 phase（如 secondary_template）
   max_calls > 90%   → 终止 evolve loop 但保持 converge: MET
   memory > 70%      → 主动触发 Compact + Prune
   memory > 90%      → 暂停新的 memory.Append 直到下一迭代
   timeout 连续 3 次  → 降级当前 agent 到更低 tier + 缩短 timeout
   no-progress 2 次  → 切换 evolve loop 到 lighter mode
   ```
3. **Per-phase budget quota**: `MaxAgentCalls` 从全局共享改为每个 phase 有配额。
   一个 phase 提前消耗完自己的配额不影响其他 phase（见方向三）。
4. **Degradation log**: 每次降级决策记录到 trace，格式:
   `{kind: "degradation", phase: "implementer", from: "sonnet", to: "haiku", reason: "3 consecutive timeouts"}`
   使得事后审计可追溯「系统为什么变慢了」。
5. **Recovery path**: 一旦降级后，定义何时恢复（而不是永远停留在降级状态）:
   连续 N 次 phase PASS 且 latency 低于 P50 → 自动恢复 tier。

---

## 总结

| 方向 | 代码已就位的基础 | 已有分析覆盖 | 建议优先级 |
|---|---|---|---|
| 1. 确定性回放 | trace/checkpoint/memory 全管线 | 仅 test fixture 讨论 | P1 — 审计/合规基础 |
| 2. 相位级补偿撤销 | on_fail/loop-back/rejection marker | 仅 CLI rollback 概念 | P1 — 编排基础原语 |
| 3. 故障隔离与熔断 | 锁顺序合约 + fail-fast | **零覆盖** | P1 — 自治可靠性基础 |
| 4. 信任加权记忆 | Supersedes/Confidence/Source 字段 | **零覆盖** | P2 — 智能增强 |
| 5. 梯度响应安全 | 四个硬护栏 + backoff | **零覆盖** | P2 — 运营安全增强 |

五个方向的共同主题: **ForgeOS 的编排运行时已经完整（5 引擎、18 Go 包、~33k LOC），
但它在「韧性/可信自治/运营安全」维度的原语仍然是二值的、单层次的。
将这五个方向落地，将把系统从「能跑」提升到「能可靠地、可审计地、可恢复地自治运行」。**
