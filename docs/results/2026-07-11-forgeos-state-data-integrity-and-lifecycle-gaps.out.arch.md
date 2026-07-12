现在我已充分理解项目上下文。以下是我的架构分析。

---

# ForgeOS 核心运行时架构分析

## 1. 架构评估

### 核心优势

**中枢旋钮（mode×lifecycle）是一个卓越的设计决策。** 它实现了一个跨三轴（Router 档位、Harness 严格度、Workflow 深度）的单一安全策略向量，其统一效果类似于 Kubernetes 的 `PodSecurityPolicy`，但嵌入到编排循环本身中，而非作为事后审计层。`production lifecycle` 否决所有宽松 mode 的能力提供了一个可证明的不可绕过安全下限——这是一个在竞争系统中很少见的属性。

**零依赖 Go 运行时是一项重大的工程成就。** 13 个包编译成一个静态二进制文件，无需任何外部依赖，意味着部署等于 `scp`。与 Temporal/Postgres/NATS 集群（north-star 架构设想）相比，这提供了一个极端务实的当前状态——避免在验证核心循环是否存在根本性缺陷之前进行过度工程化。

**Converge-driven 编排**（基于 ROADMAP 完成度和 gate 状态，而非轮数）避免了代理编排中最常见的反模式：循环固定轮数，然后要么过早停止（遗漏工作），要么在已满足条件后继续执行（烧钱）。MET（最小有效阈值）信号——`GatesGreen` + `RoadmapCompletion` + `ReviewStatus` + `RequirementConfidence` + `FileDelta`——代表了一个经过深思熟虑的、定义何为"完成"的多元化方法。

### 关键局限

**当前架构存在三个井水不犯河水的持久化子系统（trace、checkpoint、memory），各自独立演化，缺乏统一的存储契约。** 这在 cross-validation 中暴露为一组模式：

| 子系统 | 写入者 | 读取者 | 一致性模型 | 竞态风险 |
|---|---|---|---|---|
| Trace (trace.jsonl) | `Tracer.Emit` → `*os.File` | `scorecard_wind.go` (新句柄) | 无缓冲 → 无刷新 → 依赖"文件仍打开"不变量 | P0 - 轮转+wind-down 竞争导致静默跳过 |
| Checkpoint (checkpoint.json) | `persist.Save` (每次迭代 ~1+6N 次) | `LoadCheckpointChain`、`forge status --history` | 无文件锁、OS write 原子性不足以保证结构化 JSON | P0 - 双进程静默覆盖 |
| Memory (memory.jsonl) | `memory.Append` (O_APPEND) | `memory.Load` (sync.Map 缓存) | O_APPEND 行级原子性 ≤ PIPE_BUF、跨进程缓存失效 | P1 - 性能退化、非损坏 |

这些并不是三个独立的问题。它们是根本性缺失的体现：**共享持久化层的单一写入者契约**。每个子系统都暗自假定自己是唯一的写入者，但文件系统级隔离（每个进程自己的 `.forge/` 或以协同锁作为前缀）从未被强制执行。

**Frame 问题（方向四）是最危险的单个问题。** 一个在 `mvp` lifecycle 下开始 `forge evolve` 的用户，在中途切换到 `production`，却继续以 `mvp` 宽松策略运行——这不仅仅是一个次优行为，而是安全政策的静默降级。类比：在 Kubernetes 中更新 Pod 的 `securityContext.runAsNonRoot: true`，但 kubelet 从不重新读取它。系统不会让它发生，因为没有机制去检测变化。

**trace pipeline 的 "load-bearing invariant" 模式是一个架构反模式。** 代码库中有多处注释承认脆弱的假设（`evolve.go:478-483`，`scorecard_wind.go:34-38`），但没有类型来强制执行它们。这是一个系统代码中"靠运气编程"的指标——正确性依赖于未被强制执行的行为契约。Flush 方法的缺失不只是疏忽；它是当前架构中缺乏一致性管理仪式的前沿表现。

**文件数预算作为架构约束是有问题的。** `cmd/forge` 包上限为 14-17 个文件，虽然对防止上帝包有良好的意图，但已经导致了 repeated 的违规→拆分→违规循环（Sprint 27、29、30）。一个 Go 包拥有 18 个文件与拥有 14 个在工程上并无区别。每次在 I/O 边界上拆分（独立 Responsibility）中都有真正的好处，但该指标正在引起行为适应（如 `internal/doctor` 和 `internal/attribution` 拆分所示，这些都是有效的），与 gamesmanship（只调整上限值）。我建议将该限制视为"早期预警"，而非"硬停下"——除了 `main.go` 500 行的限制，它已经阻止了合理拆分类似逻辑的函数。

## 2. 扩展方向

### 方向 A：持久化层单一写入者契约（P0 基础）

**为什么需要：** 三个竞态（trace/checkpoint/memory）都源于同一个根本原因：系统假设 `.forge/` 内的文件是进程私有的，但实际的文件系统却无法强制执行这一点。该跨域验证已将此确立为当前最昂贵的单一架构缺陷。

**核心挑战：** 解决方案必须同时满足多个约束：
- **向后兼容性：** 现有的 `.forge/` 目录不能被破坏
- **进程发现：** 多个 `forge evolve` 必须能检测到彼此
- **读取路径：** `forge status --history` 和 `forge doctor --anomaly` 必须能在其它进程存在的情况下读取 checkpoint
- **性能：** checkpoint 在每次迭代中写入 ~1+6N 次；锁争用不得阻止循环

**架构变更：**

**选项 A（推荐）：`flock(2)` 读/写分离锁。** 原因如下：
- Go 标准库 `os` 包通过 `syscall.Flock` 直接暴露 `flock(2)`
- 零依赖增量
- 读写锁模式使 `forge status` 在持有共享锁时读取
- 写入者获取排他锁；若锁不可用，则跳过 checkpoint（而非阻塞）——保持循环进行

```
[写入者] acquire write lock → persist.Save → release lock
[读取者] acquire read lock → persist.Load → release lock
[竞争写入者] flock(LOCK_EX|LOCK_NB) → EAGAIN → 跳过,日志记录
```

**选项 B（更昂贵但更干净）：重构为 `server`/`client` 进程架构。** 一个轻量级 `.forge/daemon` Unix socket 代理所有 I/O。这消除了竞态，但会：
- 增加 ~500 行守护进程管理代码
- 创建一个难以在无 head 的 CI 环境中管理的生命周期依赖关系
- 需要一个 `start`/`stop` 命令

**对现有系统的影响：** 选项 A 影响范围小：`persist.Save` 和 `persist.Load` 各添加约 10 行。无文件重组。无 CLI 变化。应该先实施方向 A，再实施方向 C（Frame 问题），因为安全修复依赖于稳定的持久化层。

### 方向 B：ModePolicy 运行时刷新（P0 安全）

**为什么需要：** 已确立为安全政策的静默降级。用户将 lifecycle 从 `mvp` 更改为 `production`，期望发生更严格的执行，但正在运行的 `forge evolve` 会继续以宽松的策略运行，直到下次重启。这是一个安全漏洞，等价于"更新了防火墙规则但防火墙进程不重读配置。"

**核心挑战：**
- **检测变更而不轮询文件系统：** 在每次迭代中读取和解析 `.agent/project.yml` 会带来开销
- **原子性切换：** `mode.Effective` 的返回是一个值类型 `Policy` 结构体。在迭代中间切换可能导致一个 gate 使用旧阈值而另一个 gate 使用新阈值
- **可观测性：** 用户必须能看到正在使用哪个 lifecycle，以及它是否已过时

**架构变更：**

在每个迭代顶端添加重新评估入口点（方法 A 中的等价物，在 `LoopEngine.runIteration` 中的 `OnBeforeIteration`）：

```
func refreshPolicy(ctx context.Context, root string) (mode.Policy, bool) {
    lifecycle := resolveLifecycle(root)     // 读取 .agent/project.yml
    pol := mode.Effective(o.mode, lifecycle)
    return pol, true
}
```

关键设计决策：**在迭代边界上切换，绝不在迭代中间切换。** 迭代是不可变的——一旦开始，整个迭代使用同一个 `Policy` 快照。这是正确的，因为：
1. Gate 评估基于单一 policy 是一致且可审计的
2. 当前 checkpoint 系统假设 policy 在迭代过程中是稳定的
3. 调试更简单：每个 checkpoint 记录它所用的 policy

`OnBeforeIteration` hook（当前仅用于 checkpoint）应被扩展成一个完整的回调链，policy 刷新时产生一个可追溯的事件：

```
checkpoint hook      (当前) →  checkpointHook
policy refresh hook  (新增) →  refreshPolicyHook  →  emit trace event on change
memory prune hook    (新增) →  compactMemoryIfDue  (已经存在,但应从迭代循环中分离)
```

**对现有系统的影响：** 中等。需要更改：
- `LoopEngine` 添加 `refreshPolicy` 入口点
- `checkpointHook` 新增 `policy_snapshot` 字段（向后兼容，因为缺失字段导致 JSON `null`，可由现有消费者处理）
- `forge status` 显示 "LIFECYCLE: production (active) / mvp (stale)" 以指示漂移

### 方向 C：Trace Pipeline 契约形式化（P1 → P0 候选）

**为什么需要：** "load-bearing invariant" 是系统代码中"靠运气编程"的架构反模式。trace pipeline 目前依赖于：
1. `Emit` 直接写入 `*os.File`，无缓冲
2. Wind-down 读取者打开一个独立句柄
3. 没有 `Flush()` 方法，没有 `Sync()` 方法，没有错误传播
4. 注释是唯一的"契约"

**核心挑战：**
- 引入 `Flush()` 是微不足道的，但也应该修复根本原因：在 wind-down 时共享文件句柄
- 需要重新排序 `closeTrace()` → `windDownScorecards()`，而不是依赖"文件仍打开"的不变量
- 如果在 wind-down 后添加了 trace 事件，close-then-reopen 模式就会崩溃

**架构变更：**

```
// 当前（隐式，基于不变量）：
defer closeTrace()
// ... loop.Run(...)
windDownScorecards(wf, o, logln, ...)   // 必须发生在 closeTrace 之前

// 所需（显式，受限制的）：
// 1. 在 wind-down 前关闭写入端：
closeTrace()
// 2. Wind-down 以只读方式重新打开文件：
windDownScorecards(wf, o, logln, ...)   // 内部 os.Open(tracePath)
// 3. 不再需要 defer closeTrace()
```

并发性变更：`closeTrace()` 必须是幂等的（多次调用是安全的）并且线程安全的。trace 轮转也需要被保护起来，防止在 wind-down 关闭后调用 `Emit`。

**对现有系统的影响：** 中等偏低。`closeTrace()` 的变更很小。真正的努力在于审计所有 `Emit` 调用点，确保在 wind-down 后没有调用。当前代码在 `loop.Run` 返回后没有调用，但需要通过代码审查或 linter 规则强制执行。

### 方向 D：统一产物生命周期管理（P2 运维）

**为什么需要：** 目前，ForgeOS 产生三种持久性工件（trace、checkpoint、memory），各自独立进行清理。没有 `forge clean`。CI 环境在长时间运行的 `forge evolve` 中可能会遇到磁盘耗尽（ENOSPC），而当前的 fail-loud-and-continue 处理意味着 ENOSPC 可能导致静默数据损坏，而不是彻底停止。

**核心挑战：**
- **LRU 与 TTL：** 哪个语义适合 ForgeOS？`forge status --history` 需要 checkpoint 历史记录，时间范围未知。简单的 TTL 可能会移除重要的诊断数据。
- **运行时线程安全：** `forge clean` 在作为并发 `forge evolve` 运行时不能删除活动文件。
- **发现阶段清理：** 并非所有产物都位于 `.forge/` 中——agent 工作目录可能包含临时文件。

**架构变更：**

```
forge clean [--traces] [--checkpoints] [--memory] [--all] [--dry-run]
forge clean --older-than 24h
forge clean --except-last 3        # 保留最后 3 个 checkpoint，修剪其余
```

产物 registry（在方向 A 中添加，作为 `persist` 包的一部分）应暴露一个用于清理的迭代器接口：

```go
type ManagedArtifact interface {
    Path() string
    Kind() ArtifactKind     // trace, checkpoint, memory
    ModTime() time.Time
    Remove() error
}
```

**对现有系统的影响：** 低。这是一个纯新增功能，不改变现有行为。应与方向 A 协调，确保清理不会混淆锁逻辑。

### 方向 E：Eval→Router 回灌管道（v3 基础）

**为什么需要：** 这是 north-star 架构中"学习闭环"的核心。目前，scorecard 数据被收集并存储在 `.forge/scorecard.json` 中，但从不反馈到模型路由。路由使用 `HistoryTiebreak`，它一次只考虑一个候选项。没有多维评分（复杂度/依赖/上下文/业务影响）驱动实际的执行路径。

**核心挑战：**
- **数据新鲜度：** scorecard 数据在 evolve 的每次迭代后都会更新，但 routing 决策在每次迭代开始时做出。Eval→Router 延迟必须是迭代间的（非实时）。
- **冷启动：** 对于新项目，没有历史数据可以路由。跨项目的可转移信息是有限的，因为每个项目的代码库都不同。
- **信号稀释：** 质量/latency/cost——这三个维度是正交的，对它们的加权是一个政策决策，而非技术决策。过早设计加权是危险的。

**架构变更：**

此变更不应在当前 sprint 中实施。Scorecard 模式是健全的；数据正在被收集。v3 的"回灌"只需要一条代码路径：

```go
// 在 routing.TierFor 中（当前模式，伪代码）：
func TierFor(req Request) Tier {
    history := LoadScorecard(req.Repo)
    if history.HasData() {
        // 基于过往性能提升/降低至次优 tier
        req.Complexity = interpolate(history)
    }
    return base(req)
}
```

**对现有系统的影响：** 最小，如果推迟到 v3。一切都已就位：scorecard 数据正在被写入，`HistoryTiebreak` 存在，`ScorecardPair` 类型存在。缺失的部分是 `TierFor` 中的实际评分权重。

## 3. 接口设计建议

### 关键模块契约

**Persist 包应成为所有文件系统 I/O 的网关，而非三个并行子系统之一。**

目前：
```
persist.Save / persist.Load     ← checkpoint 使用
trace.Tracer.Emit               ← 跳过 persist，直接写入 os.File
memory.Append / memory.Load     ← 跳过 persist，直接写入 os.File
```

所需：
```
persist.Save / persist.Load     ← 所有结构化数据（checkpoint + 任何新子系统）的网关
trace.Tracer.Emit               ← 通过 persist 层去写入（获得锁、错误处理、flush 契约）
memory.Append / memory.Load     ← 通过 persist 层去写入（获得文件级锁定）
```

这引入了一个抽象层，代价是 Go 函数调用开销。对于每迭代 ~1+6N 次 checkpoint 写入 + N 次 trace 写入来说，这是微不足道的。

**关键接口：**

```go
// persist 包
type Store struct {
    mu sync.RWMutex        // 进程内，协程安全
    f  *os.File            // 可选：活动文件句柄
}

func (s *Store) Save(ctx context.Context, path string, v any, locking LockMode) error
func (s *Store) Load(ctx context.Context, path string, v any) (found bool, err error)
func (s *Store) Append(ctx context.Context, path string, v any) error
func (s *Store) Flush() error
func (s *Store) Lock(ctx context.Context, mode LockMode) (func(), error)  // 返回解锁器
```

**向后兼容：** `LoadCheckpointChain` 和 `forge status --history/doctor --anomaly` 不需要改变。它们读取的文件格式保持不变。`Store.Save/Load` 被替换为内部实现，但 `persist.Load` 的公共签名保持不变。

### OnBeforeIteration 回调链

目前，`LoopEngine.OnBeforeIteration` 是一个单一回调（用于 checkpoint）。它应该变成一个链：

```go
type IterationHook interface {
    OnBeforeIteration(ctx context.Context, i int) error
    OnAfterIteration(ctx context.Context, i int) error
}

type Hooks []IterationHook

func (h Hooks) OnBeforeIteration(ctx context.Context, i int) error {
    for _, hook := range h {
        if err := hook.OnBeforeIteration(ctx, i); err != nil {
            return err
        }
    }
    return nil
}
```

这允许在不修改 `LoopEngine` 的情况下附加策略刷新、内存压缩和迭代后清理。每个 hook 由一个单一职责结构体实现。现有 `checkpointHook` 保持不变。

## 4. 技术选型

### 可以引入的内容

| 技术 | 评估 | 何时 |
|---|---|---|
| `flock(2)` 用于文件锁定 | 已内置 Go `syscall`，零依赖 | 立即——方向 A |
| 文件系统监控（inotify/kqueue） | 用于检测 `project.yml` 变更 | 可选——轮询每迭代更简单、足够好 |
| SQLite 用于持久化 | 替换 `.forge/*.json`/`.jsonl` | **不推荐**——为单文件、单进程工作负载增加 3MB 二进制文件 |
| Go YAML 库（`gopkg.in/yaml.v3`） | 不需要 Python shim | **相反推荐**——但打破零依赖约束。等待 v3。 |

### 自建 vs 采购的方向

对于 ForgeOS `forge-core` 来说，**自建是自然而然的默认选择**，因为零依赖是一条纪律，而不是一个成本函数。Go 标准库覆盖了：

- `flock(2)`: `syscall` 包（零添加）
- JSON: `encoding/json`（已使用）
- 文件系统监控：`os` + `fsnotify`（非标准库，但只有为 CLI 工具添加时——不必要，轮询工作良好）
- YAML：标准库的缺失是唯一真正的差距。Python shim 是一种务实的权宜之计。

**唯一重要的采购讨论是关于 Temporal。** Sprint 30/31 已确立，`forge-core` 的编排模式（带有收敛的同步循环）适用于直接的 Go 协程。Temporal 在以下情况下才有必要：
1. **跨机器持久化编排**（多个 CI runner 并行执行 workflow）
2. **人审 durable wait**（审查在几小时/几天后恢复）
3. **Cron 触发**（定时 evolve 运行）

在形成这些需求之前，自研编排是正确的决策。购买 Temporal 是等 v3 及之后才有的明确需求出现后才考虑的。

### 锁策略比较

| 方案 | 复杂度 | 性能 | 正确性 | CI 兼容性 |
|---|---|---|---|---|
| `flock(2)` 读写锁 | 低（~10 行/函数） | 高（非阻塞检查 ~0.01ms） | 进程级；NFS 不可靠 | ✅ 本地磁盘 |
| 空文件标记（`.lock`） | 最低 | 中等（原子重命名） | 崩溃时残留 | ✅ |
| Unix domain socket 代理 | 高（~500 行服务器代码） | 高 | 完美 | ❌ 不适用于 CI 短暂进程 |
| `sqlite` WAL 模式 | 中等（知识负担） | 中等 | 协程+进程级 | ✅ |

**推荐：`flock(2)` + `.lock` 回退标记**。`flock(2)` 覆盖了 95% 的情况。`.lock` 标记处理 NFS 和 CI 特殊情况。

## 5. 实施路线图

### 阶段 I：基础韧性（1 个 sprint）

| # | 项目 | 优先级 | 难度 |
|---|---|---|---|
| I-1 | 方向 A：`persist.Save/Load` 中的 `flock(2)` 读写锁 | P0 | 低（2 个函数，~20 行） |
| I-2 | 方向 C：`closeTrace()` → `windDownScorecards()` 重新排序 | P0(P1→P0) | 低（4 行变更 + 顺序重新排列） |
| I-3 | 方向 C：`trace.Tracer.Flush()` + `Sync()` | P0 | 低（1 个函数, ~5 行） |
| I-4 | 方向 C：错误处理——trace.Emit 失败传播到 checkpointHook | P0 | 低（2 次编辑） |

**验证：** `forge accept` 必须仍是 ACCEPTED。添加进程级并发测试（Go `testing.T.Parallel()` 中的两个 `forge evolve` 共享同一个根）。

**风险：** `flock(2)` 在 NFS 上不可靠。缓解：添加 `.lock` 标记回退，当 `flock` 失败时使用。

### 阶段 II：安全策略强制执行（1 个 sprint）

| # | 项目 | 优先级 | 难度 |
|---|---|---|---|
| II-1 | 方向 B：`LoopEngine` 中每迭代的 ModePolicy 刷新 | P0 | 中等（`OnBeforeIteration` 链） |
| II-2 | 方向 B：`projectYAMLValue` 缓存失效 | P0 | 低（从文件 mtime 开始） |
| II-3 | 方向 B：Lifecycle 可观测性——`forge status` 显示"active/stale" | P1 | 低（新字段在 status 输出中） |
| II-4 | 方向 B：Checkpoint 模式快照——每次迭代记录 policy | P1 | 低（checkpoint JSON 中的 1 个字段） |

**验证：** 双进程测试：进程 A 以 `mvp` 开始，进程 A 写入 `.agent/project.yml` 更改为 `production`，进程 A 的下一次迭代使用 `production` 策略。`forge accept` ACCEPTED。

**风险：** 如果 `project.yml` 的写入与读取部分重叠，在 YAML 解析期间竞态失效。缓解：读取到 `[]byte` 然后解析（Go 的 `os.ReadFile` 做到这一点——一次原子读取直到 <64KB）。对于在生命攸关的系统中修改 `project.yml` 的用户来说，这是完全可接受的。

### 阶段 III：可持续管理（1 个 sprint）

| # | 项目 | 优先级 | 难度 |
|---|---|---|---|
| III-1 | 方向 D：`forge clean` 命令——profile/清理 6 个工件类型 | P2 | 中等 |
| III-2 | 方向 D：`forge clean --older-than`，`--except-last` | P2 | 低 |
| III-3 | 方向 A：`.lock` 标记文件回退（NFS） | P1 | 低 |
| III-4 | 方向 D：ENOSPC 检测——checkpointHook 终止（而非 fail-loud-and-continue） | P1 | 低（1 行错误类型切换） |

**验证：** 创建人工工件，运行 `forge clean --dry-run`，确认正确的工件被标记，运行 `forge clean`，确认它们消失。ENOSPC 测试模拟磁盘已满错误。

**风险：** `forge clean` 在并发访问期间删除活动 checkpoint 文件。缓解：flock(2) 排他锁检查——如果无法获取锁，则跳过该文件，报告为 "skipped (in use)"。

### 阶段 IV：诊断与路由管道（1-2 个 sprint）

| # | 项目 | 优先级 | 难度 |
|---|---|---|---|
| IV-1 | 方向 E：`DetectAnomalies` 增强——生命周期漂移检测 | P2 | 低（现有 5 个启发式 +1） |
| IV-2 | 方向 E：Checkpoint `retain` 从 5 重新打开 → 通过 `retain=N` 标志可配置 | P2 | 低（常量变为参数） |
| IV-3 | 方向 E：Eval→Router `HistoryTiebreak` 的正式 scorecard 回灌 | P2 (v3) | 中等（新路线评分逻辑） |

**验证：** 熟悉但具备。

### 优先级映射汇总

| 阶段 | 方向 | 原始优先级 | 最终优先级 |
|---|---|---|---|
| I | 方向一（进程隔离） | P0 | **P0** |
| I | 方向三（Trace 一致性） | P1→P0 | **P0**（与方向一基于同一个基础契约） |
| II | 方向四（Mode 固定） | P1→P0 | **P0**（安全政策问题，但缺少隔离则无法安全测试） |
| III | 方向五（无清理） | P2 | **P2**（运维债务，无活跃损坏） |
| IV | 方向二（保留无消费者，降级后） | P1→P2 | **P2**（`forge status --history` 是有价值的诊断工具） |

### 风险登记

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| `flock(2)` 在 NFS 上静默失败 | 中 | 高——回退到无保护的写入 | `.lock` 标记回退 + CI 确认在绑定/临时挂载上运行 |
| 对于模式刷新，`project.yml` 解析竞态 | 低 | 中——使用错误策略执行一次迭代 | `os.ReadFile` 原子性 + 迭代间切换 |
| 并发测试在 CI 中不稳定 | 中 | 低——超时、端口冲突 | `TMPDIR` 隔离、`t.Cleanup` 清理、合理的 10s 超时 |
| 用户期望跨机器 checkpoint（Temporal 风格） | 低 | 中——当前架构模型不支持 | 延迟到 v3 真实 Temporal 讨论；在 `.forge/` README 中归档决策 |

### 结论

ForgeOS `forge-core` 展示了非同寻常的架构纪律：
- 零依赖 Go 静态二进制作为运行时
- 统一的中枢旋钮（mode×lifecycle）驱动三个正交行为轴
- 收敛驱动编排替代了脆弱的轮数计数

cross-validation 中暴露的三个根本性架构缺陷——进程隔离缺失、安全策略冻结、trace 契约缺失——全部是可修复的，且每个修复影响有限。更重要的是，它们可以被独立编码、测试和部署，因为当前的架构已经足够模块化，每个问题都隔离在单一的包边界内（`persist`、`orchestrator`、`trace`）。

真正值得关注的是，这三个问题具有相同的根本原因：**每个持久性子系统都假设自己是 `.forge/` 内文件的唯一写入者，而且这个假设无法在文件系统层面强制执行。** 修复这个根本原因（阶段 I 的方向 A）会同时修复三个表面问题。这就是架构 leverage 的意义所在。
