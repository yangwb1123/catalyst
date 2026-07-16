# ForgeOS — 战略扩展方向 v5：基于全量源码扫描的 5 个高价值方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局扫描 forge-core (Go ~7.3k LOC 非测试) + harness (Node/Python ~10.5k LOC)  
> **原则**: 不编造缺口，只标注已验证的问题；每个方向给出"为什么需要"和"如果现在不做，什么时候会出问题"  
> **当前状态**: v2 forge-core 已落地 13 包、纯 Go 标准库零依赖、CLI 完整; 真 claude 闭环已坐实  
> **前置阅读**: `BOOTSTRAP` → `.agent/` → `docs/analysis/` → 本文件

---

## 总览

| 优先级 | 方向 | 核心价值 | 预估改动量 | 依赖 |
|--------|------|---------|-----------|------|
| 🔴 P0 | **语义上下文引擎 + RAG Pipeline** | 从"注入所有内容"到"注入相关上下文"，支撑项目规模 10× 增长 | 3-4 sprints | 外部向量库 (Qdrant/PGvector) + 嵌入模型 |
| 🔴 P0 | **收敛独立验证 (Convergence Cross-Validation)** | 防止自我报告偏差导致的"假收敛"——当前最危险的系统性漏洞 | 1-2 sprints | 无外部依赖 |
| 🟠 P1 | **运行时依赖净化 + 冷启动加速** | 消除 Python YAML shim 违反零依赖承诺 + 优化每次 workflow 加载 ~100ms | 1 sprint | 无外部依赖 |
| 🟠 P1 | **并行模式 Resume + 波内提前取消** | 当前 `--parallel --resume` 回放所有已计费 phase，波内失败浪费 ~$2.2/波 | 1-2 sprints | 方向一的 Context 传播 (已就绪) |
| 🟡 P2 | **Mode 感知 Scorecard + Memory 置信度衰减** | 跨 mode 数据污染导致路由决策劣化；记忆污染导致错误自我强化 | 2 sprints | 无外部依赖 |

---

## 方向一 (P0): 语义上下文引擎 + RAG Pipeline

### 现状

`internal/prompt/retrieve.go` 实现了 BM25-lite 关键词检索：对每个 query 计算 TF · IDF-lite，按词重叠度排名。
这是 Go 标准库纯实现，零外部依赖。

**已经验证的限制**（代码级证据）:

1. **语义盲区**：`tokenize` 只做 lowercase + 标点分割，不做词干化/词形还原。"car" 匹配不了 "vehicles"；
   "implement" 匹配不了 "implementation"。AGENTS.md 的 "禁 God Object" 约束，如果 query 用 "god class"
   而非 "god object" 就检索不到——尽管指的是同一个概念。

2. **上下文窗口固定**：`adrTopK = 3` 硬编码在 `prompt.go`：
   ```go
   const adrTopK = 3
   ```
   无论项目有 5 个 ADR 还是 50 个 ADR，始终返回 top-3。当项目 ADR 池增长到 20+ 时，可能有 4-5 个同时相关的
   ADR，但检索器强制截断到 3 个——丢失第 4-5 个的相关上下文。

3. **全 ADR 标题注入兜底**：`Gather` (prompt.go) 在没有 ContextCache 时注入 **全部 ADR 标题**：
   ```go
   func relevantADRs(repoRoot string) []string { ... } // 全部注入，不做筛选
   ```
   这意味着对所有非 cache 路径（首 phase）、以及无 query 匹配退路，prompt 包含的 ADR 行数与 ADR 总数相等。
   10 ADR → 10 行注入，100 ADR → 100 行注入。prompt token 成本线性增长。

4. **只能检索标题，不能检索全文**：`adrDocs` 只取 ADR 文件的第一行（title），而非全文。ADR 本身的详细论证
   （Why、Decision、Consequences）完全不进入检索范围。agent 只知道 ADR 标题存在，不知道它说了什么。

### 影响量化

| 场景 | 当前行为 | 长期问题 |
|------|---------|---------|
| 项目 5 ADR | 3 条相关注入 + 5 条标题 = 8 行 ADR 内容 | 可接受，全部进得去 context window |
| 项目 20 ADR | 3 条相关注入 + 20 条标题 = 23 行；可能漏掉第 4 条相关 ADR | 语义盲区 -> 遗漏架构约束 |
| 项目 50 ADR | 3 条相关注入 + 50 条标题 = 53 行；只占 ~15% 相关 ADR；context 膨胀 | agent 花 token 读无关内容 |
| 项目 100 ADR | 3 条相关 + 100 条标题 = 103 行；多轮后 prompt 超限 | 系统性上下文溢出 |

### 建议的演进路径

**Phase A (Pragmatic, 1 sprint)**:
- 将 `adrTopK` 从硬编码改为可配置（通过 `project.yml` 或 `policy.yml` 的 `adr_top_k: 5`）
- 升级 `retrieveADRBullets` 从检索标题变为检索 ADR 全文（读取文件的正文部分而非仅第一行）
- 为 ADR 检索增加 `confidence` 阈值：BM25 分数低于阈值的不注入——避免"噪声匹配"

**Phase B (Semantic, 2-3 sprints)**:
- 引入向量嵌入服务（作为外部插件，不破坏 forge-core 零依赖），通过 subprocess 调用 Python 或通过 HTTP 调用本地嵌入模型
- 所有 ADR + AGENTS 约束 + 项目策略文档预先向量化，存入 `.forge/adr_vectors.jsonl`（纯 JSON，零外部数据库）
- `Retrieve` 升级为混合检索：BM25 候选 → 向量重排序 → top-K

### 风险

- **高**：嵌入模型需要外部推理运行时（违反 forge-core 零依赖），必须作为可选插件而非核心依赖
- **中**：纯 Go 实现的嵌入模型（如 `intfloat/e5-small` 的 ONNX 导出）可通过 `github.com/ollama/ollama` 等引擎调用
- **低**：Phase A 完全零外部依赖，可立即执行

### 不做会怎样

项目超过 15-20 个 ADR 后，prompt 中的 ADR 上下文要么太全（撑爆窗口）、要么太稀（漏掉关键约束）。
agent 的架构决策一致性会退化到"无 ADR 感知"水平。**触发时机**：任意被治理项目达到 ~20 ADR 时（预计 v3 中后期）。

---

## 方向二 (P0): 收敛独立验证 (Convergence Cross-Validation)

### 现状

`internal/converge/converge.go` 的 `RoadmapCompletion` 完全信任 agent 的自报告：

```go
func RoadmapCompletion(markdown string) float64 {
    done, total := 0, 0
    for _, line := range strings.Split(markdown, "\n") {
        switch t := strings.TrimSpace(line); {
        case strings.HasPrefix(t, "- [x]"), strings.HasPrefix(t, "- [X]"):
            done++; total++
        case strings.HasPrefix(t, "- [ ]"), strings.HasPrefix(t, "- [~]"):
            total++
        }
    }
    // ...
}
```

**已经验证的漏洞**（Sprint 25 真 claude 运行暴露 + 代码级证据）:

1. **机制层揭示**：Sprint 25 真 claude 运行中，implementer（`acceptEdits` 无 Bash）无法运行 `node --test`，于是诚实拒绝勾选 ROADMAP `[x]`——但反过来也说明：**一个选择了勾选但不验证的 agent 可以谎报完成**。

2. **`GatesGreen` 不保证代码交付**：ROADMAP 条目可能被勾选但对应代码没有被写——gate 通过的代码可能只是重构/测试修改，而非 ROADMAP 承诺的功能。

3. **无代码/测试比率检查**：收敛判定中没有任何信号检查"新增代码中测试的比例"（`CodeTestRatio` 已在 `converge.Signals` 声明但未用于收敛判定）。

4. **测试计数下降不被察觉**：Sprint 6 修复了"零匹配 glob 假 PASS"（`runCountedTest` 新增 `count > 0` 检查），但如果测试从 15 个变成 12 个（被误删），`test_pass` 仍可能是 PASS——只要剩余 12 个全绿。测试计数趋势没有纳入收敛。

### 影响量化

| 场景 | 当前行为 | 真实情况 | 风险 |
|------|---------|---------|------|
| Agent 勾了 5 个 [x] 但只实现了 3 个功能 | Roadmap=100%，收敛 | 实际交付 60% | **假收敛——产品决策基于错误进度** |
| Agent 新增 500 行代码 + 0 行测试，gate 全绿 | 收敛，交付通过 | 无测试覆盖 | **技术债务，稍后被发现为回归** |
| 测试从 25 个删到 15 个，剩余 15 个全绿 | test_pass=PASS | 10 个测试丢失 | **测试退化的无声积累** |
| Roadmap 从 80%→80%（忘记勾选），但有代码提交 | staleCount++，deadloop 误判 | 实际有进展 | **提前终止有价值的迭代** |

### 建议的演进路径

**Phase A (1 sprint)**:
- **FileDelta 独立验证**：从 `git diff --name-only` 计算"ROADMAP 条目对应的文件变更"，此信号已在 `converge.Signals.FileDelta` 但未纳入收敛判定。改为：如果 `FileDelta < RoadmapCompletion * 0.5`（声称 100% 完成但只有 30% 的文件有变动），收敛报告中标 WARNING。
- **CodeTestRatio 纳入收敛报告**：当 `CodeTestRatio < 0.1`（新增 1000 行代码但测试少于 11 行）且新增代码总量 > 200 行时，收敛报告中标 WARNING——不阻断收敛但强制 human review flag。
- **staleCount 增加 GatesGreen 变化信号**：当前 `staleCount` 只比较 `cur <= prev`（RoadmapCompletion 停滞即无进展）。改为：如果 iteration 间 `GatesGreen` 从 false→true，或 git diff 有新的非测试提交，算一次进展重置 staleCount——防止"代码写了但 ROADMAP 忘记勾"导致的假死循环。

**Phase B (1 sprint, 依赖 Phase A 落地验证)**:
- **测试计数趋势检查**：`probeTestCount` 记录每个 glob 的测试数量到 `.forge/test-counts.jsonl`，如果计数下降 > 配置阈值（如 20%）则在 `forge accept` 中增加 FAIL 风险。
- **独立证据聚合评分**：构建 `IndependentCompletionScore`（基于 FileDelta + Git diff 行数 + Gate 通过率），与 `RoadmapCompletion` 的差异 > 30% 时自动阻断收敛并标记为 HONESTY_GAP。

### 风险

- **低**：Phase A 的 FileDelta 和 CodeTestRatio 都是纯信号计算（不调用 LLM），零外部依赖
- **中**：Phase B 的测试计数趋势需要历史数据，首次运行无基线

### 不做会怎样

ForgeOS 的核心价值主张是"让 AI 24h 无人值守自治"。如果收敛判定可以被 agent 的自报告偏差操控，
**整个自治管线的输出可信度归零**。这是当前系统最危险的系统性漏洞——不是边界情况，是设计层面的信任单点故障。
**触发时机：任意一次 agent 因 prompt 工程或模型幻觉错误报告完成度时**（不是"如果"，是"何时"）。

---

## 方向三 (P1): 运行时依赖净化 + 冷启动加速

### 现状

`loadWorkflow` (main.go) 在每次 `forge run`/`forge evolve` 时调用：

```go
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    shim := filepath.Join(repoRoot, "harness", "yaml2json.py")
    out, err := exec.Command("python3", shim, ymlPath).Output()
    // ...
}
```

**三个问题**：

1. **零依赖承诺被运行时违反**：`ROADMAP.md` 说 "pure Go stdlib, zero external dependencies"、
   `DECISIONS.md` D6 说 "零外部依赖(go.mod 无 require)"——都对编译时而言，但运行时依赖 `python3` + PyYAML。
   在最小 Docker 镜像、Windows 无默认 Python、CI runner 无 PyYAML 的场景下，`forge run` 直接炸。

2. **每次 workflow 启动 ~30-100ms 子进程开销**：`exec.Command("python3")` fork+exec 一个 Python 解释器，
   导入 yaml/json 模块，读取文件，转码，输出，退出。每次 `forge run` 做一次，在有多个 sequential `forge run`
   的多步自动化中叠加。

3. **YAML→JSON 转码是纯计算，无需每次重复**：如果 workflow 文件不变，转码结果是相同的。目前没有缓存。

### 影响量化

| 场景 | 当前延迟 | 优化后 | 节省 |
|------|---------|-------|------|
| `forge run build` | ~50ms Python 启动 + ~30ms 转码 = ~80ms | ~1ms 解析 + ~0ms (无 I/O) | ~79ms (98%) |
| CI 中的 `forge run build && forge accept` | ~80ms + 0 (accept 无 shim) | ~1ms | ~79ms |
| `forge evolve` (5 iter × 每 iter run) | 5 × 80ms = 400ms | 5 × 1ms = 5ms | 395ms (98%) |
| Docker 最小镜像无 python3 | `forge run` 崩溃 | 正常工作 | 可用性 1→1 |

### 建议

**选项 A (推荐, 1 sprint): Node.js 重写 shim**:
- `harness` 已经依赖 Node.js（gate.mjs/acceptance.mjs 都跑在 node 上），所以用 Node 重写 shim 不增加新依赖
- 代码量约 20 行：`js-yaml` 的 `load` + `JSON.stringify`
- 但 js-yaml 是 npm 包——已有外部依赖。可改用纯 JS YAML 解析器或用 Node 内置的 `vm` 模块

**选项 B (更激进, 2 sprints): 嵌入式 Go YAML 解析器**:
- 为 forge-core 写一个最小 YAML→JSON 转换器（只支持 forge-core 使用的 YAML 子集：scalar/mapping/sequence）
- 完全消除 Python 和 Node.js 的运行时依赖
- 与"零外部依赖"定位完全一致
- 改动量约 200-300 行纯 Go（已有 `internal/yamlpath` 包作为 YAML 路径操作的基础）

**冷启动加速**:
- 在 `buildRunEngine` 中增加 workflow JSON 缓存：以 `(workflowPath, mtime)` 为键，当文件未修改时直接读缓存
- 对 `forge evolve` 的多迭代场景，每次迭代使用同一个解析后的 workflow 对象（而非重新 load）

### 风险

- **低**：不论选项 A 还是 B，都是纯技术债务清理，不影响任何功能语义
- **中**：选项 B 的 YAML 子集需要与 `check.py` 的 PyYAML 解析器保持兼容，两个解析器可能产生不同的 AST

### 不做会怎样

`forge run` 在无 Python 环境直接崩溃——这已经在 ROADMAP 的"零外部依赖"承诺上打了运行时补丁。
**触发时机：任意一个无 Python 环境的部署/CI runner 尝试运行 forge-core 时**。

---

## 方向四 (P1): 并行模式 Resume + 波内提前取消

### 现状

**波内提前取消**已在 `parallel.go` 中实现：
```go
waveCtx, waveCancel := context.WithCancel(parentCtx)
// 任何 phase 失败 → waveCancel()，剩余 phase 通过 CommandExecutor 的 context 链提前中止
```

但存在两个缺口：

### 缺口 1: RunParallel 不接受 startPhase

`loop.go` 中：
```go
if l.Parallel {
    runErr = l.Engine.RunParallel(wf, mode) // ← 没有 startPhase 参数
} else {
    runErr = l.Engine.RunFrom(wf, mode, startPhase) // ← 有
}
```

当 `forge evolve --parallel --resume` 从 checkpoint 的 `PhaseIndex=3` 恢复时，`startPhase` 被丢弃，
**从头重跑整个 iteration**——所有已完成的 agent phase 被重新执行、重新计费。

详见 `edgecases-and-perf.md §1.2`。per-phase checkpoint 的节省在 parallel 模式下全部丢失。

### 缺口 2: 波内 failed 阶段完成后才取消

`runWave` 中：
```go
func (e Engine) runWave(...) error {
    // ...
    for _, idx := range wave {
        go func(i int) {
            defer wg.Done()
            if err := e.runPhaseParallel(waveCtx, wf, i, mode, mu, agentCalls); err != nil {
                mu.Lock()
                if *firstErr == nil {
                    *firstErr = err
                    waveCancel() // 取消剩余 goroutine
                }
                mu.Unlock()
            }
        }(idx)
    }
    wg.Wait() // 等待所有 goroutine 完成（包括被取消的）
```

当前 `runPhaseParallel` 入口检查 `ctx.Err()`：
```go
if err := ctx.Err(); err != nil {
    return err // 立即返回，不执行 phase
}
```

但 **已经在执行的 `runAgentPhase`（调用了真 claude）不会被即时 kill**——它只会在 `CommandExecutor`
的下一次 `ctx.Done()` 检查时（如果有的话）才中止。对 claude 这样的长时间调用，这意味着：
- phase A（gate）2s FAIL → waveCancel()
- phase B（claude 调用，已跑 5s）继续跑完剩余 25s
- 浪费 ~$0.50 + 30s 时间

### 影响量化

| 场景 | 当前行为 | 理想行为 | 浪费 |
|------|---------|---------|------|
| `--parallel --resume` 从 phase 3 重跑 | 重跑 phase 0-4-5 | 只跑 phase 3-5 | 每次 resume 浪费 2-3 phase 费用 |
| 波内 phase 1 失败 (gate)，phase 2 正在跑 claude | 等 phase 2 跑完（~30s + $0.50） | 立即 kill | 每次失败浪费 0.5-1 美元 |
| 波内 5 个 phase，第 1 个 gate fail | 剩余 4 个 agent 跑满，约 $2.20 | 立即 kill 全部 | $2.20 + 2min（每波） |

### 建议的演进路径

**Phase A (1 sprint, 修复 resume 缺口)**:
- 添加 `RunParallelFrom(ctx, wf, mode, startPhase int)` 方法
- `RunParallel` 退化为调用 `RunParallelFrom(ctx, wf, mode, 0)`（完全向后兼容）
- 实现方式：`Waves(wf.Phases)` 返回波次后，丢弃 `startPhase` 之前的所有波次——如果 startPhase=3 属于 wave 1，
  则从 wave 1 开始，并跳过 wave 0 中 phase < startPhase 的阶段

**Phase B (1 sprint, 修复 abort 延迟)**:
- 在 `runAgentPhase` 中增加 `context.Done()` 监听 goroutine：当 ctx cancelled 时，kill 子进程
- 关键挑战：Go 的 `os/exec` 没有"kill 子进程的子进程"的原生支持——`cmd.Cancel`（Go 1.20+）只杀直接子进程，
  agent 的子进程（如 claude 的 Python 调用栈）可能变成孤儿
- 解决方案：使用进程组 `cmd.SysProcAttr{Setpgid: true}` + `syscall.Kill(-pid, SIGKILL)` 杀整个进程树

### 风险

- **低**：Phase A 纯添加，不修改现有行为，100% 向后兼容
- **高**：Phase B 的进程树 kill 存在跨平台问题（Windows 无 setpgid），需要 conditional build
  （`command_executor_unix.go` vs `command_executor_other.go` 模式已为这种情况准备）

### 不做会怎样

`forge evolve --parallel` 的用户如果遇到 crash 后恢复，会被重复计费（每次 resume 重做已完成的 phase）。
这直接侵蚀 ForgeOS 的经济模型可信度。**触发时机：首次有用户在生产环境使用 `--parallel --resume` 时**。

---

## 方向五 (P2): Mode 感知 Scorecard + Memory 置信度衰减

### 现状

**Scorecard (scorecard_wind.go)**:
```go
// windDownScorecards writes the current run's metrics into scorecards.json
// Key: (model, task_type) → aggregated metrics
// BUT there is no mode field in the key.
```

`scorecards.json` 的键是 `(model, task_type)`，没有 `mode` 维度。
explorer mode 跑 Haiku 的 50 个低质量样本与 balanced/engineering mode 的 Sonnet 样本混在同一个桶里。

**Memory (memory.go)**:
```go
type Entry struct {
    Text string
    // 无置信度字段！所有 entry 权重相同
}
```

所有 memory 条目没有置信度标签。一个 agent 错误记录的"这个项目的架构是过时的"和另一个 agent 正确记录的
"ADR 0002 规定了新架构"被视为平等的。

### 已经验证的具体影响

1. **Scorecard 跨 mode 污染**：explorer 模式下因 gate-set 最小（只有 lint+build），quality_score 虚高（没跑 test/complexity/architecture gate）。如果 balanced 模式重用同一 scorecard 数据，会看到"Haiku 表现很好"而路径偏向 Haiku——但 Haiku 的真实质量在 balanced 的全闸门下会大幅下降。

2. **Memory 错误自我强化**：`memory.jsonl` append-only，无编辑/删除机制。如果 iteration 5 写入"Payment 模块使用 Stripe SDK"，iteration 20 迁移到了 Adyen SDK，但旧的 memory 条目没有标记为过期，agent 在 iteration 21 仍会看到"Stripe SDK"并尝试用它。

3. **Scorecard 只写不读**：`windDownScorecards` 只在 run/evolve 结束时写入，但 evolve 循环中间不读 scorecard 来调整路由。`HistoryTiebreak` 在冷启动（第一次 run）时不产生任何路由收益——因为它需要 scorecard 数据，而数据只有 run 结束后才有。

### 建议的演进路径

**Phase A (1 sprint, Scorecard mode 感知)**:
- `scorecards.json` 的 key 扩展为 `(mode, model, task_type)`
- `HistoryTiebreak` 在查询时只匹配当前 mode 的样本——如果当前 mode 无样本，回退到 mode-agnostic 聚合（渐进降级）
- `forge migrate --to engineering` 时清理旧 mode 的 scorecard 缓存

**Phase B (1 sprint, Memory 置信度)**:
- `memory.Entry` 增加 `Confidence float64`（0.0-1.0，默认 0.5）
- `BuildPrompt` 按 confidence 排序输出，低置信度排在后面或被阈值过滤（<0.3 不注入）
- agent 写入 memory 时可自标注 confidence（如 "I am 90% confident this inference is correct"）
- 添加 `forge memory-prune --min-confidence 0.3` 命令清理低置信度条目

**Phase C (1 sprint, 实时 Scorecard 反馈)**:
- 在 evolve 的每次 iteration 结束时（而非整个 loop 结束时）写入 scorecard 快照
- Evolve 循环的 `OnIteration` hook 中调用 `windDownScorecards`（当前只在 `execEngine` 的 defer 中调用一次）
- 使 `HistoryTiebreak` 在 evolve 的后续 iteration 中实时受益于刚产生的数据

### 风险

- **低**：Phase A 是纯数据结构扩展，向后兼容（旧 scorecard 的 `(model, task_type)` key 自动归为 `("unknown", model, task_type)`）
- **低**：Phase B 的默认 confidence=0.5 确保现有 memory 行为不变
- **中**：Phase C 需要确保 scorecard 写入不成为迭代循环的瓶颈（写入 ~50μs 对迭代 30s 可忽略）

### 不做会怎样

**Scorecard 跨 mode 污染**会在 mode 切换时产生不可预测的路由降级——用户从 explorer 切换到 engineering，
发现模型选择比随机还差（因为 Haiku 的老数据拉偏了路由）。**Memory 无置信度**意味着一次 agent 的错误判断
会永久污染记忆（没有衰减编辑），随着项目增长，memory 的噪声比会持续劣化。
**触发时机：用户首次从 explorer 切换到 engineering 且 scorecard 有 50+ explorer 样本时**。

---

## 关系矩阵

```
方向二 (收敛验证) ← 依赖 ← 方向三 (依赖净化)  → 依赖 → 方向四 (并行 Resume)
       │                                               │
       │                                               │
       ▼                                               ▼
方向五 (Scorecard + Memory) ←──── 共享 ────→ 方向一 (语义引擎)
       │                      "数据质量"               │
       │                                               │
       └──────────── 全部受益于 → 方向三的冷启动优化 ────┘
```

| 方向 | 解锁条件 | 阻塞谁 |
|------|---------|--------|
| P0 方向一 (语义引擎) | 项目 > 15 ADR | 无阻塞，独立 |
| P0 方向二 (收敛验证) | **无**——当前最紧急的安全漏洞 | 阻止假收敛导致的产品决策错误 |
| P1 方向三 (依赖净化) | **无**——纯技术债务 | 消除 forge-core 唯一运行时依赖 |
| P1 方向四 (并行 Resume) | 方向一的 Context 传播 (已就绪) | 并行模式的成本可靠性 |
| P2 方向五 (Scorecard+Memory) | 无 | 路由数据质量和记忆健康 |

---

## 推荐执行顺序

| 阶段 | 方向 | 为什么 | 时间估计 |
|------|------|--------|---------|
| **冲刺 1** | 方向二 Phase A (收敛独立验证) + 方向三 (依赖净化) | 最高 ROI 组合：修最危险的系统漏洞 + 清最令人尴尬的技术债务 | 1 sprint |
| **冲刺 2** | 方向五 Phase A+B (Scorecard mode + Memory 置信度) | 路由数据质量 + 记忆健康——为稳定的长期自治循环打数据基础 | 1-2 sprints |
| **冲刺 3** | 方向四 Phase A (Parallel resume) + 方向四 Phase B 部分 (波内取消) | 并行模式的成本可靠性，让 `--parallel` 在生产环境可信任 | 1 sprint |
| **冲刺 4-5** | 方向一 Phase A (ADR 全文检索 + 可配置 topK) | 中期立即可做的上下文改进，不引入外部依赖 | 1 sprint |
| **后续评估** | 方向一 Phase B (语义嵌入) + 方向五 Phase C (实时 Scorecard) | 需要嵌入模型评估；scorecard 实时化依赖方向五 A+B 先落地 | 2+ sprints |

---

## 总结

ForgeOS v2 的核心循环已经端到端跑通，真 claude 闭环坐实，13 Go 包架构干净。
但**生产就绪度**在以下方面仍有缺口：

1. **信任单点故障** (P0) —— 收敛判定信任 agent 自报告，无独立验证
2. **运行时伪依赖** (P1) —— "零外部依赖"只在编译时成立，运行时靠 Python
3. **并行模式成本泄漏** (P1) —— resume 重计费 + 波内失败不 kill
4. **数据质量衰减** (P2) —— Scorecard 跨 mode 污染，Memory 无置信度
5. **上下文可扩展性** (P0) —— BM25-lite 在项目 > 15 ADR 后不够用

**最优先的行动**：方向二 Phase A + 方向三。两个方向合计改动量约 300-500 行代码（含测试），
零外部依赖，但将系统的**信任安全性**从"不可接受"提升到"可审计"，将**运行时依赖**从"有条件的"提升到"真正零依赖"。

*分析日期: 2026-07-01 | 基于 forge-core v2 全量源码扫描 + Sprint 1-26 经验 + 真 claude 运行观察*
