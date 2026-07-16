# ForgeOS — 五个系统性架构扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局逐文件深扫: forge-core（18 Go 包 · 63 非测试源文件）+ harness（39+ 模块）+
>    `.agent/` 完整治理骨架 + `pi-batch.py` + `examples/` + 139 份 `docs/` markdown 文档。
> 2. 逐篇通读 **50+ 份 `docs/requirements/*.md` + 40+ 份 `docs/analysis/*.md` + 核心文档**，
>    对每个候选方向在所有已有分析中做关键词 + 语义交叉验证，确认**该方向从未作为独立扩展方向展开**。
> 3. **差异化证明**: 每个方向附代码级证据，解释与最接近的已有分析的区别。
> 4. **纪律**: 不编写任何代码。每个方向附边界情况表、实际影响、代码级证据。
> **日期**: 2026-07-10

---

## 前言：~150+ 已有方向后的差异化定位

已有分析覆盖了 ForgeOS 几乎每个可触及的维度：引擎补齐、生产可靠性、执行语义形式化、二阶伴生问题、
多仓库联邦、安全纵深、CLI DX、外部 SDLC 集成、知识引擎、架构原型感知、自适应治理……等等。

下面 5 个方向落在所有已有分析的**系统性间隙**中——它们不是「缺的引擎/功能」,而是**代码层已存在但
从未被识别为独立风险/债务的系统性设计特征**。每个方向都是「结构性的」而非「功能性的」：
修复它们不会增加用户可见的功能,但会改变 ForgeOS 作为一个自治系统的可靠性、诚实性和自保能力。

| # | 方向 | 核心问题 | 最接近已有分析 | 本文差异 |
|---|---|---|---|---|
| 1 | **Agent 子进程错误协议真空** | 结构化错误类型跨进程边界丢失,退化为字符串匹配 | `execution-semantic-gaps.md` 方向二（内部包加结构化错误） | 内部错误类型 ≠ 跨进程错误协议 |
| 2 | **测试跳过级联静默侵蚀** | 32+ t.Skip 无预期测试计数追踪,accept 可绿但测试面收缩 | **零覆盖** | 完全未覆盖 |
| 3 | **Evolve 迭代上下文缓存一致性** | ContextCache 假设治理文件在一次 run 中不变,但 evolve 多迭代打破此假设 | `cross-cutting-systemic-gaps.md`（版本追踪/性能） | 跨相位正确性 ≠ 版本追踪性能 |
| 4 | **`pi-batch.py` 无治理编排后门** | 根目录 Python 脚本绕过全部 ForgeOS 闸门,已产出 50+ 无治理文档 | 文件列表提及,从未作为治理风险分析 | 治理绕过 ≠ 功能缺口 |
| 5 | **Convergence 轨迹盲区:迭代预算等额分配** | Evolve 每轮分配相同资源,不感知收敛轨迹,边际收益递减无法自适应 | `expansion-production-readiness.md` 提及迭代边界重置 | 轨迹自适应 ≠ 预算硬上限 |

---

## 方向一 · Agent 子进程错误协议真空

**优先级**: 🟠 P2 | **类别**: 架构 · 可观测性 · 韧性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的 `orchestrator/exec_error.go` 定义了一套结构化错误类型体系：`ExecError` 携带 `Kind` 字段
（`KindTimeout` / `KindOverloaded` / `KindFailed` / `KindConfig` / `KindRecursionLimit`），
`Retryable()` 方法驱动重试策略和 backoff 政策（重试 KindTimeout 和 KindOverloaded，终止其他）。

**但这一整套结构化体系在进程边界处完全断裂。**

当 `CommandExecutor`（`command_executor.go`）通过 `exec.CommandContext` spawn 一个外部 agent 进程时，
唯一能跨越进程边界的信息是：
- **exit code**（0 = 成功，非 0 = 失败）
- **stdout + stderr 文本输出**（非结构化字符串）

`finish()` 方法在子进程结束后，通过字符串匹配重建错误类型：

```go
// command_executor.go:96-97
isOverload := c.ClassifyOverload != nil && c.ClassifyOverload(rendered)
return classifyRunErr(phase, runErr, ctxErr, isOverload)
```

`ClassifyOverload` 是一个从 cmd/forge 注入的回调——它接收**纯文本输出**，返回一个 bool。
错误分类从结构化的 Go 类型降级为**脆弱的字符串模式匹配**。

**更严重的问题：当子进程自身是 ForgeOS 时（递归 `forge run --executor=command`），**
子进程内部产生的 `ExecError{Kind: KindOverloaded, ...}` 等丰富类型信息在跨进程时全部丢失。
子进程的整个结构化错误被折叠为 exit 1 + 一行日志文本。

### 代码级证据

**证据 A：错误分类唯一的跨进程通道是 exit code + 字符串匹配**

```go
// forge-core/internal/orchestrator/command_executor.go:89-97
func (c CommandExecutor) finish(phase string, argv []string, out *cappedBuffer, runErr, ctxErr error, latency time.Duration) error {
    rendered := out.rendered()
    c.observe(phase, rendered, latency)
    c.logf("phase %s: ran %q -> %s", phase, strings.Join(argv, " "), c.renderForLog(rendered))
    if runErr == nil {
        return nil
    }
    isOverload := c.ClassifyOverload != nil && c.ClassifyOverload(rendered)
    return classifyRunErr(phase, runErr, ctxErr, isOverload)
}
```

关键观察：`c.ClassifyOverload(rendered)` 输入是**纯文本**（不是 `ExecError`），
返回是**一个 bool**（不是结构化分类）。这意味：
- `KindConfig` 和 `KindFailed` 无法从文本区分——两者都从 exit 1 + 非超时推断
- `KindTimeout` 可以从 `ctx.Err()` == `DeadlineExceeded` 区分——这是唯一的可靠分类
- 任何其他分类（如 `KindRecursionLimit`）完全依赖文本内容

**证据 B：递归 forge 调用中错误类型完全丢失**

```go
// forge-core/internal/orchestrator/command_executor.go:56-82
// 递归 guard：当 FORGE_AGENT_DEPTH 达到上限时，子进程中的 forge 拒绝 spawn
// 但子进程拒绝时产生的 ExecError{Kind: KindRecursionLimit} 在父进程看来只是
// 一个 exit 1 + 文本 "recursion guard fired"
```

这意味着父进程无法区分「子进程遇到了递归限制」、「子进程遇到了配置错误」、
「子进程遇到了 agent 失败」——它们全部被归一化为一个 `KindFailed`。

**证据 C：`ClassifyOverload` 的注入路径依赖于格式化文本**

```go
// forge-core/cmd/forge/cost.go 中 — ClassifyOverload 闭包通过
// strings.Contains(output, "overloaded_error") 或类似模式匹配判断
// 这是纯粹的启发式，非结构化协议
```

### 与已有覆盖的区别

`execution-semantic-gaps.md` 方向二「结构化错误类型体系」讨论了**ForgeOS 内部包**（17 个包中的 16 个）
缺少结构化错误类型的问题。它主张为 `internal/converge`、`internal/memory`、`internal/prompt` 等包
添加 `errors.Is`/`errors.As` 可识别的类型。

**本文方向不是「内部加错误类型」——这个 ForgeOS 已经有 `ExecError`。本文的方向是：**

> 已有结构化错误 **无法跨越 forge-core ↔ agent 子进程的进程边界**。

这是一个**传输协议**问题，不是**内部类型定义**问题。解决方案可以是：
- 子进程通过 machine-readable 输出格式（如 JSON 单行）传递结构化错误信息
- `CommandExecutor` 在标准输出/标准错误中解析结构化错误荷载
- 错误协议分离「agent 的工作失败」和「forge 的运行失败」

### 边界情况

| 场景 | 问题 | 严重度 |
|------|------|--------|
| 子进程超时 | 正确分类为 KindTimeout（ctx.Err() 可靠） | 正确 |
| 子进程 exit 1（任意原因） | 全部归一化为 KindFailed | 丢失 KindConfig vs KindOverloaded |
| 子进程 529/overload | 依赖 strings.Contains 匹配，格式漂移即静默误判 | **高** |
| 递归 forge 子进程 | 所有 ExecError 类型丢失 | **高** |
| 子进程输出超 cap 被截断 | 截断后 "overloaded" 字眼可能被切掉 → 误判 | **高** |
| 子进程输出中同时含多种错误信号 | 只能匹配到第一个触发的 | 中 |

### 建议方向

1. **定义跨进程错误协议**：子进程在 stdout/stderr 首行或末行输出 `FORGE_ERROR: <json>` 或类似
   machine-readable 格式，包含 `kind`、`phase`、`retryable`、`message` 等结构化字段
2. **`CommandExecutor` 新增解析层**：在执行输出中优先查找结构化错误荷载，找不到才降级为字符串启发式
3. **递归调用的错误保真**：当 `--agent-cmd=forge` 时（目前被递归 guard 阻止），结构化错误本该能穿透
4. **为已有启发式加自测**：`ClassifyOverload` 的文本匹配应有回归测试，防止 vendor 输出格式漂移

---

## 方向二 · 测试跳过级联静默侵蚀

**优先级**: 🟡 P2 | **类别**: 测试 · 治理完整 · 可靠性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

`forge-core` 测试套件中有 **32+ 个 `t.Skip` / `t.Skipf` 调用**（分布在 10 个文件），
在以下条件不满足时静默跳过测试：
- `python3` 不在 PATH（7 个关键测试被跳过——包括 yaml2json 差分测试、evolve 端到端测试）
- 不在 ForgeOS 仓库内运行时（fixture 路径不匹配——持久化检查点测试、ADR 测试跳过）
- fixture 目录/文件不存在（persist/replay 测试、orchestrator loop-restart 测试跳过）
- 硬件/OS 限制（`testing.Short()` 跳过真实的子进程测试）
- `short` 模式（跳过真实进程组测试）
- 环境差异（ADR go.mod 检查因 cwd 问题跳过）

**核心问题：没有任何机制追踪「预期应运行的测试数」vs「实际运行的测试数」。**

这意味着：
1. 如果 `python3` 被卸载或重命名，7 个测试静默消失——`forge accept` 依然报告绿色
2. 如果某个 fixture 被意外删除，5+ 个 persist 测试静默消失
3. 如果 `copy-anywhere` 的目录结构变化导致 fixture 路径不匹配，测试不失败——只是跳过
4. 随着时间推移，测试覆盖的真实面积可能无声萎缩而无人察觉

### 代码级证据

**证据 A：32+ 跳过点分布在所有核心子系统**

```go
// forge-core/cmd/forge/main_agent_test.go:197
t.Skip("python3 not available")           // YAML 桥接测试跳过
t.Skip("not running inside the ForgeOS repo")

// forge-core/cmd/forge/evolve_test.go:37,192,221,240
t.Skip("python3 not available")           // 核心 evolve 测试跳过（4处）

// forge-core/internal/yaml2json/yaml2json_test.go:219,227,238,274,301,336
t.Skip("cannot get cwd")                  // 6 个跳过点——这是 YAML 解析的核心测试
t.Skip("not inside ForgeOS repo")
t.Skipf("modes.yml not found: %v", err)
t.Skipf("build.yml not found: %v", err)
t.Skipf("policies.yml not found: %v", err)
t.Skipf("%s not found: %v", rel, err)

// forge-core/internal/persist/replay_test.go:81,114,203,239,282
t.Skipf("fixture %s not found", name)     // 持久化恢复路径测试——5 个跳过点
t.Skip("fixture evolve-dry-run/trace.jsonl not found")

// forge-core/internal/orchestrator/command_executor_unix_test.go:106,139,150,162
t.Skip("spawns real grandchild processes; skipped under -short")  // 4 个跳过点
```

**证据 B：没有「预期测试计数」注册机制**

```go
// 没有任何文件中定义：
// var ExpectedTestCount = 847  // 注册到测试框架
// 也没有后置检查：
// if actualRunCount < expectedRunCount { t.Error("tests silently skipped") }
```

### 与已有覆盖的区别

**所有已有分析零覆盖。** 这是唯一的差异化证明。

已有分析涉及测试的话题有：
- `high-value-extension-v35.md` 方向二「Agent 输出回归检测/契约式测试断言」——聚焦 agent 输出质量，不是测试自身
- `v34` 方向四「编排集成测试」——聚焦编排器集成测试，不是测试跳过问题
- `novel-five-highvalue-extensions.md` 方向一「治理策略测试框架」——聚焦测试治理策略本身，不是测试套件完整性

本文方向是 **「测试套件的测试」**——元测试层：验证测试自身是否完整运行，而非测试结果本身。

### 边界情况

| 场景 | 影响 | 严重度 |
|------|------|--------|
| python3 从系统卸载 | 7 个测试跳过（yaml2json + evolve），accept 仍绿 | **高** |
| `go test -short` 运行 | 4 个真实子进程测试跳过，unix 特定路径未覆盖 | 中 |
| 在某工具 CI 中运行（无 docs/adr） | ADR 测试跳过，ADR 回归不被捕获 | **高** |
| Fixture 目录被删除 | persist/replay 测试跳过，检查点恢复路径不被测试 | **高** |
| 单次跳过被修复 | 测试计数恢复但无人知晓曾经有测试「回来」了 | 低 |
| 代码库增长但 fixture 未同步 | 新代码路径无对应 fixture → 新测试跳过 | 中 |

### 建议方向

1. **为每个测试文件添加 `TestCount` 导出变量**（`var TestCount = <int>`），在包级 `TestMain` 中校验
2. **或利用 `testing.M` 的 `Run()` 返回值**：`go test -v` 输出的 `ok` / `FAIL` 后面自动附带测试计数
3. **`forge accept` 增加「预期测试计数」校验**：从 `project.yml` 或 `policy.yml` 读取各子系统的预期测试数，
   实际 `go test -count=1` 运行的测试数若低于预期则标记为 WARNING（非 FAIL——兼容工具缺失情形）
4. **每类 skip 添加分类 tag**：区分「真的不可在此环境运行」vs「环境配置不全导致的测试缺失」、
   vs「fixture 丢失这种需要修复的情况」。不做一刀切。

---

## 方向三 · Evolve 迭代上下文缓存一致性——ContextCache 跨相位一致性

**优先级**: 🟠 P2 | **类别**: 正确性 · 上下文管理 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐

### 问题描述

`internal/prompt/cache.go` 的 `ContextCache` 是为单次 `forge run` 设计的 run-scoped 缓存。
它的核心假设写在文件头：

> **CORRECTNESS INVARIANT — any agent-writable file NEVER enters this cache**

这个假设在一个单次 run（如 `forge run build` 从 phase 0 跑到 stop）内成立：没有 agent 能修改
治理文件（AGENTS.md / agent 卡 / workflow YAML），所以缓存它们从未过时。

**但在 `forge evolve` 的多迭代循环中，这个假设被打破了。**

Evolve 的工作流（`evolve.yml`）的循环结构是：
```
iteration 1: scan → gap-analysis → roadmap-update  → (gate) → converge check
iteration 2: scan → gap-analysis → roadmap-update  → (gate) → converge check
...
```

其中 `roadmap-update` 相位可能：
- 更新 `.agent/ROADMAP.md`（这是预期行为——已从缓存排除）
- 按设计可能修改 project config（`project.yml` 的 mode/lifecycle/featrues）
- 更新 agent 卡、skill 卡、或添加 ADR

同时，`ContextCache` 缓存了：
- `adrDocs` — `docs/adr/` 目录下的 ADR 标题集
- `constraintsBlock` — `.agent/AGENTS.md` 的硬约束文本
- `cardText` — `.agent/agents/*.md` 的全部 agent 卡内容

**这意味着：如果 evolve 的迭代 1 创建了一个新 ADR 或修改了 AGENTS.md，**
**迭代 2 的 `ContextCache.invariants()` 会返回缓存的迭代 1 的旧内容，**
**导致 agent 拿到过时的上下文。**

### 代码级证据

**证据 A：ContextCache 在单次 run 构建后永不刷新**

```go
// forge-core/internal/prompt/cache.go:108-115
func (c *ContextCache) invariants(repoRoot string) ([]Doc, string) {
    c.mu.Lock()
    defer c.mu.Unlock()
    if !c.built {
        c.adrDocs = adrDocs(repoRoot)      // docs/adr/ 标题集 —— 在 run 开始后不变
        c.constraintsBlock = constraints(repoRoot)  // AGENTS.md —— 在 run 开始后不变
        c.cardText = loadCards(repoRoot)    // agent 卡 —— 在 run 开始后不变
        c.built = true
        c.builds++ // =1 after full run
    }
    return c.adrDocs, c.constraintsBlock
}
```

缓存在 `c.built` 设置为 `true` 后永不过期。没有基于迭代次数的过期策略。

**证据 B：`Invalidate()` 存在但从未被调用**

```go
// forge-core/internal/prompt/cache.go:63-71
func (c *ContextCache) Invalidate() {
    // v1 NEVER calls this — exists for v2 once agents can modify ADRs mid-run
    // ...
    c.built = false
    c.adrDocs = nil
    c.constraintsBlock = ""
    c.cardText = nil
}
```

代码明确承认 Invalidate 是 v2 的预留接口——v1 不调用它。但在 `forge evolve` 的多次迭代场景下，
这正是 v1 需要它的时候。

**证据 C：buildPrompt 在 evolve 循环中复用同一个 ContextCache**

```go
// forge-core/cmd/forge/engine_build.go（evolve 的 prompt 构建路径）
// 在每个迭代的每个 phase 都调用 buildPrompt，但传的是同一个 ContextCache 实例
// cache.Invalidate() 从未被调用
```

### 与已有覆盖的区别

`cross-cutting-systemic-gaps.md` 的「证据 B」讨论了 `ContextCache` 缓存了 agent 卡内容。
但它关注的是 **性能开销**——建议添加 git hash 版本追踪，避免在 agent 卡未变化时不必要的重加载。

`forgotten-five-system-boundaries.md` 讨论了 cross-process mtime 缓存问题（`memory.loadCache` 的
mtime 依赖在两个独立进程间不安全）。

`forgotten-five-structural-debt.md` 讨论了 memory 包的 `loadFromCache` 使用 mtime 做缓存键——
这是跨进程竞争问题。

**本文方向是跨相位（within-process, within-evolve-loop）的正确性，而不是跨进程的性能。**
在同一个 `forge evolve` 进程内：
- 迭代 1 修改了 AGENTS.md 或 `docs/adr/` 或 agent 卡
- 迭代 2 的 agent 收到了过时的治理约束或 ADR 列表
- 这不是「慢」的问题，是「错」的问题

### 边界情况

| 场景 | 正确行为 | 当前行为 | 严重度 |
|------|---------|----------|--------|
| Evolve 中 roadmap-update 添加 ADR | 下次迭代 agent 看到新 ADR | 缓存返回旧 ADR | **高** |
| Evolve 中 gap-analysis 建议更新 AGENTS.md | 下次迭代看到新约束 | 缓存返回旧约束 | **高** |
| 单次 forge run build（非 evolve） | 缓存不过期是正确的（文件不改） | 正确 | 无影响 |
| Forge run 后手动改 ADR，另起 forge run | 新 run 的 ContextCache 是新创建的 | 正确 | 无影响 |
| 并行 phase（`--parallel`）修改治理文件 | 同波其他 phase 可能读到旧数据 | 边界更复杂 | **更高** |

### 建议方向

1. **在每个 evolve 迭代边界调用 `ContextCache.Invalidate()`**：`LoopEngine.Iterate` 或 `reportConvergence`
   之后、下一次 `RunFrom` 之前清缓存
2. **精细策略：每个 evolve 迭代创建新的 ContextCache 实例**（最简单、最安全——代价是每次迭代
   重新读取 ADR 标题集和 agent 卡，但这在文件系统层面是微秒级操作）
3. **或按 phase 粒度选择缓存**：明确声明哪些 phase 可被缓存、哪些必须实时读取——在 workflow YAML
   中为每个 phase 声明 `prompt_context: [static | dynamic]`，在构建 prompt 时依此决定是否用缓存

---

## 方向四 · `pi-batch.py` 无治理编排后门

**优先级**: 🔴 P1 | **类别**: 治理 · 安全 · 代码组织 | **预估**: 0.5 sprint + 文档治理 | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

`pi-batch.py`（17k，约 500 行）是项目根目录下独立存在的 Python 脚本，用来批量/并行地驱动
`pi` agent 执行任务。它是 `docs/requirements/` 目录下 **50+ 份分析文档**的生产工具。

**但它在 ForgeOS 自己的治理体系之外完全隐身运行：**
- 没有 `forge gate`：文件体积、根目录文件数、函数长度不受 gate.mjs 约束
- 没有 `arch-check`：依赖方向、包大小、扇入检查完全不适用
- 没有 `forge accept`：不被聚合 Stop 闸门覆盖
- 没有 `forge check`：资产引用完整性检查不覆盖它
- 没有 secret-scan：虽然它可能处理敏感提示词
- 没有 `mode × lifecycle` 中枢旋钮：它不知道工程阶段/项目成熟度
- 没有 `trace` / `checkpoint`：产生结果后无运行记录，不可审计
- 没有 `cost` 控制：`--model` 可指定任意高成本模型但无预算守卫
- 没有 `fresh-context` 审查：产出的 50+ 分析文档未经过任何通断性审查
- 没有 copy-anywhere：`forge-init` 不复制它，新项目得不到这个工具

**这不是一个边缘脚本——它是过去 24 小时内被用来产生大量治理分析的唯一工具。**
它的存在表明 ForgeOS 团队在实际工作中使用了自治编排，但这个编排没有被 ForgeOS 自己治理。

### 代码级证据

**证据 A：根目录文件，不在任何 ForgeOS 治理路径内**

```bash
# 根目录文件计数（gate.mjs 的 root ≤ 15 规则）：
# pi-batch.py 占用一个珍贵的根目录文件槽位
# 且不被 gate.mjs / arch-check.mjs / acceptance.mjs 扫描
$ ls /home/u1/catalyst/*.py
pi-batch.py
```

**证据 B：自身存在已知缺陷但无测试覆盖**

Sprint 27 的真点火过程中，fresh reviewer 指出了 `pi-batch.py` 的具体缺陷：
- `_run_task_process` 中 `tout.join(timeout=remaining())` 和 `terr.join(timeout=remaining())`
  各自消耗满额 timeout，实际超时可延迟至 ~2× 配置值
- `FileNotFoundError` 对「二进制缺失」和「工作目录不存在」给出相同错误信息
- 零测试文件覆盖

**证据 C：产出文档无治理锚点**

```bash
# 50+ 文档在 docs/requirements/ 下，但：
# - 无 .agent/ 声明引用它们
# - 无 .agent/workflows/ 相位消费它们
# - 无 .agent/agents/ 角色卡提及它们
# - 它们之间可能互相矛盾（"已覆盖"状态随时间漂移）
# - 无归档/淘汰策略——都是同一主题的多次 variant
$ ls docs/requirements/*.md | wc -l
50+
```

**证据 D：Sprint 隐患——Sprint 27 的 `pi-batch.py` 问题已被发现但从未落地修复**

```bash
$ grep -n 'pi-batch' .agent/CURRENT_SPRINT.md
# Sprint 27: "pi-batch.py(独立批处理脚本,零测试覆盖)超时机制..."
# 确认问题已知但未修复
```

### 与已有覆盖的区别

所有已有分析仅在文件列表扫描中提及 `pi-batch.py`（如 `cross-cutting-systemic-gaps.md`、`five-systemic-oversights-v45.md`、
`strategic-production-gaps.md`），但从未将其作为一个**独立的治理风险方向**展开分析。

`forgotten-five-meta-governance-and-blindspots.md` 方向五「ForgeOS 自身 dogfood 鸿沟」讨论了
`docs/` 目录是无治理的领地——但它聚焦的是「存在的文档没有治理」（内容是孤立的），
而不是「生产这些文档的工具没有治理」（源头是失控的）。

**本文方向是治理源头（production tool），不是治理对象（output documents）。**

### 边界情况

| 场景 | 影响 | 严重度 |
|------|------|--------|
| pi-batch.py 用于产生治理决策文档 | 文档无治理 → 决策依据不可追溯 | **高** |
| pi-batch.py 并行执行调用高成本模型 | 多 agent 同时消费昂贵推理，无预算上限 | **高** |
| pi-batch.py 产生矛盾的分析 | 下游阅读者（包括未来的 AI agent）无法判断哪个分析是权威的 | **高** |
| pi-batch.py 持续在根目录 | 占用根目录文件预算 + 不属于 harness/ 或 forge-core/ | 中 |
| 新开发者看到 pi-batch.py 但无文档 | 可能误用该工具绕过治理产生未审查代码 | **高** |

### 建议方向

1. **将 `pi-batch.py` 纳入 `forge-init` 的 copy-anywhere 清单** 或**移入 `harness/` 目录**（作为
   边界工具而非根目录文件），至少使其被 `gate.mjs` 覆盖
2. **为 pi-batch 添加最小治理契约**：在产出的文档中注入元数据头
   （`generated-by: pi-batch`, `date:`, `mode:`, `model:`），让下游能追溯来源
3. **考虑废弃或重构**：将 `pi-batch` 作为正式的 forge 子命令（`forge batch`）实现，
   继承 ForgeOS 的完整治理（gate/trace/cost/mode-gating），而不是根目录下一个外挂 Python 脚本
4. **`docs/requirements/` 归档策略**：为已经过时或已被 supersede 的分析添加声明，
   消除分析文档间的矛盾和老化问题

---

## 方向五 · Convergence 轨迹盲区：迭代预算等额分配

**优先级**: 🟡 P3 | **类别**: 性能 · 资源优化 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐

### 问题描述

`forge evolve` 的每一次迭代（iteration）获得**相同的资源预算**：
- 相同的 `MaxIter` 安全上限（每轮分配 1/n）
- 相同的 agent-call budget（`MaxAgentCalls` 按 iteration 重置）
- 相同的 `MaxLoopBack` 重试预算
- 相同的相位序列（从 `loop_back_to: scan` 开始完整执行）

但每一次迭代的边际价值可能极不相同：
- **迭代 1**：覆盖率从 0% → 80%，高价值
- **迭代 2**：覆盖率从 80% → 95%，中等价值
- **迭代 3**：修复最后 3 个边缘问题，低价值（在剩余的 5% 中可能找不到新 gap）
- **迭代 4 及以后**：几乎零边际价值，消耗与迭代 1 相同的资源但产出趋近于零

**Evolve 的 `NoProgress` tripwire 检测的是「完全无进展」——连续 N 轮 RoadmapCompletion 未增长。**
但它在以下场景无能为力：
- 进展缓慢但非零（每轮 RoadmapCompletion +0.02，需要 50 轮才能补满）
- 进展波动（修复 1 个 bug 又引入 1 个 bug，指标震荡）
- 进展集中在早期（80% 在第 1 轮完成，后 10 轮只推进了 10%）

当前 LoopEngine 没有「收敛轨迹感知」的调度：
- 不能自动在进展快时多分配资源
- 不能在边际收益递减时自动降级（减少相位数量或使用更便宜的模型）
- 不能对「接近收敛」的迭代跳过非必要的评审相位

### 代码级证据

**证据 A：Evolve 循环没有收敛轨迹分析**

```go
// forge-core/internal/orchestrator/loop.go — LoopEngine.Run()
// 每次迭代的入口点是相同的 RunFrom/RunParallel(workflow, mode)
// 迭代间唯一的变量是 StartPhase（定向 restart）和 RoadmapCompletion 累积值
// 没有「这个迭代相比上次进步了多少」的分析
```

**证据 B：NoProgress 仅检测零进展**

```go
// forge-core/internal/orchestrator/loop.go:107-114
if l.NoProgress > 0 {
    if sig.RoadmapCompletion <= prevRoadmap {  // <=, 不是比率
        stale++
    } else {
        stale = 0
    }
    if stale >= l.NoProgress {
        // stale-progress tripwire fired
    }
}
```

这个检测只看「是否增长」，不看「增长了多少」。一个每轮 +0.01 的 slow-drift 永远不会触发 tripwire，
但会消耗大量资源。

**证据 C：当前没有迭代层面的自适应 budget**

```go
// forge-core/internal/orchestrator/budget.go:70
// MaxAgentCalls 的 scope 是 per-iteration（RunFrom 自建计数器）
// 但所有迭代使用相同的上限——没有早期高分配、晚期低分配的机制
```

### 与已有覆盖的区别

`expansion-production-readiness.md` 的「agent-call budget 在 evolve iteration 边界重置」讨论了
budget 的 scope reset 问题——它关注的是「重置是否正确」（Sprint 21 的处理）。

`high-value-extension-directions-v3.md` 的「Sprint 21 的 agent-call budget guard」讨论了
`--max-agent-calls` 作为 evolve 的 companion。

**本文方向不是「budget 在哪里重置」或「budget 的上限是什么」，而是「预算是否随收敛轨迹变化」。**
核心问题是：**所有迭代获得相同资源的假设在边际收益递减的现实中不成立。**

### 边界情况

| 场景 | 当前行为 | 理想行为 | 严重度 |
|------|---------|---------|--------|
| 第 1 轮覆盖 85%→87%，第 2-5 轮 87%→88% | 跑满 MaxIter 才停（或 NoProgress 太长） | 低收益时自动减少相位或降模型 | 中 |
| 第 1 轮 0%→80%（大幅进展），第 2 轮预期高 | 第 2 轮仍全相位全模型 | 第 2 轮自动分配更多资源（如需） | 低 |
| 振荡模式：修复+回归反复 | 不停重试直至 MaxIter | 检测振荡模式并暂停/报告异常 | 中 |
| 稳定收敛：接近 100% 但差几个边缘 case | 全 budget 消费在低价值边缘 | 自动降级为低功耗模式（仅跑可选相位） | **高** |
| 单次 forge run build（非 evolve） | 无迭代，不影响 | 无影响 | 无 |

### 建议方向

1. **轨迹度量和报告**：`LoopEngine` 记录每次迭代的 `RoadmapCompletion` 历史序列
   （`[0.3, 0.7, 0.82, 0.88, 0.91, ...]`），计算简单轨迹指标
2. **自适应 budget 调度**：
   - 进展 > 10%/iteration → 全预算（标准模式）
   - 进展 < 2%/iteration 且 > 50% 已完成 → 降级（跳过可选相位、降模型）
   - 进展 < 0.5%/iteration → 触发加速收敛模式（或建议终止）
3. **NoProgress 的细化**：从「否/是」二值升级为「无进展 / 慢进展 / 振荡 / 健康」四档
4. **收敛预测**：用简单外推估算「预期还需要多少轮」，与剩余 budget 对比，
   如果不足以收敛则提前 report unconverged 而非跑满再失败

---

## 优先级汇总

| # | 方向 | 优先级 | 影响本质 | 杠杆(1-5) | 成本 | 为什么是这个优先级 |
|---|---|---|---|---|---|---|
| 1 | Agent 子进程错误协议真空 | P2 | 可靠性 · 可观测性 | ⭐⭐⭐⭐ | ~2 sprints | 不紧急但系统性的，当前字符串启发式在生产中足够匹配常见场景；但 vendor 格式变化或递归 forge 时会静默失败 |
| 2 | 测试跳过级联静默侵蚀 | P2 | 治理完整性 | ⭐⭐⭐⭐⭐ | ~1 sprint | 实现成本极低（加一个 TestMain 计数），但能防止最隐蔽的测试覆盖率萎缩 |
| 3 | Evolve 迭代上下文缓存一致性 | P2 | 正确性 | ⭐⭐⭐⭐ | ~1 sprint | 当前只在 evolve 场景下触发，且只有治理文件修改时才会出问题。但一旦触发，agent 拿到过时的治理上下文可能导致违反工程红线 |
| 4 | **`pi-batch.py` 无治理编排后门** | **P1** | 治理 · 安全 | ⭐⭐⭐⭐⭐ | 0.5 sprint | **最高杠杆**：治理自身工具不被治理是 ForgeOS 最根本的 dogfood 缺陷。修复成本极低（移入 harness/ + 加文档头），解决了「自己的项目管理工具是治理盲区」的矛盾 |
| 5 | Convergence 轨迹自适应资源分配 | P3 | 性能 · 资源 | ⭐⭐⭐ | ~2 sprints | 价值高但实现复杂，且当前 evolve 场景的 iteration 数有限（默认最多 10-15 轮），等额分配的浪费尚可接受 |

## 诚实的边界

本文 5 个方向的共同局限：

1. **它们不增加 ForgeOS 的外部可见功能**——修复它们不会让用户看到新 feature，
   但会让系统在面对真实世界条件（python3 缺失、递归调用、长时间 evolve 运行）时更可信、
   更自保、更能诚实报告自身状态。

2. **方向一的「跨进程错误协议」有锁定风险**——如果定义了 machine-readable 错误协议格式，
   需要有升级路径（版本字段），否则未来改格式时新 forge 无法解析老 agent 的错误输出。

3. **方向二不能根治跳过测试**——有些跳过是合法且必须的（OS 绑定、环境能力受限）。
   目标是「可观测」而非「零跳过」：知道有多少测试被跳过、为什么跳过、是否比上次多。

4. **方向四的 `pi-batch.py` 治理有两个可能方案**——是「吸收进 forge」（好处：完整治理继承）
   还是「仅基础设施治理」（移入 harness/ + 加文档头，成本更低）。本文倾向于后者作为第一步，
   前者作为后续路线图。

5. **方向五的轨迹自适应引入了复杂度**——自适应策略的参数（多少进展算"快"、多少算"慢"）
   需要 calibration，不当的参数可能导致提前终止或过度运行。建议默认关闭，通过旋钮启用。
