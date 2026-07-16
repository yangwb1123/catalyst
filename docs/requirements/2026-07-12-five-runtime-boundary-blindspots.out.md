# 审阅：2026-07-12-five-runtime-boundary-blindspots — 五个运行时边界盲区

> **审阅者**: 独立 Reviewer（交叉验证 agent）
> **方法**: 跨文件关键词 grep + 语义比对，覆盖 docs/requirements/ 全部 ~180 篇已有文件 + 代码仓 HEAD
> **日期**: 2026-07-12

---

## 总体评价

本文 5 个方向的共同命题——「机制存在但边界状态未覆盖」——定义精准且可操作。代码引用（`engine_build.go:109-110`、`prompt_artifacts.go:37-55`、`checkpoint.go`、`budget.go:76-90`、`parallel.go:130-140`）经逐行核实全部精确。

**核心声明「每方向 0 篇独立覆盖」**: ✅ 对方向二/四/五成立；🟡 方向一/三有部分间接重叠见下。

| 方向 | 新颖性 | 0 覆盖声明 | 代码引用精度 | 设计建议可操作性 |
|------|--------|-----------|------------|----------------|
| 一 · 厂商无关成本护栏 | 🟢 **95% 新颖** | ✅ 成立 | ✅ 精确 | ✅ 明确（AgentExecutor 扩展 + pre-Execute 拦截） |
| 二 · 声明式输出契约验证 | 🟢 **100% 新颖** | ✅ 成立 | ✅ 精确 | ✅ 明确（phase 后钩子 + trace 记录 + converge 信号） |
| 三 · 三存储一致性审计 | 🟡 **70% 新颖** | ⚠️ 有间接覆盖 | ✅ 精确 | 🟡 需细化（见下） |
| 四 · 预算感知梯度决策 | 🟢 **95% 新颖** | ✅ 成立 | ✅ 精确 | ✅ 明确但需防过工程（见下） |
| 五 · 并发资源争用检测 | 🟢 **100% 新颖** | ✅ 成立 | ✅ 精确 | ✅ 明确且边界诚实 |

---

## 方向一 · 厂商无关的每调用美元成本护栏

**新颖性**: 🟢 **95% 新颖** | **0 覆盖声明**: ✅ 成立

### 交叉验证确认

对 180+ 篇已有文档搜索以下关键词组合：
- `--agent-max-budget-usd` + `vendor` / `agnostic` / `per-call`：零独立覆盖
- `budget` + `vendor-specific` / `vendor-agnostic`：零匹配
- `five dimensions` + `resource guardrails`：文档自家的框架，无前置

唯一间接提及：
- `high-value-extension-directions.md` 表格中将 `--agent-max-budget-usd` 列为已就绪——但如本文指出的，这是**混淆了透传与自有实现**。本文的贡献正是解开这个混淆。

### 代码核实

`engine_build.go:108-111`：
```go
if o.agentMaxBudgetUSD != "" {
    argv = append(argv, "--max-budget-usd", o.agentMaxBudgetUSD)
}
```
✅ 与文档引用完全一致。该代码在 `claudeArgv()` 函数内，仅 claude CLI 路径可达。

`executor.go:26-31` 的 `AgentExecutor` 接口无预算约束——✅ 确认。

### 补充发现

在 `internal/budget/budget.go` 中还有一个 `PhaseBudget` 结构体：
```go
type PhaseBudget struct {
    MaxCostUSD float64 // 0 = unlimited
    CurrentCostUSD float64
}
```
该结构**已定义但未被 `AgentExecutor.Execute` 使用**，只在内部分配器可见。这恰好是本文论证的正面证据：**数据模型存在，但接口层无强制**。可以在 `AgentExecutor` 上增加 `SetCostCap(capUSD float64)` 后用这个已有结构体。

### 对建议的补充

文档建议在 `AgentExecutor.Execute` **之前**拦截。更精确的设计点：
- 拦截位置不在 `AgentExecutor` 方法级，而在 `orchestrator.runPhase` 调用 `CommandExecutor.Build` 和 `exec.Execute` **之间**：先 Build 获取 CLI 指令，在 spawn 前校验成本上限，再 spawn
- 这样即使用户设置 `--agent-max-budget-usd 5` 且 CLI 是自定义的，forge-core 也拦截

---

## 方向二 · 声明式输出契约的自动化验证

**新颖性**: 🟢 **100% 新颖** | **0 覆盖声明**: ✅ 成立

### 交叉验证确认

搜索 `emits` + `verify` / `validate` / `enforce`：零独立覆盖
搜索 `emits` + `WARNING`（`prompt_artifacts.go` 关键行为）：零覆盖
搜索 `declared output` / `output contract` / `artifact manifest`：零匹配

### 代码核实

`prompt_artifacts.go:37-55`：
```go
if err != nil {
    if logln != nil {
        logln(fmt.Sprintf("forge: WARNING emits %q not found (%v)", fullPath, err))
    }
    continue
}
```
✅ 文档描述精确——缺失文件不返回错误，不阻止执行流，不影响 trace。

`asset.go:87-91` 文档（"OPTIONAL" + "当 populated 时 prompt builder 可以读取"）：
```go
// Emits is an OPTIONAL list of file paths that this phase is declared to produce...
```
✅ 确认——语义是「声明」而非「契约」，不存在验证预期。

### 补充发现

`engine_build.go:197-201` 在 readonly narration 中使用 emits 路径——这产生了一个有趣的二级问题：**如果 readonly phase 声明了 emits 但 agent 未创建对应文件，下游的 readonly 权限假设也无效**（下游可能不实际需要写这些路径）。验证 emits 存在性同时也能校验 readonly 权限声明的正确性。

### 对建议的补充

「收敛信号的发射源」是最有价值的建议。可以将 emits 验证的输出格式化为 converge 信号的结构化字段：
```go
type EmitsVerification struct {
    Declared  []string `json:"declared"`
    Found     []string `json:"found"`
    Missing   []string `json:"missing"`
    Untracked []string `json:"untracked"`
}
```
这样 convergence gate 可以直接检查 `emits_verification.missing == []`。

---

## 方向三 · 三存储跨会话一致性审计

**新颖性**: 🟡 **70% 新颖** | **0 覆盖声明**: ⚠️ 有间接覆盖需要澄清

### 间接覆盖

| 文件 | 重叠内容 |
|------|----------|
| `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向三 | 三存储状态生命周期——聚焦**单文件完整性**（文件系统级），但已识别三存储的存在 |
| `forgotten-five-foundations.md` 方向三 | 持久化状态自校验——提出 `forge doctor` 检查文件格式可解析性，触及跨存储但未展开 |
| `novel-extensions-v36-deep-architectural.md` 方向三 | 崩溃恢复状态重建——涉及 trace replay 场景，与本文幽灵 memory（方向三第 2 点）重叠 |
| `forgeos-five-codegrounded-expansion-priorities.md:23` | 提到三存储协调的优先级建议 |

本文的独特贡献在于：**不是单文件完整性，也不是崩溃恢复，而是运行时的交叉一致性**——三个存储各有独立写入路径且无事务边界。这确实没有被任何已有文档作为独立方向专门展开，但**组件级提及**存在。建议将上述 4 篇列为「侧面覆盖」而非「零覆盖」。

### 代码核实

`checkpoint.go` → `persist.Save` 仅写 checkpoint——✅
`trace.go` → `Tracer.Emit` 仅写 trace.jsonl——✅
`memory.go` → `memory.Append` 仅写 memory.jsonl——✅

一个关键的补充证据：**`persist.Save` 的备份机制使用后缀（`.bak1`/`.bak2`）不包含时间戳或序列号**，因此即使在备份中也无法交叉关联到 trace 或 memory 的快照：

```go
// internal/persist/checkpoint.go:~125
os.Rename(path+".bak1", path+".bak2")
os.Rename(path, path+".bak1")
```

备份文件名不含 seq 或 timestamps，三者无任何交叉引用键。

### 对建议的补充

「一致性标记」是最小侵入方案——在 checkpoint JSON 中增加两个 uint64 字段：
```go
type Checkpoint struct {
    // ... 现有字段 ...
    TraceSeq     uint64 `json:"trace_seq,omitempty"`     // 写入 checkpoint 时的 trace 事件计数
    MemorySeq    uint64 `json:"memory_seq,omitempty"`    // 写入 checkpoint 时的 memory 条目计数
}
```
恢复时从 checkpoint 读 seq，然后扫描 trace/memory 起始行验证。零依赖、零额外写入路径变更。

---

## 方向四 · 预算感知的相位执行梯度决策

**新颖性**: 🟢 **95% 新颖** | **0 覆盖声明**: ✅ 成立

### 交叉验证确认

搜索 `gradient` / `tiered` / `watermark` + `budget`：零覆盖
搜索 `budget exhaustion` + `phased` / `graceful` / `degrade`：零覆盖
搜索 `checkRunBudget` + 任何伴生行为：零覆盖

唯一间接相关：
- `routing.go:150-200` 的 `BudgetAdjustTier`（文档已承认）——但它只影响模型选择，不涉及文档提议的完整降档级联

### 代码核实

`budget.go:76-90`：
```go
func (e Engine) checkRunBudget(completed int) error {
    if e.BudgetExhausted == nil || !e.BudgetExhausted() {
        return nil
    }
    return fmt.Errorf("run budget exhausted...")
}
```
✅ 清晰的二元对比——要么 `nil`（继续），要么 error（硬停）。无中间状态。

`loop.go:45-60` 的 `MaxIter`：
```go
// MaxIter is a SAFETY backstop — never the goal
```
✅ 文档精确。`MaxIter` 不是预算感知的。

### 补充发现

`routing.go` 的 `BudgetAdjustTier` 虽然只是模型 tier 调整，但它是**唯一**的预算反馈回路。它的存在证明了设计者已经考虑了「预算影响行为」的概念——只是未扩展到文档提议的完整梯度。这其实降低了方向四的实现成本：可以复用 `BudgetAdjustTier` 的触发点，扩展其影响力到更多维度。

### 对建议的补充

「梯度降档级联」表需要增加一条原则——**所有降档决策必须写入 trace**，且降档必须是**可逆的**：如果预算水位恢复（例如因 `BudgetExhausted` 回调重新校准），升档应自动发生。否则系统可能在 budget 还剩 25% 时永久锁定在低质量模式，因为 `<30%` 降档后 budget 不会自动回到 30% 以上。

---

## 方向五 · 并发相位资源争用检测与缓解

**新颖性**: 🟢 **100% 新颖** | **0 覆盖声明**: ✅ 成立

### 交叉验证确认

搜索 `parallel` + `resource contention` / `file conflict` / `rate limit` / `ulimit`：零覆盖
搜索 `runPhaseParallel` + 任何外部资源维度：零覆盖
搜索 `wave` + `resource budget` / `concurrency limit`：零覆盖
搜索 `depends_on` + parallelism：覆盖了拓扑排序但零覆盖资源维度

### 代码核实

`parallel.go:130-140`：
```go
go func(i int) {
    defer wg.Done()
    if err := e.runPhaseParallel(waveCtx, wf, i, mode, mu, agentCalls); err != nil {
        // fail-fast
    }
}(idx)
```
✅ 每个 phase 独立 goroutine + 子进程，无资源协调。

`waves.go:45-70`：Kahn 拓扑排序仅依赖 `depends_on`——✅
`command_executor.go:110-130`：`cmd.Run()` spawn 子进程——✅

### 补充发现

`parallel.go:25-50` 的锁排序合约文档明确声明只覆盖共享内存状态：
```
// Lock ordering contract (shared memory state only, not external resources)
```
这正是文档引用的证据——设计者明确意识到外部资源不在合约范围内。

另外，`parallel.go:96-98` 的 `maxParallel` 逻辑：
```go
n := len(wave)
if n > runtime.GOMAXPROCS(0) {
    n = runtime.GOMAXPROCS(0)
}
```
当前用 `GOMAXPROCS` 限制并发数——这是 CPU 维度，完全不考虑进程表、文件描述符、API 速率。这既是证据也是切入点的线索：`maxParallel` 已经存在，只需扩展其计算模型。

### 对建议的补充

「写冲突检测」使用 `git status` 快照比对是务实的——但需注意：git 快照在 phase 开始时记录的「受影响文件」是未知的（agent 还没运行）。更实际的方案：**在 wave 开始时对所有被并发 phase 声明的 `emits` 路径和 `.git/index` 做快照，wave 结束时用 `git diff --name-only` 检测重叠修改**。这不需要 agent 的配合，纯 git 操作，零外部依赖。

---

## 优先级重新评估

| 方向 | 文档优先级 | 审阅后评级 | 理由 |
|------|-----------|-----------|------|
| 一 · 厂商无关成本护栏 | P1 | **P1** ✅ 维持 | 架构承诺与实现之间的根本裂痕 |
| 三 · 三存储一致性审计 | P1 | **P1** ✅ 维持 | 24h+ 长跑中数据可信度的底线 |
| 二 · 声明式输出契约验证 | P2 | **P2** ✅ 维持 | 无声降级——严重但不破坏数据 |
| 四 · 预算感知梯度决策 | P2 | **P2 → P3** ⬇ 下调 | UX 改进而非安全/完整性缺口。降档是好事但二元硬停已经在守卫预算不超限 |
| 五 · 并发资源争用检测 | P2 | **P3** ⬇ 下调 | 文档自身诚实标注了 `depends_on` 为零，并行路径休眠。争用检测在并行启用后才有意义 |

### 收敛建议（审阅版）

- **第一优先级**: **方向三 → 方向一**。先做方向三因为它是数据完整性的底线且改动最小（checkpoint 加两个 seq 字段 + `forge doctor` 交叉校验）。再做方向一因为它是安全缺口且代码改动也小（`AgentExecutor` 接口增加预算方法 + `runPhase` 拦截点）。
- **第二优先级**: **方向二**。emits 验证不涉及运行时路径变化，可独立交付。建议在 emits 采用率较高的 workflow（evolve loop 的 planner → reviewer 链）上先落地。
- **方向四和五**维持文档的"高阶优化"定位。方向四在 `BudgetAdjustTier` 已有基础上可以低强度推进降低档水位告警（trace event at 20%/10%），方向五等并行真实启用后再做。

---

## 与已有分析的关系总表

| 方向 | 文档声明的已有覆盖 | 审阅确认的已有覆盖 | 判定 |
|------|-------------------|-------------------|------|
| 一 | `high-value-extension-directions.md`（混淆） | 同上 + 零其他 | ✅ 0 独立覆盖成立 |
| 二 | 3 篇（各自部分重叠） | 同上 + 零其他 | ✅ 0 独立覆盖成立 |
| 三 | 3 篇（各自部分重叠） | **+ `forgotten-five-foundations.md` 方向三 + `forgeos-five-codegrounded-expansion-priorities.md:23`** | ⚠️ 间接覆盖存在，但核心论点（运行时交叉一致性）仍是新的 |
| 四 | 2 篇 | 同上 + 零其他 | ✅ 0 独立覆盖成立 |
| 五 | 3 篇 | 同上 + 零其他 | ✅ 0 独立覆盖成立 |

---

## 结语

本文是 docs/requirements/ 中**代码引用最精确、论证结构最完整**的分析文档之一。五个方向定义清晰，差异化证明扎实，诚实边界部分显著增强可信度。唯一需修正的是方向三的「零覆盖」声明（应改为「零作为独立方向展开，但有组件级间接覆盖」）。

**推荐状态**: ACCEPT（已存在文件，审阅补充为 `.out.md`）
**推荐行动**: 方向三（一致性标记）入 Sprint 32 backlog；方向一（每调用成本护栏）入 Sprint 32 或 33；方向二（emits 验证）入后续 sprint；方向四/五漂移至 backlog 待并行启用。
