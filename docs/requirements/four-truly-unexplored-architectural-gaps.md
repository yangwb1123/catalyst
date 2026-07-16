# ForgeOS — 全局扫描揭示的四个真实未探索架构缺口

> **视角**: 资深架构师 / 产品经理（不编写代码）  
> **方法**:  
> 1. 全局深度扫描 forge-core（18+ Go 包 · ~33k LOC 生产代码 · 77+ 测试文件）  
> 2. 通读 harness（~10.5k LOC · 39+ 模块 · gate/check/accept/arch/secret-scan/SCA/select-tests）  
> 3. 通读 `.agent/` 完整治理骨架（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR 与 DECISIONS）  
> 4. 交叉验证: 逐篇核对 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` + 全部 41 篇 `docs/requirements/*.md` + 全部 43 篇 `docs/analysis/*.md` + `docs/expansion-*.md` + `docs/scan-*.md` ~ 共 100+ 已有分析文档  
> 5. **差异化证明**: 每个方向附「为什么 100+ 已有分析未覆盖此方向」的代码级论据  
> **纪律**: 不编写任何代码。每个方向附带 `file:line` 代码证据。  
> **日期**: 2026-07-10

---

## 已有 100+ 分析全景（本文不重复）

已有分析已高度密集地覆盖以下领域:

| 被充分覆盖的领域 | 覆盖量 | 代表性文档 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/诊断/并行/自适应装配） | ~20+ 方向 | `high-value-extension-directions.md` · `expansion-core-five.md` · `novel-architectural-extensions-v40.md` |
| 跨项目/跨会话/联邦治理（知识迁移/漂移检测/多仓库编排） | ~12+ 方向 | `novel-five-perspectives-2026-07-10.md` · `expansion-horizon-three.md` · `strategic-extensions-v23~v33.md` |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈 / 健康契约） | ~10+ 方向 | `expansion-production-readiness.md` · `expansion-production-blindspots-v36.md` · `novel-five-frontiers-v34.md` |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | ~10+ 方向 | `execution-semantic-gaps.md` · `expansion-forgeos-meta-governance.md` |
| 二阶系统问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/并行安全） | ~15+ 方向 | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` · `uncovered-frontiers-v25.md` |
| 质量/测试元治理（测试框架/突变测试/自身免疫/代码质量执法） | ~10+ 方向 | `structural-gaps-v41-genuinely-unexplored.md` · `five-genuinely-uncovered-frontiers.md` · `forgotten-five-foundations.md` |
| 配置 Schema / API 边界 / 热加载 / 产物治理 | ~10+ 方向 | `genuine-architectural-gaps-v28.md` · `structural-gaps-v41.md` · `forgotten-five-foundations.md` |
| DevOps / 事件驱动 / 并行引擎 / 迭代跳过 / 收敛可见性 | ~10+ 方向 | `high-value-extension-directions-v3.md` · `novel-five-frontiers-v34.md` |
| **总计已经有分析覆盖** | **~100+ 方向** | **通过 80+ 独立文档阐述** |

**本文的 4 个方向在这 100+ 已有方向中全部零覆盖（或仅边缘提及但从未作为独立方向展开）。**

---

## 方向一 · `cmd/forge` 包级别内聚性债务

> **类型**: 架构债务 · 可维护性  
> **优先级**: P1（结构性的、影响每个 sprint 扩展成本）  
> **预估工作量**: ~1.5 sprints（消解 + 新包落地）  
> **杠杆系数**: ⭐⭐⭐⭐⭐

### 现状

ForgeOS 架构纪律严格执行**文件级** 500 行上限、**函数级** 50 行上限。但**包级别**的内聚性从未被审视。

`forge-core/cmd/forge/` 当前状态:

```
文件数:         17
总行数:         ~12,513 (cat *.go | wc -l)
最大文件:       main.go (499 行), engine_build.go (498 行), gates.go (493 行),
                evolve.go (496 行), validate.go (488 行), cost.go (471 行),
                prompt_context.go (454 行), scorecard_wind.go (451 行)
每个文件都在 500 行以下 ✅（通过文件级闸门）
但 17 个文件处理的责任清单:
  □ CLI 子命令分发 (run/evolve/gate/check/accept/route/migrate/detect/validate/memory-prune/status/scorecard)
  □ 引擎装配 (buildRunEngine / agentExecutor / claudeArgv)
  □ 信号采集与收敛判定 (gatherSignals / reportConvergence)
  □ 成本追踪 (runBudget / costEmitter / parseClaudeCost)
  □ Trace 管理 (openTracer / wireGateTrace / costEmitter)
  □ Prompt 构建 (buildPrompt / prompt_context.go + prompt_memory.go + prompt_artifacts.go)
  □ 风险自动提取 (resolveAutoRisk / riskAdjustedTier)
  □ 模型路由 (phaseTierResolver / logPhaseHistory)
  □ Checkpoint/Resume (execLoop / openRunResources)
  □ 学习闭环 (windDownScorecards / scorecard_wind.go)
  □ 验证/诊断 (forge validate / forge doctor invoke / forge status)
  □ Memory 管理 (cmdMemoryPrune)
```

### 代码级证据

**证据 A: 单一包处理 12+ 完全不同的责任域**

```bash
$ cd forge-core/cmd/forge && grep -l "^func\|^var\|^type\|^const" *.go | wc -l
# 17 个文件贡献了 150+ 导出符号（函数+类型+变量），全部在 package main 中

$ grep -c "^func " main.go engine_build.go evolve.go gates.go cost.go validate.go
# 每个文件包含 15-30+ 个函数，全部在同一个命名空间中
```

**证据 B: 新功能永远向这个包添加文件**

```bash
$ ls -la forge-core/cmd/forge/*.go | wc -l
# 从 v2 启动时的 7 个文件增长到 17 个

$ git log --oneline -- forge-core/cmd/forge/ | wc -l
# 超过 60% 的 sprint 都在这个包内新增或修改文件

# 每次新引擎（mode/risk/migrate/attribution/yamlpath）都在 internal/ 中落地，
# 但 wiring/assembly/CLI 代码全部堆在 cmd/forge/ 中
```

**证据 C: `internal/` 包遵守单向依赖，但 `cmd/forge` 导入所有包**

```bash
$ head -20 main.go
import (
    "forgeos/forge-core/internal/asset"
    "forgeos/forge-core/internal/converge"
    "forgeos/forge-core/internal/gate"
    "forgeos/forge-core/internal/orchestrator"
    "forgeos/forge-core/internal/yaml2json"
    # ... 总计 8+ internal 包
)
# cmd/forge 导入所有 internal 包，但 internal 包从不导入 cmd/forge（✓）
# 问题是 cmd/forge 自身没有分层——所有代码在同一包中平铺
```

**证据 D: 两个 `reportConvergence` 函数——同样的概念，不同的实现**

```go
// gates.go — cmd/forge 版本的 reportConvergence
func reportConvergence(wf asset.Workflow, root string, ...) { ... }

// loop.go — orchestrator 版本的 reportConvergence  
func (l LoopEngine) reportConvergence(sig converge.Signals) { ... }
```

两个函数名称相同、概念相同但实现不同、位于不同的包。`cmd/forge` 的版本没有 `FileDelta` / `CodeTestRatio` 警告（已在 `high-value-extension-directions-v3.md` 方向四中记录为「收敛警告可见性」gap，但未追溯到其根本原因——这个 gap 本身就是包内聚性缺失的症状）。

### 与 100+ 已有分析的区别

| 已有分析 | 与本文的差异 |
|---------|-------------|
| 所有 31 篇 `docs/requirements/*.md` | 没有一篇讨论**包级别**（而非文件级别）的内聚性。所有分析关注的是文件行数闸门、函数长度闸门——`cmd/forge` 的每个文件都遵守 500 行限制 ✅，但包级别从未被审视 |
| `forgotten-five-foundations.md` 中的「可插拔 Executor/Gate 扩展框架」 | 讨论的是用户如何写自己的 Executor/Gate 插件，不是 CLI 胶水层的内聚性 |
| `novel-architectural-extensions-v40.md` 中的「编排器 Hook 契约」 | 讨论 Engine 的 callback 接口，不是 `cmd/forge` 的组装方式 |
| `structural-gaps-v41.md` 中的「Go 库 API 边界契约」 | 讨论如何将 `internal/` 包暴露为可导入的库，不是如何分解 CLI 层自身 |
| `expansion-core-five.md` 中的「自适应工作流引擎」 | 讨论引擎逻辑的演化，不是 CLI 胶水层的架构债务 |
| **所有 100+ 方向** | **零方向讨论 `cmd/forge` 自身的包级架构治理。这是整体指导原则关注文件而忽视包的盲区。** |

### 为什么需要

1. **每个 sprint 的增加成本** —— 当前架构中，80% 的新功能需要在 `cmd/forge` 中添加 at least:
   - 一个 CLI 子命令 handler
   - 引擎 wiring 代码（`buildRunEngine` 新增字段）
   - prompt 注入代码（`prompt_context.go` 新增 lane）
   - 信号提取代码（`gatherSignals` 新增字段）
   
   所有这些在同一个包中平铺，增加了认知负荷和合并冲突概率。

2. **这不是「拆分 main.go」的问题** —— ForgeOS 已经做了拆分。当前 17 个文件各自负责不同领域（`cost.go` 处理成本、`evolve.go` 处理循环、`gates.go` 处理信号），但**缺少包级别的领域边界**——它们都在 `package main` 中共享命名空间。

3. **影响 north-star 演进** —— 如果将 `cmd/forge` 拆分为服务（`forge-routerd`、`forge-orchestratord`、`forge-gated`），当前的包内聚性意味着这些服务的骨架需要被重写，而非通过裁剪现有 package 来创建。

### 建议的架构方向

不改变现有代码的执行路径，仅重构 `cmd/forge/` 的包结构：

```
当前:
  cmd/forge/ (17 文件, ~12,500 行, package main)
    main.go, engine_build.go, evolve.go, gates.go, cost.go,
    prompt_context.go, prompt_memory.go, prompt_artifacts.go,
    validate.go, preflight.go, detect.go, detect_parsers.go,
    approve.go, route.go, migrate.go, scorecard_wind.go

建议:
  cmd/forge/          (薄 CLI 分发层, ~3 文件)
    main.go           (子命令分发 + 入口, <300 行)
    flags.go          (共享 flag/option 类型, <200 行)
    usage.go          (帮助文本, <200 行)

  internal/runner/    (工作流运行器——从 cmd/forge 中提取)
    run.go            (execEngine + execLoop)
    build.go          (buildRunEngine + agentExecutor)
    signals.go        (gatherSignals + reportConvergence)
    budget.go         (runBudget + costEmitter)

  internal/prompt/    (prompt 构建——从 cmd/forge 中提取, 业务已在此)
    (已有 prompt/builder 概念, 将 cmd/forge 中的 prompt_context 下沉)
```

核心原则：
- **不做行为变更** —— 纯提取 + 重导入，零逻辑修改
- **`internal/runner/` 包**持有引擎装配和运行逻辑（当前在 `engine_build.go` + `main.go` 中）
- **`internal/prompt/` 扩展**接收当前在 `cmd/forge/prompt_*.go` 中的 CLI 胶水
- **`cmd/forge/` 减薄**为只做 CLI 分发、flag 解析和委托给 `internal/runner/`

**验收标准**：
- `cmd/forge/` 文件数 ≤ 5，总行数 ≤ 2,000
- 每个新包有清晰的领域边界（单一责任）
- `forge run/evolve/gate/check/accept` 行为逐位不变（`forge accept` 仍 ACCEPTED）
- 新包遵守 `internal/` 的单向依赖纪律（不导入 `cmd/forge`）

---

## 方向二 · 双执行轨道孤岛：pi-batch.py 在 forge-core 治理外独立运行

> **类型**: 功能扩展 · 平台一致性  
> **优先级**: P1（产品能力缺口——两个执行系统各自为政）  
> **预估工作量**: ~2 sprints（pi-batch 改为 forge-core 支持的工作流）  
> **杠杆系数**: ⭐⭐⭐⭐

### 现状

ForgeOS 仓库中存在**两个完全独立的 AI 执行系统**：

```
执行系统 A: forge-core (Go)
  ┌─ 入口: forge run/evolve
  ├─ 工作流: YAML 定义 (discover/design/review/build/evolve)
  ├─ 治理: gates / arch-check / secret-scan / converge
  ├─ 可观测: trace.jsonl / memory.jsonl / scorecard
  ├─ 宿主: Claude Code / 其他 agent CLI
  └─ 零外部依赖: 纯 Go 标准库

执行系统 B: pi-batch.py (Python)  
  ┌─ 入口: python3 pi-batch.py <tasks.yaml>
  ├─ 工作流: 自定义 YAML 格式 (tasks[].prompt + tasks[].model + tasks[].output)
  ├─ 治理: 无（零 gates、零 checks、零收敛）
  ├─ 可观测: 纯 stdout 日志（零 trace、零 memory、零 scorecard）
  ├─ 宿主: Claude（通过 subprocess 调用 claude CLI）
  └─ 自己解析 argparse + YAML + ThreadPoolExecutor
```

### 代码级证据

**证据 A: pi-batch.py 是一个 500+ 行的独立 Python 脚本**

```bash
$ wc -l pi-batch.py
500 pi-batch.py

$ head -30 pi-batch.py
"""
pi-batch -- serial/parallel batch executor for pi agent.

Usage:
  # From YAML task file
  python pi-batch.py tasks.yaml

  # Parallel execution
  python pi-batch.py tasks.yaml --mode parallel --workers 4

  # Serial execution (default)
  python pi-batch.py tasks.yaml --mode serial
"""
# 独立 CLI 设计——自己的 argparse, 自己的 YAML 解析, 自己的 ThreadPoolExecutor
# 与 forge-core 共享: 零
```

**证据 B: pi-batch.py 实现了自己的线程池执行引擎，与 forge-core 的并行引擎完全重复**

```python
# pi-batch.py: 并行执行
def run_parallel(tasks, workers=4):
    with ThreadPoolExecutor(max_workers=workers) as executor:
        futures = {executor.submit(execute_single, t): t for t in tasks}
        for future in as_completed(futures):
            task = futures[future]
            ...

# forge-core: 并行波执行 (internal/orchestrator/parallel.go)
func (e Engine) RunParallel(ctx context.Context, ...) error {
    waves := topoSort(phases)
    for _, wave := range waves {
        var wg sync.WaitGroup
        for _, p := range wave {
            wg.Add(1)
            go func(...) {
                defer wg.Done()
                ...
            }(p)
        }
        wg.Wait()
    }
}
```

**两个执行引擎完全相同的目的（并行运行 AI 任务），但完全不同的实现、不共享治理、不共享可观测性。**

**证据 C: 本仓库的 docs 生成大量使用 pi-batch.py，但不经过 forge-core 的治理**

```bash
$ find docs/requirements/ -name "*.md" | wc -l
41

$ grep -l "pi-batch" docs/requirements/*.md | wc -l
# 一些分析文档提到 pi-batch.py 作为扫描的一部分，但从未质疑它的「治理外」状态
# 实际上，这些分析文档本身就是通过 pi-batch.py 批量生成的
```

**证据 D: forge-core 无已知 pi-batch.py，反之亦然**

```bash
$ grep -rn "pi.batch\|pibatch" forge-core/ --include="*.go"
# → 零结果（forge-core 不知道 pi-batch.py 存在）

$ grep -rn "forge\|gate\|trace\|converge\|scorecard" pi-batch.py
# → 零结果（pi-batch.py 不知道 forge-core 存在）
```

两个执行系统在同一个仓库中共存，但**彼此完全不可见**。

### 与 100+ 已有分析的区别

| 已有分析 | 与本文的差异 |
|---------|-------------|
| `systemic-expansion-v26.md` 第 160 行 | 仅提到 pi-batch.py 有「不同的 CLI 风格」，作为一个例子，从未作为集成缺口讨论 |
| `novel-five-perspectives-2026-07-10.md` 方向二「事件驱动/定时执行平面」 | 讨论的是**为 forge-core 添加外部事件触发器**（cron/webhook），不是整合并行执行系统 |
| `expansion-horizon-three.md` 方向三「外部事件驱动触发器」 | 同上 |
| 所有其他文档 | pi-batch.py 被列在「扫描范围」中（与 examples/ `.forge/` 平级），但**没有一个分析将其内部实现作为一个集成缺口单独审视** |
| **本文的独特视角** | 这不是「加一个新功能」，这是**整合仓库中已存在的两个独立 AI 执行系统**。这是一个产品级缺口：ForgeOS 有两个执行层但用户只能择一使用。 |

### 为什么需要

1. **产品一致性** —— ForgeOS 的核心承诺是「统一治理层」。当同一个仓库中存在一个完全在治理之外的 AI 执行系统时，这个承诺被削弱。用户必须选择: 使用 forge-core 的有治理但串行/无并行批处理能力，或使用 pi-batch 的无治理但有并行能力。

2. **可观测性断裂** —— pi-batch.py 执行的任务产生 stdout 输出，没有任何 trace、memory、scorecard 记录。对于长时间运行的批处理分析（如本仓库生成 41 份分析文档的场景），这意味着:
   - 无法回答「这个批量任务花了多少钱？」
   - 无法回答「哪个模型被用于哪个任务？」
   - 无法从批处理输出中提取「经验教训」注入 memory
   - 无法用 gate 验证批量输出质量

3. **并行引擎已存在但 pi-batch.py 不知道** —— forge-core 已有 `--parallel` 波执行引擎（`parallel.go` + `waves.go`，`-race` 验证）。pi-batch.py 的 `ThreadPoolExecutor` 实现了一种不同的并行模型。如果整合，pi-batch 任务可以受益于 forge-core 的治理 + 并行。

### 建议的架构方向

将 pi-batch.py 的 YAML 任务格式转化为 forge-core 可消费的工作流：

```
当前:
  pi-batch.py tasks.yaml                 # 独立执行
  → 每个 task: prompt + model + output  # 无治理

桥接方案 (建议 ~1 sprint):
  forge run batch --tasks tasks.yaml     # forge-core 执行
  → 每个 task = phase                    # 按 forge-core 工作流执行
  → gates 可选 (--skip-gates 用于勘探)
  → trace 记录每个 task 的模型/成本/时长
  → memory 注入每个 task 的关键洞察

远期 (建议 ~2 sprints):
  .agent/workflows/batch.yml             # 正式工作流定义
  phases:
    - name: batch-analysis
      agent: researcher
      model: sonnet
      batch: tasks.yaml                  # 展开为多个并行 phase
      parallel: true                     # 使用 forge-core 的波执行引擎
```

核心原则:
- **不破坏 pi-batch.py 的现有功能** —— 保持向后兼容（`python3 pi-batch.py tasks.yaml` 继续可用）
- **添加 forge-core 桥接命令** —— `forge run batch` 读取 pi-batch 的 YAML 格式
- **逐步淘汰独立执行** —— 在新功能中优先使用 forge-core 执行
- **trace 覆盖** —— 每个批处理 task 在 trace.jsonl 中记录为独立事件

---

## 方向三 · 进程级健康检查端点缺失

> **类型**: 运维 · 可靠性  
> **优先级**: P1（24h 无人值守运行的操作性先决条件）  
> **预估工作量**: ~0.5 sprint（`forge health` 子命令 + HTTP endpoint）  
> **杠杆系数**: ⭐⭐⭐⭐

### 现状

ForgeOS 的 `forge run` 和 `forge evolve` 是长时间运行的进程（设计目标 24h+）。但当前没有任何机制可以回答「forge-core 进程是否健康？」:

```
$ forge run evolve build --executor command --agent-cmd claude
# 启动后进程在前台运行 24 小时
# 外部监控 (systemd / Docker HEALTHCHECK / k8s livenessProbe) 可以检查:
#   ❌ 进程是否存在? → 是的, PID 存在 (但如果是死锁/goroutine 泄漏呢?)
#   ❌ 是否接受请求? → 无 HTTP, 无 socket, 无信号
#   ❌ 是否有进展? → 无健康报告端点
#   ❌ 最后一次 checkpoint 何时? → 不在健康检查范围内
```

### 代码级证据

**证据 A: 所有健康相关的能力都是 CLI 子命令，不是进程级端点**

```go
// validate.go — forge doctor / forge status / forge preflight
// 全部是 CLI 子命令，需要 fork 一个新进程来运行
// 不能由当前正在运行的 forge 进程自身暴露

func cmdDoctor(args []string) int { ... }     // 静态诊断
func cmdStatus(args []string) int { ... }      // 当前快照
// forge preflight → 运行前检查环境
// 但没有:
//   func healthEndpoint()  // 运行中健康检查
//   func livenessProbe()   // k8s liveness probe
//   func readinessCheck()  // 运行就绪性检查
```

**证据 B: 运行中进程可以检查的只有 PID 存在性**

```go
// forge-core 没有任何信号监听器或 socket listener
// main.go — 唯一的信号处理是 SIGINT/SIGTERM → 优雅关闭
signal.Notify(ch, syscall.SIGINT, syscall.SIGTERM)
// 没有 SIGHUP → 重载配置
// 没有 SIGUSR1 → 报告健康状态
// 没有 SIGUSR2 → dump 运行时状态
```

**证据 C: `cmd/forge` 事实上是一个短命令（即使它运行长时间）**

```go
// main.go — forge run 和 forge evolve 是阻塞的前台进程
// 它们没有后台模式（无 daemon 模式）
// 没有 fork 到后台
// 没有 PID 文件
// 没有 systemd notify 支持
```

**证据 D: 文档已承认这个缺口但从未作为独立方向**

```bash
$ grep -rn "health.*endpoint\|liveness\|readiness.*probe\|HEALTHCHECK" docs/ --include="*.md" | grep -v "健康契约\|health.*checklist"
# 零结果
# 「健康契约」(production-readiness.md) 讨论的是 agent 层面的健康（agent 输出格式契约）
# 非进程层面的健康
```

### 与 100+ 已有分析的区别

| 已有分析 | 与本文的差异 |
|---------|-------------|
| `expansion-production-readiness.md` 方向三「环境验证与预检查」 | 讨论 `forge preflight`（运行前检查 Node/Python/claude 是否存在），不是运行中健康检查 |
| `forgotten-five-foundations.md` 方向一「跨进程运行时状态守护」 | 讨论文件锁 + PID 文件（防止两个 forge 冲突），不是进程健康信号 |
| `novel-five-frontiers-v34.md` | 在 goroutine 泄漏上下文中提及「无进程级健康监测」，但仅作为背景，非独立方向 |
| `expansion-directions-v14-operational-trust.md` | 讨论 `forge serve` HTTP server 作为 API 入口，不是健康检查端点 |
| `genuine-uncovered-five-binary-state.md` | 讨论 `forge daemon` 作为守护进程（Unix socket 通信），但聚焦于缓存共享和跨命令协作，不是健康检查 |
| **所有 100+ 方向** | **零方向将「运行中进程健康检查端点」作为独立方向。最接近的是 daemon 讨论，但 daemon 是新的进程模型，健康检查是现有进程模型的运维要求。** |

### 为什么需要

1. **24h 自治运行的运维基础** —— 如果一个 24h 的 `forge evolve` 因为 goroutine 泄漏在第 12 小时开始降级，但 PID 仍然存活，外部监控无法知道。systemd 的 `Type=simple` 只检查 PID 存在性，不检查应用层健康。

2. **Docker/K8s 部署的前提** —— ForgeOS 的 north-star 架构包含分布式微服务。任何容器化部署都需要 `HEALTHCHECK` 指令或 `livenessProbe` / `readinessProbe`。当前 forge-core 二进制无法提供这些。

3. **当前的 `forge doctor` 信息丰富但不可被外部消费** —— `forge doctor` 可以诊断项目健康（workflow 引用、agent 卡完整性、版本一致性），但它是 CLI 命令，不是进程内置的健康端点。一个运行中的 `forge evolve` 无法被 `forge doctor` 检查。

### 建议的架构方向

轻量级健康检查，对现有代码零侵入：

**第一层: SIGUSR1 信号处理器 (0.2 sprint)**

```go
// main.go — 在现有 SIGNAL 处理中增加
signal.Notify(ch, syscall.SIGINT, syscall.SIGTERM, syscall.SIGUSR1)
// SIGUSR1 → 在 stderr 打印健康报告并继续运行:
//   - 当前阶段/迭代
//   - 最后成功的 checkpoint 时间
//   - 内存使用 (runtime.ReadMemStats)
//   - goroutine 数量
//   - trace 事件计数
```

**第二层: HTTP 健康端点 (0.3 sprint, 可选)**

```
forge run --health-addr :9191     # 启动 HTTP 健康端点
  GET /healthz → 200 OK (alive)
  GET /readyz → 200 OK (准备好运行 phase)
  GET /status → JSON 状态快照 (同 forge status --json)
```

**或者**（更保守）: Unix socket 文件:

```
$FORGE_REPO_ROOT/.forge/health.sock  → Unix domain socket
# 外部可以通过 socat 或 curl --unix-socket 查询
```

核心原则:
- **零行为变化** —— 默认不监听任何端口/信号（向后兼容）
- **opt-in** —— `--health-addr` 或 `--health-socket` flag 启用
- **与现有诊断复用** —— HTTP 端点返回的数据等同于 `forge status --json` + `forge doctor` 的运行时信息

**验收标准**:
- `forge run build --health-addr :9191` 另启 HTTP 端点
- `curl localhost:9191/healthz` → `{"status":"alive","uptime_seconds":123,"iteration":3,"last_checkpoint_unix":...}`
- 端点对运行中进程的影响 < 1ms 延迟
- 所有现有 `forge accept` 检查仍 ACCEPTED

---

## 方向四 · Prompt ContextCache 可观测性盲点

> **类型**: 可观测性 · 资源管理  
> **优先级**: P2（当前不影响正确性，但随运行时间增长变成资源泄漏）  
> **预估工作量**: ~0.5 sprint（添加 metrics + eviction）  
> **杠杆系数**: ⭐⭐⭐

### 现状

`internal/prompt/cache.go` 的 `ContextCache` 是一个精心设计的运行期缓存——它在一次 `forge run`/`forge evolve` 的全过程中缓存不变的 prompt 上下文（agent 卡文本、ADR 标题集、AGENTS.md 约束）。但它是一个**无界、无度量、无驱逐策略**的纯增长缓存。

### 代码级证据

**证据 A: cache.go 当前只有一个度量标准——`builds` 计数器，且只用于测试断言**

```go
// cache.go:48-50
type ContextCache struct {
    // ...
    builds int  // 纯测试仪器——生产中从未读取
}

// 没有:
//   - hitCount / missCount（缓存命中率）
//   - memoryUsage（内存占用）
//   - entryCount（缓存条目数）
//   - invalidationCount（失效次数）
```

**证据 B: `loadCards` 和 `adrDocs` 随运行时间单调增长**

```go
// cache.go — 缓存会增长，永不收缩
func loadCards(repoRoot string) map[string]string {
    // glob .agent/agents/*.md → 读取所有卡 → 存入 map
    // 如果新 agent 卡在工作流运行中被添加 → 下次 lanz build 新条目
    // 但旧条目永不删除
}

// cache.go — cardText map 没有大小限制
type ContextCache struct {
    cardText map[string]string // 每个 agent 卡的完整文本
    // 对于有 12 个 agent 卡的系统，这是 ~50KB
    // 如果 agent 卡数量增长到 50+（多项目共享），这是 ~200KB+
    // 不是内存问题——但完全不可见
}
```

**证据 C: `ContextCache.Invalidate()` 存在但从未被调用**

```go
// cache.go — Invalidate 是 v2 钩子，v1 从不调用
func (c *ContextCache) Invalidate() { ... }
// 注释明确说「v1 NEVER calls this」
// 这意味着在单个 24h evolve 运行中，缓存只能建立条目，不能逐出或重置
```

**证据 D: 缓存的命中率完全不可观测**

```go
// 唯一可以推断缓存行为的途径是:
//   - 观察 `builds` 计数是否 > 1（如果 > 1 说明缓存被重建，= bug）
//   - 观察文件 I/O 次数（通过 strace / 性能分析器）
// 没有程序化的方式从 forge-core 内部查询缓存状态
```

### 与 100+ 已有分析的区别

| 已有分析 | 与本文的差异 |
|---------|-------------|
| `novel-architectural-extensions-v40.md` 方向一「Gate 执行经济学」 | 讨论 gate 结果的缓存（同 phase 同输入下复用 gate 输出），不是 prompt 上下文的缓存 |
| `expansion-directions-v20.md` 中的「缓存一致性」讨论 | 讨论 memory 缓存的 cache miss → 全量重读（文件 I/O 问题），不是 ContextCache 的度量 |
| `expansion-strategic-v5.md` 中的「YAML 解析缓存」 | 讨论 YAML → JSON 解码的缓存，不是 prompt 上下文缓存 |
| `forgotten-five-foundations.md` 方向五「运行时状态自校验」 | 讨论 checkpoint/memory/trace 的 cross-file 一致性，不是缓存度量 |
| `genuine-uncovered-five-binary-state.md` 中的 daemon 缓存 | 讨论 Unix socket 缓存共享（多进程共享缓存），不是单个缓存的可观测性 |
| **所有 100+ 方向** | **零方向将 `internal/prompt/cache.go` 的度量和逐出策略作为独立方向。缓存的存在和正确性在 Sprint 30 的 FUNCTIONAL_REQUIREMENTS_AUDIT 中确认，但它的可观测性从未被审视。** |

### 为什么需要

1. **可观测性是 ForgeOS 的第一原则** —— 系统记录了 `trace.jsonl` 中每个事件的 `DurationMs`/`CostUsdMicros`/`Model`，scorecard 中每个 model 的 p95 延迟和平均成本。但最主要的**性能优化设备**——ContextCache——完全没有度量。缓存命中率是高还是低？它在节省 I/O 吗？无人知晓。

2. **24h 运行的资源泄漏** —— 当前 `cmd/forge` 在运行开始创建一个 `ContextCache`，在运行结束时丢弃它。对于 24h 运行，这没问题。但如果有跨运行共享的缓存（daemon 模式），或如果 `loadCards` 的 `filepath.Glob` 遍历大量文件，缓存成长为 unbounded 是一个无声的资源泄漏。

3. **无法回答「缓存是否有效」** —— 当团队优化 prompt 构建路径时（如添加 `ContextCache`），需要知道缓存是否实际减少了文件 I/O。当前唯一的答案是「我们在测试中验证了 `builds == 1`」——这是一个正确性检查，不是性能度量。

4. **这是面向 north-star 的基础设施** —— 当 forge-core 变为分布式服务时，prompt context 缓存将需要在服务间共享。那时的缓存需要有命中率、驱逐策略和资源限制——这些能力应该在当前的单进程缓存中先建立。

### 建议的架构方向

**第一层: 缓存度量 (~0.2 sprint)**

向 `ContextCache` 添加纯粹的观测计数器（零行为变化）：

```go
type ContextCache struct {
    // ... 现有字段 ...
    
    // CallCount 记录了 GatherCached 被调用的次数（总调用数）
    CallCount int64
    // ADRDocsHitCount 记录了 adrDocs 从缓存服务的次数（vs 从文件系统重建）
    ADRDocsHitCount int64
    // CardTextCallCount 记录了 CardText 被调用的次数
    CardTextCallCount int64
    // LastResetTime 记录了缓存上次被创建/重置的时间
    LastResetTime int64
}
```

通过 `forge status --json --cache` 或 trace 事件 `kind: "cache_report"` 暴露这些度量。

**第二层: 随时间变化的命中率 (~0.1 sprint)**

```
格式: [kind: "cache_report"] 在运行结束时或定期发出
{
  "seq": 42,
  "kind": "cache_report",
  "detail": "ContextCache: call_count=47, adr_hits=46/47(97.9%), card_calls=94, age_seconds=3721"
}
```

**第三层: 可选的大小限制 (~0.2 sprint, future)**

```go
type ContextCacheConfig struct {
    MaxADRDocs int  // 0 = unlimited (current)
    MaxCards   int  // 0 = unlimited (current)
}
```

核心原则:
- **零行为变化** —— 度量收集是纯附加的，不影响现有执行路径
- **轻量级** —— 只有几个 int64 计数器和一次运行结束时的日志输出
- **为 daemon 模式铺路** —— 当缓存跨运行共享时，度量和限制已经就位

**验收标准**:
- `forge run build` 完成后 trace.jsonl 包含一个 `kind: "cache_report"` 事件
- 缓存的命中率在同一 run 的所有 phase 后可计算
- `forge accept` 仍 ACCEPTED（零行为变化）
- 测试验证 `builds` 计数器的正确性（已有）+ 新度量计数器的正确性

---

## 优先级汇总

| # | 方向 | 优先级 | 类别 | 工作量 | 核心差异化 |
|---|------|--------|------|--------|-----------|
| 1 | **`cmd/forge` 包级别内聚性债务** | **P1** | 架构债务 | ~1.5 sprints | 所有已有分析只关注文件级（500 行）和函数级（50 行）治理，从未审视包级内聚性。17 个文件/12,500 行的 `package main` 是「上帝包」，不是「上帝文件」 |
| 2 | **双执行轨道孤岛: pi-batch.py vs forge-core** | **P1** | 功能扩展 | ~2 sprints | pi-batch.py 被 10+ 篇分析列为「扫描范围」但从未作为集成缺口。仓库中存在两个完全独立的 AI 执行系统，彼此不可见 |
| 3 | **进程级健康检查端点缺失** | **P1** | 运维/可靠性 | ~0.5 sprint | 跨进程守护（方向已有）讨论的是文件锁，健康检查讨论的是 supervisor（systemd/Docker/k8s）如何知道 forge-core 进程健康 |
| 4 | **Prompt ContextCache 可观测性盲点** | P2 | 可观测性 | ~0.5 sprint | 所有分析讨论缓存的正确性，零讨论缓存的可观测性。ForgeOS 的「可观测性优先」原则在这里有一个被忽视的盲点 |

### 推荐实施顺序

1. **方向三（健康检查端点）** — 成本最低（~0.5 sprint），收益最高（打开 Kubernetes/Docker 部署路径）。与现有代码零冲突，可独立完成。

2. **方向一（包内聚性）** — 最大的长期收益。不影响外部功能，但对内部可维护性的改进是乘数级的。建议在方向三之后立即进行，因为增加的命名空间隔离使后续所有扩展更便宜。

3. **方向二（pi-batch 集成）** — 最大的产品可见收益。统一两个执行系统使 ForgeOS 的「统一治理」承诺真实落地。需要方向一先完成（因为 `cmd/forge` 需要干净的结构来添加 batch runner 子命令）。

4. **方向四（缓存度量）** — 可独立进行，不依赖其他方向。建议在方向三之后并行推进。

---

## 排除的方向（已有 100+ 分析已充分覆盖）

| 候选方向 | 已有覆盖文档 | 排除理由 |
|---------|-------------|---------|
| 跨项目治理策略漂移 | `novel-five-perspectives-2026-07-10.md` 方向一 | 已完整展开 |
| 事件驱动/定时执行平面 | `novel-five-perspectives-2026-07-10.md` 方向二 | 已完整展开 |
| 非确定性收敛停滞诊断 | `novel-five-perspectives-2026-07-10.md` 方向三 | 已完整展开 |
| 跨会话学习/知识迁移 | `novel-five-perspectives-2026-07-10.md` 方向五 | 已完整展开 |
| YAML 双解析器差分测试 | `high-value-extension-directions-v3.md` 方向一 | 已完整展开 |
| 迭代不变 phase 跳过 | `high-value-extension-directions-v3.md` 方向三 | 已完整展开 |
| 并行引擎工作流适配 | `high-value-extension-directions-v3.md` 方向五 | 已完整展开 |
| Go 库 API 边界契约 | `structural-gaps-v41-genuinely-unexplored.md` 方向一 | 已完整展开 |
| 测试质量元治理 | `structural-gaps-v41.md` 方向二 + `five-genuinely-uncovered-frontiers.md` | 已覆盖 |
| 韧性验证/混沌工程 | `structural-gaps-v41.md` 方向三 | 已完整展开 |
| 配置 Schema 版本化 | `structural-gaps-v41.md` 方向五 | 已完整展开 |
| 跨进程运行时守护 | `forgotten-five-foundations.md` 方向一 | 已完整展开 |
| 治理资产热加载 | `forgotten-five-foundations.md` 方向二 + `genuine-architectural-gaps-v28.md` 方向二 | 已覆盖 |
| 信号源可靠性分层 | `genuine-architectural-gaps-v28.md` 方向一 | 已完整展开 |
| 跨相位产物版本一致性 | `genuine-architectural-gaps-v28.md` 方向五 | 已完整展开 |
| 产物质量治理 | `structural-gaps-v41.md` 方向四 | 已完整展开 |
| Phase 级确定性回放 | `genuine-architectural-gaps-v28.md` 方向三 | 已完整展开 |
| Agent 能力协商分配 | `genuine-architectural-gaps-v28.md` 方向四 | 已完整展开 |
| 跨进程守护/文件锁 | `forgotten-five-foundations.md` 方向一 | 已完整展开 |
| Gate 执行经济学 | `novel-architectural-extensions-v40.md` 方向一 | 已完整展开 |
| `forge plan` 执行计划预览 | `novel-architectural-extensions-v40.md` 方向四 | 已完整展开 |
| 编排器通用 Hook 契约 | `novel-architectural-extensions-v40.md` 方向五 | 已完整展开 |
| 收敛信号证据链 | 多篇分析覆盖 | 已充分覆盖 |
| 多仓库联邦编排 | `expansion-horizon-three.md` 方向二 | 已完整展开 |

---

## 结语

这 4 个方向的共同特征: 它们都是**系统已存在结构中的缺口**，而非「加一个新引擎」或「实现一个新功能」:

- **方向一（包内聚性）** 发现已有的治理纪律（文件行数/函数长度）在包级别不存在
- **方向二（pi-batch 孤岛）** 发现两个执行系统在同一个仓库中独立运行
- **方向三（健康端点）** 发现 24h 自治运行缺少运维基础设施
- **方向四（缓存盲点）** 发现可观测性原则在最重要的性能优化设备上未被应用

它们修复后都使系统更健壮，而非更大。
