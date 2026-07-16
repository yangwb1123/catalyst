# ForgeOS — 高价值扩展方向（全局扫描 v11）

> **角色**: 资深架构师 / 产品经理  
> **扫描范围**: forge-core (Go ~7.3k non-test LOC + 11.2k test LOC) + harness (Node/Python ~10.5k LOC) + `.agent/` 声明层  
> **方法**: 全局代码扫描 + 跨层追踪 + 已有 20+ 份分析文档的盲区分析  
> **原则**: 不重复已经被充分论证的方向（信号处理、CI 补完、WASM 门、跨厂商路由）  
> **产出**: 5 个未被已有分析文档充分覆盖的高价值方向

---

## 方向总览

| # | 方向 | 类型 | 代码影响 | 紧急度 |
|---|------|------|---------|--------|
| 1 | **Agent 身份与权限模型** | 核心安全 | forge-core + workflows | 🔴 P0 |
| 2 | **并行引擎fail-fast短路** | 性能/成本 | `parallel.go` | 🟠 P1 |
| 3 | **确定性 Replay 调试引擎** | 工具链 | `trace` + `persist` + new cmd | 🟡 P2 |
| 4 | **Memory 衰减/去重/可溯源** | 数据质量 | `internal/memory` + prompts | 🟡 P2 |
| 5 | **Meta-Test：ForgeOS 自测缺失链路** | 治理 | harness + CI | 🟢 P3 |

---

## 方向 1: 🔴 Agent 身份与权限模型

### 现状

当前所有 agent 角色（planner、implementer、reviewer、qa、architect …）在路由和 prompt 层面的**行为完全相同**，差异仅在于：
- `routing.TierFor(agent, mode)` 的 Opus 硬下限（architect/cto/reviewer → Opus）
- 构建 prompt 时注入的角色卡不同
- `required_when` 决定该 phase 是否被 mode gating 跳过

**不存在真正的权限边界**：
- `internal/orchestrator/command_executor.go` 的 `Sandbox *SandboxConfig` 是空骨架（v3 占位），当前所有 agent 在**同一主机**运行
- `prompt_context.go` 的 `defaultAgentAllowedTools` 对所有 agent 统一授予 `Bash(node --test*)` + `Bash(node harness/gate.mjs*)`，不区分 agent 角色
- `agent-permission` 是全局统一的 `acceptEdits`，没有「implementer 可写代码、reviewer 只读」的强制区分——reviewer 拿到 `acceptEdits` 权限理论上也能改文件
- `SandboxConfig` 是 struct 声明但**零消费**：没有任何地方检查或使用它

### 为什么需要

这是**最危险的安全盲区**。当前体系建立在「agent 会遵守角色卡」的信任上，没有任何机制层强制：

1. **Reviewer 可写代码**：reviewer 的 `claude -p --permission-mode acceptEdits` 与 implementer 完全一致。一个不诚实的 reviewer agent 可以偷偷改文件，绕过「fresh-context 独立审查」的安全假设。当前唯一阻止它的是 agent 自发的诚实——这不够。
2. **无沙箱隔离**：所有 agent 共享主机文件系统。implementer 写文件时可以直接修改 `.agent/ROADMAP.md` 把自己的完成度标 100%，RoadmapCompletion 信号自报告，没有独立验证。
3. **SandboxConfig 骨架化**：v3 路线图的「Firecracker 沙箱」当前是空 struct。但设计上没有预留从「无沙箱」到「按角色沙箱」的渐进迁移路径——等真要实现时会发现所有代码路径都假设 agent 在本地运行。

### 已验证的缺口

```
# promopt_context.go 中的统一 whitelist
const defaultAgentAllowedTools = "Bash(node --test*) Bash(node harness/gate.mjs*)"
# → 所有 agent 共用，reviewer/qa 也拿到 gate 读权限

# command_executor.go 中的沙箱骨架
type SandboxConfig struct { Type, Image string; MemoryMB, TimeoutSec int }
# → 声明了但不消费：Execute() 从不读 c.Sandbox

# build.yml 的 reviewer phase
- name: reviewer
  agent: reviewer
  readonly: true           # ← 这只是声明，不是强制
  fresh_context: true      # ← 同样只是声明
```

### 建议

**Phase A（低改动）**:
- 在 `CommandExecutor.Execute()` 中增加 `AllowedTools` 字段，允许**按角色**配置 agent 可用的 Bash 白名单。implementer 可 `node --test*`，reviewer 白名单为空（只读）。
- `agentPermission` 增加 `readonly` 选项：reviewer/qa 强制用 `--permission-mode plan`（只描述不施加编辑）。

**Phase B（中改动）**:
- 建立 `Authority` 接口：`CanWrite(path) bool`、`CanExecute(path) bool`，按 agent 角色和 phase 类型裁决写权限。
- 在 `engine_build.go` 的 `Build` 函数中注入权限裁决——implementer 写 `src/` 通过、写 `.agent/ROADMAP.md` 拒绝。

**Phase C（未来）**:
- 按角色路由到不同沙箱：reviewer 在只读容器中运行、implementer 在带网络隔离的容器中。

### 风险

- **低**：Phase A 是纯新增限制，向后兼容（不设白名单时行为不变）
- **中**：Phase B 需要定义什么是「不可写区域」，初期可能漏或过度限制

---

## 方向 2: 🟠 并行引擎 fail-fast 短路

### 现状

`internal/orchestrator/parallel.go` 的 `runWave` 使用标准 `sync.WaitGroup`：

```go
for _, idx := range wave {
    wg.Add(1)
    go func(i int) {
        defer wg.Done()
        if err := e.runPhaseParallel(...); err != nil {
            // 记录 firstErr + waveCancel
        }
    }(idx)
}
wg.Wait()
```

**问题**：如果波内第 1 个 phase（gate phase）在 2 秒后 FAIL，剩余 4 个 agent phase 仍然跑满全程（每个可能 10-60 秒 + 对应美元成本）。这些 phase 的输出会被丢弃（因为整个波返回 error），但钱已经花了。

### 为什么需要

并行编排的**核心价值**是加速，但当前实现把「加速」和「浪费」绑在一起：

| 场景 | 波 1（5 并发） | 浪费 |
|------|---------------|------|
| gate FAIL @ 2s | 剩余 4 agent 跑满 30s | ~$2.00 + 2 分钟 |
| `context.Canceled` 波 3 | 已 spawn 的 agent 继续跑 | 取决于 Timeout |
| 多轮 evolve 迭代 | 每轮重复浪费 | 线性放大 |

该问题在 `docs/analysis/edgecases-and-perf.md §1.1` 已记录但 **从未修复**。ForgeOS 的 dogfood `examples/url-shortener` 是单 agent 串行 workflow，没暴露此问题，但真实并行 workflow（如 `discover.yml` 的 3-phase fan-out、`evolve.yml` 的 scan/gap-analysis fan-out）将直接受害。

### 建议

将 `sync.WaitGroup` 替换为 **errgroup** 或**手动 context 取消 + 已完成计数**：

- 使用 `golang.org/x/sync/errgroup`（唯一的合理外部依赖引入点；or Go 1.20+ 的 `errors.Join` + 手动 select）
- 每个 goroutine 在启动时检查 `waveCtx.Err()`，已被 cancel 则直接 return
- `WaveCancel()` 在 firstErr 设置时立即调用，保证后续还未启动的 goroutine 直接退出

```
// 改动示意图（仅 parallel.go 的 runWave 函数，~10 行）
g, gCtx := errgroup.WithContext(waveCtx)
for _, idx := range wave {
    i := idx
    g.Go(func() error { return e.runPhaseParallel(gCtx, wf, i, mode, ...) })
}
return g.Wait()  // 第一个 error 自动取消其他 goroutine
```

### 风险

- **低**：`errgroup` 是 `golang.org/x/sync` 的子包，Go 准标准库，唯一的增量外部依赖
- **低**：改动只影响 `parallel.go`，序列路径字节级不变

---

## 方向 3: 🟡 确定性 Replay 调试引擎

### 现状

ForgeOS 有完整的 trace 系统（`internal/trace`）记录每个事件的时间戳和详情，也有 `persist.Checkpoint` 支持 resume。但 trace 是**只读观测**的：

- trace.jsonl 记录的是「跑完后的事实」，不能用来「重放当时的 prompt」
- checkpoints 只是粗粒度的迭代/phase 索引 resume，不保存 agent 的输入/输出
- 一个 crash 后，调试者只能看 trace + memory.jsonl 猜测哪里出错，**无法重放当时发给 agent 的完整 prompt**

### 为什么需要

这是**可调试性**的根本差距：

1. **Agent 行为不可重现**：`claude -p <prompt>` 的输出依赖于 model 版本、temperature、甚至当天 API 负载。一个 bug 能复现一次就再难复现。
2. **无 golden test 集**：无法保留一组「已知好」的 agent 响应来做回归测试。ForgeOS 改一行 prompt 模板后，无法判断输出质量是变好还是变坏。
3. **审计完整性缺口**：真点火跑完后，trace 只记录 "ran phase X: output=..." 的 summarize，不保留完整的 `prompt → agent_output` 对。在需要审计 agent 决策时（如「为什么 reviewer APPROVE 了一个坏实现」），无法回溯到原始 prompt。

### 建议

**Phase A：Record & Replay（低改动）**

在 `executor` 层增加一个 `RecordingExecutor`（装饰器模式）：

```
type RecordingExecutor struct {
    inner AgentExecutor
    store ReplayStore  // 记录 prompt + output + metadata
}

func (r *RecordingExecutor) Execute(ctx, phase, mode) error {
    prompt := buildPrompt(phase, mode)  // 捕获此时 prompt
    err := r.inner.Execute(ctx, phase, mode)
    r.store.Save(ReplayEntry{
        Phase: phase,
        Mode:  mode,
        Prompt: prompt,
        Output: captured_output,
        Error:  err,
        Timestamp: time.Now(),
    })
    return err
}
```

记录在 `.forge/replay/` 下，按 workflow + 时间戳分目录。replay 数据非 load-bearing（不影响运行），只在 `--record` 或 CI 中启用。

**Phase B：Replay Playback（中改动）**

新 CLI：`forge replay <workflow> [--replay-from .forge/replay/<dir>]`

- 读取 replay 记录
- 跳过真 agent 调用（用记录的 output 代替）
- 运行完整 gates 验证是否与记录时的结果一致
- 用于 CI 回归测试：可断言「在相同输入下，系统行为没有退步」

### 风险

- **低**：Phase A 纯新增，零向后兼容影响
- **中**：replay 数据可能很大（完整 prompt + output）。需要合理 cap（每个 entry ≤ 1MB，自动滚动删除）
- **低**：record 仅在 `--record` 时启用，默认不产生 I/O 开销

---

## 方向 4: 🟡 Memory 衰减/去重/可溯源

### 现状

`internal/memory` 已经是一个成熟的知识存储：

- `Append` / `Load` / `Query` / `Prune` / `Compact` 全部实现
- `Confidence` 字段已存在（默认 1.0）
- `Supersedes` 字段已存在（支持条目更新）
- `filterSuperseded` 已实现

**但存在三个未被触及的问题**：

1. **无衰减加权查询**：`Query` 是精确匹配过滤器（kind + topic），返回所有匹配条目。没有时间衰减、置信度加权或按相关性排序。一个 100 轮前的旧洞察与刚写入的洞察在 prompt 中权重相同。
2. **无去重**：同一个 gap 可能被多个迭代写入多次。`filterSuperseded` 只处理**主动标记 Supersedes** 的条目，不检测重复。一个 implementer 连续 5 轮都写 `Topic="dependency XYZ outdated"`，prompt 中就会出现 5 条类似的条目。
3. **Source 不可溯源**：`Source` 字段已声明（`source,omitempty` 标记来源 agent），但 `BuildPrompt` 查询 memory 时不区分来源——implementer 自己写的 memory 和 reviewer 写的 memory 在 prompt 中不分主次。这意味着 agent 可以「自己强化」：implementer 写下一个错误的 self-assessment，然后同一轮的后续 phase 读到它，以为它是外部确认。

### 为什么需要

Memory 是 ForgeOS **学习闭环的核心**。当前它更接近一个 append-only bucket，而不是一个智慧的知识库：

1. **衰减缺失意味着知识永远不会「过时」**。一个 50 轮前的架构决策（那时系统是单体）在 50 轮后（系统已拆分微服务）仍然以相同权重出现在 prompt 中——浪费 token 并可能误导 agent。
2. **去重缺失意味着 token 浪费 + 假共识**。5 条「deadline 紧张」的重复 memory 会让 agent 以为「所有人都觉得 deadline 紧张」，而实际上只是同一个 agent 重复写了 5 次。
3. **来源不可溯源意味着自我强化无法检测。** 迭代 i 的 implementer 写 `Topic="arch_is_clean"`，迭代 i+1 的 reviewer 读到它并降低警觉。没有人记录这条 memory「是 implementer 自己写的、不是独立评估」。

### 建议

**Phase A（低改动）**:
- `memory.Query()` 增加 `SortBy` 选项（`created_at_unix` 从新到旧、`confidence` 从高到低）
- `BuildPrompt` 在注入 memory 时按时间排序（最新的前 10 条），而不是全量注入
- 为每个 Entry 增加 `EntryHash string`（对 Topic+Detail 取 SHA256），`Append` 时检测最后 N 条中是否有相同 hash——有则跳过（简单去重）

**Phase B（中改动）**:
- 在 `BuildPrompt` 中对 memory 做**来源标记**：implementer 写的 memory 前缀 `[self]`，reviewer 写的 `[verified]`，让 agent 能区分信息来源的可信度
- 增加**衰减加权**：`Query` 返回 `(Entry, weight float64)` 权重对，`weight = confidence × decay(age)`，prompt 注入时按权重大小排序

### 风险

- **低**：Phase A 仅影响 memory 的**消费端**（`BuildPrompt` / `Query`），不改变存储格式
- **中**：去重 hash 可能误杀「表达不同但核心相同」的条目（如 `"deadline tight"` vs `"deadline is tight"`）

---

## 方向 5: 🟢 Meta-Test：ForgeOS 自测缺失链路

### 现状

ForgeOS 声称自己在 dogfood，但自测存在系统性的缺失链路：

| 测试层 | 当前状态 | 缺口 |
|--------|---------|------|
| **Go 单元测试** | 13 包全覆盖 | ✅ 好 |
| **harness 自测** | `test_gate.mjs` + `test_acceptance.mjs` + `test_check.py` + `test_sca.mjs` … 全在 `node --test harness/` | ⚠️ 但不跑在 CI |
| **CI pipeline** | `.github/workflows/forge.yml` | ⚠️ 不跑 `-race`、不跑 `node --test harness/` |
| **app-test（dogfood 测试）** | `examples/go-taskd` + `examples/url-shortener` 的 app 测试被 acceptance gate 包含 | ✅ |
| **Meta-test（测试测试）** | 谁测试 test_acceptance.mjs 测测试框架本身的正确性？ | ❌ **缺失** |
| **Gate 自身 gate** | 谁来确保 gate 不误报？ | ❌ **缺失** |

具体来说：

1. **CI 不跑 `-race`**：并行引擎 `parallel.go` 有复杂的锁序合约（`parallel.go` 文件头部万言书），但 CI 中的 `go test ./...` 不带 `-race`。一个数据竞争 bug 可能在开发环境被发现，但 CI 不会拦截。
2. **Ci 不跑 harness 自测**：CI 只跑 `forge accept`，但 `forge accept` 在缺少一些工具时会 N/A（如适配器 lint/coverage），而这些 N/A 可能掩盖 harness 本身的退化。如果 `test_sca.mjs` 有一个突变导致一个 SCA 漏洞被漏报，CI 不会发现。
3. **无 gate-mutation 测试**：如果 `gate.mjs` 的体积检测有一个 off-by-one 错误（501 行才报、500 行不报），当前自测覆盖不到。没有一个「故意制造一个 501 行的文件，确认 gate FAIL」的测试。
4. **无集成测试覆盖 `forge evolve` 的 checkpoint/resume**：`test_evolve.mjs`（如果有）不跑 checkpoint 恢复路径。crash → resume 是最高价值路径之一，但没有被测试覆盖。

### 为什么需要

ForgeOS 的本质是一个「治理系统」——它确保被治理的代码符合红线。**如果治理系统自己的测试有盲区，那盲区就是所有被治理项目的共同脆弱面**：

- 一个 CI 不跑的 `-race` bug 流入生产后，可能在 `forge evolve --parallel` 中触发死锁（见 `parallel.go` 锁序合约），卡死整个软件工厂
- 一个 gate 的 N/A 降级逻辑如果被错误地改成「所有缺失工具都算 PASS」，所有消费该 gate 的项目都会出现假阳性安全信号
- acceptance.mjs 的诚实性合约（N/A ≠ PASS）如果被意外破坏，收敛引擎会把未检查的 gate 当通过

### 建议

**Phase A（低改动，纯 CI 配置）**:
- `.github/workflows/forge.yml` 增加 `go test -race -count=1 ./...` 步骤
- 增加 `go build ./...` 步骤（显式验证编译，不依赖 acceptance gate 的 N/A）
- 增加 `node --test harness/` 步骤（独立验证 harness 自测，不依赖 `forge accept` 的间接覆盖）
- 增加 `python3 -m pytest harness/test_check.py harness/test_yaml2json.py` 或等效命令

**Phase B（中改动，新增 mutation 测试）**:
- 新增 `harness/test_gate-mutation.mjs`：故意创建超阈值文件，确认 `gate.mjs` exit 1；修复后 exit 0
- 新增 `harness/test_acceptance-honesty.mjs`：通过注入伪造的 probe 输出来验证 N/A 不会被判为 PASS
- 新增 `forge-core/cmd/forge/evolve_integration_test.go`：用 echo executor + fake gate 跑完整 evolve 循环，验证 checkpoint/resume 路径

### 风险

- **极低**：Phase A 是纯 CI 配置，不影响 forge-core 或 harness 一行代码
- **低**：Phase B 的新测试文件只在测试时运行

---

## 执行优先级

| 阶段 | 方向 | 为什么先做 |
|------|------|-----------|
| **Sprint 27-28** | 方向 5（Phase A） + 方向 2 | CI 补全是最高 ROI（零代码改动，立即拦截回归）；并行引擎短路是当前版本的功能缺陷 |
| **Sprint 29-30** | 方向 1（Phase A） + 方向 4（Phase A） | 权限模型是安全基线；Memory 衰减/去重提升知识质量且改动有限 |
| **Sprint 31+** | 方向 3 | Replay 引擎是工具链增强，依赖前述方向稳定后方可设计 Record 格式 |
| **评估中** | 方向 1（Phase B-C） | 沙箱隔离需要运行时基础设施（Firecracker/Docker），与路线图 v3 对齐 |

---

## 与既有分析的关系

| 本方向 | 已有分析 | 是否重复 |
|--------|---------|---------|
| Agent 身份与权限 | 已有分析涉及「路由」和「角色卡」，但未讨论 agent 之间的**互信隔离** | ❌ 新角度 |
| 并行 fail-fast | `edgecases-and-perf.md §1.1` 已记录但未修复 | ✅ 补充实现方案 |
| 确定性 Replay | `expansion-directions.md` 提到「轨迹重放」是外延场景 | ❌ 新角度 |
| Memory 衰减/去重 | `expansion-core-five.md` 提到「记忆置信度」但没涉及去重 | ✅ 补充 |
| Meta-Test 链 | `self-testing-and-dogfooding.md` 谈自我测试质量但没覆盖 CI 盲区 | ✅ 补充 |

---

*生成日期: 2026-07-01 | 基于 forge-core full GO 代码 + harness 代码 + 20+ 已有分析文档的盲区分析*
