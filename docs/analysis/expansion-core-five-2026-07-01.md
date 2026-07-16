# ForgeOS — 全局扫描后的高价值扩展方向（2026-07-01）

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库深扫（forge-core 13 内部包 + cmd/forge 18+ CLI 命令 + harness 26+ 模块 +  
>   全部 27+ 份已有 docs/analysis/ 交叉核对），聚焦**已有分析未充分覆盖**的结构性盲区  
> **纪律**: 不写代码；每方向标注已有覆盖以证明新颖性  
> **基线**: Sprint 26-27 全状态（真点火 multi-agent 端到端坐实、Learning loop 三维真数据落盘、  
>   parallel 模式完整交付含锁顺序契约、四维资源安全护栏、gate ledger feed-forward 闭环）

---

## 已有 27+ 份分析已覆盖的域（本文不再重复）

| 已有覆盖域 | 对应文档 |
|---|---|
| 自适应工作流 / 信号驱动编排 | `high-value-extensions.md` 方向一 |
| 闸门自省 / 元学习闭环 | `high-value-extensions.md` 方向二 |
| 增量式治理执行 / git-diff 执法 | `high-value-extensions.md` 方向三 |
| 跨项目知识联邦 / 组织学习 | `expansion-gaps-v7-novel.md` 方向一 |
| 运行时模型质量自适应 | `expansion-gaps-v7-novel.md` 方向二 |
| 多租户安全隔离 / Agent 权限模型 | `expansion-gaps-v7-novel.md` 方向四 + `high-value-perspectives-v11.md` 方向一 |
| 确定性 Replay / 调试引擎 | `expansion-gaps-v7-novel.md` 方向三（深） + `expansion-directions-v4.md` 方向四 |
| Memory 衰减 / 去重 / 可溯源 | `high-value-perspectives-v11.md` 方向四 |
| 平行引擎 fail-fast 短路 | `edgecases-and-perf.md` §1.1 + `high-value-perspectives-v11.md` 方向二 |
| 配置表面积 / 跨文件一致性 | `configuration-surface-and-adoption.md` |
| ADR 架构决策衰退审计 | `eighth-wave-adr-decay.md` |
| 长运行时数据生命周期 | `fresh-scan-strategic-expansion.md` 方向一 |
| YAML-Shim 消除 / Go-Native Asset | `fresh-scan-strategic-expansion.md` 方向二 |
| 跨 Agent Prompt 注入防护 | `expansion-directions-v6-novel-perspectives.md` 方向一 |
| 自愈层运行时 | `expansion-directions-v6-novel-perspectives.md` 方向四 |
| 架构度量趋势分析 / 早期预警 | `expansion-directions-v6-novel-perspectives.md` 方向五 |
| 收敛理论隐藏陷阱 | `edgecases-and-perf.md` §3 |
| ForgeOS 自我测试缺口 | `self-testing-and-dogfooding.md` |
| 置信度感知决策引擎 | `expansion-directions-v6-novel-perspectives.md` 方向二 |
| Growth bottlenecks / cmd/forge 膨胀 | `growth-bottlenecks-and-scalability.md` |
| Meta-governance 自身治理差距 | `expansion-forgeos-meta-governance.md` |

---

## 本文的 5 个方向（从代码级微观模式 + 真实运维痛点推导）

以下方向均从 **代码层面的具体证据** 出发，结合 `docs/analysis/` 交叉对比确认未被已有分析充分覆盖。

---

## 方向一：跨周期收敛状态机——从「无状态迭代」到「有记忆的演化轨迹」

### 代码级证据

`internal/converge/converge.go` 的 `Evaluate` 函数执行纯函数判定：`Signals` 输入 → `Result` 输出。每次迭代的收敛状态是**完全独立的快照**：

```go
func Evaluate(allOf []asset.Criterion, sig Signals) (results []Result, allMet bool) {
    allMet = len(allOf) > 0
    for _, c := range allOf {
        r := evalOne(c, sig)
        results = append(results, r)
        if !r.Met {
            allMet = false
        }
    }
    return results, allMet
}
```

这个纯函数的**每次调用都是冷启动**——不接收：
- 上一次迭代的 `Result` 历史
- 收敛趋势方向（progressing / regressing / plateauing）
- 收敛速度（每 iteration 增加的 roadmap_completion 百分比）

`LoopEngine.Run`（`loop.go:89`）在每次迭代后通过 `checkStop` 调用 `Converge`，但 `checkStop` 只返回一个 `met bool`：

```go
if _, met := converge.Converge(l.Stop, sig); met {
    return LoopOutcome{i, true, "converged"}, true
}
```

**关键缺口**：收敛判定是**硬二值**——`met || !met`。没有第三态 `"trending"`，没有 `"stuck"`，没有 `"regressing"`。

`LoopEngine` 的 `staleCount` 尝试检测停滞：

```go
func staleCount(cur, prev float64, stale int, gatesGreen, prevGatesGreen bool) int {
    if cur > prev || (!prevGatesGreen && gatesGreen) {
        return 0
    }
    return stale + 1
}
```

但它只追踪 roadmap_completion 的**纯数值增减**，完全不考虑：
1. 收敛速度的变化率（加速度/减速度）
2. 单个 criterion 的细粒度进展（gates 从 2/5 green → 4/5 green 但 roadmap 没变）
3. 跨 iteration 的 criterion 波动模式（test_pass 在 iter1 FAIL → iter2 PASS → iter3 FAIL）

`internal/converge/converge.go` 的 `Signals` 结构体有 `FileDelta` 和 `CodeTestRatio`，它们只在日志中暴露：

```go
if sig.RoadmapCompletion > 0.5 && sig.FileDelta < 0.3 {
    l.logf("  ⚠ honesty: roadmap=%.0f%% but file-change coverage=%.0f%% ...")
}
```

这些信号**被观察但未被收敛逻辑消费**。

### 高价值扩展

**跨周期收敛状态机（Tracking Convergence State Machine）**：

| 状态 | 含义 | 触发条件 | 影响 |
|---|---|---|---|
| `cold_start` | 首次迭代 | iteration=1 | 基线采集，至少运行 N 轮后才判收敛 |
| `progressing` | 正向收敛 | cur > prev × (1 + ε) | 正常继续 |
| `plateauing` | 表面停滞但内部有变化 | cur ≈ prev 但 gatesGreen 或 FileDelta 变化 | 调低收敛阈值或增加迭代 |
| `decelerating` | 速度下降 | Δcur/Δiter 递减趋势持续 M 轮 | 触发预算审查或模型升级 |
| `regressing` | 退化 | cur < prev 或关键 gate 从绿变红 | 主动触发分支建议（分岔/回滚） |
| `oscillating` | 震荡 | 同一 criterion 交替 PASS/FAIL | 标记为脆弱闸门，隔离影响 |
| `stuck` | 完全无进展 | staleCount ≥ tripwire | 当前 abort + 聚合报告 |
| `converged` | 已满足 | allOf ALL met | 正常终止 |

**不需要**修改 `converge.Evaluate` 的纯函数性质。新增 `internal/converge/tracker.go`：

```go
// Tracker 维护跨迭代的收敛轨迹，对 Evaluate 的结果做时序分析
type Tracker struct {
    history []Snapshot     // 每次迭代的收敛快照
    config  TrackerConfig  // 判定参数（ε, M, tripwire 等）
}

type Snapshot struct {
    Iteration int
    Results   []Result     // Evaluate 的输出
    Met       bool
    Signals   Signals
    Timestamp time.Time
}

// Trend 返回当前收敛趋势（纯函数，只读 history）
func (t *Tracker) Trend() ConvergenceState { ... }
```

**为什么高价值**：
- 当前硬二值收敛判定导致大量**边缘情况**：roadmap_completion=99% 永远不触发 100%（但实际已完工）、迭代多次停滞但微幅震荡不触发 stale tripwire
- 24h 长跑下，操作员需要知道**「是还在稳步推进，还是已经进入了无用循环」**——当前只能看日志人工判断
- 跨周期信息能让 `NoProgress` tripwire 从简单的 stale 计数升级为**趋势感知**：平稳收敛中一次 flat 不触发，而三次 flat + gate 不变才触发

### 边界情况

- **收敛回退/回滚**：regression 状态应区分「自然波动」（测试偶然失败）和「结构性回退」（代码被覆盖/架构被破坏）
- **长周期 criterion**：某些 criterion（架构合规性）可能 N 轮不变然后突然达标——tracker 需要区分「真停滞」和「长周期信号」
- **初始基准**：cold_start 阶段至少采集 3 个数据点才输出 Trend，避免单次噪声误判

### 已有覆盖检查

`edgecases-and-perf.md` §3 讨论了收敛理论的隐藏陷阱（真空收敛、converge 与 stop 类型不匹配），但未涉及跨迭代趋势分析。`fresh-scan-strategic-expansion.md` 方向三提到"收敛速度诊断"但作为 doctor 子命令的一个小功能，而非独立的收敛状态机。

---

## 方向二：统一验证引擎——消除三语言分裂治理债务

### 代码级证据

ForgeOS 的验证/治理逻辑分裂在 **三个语言**、**三个代码库**、**至少两套不一致的解析器**中：

| 文件 | 语言 | 行数 | 职责 | 缺陷 |
|---|---|---|---|---|
| `forge-core/cmd/forge/validate.go` | Go | **994 行** | workflow/asset/policy/scorecard 验证 | 违反本仓 500 行红线，混合 schema 验证 + 状态查询 + ADR 审计 + memory-prune |
| `harness/check.py` | Python | 499 行 | 治理完整性验证（policies/modes/workflows） | 双 YAML 解析器分裂（`parseRules` vs PyYAML） |
| `harness/arch/arch-check.mjs` | Node | ~466 行 | 架构 8 检查 | `scan.mjs` 导出图构建，`arch-check.mjs` 检查逻辑 |

代码级矛盾证据：

1. **validate.go:994 行**是整个代码库最大的文件，**大于是第二大文件（yaml2json.go:755）的 1.3 倍**，也是唯一超过 900 行的文件。它混合了：
   - `status` 子命令（`cmdStatus`）——读取 checkpoint/trace/memory 的状态查询
   - `doctor` 子命令（`cmdDoctor`）——诊断检查 + 修复建议
   - `validate` 子命令（`cmdValidate`）——workflow/policy/checkpoint/scorecard/schema 验证
   - `memory-prune` 子命令（`cmdMemoryPrune`）——memory 压缩
   - `forget` 子命令（`cmdForget`）——删除 checkpoint/memory
   - 至少 **6 个不同职责**挤在一个文件里

2. **check.py 的双 YAML 解析器问题**（`test_check.py` `test_double_yaml_parser`）:验证逻辑手写 `parseRules`，但真正的 workflow 消费者 `asset.LoadWorkflowJSON` 经 Python shim + Go JSON 解析，两条路径的分裂意味着 **check.py 认为是合法的，asset 解析可能失败**，反之亦然。

3. **arch-check.mjs 的 scan.mjs 缺失 `export-from`/`export *`/`dynamic import()` 支持**——这是已知的假阴盲区（方向三已修），但证明了三语言维护的认知负担导致 Bug 更易漏入。

### 高价值扩展

**将验证统一为一个 Go 包 `internal/validate/`**，合并 validate.go + check.py + arch-check.mjs 的治理逻辑：

```
当前:
  validate.go (Go) ────→ workflow 资产验证
  check.py    (Python) ─→ 治理完整性验证
  arch-check.mjs (Node) → 架构 8 检查
  ── 三条独立路径，分裂解析器 → 不一致

扩展:
  internal/validate/ (Go, 纯标准库)
    ├── workflow.go     ← workflow 结构校验（原 validate.go 部分）
    ├── policy.go       ← modes/policies 一致性（原 check.py）
    ├── arch.go         ← 架构 8 检查（原 arch-check.mjs）
    ├── asset.go        ← 资产完整性（原 validate.go 部分）
    ├── cross_file.go   ← 跨文件约束验证
    └── cli.go          ← thin CLI 包装（原 cmd/validate.go stub）
```

核心原则：
- **只消耗已存在的 AST**：`internal/yaml2json` 的 YAML→Go 解析 + `asset.LoadWorkflowJSON` 的 JSON→struct 解析 → validate 直接消费 `asset.Workflow`，不重复解析
- **统一解析器**：消除 check.py 的 `parseRules`——所有验证基于同一个 yaml2json 管道
- **验证器可组合**：`AssetValidator` / `PolicyValidator` / `ArchValidator` 是独立单元，`forge validate` 按需组合调用
- **跨文件约束引擎**：`checkCrossFile(repoRoot)` 验证模式间的交叉引用（target_phase 存在、model_tier 合法、on_fail 目标可到达）

**为什么高价值**：
- 消除 Python shim 后，**Python 验证器 check.py 是 forge-core 零外部依赖宣言的唯一外部语言依赖**——Go-native 化后实现真正零依赖
- 三个验证器目前对同一份数据（`.agent/workflows/*.yml`）有三套解析——一个 Bug 在三套中修复三次
- 994 行的 validate.go 本身是治理债务的**活体证据**——一个验证自身规则的系统如果自己都不遵守，治理可信度受损
- 新 contributor 只需要 Go 即可理解和修改全套验证逻辑

### 边界情况

- **迁移策略**：不能 overnight 消灭 check.py/arch-check.mjs——需要兼容期。新 validate 用 `--go-native` 标志输出与原检查相同的格式，CI 双跑对比一致性
- **解析器对等性**：Go yaml2json 需要证明其 YAML 解析覆盖度 >= Python PyYAML + Node js-yaml——训练集用本仓所有 `.yml` 文件做 fuzz
- **性能**：Go 原生解析比 shell 出 python3 快 10-100 倍，但 arch 检查的 AST 构建需要维持相同精度

### 已有覆盖检查

`fresh-scan-strategic-expansion.md` 方向二覆盖了 YAML-shim 消除（yaml2json.go Go-native），但没有覆盖**验证器本身的统一**。`growth-bottlenecks-and-scalability.md` 讨论了 cmd/forge 膨胀但没有特别指出 validate.go 是最大文件。`edgecases-and-perf.md` §5 讨论了「治理盲区 / 信号缺失」但没有从三语言分裂角度分析。

---

## 方向三：实时可观测性层——从「事后 JSONL」到「流式运维仪表」

### 代码级证据

ForgeOS 的观测体系完全是**基于文件的、事后的**：

```
trace.jsonl       → jq / scorecard 事后分析
memory.jsonl      → agent prompt 注入（迟滞 N 迭代）
checkpoint.json   → forge status（轮询读取）
```

没有任何 **实时** 可观测性：

1. **`trace.Tracer`**（`trace.go:73`）写入 `*os.File`，每次 `Emit` 是同步 `f.Write`——数据已落盘但只有事后读取路径。没有 `Subscribe()` / `Watch()` 接口供外部消费者实时消费：
   ```go
   func (t *Tracer) Emit(ev Event) error {
       t.mu.Lock()
       defer t.mu.Unlock()
       t.seq++
       ev.Seq = t.seq
       line, err := encode(ev)
       // ... -> t.w.Write(line)
   }
   ```

2. **`forge status`** 是唯一的状态查询命令，但它在 `validate.go:994` 中作为混合职责的一部分。它返回的时间点快照（checkpoint + trace 大小 + memory 大小）没有：
   - 运行中/空闲/卡死的区分（`LoopEngine` 没有暴露 `IsRunning()` / `CurrentPhase()`）
   - 收敛趋势（当前 roadmap_completion vs N 分钟前）
   - 预算消耗速率（dollars/hour 和预计剩余时间）

3. **`LoopEngine`**（`loop.go:35`）执行迭代但不暴露进度：
   ```go
   type LoopEngine struct {
       Engine    Engine
       Stop      asset.StopCondition
       Signals   func() converge.Signals
       MaxIter   int
       // ... 没有 StatusFunc / ProgressChan / TelemetrySink
   }
   ```

   `OnIteration` / `OnBeforeIteration` 回调只在迭代边界触发，`OnPhase` 只在 agent phase 干净完成后触发。**没有 "in-flight phase 正在运行"的事件。** 一个 30 秒的 phase 在 0-30 秒之间完全黑盒。

4. **`runBudget`**（`cost.go:36`）累积 `spent` 但不暴露历史记录：
   ```go
   type runBudget struct {
       mu    sync.Mutex
       spent float64
       cap   float64
   }
   ```
   没有每 phase 的花费历史、没有费用预测、没有 `remaining budget / estimated time`。

5. **`forge doctor --anomaly`** 做离线分析（checkpoint 跳跃/停滞/回退），但它跑一次就走了，不是守护进程。

### 高价值扩展

**轻型流式遥测层（Streaming Telemetry Layer）**——不引入外部依赖，用 Go 标准库的 `net/http` + JSON event stream：

```
                     ┌──────────────────────┐
   trace.Emit() ────→│  StreamingTelemetry  │───→ TUI dashboard (tcp localhost)
                     │  (new package)        │───→ forge observe CLI (--follow)
   LoopEngine ──────→│                        │───→ JSON log (rotate, structured)
   runBudget ───────→│                        │
   memory ├─────────→│                        │
   checkpoint ──────→│                        │
                     └──────────────────────┘
```

新增 `internal/telemetry/` 包：

```go
// Sink 是轻量遥测汇聚点——trace/tracer 和 loop/engine 的双向消费者，
// 不取代 trace 的持久化（trace 继续写 JSONL），而是在内存中维护一段
// 最新的运行状态快照，并通过 HTTP/SSE 暴露实时视图。
type Sink struct {
    mu       sync.RWMutex
    events   []Event          // 环状缓冲区，保留最近 1000 条
    phases   []PhaseStatus    // 当前运行的 phase 状态
    budget   BudgetStatus     // 预算消耗概览
    progress ProgressStatus   // 收敛趋势
    subs     map[string]chan Event  // SSE 订阅者
}

type PhaseStatus struct {
    Name       string
    Agent      string
    StartedAt  time.Time
    Duration   time.Duration  // 当前 phase 已运行时间
    State      string         // pending / running / completed / failed
    Iteration  int
}

type ProgressStatus struct {
    RoadmapCompletion []float64   // 最近 N 轮迭代的 roadmap 值（趋势）
    GatesGreen        bool
    Iteration         int
    StaleCount        int
    BurnRate          float64     // 预算消耗速率（USD/hour）
    ETALeft           time.Duration // 按当前 burn rate 估算剩余时间
}
```

关键设计原则：
- **零外部依赖**：Go 标准库 `net/http` + `encoding/json`
- **不改变任何现有接口**：`Sink` 是附加的观察者，不是现有接口的修改
- **环状缓冲区**：内存固定大小，永不 OOM
- **SSE 流**：`GET /events` 返回 `text/event-stream`，TUI/CI 工具消费

伴随 CLI 扩展：

| 命令 | 功能 |
|---|---|
| `forge observe` | 实时控制台仪表盘（TUI，类似 `htop` for ForgeOS） |
| `forge observe --json` | JSON 流管道（`pipe forge observe --json | jq`） |
| `forge status --watch` | 持续刷新状态（已有 status 的轮询模式） |

**为什么高价值**：
- 当前 24h 自治运行对操作员是**黑盒**：只能 SSH 进去 `cat .forge/checkpoint.json` 看最后快照——不知道当前在跑哪个 phase、跑了多久、是否卡死
- **无 dashboard 的自动化系统，运维信心永不可能升高到「敢无人值守」**——这正是 Sprint 20-22 安全护栏要解锁的场景
- 预算 burn rate 可视化让 operator 在早期就发现「这个迭代烧了 80% 预算只推进了 5% roadmap」并手动介入，而不是等 budget exhaust 强制 abort

### 边界情况

- **SSE 吞吐**：claude phase（30-60s emit 约 3-5 event）和 gate phase（秒级 emit 1 event）频率不高——单端口 SSE 轻松处理。但在 `--parallel` 下 N 个 phase 并行 emit 可能突增——环状缓冲区 + 订阅者降级（慢消费者自动断开）做保护
- **安全**：本地端口（`127.0.0.1:0`）默认不对外暴露——operator SSH 端口转发后访问。`--telemetry-bind 0.0.0.0:9090` 可选
- **——parallel 下的相位状态**：当前 phase 状态是 N 个并发 phase/all——需要显示所有并发 phase 的列表
- **进程重启**：run 不是守护进程——telemetry 进程终止后 dashboard 断开。Operator 需要 `forge observe --reconnect` 或 systemd 封装

### 已有覆盖检查

`edgecases-and-perf.md` §4 讨论了 trace 的序列化瓶颈，但没有覆盖实时观测。`expansion-directions-v6-novel-perspectives.md` 方向四（自愈层运行时）提到了运行时暴露状态的需求，但作为自愈的子功能，未专门讨论可视化仪表。`fresh-scan-strategic-expansion.md` 方向一（数据生命周期）关注存储管理而非实时消费。**流式遥测作为一个独立包未被覆盖。**

---

## 方向四：分岔/回滚引擎——从线性演进到安全探索

### 代码级证据

当前 `LoopEngine` 的演进模型是**纯线性**的：

```
iteration 1 → iteration 2 → … → iteration N → CONVERGED / FAILED / STALE
```

没有分支、没有回滚、没有并行探索多条路径：

1. **没有并行试验**：`forge evolve` 只能按一条轨迹推进。如果 iteration 5 实现了一个方案但 iteration 8 发现方向不对，iteration 5-7 的**所有工作全部丢弃**，没有从中提取可复用知识的方法。

2. **没有回滚**：`internal/persist/checkpoint.go` 的 `Save` 支持 `retain > 0` 保留历史 checkpoint（`rotateRetain`），但 `loop.go` 的 `Run` 方法从不使用它：
   ```go
   // loop.go 每次迭代调 persist.Save(path, cp, retain) — retain 从命令行传
   // 但 loop 本身没有"回滚到历史 checkpoint"的能力
   func (l LoopEngine) Run(wf asset.Workflow, mode string) (LoopOutcome, error) {
       // ...
       for i := start; i <= l.MaxIter; i++ {
           // Checkpoint 只向前写，从不往回读历史
       }
   }
   ```

3. **`internal/persist/checkpoint.go` 的 `Load` 接受 path + format**，但 `Save` 的历史版本（`path.1`, `path.2` 等）**没有任何读取路径**——写了但没地方消费：
   ```go
   // rotateRetain 保留 path.1, path.2, …, path.N
   // 但整个代码库没有一个 LoadByVersion / LoadHistory 函数
   func rotateRetain(path string, retain int) { ... }
   ```

4. **`internal/memory/memory.go` 的 `Supersedes` 字段**是唯一的知识回退机制，但它只能标记「新知识覆盖旧知识」，不能表达「这个分支失败，那个分支成功」：

   ```go
   // Supersedes 只是 topic-level 替代，没有分支/版本概念
   type Entry struct {
       Supersedes string `json:"supersedes,omitempty"`
       // 没有 BranchID / ForkPoint / ParentEntry
   }
   ```

5. **`LoopEngine` 的 `staleCount`** 在检测到停滞后会 abort，但 abort 后 operator 没有工具回溯到停滞前的稳定状态并尝试不同路径：
   ```go
   // staleOutcome 只报错，没有任何"建议回滚到 checkpoint.2"的逻辑
   func (l LoopEngine) staleOutcome(i int) LoopOutcome {
       return LoopOutcome{i, false, "no-progress tripwire (anti doom-loop)"}
   }
   ```

### 高价值扩展

**分岔/回滚引擎（Fork & Rollback Engine）**——利用已存在的 checkpoint 历史 + memory Supersedes，增加安全分支探索能力。

核心能力：

| 能力 | 描述 | 利用的现有基础设施 |
|---|---|---|
| **Fork** | 从 checkpoint.N 创建分支，新分支独立演进 | `persist.Save` + `persist.Load` + `forge evolve --resume` |
| **Rollback** | 从当前 iteration 退回之前的 checkpoint | `checkpoint.1` ~ `checkpoint.N` 历史文件 |
| **Merge** | 不同分支的 memory lessons 合并 | `memory.Append` + `Supersedes` 去重 |
| **Diff** | 比较两分支的 roadmap 完成情况和 trace | `checkpoint.json.1` vs `checkpoint.json.2` |
| **AutoGraft** | 在 stale 触发时不 abort，自动 fork + rollback | 修改 `staleOutcome` 从 abort 变为 fork |

架构示意：

```
internal/evolve/ （新包，或在 /orchestrator 中扩展）
├── fork.go          从历史 checkpoint fork 新分支
├── merge.go         合并分支 memory → 主线
├── diff.go          分支间 checkpoint/trace 对比
└── strategy.go      分支策略选择（当前 main 分支 vs fork 分支）

.forge/
├── checkpoint.json         ← 当前 main 分支
├── checkpoint.json.1      ← 历史版（已存在）
├── checkpoint.json.2      ← 历史版（已存在）
├── fork/                  ← 新：分支目录
│   ├── experiment-a/
│   │   ├── checkpoint.json
│   │   └── memory.jsonl
│   └── experiment-b/
│       ├── checkpoint.json
│       └── memory.jsonl
└── memory.jsonl           ← 当前 main 分支的 memory
```

CLI 扩展：

| 命令 | 行为 |
|---|---|
| `forge fork --from checkpoint.2` | 从 checkpoint.2 创建 `.forge/fork/<name>/` 分支 |
| `forge merge <branch>` | 将分支的 memory 合并到主线 |
| `forge diff --main --branch <name>` | 对比两分支的 checkpoint 状态 |
| `forge grow --auto-fork` | LoopEngine 在 stale 时自动 fork 而不是 abort |

**为什么高价值**：
- 当前的线性演进是 **"all or nothing"**：方向错了一次 N 轮迭代全部浪费。有 fork 后可以在方向不确定性高时（特别是探索新的架构路径）并行跑 2-3 个试验，选择收敛最快的合并
- `checkpoint.json.1` ~ `.N` 已经存在（因为 `persist.Save` 已经写了），只是**没有任何代码消费它们**——仓库只有一半的分岔基础设施
- 安全关键场景（生产迁移、支付改造）尤其需要：主分支可以继续现有的渐进优化，fork 分支尝试突破性变更，失败后主线不受影响
- 与 `parallel` 模式互补：parallel 是**同一 iteration 内的并行**；fork 是**跨 iteration 的并行探索**

### 边界情况

- **memory 冲突**：两个 fork 可能产生互相矛盾的决策（`Supersedes` 同一 topic）。merge 时需要冲突检测和解决策略（"fork 自 checkpoint.2，晚于主线 checkpoint.5 → fork 的 Supersedes 失效"）
- **状态空间爆炸**：允许无限 fork 会产生"分支碎片化"——需要最大分支数限制（`--max-forks 3`）+ 自动清理策略
- **真 agent 回滚复杂性**：rollback 只回滚 checkpoint 状态，**不能回滚已写文件**——实现层面的回滚需要 git 操作（`git checkout` 到分支时间点的 commit）或文件级快照。v1 可只做 **checkpoint 级别回滚** + 告知 operator 需要手动 git reset

### 已有覆盖检查

`five-extensions-v10-distinct.md` 方向四（检查点历史 Diff / 收敛回归浏览器）讨论了 checkpoint 历史的读取对比，但没有覆盖分支创建和合并。`edgecases-and-perf.md` §3 讨论了收敛陷阱但没有分支探索方案。`expand-directions-v6.md` 方向四（自愈层运行时）提到「自动回滚 checkpoint」，但仅限于简单回退，缺少分支能力。

---

## 方向五：跨工作流编排/管道链接——从单一步骤到工厂流水线

### 代码级证据

当前 ForgeOS 的工作流是**独立、手工触发的单元**：

```bash
forge evolve discover   # 阶段一：需求发现
forge evolve design     # 阶段二：架构设计（需人审批）
forge evolve build      # 阶段三：实现（需前序收敛）
forge evolve review     # 阶段四：安全/分布式/性能/CTO 审查
```

每次 CLI 调用都是**完全独立的进程**——不共享进程内状态，依赖 `.forge/checkpoint.json` 和 `.agent/` 文件做跨工作流通信。

代码级证据：

1. **`cmd/forge/main.go` 的 `run()` 函数** 直接 dispatch 到 `cmdRun` / `cmdEvolve`——子命令之间没有编排层：
   ```go
   func run(args []string) int {
       cmd, rest := args[0], args[1:]
       switch cmd {
       case "run":    return cmdRun(rest)
       case "evolve": return cmdEvolve(rest)
       // ...
       }
   }
   ```

2. **工作流之间没有声明式依赖**。`asset.Workflow` 没有 `NextStage` 字段来声明「本工作流收敛后自动触发 build」：
   ```go
   type Workflow struct {
       Stage  string        `json:"stage"`
       Phases []Phase       `json:"phases"`
       Loop   *LoopBody     `json:"loop"`
       Stop   StopCondition `json:"stop_condition"`
       // 没有 NextWorkflow / PipelineStage 字段
   }
   ```

3. **`converge.StopCondition.OnApproved`** 部分承担了这个角色（`on_approved.next_stage: "build"`），但只在 human_gate 的 approve 路径上触发，且只在 `reportConvergence` 中做日志叙述，没有**实际触发下一阶段**：

   ```go
   // converge.go 只报告，不编排
   func nextStageLabel(stop asset.StopCondition) string {
       if stop.OnApproved.NextStage == "" {
           return "(no next_stage declared)"
       }
       return "next_stage=" + stop.OnApproved.NextStage
   }
   ```

4. **`forge migrate --to engineering`** 是唯一一个跨工作流的状态迁移，但它是一次性的治理升级，不是持续的工作流管道。

5. **`detect.go` 的 `projectProfile`** 检测项目状态（语言/测试/CI/生命周期）并建议 workflow，但建议结果只打印到 stdout，不写入 `.agent/project.yml` 或触发任何自动执行。

### 高价值扩展

**管道编排引擎（Pipeline Orchestration Engine）**——在工作流之上增加一个薄层，自动按阶段触发工作流，传递状态和工件。

```
当前:
  User: forge evolve discover
  User: forge evolve design
  User: --approved
  User: forge evolve build

扩展:
  User: forge pipeline init
  ForgeOS: detect → auto-suggest pipeline [discover→design(approve)→build→review]
  User: forge pipeline run
  ForgeOS:
    ├── evolve discover → auto-trigger
    ├── converge? → yes → auto-trigger design
    ├── human_gate hit → notify user → wait
    ├── approved → auto-trigger build
    ├── converge? → auto-trigger review
    └── converge? → report DONE
```

架构示意：

```
internal/pipeline/ （新包）
├── pipeline.go       Pipeline 定义（有序工作流序列）
├── stage.go          单阶段编排（触发、等待、状态查询）
├── state.go          跨工作流状态持久化
└── trigger.go        自动触发逻辑（converge→next / human_gate→notify）

new CLI:
  forge pipeline list              # 列出可用管道
  forge pipeline init              # detect 项目状态 + 建议管道
  forge pipeline run [name]        # 运行完整管道
  forge pipeline pause/resume      # 手动控制
  forge pipeline status            # 当前管道进度

.agent/pipeline.yml（新资产类型）:
  stages:
    - workflow: discover
      mode: explorer
    - workflow: design
      mode: engineering
      wait_for: human_approval
    - workflow: build
      mode: engineering
      depends_on: [design]
    - workflow: review
      mode: cto
      depends_on: [build]
```

**为什么高价值**：
- 这是 **ForgeOS 从「工作流执行器」到「软件工厂」的最后一步 Architecture 跃升**——单个工作流是工厂的一台机器，管道是整条生产线
- 当前 operator 必须手动编排 4 个 `forge evolve` 命令 + 等待 human_approval——消 gcd 除这个手动环节是自治的最终形态
- `detect.go` 已经有项目 profile 检测——管道建议可以使 detect 的输出**真正被消费**，而非仅展示
- 与 `SPRINT.md` 的 "Human Approval 是最高杠杆闸门" 一致：pipeline 执行到 human_gate 后阻塞并 notify，approve 后自动继续——**不绕过人类，但消除人类的编排负担**

### 边界情况

- **失败传播**：build 阶段 FAIL → pipeline 是 stop（等待 operator 决定）还是回退到 design？需要可配置的 `on_failure` 策略
- **长时间等待**：human_gate 可能等待数小时/数天——pipeline 需要保存状态后退出，operator 批准后用 `forge pipeline resume` 继续。这与 `durable_wait`（Temporal v2+）对齐
- **并发管道**：同一仓库可以并行的不同特性分支各跑一个 pipeline 吗？——作用域必须是分支级隔离
- **与 `forge migrate` 的交互**：migrate 从 mvp→engineering 的治理升级应作为 pipeline 的一个特殊阶段，而非独立的 CLI 命令
- **detect 结果漂移**：pipeline init 时 detect 建议的 profile 可能在运行过程中变化（例如 implementer 给项目新加了测试框架）——管道编排器需要 mid-pipeline 的 re-detect 点

### 已有覆盖检查

`expansion-directions.md` 方向三（持久化人工审批）提到了「设计→构建的自动触发」，但重点在审批流程本身而非整体管道编排。`high-value-extensions.md` 方向一（自适应工作流）讨论的是**单个工作流内的动态阶段选择**，不是**工作流之间的编排**。`expansion-next-frontier.md` 提到了「端到端自动化」的概念但没有深入到架构设计。**跨工作流管道作为一个独立系统未被覆盖。**

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 | 风险等级 |
|---|---|---|---|---|
| **一 跨周期收敛状态机** | **P1** | 功能+边界 | 从硬二值收敛到趋势感知，消除停滞误判和微幅震荡漏判，提升 24h 长跑可靠性 | 低（纯新增，不改现有收敛路径） |
| **二 统一验证引擎** | **P0** | 架构债务 | 消除三语言分裂，validate.go:994 行红线违例，治理者先治己 | 中（需兼容期双跑 + fuzz 验证等价性） |
| **三 实时可观测性层** | **P1** | 功能+运维 | 从文件级事后观测到流式仪表——24h 无人值守的运维信心前提 | 低（纯新增，零外部依赖） |
| **四 分岔/回滚引擎** | **P2** | 功能 | 利用已存在但不消费的 checkpoint 历史，从线性演进到安全分支探索 | 高（真文件回滚需 git 操作） |
| **五 跨工作流管道** | **P1** | 功能 | 从「工作流执行器」到「软件工厂」的架构跃升——自治的最后一步 | 中（human_gate 长等待持久化需 v2+） |

**收敛建议（若资源有限）**：
- **必做**：方向二（统一验证引擎）——治理系统自身的最大债务，且 validate.go:994 行已经触发了 gate.mjs 的 block。**不改就永远有 gate failure。**
- **首期**：方向三（实时可观测性）——最简单的独立新增（`internal/telemetry` 零外部依赖），最大运维信心收益。方向一（跨周期收敛）紧随其后。
- **方向五（管道）与方向四（分岔）**依赖方向三的遥测基础：operator 只有在能看到实时 pipeline 状态时才信任自动管道；只有在能回滚时才敢尝试自动 fork。建议方向三→方向五→方向四的顺序。

---

> **本文所有方向均从代码级具体证据出发，与 27+ 份已有分析交叉对比确认新颖性。**  
> 不写代码——只做判断。每方向标注了架构位置、利用的现有基础设施、以及需要避免的边界情况。
