# ForgeOS — 高价值扩展方向:从未被覆盖的五个视角

> **视角**: 资深架构师 / 产品经理  
> **方法**: 全局代码库扫描(forge-core 41 源文件 + harness 30+ 模块 + `.agent/` 全套治理 + docs/analysis 18 份已有分析)  
> **原则**: 不重复已有分析已覆盖的方向(沙箱/Provider 抽象/Learning 回灌/Discover Engine/PhaseGate/多仓治理/性能优化)。  
> **基线**: `b0c80e4` — Loop Memory/Learning + Adaptive Assembly + Reflect; Sprint 26 全状态。  
> **日期**: 2026-07-01  
> **不写代码**,只做判断与优先级排序。

---

## 当前能力基线(已有分析已覆盖 + 落地状态)

| 域 | 状态 | 覆盖已有分析 |
|------|--------|-------------|
| 编排引擎 + 收敛 + Loop | ★★★★★ 生产就绪 | 12+ 份 |
| 中枢旋钮(mode×lifecycle) | ★★★★★ 完整 | 6+ 份 |
| 真点火 Multi-Agent 闭环 | ★★★★★ 已验证 | 4+ 份 |
| 安全护栏(4 维) | ★★★★★ 完整 | 3+ 份 |
| Harness 闸门套件 | ★★★★☆ 完整 | 10+ 份 |
| Memory/Trace/Scorecard | ★★★★☆ 数据面完整 | 8+ 份 |
| Runner/Sandbox | ❌ 不存在 | 已充分分析 |
| Cross-Vendor Provider | ❌ 不存在 | 已充分分析 |
| Discover Engine | ❌ 不存在 | 已充分分析 |
| Learning 回灌 | ⚠️ 骨架 | 已充分分析 |
| Web UI / Dashboard | ❌ 不存在 | 已充分分析 |

> **核心判断**: 已有 18 份分析文档覆盖了上述方向。  
> 以下 **5 个方向** 是全局扫描后发现**尚未被任何已有分析覆盖**的高价值缺口。

---

## 目录

1. [方向一:外部事件反应式 Workflow 引擎(Event-Driven Reactivity)](#方向一外部事件反应式-workflow-引擎event-driven-reactivity)
2. [方向二:并行 Agent 输出合并与冲突解决(Output Merging & Conflict Resolution)](#方向二并行-agent-输出合并与冲突解决output-merging--conflict-resolution)
3. [方向三:人类反馈分析系统与质量度量(Human Feedback Analytics)](#方向三人类反馈分析系统与质量度量human-feedback-analytics)
4. [方向四:确定性 Replay 与 Agent 行为调式设施(Deterministic Replay & Debug)](#方向四确定性-replay-与-agent-行为调式设施deterministic-replay--debug)
5. [方向五:Workflow 成本预测与预算规划(Cost Forecasting & Budget Planning)](#方向五workflow-成本预测与预算规划cost-forecasting--budget-planning)

---

## 方向一:外部事件反应式 Workflow 引擎(Event-Driven Reactivity)

### 核心洞察

ForgeOS 当前所有工作流都是**同步轮询式**的:

```
forge run build    → 启动 → 等完成 → exit
forge evolve       → 启动 → 等收敛 → exit
forge run approve  → CLI flag → 标记文件 → exit
```

系统中**没有以下能力**:
- 监听一个 GitHub PR 合并事件 → 自动触发 evolve
- 在 CI 完成时收到 webhook → 决定是否继续下一个 stage
- 等待一个异步的外部审批(不是 CLI `--approved`,而是真实的人通过 Slack/Web 点击"批准")→ workflow 自动恢复
- 在工作流中途等待外部条件(等依赖发布、等安全扫描完成)→ 再继续剩余的 phases

**后果**:ForgeOS 无法被嵌入到真实的 CI/CD 流水线中。它只能是一个"手动启动→手动等结果→手动决定下一步"的工具,而不是一个真正的自治平台。在 24h 无人值守场景下,一个卡在"等待安全扫描结果"的 workflow 只能超时报错,而不是优雅挂起等人回来。

### 现状代码锚点

```go
// converge.go — 当前只有同步信号
type Signals struct {
    RoadmapCompletion float64  // 文件系统读取
    GatesGreen        bool     // 即时运行 gate
    HumanApproved     bool     // 文件标记或 CLI flag
}

// loop.go — 没有"等待外部信号"的阶段
// LoopEngine.Run 是严格同步的:
for i := startIter; i <= l.MaxIter; i++ {
    runErr := l.Engine.RunFrom(wf, mode, startPhase)  // 阻塞等全部 phase 完成
    sig := l.Signals()                                 // 立即读取信号
    if converge.Converge(l.Stop, sig).Met { break }    // 立即判断
}
```

**关键缺口**:`Signals()` 是一个同步函数,返回调用时刻的快照。没有 `WaitForSignal(name) (Signal, error)` 的概念 —— 等待一个外部事件发生,然后继续。

### 设计思路:External Event Bus

```go
// 新增接口（框架,非完整实现）
type SignalSource interface {
    // Name returns a unique identifier for this signal source, e.g. "github-pr-merge"
    Name() string
    // Await blocks until the external signal arrives or ctx is cancelled.
    // Returns the signal value and a done indicator.
    Await(ctx context.Context) (value float64, done bool, err error)
}
```

LoopEngine 中的新状态:

```go
// LoopEngine 新增字段
ExternalSignals []SignalSource  // workflow 声明的外部信号源

// Run 中的新逻辑:
for i := startIter; i <= l.MaxIter; i++ {
    // 1. 运行相位(与现有逻辑相同)
    runErr := l.Engine.RunFrom(...)
    
    // 2. 检查收敛(与现有逻辑相同)
    sig := l.Signals()
    if converge.Converge(l.Stop, sig).Met { break }
    
    // 3. ★新:检查是否需要等待外部信号
    for _, src := range l.ExternalSignals {
        if converge.NeedsSignal(l.Stop, src.Name()) {
            l.Log(fmt.Sprintf("awaiting external signal: %s", src.Name()))
            val, done, err := src.Await(ctx)
            if err != nil { return err }
            if done { break }  // 外部信号终止了整个流程
            // 将 val 注入下一次迭代的收敛判断
        }
    }
}
```

### 边界情况

| 场景 | 问题 | 方案 |
|------|------|------|
| **Workflow crash 后 resume** | 外部信号已到达但 checkpoint 丢失 | checkpoint 应记录已收到的 signal + 值,resume 时不再重等 |
| **多个信号同时等待** | CI 完成 + 安全扫描完成,顺序无关 | 用 `select` 模式并发等所有,或声明依赖顺序 |
| **信号超时** | 外部系统 3 天没响应 | 声明式 `timeout` 字段 + fail-closed 默认;workflow 可声明 `on_timeout: abort \| skip \| continue_unchecked` |
| **信号伪造/可信度** | 如何验证 webhook 来自真实的 GitHub | webhook secret 签名验证 + `SignalSource` 的 `Verify(payload) error` |
| **多次信号送达** | PR 被合并后又 reopen+merge | 幂等性:signal 带唯一 ID,已处理则忽略 |

### 为什么高价值

- **从"手动 CLI"到"事件驱动的自治平台"的跃升** — ForgeOS 不再需要人守候
- **打开 CI/CD 集成场景** — GitHub Actions/GitLab CI/Jenkins 发 webhook → ForgeOS 自治响应
- **使 24h 无人值守真正可行** — workflow 不需要在一次 session 内跑完,可以挂起数天等人或外部系统
- **与现有架构正交**:不改变任何 orchestration 核心逻辑,只在 LoopEngine 收敛检查与下一迭代之间插入一个等待层

### 接入代价估计

- Core: ~300 行(接口定义 + LoopEngine 集成 + checkpoint 扩展)
- 第一个实现(GitHub webhook 监听): ~200 行 + 外部依赖(HTTP server)
- 对现有 workflow 的冲击:零(未声明 external_signals 的工作流行为不变)

---

## 方向二:并行 Agent 输出合并与冲突解决(Output Merging & Conflict Resolution)

### 核心洞察

`RunParallel` (parallel.go + waves.go) 使能了**波内并行 agent 相位**:同一个 workflow 中多个 implementer 同时写代码。但当前系统**完全不处理输出冲突**:

```go
// parallel.go — runWave 中:
for _, idx := range wave {
    go func(i int) {
        if err := e.runPhaseParallel(waveCtx, wf, i, mode, ...); err != nil {
            // 记错误,取消波,丢弃所有输出
        }
    }(idx)
}
```

**关键问题**:
1. **两个并行 agent 编辑同一文件**:agent A 修改 `user.go` 的 `CreateUser()`,agent B 修改 `user.go` 的 `DeleteUser()`——后者覆盖前者的改动(取决于谁最后写盘)
2. **没有 diff 合并**:即使用户代码最终正确,两个 agent 输出的 `go.mod` 或 `package.json` 会互相覆盖依赖
3. **没有依赖一致性**:agent A 添加了 `uuid` 库,agent B 的代码依赖它——但 B 是在没有 `uuid` 的工作空间里跑的

### 现状代码锚点

```go
// engine_build.go — runAgentPhase 直接让 agent 写盘:
func (e Engine) runAgentPhase(ctx context.Context, p asset.Phase, mode string) error {
    // agent 直接在项目根目录写文件 (CommandExecutor.Dir = o.root)
    // 不同 phase 之间没有工作空间隔离
    // 同一个文件可以被多个并发 agent 同时写
}
```

> 注意:当前所有 workflow 中 build.yml 是串行的(planner→implementer→harness→reviewer→qa),所以实战中尚未遇到并行冲突。但 `--parallel` 已在代码中可用,且 waves.go 支持 fan-out pattern。一旦用户定义了一个有多个并行 implementer 的 workflow,冲突就是必然的。

### 设计思路:Isolated Workspace + Diff-Based Merge

```
当前模型(无隔离):
  RunPhase A: agent → 直接写 project/src/        ← A 写 user.go
  RunPhase B: agent → 直接写 project/src/        ← B 写 user.go(覆盖 A)

建议模型(工作空间隔离):
  RunPhase A: agent → 写 .forge/workspace/a/     ← A 写 user.go
  RunPhase B: agent → 写 .forge/workspace/b/     ← B 写 user.go
  ── 波完成后 ──
  MergeEngine: diff .forge/workspace/a/ 和 project/src/  → patch_a
               diff .forge/workspace/b/ 和 project/src/  → patch_b
               auto-merge patch_a + patch_b → merged
               如果有冲突 → 标记冲突文件 → 报错或交给合并 phase
               如果无冲突 → apply merged → 更新 project/src/
```

### 冲突类型与处理策略

| 冲突类型 | 检测方式 | 处理策略 |
|----------|---------|---------|
| **同一文件同一函数被修改** | `git merge` 风格 3-way diff | 标记冲突,加 `>>>>>>>` 标记,走人工解决或提级给合并 agent |
| **同一文件非重叠区域被修改** | patch 可光滑合并 | 自动合并,零人工 |
| **同一 go.mod 依赖冲突** | semver 区间分析 | 取更高版本或更宽区间,记录决策理由 |
| **同一 import 路径冲突** | 合并去重 | 自动去重 |
| **结构性冲突(A 删了 B 依赖的函数)** | `go build` 或 `tsc` 编译检测 | 合并后自动运行编译 gate;FAIL 则回退到串行重跑 |

### 边界情况

| 场景 | 问题 | 方案 |
|------|------|------|
| **三路合并并非总是正确的** | 并行 agent 对同一变量的修改 != 串行叠加 | 声明式 `merge_strategy: sequential|parallel-safe|isolated` |
| **合并后的文件需要格式化** | A 的缩进风格 != B 的 | 合并后统一运行 formatter(gofmt/prettier) |
| **工作空间文件系统压力** | N 个并行 agent = N 份全项目副本 | 写时复制(overlay fs)或符号链接只读基准 |
| **合并失败后恢复** | 合并冲突中断了整个波 | 回退到串行模式重新执行冲突的 phases 或走 on_conflict: abort|retry-serial |

### 为什么高价值

- **真正释放并行编排的潜力**:没有输出合并,`--parallel` 就无法安全地用于任何会写代码的场景
- **防止隐蔽的数据丢失**:当前并行 agent 写到同一目录,文件被静默覆盖——最坏的情况是一个 agent 的代码在测试中通过但被另一个 agent 的写入静默覆盖
- **为 future 的分治策略铺路**:未来可以"把这个 feature 切成 3 块 → 3 个 implementer 并行写 → merge → review"
- **所有基础设施已就绪**:`checkpoint` 可记录每波完成状态;`trace` 可记录合并事件;`gate` 可验证合并后代码

### 接入代价估计

- 工作空间隔离: ~200 行(for each phase, set Dir to workspace dir + copy baseline)
- Diff/Merge 引擎: ~400-600 行(基于 Go stdlib 的 `bytes`/`strings` 差异对比,或 embed `diff` 命令;不引入外部 git 依赖)
- 冲突检测 + 报告: ~200 行
- 现有 workflow 冲击:零(串行 workflow 不走隔离+合并);parallel 启用此路径但仅在使用 `depends_on`+`--parallel` 时生效

---

## 方向三:人类反馈分析系统与质量度量(Human Feedback Analytics)

### 核心洞察

ForgeOS 当前有**四个触点**产生人类反馈:

1. **Human Approval Gate**(design.yml):人类说"批准"或"拒绝"
2. **Reviewer VERDICT**(build.yml):人类(或 AI reviewer)说 APPROVE / REQUEST_CHANGES / REJECT
3. **QA 结果**:测试通过/失败
4. **Converge 判否**:人类看到 ROADMAP 没完成,手动中断迭代

**现状**:所有这些都是二进制信号,用完即弃。系统从不问以下问题:
- "这个 implementer agent 被 reviewer 打回修改的概率是多少?"
- "explorer mode 下产出的代码被 Approve 的概率是否显著低于 balanced mode?"
- "哪个 agent 类型的产出需要最多的人工重写?"
- "人类的 approve 率是否在随时间下降(系统在变好)还是在上升(系统在变差)?"
- "REQUEST_CHANGES 最常见的根因是什么——缺少测试?架构违规?还是单纯的 bug?"

**后果**:系统对自己的质量表现**完全没有感知**。它知道"这次通过了/没通过",但不知道"我是不是在变好"。无法做数据驱动的 agent 选择、prompt 调优、或 mode 校准。

### 现状代码锚点

```go
// prompt_verdict.go — reviewer VERDICT 被解析后只用于收敛:
type Verdict struct {
    Decision     string            // APPROVE | REQUEST_CHANGES | REJECT
    ChangesRequested []string      // 评审意见
}
// 之后:verdict 被 verdictLedger 记录,被 wasReworked() 消费 → 仅 trace 和 converge 使用
// 但从无汇总分析:不同 agent 类型的 rework rate? trend over time?

// converge.go — HumanApproved 是布尔值,没有"谁批准的""多少次通过的"元数据
```

### 设计思路:HumanFeedback Recorder + Analytics Pipeline

```go
// 新增数据结构(在 trace 或独立的 feedback 包中):
type HumanFeedback struct {
    Timestamp    time.Time
    Stage        string       // design | build | review | qa
    Decision     string       // approved | rejected | changes_requested | interrupted
    Actor        string       // human (always — AI reviewer feedback 是不同路径)
    PhaseName    string       // 被评审的 phase 名称
    AgentCmd     string       // 哪个 agent 产生了这段输出
    ModelTier    string       // 使用的模型档位
    Mode         string       // 当时的 mode
    Reason       string       // 人类给出的原因(自由文本,如果有)
    IterationN   int          // 在 evolve 的第几次迭代中
    ReworkCount  int          // 这是第几次重跑
}
```

**消费端:系统应该能回答的查询**:

| 查询 | 实现方式 | 价值 |
|------|---------|------|
| 某个 agent 类型的 REQUEST_CHANGES 率 | `SELECT agent_cmd, count(*) WHERE decision=changes_requested GROUP BY agent` | 数据证明哪个 agent 最好 |
| Approve 率的时间趋势 | 按周聚合 approve/(approve+reject+changes_requested) 比率 | 检测系统退化 |
| 最常见的 REJECT 原因 | 对 Reason 做关键词聚类 | 针对性改进(如 60% 的原因是"缺测试"就去改进测试 prompt) |
| 不同 mode 下的 human approve 率对比 | explorer vs balanced vs engineering | 校准 mode 默认值 |
| 某个特定 agent 的 rework cost | 每次 rework × 对应 phase 的 cost | 为 agent 选择提供成本依据 |

### 边界情况

| 场景 | 问题 | 方案 |
|------|------|------|
| **反馈稀疏** | 一天只有 2 个人类反馈,统计不可靠 | 设置 `min_samples` 阈值,低于时不报告趋势;使用贝叶斯平均 |
| **反馈者偏差** | 同一个人既 approve 又 reject,标准不一 | 记录 actor ID(但尊重隐私);不做人之间的比较 |
| **因果关系混淆** | approve 率上升可能是因为 code 变好了,也可能是因为 reviewer 变宽松了 | 分离"代码质量"和"审核严格度"两个维度;ai-reviewer 使用固定 prompt 作为校准基线 |
| **冷启动** | 新系统没有历史数据 | 使用 `scorecard` 的 bootstrap prior 作为初始基线;随真实数据增加衰减 prior 权重 |

### 为什么高价值

- **唯一一个能让 ForgeOS"知道自己做得好不好"的方向** — 没有这个,所有"改进"都是猜的
- **直接驱动 Prompt 优化和 Agent 选择**:数据证明 agent X 的代码被 REJECT 率比 agent Y 高 3 倍 → 换 agent Y
- **向管理层提供可量化的 ROI**:"实施此 feature 的平均 agent 成本是 $X,平均需要 Y 次迭代,人类介入 Z 次"
- **与现有的 trace/memory/scorecard 基础设施完全兼容**:feedback 数据可以写入 trace.jsonl(新增 kind=feedback)、可以被 scorecard 聚合、可以被 memory 查询

### 接入代价估计

- 数据模型 + 写入: ~150 行
- 聚合查询 + 报告: ~200 行
- 现有 reviewer/approve 路径接入: ~50 行(在 parseReviewerVerdict 和 cmdApprove 处加一行 emit)
- CLI 查询: `forge feedback report` — ~150 行

---

## 方向四:确定性 Replay 与 Agent 行为调试设施(Deterministic Replay & Debug)

### 核心洞察

当一个 24h 的 `forge evolve` 在生产环境中失败时(比如第 37 次迭代 agent 写了一行错误的代码导致编译失败),**当前系统没有任何方式回答"为什么"**:

- Memory 记录了 agent 输出了什么(`KindNote`, `KindDecision`),但**不记录为什么**
- Trace 记录了耗时和状态(agent ok, gate FAIL),但**不记录 agent 收到了什么 prompt**
- Checkpoint 记录了执行位置(iter=37, phase=3),但**不能恢复到那个时刻重放**
- Cost ledger 记录了花了多少钱,但**不能告诉你花在哪了**

结果:一个 24h 的自治运行如果失败,除了看 claude 的原始输出日志(如果还有留存)外,没有任何工具可以调试。这直接导致了**对自治运行的信任赤字**——Operator 不敢让系统无人值守跑过夜,因为出了事搞不清原因。

### 现状代码锚点

```go
// trace.Event — 记录 WHAT 发生了,不记录 WHY:
type Event struct {
    Kind       string  // "agent" | "gate" | "iteration"
    Status     string  // "ok" | "FAIL"
    DurationMs int64
    // 没有: PromptHash, AgentOutput, Seeds, ConfigSnapshot
}

// checkpoint — 只记录位置,不记录上下文:
type Checkpoint struct {
    Iteration    int
    PhaseIndex   int
    // 没有: mode/lifecycle/agent-cmd/workflow 的快照
    // 所以即使能 resume,也无法保证和之前跑在相同配置下
}
```

**缺失的关键原语**:

| 原语 | 现状 | 需要 |
|------|------|------|
| **Prompt 可复现性** | prompt 每次动态组装(Files/Gather/Memory),无法还原 | 记录 `SHA256(prompt)` 或完整 prompt 到 trace |
| **Agent Seed** | agent 调用无 seed/温度控制,非确定 | 对非 LLM 测试,seed 控制;对 LLM,至少记录 model + temperature 参数 |
| **Config Snapshot** | mode/lifecycle/agent-cmd 在 run 开始后可能变化 | checkpoint 在工作流开始时存一份 config snapshot |
| **Input Versioning** | agent 读到的文件版本 = 当前文件系统 | 在 agent phase 开始前 git commit-ish/git diff 快照 |
| **Decision Trace** | agent 的内部推理过程不可见 | 依赖 LLM provider 的 `thinking`/`reasoning` 输出(如果支持) |

### 设计思路:Replay Log

```go
// 新增类型:在 checkpoint 旁边或作为新的 .forge/replay/ 目录
type ReplayRecord struct {
    Iteration    int
    PhaseIndex   int
    PhaseName    string
    Timestamp    time.Time
    ConfigHash   string       // mode+lifecycle+executor+agent-cmd+workflow 的 SHA256
    // 输入
    PromptHash   string       // 发给 agent 的完整 prompt 的 SHA256
    PromptBytes  int          // prompt 大小(token 数可通过外部工具估算)
    FileSnapshot string       // phase 开始前项目文件的 git-tree-ish 或 tar.gz hash
    // 输出
    AgentOutput  string       // agent 产生的完整输出(可能很大,可配置是否保存)
    ExitCode     int
    DurationMs   int64
    GateResults  []GateResultSnapshot
}
```

**Replay 命令**:

```bash
forge replay --iter=37 --phase=3   # 重放第 37 次迭代第 3 阶段的 prompt + 环境
forge replay --iter=37 --phase=3 --diff  # 重放并对比输出是否与原始一致
```

### 边界情况

| 场景 | 问题 | 方案 |
|------|------|------|
| **Prompt 巨量(100K tokens)** | 存完整 prompt 会迅速填满磁盘 | 可配置:默认存 hash,`--store-prompts` 存完整文本;`max-prompt-storage-mb` 做上限 |
| **文件系统快照** | 每次 phase 前 tar 整个项目 CV 爆炸 | 用 git 做快照:在 phase 前 `git stash` + 记录 HEAD;replay 时 `git checkout` 到对应 commit |
| **LLM 非确定性** | 同一 prompt 给 Claude 两次可能得不同结果 | replay 标记 "non-deterministic — actual output may differ";比较实际输出与记录输出的 diff |
| **重放时外部依赖不同** | 重放时 Go 模块版本或 npm 包版本已更新 | 快照中记录 `go.sum` hash 或 `package-lock.json` hash;重放时警告依赖变化 |
| **存储成本** | 一次 50 迭代 evolve 可能产生 500+ 条 replay 记录 | 默认只保留最近 3 次 evolve 的 replay;`forge replay prune --keep 3` |

### 为什么高价值

- **解决自治系统最大的信任问题**:Operator 不敢无人值守的根本原因是"出事了没法查"。replay 直接消灭这个恐惧
- **使 Agent 行为可审计**:不是"claude 说了什么",而是"在什么环境下、看到什么上下文、做了什么决定"
- **质量改进飞轮**:replay 失败的 phase → 定位 root cause → 修复 prompt/engine → 回放验证修复有效
- **与已有设施正交**:trace 已经有 sequence;checkpoint 已经有 iteration tracking;只需在 checkpoint 中扩展 replay 元数据

### 接入代价估计

- ReplayRecord 模型 + 写入: ~200 行
- File snapshot(git-based): ~100 行
- 重放 CLI 命令: ~300 行
- 现有代码集成(在 phase 前后加记录点): ~100 行
- 存储管理 + 裁剪: ~100 行

---

## 方向五:Workflow 成本预测与预算规划(Cost Forecasting & Budget Planning)

### 核心洞察

ForgeOS 当前的成本管理是**反应式**的:

- `--run-budget-usd`:设置硬上限,超了停
- `--max-agent-calls`:设置 phase 数上限
- `--timeout`:设置 wall-clock 上限

**没有一个问题是"主动规划"的**:
- "实现这个 ROADMAP item 大概要花多少钱?" → 不知道
- "用 Opus 实现 vs Sonnet 实现,成本差多少,质量差多少?" → 不知道
- "Budget 还剩 $50,够不够跑完剩下的 3 个 ROADMAP items?" → 不知道
- "如果我把 mode 从 balanced 换成 explorer,成本节省多少,但质量风险增加多少?" → 不知道

**后果**:Operator 要么设置一个过于慷慨的 budget 导致浪费,要么设置过于紧张的 budget 导致 workflow 反复被中断。**成本管理是"撞墙式"的**,不是"导航式"的。

### 现状代码锚点

```go
// cost.go — 只追踪已发生的成本:
type runBudget struct {
    spentMicros int64      // 累积花费
    capMicros   int64      // 硬上限
    mu          sync.Mutex
}

// scorecard_wind.go — 只记录历史:
type ModelCost struct {
    AvgCostUSD   float64   // 历史平均值
    P95LatencyMs int64
}
```

**缺失**:一个将**历史成本 + 当前配置 + 剩余工作量 → 预测未来成本**的推理层。

### 设计思路:CostForecaster

```go
// 新增类型(在 cost.go 或新文件 cost_forecast.go):
type CostForecast struct {
    // 数据源:从 scorecard 的历史记录中读取
    history *ScorecardHistory
}

// Estimate returns a cost range (P50, P95) for implementing a given set of items
// under the given mode/lifecycle/agent config. It uses historical per-agent-phase
// cost data from scorecards, NOT LLM guessing.
func (f *CostForecast) Estimate(items []RoadmapItem, mode, lifecycle string) (*CostRange, error)

type CostRange struct {
    P50  CostBreakdown  // 典型情况
    P95  CostBreakdown  // 恶劣情况(考虑 rework/loop-back)
    Unit string         // "USD" | "tokens" | "agent-calls"
}

type CostBreakdown struct {
    Total         float64
    ByPhaseType   map[string]float64  // implementer: $10, reviewer: $5, qa: $2
    ByModelTier   map[string]float64  // sonnet: $8, opus: $9
    ExpectedIters  int
}
```

**CLI 命令**:

```bash
forge budget estimate --roadmap "user-auth,payment-api" --mode balanced
# → 预计成本: $15-$28 (P50-P95)
#   implementer: $8-$14
#   reviewer:    $4-$8
#   gate/qatest: $3-$6
#   预计迭代: 2-4 次

forge budget forecast --remaining 3-items --budget 50
# → 当前 budget $50
#   roadmap 剩余 3 items 预计 $12-$25
#   ✓ 预算充足,预计剩余 $25-$38
```

### 边界情况

| 场景 | 问题 | 方案 |
|------|------|------|
| **冷启动(无历史数据)** | scorecards.json 有 bootstrap prior,但那是质量分不是成本 | 使用 bootstrap 成本:Opus ~$0.1/phase,Sonnet ~$0.03/phase(硬编码参考值,标注"估算") |
| **模式切换后成本变化** | 同一 feature 在 explorer vs engineering mode 成本不同 | forecaster 接受 mode/lifecycle 参数,按 mode 过滤历史数据;无该 mode 数据时退到全局平均 + 警告 |
| **模型价格变化** | Anthropic 降价 50% 后历史数据过时 | `ModelCost` 结构中包含 `price_version`;forecaster 检测价格版本变化并应用折价系数 |
| **Rework 不确定性** | 简单的 feature 可能一次 pass,复杂的可能 rework 3 次 | P50/P95 区间覆盖典型和恶劣情况;P95 基于历史中 rework 次数最多的 feature 的成本 |
| **项目异质性** | 前端项目的 agent phase 成本 vs 后端项目差异大 | forecaster 接受 `project_type` 参数(go/node/python);区分不同技术栈的成本模型 |
| **并发执行** | `--parallel` 下 N 个 agent 同时跑,N 倍成本同时发生 | forecaster 对并行波标记为 "cost-per-wave = sum(phase costs)";P95 考虑并行波数 |

### 为什么高价值

- **从"撞墙式成本管理"到"导航式成本管理"的跃升** — Operator 在启动前就能知道要花多少钱
- **直接支持"budget-aware routing"**:如果 budget 紧张,自动降档到 sonnet 而非 opus;如果 budget 充裕,自动提档
- **Operator 体验的质变**:从"希望 budget 够"到"知道 budget 够"
- **所有数据已就绪**:scorecard 已有每个 model 的 `avg_cost_usd`;trace 已有每个 phase 的 `duration_ms`;只需一个预测模型来消费它们

### 接入代价估计

- CostForecaster 核心(读取 scorecard + 计算 P50/P95): ~200 行
- CLI 命令 `forge budget`: ~200 行
- 与 engine 的集成(budget-aware routing): ~100 行
- 现有代码冲击:零(纯附加功能)

---

## 优先级矩阵

| 方向 | 商业价值 | 技术依赖 | 接入成本 | 优先级 |
|------|---------|---------|---------|--------|
| **方向一:事件驱动 Workflow** | ★★★★★ 打开 CI/CD 集成 + 24h 自治 | 无:纯 LoopEngine 扩展 | ~700 行 | **P0** |
| **方向三:人类反馈分析** | ★★★★☆ 质量可量化 + 管理可见度 | 无:纯数据 + 报告 | ~400 行 | **P1** |
| **方向五:成本预测** | ★★★★☆ 预算可预测 + cost-aware routing | 依赖 scorecard 数据(已有) | ~500 行 | **P1** |
| **方向二:输出合并与冲突解决** | ★★★☆☆ 释放并行真正潜力 | 依赖方向一(非阻塞) | ~800 行 | **P2** |
| **方向四:确定性 Replay** | ★★★☆☆ 建立自治信任 | 无:纯数据 + CLI | ~700 行 | **P2** |

### 依赖关系

```
方向一(事件驱动) ← 独立,先做
方向三(反馈分析) ← 独立,可与方向一并行
方向五(成本预测) ← 独立,可与方向一并行
方向二(输出合并) ← 最好在方向一之后(外部事件触发并行 workflow)
方向四(确定性 Replay) ← 独立,但数据量较大,建议在存储管理策略就绪后做
```

---

## 总结:不与已有分析重叠的五个新视角

| 已有分析覆盖的方向 | 本文件覆盖的方向 |
|------------------|----------------|
| Sandbox/Runner 隔离 | **事件驱动 Workflow** (从同步到反应式) |
| Cross-Vendor Provider 抽象 | **输出合并与冲突解决** (从并行写到安全合并) |
| Learning 回灌与 Scorecard → Router | **人类反馈分析** (从二进制信号到质量可观测) |
| Discover Engine 与 Requirement | **确定性 Replay** (从黑盒到可复现调试) |
| PhaseGate 与语法级验证 | **成本预测** (从反应式到导航式) |
| 多仓组合治理 | — |
| 信号处理与优雅关闭 | — |

> **底线**:ForgeOS 的执行引擎(v2)已经非常成熟。下一个量级来自 **(1) 让它在真实事件流中自治运行**、(2) **让它知道自己的产出好不好**、(3) **让它提前知道要花多少钱**。这三个能力组合起来——事件驱动 + 质量自省 + 成本可预测——才是真正的"AI 软件工厂"的运营层(对应于基础设施层的 v3)。
