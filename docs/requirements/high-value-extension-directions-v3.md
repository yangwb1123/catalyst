# ForgeOS — 全局扫描后五个高价值扩展方向（v3）

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深扫 —— forge-core 18 Go 包 / 77 测试文件 / harness 39 模块 /  
>   `.agent/` 完整骨架（12 agent 卡 + 9 skill 卡 + 5 workflow + 10 prompt 模板）/  
>   pi-batch.py / examples / `.forge/` 运行时产物 / Sprint 1–31 完整演进 /  
>   ADR-0001~0004 / DECISIONS.md / PROJECT.md / ARCHITECTURE.md  
> **交叉验证**: 通读 40+ 篇 `docs/analysis/*.md` + 12 篇 `docs/requirements/*.md` +  
>   `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`，确保**每个方向与已有分析的核心论点不重叠**  
> **承诺**: 下面每个方向在全部 ~60 个已有方向中零覆盖或仅边缘触及  
> **纪律**: 不编写任何代码。每条方向附代码级证据 + 与已有分析的差异化证明  
> **日期**: 2026-07-09

---

## 已有 60+ 分析已覆盖的域（本文不重复）

| 维度 | 代表文档 | 方向数 |
|------|----------|--------|
| 功能引擎补齐（路由/编排/记忆/收敛/诊断/自适应装配） | `high-value-expansion-directions.md`, `expansion-production-readiness.md`, `expansion-core-five.md` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md`, `expansion-gaps-v7-novel.md` | ~10 |
| 生产可靠性（prompt QA / 信号硬化 / 环境验证 / 自愈层） | `expansion-production-readiness.md`, `expansion-directions-v6.md` | ~8 |
| 执行语义形式化（原子性/幂等性/因果一致性/版本演化） | `execution-semantic-gaps.md`, `expansion-forgeos-meta-governance.md` | ~8 |
| 二阶伴生问题（知识衰减/配置爆炸/追踪/竞争条件/数据生命周期） | `second-order-architectural-gaps.md`, `systemic-expansion-v26.md`, `edgecases-and-perf.md` | ~10 |
| 系统边界盲区（TOCTOU/持久语义/无声数据丢失/可移植性） | `uncovered-frontiers-v25.md`, `strategic-extensions-v22-v24.md` | ~10 |
| 架构盲区与多波分析 | 40+ 篇 `docs/analysis/*.md` + 8 篇 `docs/requirements/*.md` | ~60 |
| **总计已有覆盖** | | **~60 个方向** |

---

## 方向一：YAML 解析器双轨差分测试 —— 两个解析器可静默分歧，无交叉验证护栏

**类型**: 测试基础设施 · 正确性  
**优先级**: P1（回归捕获，一次遗漏可导致工作流定义损坏）  
**代码影响**: `internal/yaml2json/` · `harness/yaml2json.py` · CI

### 现状

ForgeOS 有两个独立的 YAML→JSON 解析器：

```go
// main.go:190-210 — loadWorkflow 的解析策略
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    // 1. 优先尝试 Go 原生解析器 yaml2json.Decode
    val, err := yaml2json.Decode(f)
    if err == nil {
        data, marshalErr := json.Marshal(val)
        if marshalErr == nil {
            wf, parseErr := asset.LoadWorkflowJSON(data)
            if parseErr == nil && len(wf.Phases) > 0 {
                return wf, nil  // ← Go 解析成功即返回，Python 路径从不运行
            }
        }
    }
    // 2. 回退到 Python shim
    shim := filepath.Join(repoRoot, "harness", "yaml2json.py")
    out, execErr := exec.Command("python3", shim, ymlPath).Output()
    return asset.LoadWorkflowJSON(out)
}
```

**核心问题**: Go 解析器是**主路径**，一旦它的 `Decode` 不报错（即使输出是错的），Python shim 路径**永远不会被调用**，两个解析器的输出也**从不比较**。

这曾真实发生过：Sprint 27 发现 Go 的 `consumeBlockScalar` 把 `"> "`/`"| "` 指示符拼进解码值，导致 **6/7 个真实 workflow 文件**的 `description:`/`note:` 字段被注入字面前缀。这个 bug 存活了多月，而 Python shim 的输出一直是正确的。存在一个差分测试（`TestToJSON_MatchesPythonShim`），但用的是 `t.Logf` 而非 `t.Errorf`——**测试本身永远 PASS，真正的问题从未被标记为失败**。

### 未被已有分析覆盖的证明

| 已有分析 | 与本文的差异 |
|----------|-------------|
| `configuration-surface-and-adoption.md` | 关注 YAML 配置的跨文件一致性（同名键在多处定义），不是双解析器的输出分歧 |
| `edgecases-and-perf.md` | 关注 Go 解析器自身的边界情况（block scalar 缩进/空行），不是它和 Python 的差异 |
| `eighth-wave-adr-decay.md` | 关注 ADR 知识衰退，不是 YAML 解析 |
| Sprint 27 本身 | 修了 bug、修了测试，但留下了**结构性风险**：没有 CI 检查确保两个解析器的输出逐位一致 |
| 所有 60+ 分析 | 「Go 解析器 + Python 回退无交叉验证」这个具体的测试架构缺口**零覆盖** |

### 为什么需要

1. **双解析器分歧是回归的定时炸弹** —— `internal/yaml2json/` 是纯 Go 手写解析器（从零实现，无 yaml 库）。`normalize.go`（block scalar 处理）和 `sequence.go`（序列项处理）包含复杂的缩进/折叠/换行逻辑。任何后续修改都可能引入与 Python 参考实现的微分歧，而主路径会静默使用错误的输出。

2. **当前"测试"形同虚设** —— `TestToJSON_MatchesPythonShim` 虽然做了真实比较（7 个真实文件对 PyYAML 参考输出），但 `t.Logf` 让它永远不可 FAIL。这是元层面的教训：**治理自身治理工具的测试质量**。

3. **工作流定义损坏的严重性** —— 如果 `description:` 字段又被静默损坏，agent 将收到错误的指令（如向 implementer 发送正确的任务描述）。这是 prompt 注入级别的正确性问题，不是一个"边角"。

### 建议的架构方向

- 对 `loadWorkflow` 增加**双解析器校验模式**（仅开发/CI，非生产路径）: 当 Go 解析成功后，同时也跑 Python shim，比较两个 JSON 输出是否逐位一致。如果不一致，记录差分日志并 FAIL 测试。
- 将这个校验接入 CI（`.github/workflows/forge.yml`）—— 对 `.agent/workflows/*.yml` 每个文件跑双解析器比较。
- 将 `TestToJSON_MatchesPythonShim` 从 `t.Logf` 改回真断言（已在 Sprint 27 做完），但加 CI 级强制执行（非仅单测）。

### 代价与收益

| 维度 | 收益 |
|------|------|
| 正确性 | 双解析器分歧永远不会再被静默忽略 |
| 治理可靠性 | 工作流定义损坏是最严重的 prompt 正确性问题之一，此防护直接守住 |
| 测试成本 | 对 5 个 YAML 文件跑两个解析器 + 比较 < 50ms |

---

## 方向二：风险自动检测的 `ProdTraffic` 与 `Reversible` 盲区 —— 关键风险评估路径从自动路径不可达

**类型**: 功能缺口 · 路由正确性  
**优先级**: P1（安全 override 的触发条件存在不可达路径）  
**代码影响**: `internal/risk/risk_diff.go` · `internal/risk/risk.go`

### 现状

`risk.FromChangedPaths` 是风险自动提取的唯一条目。它的诚实性边界很清晰：

```go
// risk_diff.go:33-38
func FromChangedPaths(paths []string) (Signals, []string) {
    s := Signals{Reversible: true, BlastRadius: len(paths)}
    // 设置 TouchesPayment / TouchesAuth / TouchesSecrets / TouchesMigration
    // ★ 永不设置 ProdTraffic
    // ★ Reversible 仅对 migration 设为 false
    // ★ ProdTraffic 永远 false（显式注释："inferring it from a path would be a guess"）
}
```

但在 `risk.Classify` 中，`critical` 级别的判定路径有一条：

```go
// risk.go:67-80 — criticalReason
func criticalReason(s Signals) (string, string) {
    if !s.Reversible {
        var why []string
        if s.TouchesPayment { why = append(why, "touches payment") }
        if s.BlastRadius >= largeBlastRadius { why = append(why, "large blast radius") }
        if s.ProdTraffic {
            // ★★ 这条路永远不可能从自动提取触发 ★★
            if s.TouchesMigration { why = append(why, "production migration") }
            else { why = append(why, "production traffic") }
        }
        if len(why) > 0 { return Critical, ... }
    }
}
```

**问题链条**：要到达 `critical`，条件链是 `!Reversible && (Payment | largeBlast | ProdTraffic | productionMigration)`。但在自动提取中：
- `ProdTraffic` 永远 false（显式放弃）
- `Reversible` 只在 migration 时 false（其他情况一律 true）
- `TouchesPayment` + `largeBlastRadius` 都可能触发 critical——但缺少 `ProdTraffic` 意味着 `irreversible + production traffic` 这个重要场景完全不可达

**一个具体的、无法被当前自动检测覆盖的场景**：一个开发者在 `deploy/main.go` 中修改了生产部署配置（这是生产流量代码），同时将 `config.go` 中的超时设置从 30s 改为 5s。这个改动是**不可逆的**（部署配置改了很难回滚），且命中生产流量。但：
- 路径中没有 `payment`/`auth`/`secret`/`migration` 子串 → `Classify` 判为 `medium`（"non-trivial blast radius"）
- 实际应该判为 `critical`（`irreversible + production traffic`）
- 安全 override 不触发 → reviewer 拿到 Sonnet 而非 Opus

### 未被已有分析覆盖的证明

| 已有分析 | 与本文的差异 |
|----------|-------------|
| `expansion-core-five-2026-07-01.md` 方向二「统一验证引擎」 | 关注跨语言执法一致性，不是风险分类器的覆盖度 |
| `expansion-directions-v4.md` 的「置信度感知决策引擎」 | 关注 agent 输出置信度如何影响收敛，不是路由风险信号 |
| `expansion-blind-spots-v15.md` | 关注路由策略的显式声明 vs 代码实现漂移，不是某个具体信号是否从自动路径可达 |
| 所有已有分析 | `ProdTraffic` 自动提取的盲区——这个具体的信号级缺口——**零覆盖** |

### 为什么需要

1. **安全 override 的触发条件是整个系统的最高安全护栏** —— `risk == critical` 强制 Opus，且 `safety_override` 在 `routing.go` 中被实现为不可绕过。但这个护栏的触发依赖于 `risk.Classify` 能准确设置 `Signals`。如果一个重要的触发路径从自动输入中不可达，那这个护栏对自动提取的输入就**部分失效**。

2. **`ProdTraffic` 并非完全不可猜测** —— 代码中现有的 `surface` 模式（路径子串匹配）虽然粗，但可以扩展到 `deploy/`、`prod/`、`main.go`、`handler/` 等常规生产路径。风险分类器的诚实性边界已经为此留了空间：「粗启发式但仍优于人工猜测」。

3. **代价极小** —— 在 `surface.go` 中加几行 needle 映射，引入像 `production_needles` 的标志位，将 `ProdTraffic` 从「永远 false」改为「有证据就 true」，完全保留 fail-closed 的「无证据保留 false」。不改 `Classify` 任何逻辑。

### 建议的架构方向

- 在 `risk_diff.go` 中为 `ProdTraffic` 添加一个 `productionNeedles` 启发式映射（`deploy/`, `prod/`, `main.go`, `handler/`, `router.go`, `live/`……）。保持注释诚实：「这是启发式，不是证明」。
- `FromChangedPaths` 的输出增加第三个返回值（`ProdTrafficDerived` bool），让调用者知道这是推算的而非明确声明——`forge route --diff-files` 的渲染中显示 WARN 标记。
- 保持 `Reversible` 的当前规则不变（只有 migration 能设 false），但增加一条新规则：修改生产关键路径（如 `main.go`、部署配置）且没有 rollback 机制时，可选地将 `Reversible` 设为 false。

---

## 方向三：迭代不变阶段跳过 —— evolve 循环每次迭代都全量重跑所有 phase，大部分不必要

**类型**: 性能优化 · 成本优化  
**优先级**: P1（evolve 多次迭代的累计成本可显著降低）  
**代码影响**: `internal/orchestrator/loop.go` · `cmd/forge/evolve.go`

### 现状

`LoopEngine.Run` 当前每次迭代都完整调用 `RunFrom(wf, mode, startPhase)`，从头到尾跑全部 phase：

```go
// loop.go:77-94 — runIteration 每次迭代都全量执行
func (l LoopEngine) runIteration(...) (*LoopOutcome, error) {
    // ...
    l.logf("iteration %d/%d", i, l.MaxIter)
    runErr = l.Engine.RunFrom(wf, mode, *startPhase)  // ★ 每次都全量
    // ...
    sig := l.Signals()
    if lo, done := l.checkStop(i, sig); done { return &lo, nil }
    // ...
}
```

其中 `Signals()` 包含 `RoadmapCompletion` —— 这可能是每次迭代中唯一真正变化的东西。但 planner 每次迭代还是重新拆分任务、implementer 还是重新审视整个 ROADMAP、reviewer 还是重新审查所有代码——即使上次迭代只修了 3 行代码、ROADMAP 只多了 1 个 `[x]`。

已经有了这些可用的增量信号：

| 信号 | 来源 | 更新频率 |
|------|------|---------|
| `RoadmapCompletion` | ROADMAP.md `[x]` 勾选率 | 每 iteration 变化（agent 勾选后） |
| `GatesGreen` | Harness gate exit code | 每 iteration 可能变化 |
| 文件变更集 | `git diff --name-only` | 每 iteration 唯一确定 |
| `Memory` 条目数 | memory.jsonl Append | 每 iteration 可能增加 |
| `gateLedger` | 运行时 gate 结果 | 已缓存在内存（不变者不重跑） |

**但系统从不问「这个 phase 的输入在上次迭代后变了吗？」**——它假设每次都变了，每次都全量重跑。

### 未被已有分析覆盖的证明

| 已有分析 | 与本文的差异 |
|----------|-------------|
| `edgecases-and-perf.md` 方向一「并行波 fail-fast 短路」 | 关注并行 wave 内的错误传播，不是串行迭代的 phase 跳过 |
| `expansion-core-five.md` 方向四「分岔/回滚引擎」 | 关注跨 iteration 的分岔回滚，不是「输入不变就跳过」的增量策略 |
| `uncovered-frontiers-v25` / `systemic-expansion-v26` | 关注数据生命周期和并发安全，不是迭代级别的工作量优化 |
| Sprint 10-31 中 checkpoint 粒度增强（per-phase） | checkpoint 解决的是**崩溃恢复**问题（resume 跳过已完成 phase），不是主动跳过输入不变的 phase |
| 所有已有分析 | 「基于输入变化的 phase 跳过」——**零覆盖** |

### 为什么需要

1. **evolve 循环的典型成本分布是不均匀的** —— 通常第 1 次迭代做最多工作（大幅推进 ROADMAP），后续迭代只做 micro-adjustments（修 gate 问题、调整 1-2 个 `[x]`）。但当前所有迭代的 agent 调用次数相同。对于 stable 阶段的迭代（RoadmapCompletion 从 80%→82%），花费全量 budget 重新检查所有代码是巨大的浪费。

2. **现有基础设施已经为增量判断铺好了路** —— `risk.FromChangedPaths` 可以告知本次 iteration 改了哪些文件；`gateLedger` 已经缓存了 gate 结果（供 reviewer 注入）；`memory.Load` 也在用 mtime 缓存。就差一个「phase 输入哈希 → 缓存 → 跳过」的循环了。

3. **Sprint 21 的 agent-call budget guard 是本方向的天然配套** —— `--max-agent-calls` 限制的是 agent 调用总数。如果每条 iteration 都跑满 6 个 phase × loop-back，这个上限很快用完。跳过不变的 phase 可以直接延长 evolve 的有效轮次。

### 建议的架构方向

在 LoopEngine 或 `runIteration` 中引入**增量编排模式**（非默认，opt-in）：

1. **阶段级别输入摘要**：每次迭代前计算每个 phase 的「输入哈希」—— 对 planner 来说，输入是 ROADMAP.md + memory entries 中未看到的最新条目；对 implementer 来说，输入是 planner 的 feed-forward + gate 结果 + ROADMAP 增量；对 gate phase 来说，输入是被改动的文件集。

2. **跳过不变 phase**：如果 phase 的输入 hash 与上次迭代相同，且 phase 不是 `fresh_context: true`（reviewer 需要干净 slate，不受之前 iteration 影响），则跳过该 phase（标记 `[skipped]` 到 trace）。

3. **诚实标注**：跳过不等于收敛——被跳过的 phase 仍需要在 convergence 报告中显式标注为 `skipped: unchanged`，而非假装跑了。

4. **fail-open 兜底**：当无法计算输入摘要（如第一次迭代、哈希冲突、增量信息不足）时，退回全量重跑。

---

## 方向四：收敛警告可见性提升 —— `FileDelta` / `CodeTestRatio` 警报现在仅埋在迭代日志中，易被忽视

**类型**: 可观测性 · 诚实性  
**优先级**: P2（不影响收敛判定，但影响人对收敛判定的信任）  
**代码影响**: `internal/orchestrator/loop.go` · `cmd/forge/gates.go`

### 现状

`reportConvergence` 中有两条关键的 honesty 交叉验证警报：

```go
// loop.go:221-229 — reportConvergence
func (l LoopEngine) reportConvergence(sig converge.Signals) {
    // ...
    if sig.RoadmapCompletion > 0.5 && sig.FileDelta < 0.3 {
        l.logf("  ⚠ honesty: roadmap=%.0f%% but file-change coverage=%.0f%% — agent self-report may overstate progress",
            sig.RoadmapCompletion*100, sig.FileDelta*100)
    }
    if sig.CodeTestRatio >= 0 && sig.RoadmapCompletion > 0.3 && sig.CodeTestRatio < 0.1 {
        l.logf("  ⚠ test gap: code-to-test ratio=%.0f%% — new code may lack test coverage", sig.CodeTestRatio*100)
    }
}
```

这些警报通过 `l.logf` 输出，在 `forge evolve` 中表现为多行迭代日志中的两行。对于单次 `forge run`，`runWorkflow` → `execEngine` → `reportConvergence` **完全不会触发这些警报**，因为 `runWorkflow` 调用的是 `eng.Run` 而非 `LoopEngine.Run`，`reportConvergence` 走的是 `cmd/forge/gates.go` 中的独立版本。

```go
// gates.go — reportConvergence（cmd/forge 版本）
func reportConvergence(wf asset.Workflow, root string, ...) {
    // ...
    results, met := converge.Converge(wf.Stop, gatherSignals(...))
    for _, r := range results {
        fmt.Printf("  [%s] %s — %s\n", mark(r.Met), r.Expr, r.Detail)
    }
    // ★ 没有 FileDelta / CodeTestRatio 警告 ★
}
```

也就是说：
- 在 `forge evolve` 中，这些警告混在迭代日志中——没有独立的 stdout 输出，没有独立的告警级别。
- 在 `forge run` 中，这些警告**完全不显示**——`cmd/forge` 版本的 `reportConvergence` 没有实现它们。

### 未被已有分析覆盖的证明

| 已有分析 | 与本文的差异 |
|----------|-------------|
| `expansion-direction-analysis.md` 方向四「可观测性层」 | 关注 trace.jsonl 的结构化事件，不是收敛报告的可见性 |
| `expansion-core-five.md` 方向三「实时可观测性层/流式遥测」 | 关注跨进程实时事件流，不是 stdout 报告的警告可见性 |
| `expansion-production-readiness.md` 方向二「信号硬化」 | 关注 agent 输出中信号提取的健壮性，不是引擎如何渲染收敛结果 |
| 所有已有分析 | FileDelta/CodeTestRatio 警告的可见性差异——**零覆盖** |

### 为什么需要

1. **ForgeOS 的核心卖点是诚实性** —— 系统明确区分"什么被验证了"（gate 客观信号）和"什么是 agent 自报的"（RoadmapCompletion）。FileDelta 交叉验证正是这个诚实性模型中最重要的一环。如果这个警报可以被轻易忽视，那它的存在价值就大幅降低了。

2. **`forge run` 完全看不到这是一个真正的不对称** —— Gateway 的单次使用（如 `forge run build`）是 ForgeOS 最常用的操作。在这个路径中，ROADMAP 完成度可能被 agent 虚报，而完全没有人或系统注意到。

3. **修复成本极低** —— 只需将 `logf` 升级为 `fmt.Printf`（以 `⚠` 或 `HONESTY WARNING` 前缀标记到 stdout），并在 `cmd/forge` 版本的 `reportConvergence` 中镜像 sameFileDelta/CodeTestRatio 检查。

### 建议的架构方向

- 将 `LoopEngine.reportConvergence` 中的两条警告从 `l.logf` 提升到 `fmt.Printf`（stdout），使用醒目前缀如 `⚠ HONESTY:`，使其在迭代报告的末尾清晰可见。
- 在 `cmd/forge/gates.go` 的 `reportConvergence` 中添加相同的警告逻辑，使 `forge run build` 也能看到这些交叉验证。
- 可选：将警告纳入 trace 事件（以 `kind: "honesty_warning"` 记录），使自动化工具也能消费。

---

## 方向五：并行引擎的工作流适配 —— 5 个已发运工作流全部零 `depends_on`，并行路径由测试独占

**类型**: 功能交付缺口 · 性能  
**优先级**: P1（已交付的引擎无真实工作负载，形成「架构镀金」风险）  
**代码影响**: `.agent/workflows/discover.yml` · `.agent/workflows/review.yml` ·  
  `.agent/workflows/build.yml` · `forge-core/internal/orchestrator/waves.go`

### 现状

`forge run/evolve --parallel` 经过完整实现和测试（`parallel.go` + `waves.go` + `-race`），但其激活条件是：

```go
// main.go:162-170 — parallelEnabled
func parallelEnabled(o runOpts, wf asset.Workflow, logln func(string), ctx string) bool {
    if !o.parallel { return false }
    if declaresDependsOn(wf) { return true }  // ★ 需要 workflow 声明 depends_on
    logln(ctx + ": --parallel ignored (workflow declares no depends_on) — running serially")
    return false
}
```

目前全部 5 个 workflow 的 `depends_on` 列表：

| Workflow | 声明的 depends_on | 自然可并行的 phase |
|----------|------------------|-------------------|
| `discover.yml` | **全空** | 3 个独立 phase: scan → market-research → capability-matrix（均可独立跑） |
| `design.yml` | **全空** | 2 个顺序 phase（solution-architect → proposal-generator），本质串行 |
| `review.yml` | **全空** | 4 个独立 review: security / distributed / performance / cto（均可独立跑） |
| `build.yml` | **全空** | 3 个串行段（planner → implementer → harness-gates → reviewer → qa），但 reviewer 内多个 phase 可并行 |
| `evolve.yml` | **全空** | scan → gap-analysis → roadmap-update → implement → review → evaluate，前半段可部分并行 |

最明显的并行机会：
- **discover.yml**：P1 行业研究、P2 竞品分析、P3 能力矩阵——完全独立的三个研究任务
- **review.yml**：P1 安全评审、P2 分布式评审、P3 性能/可靠性评审、P4 CTO 综合裁决——前三个彼此逻辑独立，数据不交叉
- **build.yml**：如果未来支持多 implementer（ROADMAP 中的 fan-out implementers），多个 implementer phase 可以并行

缺少 `depends_on` 意味着 `--parallel` 当前对任何真实工作流都是**静默 no-op**——引擎被执行、拓扑排序跑完、单 wave（所有 phase）串行跑。用户传了 `--parallel` 没有任何效果，但也不报错。

### 未被已有分析覆盖的证明

| 已有分析 | 与本文的差异 |
|----------|-------------|
| `expansion-core-five.md` 方向一「自适应工作流引擎 / 信号驱动编排」 | 关注工作流引擎根据信号动态调整执行路径，不是为现有 workflow 加 `depends_on` 声明 |
| `strategic-extensions-v24.md` 方向四「并行编排的默认退化链」 | 关注并行错误传播（一个 phase 失败时取消同波），不是 lacks `depends_on` 导致引擎空转 |
| ROADMAP.md 方向五「并行编排 opt-in」 | 诚实标注了「默认串行不变、并行路径今天休眠」，但**从未说明为什么已发运 workflow 不加 `depends_on`** |
| Sprint 26 的并行交付 | 交付承诺是「机制就绪，opt-in」，解释了为什么默认串行——但没解释为什么 discover.yml 和 review.yml 这种天然可并行的 workflow 也不加标注 |
| 所有已有分析 | 「已完整交付的并行引擎从未在真实工作流中被启用」——这是一个**已交付功能的采用缺口**，不是新功能提案 |

### 为什么需要

1. **这是架构镀金（gold-plating）的反面信号** —— ForgeOS 的纪律之一就是「不做镀金」。一个完整的、经过 `-race` 验证的引擎在真实工作流中使用率为零，恰好是**最值得审视的投资回报比信号**。要么它有真正的用户场景（本文论述是有的），要么它是镀金。

2. **Discover 与 Review 是天然的可并行阶段** —— Discover 的三个 phase（行业研究 → 竞品分析 → 能力矩阵）之间几乎无数据依赖，可以独立并行运行（每个 phase 向各自的 `emits:` 路径写文件）。Review 的四个评审维度同样如此。让它们在 `--parallel` 下同时运行可以将 Discover 阶段墙钟从 3× 降至 1×，Review 阶段从 4× 降至约 1.2×（P4 CTO 需要前三个的输出，所以不能完全并行）。

3. **`depends_on` 的声明成本为零** —— 这只是 YAML 中的几行声明，且 `asset.Phase.DependsOn` 已经解码。对于 discover.yml，声明 `market-research depends_on: [scan]` 和 `capability-matrix depends_on: [scan]` 即可表达「scan 先跑，后两个并行」。对于 review.yml，声明 `distributed-review depends_on: [security-review]` 等即可。未来的 `feeds_forward` 相位间依赖会自动隐含 `depends_on` 语义。

4. **这是性能收益最大的杠杆之一** —— 并行引擎已经通过测试验证（无竞态、正确排序）。现在只需声明依赖关系，就可以在不改任何引擎代码的情况下，将 Discover 和 Review 的墙钟时间减半。这与需要改引擎的优化（增量测试选择、context 缓存等）不同。

### 建议的架构方向

- 为 `discover.yml` 的 P1/P2/P3 添加 `depends_on` 声明：P1（`scan`）无依赖，P2（`market-research`）和 P3（`capability-matrix`）`depends_on: [scan]`。`stop_condition` 保持为在所有 phase 完成后评估。
- 为 `review.yml` 的 P1/P2/P3/P4 添加 `depends_on`：P1（`security-review`）无依赖，P2（`distributed-review`）`depends_on: [security-review]`，P3（`performance-reliability-review`）`depends_on: [security-review]`，P4（`executive-review`）`depends_on: [distributed-review, performance-reliability-review]`。
- 在 `build.yml` 中保持无 `depends_on`（目前 phase 链本质串行，并行化需要多 implementer 支持——那是另一方向）。
- 更新 ROADMAP.md 中的方向五诚实标注：从「并行路径今天休眠」改为「discover/review 工作流可通过 `--parallel` 并行执行」。
- 确保 `forge run`/`forge evolve` 的 narraction 在并行模式下明确打印 wave 结构（「wave 1: [scan], wave 2: [market-research, capability-matrix]」）。

---

## 优先级总览

| 方向 | 优先级 | 类别 | 一句话杠杆 |
|------|--------|------|-----------|
| ① YAML 双解析器差分 | **P1** | 测试/正确性 | 两个解析器静默分歧曾导致 6/7 工作流定义损坏，无 CI 防护 |
| ② ProdTraffic 自动检测盲区 | **P1** | 功能/安全 | `critical` 级别的一个关键触发路径从自动提取完全不可达 |
| ③ 迭代不变 phase 跳过 | **P1** | 性能/成本 | evolve 每次全量重跑所有 phase，多数 iteration 只有部分 phase 输入变化 |
| ④ 收敛警告可见性 | P2 | 可观测性 | FileDelta 交叉验证警报埋在迭代日志中，forge run 路径完全缺失 |
| ⑤ 并行引擎工作流适配 | **P1** | 功能交付 | 完整的并行引擎无真实工作流使用，discover/review 天然可并行 |

**收敛建议**：
- **若只做一件**：方向①（YAML 双解析器差分）。防止元数据损坏是治理系统的地基，且修复成本极低（CI 加一步比较）。
- **做前三件**：① + ② + ③。分别守住「元数据正确性」、「风险评估完整度」、「运行时成本效益」，且三者在架构上互不依赖，可独立推进。
- 方向④（警告可见性）可在方向③的实现中一并处理（都在 loop 的收敛报告路径上）。
- 方向⑤（并行适配）独立于其他方向，但建议放在方向③之后——因为 phase 跳过逻辑和 wave 拓扑排序在增量编排中可能有交互需要统一设计。
