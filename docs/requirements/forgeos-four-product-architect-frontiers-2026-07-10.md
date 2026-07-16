# ForgeOS — 四点产品/架构级扩展方向（代码级全局扫描）

> **角色**: 资深架构师 + 产品经理  
> **方法**:  
> 1. 全局逐文件扫描 `forge-core/`（19 Go 包 / ~32k LOC 运行时 + CLI 层）、`harness/`（39+ 模块 / ~7.3k LOC 执法层）、`.agent/`（12 agent 卡 / 9 skill 卡 / 5 工作流 / 全部 policies+ADR+DECISIONS）、`examples/`、`.github/workflows/`、`.forge/` 运行时产物  
> 2. 交叉验证已有 70+ 篇扩展分析文档（`docs/requirements/` + `docs/analysis/`），确认每方向**作为独立系统性方向**未被覆盖。如有子概念在某篇的侧栏被提及，标注出处并说明差异。  
> 3. **纪律**: 不编写任何代码。每方向附精确到 `file:line` 的代码级证据、实际影响、边界场景。  
> **日期**: 2026-07-10

---

## 已有覆盖全景（本文不重复）

| 覆盖域 | 代表篇数 | 本文方向 |
|--------|----------|----------|
| 引擎补齐（编排/路由/记忆/收敛/并行/回灌/AD 层） | ~20 | ✅ |
| 生产可靠性（超时/重试/护栏/进程组/退避/预算） | ~15 | ✅ |
| 执行语义（原子性/幂等/版本/一致性/回滚） | ~12 | ✅ |
| 安全纵深（secret-scan/递归/SCA/prompt 防御/OS 级） | ~12 | ✅ |
| 学习闭环（trace/telemetry/scorecard/自适应） | ~10 | ✅ |
| 治理/执法（arch-check/check.py/漂移守卫） | ~10 | ✅ |
| 运营可信（Run Identity/状态隔离/审计/健康/自省） | ~10 | ✅ |
| CLI DX / 配置 / Shell 集成 / tutorial | ~6 | ✅ |
| 第三地平线（多仓库/事件驱动/Web UI/管道组合） | ~10 | ✅ |
| 元治理 / 自反治理 | ~6 | ✅ |
| HTTP API / SDK 面 | ~3 | ✅ |
| 多 Provider 抽象 | ~5 | ✅ |

**本文 4 个方向落在上述所有覆盖域的间隙中**，是代码级真实存在的结构性缺口而非「加功能」建议。

---

## 方向一 · 实时执行流与可观测性缺口

> **优先级**: 🔴 **P1** | **类别**: 产品 · UX · 可观测性 | **代码影响**: `internal/trace/` · `cmd/forge/` · `harness/`
>
> **差异化证明**: 关键词 `实时/stream/watch/follow/tail/SSE/WebSocket/live update` 在 70+ 篇已有分析中**零作为独立方向**。第七波分析（`seventh-wave-data-realism.md`）讨论的是 trace 事件字段质量，不是「如何让用户看到运行中的进度」。

### 问题描述

ForgeOS 是完全的批处理系统。运行 `forge evolve` 之后，用户只能看到 Log 回调输出的文本行——直到整个迭代完成才看到下一个输出。没有进度条，没有阶段完成百分比，没有「当前正在运行 planner（已用 3s 预计还有 12s）」的实时反馈。

代码证据：

```go
// forge-core/internal/orchestrator/orchestrator.go:303-309
// RunFrom 执行每个阶段，但阶段之间没有任何进度对外推送：
for i := start; i < len(wf.Phases); i++ {
    p := wf.Phases[i]
    if len(p.RequiredGates) > 0 {
        if err := e.runGates(p, e.gatesFor(p)); err != nil { ... }
    }
    // ... 完成后进入下一阶段，没有 emit 任何结构化进度事件
}
```

```go
// forge-core/internal/trace/trace.go:109-127
// Tracer.Emit 是唯一的结构化事件输出——但它是延迟写入 .forge/trace.jsonl：
// 事件只在 COMPLETE 后写入，不是实时推送。且 trace 文件只增不读。
// 没有人能从外部观察到「正在发生什么」。
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    defer t.mu.Unlock()
    // 序列化 + 写入文件。没有广播，没有 WebSocket，没有 SSE。
}
```

```go
// forge-core/cmd/forge/engine_build.go:27-38
// 唯一的外部输出是 Log func(string) —— 自由文本，不可解析。
func agentExecutor(o runOpts, logln func(string), ...) orchestrator.AgentExecutor {
```

对比 CI 系统（GitHub Actions、GitLab CI）都有运行中的日志流、阶段标记、时间戳。ForgeOS 作为一个自治 24h 运行的编排器，**运行时完全是个黑箱**。

### 为什么这被忽略

团队焦点在内核质量（编排正确性、收敛、预算、并行安全）。实时流被认为是「前端问题」——但在 v2 没有 Web UI 的情况下，CLI 是唯一的人机界面。没有一个 CLI 用户愿意等 5 分钟没有任何反馈。

### 边界场景

| 场景 | 影响 | 当前处理 |
|------|------|----------|
| `forge evolve` 跑 10 迭代 × 5 阶段 = 50 agent 调用，每调用 30s-2min | 用户在 15-80 分钟内零反馈 | `Log` 文本行混杂在 stdout |
| 用户按 Ctrl+C 想知道「跑到哪了」 | 无法知道当前在什么阶段 | 只能猜 |
| CI 集成：外部系统想实时显示运行状态 | 无可能 | 只能等 exit code |
| 两个阶段间有 30s 间隙（gate 运行） | 用户以为进程挂死了 | 无进度输出 |

### 建议扩展范围（v1）

- **进度行协议**: 在 `Log` 之外增加 `OnProgress func(phase string, elapsed time.Duration, status string)` 回调。阶段启动/完成/门通过/代理调用等事件作为一个简短的结构化行（含时间戳+阶段名+状态）输出到 stderr，用户可以 `tail -f` 或管道解析。
- **+watch 模式**: `forge evolve --watch` 将 `.forge/trace.jsonl` 作为流输出到 stdout（类似 `tail -f`），让外部工具实时消费。
- **v2**: Unix socket 或 TCP 端口暴露结构化事件流，外部 Dashboard 可以直接订阅。

---

## 方向二 · 增量/差异门执行

> **优先级**: 🔴 **P1** | **类别**: 性能 · 开发者体验 | **代码影响**: `harness/` · `internal/gate/` · `cmd/forge/gates.go`
>
> **差异化证明**: 关键词 `incremental/differential/selective/diff-based/file-level gate` 在 70+ 篇已有分析中**零作为独立方向**。`edgecases-and-perf.md` §2.3 提到了 gate caching（门结果缓存减少重复执行），但那个缓存是在一次 run 内的多次调用间（same run, 同次运行），不是跨 run 的增量执行。本文讨论的是**跨运行、基于文件变更集**的有选择门执行——完全不同的维度。

### 问题描述

每次 `forge run` 或 `forge evolve` 都会执行所有门（lint/build/test/security/complexity/arch）于**整个仓库**。在 1000+ 文件的项目中，只改了一个文件却跑全量检查，本质上是浪费。

代码证据：

```go
// forge-core/internal/gate/resolve.go:37-53
// ResolveGate 为每一个门名执行一次检查，没有文件级别的跳过：
func ResolveGate(repoRoot, name string, probe map[string]string) Result {
    switch name {
    case "complexity":
        return Gate(repoRoot)  // 跑 gate.mjs 扫描整个仓库
    case "arch":
        return Check(repoRoot) // 跑 check.py 检查整个仓库
    case "test":
        return combinedGate(..., "test_pass", "app_test_pass") // 跑所有测试
    // ...
    }
}
```

```javascript
// harness/acceptance.mjs:124-135
// 门的实现是全量的：
export function probeComplexity() {
  const r = run('node', [join(HARNESS_DIR, 'gate.mjs')]);
  // gate.mjs 扫描整个仓库的文件体积
  return result('complexity_violations', r.ok ? PASS : FAIL, ...);
}
```

gate.mjs 没有接收路径参数或排除列表。`check.py` 固定从根目录递归扫描。`secret-scan.mjs` 扫描所有非 `.git` 文件。没有一个门能说「只有 `src/foo.go` 变了，只检查这个文件和它的依赖图」。

更关键的是，ForgeOS 的 `converge` 信号也受此影响：`FileDelta`（`gatherSignals` 中计算）跟踪变动的文件集，但这个信息**只用于报告警告**，从未被门的执行计划使用。

### 边界场景

| 场景 | 低效比 | 当前处理 |
|------|--------|----------|
| 单文件 docs 修改 | 1 文件变，跑 8 道门覆盖 500+ 文件 | ✅ 全量跑 |
| 改一个 `.gitignore` | 触发 complexity+arch+lint+build+test | ✅ 全量跑 |
| 重构阶段：改了 50 个文件但只改函数签名 | 仍跑全量安全扫描（与变更无关） | ✅ 全量跑 |

### 建议扩展范围

- **变更清单**: `gatherSignals` 已计算 `FileDelta`，但未传递给门执行器。在 `harness/` 层面增加一个可选参数（`--changed-files` 或通过 stdin 传 JSON），让感知变更的门只检查相关文件。
- **门-依赖声明**: 每个门在 `adapters/*.yml` 中声明它依赖的文件模式（如 `lint: *.py, *.go`; `test: **/*_test.go`等——目前已有文件但无此字段）。执行时按文件类型过滤。
- **v2**: git diff 驱动的全量/增量选择——`git diff --name-only HEAD~1` 自动决定哪些门要跑全量（安全/架构），哪些可跑增量（lint/测试子集）。

---

## 方向三 · 工作区状态隔离与多实验并行

> **优先级**: 🟡 **P2** | **类别**: 运维 · 产品 | **代码影响**: `cmd/forge/main.go` · `internal/persist/` · `internal/trace/` · `internal/memory/`
>
> **差异化证明**: `expansion-blind-spots-v16` 方向一覆盖的是「多进程并发访问同一个 .forge 目录的竞态窗口」，是**运行安全的子集**（crash-consistency + race）。本文方向三讨论的是 **故意创建多个独立状态工作区** 以支持并行实验——完全不同的用例。关键词 `workspace flag / per-branch state / multi-experiment` 在所有已有分析中**零命中**。

### 问题描述

`.forge/` 目录是整个 ForgeOS 运行时状态的单例。这意味着：
- 你不能同时跑两个 `forge evolve` 在不同的分支上
- 你不能对比「用 balanced mode 跑 5 迭代」和「用 engineering mode 跑 10 迭代」的结果
- 一个实验 crash 的状态文件会污染或覆盖另一个实验

代码证据：

```go
// forge-core/internal/persist/checkpoint.go:193-207
// Save 总是写入同一个 checkpoint.json：
func Save(path string, cp Checkpoint, retain int) error {
    // path = <root>/.forge/checkpoint.json — 永远不变
```

```go
// forge-core/cmd/forge/main.go:316-321
// forgeDir 返回固定路径：
func forgeDir(root string) string { return filepath.Join(root, ".forge") }
// memoryPath 也固定：
func memoryPath(root string) string { return filepath.Join(forgeDir(root), "memory.jsonl") }
```

所有状态（checkpoint、trace、memory、scorecard）都硬编码到 `.forge/` 下。没有任何 `--workspace` 或 `--state-dir` 参数。

```go
// forge-core/cmd/forge/cost.go:117-122
// runBudget 是进程内变量，直接关联到 .forge/ 目录——两个进程共享 .forge/
// 会互相覆盖 runBudget 状态，导致预算跟踪完全不准确。
```

### 为什么这被忽略

ForgeOS 当前定位是「单程序员的单项目工具」，没有考虑团队场景或多实验场景。但 v3 目标明确说要支持「多仓库联邦」——没有工作区隔离，多项目联邦是空中楼阁。

### 边界场景

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 两个工程师在同一仓库上跑 `forge evolve` | state 相互覆盖 | ❌ 无隔离 |
| 同一个工程师同时实验两个不同 feature | 无法对比结果 | ❌ 无隔离 |
| CI 上多个 job 共用同一 checkout | checkpoint 互相污染 | ❌ 无隔离 |
| `forge upgrade` 读旧版本 .forge 文件 | 格式不兼容 | ❌ 无版本检查 |

### 建议扩展范围

- **`--workspace <name>` 参数**: 将 `.forge/` 改为 `.forge/<workspace>/`。工作区名缺省为 branch 名。这样每个分支自然隔离。
- **`forge list-workspaces`**: 列出所有已有工作区及其状态摘要。
- **`forge diff-workspaces`**: 对比两个工作区的输出（roadmap_completion、cost、gate 结果、迭代次数）。
- **锁文件**: 当 `--workspace` 未指定时，`.forge/lock` 防止多进程同时写同一个工作区。

---

## 方向四 · 跨会话门结果持久化与渐近式学习

> **优先级**: 🟡 **P2** | **类别**: 性能 · 学习 · 产品 | **代码影响**: `internal/memory/` · `internal/gate/` · `cmd/forge/gates.go` · `internal/converge/`
>
> **差异化证明**: 已有分析覆盖了 `cross-session memory`（memory 系统）和 `checkpoint`（运行时恢复），但**从未作为系统性方向讨论过「门的执行结果应该在运行之间保持和积累」**。memory 存的是知识（gap/decision/lesson），从不存储门的结果。每次 `forge run` 都是冷启动门执行——即使上周 lint 通过了，今天仍然从零跑 lint。

### 问题描述

ForgeOS 的门执行是完全无状态的。每次运行都重新跑所有门，即使最近一次运行已经跑过且通过了。在迭代式开发中（一天跑 10+ 次 `forge run`），每次都跑全量 lint/build/test/security——绝大多数门的结果与上次完全相同。

代码证据：

```go
// forge-core/cmd/forge/gates.go:54-68
// probeStatuses 每次调用都重新 spawn `node harness/acceptance.mjs --json`：
func probeStatuses(root string) (statuses, categories map[string]string) {
    statuses, categories, err := gate.ProbeAll(root) // 每次 spawn 新进程
    // ...
}
```

```javascript
// harness/acceptance.mjs:61-79
// 每个 probe 函数独立执行子进程，不做任何缓存：
export function probeComplexity() {
  const r = run('node', [join(HARNESS_DIR, 'gate.mjs')]);
  // 每次都 spawn gate.mjs -> 遍历所有文件
}
```

而 memory 系统（`internal/memory/`）虽然能跨会话持久化知识，但只存 gap/decision/lesson，不存 gate 结果：

```go
// forge-core/internal/memory/memory.go:172-197
// Entry 的 Kind 约束为 gap|decision|lesson，没有 gate_result 类型：
const (
    KindGap      = "gap"
    KindDecision = "decision"
    KindLesson   = "lesson"
)
// 没有 KindGateResult
```

唯一接近持久化的机制是 `checkpoint`（`internal/persist/`），但它只保存迭代级的收敛状态（roadmap_completion + gatesGreen bool），不保存每个门的具体通过/失败明细。恢复后 gatesGreen 只是一个 bool——不知道哪个门之前 FAIL 过、为什么。

### 为什么这被忽略

团队假设「每次运行都是完整端到端验证」——这与 `forge run` 的单次语义一致。但 `forge evolve` 的迭代循环和 `forge run` 的频繁使用场景中（调试时一天 20+ 次），全量门执行产生了巨大的重复成本。

### 边界场景

| 场景 | 浪费 | 当前处理 |
|------|------|----------|
| 一天跑 20 次 `forge run` 调试 | 20 次全量 lint+build+test（结果完全一样） | ❌ 无缓存 |
| 门 A 上次通过后文件系统无变化 | 重新跑门 A → 必然同样通过 | ❌ 无缓存 |
| 门 B 上次 FAIL（lint error）修复后但未重新跑 | 不能从上次状态恢复 | ❌ 无跨会话记录 |

### 建议扩展范围

- **门结果存储到 memory**: 增加 `KindGateResult` entry 类型，每次运行后将每个门的结果（名称+状态+时间戳+git commit）持久化到 `memory.jsonl`。后续运行先查「这个门在 HEAD commit 或工作树未变时上次结果是什么」。
- **TTL 感知缓存**: 门结果设定 TTL（lint: 1h, test: 30min, security: 0s 从不缓存，arch: 5min）。TTL 内且工作树未变 → 跳过真实执行。
- **门-文件依赖追踪**: `FileDelta` 已经计算了变更文件集（`gatherSignals`）。将文件变更集与门依赖模式（方向二的门依赖声明）交叉：如果一个门依赖的文件集无变化，且 TTL 未过期 → 复用上次结果。

---

## 优先级总结

| 方向 | 影响面 | 技术风险 | 用户体验价值 | 运维价值 | 推荐优先级 |
|------|--------|----------|-------------|----------|-----------|
| 方向一：实时执行流 | CLI + 可观测性 | 低（Log 基础上的结构化输出） | 🔴 极高 | 🟡 中 | **P1 Sprint N+1** |
| 方向二：增量门执行 | 性能 + Harness | 中（门依赖声明 + diff 驱动） | 🟡 中 | 🟢 极高 | **P1 Sprint N+2** |
| 方向三：工作区隔离 | 运维 + 产品 | 低（路径参数化） | 🟡 中 | 🔴 极高 | **P2 Sprint N+3** |
| 方向四：门结果持久化 | 性能 + 学习 | 低（memory 扩展 + cache 逻辑） | 🟢 低 | 🟢 极高 | **P2 Sprint N+2** |

---

## 与已有分析的关系

以下已有方向与本文方向有边界的接壤，但**核心论点不同**：

| 已有方向 | 本文方向 | 界线 |
|----------|----------|------|
| `seventh-wave-data-realism.md`（trace 事件质量） | 方向一（实时流） | 前者讨论 trace 承载什么字段；后者讨论如何让用户实时看到运行进度 |
| `edgecases-and-perf.md` §2.3（同运行门缓存） | 方向二（增量门） | 前者缓存的是一次 run 内的多次调用同一定门；后者是基于文件变更集的跨运行有选择执行 |
| `expansion-blind-spots-v16.md` 方向一（竞态窗口） | 方向三（工作区隔离） | 前者防御多进程共享 .forge 的 crash 安全；后者是主动创造多工作区以支持并行实验 |
| `go-runtime-health.md` §5.2（cross-session memory） | 方向四（门持久化） | 前者讨论 memory 的遗忘机制；后者是门的结构化结果持久化——数据类型不同 |
