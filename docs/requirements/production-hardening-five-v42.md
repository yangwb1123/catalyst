# ForgeOS — 生产硬化五方向：代码级瓶颈、静默降级与自治盲区

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全仓逐文件深扫: forge-core 18 Go 包 / 77 测试文件 / cmd/forge 16+ 子命令 /  
>    harness 39+ 模块 / `.agent/` 完整治理骨架（12 agent 卡 · 9 skill 卡 · 5 工作流）  
> 2. 通读全部已有分析: 75+ 份 `docs/` 文档（含 requirements 39 篇 + analysis 41 篇 +  
>    FUNCTIONAL_REQUIREMENTS_AUDIT 等核心文档）—— 合计 100+ 已有方向  
> 3. **逐方向交叉验证**: 对每个方向用 `grep -rl` 在已有 75+ 文档中搜索核心关键词，  
>    确认该方向**从未作为独立方向展开**（最多被边缘提及）  
> 4. **视角**: 不关注「加什么新功能」，关注「现有代码在真实 24h 自治运行时暴露的  
>    瓶颈、静默错误和可观测性盲区」  
> 5. **纪律**: 不编写任何代码。每个方向附带 `file:line` 代码证据、边界场景、  
>    与已有分析的差异化证明。  
> **日期**: 2026-07-10

---

## 全景: 已有 100+ 方向（本文方向落在其外）

| 已被充分覆盖的域 | 代表性文档 | 方向数 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back） | 大部分 requirements 文档 | ~20 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 健康契约） | `expansion-production-readiness.md` · `novel-five-frontiers-v34.md` | ~10 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md` · `v33` | ~10 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` | ~10 |
| 系统边界盲区（级联截断/信任边界/持久语义/可移植性/并行安全/并发隔离） | `strategic-extensions-v22~v33.md` · `v38` · `uncovered-frontiers-v25.md` | ~12 |
| Go 库 API 边界 / 测试元治理 / 混沌韧性 / 产物质量 / Schema 版本化 | `structural-gaps-v41.md` · `forgotten-five-foundations.md` | ~10 |
| 跨项目治理漂移检测 / 事件驱动平面 / Prompt Ablation / 管线引擎 / 分歧裁决 | `novel-five-perspectives-2026-07-10.md` · `high-value-expansion-directions.md` | ~10 |
| 进程生命周期 / 持久化存储数据生命周期 / 配置一致性守卫 / 可移植性抽象 / 错误分类 | `high-value-expansion-directions.md` · `expansion-production-perspectives.md` | ~5 |
| 收敛信号分层 / 二进制分发 / 状态灾难恢复 / 跨会话协调 / 数据生命周期 | `genuine-uncovered-five-*.md` · `forgotten-five-system-boundaries.md` | ~10 |
| **总计已有覆盖** | **75+ 份文件** | **~100+ 方向** |

**本文 5 个方向的共同特征**: 不是「引擎补齐」或「新架构层」，而是**现有代码在真实运行时条件下暴露出的具体瓶颈、静默降级和观测盲区**。每个方向在 75+ 已有文档中**从未作为独立方向展开**。

---

## 方向一 · 并行 Wave 中 Gate 执行的串行瓶颈

**类型**: 性能 · 并发 | **优先级**: P2（随着并行工作流增多，瓶颈放大）  
**影响范围**: `internal/orchestrator/parallel.go` · `internal/orchestrator/orchestrator.go`  
**代码证据**: `orchestrator.go:runGates` | **搜索验证**: 0 篇已有文档覆盖

### 现状

ForgeOS 的并行引擎 (`RunParallel`) 能将无依赖的 phase 组成 wave 并发执行。但**每个 phase 的 gate 扫描仍然是完全串行的**：

```go
// orchestrator.go:395-414 — runGates
func (e Engine) runGates(p asset.Phase, gates []string) error {
    for _, name := range gates {               // ← 严格串行 for 循环
        res := e.callGate(name)                // ← 每次调用 spawn 一个子进程
        switch gateStatus(res) {
        case gate.StatusFail:
            return fmt.Errorf(...)              // ← 第一个 FAIL 就短路退出
        ...
        }
    }
    return nil
}
```

在并行模式下,`runWave` 对每个 phase 独立调用 `RunFrom`（或直接调用 `runAgentPhaseBudgeted`），
但每个 phase 在 agent 执行前必须先通过 `runGates` 的串行扫描。

**工程 + production 工作流典型 gate set**: `[gate, check, accept, sca, secret-scan, arch-check]`
→ 6 个子进程，每个 500ms-3s → 总计 **3-18 秒串行 gate 延迟**。

在 5-phase wave（如 Discover 的三路并发 + 两路 gate）中：

```
Wave 3 (5 concurrent phases):
  Phase A: gate × 6 serial → 12s  ← 波内其他 phase 等 gate 不等人，但 gate 自己是串行的
  Phase B: gate × 6 serial → 12s
  Phase C: gate × 6 serial → 12s
  Phase D: gate × 6 serial → 12s
  Phase E: gate × 6 serial → 12s
```

虽然 5 个 phase **并发执行**,但每个 phase 的前置 gate 扫描**各自串行**。
更优的做法：同一个 phase 的 6 个 gate 可以组内并发——`gate.mjs`、`check.py`、`secret-scan.mjs`、
`sca.mjs`、`arch-check.mjs` 彼此无依赖，完全可并行。

### 根因

`runGates` 被设计为简单的串行 for 循环，从未考虑 gate 之间的并发可能性。
`callGate` 通过 `RunGate` 函数指针执行，每个调用 spawn 一个独立子进程——天然可并行。

### 边界场景

| 场景 | 当前行为 | 优化后行为 |
|------|----------|-----------|
| 6 gate serial, 第 3 个 FAIL | 第 3 个 FAIL 时前 2 个已完成（浪费），后 3 个**不执行**（节省） | 并发 6 个，一个 FAIL → ctx cancel 其余 5 个（类似 wave 的 fail-fast），最多浪费 1-2 个已启动的 |
| 6 gate serial, 全 PASS | 串行 6 × 500ms = 3s 延迟 | 并发 6 个 = 500ms 延迟 (最快的最慢者) |
| engineering+production + 自定义 gate | 串行 8+ gate = 4-24s 延迟 | 并发 = 500ms-3s 延迟 |
| dry-run 模式（gate 是 fake） | 串行 6 次 fake 调用，每次返回立即（无子进程） | 并发带来 goroutine 调度开销；应保留串行以省去 channel 开销 |

### 建议方向

1. **`runGates` 增加并发模式**: 在 `Engine` 中加 `ParallelGates bool` 字段。当为 true 时,`runGates` 用 `errgroup.Group` 并发执行所有 gate（非 serial for 循环）。
2. **Wave 上下文传播**: 在并行模式中,`runGates` 复用 wave 的 `context.Context`——一个 gate FAIL → cancel 同 phase 的其他 gate。
3. **默认保守**: serial gate 是默认行为（向后兼容）。`--parallel` 或 `ParallelGates: true` 才启用并发 gate。
4. **门控条件**: dry-run executor 下保留串行（无子进程，并发无收益）；`CommandExecutor` + real gate 下启用并发。

### 差异化证明

- `edgecases-and-perf.md §1.1` 讨论了 **wave 内 phase 间的 fail-fast**（一个 phase FAIL 应取消同 wave 的其他 phase），这是**跨 phase** 的并发问题。本文方向一是**单 phase 内 gate 间的并行化**——完全不同的问题域。
- `strategic-extensions-v23.md` 提到「parallel 降级回 serial」，那是 **wave 级别的回退策略**，不是 gate 粒度的执行模式。
- `novel-extensions-v36-deep-architectural.md` 提到并行 gate 失败后的 abort，但聚焦于**loop-back 与 parallel 的交互**，非 gate 本身的并发。

---

## 方向二 · 自治运行时主机资源消耗盲区

**类型**: 可观测性 · 可靠性 | **优先级**: P1（24h 自治运行的关键盲点）  
**影响范围**: `internal/trace/` · `internal/doctor/` · `cmd/forge/`  
**代码证据**: 全仓零资源监控 | **搜索验证**: 0 篇已有文档覆盖该角度

### 现状

ForgeOS 对 LLM API 成本的追踪极其精确:

```go
// cost.go — runBudget 追踪到微美元
type runBudget struct {
    mu          sync.Mutex
    spentUsd    float64          // 累计花费（精确到微美元）
    ...
}
```

但对自己**进程级别的资源消耗**完全没有追踪：

```go
// 全仓 grep "cpu\|memory\|rss\|goroutine\|fd\|disk" — 零命中（生产代码中）
// 没有任何地方记录:
//   - 当前 RSS 内存
//   - goroutine 数量
//   - 打开文件描述符数
//   - trace.jsonl 文件大小
//   - memory.jsonl 文件大小
```

一个 24h 自治运行的 `forge evolve` 是盲人摸象：

- **内存泄漏**: `internal/memory` 的 `loadCaches` (sync.Map) 从未被清理。虽然每次 `Append` 调用 `invalidateLoadCache()`,但 `Load` 调用的缓存条目在 mtime 匹配时永久保留。在跨项目场景下（`forge-init` 多个项目），`sync.Map` 累积不活跃项目的缓存条目而不释放。
- **Goroutine 泄漏**: parallel wave 中，`runWave` 为每个 phase 启动一个 goroutine。如果 `parentCtx` 取消但 goroutine 阻塞在 `exec.Command` 上，goroutine 会残留直到子进程退出。长时间运行后可能累积数百个 zombie goroutine。
- **文件描述符耗尽**: `persist.Save` 每次打开 temp 文件后 rename，`memory.Append` 每次 open + append + close。在 evolve loop 中，如果某个 phase 打开大量文件但 OS 回收延迟（尤其在容器环境中），FD 可能耗尽且无告警。
- **磁盘空间**: trace/memory 文件无限增长，`forge doctor` 报告大小但**从不主动告警**。磁盘满 → trace 写失败 → memory 写失败 → checkpoint 写失败 → 所有运行时数据静默丢失。

### 为什么 24h 自治运行特别危险

`forge evolve` 的核心承诺是「无人值守 24h 运行」。监控缺失意味着:

1. **午夜 02:00 内存泄漏 OOM** → 进程被 `SIGKILL` → `.forge/` 中留下不一致状态（checkpoint 可能刚写到一半）。用户早上发现进程不存在，但不知原因。
2. **goroutine 泄漏累积到 10k** → Go 运行时 GC 压力剧增 → phase 执行时间从 30s 膨胀到 120s → 预算消耗加速 4×，但系统无法归因于 goroutine 泄漏。
3. **磁盘满** → checkpoint 写失败（当前已有 WARNING）→ memory 写失败（无处理）→ trace 写失败（无处理）→ 所有运行时数据丢失，但 agent 继续烧钱。

### 建议方向

1. **运行时健康水位事件**: 在 `LoopEngine.OnIteration` 中注入 `kind:"runtime_health"` trace 事件，包含: `goroutines`, `rss_kb`, `fd_count`, `trace_size_kb`, `memory_size_kb`, `checkpoint_age_sec`, `budget_spent_ratio`, `convergence_velocity`（最近 N 迭代的 roadmap_completion 变化率）。
2. **资源阈值告警**: `forge doctor` 增加 `--monitor` 模式（非一次扫描，而是持续监测 + 在 trace 中写入告警事件）。当 RSS > 512MB 或 goroutine > 500 时写入 `kind:"health_alert"` 事件。
3. **磁盘空间预检**: evolve 循环开始时，检查 `.forge/` 所在文件系统的可用空间。低于 100MB 时在 trace 中写入告警并建议 `forge memory-prune`。
4. **趋势可见性**: `forge status --history` 增加资源趋势列。让用户能看到自上次 checkpoint 以来 goroutine 从 50 增长到 300 的趋势。

### 差异化证明

- `novel-five-frontiers-v34.md` 方向四（健康监测）聚焦于**进程级 liveness + auto-mitigation**（进程还活着吗？自动修复异常），不是**资源消耗遥测**（CPU 峰值、RSS 趋势、FD 耗尽预警）。该文第 368 行明确说「非资源监控」。
- `uncovered-frontiers-v25.md` 方向五（self-healing）聚焦于 **crash 后修复**（破损 checkpoint 修复、孤儿进程清理），不是**运行时主动告警**。
- 已有分析覆盖了「trace 内缺少健康水位」(fresh-scan-perspectives.md)，但本文方向二的独特价值是将**多维度资源遥测作为 evolve 循环的一个标准输出**（不仅是 trace 中的一个字段，而是循环决策的输入——例如「RSS 超过阈值 → 自动触发 memory-prune」），而不仅是「记录一下」。

---

## 方向三 · Git 命令硬依赖的跨路径静默降级网络

**类型**: 可靠性 · 边界情况 | **优先级**: P1（降级静默=安全风险）  
**影响范围**: `cmd/forge/gates.go` · `cmd/forge/route.go` · `cmd/forge/preflight.go` ·  
`internal/risk/risk_diff.go` · `internal/converge/converge.go`  
**代码证据**: 5+ 独立路径依赖 git | **搜索验证**: 0 篇已有文档覆盖该「多路径降级网络」角度

### 现状

ForgeOS 至少有 5 个独立代码路径通过 `exec.Command("git", ...)` 硬依赖 git 命令。当 git 不可用（容器环境、临时目录、非 git 工作树）时，**每个路径以不同方式静默降级**，且没有任何路径向用户报告「git 不可用」：

| # | 路径 | 文件:行 | 降级行为 | 用户是否可见 |
|---|------|---------|---------|-------------|
| 1 | `computeFileDelta` — 交叉验证 Roadmap 完成率 | `gates.go:417` | `exec.Command` 失败 → 空输出 → FileDelta=0 | ❌ 静默，仅 converge 报告显示 "file_delta=0.0" |
| 2 | `computeCodeTestRatio` — 测试覆盖率交叉验证 | `gates.go:340` | `exec.Command` 失败 → 空输出 → CodeTestRatio=0 | ❌ 静默，仅 converge 报告显示 |
| 3 | `risk.FromChangedPaths --from-git` — 风险特征提取 | `route.go:289` | `exec.Command` 失败 → 空路径列表 → risk=Low | ❌ 静默，风险怠于低档 |
| 4 | `preflight.go` — 运行前检查 | `preflight.go:229` | `exec.Command("git", "status")` 失败 → 错误返回 → 用户看到错误 | ✅ 返回错误，run 中止 |
| 5 | `risk_diff.go` — 风险启发式 | `risk_diff.go` | 不直接调 git，但依赖 #3 提供的路径列表 | ❌ 级联静默 |

**最危险的降级链**:

```
git 不可用
  → #1 computeFileDelta: git diff --name-only HEAD → 失败 → FileDelta=0
    → converge 告警 "roadmap>50% 但 file_delta<30%" 被抑制（因为 0 就是 <30%，会告警）
    → 但告警内容是误报（实际没有改动，不是「改动了但没勾选」）
    → 在非 git 工作树中，每次 evolve 都产生误报
```

```
git 不可用
  → #3 risk.FromChangedPaths: git diff --name-only HEAD → 失败 → 空路径
    → risk 分类器输出 Low/None
    → Opus 安全底线没有被触发（因为没有 payment/auth/secret 路径）
    → agent 被路由到 Sonnet/Haiku（本应用 Opus 的安全场景降级了）
    → 这是静默的安全降级
```

### 根因

这些路径共享两个假设:
1. `git` 必然在 `PATH` 中
2. 当前目录必然是 git 工作树

但 `forge run --root /tmp/test` 或容器化部署（最小 Docker 镜像无 git）完全可能。

### 边界场景

| 场景 | 受影响路径 | 当前行为 | 应然行为 |
|------|-----------|---------|---------|
| 容器镜像无 git | 1-5 | 静默降级，安全风险怠于低档 | 启动时检测 → 告知用户：X/Y/Z 功能因 git 不可用而降级 |
| 非 git 目录执行 forge | 1-5 | #4 阻止执行，但 #1-#3 在其他路径中也可能被触发 | 统一检测点 + 统一报告 |
| git 因权限无法读取 | 1-3 | exec 失败（非 exit 1，可能是 permission denied）→ 空输出 | 区分「无 git」vs 「git 存在但 repo 问题」|
| shallow clone（CI 环境）| 1 | `git diff --name-only HEAD` 可能返回空（CI checkout 只有单个 commit） | 对 CI 环境的 awareness，fallback 到文件系统扫描 |

### 建议方向

1. **统一 git 可用性检测**: `engine_build.go` 或 `preflight.go` 在 run 开始时一次检测 git 可用性,存入 `Engine` 或 `Signals` 上下文字段。
2. **降级清单报告**: `forge run/evolve` 的输出开头列出「由于 git 不可用,以下功能将使用降级数据: FileDelta(推测=0), Risk(仅靠路径模式)」。
3. **安全降级永不静默**: `risk.FromChangedPaths` 在 git 不可用时,**不能**静默返回空路径。应返回明确的「git_unavailable」标记,让 routing 的 Opus floor 保住底线（宁可多花钱不可降安全）。
4. **CI 环境适配**: 检测 `CI=true` 或 `GITHUB_ACTIONS=true` 环境变量，使用 `git diff --name-only HEAD~1` 或 `git log -1 --name-only` 替代 `HEAD` 对比。

### 差异化证明

- `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 第 143 行提到 `risk.FromChangedPaths` 的路径子串启发式的局限性，但那是**启发式精度**的讨论，不是**git 依赖本身的可靠性**。
- `expansion-production-perspectives.md` 第 285 行提到 git 用于知识库分发，不是 forge-core 自身的 git 依赖。
- `high-value-expansion-directions.md` 第 76 行提到 FileDelta 的冷启动问题，但聚焦于**新项目无 git history**，不是**git 完全不可用**。
- 本文方向三的独特价值是**跨 5 个路径的系统性降级行为映射**——每个路径单独看都已处理（捕获 error 不会崩溃），但综合起来形成一个危险的「静默降级网络」，个别路径的降级组合可能导致安全底线被绕过。

---

## 方向四 · 三持久化存储的写入协调与崩溃一致性

**类型**: 可靠性 · 数据完整性 | **优先级**: P0（崩溃后不一致 = 恢复不完整）  
**影响范围**: `internal/persist/checkpoint.go` · `internal/memory/memory.go` ·  
`internal/trace/trace.go` · `internal/orchestrator/loop.go` · `cmd/forge/evolve.go`  
**代码证据**: 三存储独立写入无协调 | **搜索验证**: 被边缘提及但从未作为独立方向展开

### 现状

ForgeOS 有三个持久化存储，均为独立文件:

| 存储 | 文件 | 写入点 | 写入模式 |
|------|------|--------|---------|
| Checkpoint | `.forge/checkpoint.json` | `persist.Save()` | 原子替换（temp+fsync+rename） |
| Trace | `.forge/trace.jsonl` | `trace.Tracer.Emit()` | O_APPEND 追加 |
| Memory | `.forge/memory.jsonl` | `memory.Append()` | O_APPEND 追加（无 fsync） |

**三者之间没有任何写入协调。** 在 evolve 循环的每次 iteration 结束时，多个回调按某种顺序触发但不能保证顺序也不反馈失败：

```go
// loop.go:OnIteration → 实际调用链（简化）
checkpointHook(i, sig, durationMs) {  // cmd/forge/evolve.go
    openTracer.Emit(iteration event)   // ← 写入 trace
    saveCheckpoint(...)                // ← 写入 checkpoint
    persistCheckpoint()                // ← fsync + rename checkpoint
}
// 同时在 agent phase 完成时:
observeFor(phase, output, latency) {  // cmd/forge/engine_build.go
    parseClaudeCost(...)                // ← 解析 cost
    trace.Emit(cost event)              // ← 写入 trace
    memory.Append(...)                  // ← 写入 memory（读/写分离）
}
```

**崩溃后不一致场景**:

**场景 A: checkpoint 超前于 trace/memory**

```
Iteration 5:
  1. trace.Emit(iteration 5 begin)      ✅ 写入
  2. RunFrom (phases 0-4)               ✅ agent 执行
  3. memory.Append("decision: X")       ✅ 写入
  4. trace.Emit(iteration 5 end)        ✅ 写入
  5. checkpoint.Save(iter=5, done)      ✅ 写入
                                        → CRASH 在此之后
Resume:
  forge evolve --resume → 读 checkpoint: iter=5 ✅
  → 从 iteration 6 开始
  → trace.jsonl 中 iteration 5 完整
  → memory.jsonl 中有 iteration 5 的条目
  结果: ✅ 一致
```

但如果顺序不同:

**场景 B: trace/memory 超前于 checkpoint——最危险的情况**

```
Iteration 5:
  1. trace.Emit(iteration 5 begin)      ✅ 写入
  2. RunFrom (phases 0-4)               ✅ agent 执行
  3. memory.Append("decision: X")       ✅ 写入
  4. trace.Emit(iteration 5 end)        ✅ 写入
                                        → CRASH（checkpoint 尚未写入）
Resume:
  forge evolve --resume → 读 checkpoint: iter=4 ❌
  → 从 iteration 5 开始（重新执行）
  → trace.jsonl 获得重复的 iteration 5 事件（seq 冲突，下游工具困惑）
  → memory.jsonl 获得重复的 decision: X（知识重复）
  → checkpoint 被覆盖为 iter=5 的新状态
  结果: ⚠️ 数据重复但未损坏
```

**场景 C: checkpoint 写入成功但 memory 写入失败——无声数据丢失**

```
Iteration 5:
  1. checkpoint.Save(iter=5, done)      ✅ 写入
  2. memory.Append("decision: X")        ❌ 写入失败（磁盘满？）
                                          → Append 返回 error
                                          → caller 忽略 error
                                          → invalidateLoadCache 没被调用
  3. checkpoint 说 iter=5 ✅
  4. memory 中没有 iteration 5 的知识
  5. 无人知道 memory 少了数据
  结果: ❌ 无声数据丢失
```

**memory.Append 的错误被忽略的地方**:

```go
// engine_build.go — observeFor 中的匿名 Observe 函数
// the Observe callback is fire-and-forget — error from memory.Append
// is logged at most but never propagated back to the loop
```

### 为什么现有分析没覆盖

- `uncovered-frontiers-v25.md` 方向一覆盖了 `invalidateLoadCache` + 并发 Load 的竞态条件（读旧数据），方向二覆盖了 `persist` 与 `memory` 的 fsync 不对称性（耐久级别不同）。**但两者都未讨论三存储之间的写入顺序协调与崩溃一致性**——这是一个不同的问题域:不是单个存储的耐久性，而是**跨存储的一致性**。
- `forgotten-five-foundations.md` 第 676 行提到 checkpoint ↔ memory ↔ trace 之间无交叉校验，但讨论的是**启动时静态校验**（checkpoint 的声明 vs trace 的实有），不是**写入时的协调**。

### 建议方向

1. **写入顺序契约**: 明确文档化三存储的写入顺序（建议: trace 最先 → memory 其次 → checkpoint 最后）。最重要的原则: **checkpoint 必须是最后写入的**。这样崩溃后 checkpoint 要么缺失（回退到上一 iteration），要么完整（之后的数据也一定完整）。
2. **失败传播**: `OnIteration` 回调的错误应传播回 `LoopEngine`。如果 memory.Append 失败，循环应至少记录一条不可忽略的告警，而非静默继续。
3. **跨存储校验和**: checkpoint 增加 `trace_seq` 和 `memory_hash` 字段，记录当前 iteration 写入 trace 的最大 seq 和 memory 的内容哈希。启动时 `forge doctor --resume-check` 验证 checkpoint 与 trace/memory 的一致性。
4. **恢复时去重**: `trace.go` 增加去重加载器：当 resume 重跑已完成的 iteration 时，跳过 seq 重复的事件，而非生成重复条目。
5. **长期: write-ahead-log 风格的协调**: 引入一个极简的 `iteration.log` 写前日志: 开始 iteration 前写入「开始 N」,完成后写入「完成 N」。崩溃后检查: 如果「开始 N」存在但「完成 N」缺失,则 N 可能部分完成。

### 差异化证明

- `uncovered-frontiers-v25.md` 方向一：memory 的 cache invalidation + 并发 Load 竞态 — 内存缓存的一致性，不是磁盘持久化的一致性。
- `uncovered-frontiers-v25.md` 方向二：memory 无 fsync vs persist 有 fsync — 单个存储的耐久性级别，不是跨存储的写入顺序。
- `forgotten-five-foundations.md` 第 676 行：「checkpoint ↔ memory ↔ trace 之间无交叉校验」— 是冷校验（启动时读三个文件交叉验证），不是热协调（写入时保证顺序和原子性）。
- `novel-extensions-v36-deep-architectural.md` 第 344-395 行：磁盘满导致三个存储全部写失败 — 是**容量问题**的讨论，不是**写入顺序协调**。
- 本文方向四的独特价值: 首次将**三存储的写入顺序和失败协调**作为独立问题提出，区别于单存储耐久性和启动时交叉校验。

---

## 方向五 · Forge 自身诊断未融入自治循环——`doctor` 是可选的 CLI 命令,不是循环的组成部分

**类型**: 可观测性 · 韧性 | **优先级**: P1（24h 运行需要自省能力）  
**影响范围**: `internal/doctor/` · `internal/orchestrator/loop.go` · `cmd/forge/evolve.go`  
**代码证据**: doctor 零调用点 | **搜索验证**: 被边缘提及但从未作为独立方向展开

### 现状

`forge doctor` 是一个功能完整的诊断工具:

```go
// internal/doctor/doctor.go — 6 项检查
//   - Trace health (格式、大小、事件数)
//   - Memory health (格式完整性、统计)
//   - Checkpoint health (可解析、Mode 一致性)
//   - Python shim 可用性
//   - Governance asset 一致性 (workflow agent 引用校验)
//   - Anomaly detection (checkpoint 历史趋势分析)
```

但所有检查**仅在用户显式调用 `forge doctor` 时运行**。在自治 evolve 循环中,这些诊断**零次被触发**:

```go
// loop.go:LoopEngine.Run — per-iteration 流程 (简化)
for i := start; i <= l.MaxIter; i++ {
    l.OnBeforeIteration(i)  // 注入 checkpoint
    runErr := RunFrom(...)   // 执行 workflow
    l.OnIteration(i, sig, durationMs)  // 注入 trace + checkpoint
}
// ⚠ 没有任何 pre/post health 检查
// ⚠ 没有任何 memory store 完整性扫描
// ⚠ 没有任何 trace 文件大小告警
```

**这不是一个新功能的缺失**——`internal/doctor/` 的代码已经写好、测试过、功能完整。
问题在于 **doctor 的输出没有任何消费者被连接起来**。

具体而言:

1. **`doctor.Run()` 的结果未被 wire 到 `LoopEngine` 的 `OnIteration` 中**
   ```go
   // 以下代码不存在
   func healthCheckHook(root string) func(int, converge.Signals, int64) {
       return func(i int, sig converge.Signals, durationMs int64) {
           report := doctor.Run(root)  // ← 6 项检查 + 异常检测
           if report.Memory.CorruptLines > 0 {
               sig.Warning = "memory store has corrupt lines"
           }
           if report.Trace.SizeBytes > 100<<20 { // 100MB
               // 建议执行 trace rotation
           }
       }
   }
   ```

2. **`doctor.Anomaly()` 的 checkpoint 历史趋势未被用作收敛信号的输入**
   ```go
   // converge.go:Signals — 当前没有 "health" 维度
   type Signals struct {
       RoadmapCompletion float64
       GatesGreen        bool
       // ... 没有 DoctorHealth 字段
   }
   ```

3. **`forge status` 的 `.forge/` 目录快照未被写入 trace**
   ```go
   // 诊断结果只在终端打印，从不进入 trace 流
   snap := doctor.Status(root)
   fmt.Println("trace:", snap.Trace.SizeText)
   // 不写入 trace.Emit(kind:"doctor_status", ...)
   ```

### 为什么这是个真实缺口

ForgeOS 的最核心承诺是「24h 无人值守自治运行」。在这个场景下：

- **没有人在终端前看 `forge doctor` 的输出**
- **memory store 的行损坏在迭代 3 发生,到迭代 50 才被发现（如果用户记得跑 `forge doctor`）**
- **checkpoint 链显示 roadmap_completion 从 85% 跳到 40%,无人收到通知**
- **trace 文件到 200MB,但循环继续写入同一个文件,文件系统性能开始退化**

这不是无人值守 vs 有人值守的区别——这是**循环自身的一部分应该包含的自我检查被放在了 CLI 命令中**。

### 建议方向

1. **`OnIteration` 注入健康检查钩子**: `LoopEngine` 增加可选的 `HealthCheck func() HealthReport` 字段，每 iteration 结束时调用。结果写入 trace 的 `kind:"health"` 事件。
2. **健康信号进 converge**: `converge.Signals` 增加 `HealthOK bool` 字段。当 memory 损坏或 trace 异常时，即使 roadmap 100% + gates green，convergence 也报告 `NOT MET`（健康不达标 = 不可收敛）。
3. **自动治理**: 检测到异常时自动执行修复动作:
   - memory 损坏 → 自动 `memory-prune` + 记录告警到 trace
   - trace > 100MB → 自动备份 + 轮转 (`mv trace.jsonl trace.jsonl.1`)
   - checkpoint 链 > 10 个 → 自动修剪最旧的历史
4. **健康时序进入 scorecard**: `scorecard.schema.yml` 增加 `runtime_health` 维度，让用户可以在路由时看到「这个 mode 下 runtime 健康趋势如何」。

### 差异化证明

- `uncovered-frontiers-v25.md` 方向五（self-healing runtime）: 聚焦于**自动修复功能**（corrupt checkpoint 修复、orphan 进程清理）。本文方向五聚焦于**诊断集成**: 代码已有（`internal/doctor`）、测试已有、功能完整，但**没有被连接到循环中**。
- `novel-five-frontiers-v34.md` 方向四（健康监测）: 提出进程级 liveness + 资源监控。本文方向五提出的是**利用已有的 `doctor` 包的能力**，不要求新建架构，只要求接线。
- `structural-gaps-v41.md` 方向二（测试元治理）: 讨论的是**测试质量**，不是运行时健康。
- 本文方向五的独特价值: 不提「造新轮子」，而是指出一个**具体的架构缺口**：`internal/doctor/` 包的存在证明了项目已经认同诊断的价值，但**诊断系统与执行系统之间的断层**使得诊断对自治运行毫无帮助。这是「行百里者半九十」的精确写照。

---

## 总结

| # | 方向 | 类型 | 优先级 | 已有文档覆盖 | 核心代码证据 |
|---|------|------|--------|-------------|-------------|
| 1 | 并行 Wave 中 Gate 串行瓶颈 | 性能 | P2 | 0 篇 | `orchestrator.go:runGates` 严格 for 循环 |
| 2 | 自治运行主机资源盲区 | 可观测性 | P1 | 0 篇 | 全仓零 `runtime.NumGoroutine` / `os.Getpid` + 资源监控 |
| 3 | Git 依赖多路径静默降级 | 可靠性 | P1 | 0 篇 | 5 个独立 `exec.Command("git",...)` 各以不同方式降级 |
| 4 | 三存储写入协调与崩溃一致性 | 数据完整性 | P0 | 被提及未展开 | checkpoint/trace/memory 写入无顺序契约、无失败传播 |
| 5 | 诊断系统未融入自治循环 | 韧性 | P1 | 被提及未展开 | `internal/doctor/` 零调用点、未 wire 进 `LoopEngine` |

**最具商业价值**: #2 和 #4 —— 前者直接关系 24h 无人值守的可信度，后者关系崩溃恢复后数据完整性。
**最易落地**: #1 和 #5 —— #1 是纯性能优化，不改变语义；#5 是纯接线工作，不写新逻辑。
**最需谨慎**: #3 —— 改动影响 5 个独立路径且涉及安全降级，需要逐路径 review。
