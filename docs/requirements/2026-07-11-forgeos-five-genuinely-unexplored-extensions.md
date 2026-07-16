# ForgeOS — 五个尚未被系统性覆盖的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局逐包扫描 forge-core（18 Go 包 · 140+ 源文件 · 77 测试文件 · 纯 stdlib 零依赖）  
> 2. 完整阅读 harness 全套闸门（gate.mjs / check.py / acceptance.mjs / arch-check / secret-scan / SCA / select-tests）  
> 3. 完整阅读 `.agent/` 治理骨架（5 工作流 · 12 agent 卡 · 9 skill 卡 · policies · modes · routing）  
> 4. 审阅 Sprint 1–31 完整演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`  
> 5. **差异化验证**: 对每个方向的核心关键词在已有 120+ 份分析文档（`docs/requirements/` 80+ 篇 + `docs/analysis/` 40+ 篇）中进行全文检索，确认核心论点**从未作为独立方向系统展开**  
> 6. **纪律**: 不编写任何代码。每个方向附精确代码证据、边界场景、产品价值判断

---

## 全景定位

ForgeOS 经过 31 轮 Sprint 和 120+ 份分析文档的反复深扫，编排内核、治理闭环、安全护栏和性能优化已被深度覆盖。系统目前在以下维度已达到较高成熟度：

| 维度 | 成熟度 | 最后一轮补齐 |
|------|--------|------------|
| 编排状态机 | ✅ 完整 | checkpoint phase 粒度、定向 loop-back、parallel opt-in |
| 治理执法 | ✅ 完整 | 8 项 arch-check 机器执法、secret-scan、SCA 框架 |
| 中枢旋钮 | ✅ 完整 | mode×lifecycle 三处驱动（Router + Harness + Workflow Depth） |
| 收敛信号 | ✅ 完整 | 8 个 Signals 字段全部赋值闭合 |
| 真点火韧性 | ✅ 完整 | 四维资源护栏 + 成本三维 + 真 claude 全链路坐实 |
| 学习闭环 | ✅ 完整 | scorecard 产消对接 + history-tiebreak + converge per-criterion |

但深度扫描代码后发现，有 **5 个方向至今未被已有分析作为独立方向系统展开**。它们不是「新功能的设想」，而是代码中存在完整机制但被零消费/零覆盖/零防护的结构性缺口。

---

## 方向一 · 缺位：Per-Role Memory 隔离 —— Fresh-Context 原则在记忆层存在静默旁路

> **类型**: 架构完整性 · 治理 · **优先级**: P1  
> **关键词去重验证**: `role.*isolat\|per.role\|memory.*isolat\|memory.*namespace.*role\|memory.*scope.*role\|fresh.*context.*memory\|memory.*contamin\|cross.role.*memory`  
> **在 120+ 篇已有分析中命中数**: **0 篇**

### 问题

ForgeOS 的治理宪法（AGENTS.md）有一条不可谈判的红线：**Reviewer 必须是 fresh-context 独立 Agent**，绝不能看到前序 phase 的输出、gate 裁决、或评审意见。工程对此有严密保障：

- `phaseOutputLedger` 只向 `feeds_forward` phase 的下游注入（`prompt_memory.go:266-283`）
- `reviewFindingsLedger` 只向 loop-back target phase 注入（`prompt_memory.go:316-327`）
- `review_fresh_context: true` 使 `appendFeedbackLanes` 完全跳过

**但记忆层存在一条静默旁路。** 所有 agent phase 共享同一个 `memory.jsonl` 存储，`memoryContext()` 在构建 prompt 时**无条件注入所有条目**，无论当前 phase 的角色是什么：

```go
// forge-core/cmd/forge/prompt_memory.go:159-178
func memoryContext(repoRoot, query string) []string {
    entries, err := memory.Load(memoryPath(repoRoot))  // ← 加载全部条目
    // ...
    rel := boundMemory(entries, query)                   // ← 只按时效+相关性过滤，不按角色
    // ...
    for _, e := range rel {
        // 注入所有 entry，包括 source: "implementer" 的决策
        // Reviewer 看到 implementer 写的 memory 条目
    }
}
```

`memory.Entry.Source` 字段（`memory.go:149`）已存在且记录了来源 agent 角色，但**零消费**——它在注入时仅作为辅助信息印在行末（`[source: implementer]`），从不用于过滤。这意味着：

- **Reviewer phase** 收到 implementer 上次迭代记录的 "这个 API 我决定用 REST 而不是 GraphQL"
- **Implementer phase** 收到 planner 的 memory 条目，从而可能被偏置去实现某个特定方案
- **CTO review phase** 看到前面所有 agent 的自我评估——它的裁决不再「fresh」

### 代码级证据

**证据 A：`memory.Entry` 有 Source 字段但无人消费**

```go
// forge-core/internal/memory/memory.go:138-171
type Entry struct {
    // ...
    Source  string  `json:"source,omitempty"`  // phase/agent 来源，如 "implementer"
    // ...
}
// 全仓 grep "\.Source" 的结果：只有 setter（Append 时写入）和 reader（memoryContext 印出来）
// 没有任何路径用它做过滤：
//   $ grep -rn '\.Source\|source.*filter\|filterBySource\|BySource' forge-core/ --include="*.go"
//   → memory.go:149     Source  string  (定义)
//   → prompt_memory.go:174  fmt.Fprintf(..., e.Source)  (只印出来，不过滤)
```

**证据 B：`boundMemory` 只按 recency + relevance 过滤，不看角色**

```go
// forge-core/cmd/forge/prompt_memory.go:81-130
func boundMemory(entries []memory.Entry, query string) []memory.Entry {
    // 1. 按 Iteration 降序分离最新 N 条（memoryRecencyFloor）
    // 2. 剩下的按 BM25-lite 相关性打分（prompt.Retrieve）
    // 3. 合并后按 Iteration 升序返回
    // → 完全无视 Entry.Source
}
```

**证据 C：`memoryContext` 被 `buildPrompt` 无条件调用**

```go
// forge-core/cmd/forge/prompt_context.go:368
if mc := memoryContext(repoRoot, query); len(mc) > 0 {
    lanes = append(lanes, mc...)  // ← 无例外地注入所有内存条目
}
```

### 边界场景

| 场景 | 当前行为 | 应有行为 |
|------|---------|---------|
| Implementer 记录了一条 "决定用 MySQL" 的 decision | Reviewer 在 prompt 中看到这条记录，可能放弃质疑数据库选型 | Reviewer 不应看到 implementer 的内部决策记忆（除非显式 feeds_forward） |
| Planner 记录了 "用户调研显示 80% 需要移动端优先" | 这是合理的上下文，应该对所有后续 phase 可见 | 应该保留——但需要通过显式声明（如 `share_with: [implementer, reviewer]`）而非默认全局可见 |
| 两个独立 evolve 循环在同一项目上并发运行 | A 循环的 memory 条目被 B 循环的 agent 读到，互相污染 | 应按 run_id 或 workspace 命名空间隔离 |
| Reviewer 自己产生了一条 memory 条目 | 下次迭代的 Reviewer 读到自己的旧条目，失去 fresh-context | 应默认不让 Reviewer 的 memory 条目注入未来的 Reviewer phase（除非显式 override） |

### 产品价值判断

这不是「优雅优化」——这是**治理模型的完整性缺口**。ForgeOS 花了大功夫确保 fresh-context 在 prompt 层面不被污染（`phaseOutputLedger`、`reviewFindingsLedger`、`fresh_context` flag），却在记忆层面开了一个口子——而且这个口子**不需要 agent 恶意操作**，它只是系统默认行为的自然结果。

**修复成本极低**（~1 sprint）：在 `memoryContext()` 或 `boundMemory` 中增加按角色过滤——读取 `phase.Source`（或者从上下文中获取当前 agent 角色），然后过滤掉不应跨角色的条目类型。核心是确立一条规则：

> **生产者角色 / 消费者角色 = read-writer → 可见; reader→read-writer → 不可见**

此规则与 AGENTS.md 的「reviewer fresh-context」原则一致。

---

## 方向二 · 缺位：Phase 输出内容寻址与跨迭代增量重用

> **类型**: 性能 · 运行时效率 · **优先级**: P2  
> **关键词去重验证**: `phase.*output.*hash\|content.*hash.*reuse\|artifact.*hash.*reuse\|output.*fingerprint.*reuse\|content.*address.*phase\|phase.*identity.*hash`  
> **在 120+ 篇已有分析中命中数**: **0 篇**（已有分析中的「structural fingerprinting」聚焦于检测输出是否变化，非重用它）

### 问题

`forge evolve` 循环中，每次迭代从头执行所有 phase：planner 重新生成计划、implementer 重新实现、reviewer 重新审查——即使项目状态自上次迭代以来几乎没有变化。

当前系统已有以下机制但彼此独立：
| 机制 | 作用 | 局限性 |
|------|------|--------|
| `internal/converge.Signals` 的 `RoadmapCompletion` 对比 | 知道 roadmap 完成度从多少涨到多少 | 只知道百分比，不知道「哪些项变了」 |
| `internal/risk.FromChangedPaths` | 知道哪些文件被修改 | 只读文件路径不读内容 |
| `internal/prompt/cache.go` ContextCache | 缓存**读取侧**不变上下文（ADR/AGENTS.md） | 不缓存**写入侧**产物（plan、code、review） |
| `harness/select-tests.mjs` | 增量测试选择 | advisory，永不替代全量 gate |
| `internal/orchestrator/loop.go` NoProgress tripwire | 检测多次迭代无进步时停止 | 是停止机制，不是跳过机制 |

**缺失的关键原语**：Phase 输出的**内容标识**（content identity）。当 planner 在第 N 轮和第 N+1 轮产生相同的 sprint split + acceptance criteria 时，系统没有能力识别这一点——于是照常重新调度 implementer。

这与「缓存 LLM 输出」完全不同——LLM 响应**绝不能缓存**（同一 prompt 应产生独立输出）。这里说的是**相位结构化产物的内容寻址**——planner 的 task split 是一个结构化产物（`feeds_forward` 向前投递的内容），它与「prompt → LLM 响应」之间隔了一层 agent 的自主推理。当 agent 自主决定输出和上次完全相同的计划时，下游应能识别并跳过。

### 代码级证据

**证据 A：`converge.Signals` 只有聚合度量，没有 phase 输出指纹**

```go
// forge-core/internal/converge/converge.go:39-90
type Signals struct {
    RoadmapCompletion float64    // 0.92 → 从 0.89 涨到 0.92，但不知道哪 3% 新增了
    GatesGreen        bool       // true → 全部通过，但不知道哪些 gate 的判定变了
    // ... 没有 PhaseOutputFingerprints map[string]string
    // ... 没有 PhaseOutputHashes    map[string]string
}
```

**证据 B：`phaseOutputLedger` 存储输出但无标识**

```go
// forge-core/cmd/forge/prompt_memory.go:225-250
type phaseOutputLedger struct {
    summary map[string]string // phase name → 最新输出文本（截断至 800 字符）
    order   []string          // phase names in first-seen order
    // 没有 hash map[string]string
    // 没有 version map[string]int
}
```

每次 `record()` 只覆盖文本，不保留任何可用于判断「是否和上次相同」的指纹。

**证据 C：`LoopEngine` 每次迭代从 phase 0 开始**

```go
// forge-core/internal/orchestrator/loop.go:70-90
func (e *Engine) loopIteration(ctx context.Context, wf asset.Workflow, mode string, iter int) error {
    // 每次迭代重新执行 RunFrom(phase 0)
    // ... 没有 "skipPlannerIfPlanUnchanged" 逻辑
}
```

### 实现轮廓

1. **`internal/contenthash` 新包**（~80 行，零依赖）——提供 `Fingerprint(text string) string`（归一化后 SHA256）和 `Diff(older, newer map[string]string) map[string]bool`（哪些 phase 的输出变了）。

2. **`phaseOutputLedger` 扩展**——`record()` 同时计算并存储 `Fingerprint`；暴露 `ChangedSince(phase string, prior map[string]string) bool`。

3. **`LoopEngine` 可选跳过**——新增 `--skip-unchanged-phases` flag（默认 false）：当某 phase 的 `feeds_forward` 输出指纹与上一轮完全相同时，跳过该 phase 及其下游依赖的实现，只跑 gate 验证。planner 输出变了才重新调度 implementer。

4. **诚实边界**：
   - **永不跳过 gate phase**——gate 必须每轮全量跑（这是安全基线）。
   - **永不跳过 reviewer/QA**——质量裁决层必须每轮独立运行（fresh-context 的一部分）。
   - **跳过的 phase 在 trace 中标记为 `skipped(fingerprint_match)`**，不可静默消失。
   - 短路依赖链的完整语义追踪：跳过 planner → 自动跳过 implementer（因为输入没变）→ 但 reviewer 仍跑。

### 产品价值

在 evolve 循环中，典型模式是「前 1-2 轮大量变化，后续 5-10 轮微小调整」。当前系统每轮全量重跑，墙钟和成本随迭代次数线性增长。内容寻址让 evolve 循环在后期迭代中跳过大量冗余计算——在保持相同安全基线的同时，将 24h 自治运行的效率提升 2-5 倍。

---

## 方向三 · 缺位：跨工作流管道状态机 —— `next_stage` 声明但零消费

> **类型**: 功能缺口 · 管线完整性 · **优先级**: P1  
> **关键词去重验证**: `next_stage.*consum\|workflow.*chain\|pipeline.*state.*machine.*workflow\|auto.*chain.*workflow\|workflow.*sequence.*orchestr`  
> **在 120+ 篇已有分析中命中数**: **0 篇**（已有分析中的「跨工作流管线 DAG」聚焦于多仓库编排，非单项目内 workflow 自动串联）

### 问题

ForgeOS 的脊柱是 `Discover → Design → REVIEW → Build → Evolve`。每个阶段对应一个 YAML 工作流文件，每个工作流在 `on_approved.next_stage` 字段中声明下一阶段：

```yaml
# .agent/workflows/discover.yml:84
on_approved:
  next_stage: design     # 探索达标 → 交给设计

# .agent/workflows/design.yml:69
on_approved:
  next_stage: review     # 设计获批 → 进入评审

# .agent/workflows/review.yml:140
on_approved:
  next_stage: build      # 评审通过 → 开始构建

# .agent/workflows/build.yml:116
on_approved:
  next_stage: evolve     # 构建完毕 → 持续演化
```

**但 `next_stage` 字段在 Go 运行时中零消费。** 引擎从不读取它、不验证它、不基于它做任何编排决策。当前工作流之间的转场全靠用户手动执行：

```
# 用户手动操作链：
$ forge run discover --mode engineering
  → ... wait for completion ...
$ forge run design --mode engineering
  → ... wait for human approval ...
$ forge run review --mode engineering
  → ... wait for approval ...
$ forge run build --mode engineering
  → ... wait for converge ...
$ forge evolve build --max-iter 10
```

### 代码级证据

**证据 A：`OnApproved.NextStage` 被解析但从不消费**

```go
// forge-core/internal/asset/asset.go:225-230
type OnApproved struct {
    NextStage string   `json:"next_stage"`          // 下一阶段名称
    Emit      []string `json:"emit,omitempty"`      // 批准后产出的文件
}

// 全仓 grep "NextStage" 的结果：
//   $ grep -rn "NextStage\|next_stage" forge-core/ --include="*.go" | grep -v "_test.go"
//   → asset/asset.go:228     NextStage string        （定义）
//   → asset/asset_test.go:319 "next_stage":"build"    （测试）
//   → main.go:395-436 叙述性打印 "next_stage=build"   （只叙述，不执行）
//   → gates.go 从 stop.OnApproved.NextStage 读取后只用来渲染 report 行
//   → 没有任何地方：forge run next_stage || 自动 cd 到下一个 workflow
```

**证据 B：main.go 的 dispatch 是单命令入口**

```go
// forge-core/cmd/forge/main.go:54-68
var subcommands = map[string]func([]string) int{
    "run":    cmdRun,      // 接受一个 workflow 名称作为参数
    "evolve": cmdEvolve,   // 接受一个 workflow 名称作为参数
    // ...
    // 没有 "pipeline" 或 "chain" 子命令
    // 没有 "forge run --chain" flag
}
```

`cmdRun` 只执行一个 workflow 的一次 pass（`forge run build`），完成后即退出。它不检查该 workflow 是否声明了 `next_stage`，不自动推进到下一阶段。

**证据 C：human_gate 的 report 里提到 `next_stage` 但只作为信息展示**

```go
// forge-core/cmd/forge/gates.go:431-436
func (r *runReport) nextStage() string {
    if r.stop.OnApproved == nil || r.stop.OnApproved.NextStage == "" {
        return "(no next_stage declared)"
    }
    return "next_stage=" + r.stop.OnApproved.NextStage
}
// 该值仅在终端输出中出现，从未被编程方式消费
```

### 边界场景

| 场景 | 当前行为 | 应有行为 |
|------|---------|---------|
| design.yml 的 human_gate 被批准，输出 `next_stage=review` | 进程退出，用户需手动执行 `forge run review` | `forge run design --chain` 自动调用 `forge run review` |
| 自动链中某个 workflow 失败（gate 红/agent 错误） | 链中断，上游 `forge run` 已退出，无错误传播 | 链应自动中断，输出结构化错误报告，标明哪一步失败 |
| discover.yml 跑完但 `requirement_confidence < 80`（不达标） | 进程退出，`converge` 报告 NOT MET | 链不应推进到下一阶段，因为 stop condition 未满足 |
| 用户想跳过某个阶段（如 explorer mode 下跳 review） | 需手动决定不跑 `forge run review` | 链应感知 mode 并遵循 mode gating 自动跳过 |

### 产品价值

这是 ForgeOS 「自治」愿景最大的单项功能缺口。系统声称是「24h 无人值守软件工厂」，但完整的 Idea→Production 脊柱仍需用户手动串联 5 个独立命令。`next_stage` 声明已在 YAML 中存在（作为设计意图的记录），但缺乏执行引擎来兑现这个声明。

实现一个 `forge run --chain`（或 `forge pipeline` 子命令）可以在不破坏现有单命令行为的前提下，使脊柱实现真正的端到端自动化。关键设计约束：

- **显式 opt-in**：`forge run discover`（无 flag）保持现有行为——只跑一个 workflow，然后退出。`forge run discover --chain` 在 workflow 收敛后自动解析 `next_stage` 并推进。
- **收敛条件检查**：只有 stop condition MET 时才推进（human_gate 的 `approved`、conjunction 的 `all_of` 全达标）。NOT MET 时输出结构化失败报告并 exit 1。
- **mode gating 感知**：如果 mode=explorer 且 `ReviewDepth=skip`，自动跳过 review.yml。
- **阶段标记持久化**：`.forge/discover.approved` + `.forge/discover.converged` 标记，使链可在任意中断点恢复。

---

## 方向四 · 缺位：ForgeOS 自身资源自保 —— 单进程运行时缺乏自我资源约束

> **类型**: 运维可靠性 · **优先级**: P2  
> **关键词去重验证**: `self.*protect\|disk.*growth\|trace.*rotat\|memory.*disk.*unbound\|file.*growth.*forge\|self.*resource\|forge.*disk\|runtime.*self.*guard`  
> **在 120+ 篇已有分析中命中数**: **0 篇独立方向**（已有分析中的「admission control」聚焦于**并发 forge 进程间的互斥**与文件锁，非本进程的自保）

### 问题

ForgeOS 为 agent 进程设置了四维资源护栏：

| 维度 | 保护机制 | 保护对象 |
|------|---------|---------|
| 深度 | `MaxDepth` / `FORGE_AGENT_DEPTH` | 防止 agent fork-bomb |
| 数量 | `MaxAgentCalls` / `MaxLoopBack` | 防止 agent 无限 spawn |
| 时间 | `Timeout` | 防止 agent 卡死 |
| 内存 | `MaxOutputBytes` | 防止 agent 输出 OOM |

**但 ForgeOS 对自己没有任何资源约束。** 一个运行 24h 的 `forge evolve` 会产生：

1. **`trace.jsonl` 无限增长**——每 iteration 写 ~5-15 行事件，24h 数十次迭代后轻松达到数 MB → 数百 MB
2. **`memory.jsonl` 无限增长**——每次迭代 append 一条 entry，随 evolve 时长线性增长（已修内存 cap `memoryCap=32`，但仅限**注入 prompt 时**的截断，**磁盘文件无限增长**）
3. **`.forge/` 目录无清理策略**——`checkpoint.json` 每次迭代重写但保留单文件；没有旧 checkpoints 归档
4. **`scorecards.json` 重写但无历史**——每次 `scorecard-update.mjs` 全量重写，`LoadScorecards` 读最新的，旧数据永久丢失

### 代码级证据

**证据 A：`trace.jsonl` append-only，无 rollover**

```go
// forge-core/internal/trace/trace.go:120-140
func (t *Tracer) Emit(e Event) error {
    // ...
    if _, err := f.Write(append(line, '\n')); err != nil {
        // 无限追加，无文件大小检查，无 rotate
    }
}
// forge run 结束后 trace 文件被保留，下一次 run 继续追加（如果有 --resume）
// 但没有任何机制限制单个 trace 文件的大小或事件数量
```

无 `maxTraceSize`、无 `maxTraceEvents`、无 `rotate` 配置、无自动归档策略。

**证据 B：`memory.jsonl` 磁盘文件无界**

```go
// forge-core/internal/memory/memory.go:185-208
func Append(path string, e Entry) error {
    // 每次 append 一行到 memory.jsonl，无上限
    // memoryCap=32 控制注入 prompt 的条目数，但文件中存了成百上千条历史
    // 文件大小：每条 ~200-500 字节 × 迭代次数 → 100 次迭代 ≈ ~50KB（尚可），但 24h 数月跑下来轻松兆级
}
```

**证据 C：checkpoint 单文件覆盖，无历史**

```go
// forge-core/internal/persist/checkpoint.go:75-95
func Save(path string, cp Checkpoint) error {
    // 写入 tmp → rename 覆盖
    // 旧 checkpoint 数据被覆盖，无备份
    // 如果一次 crash 前 checkpoint 已损坏（极少情况），resume 时没有任何前一版本的 fallback
}
```

**证据 D：`cmd/forge/main.go` 无任何资源自检**

```go
// forge-core/cmd/forge/main.go:55-68
// 启动时检查：FORGE_AGENT_DEPTH ✓（防递归）
// 启动时检查：claude 在 PATH ✓（在 engine_build.go 中）
// 启动时检查：python3 yaml2json ✓（在 yaml2json.go 中）
// 启动时检查：.forge/ 磁盘剩余空间？✗
// 启动时检查：memory.jsonl 大小？✗
// 启动时检查：trace.jsonl 大小？✗
// 启动时检查：.forge/ 目录是否来自旧版 forge-core？✗
```

### 边界场景

| 场景 | 当前行为 | 风险 |
|------|---------|------|
| 24h evolve 跑了 200 迭代，trace.jsonl 达到 500MB | 正常写入，无告警 | 磁盘满 → gate/check/accept 子进程无法写输出 → 假 FAIL |
| memory.jsonl 积累 5 年历史（~150KB/月 ≈ 9MB） | Load 每次重新解析整个文件 | 启动变慢（O(n) 解析），无性能退化告警 |
| checkpoint.json 因硬件故障写损坏 | 下次 resume 读到损坏 JSON → error | 用户失去恢复点，整次 run 需从头重放 |
| 升级 forge-core 后 checkpoint 格式变了 | `FormatVersion` 字段存在，但当版本不匹配时 | 当前无自动迁移逻辑——旧 checkpoint 被静默丢弃或解析出错误状态 |

### 产品价值

这不是「优雅运维」——这是**系统自身的保护性退化**（defensive degradation）能力。一个 24h 无人值守系统最危险的不是「出错」，而是「慢性地、无告警地走向资源耗尽」。ForgeOS 已经为 agent 建立了完善的资源护栏，但对自己的运行时产物没有任何保护。

最小可行实现（~1 sprint）：
- `forge preflight` 检查 `.forge/` 磁盘空间、trace 大小、memory 大小，输出结构化的健康报告
- `forge doctor --self` 检查运行时产物的一致性（checkpoint 是否可解析、trace 是否完整、memory 是否有损坏条目）
- 可选的 `--max-trace-size` / `--max-memory-entries` 配置，超限时自动 rotate（trace 归档压缩、memory 修剪最旧条目）

---

## 方向五 · 缺位：Agent CLI 版本兼容性契约 —— `claude --model` 下推时缺少版本验证

> **类型**: 运行时可靠性 · 运维 · **优先级**: P2  
> **关键词去重验证**: `agent.*version.*compat\|CLI.*version.*check\|claude.*version\|model.*map.*version\|executor.*version\|command.*compat`  
> **在 120+ 篇已有分析中命中数**: **0 篇**（已有分析中的「agent contract version」聚焦于机读裁决 token 版本化，非 CLI 二进制版本）

### 问题

ForgeOS 通过 `CommandExecutor` 将模型路由决策下推给真实的 Agent CLI：

```go
// forge-core/internal/routing/routing.go:190-200
var ModelMap = map[string]map[string]string{
    "anthropic": {
        Haiku:  "claude-sonnet-4-haiku",
        Sonnet: "claude-sonnet-4",
        Opus:   "claude-opus-4",
    },
}

// engine_build.go: 路由决定 tier → 映射到 model name → 传递给 claude --model <name>
```

**但系统从不验证已安装的 `claude` CLI 版本是否支持这些模型名。** 当 Anthropic 发布 `claude-sonnet-5` 并弃用 `claude-sonnet-4` 时：

1. `routing.ModelMap` 中的 `Sonnet → "claude-sonnet-4"` 可能已过时
2. `claude --model claude-sonnet-4` 可能返回 error（模型已弃用）或静默降级到更便宜的模型
3. 用户无任何告警——`forge run` 输出显示 "phase X → tier sonnet → model claude-sonnet-4"，但实际跑的可能是 haiku

### 代码级证据

**证据 A：`CommandExecutor.Build` 只组装 argv，不验证模型名**

```go
// forge-core/internal/orchestrator/command_executor.go:165-190
// Build 产生 argv，如 ["claude", "-p", prompt, "--model", "claude-sonnet-4"]
// 但 claude 是否理解这个 --model 值，Executor 从不检查
// exit 非零时 classifyRunErr 分类错误类型，但不判断 "wrong model name"
```

**证据 B：`ResolveModel` 无版本上下文**

```go
// forge-core/internal/routing/routing.go:203-215
func ResolveModel(provider, tier string) string {
    // 纯静态映射表
    // 不查询 claude 支持的模型列表
    // 不检查 claude 版本
}
```

**证据 C：`--agent-cmd claude --version` 从未被调用**

```bash
# 全仓搜索 "claude.*version\|--version\|agent.*version.*check"
$ grep -rn "claude.*version\|agent.*version.*check\|cmd.*version" forge-core/ --include="*.go"
# → 零命中（除了 forgeVersion 自身）
```

**证据 D：安装后的 CLI 与 routing 假设之间存在隐式契约**

```
# 用户有 claude CLI 4.0（默认模型 claude-sonnet-4-20250514）
# ForgeOS routing 认为 Sonnet → "claude-sonnet-4"
# 实际 claude 4.0 理解的模型名是 "claude-sonnet-4-20250514"
# 可能工作（模糊匹配），也可能不工作（精确匹配）
# 没有任何一方验证这个假设
```

### 边界场景

| 场景 | 当前行为 | 风险 |
|------|---------|------|
| claude CLI 升级到 v5，`claude-sonnet-4` 被弃用 | `forge run` 路由到 sonnet → `claude --model claude-sonnet-4` → claude 报错或静默回退 | Agent 以低档模型运行，产出质量不可控 |
| 用户安装了 `claude-code`（旧版，不支持 `--model` flag） | CommandExecutor 传 `--model claude-sonnet-4` → claude 不识此 flag → 报错 | 整次 run 从 phase 0 就失败 |
| Anthropic 发布 `claude-sonnet-4-20250701`（新快照） | ModelMap 还是 "claude-sonnet-4"（模糊名可能解析到新快照） | 无法精确控制模型快照版本 |
| 用户通过 `--agent-cmd` 使用了非 claude 的 CLI（如 Codex） | ModelMap 假设 anthropic 格式的模型名，传给 Codex 完全不认识 | 模型路由逻辑完全失效 |

### 产品价值

ForgeOS 的「模型路由」是其核心差异化能力之一。但如果路由决策（应该用 Opus）无法可靠地转化为 CLI 实际执行的模型（实际用了 Sonnet），那么整个路由系统的价值就归零了。

目前这个契约是隐式的——靠 claude CLI 的向后兼容性和模糊匹配来维持。一个真实的自治系统需要显式验证：

1. **`forge preflight` 阶段检测**：`claude --version` + `claude --model claude-sonnet-4 --dry-run` 测试模型名可用性
2. **`routing.ModelMap` 版本化**：增加 `min_claude_version` 字段，forge 启动时验证已安装版本
3. **模型名验证接口**：`AgentExecutor` 接口加可选 `ValidateModel(model string) error` 方法，`CommandExecutor` 实现时调用 `claude --model <name> --dry-run`
4. **诚实降级**：当模型名无法验证时，不静默 fallback——而是报告 `route: model "claude-sonnet-4" not verified by claude CLI — run preflight to validate`，让用户明确知情

### 实现影响

- 无新依赖：`exec.Command("claude", "--version")` 和 `exec.Command("claude", "--model", name, "--dry-run")` 都是标准库操作
- 低成本：模型名验证是一个 start-up check，不引入额外运行时延迟
- 向后兼容：验证失败不会阻断 run（fail-open），但会在 trace 中记录 `model_unverified` 事件

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 | 预估 Sprint |
|------|--------|------|-----------|------------|
| **一 · Per-Role Memory 隔离** | **P1** | 治理完整性 | 治理宪法在记忆层存在静默旁路——修复成本极低（~1 sprint），伤害极高（知识污染静默不可见） | ~1 |
| **三 · 跨工作流管道状态机** | **P1** | 功能 | `next_stage` 已声明但零消费——ForgeOS 自治愿景最大的单项功能缺口，兑现已有设计意图 | ~2 |
| **五 · Agent CLI 版本兼容性** | **P2** | 运维可靠性 | 核心路由能力依赖隐式 CLI 契约——验证成本极低（几行 `exec.Command`），风险暴露面大 | ~1 |
| **二 · Phase 输出内容寻址与增量重用** | P2 | 性能 | evolve 循环后期可跳过大量冗余计算——24h 运行效率提升 2-5 倍，安全基线不受影响 | ~2 |
| **四 · ForgeOS 自身资源自保** | P2 | 运维可靠性 | 系统为 agent 建了四维护栏，但对自己零防护——慢性资源耗尽是最危险的无告警失败模式 | ~1 |

**如果只做一个**：方向一（Per-Role Memory Isolation）——成本最低（只加几行 source-based 过滤逻辑），伤害最高（fresh-context 旁路意味着 reviewer 的白花钱/假独立随时可能不被察觉地发生），且已有完整的数据结构支持（`Entry.Source` 字段就绪、`boundMemory` 已有过滤骨架）。

**做前三件（P1 × 2 + P2）**：方向一 + 三 + 五——分别解决「治理可信度尾闾」、「自治脊柱完整串联」、「路由决策真实执行」三个垂直缺口，构成从「能够自治」到「可信地自治」的跨越。

方向二和四在基础设施就绪后（方向一和五落地后）自然有更准确的优先级判断——方向二的收益依赖 evolve 循环的实际迭代数据分析，方向四的紧迫性依赖真实部署环境中磁盘使用情况的监控数据。两者都不应在缺少数据支撑时「镀金式」实现。

---

## 附录：未被选为独立方向的边缘发现

以下缺口在扫描中发现但最终未被选为独立方向，原因如下：

| 发现 | 不选为方向的原因 |
|------|----------------|
| `internal/yamlpath` 包零消费 | 已作为独立方向覆盖于 `2026-07-11-forgeos-five-structural-capillary-gaps.md` 方向一 |
| `internal/routing` 多维评分器零用于 agent 执行路径 | 已覆盖于 `five-code-grounded-architectural-gaps-2026-07-11.md` 方向一 |
| 双解析器分裂（Python vs Go yaml） | 已覆盖于 `second-order-architectural-gaps.md` 方向二 |
| `forge run` 与 `forge evolve` 各自独立的 CLI 入口 | 方向三解决了其根本原因——工作流间缺乏自动串联 |
| 全局 `sync.Map` 缓存跨项目碰撞 | 已在 `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 中分析 |
| CLI 缺少 `--executor command --agent-cmd echo` 的 CI 测试 | 这是实现细节而非架构方向——在方向五的 CLI 版本验证框架建立后自然可测 |
| `internal/doctor` 的 anomaly 检测未接入 orchestration 循环 | 属于「现有功能的增量接线」而非独立方向；价值受限于先有收敛曲线数据积累 |

---

*扫描范围: forge-core 18 Go 包 · harness 41 模块 · .agent 完整骨架 · .forge/ 运行时产物 · pi-batch.py  
文档计量: docs/requirements/ 80+ 篇 + docs/analysis/ 40+ 篇 + docs/*.md + .agent/*.md + ADR × 4  
扫描日期: 2026-07-11*
