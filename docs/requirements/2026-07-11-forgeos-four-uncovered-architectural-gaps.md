# ForgeOS: 基于全局扫描的四个未被覆盖的架构/产品扩展方向

> **角色**: 资深架构师 + 产品经理  
> **方法**: 逐文件通读 forge-core 全部 17 个 Go 包 + cmd/forge CLI 层 + harness 全套 + 全部 workflow/agent 卡  
> **交叉验证**: 与 docs/requirements/ 下 80+ 篇已有扩展分析逐一比对，确保每个方向的核心论点未被已有文档作为独立方向展开  
> **纪律**: 不编写任何代码，每个方向附带代码级证据 + 实际影响 + 边界场景  
> **日期**: 2026-07-11

---

## 方向一 · `.forge/` 运行时目录缺少跨进程协调协议

### 定位

`internal/persist/checkpoint.go` · `internal/trace/trace.go` · `internal/memory/memory.go` · `cmd/forge/main.go:395-398 forgeDir`

### 现状

所有 forge CLI 子进程共享 `<root>/.forge/` 目录作为运行时状态的家目录：

```go
// main.go:395-398
func forgeDir(root string) string { return filepath.Join(root, ".forge") }
```

该目录存放三个关键文件：
- `checkpoint.json` — 迭代/阶段恢复点（`internal/persist/checkpoint.go`），`Save` 使用 tmp+rename 原子写入
- `trace.jsonl` — 结构化可观测性事件流（`internal/trace/trace.go`），多行追加写入
- `memory.jsonl` — 跨会话知识存储（`internal/memory/memory.go`），`O_APPEND` 单行原子写入

**当前没有任何跨进程协调机制：**
- 无 PID 文件或进程锁
- 无文件级 `flock` / `LockFile` 协调
- 无进程标识符写入事件记录供下游溯源
- 无读写冲突检测

### 风险：三个具体的损坏模式

#### 模式 A — checkpoint.json 多进程覆盖

`forge evolve --executor=command` 进程 A（工程师终端）与 `forge run --parallel` 进程 B（CI 构建）同时运行：

```
进程 A: Save(checkpoint.json, cpA)       // 写入迭代 3
               ↓
进程 B: Save(checkpoint.json, cpB)       // 覆盖为迭代 1
               ↓
进程 A 崩溃 → resume → Load(checkpoint.json) → 读到 B 的迭代 1 checkpoint
              → 进度回退 → 重新执行已完成阶段 → 重复计费
```

虽然 `Save` 本身通过 tmp+rename 保证了单次写入的原子性，但两个进程交替 `Save` 会**交替覆盖**同一个文件，没有任何一方能检测到自己写的内容已被另一方取代。

#### 模式 B — trace.jsonl 行交织

`internal/trace/trace.go` 的 `Emit` 在单进程内使用 `sync.Mutex` 保护写入行完整，但跨进程完全无保护：

```
进程 A: Emit({Kind:"agent", Name:"planner", …})\n
进程 B: Emit({Kind:"gate", Name:"test", …})\n
               ↓
文件系统缓冲交织：
  {"seq":1,"kind":"agent",...}\n{"seq":1,"kind":"gate",...}\n
```

结果：下游 `jq` / replay 工具读到**两个 seq=1 的事件**，破坏了 seq 的单调性契约。更严重的是如果写入发生在不同 write(2) 调用之间，可能产生**半行交织**。

#### 模式 C — memory.jsonl 乱序

`internal/memory/memory.go` 使用 `O_APPEND`，单行是原子的，但跨进程不会破坏行完整性。然而**顺序和语义被破坏**：进程 A 的"发现 gap X"和进程 B 的"发现 gap Y"交错出现，后续 `Load` 读回的知识时间线混乱。

### 影响矩阵

| 场景 | 概率 | 影响 | 检测难度 |
|------|------|------|----------|
| 开发者+CI 同时运行 | 高（CI 设在主干合并时） | checkpoint 回退 → 重复计费 | 极难（无检测代码） |
| 两个 `forge evolve` 同时 | 低 | trace 损坏 → 审计失效 | 事后才发现 |
| `forge doctor` 与 `forge run` 同时 | 中 | 读半写 checkpoint → 误报 | 低（Go 会报解析错误） |

### 已有补偿措施与缺口

- **`persist.Save` 的 tmp+rename**：保证单次写入原子性，但不防多进程覆盖（模式 A）。
- **`trace.Tracer.Emit` 的 mutex**：进程内序列化，但跨进程无保护（模式 B）。
- **`memory.Load` 的 loadCache**：`sync.Map` 按 `(path, mtime)` 缓存，但另一个进程的写入不会触发本进程缓存失效（模式 C → 读到旧数据）。

### 扩展方向

1. **进程锁（文件锁）**：在 `.forge/` 下创建一个 `.lock` 文件，使用 `flock`（Unix）或 `LockFileEx`（Windows）在进程启动时获取共享/排他锁。读操作（`forge status/doctor`）获取共享锁，写操作（`forge run/evolve`）获取排他锁。Go 标准库 `golang.org/x/sys/unix.Flock` 是零依赖方案 — 但 forge-core 目前零外部依赖，需要评估在 `internal/lock/` 下用 `syscall.Flock` 封装的代价。

2. **进程所有权标记**：在 `Checkpoint` 和每个 `trace.Event` 中加入 `runner_pid` / `runner_host` 字段，使得下游恢复和审计能识别哪个进程写入了什么。当前 `Checkpoint` 已有一个 `FormatVersion` 字段但无进程标识。

3. **读写冲突检测**：`Load` 时检查 checkpoint 的 `UpdatedAtUnix` 是否"过于新鲜"（由另一个进程刚写入），或对比进程 PID 作为启发式警告。

---

## 方向二 · 自动内存压实未接入 evolve 循环

### 定位

`internal/memory/memory_compact.go:47` · `cmd/forge/evolve.go:395 cmdMemoryPrune` · `internal/orchestrator/loop.go`

### 现状

`internal/memory/memory.go` 实现了**追加写**的知识存储：

```go
// memory.go (in-memory.go) — Append 使用 O_APPEND 写入一行
// Store 永远不会被重写，只会增长
```

`internal/memory/memory_compact.go` 已经实现了完整的压实（compact）能力：
- `Compact(path, threshold, keepPerKind, ageSeconds)` — 超过 `threshold` 条时触发
- 按 `Kind` 分组，每类保留最近的 `keepPerKind` 条，更老的替换为摘要条目
- `rewriteStore` 实现原子的 tmp+rename 重写

但 **`Compact` 没有被接入 `forge evolve` 的循环中**。证据：

```go
// cmd/forge/evolve.go — cmdEvolve 循环
// 搜索 Compact、compact、memory-prune：不存在于 evolve 路径
```

用户必须手动运行 `forge memory-prune`（`cmdMemoryPrune`）来收紧存储。在一个长时间运行的 evolve 循环（例如 24 小时 / 50+ 迭代）中，存储会**无界增长**。

### 增长模型

```
一次 evolve 迭代写入 ~1 条 memory entry（gap 发现）
50 次迭代 = 50 条，不触达 DefaultCompactThreshold(500)
但：
- 每个 discover 阶段可能产生多条
- 多个 workflow 阶段共同贡献
- 24h × 每 30 分钟一次迭代 = 48 次迭代 × 每迭代 2–5 条 = 96–240 条
- 长周期 (一周级) × 频繁扫描 = 轻易超过 500
```

当前 `memoryContext` 函数（`prompt_memory.go`）已经使用 `boundMemory` 限制了单次 prompt 注入的条目数（`memoryCap=32`，recency floor=8）。所以**注入 prompt 是安全的，但磁盘上的 store 文件持续增长**。影响：

1. **磁盘占用**: JSONL 文件在 1000 条后可能达到数 MB，不是灾难但属于资源泄漏。
2. **Load 时间**: 每次 `memoryContext` 调用都 `memory.Load(path)` 全文读取并接码。虽然有一个 `loadCache` 按 mtime 缓存，但每次 Append 后缓存被 `invalidateLoadCache()` 清空（`memory.go` Line ~395），所以每个有 Append 的迭代都会重新全文读取。
3. **`forge doctor` 扫描**: `memoryCheck` 每次读取全部条目来计数，无界增长使 `forge doctor` 变慢。

### 为什么这是一个产品缺口而非仅性能问题

`memory.go` 的 doc 声明"honest, fault-tolerant load" 和 "grow-only log，绝不 rewrite"，但 `memory_compact.go` 已经是一个 rewrite 机制——有 Compact 但不自动调用。**产品契约不一致**：存储说自己是 grow-only，但 compaction 功能存在只是未启用。用户没有信号知道何时应该运行 `memory-prune`。

### 边界场景

- **同时运行自动压实和 Append**：`Append` 使用 `O_APPEND` 直接追加文件；`rewriteStore` 使用 tmp+rename 原子替换。如果两个操作并发，可能发生 Append 写入旧文件的末尾（在 rewrite 开始之后但 rename 之前）→ 这一次 Append 被丢失。需要 `memory.go` 中的写锁或文件锁。
- **压实后缓存过时**：`Compact` 的最后调用了 `invalidateLoadCache()`，但假设只有当前进程。方向一的跨进程问题在此放大。

### 扩展方向

1. **在 LoopEngine.Run 的每次迭代后自动触发内存压实检查**：在 `runIteration` 的测量之后、下一次迭代之前，检查 memory store 大小，超过阈值时自动 `Compact`。
2. **`Compact` 与 `Append` 的并发安全**：在 `memory.go` 中引入包级写锁（`sync.Mutex`），使得 `Append` 和 `rewriteStore` 互斥。
3. **渐进式压实（增量）**：当前 `Compact` 重写整个文件（`O(n)` 读写）。对长期运行的 agent 来说，增量压实（只重写受影响的部分）可以降低开销。

---

## 方向三 · Prompt 上下文构建缺少 I/O 超时保护

### 定位

`internal/prompt/prompt.go:65 Gather` · `internal/prompt/cache.go:104 GatherCached` · `cmd/forge/prompt_context.go:266 gatherContext` · `internal/orchestrator/orchestrator.go:196 runAgentPhase`

### 现状

prompt 构建路径（`buildPrompt` → `gatherContext` → `prompt.Gather` / `prompt.GatherCached`）在每次 agent phase 执行之前同步地从文件系统读取多个文件：

```
gatherContext 读取链（无超时/无取消）:
  1. os.ReadFile(<root>/.agent/ROADMAP.md)    ← 当前任务
  2. os.ReadDir(<root>/docs/adr/)              ← ADR 文件列表
  3. os.ReadFile(每个 ADR)                      ← ADR 内容（topK 个）
  4. os.ReadFile(<root>/.agent/AGENTS.md)      ← 硬约束
```

```go
// prompt.go:70-78 — 所有文件读取没有 context.Context 参数
func Gather(repoRoot, query string) []string {
    var ctx []string
    if task := currentTask(repoRoot); task != "" {       // os.ReadFile
        ctx = append(ctx, ...)
    }
    if adrs := relevantADRs(repoRoot, query); len(adrs) > 0 {  // os.ReadDir + os.ReadFile × N
        ctx = append(ctx, ...)
    }
    if rules := constraints(repoRoot); rules != "" {     // os.ReadFile
        ctx = append(ctx, ...)
    }
    return ctx
}
```

**这些文件读取没有任何超时或上下文取消机制**。如果仓库位于网络文件系统（NFS、FUSE、sshfs、Container 挂载）上，文件系统延迟或挂起会导致整个 orchestrator 线程阻塞，无法响应取消信号。

### 为什么这是生产级别的缺口

#### 1. 阻塞点位于编排器的关键路径上

`gatherContext` 在 `Engine.runAgentPhase` 内部被调用 — 即每个 agent phase spawn 之前的同步步骤。如果该调用阻塞：

```go
// orchestrator.go:196
func (e Engine) runAgentPhase(ctx context.Context, p asset.Phase, mode string) error {
    // ...
    prompt := prompt.Build(...)  // ← 这里阻塞
    // ...
}
```

此时 `ctx`（携带 SIGINT/SIGTERM 取消信号）已经存在，但 `os.ReadFile` 等系统调用**不会监听 Go context 的取消通道**。即使主 goroutine 收到信号，`os.ReadFile` 在底层 `read(2)` 系统调用返回之前不会返回。

#### 2. 现有上下文传播链在此断裂

Sprint 27 已经为 Engine 添加了 `Engine.Ctx` 和 `runAgentPhase` 的 `ctx context.Context` 参数用于取消传播，但 `prompt.Gather` / `prompt.GatherCached` **没有接收 `context.Context` 参数**。上下文在 prompt 构建这一环节断裂。

#### 3. 缓存不解决这个问题

`prompt.ContextCache` 按 run memoizes ADR/AGENTS.md 读取，假设文件系统在 run 期间是稳定的且可访问。但：
- 首次读取（cache miss）仍然同步阻塞
- 后续 phase 读取 `ROADMAP.md`（`Gather` 始终重读）每次都可能阻塞
- 网络分区发生在高频次（分钟级）agent phase 之间，每次都是一次阻塞风险

#### 4. 这是"寒蝉"类型的 bug

NFS 挂载在生产 CI 环境以及使用远程开发（SSH/VS Code Remote）时非常常见。问题在负载下间歇性出现（高 NFS 延迟时表现，低延迟时不表现），极难复现和调试。

### 边界场景

- **NFS 软挂载（soft mount）**：`soft` 挂载会超时返回 I/O error，但 Go 的 `os.ReadFile` 没有内置重试逻辑——一旦失败，整个 prompt 构建失败，agent phase 无法启动。
- **NFS 硬挂载（hard mount）**：`hard` 挂载会无限重试，领域线程永远阻塞，SIGINT 无法中断。
- **FUSE 文件系统（云桌面 / sshfs）**：在高延迟链路上极容易长时间阻塞。
- **容器挂载（Docker bind mount / Kubernetes CSI）**：存储后端故障时同样阻塞。

### 扩展方向

1. **为 prompt 读取链添加 `context.Context` 参数**：`prompt.Gather(ctx, repoRoot, query)` 等签名扩展。将 `os.ReadFile` 替换为带超时的包装，在 goroutine 中执行并 select ctx.Done()。

2. **引入可取消的文件读取包装**：在 `internal/prompt` 或 `internal/util` 中创建一个 `readFileWithContext(ctx, path, timeout)` 函数，超时后返回 `context.DeadlineExceeded` 而不是永久阻塞。

3. **降级策略**：当文件读取超时时，prompt 应以缺失的上下文内容继续（degrade gracefully），而不是让整个 agent phase 无法运行,配以清晰的日志警告 "ADRs unavailable due to I/O timeout"。

4. **`ContextCache` 与上下文结合**：`GatherCached` 的首轮构建应该也受 context 控制，使得缓存预热可以被取消。

---

## 方向四 · Gate 名目录验证缺失：workflow YAML 中的 gate 名错误被静默忽略

### 定位

`internal/orchestrator/mode_gating.go:26-36 gatesFor` · `internal/mode/mode.go:47 Allows` · `.agent/workflows/build.yml:62`

### 现状

当 `gatingActive()`（定义了 `ModePolicy`）时，`gatesFor` 获取 `required_gates` 与 policy gate 集合的交集：

```go
// mode_gating.go:26-36
func (e Engine) gatesFor(p asset.Phase) []string {
    if !e.gatingActive() {
        return p.RequiredGates
    }
    kept := make([]string, 0, len(p.RequiredGates))
    for _, g := range p.RequiredGates {
        if e.ModePolicy.Allows(g) {   // ← 不在集合中的 gate 静默丢弃
            kept = append(kept, g)
        }
    }
    return kept
}
```

`Allow` 做的是存在性检查：

```go
// mode.go:47
func (p Policy) Allows(gate string) bool {
    for _, g := range p.Gates {
        if g == gate { return true }
    }
    return false
}
```

**如果 workflow YAML 中包含一个 `required_gates` 条目（例如 `"secutiry"` 拼写错误，或者 `"ownership"` 等非标准 gate），该条目会被静默过滤掉，不会产生任何警告或错误。**

### 影响分析

#### 1. 质量问题：意图和实际执行之间的静默偏差

真实场景——build.yml 声明：

```yaml
required_gates: [lint, test, build, complexity, arch, security]
```

如果错误写成 `secutiry`（漏了 'u' 前面），在 `gatesFor` 下：
- `engineer` mode：`Allows("secutiry")` → false → 静默跳过
- 没有任何日志说 "gate secutiry not found in policy set"
- 运行报告 "stop: gates green"（实际上 security gate 从未运行）
- **安全闸门被静默省略**，而所有参与者认为它已经运行

#### 2. 故障模式：只有 E2E 测试才能检测到

`forge validate --models` 检查 agent 卡和工作流交叉引用，但不检查 gate 名称是否在已知目录中。具体的缺失验证：

- `internal/doctor/models.go` `EvaluateWorkflowModels` — 验证 agent/phase 引用，但不验证 gate 名
- `harness/check.py` — 治理检查，验证 control flow refs 但**不验证 gate name 目录**
- `internal/gate/gate.go` — 运行 gate 时只按名调度，不存在则返回 "N/A"，而不是 "unknown gate name"

#### 3. 扩展问题：自定义 gate 的情况

当有自定义 gate 时（例如 `"ownership"` 或 `"benchmark"`），目前没有注册机制让自定义 gate 名加入已知目录。`internal/mode/mode.go` 的 `allGates()` 是硬编码的固定集合：

```go
var fullGates = []string{"lint", "test", "build", "complexity", "arch", "security"}
```

### 边界场景

- **mode=explorer 下的合法过滤**：explorer 的 gate-set 只有 `[lint, build]`。`gatesFor` 过滤掉 `test` 是**预期行为**，不应报警。需要区分"已知但被 mode 过滤"和"未知 gate 名称"。
- **生命周期升级后引入新 gate**：当 `lifecycle=production` 强制全 gate 集合时，一个在 idea/mvp 下没问题的未知 gate 名到了 production 下仍会被静默忽略。
- **0 值 Policy 的退路**：当 `gatingActive() == false`（零值 Policy），`gatesFor` 返回 `RequiredGates` 原样 — 此时未知 gate 名不会被过滤，但也没有验证。

### 扩展方向

1. **Gate 名称目录验证**：在 `gatesFor` 中添加一个验证步骤——当 gate 名不在 `allGates()`（或扩展注册表）中时，输出警告而不是静默跳过。这适用于 `gatingActive()` 和 `!gatingActive()` 两种路径。

2. **引入 Gate Registry**：在 `internal/gate/` 下创建一个注册表，允许包通过 `init()` 或显式 `Register(name, runner)` 注册已知 gate。`allGates()` 改为从注册表推导，而不是硬编码列表。

3. **`forge validate` 扩展**：增加一个 `forge validate --gates` 子命令，读取所有 workflow YAML 的 `required_gates` 并与已知 catalog 交叉验证，报告未知 gate 名称。

4. **日志增强**：在 `gatesFor` 中，当 `Allows(g) == false` 时，区分"已知 gate 被 mode 过滤"和"未知 gate 名称"两种情况，分别输出 INFO 和 WARN 级别的日志。

---

## 附录：交叉验证声明

本文档的每个方向与 `docs/requirements/` 下 80+ 篇已有扩展分析文档进行了关键词比对和语义交叉验证：

| 本文件方向 | 最接近的已有文档 | 差异说明 |
|-----------|----------------|---------|
| 方向一：跨进程 `.forge/` 协调 | `docs/requirements/2026-07-11-forgeos-state-data-integrity-and-lifecycle-gaps.md` | 该文聚焦 checkpoint/trace **单进程**完整性和生命周期，不包括跨进程冲突检测或锁机制。方向一是独占的横切面缺口。 |
| 方向二：自动内存压实未接入循环 | `docs/requirements/2026-07-10-five-genuine-architectural-frontiers.md` 方向一(经济/可观测/审计)及 `docs/requirements/2026-07-11-codegrounded-edge-cases-and-extensions.md` §4(自定义gate) | 未有文档将 `memory_compact.go` 的已存在但未接入 evolve 循环作为独立方向分析。该边界存在于 storage 与 loop orchestration 的接口处。 |
| 方向三：prompt 构建 I/O 超时 | 所有 Sprint 27 信号处理文档（`docs/sprint/sprint-27-signal-handling.md`）覆盖了 orchestrator 核心路径的 context 传播，但**未覆盖 prompt 文件读取路径**。该路径缺少 `context.Context` 参数且不使用可取消 I/O。 | 方向三是 Sprint 27 的未被发现的遗留缺口。 |
| 方向四：Gate 名目录验证 | `docs/requirements/2026-07-11-codegrounded-edge-cases-and-extensions.md` §4 "Harness gate 名称与已知目录验证" | 该方向之前已被识别。本文提供了更深入的影响分析（静默安全缺失）和扩展建议（gate registry、`forge validate`）。**此处保留作为完整性参考，方向一/二/三为主打的三个全新方向。** |

**三个主打全新方向：** 方向一（跨进程协调）、方向二（自动压实接入）、方向三（prompt I/O 超时）。
