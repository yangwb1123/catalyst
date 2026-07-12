现在我已充分掌握了上下文。以下是我的架构分析。

---

# 架构分析：ForgeOS 扩展方向验证与前瞻

## 1. 架构评估

### 当前架构优势

审查验证了 ForgeOS 脊柱架构有显著的设计质量：

- **模块边界清晰**。`prompt`、`memory`、`orchestrator`、`converge` 等独立包有着清晰的单一职责——每个包都有一个可测试的职责，通过明确的接口相互通信。`prompt.go` 中跨三个通道（task、constraints、ADR）的分离是关注点分离的一个很好的例子。

- **诚实优先（Honesty-first）设计模式**。该代码库一致地实现了容错边界（`prompt.Gather` 对缺失文件静默退化，`memory.Load` 对待缺失存储为冷启动而非错误）。这种"静默降级但精确报告"的模式意味着系统不会对调用者撒谎——对于治理 OS 来说，这是一个关键属性。

- **治理作为真值来源**。Harness 闸门（`harness/gate.mjs`、`arch-check`）作为带外执法层运行，独立于任何特定的 AI 宿主。这种架构决策避免了供应商锁定，并允许随着更多执行宿主被支持而统一演进规则。

- **纯标准库零依赖约束**。`forge-core`（Go 运行时）严格坚持零外部依赖——这是一个有意的权衡，在加速构建/部署的同时，将语义检索（目前是 BM25-lite）等领域的 v1 实现边界设得较低。考虑到项目的阶段（v2 脚手架），这是合理的。

### 限制与已识别的技术债务

审查暴露了几个关键的限制：

| 限制 | 影响 | 严重程度 |
|---|---|---|
| **`prompt.Gather`/`GatherCached` 缺少 `context.Context`** | 所有 prompt 构建期间的 I/O（`os.ReadFile`、`os.ReadDir`）无法被取消或超时。长时间运行的操作（大型仓库上的 ADR 目录遍历，堵塞的文件系统）会阻塞执行器线程且无法恢复 | **高**——这是真正的断裂点 |
| **内存压缩已连接但不可配置** | `compactMemoryIfDue` 以硬编码的 `i%10` 间隔触发，使用 `DefaultCompactThreshold`/`DefaultCompactKeepPerKind`。没有 `memory.Optimize(writeHeavy bool)` 或 `memory.CompactConfig` 调整 | **中**——功能正确但缺乏生产级控制 |
| **`sync.Map` 用于缓存而无需驱逐担保** | `memory.loadCache` 使用 `sync.Map`，它无限增长。在跨多个仓库的长时间运行的 evolve 会话中（方向一的分布式场景），这会导致常驻内存的线性增长 | **低-中**——在单项目运行中不会显现 |
| **检索器是同步执行线程内的纯函数** | `prompt.Retrieve` 是纯 CPU 绑定的 BM25-lite 评分。虽然简单性值得赞赏，但缺乏插值点意味着升级到语义检索需要对 `prompt.go` 和 `retrieve.go` 同时进行重构——一个可以避免的侵入性变化，如果有一个 `Retriever` 接口的话 | **中**——影响 v3 可扩展性的设计债务 |

### 关键设计决策评估

1. **`Gather` 中容错为静默空值**（`prompt.go:66-68`）：对于给定的假设（缺失文件 = 冷启动）是正确的。但随着系统增长到跨两个仓库工作，静默空值会掩盖配置错误或用例未对齐——应该增加一个可选 `logger` 或 `warnings io.Writer` 参数，而不破坏现有的调用者。

2. **JSONL 用于内存存储**（`memory.go:44-57`）：对于仅追加的日志来说是一个很好的选择。`O_APPEND` 原子性保证是正确的。然而，同一文件在压缩后（`memory.Compact` 用摘要条目替换旧条目）从仅追加变为覆盖追加——这里存在一个微妙的竞争：如果两个 evolve 循环实例在共享挂载点上压缩同一个文件，它们会交错的 JSON 行。

3. **`orchestrator` 作为中央执行器 vs. 分布式协调**：当前架构将所有执行路径集中在一个进程中。方向一（跨进程协调）已经在三份 7 月 10 日的文档中被覆盖，这一事实佐证了它确实是一个已知的架构边界——但正因为它是已知的，才凸显了当前的集中式模型是 v2 的一个故意的简化。

---

## 2. 扩展方向

以下 5 个方向按高价值排序，基于审查的代码级验证。

### 方向 P0：为所有 I/O 绑定路径添加 `context.Context`（Prompt I/O 超时）

**为什么需要**：审查确认 `prompt.Gather` 和 `prompt.GatherCached` 执行 `os.ReadFile` / `os.ReadDir` *没有* `context.Context` 参数。在长时间运行的 evolve 循环中（24h 无人值守），单个堵塞的 I/O 操作会使整个执行器线程停顿。对于生产就绪的自治操作，这修复了一个不能协商的缺陷——它不是一个"好要有"的功能。

**核心挑战**：
- `prompt.Gather` 目前向 3 个下游 helper 传递 `(repoRoot, query string)`——将签名改为 `(ctx, repoRoot, query)` 需要改变这些内部 helper 以及所有调用者。
- 缓存层（`GatherCached`）在其缓存键查找之前也需要一个 ctx——缓存驱逐也可能从超时中受益。
- 在 Go 中，`os.ReadFile` 没有本地的上下文感知变体。包装器必须将 I/O 包装在 goroutine 中并使用 `select`——标准模式，但每个调用点都需要 ~5 行样板代码。

**预期的架构变更**：
- `prompt.Gather(ctx context.Context, repoRoot, query string) ([]string, error)`——从无错误返回类型变更为有错误，因为超时现在是一个可能的失败模式，所有调用者必须处理。
- 在 `prompt` 包中引入 `ioController`（或类似的 helper）来统一管理上下文感知的文件读取。
- 调用者（主要是在 `orchestrator` 中）必须将 `ctx` 从 evolve 循环一路传递到 `prompt.Gather`。

**对现有系统的影响**：
- 签名变更会传播到所有调用 `Gather` 的地方——每个都需要`ctx` 参数 + 错误处理。
- 对于不使用超时的简单脚本化使用，`context.Background()` 提供了一个干净的迁移路径。
- **与方向二的交互**：一旦提示 I/O 有超时，内存 I/O（压缩、追加）也应该有超时——在同一个 pr 中统一处理两者是合理的。

### 方向 P1：使内存压缩可配置且并发安全

**为什么需要**：审查纠正了"压缩没有被连接"的错误——它确实在 `evolve.go:407` 被调用。但这是一个低于标准的集成。硬编码的 `i%10` 节奏意味着：
- 写入量大的运行（每次迭代大量工件）要在 10 次迭代后才看到压缩，而此时 `memory.jsonl` 可能已经膨胀到数 MiB。
- 读取量大的运行（每次迭代有大量上下文检索）会在每次迭代的不必要时间点支付压缩成本，而不仅仅是在建立检查点之前。
- 没有并发访问控制——正如上面的架构评估中提到的，共享挂载点上的两个 evolve 循环会交错压缩写入。

**核心挑战**：
- `memory.Compact` 读取整个文件，聚合，然后写回。在多进程场景中，这是一个经典的 TOCTOU（检查时使用到使用时变更）竞争。
- 压缩阈值因迭代而异——一个只有 100 个条目的项目不需要压缩，而一个拥有 5000 个条目的 48 小时运行则需要。
- 配置需要来自调用者，而不是包的全局变量——但 `memory` 目前是无状态的纯函数。

**预期的架构变更**：
- 引入 `memory.Config` 结构体：`type Config struct { CompactThreshold int; KeepPerKind int; AgeSeconds int64; CompactInterval int; LockFile string }`。
- 将 `compactMemoryIfDue` 从 `evolve.go` 移动到 `memory` 包中作为一个导出的 `CompactIfDue`，接收一个 `*Config`。
- 为 `memory.Compact` 添加可选的基于文件锁定的互斥（`LockFile` 字段），这样并发的 evolve 就不会交错写入。

**对现有系统的影响**：
- 低中断——`compactMemoryIfDue` 是 `evolve.go` 中一个已经提取出来的函数，用配置参数替换其魔数是一个直接的提取。
- `memory.Compact` 需要在锁获取失败方面增加健壮性——"另一个进程正在压缩，跳过"对于自治操作来说是可接受的降级。

### 方向 P1：引入 `prompt.Retriever` 接口以解耦检索策略

**为什么需要**：当前的 `prompt.Retrieve` 是一个带有包级签名的纯函数：`func Retrieve(docs []Doc, query string, k int) []Doc`。对于 v1 BM25-lite 来说没问题，但升级到语义检索意味着：
- 更改正确的函数签名——这会破坏所有调用者。
- 向 `prompt` 包添加嵌入依赖项——违反零外部依赖约束。
- 没有地方来插入检索策略（例如，阶段感知的加权，针对特定查询的定制 top-K）。

一个 `Retriever` 接口允许零依赖的 BM25-lite 实现在 v2 中生存，同时允许在 *另一个* 包（`forge-ai` 用于基于嵌入的检索，`forge-ext` 用于插件检索器）中独立实现 v3。

**核心挑战**：
- 接口必须足够丰富以覆盖 BM25-lite 和语义检索：`type Retriever interface { Retrieve(ctx context.Context, docs []Doc, query string, k int) ([]Doc, error) }`——错误参数是新的，因为语义检索可能失败（嵌入 API 超时）。
- BM25-lite 的确定性契约（"相同输入 → 相同输出"）对于引用和审计是有价值的——语义检索天生是概率性的，这必须在接口文档中明确说明。
- 调用站点（`relevantADRs`）目前在没有适当错误处理的情况下使用 `Retrieve`——接口的 `error` 返回意味着处理检索失败。

**预期的架构变更**：
- `prompt` 包导出 `Retriever` 接口和一个 `NewBM25Retriever()` 构造函数。
- 当前的 `Retrieve` 函数成为 `bm25Retriever.Retrieve` 的一个薄包装或委托。
- 调用者（`relevantADRs` 和未来的使用）通过接口接收一个 `Retriever`——默认情况下，如果未指定，包会注入 `NewBM25Retriever()`。
- `Gather` 从纯函数变为需要依赖项（`Retriever`）的结构体方法——但保持向后兼容性。

**对现有系统的影响**：
- 中等侵入性：需要提取接口 + 创建结构体包装器 + 更新调用者。
- 新代码路径保持受控：默认构造避免了扰乱现有简单用例。
- 纯函数（`Retrieve`）的确定性测试可以在结构体方法（`bm25Retriever.Retrieve`）上重放。

### 方向 P2：跨进程协调的租约和锁

**为什么需要**：审查确认方向一的"跨进程协调"已被识别——它被 3 份独立文档覆盖，这一事实印证了它的重要性。然而，*在这些文档中尚未涉足*的是实现细节：具体的租约机制和锁策略。审查的附录仅引用了 `data-integrity` 文档，漏掉了两个关键的交叉引用（`orchestrator-consensus` 和 `memory-coherency`）。

目前的架构：
- 两个 `forge evolve` 进程在同一个项目目录上运行 → 都会写入 `memory.jsonl` → JSON 行交错。
- 两个 `forge run` 进程在同一个目标上运行 → 竞争工件输出目录。
- 没有领导者选举，没有心跳，没有租约。

**核心挑战**：
- ForgeOS 的领域模型是一个*图*（阶段、依赖）、一个*日志*（内存条目）和一个*文件系统*（工件）。没有像 etcd 这样的外部协调服务，你只能依靠文件系统锁定——这很脆弱。
- 独占租约（"一个 evolve 实例持有锁"）会阻止并行性——但 v2 为此的用例是"跨仓库协调"，而不是"共享 RAM 数据流"。
- 共享挂载点在 `flock` 行为方面因操作系统而异（Linux BSD 锁定 vs. POSIX vs. NFS）。

**预期的架构变更**：
- 一个 `coord/lock.go` 包，提供 `TryLock(path string, ttl time.Duration) (Lock, error)`，具有 `flock` 后备和进程内看门狗定时器用于 TTL 强制执行。
- 将每个 `forge evolve` 启动包装在一个协调的生命周期中：获取仓库锁 → 运行迭代 → 释放锁。
- 引入"观察者"模式，其中第二个 evolve 进程不运行完整的演化循环，而是仅监控进度并报告盲点——无需锁，无需写入。

**对现有系统的影响**：
- 低中断——锁机制是附加的，不是侵入性的。现有的单进程 evolve 循环保持不变。
- 然而，激活锁意味着每个 evolve 迭代在启动时进行 `flock` 检查——增加了 ~1ms 的启动开销。
- 共享挂载点上的 NFS 兼容性需要显式测试（`forge-core` 目前不测试 NFS）。

### 方向 P2：检查点和恢复的可观察性与 I/O 栅栏

**为什么需要**：我在当前代码库中没有看到这个被充分覆盖（审查没有讨论它）。目前的 evolve 循环有一个 `persist.Checkpoint`，但它缺乏：
- 检查点写延迟的可观察性（如果检查点需要 5 秒的 `os.Rename` 怎么办？）。
- 内存条目和文件系统工件之间的 I/O 栅栏（"所有工件持久化之后"的屏障）。
- 细粒度的恢复：目前，恢复意味着从上一个检查点重新启动整个演化——对于 48 小时的运行来说代价高昂。

**核心挑战**：
- `persist.Save` 使用 `os.Rename` 进行原子文件替换——这在 POSIX 系统上是原子的，但在所有文件系统上*不保证*在并发读取器之间可见。
- 内存条目被追加到 JSONL 并稍后压缩——恢复点不完全匹配到条目边界，除非压缩是幂等的。
- 长时间的 evolve 运行（>12 小时）积累了大量可以跳过恢复的易失性状态（迭代计数器、暂时的门失败）。

**预期的架构变更**：
- 一个 `observe.CheckpointStats` 钩子，报告检查点持续时间、字节大小和条目计数——这样 orchestator 就可以在检查点停顿超出阈值时决定提前终止。
- `persist.Checkpoint` 的一个可选 `IOCoordinator`，在检查点提交之前强制执行"所有文件句柄已刷新"屏障。
- 检查点元数据的顺序日志（不是每次覆盖），这样 operator 可以回滚到"N 个检查点之前"。

**对现有系统的影响**：
- 中等——新的抽象层（`IOCoordinator`，`CheckpointStats`），但现有的检查点路径接收默认实现。
- 调用者不必改变——默认实现保留当前行为（覆盖检查点，无可观察性）。

---

## 3. 接口设计建议

### 原则

1. **始终需要 `context.Context` 作为第一参数**用于任何进行 I/O 或阻塞操作的函数。这是审查的核心发现——`prompt.Gather` 和 `prompt.GatherCached` 进行文件 I/O 但没有上下文。修复此问题意味着以下签名模式：

   ```
   // BEFORE
   func Gather(repoRoot, query string) []string

   // AFTER
   func Gather(ctx context.Context, repoRoot, query string) ([]string, error)
   ```

   错误返回类型是必需的，因为超时/取消现在是一个可能的失败模式——调用者不能假设它永远不会失败。

2. **优先使用显式接口而非隐式满足**用于可扩展点。`prompt.Retriever` 应该是一个 Go 接口，而不是一个函数签名，这样：
   - 语义检索器可以在一个单独的包中实现，*无需*导入 `prompt`。
   - 单元测试可以在不触及文件系统的情况下模拟检索器。
   - 调用者通过依赖注入接收正确的实现（对于 dfault，使用 `NewBM25Retriever()`）。

3. **配置结构体，不是包变量**。`memory` 目前使用包级常量（`DefaultCompactThreshold`，`DefaultCompactKeepPerKind`）。这些应该是配置结构体上的字段：

   ```go
   type MemoryConfig struct {
       CompactThreshold    int
       KeepPerKind         int
       CompactAgeSeconds   int64
       CompactInterval     int // iterations between compactions
       LockFile            string // "" = no locking
   }
   ```

   这样可以使得单个测试可以覆盖不同的配置，并且 evolve 循环可以基于运行时情况（阶段、运行时长、条目计数）来选择设置。

### 新的抽象层

| 抽象 | 位置 | 责任 | 必需程度 |
|---|---|---|---|
| `Retriever` 接口 | `prompt/retriever.go` | 对文档集合进行评分和排序 | P1 — 为 v3 语义检索提供编码空间 |
| `ContextProvider` 接口 | `prompt/provider.go` | 包装 `Gather` + `GatherCached` 到一个单一的可模拟组合中 | P2 — 用于测试的解耦 |
| `IOCoordinator` | `persist/coord.go` | 在检查点之前强制执行"磁盘已同步"栅栏 | P2 — 用于长时间运行的生产 |
| `LockManager` | `coord/lock.go` | per-repo 文件锁，包含 TTL 和看门狗 | P2 — 用于跨进程协调 |

### 向后兼容性

- 所有接口都应提供一个零配置的默认值（例如，`NewBM25Retriever()`，`DefaultIO()`，`NoopLocker()`），以便现有的单进程无人值守使用保持工作。
- 签名变更（`Gather` 添加 `ctx` 和 `error`）是一个突破性变更——但它发生在 v2 中，此时 forge-core 的 API 消耗量很低。在 v3 锁定之前执行此中断。
- `memory.Config` 应该保持所有字段为可选，并回退到当前默认值，这样 `&MemoryConfig{}` 就等同于今天的行为。

---

## 4. 技术选型

### 需要新技术吗？

根据审查，**不需要**。所有识别的方向都可以而且应该在 Go 标准库内解决：

| 所需能力 | 方法 | 标准库支持 |
|---|---|---|
| I/O 超时/取消 | 用 `context.Context` 包装 `os.ReadFile`，每个操作使用单独的 goroutine + `select` | `context` 包 ✓ |
| 基于文件的锁定 | 为跨进程互斥使用 `flock`（Linux）或 `LockFileEx`（Windows） | `os`（`FcntlFlock`）/ `syscall` |
| 可插拔检索器 | Go 接口 + 构造函数注入 | 语言特性 ✓ |
| 配置结构体 | 普通 Go `struct` + `New*Config(options ...func(*Config))` 模式 | 语言特性 ✓ |
| 可观察的检查点 | 带 `time.Now()` 和 `log.Printf` 或可选 `statsWriter` 的计时包装器 | `time` + `log` ✓ |

**唯一的例外**是 v3 语义检索，它将需要：
- 嵌入模型客户端（外部 HTTP 调用，可能使用 `net/http`，已经是标准库）。
- 向量存储（目前尚不确定——嵌入 API 返回内联向量，所以对于 v3 的规模来说，内存中的余弦距离评分可能就足够了）。

### 第三方依赖评估标准

对于任何未来的第三方依赖，标准应该是：

1. **对纯标准库的豁免仅在以下情况下批准**：需要平台级特性（例如，Windows 上的 `LockFileEx` 通过 `golang.org/x/sys`，其与 Go 版本兼容）或者性能关键型的标准库替代品被证明是瓶颈（尚未证明）。

2. **许可证合规性**：必须与 ForgeOS 的许可证兼容。首选 MIT/Apache 2.0。

3. **零传递依赖**：如果一个库引入 >2 个间接依赖，它就是候选——ForgeOS 的"零外部依赖"政策明确禁止传递依赖。

4. **弃用记录**：每个新的依赖必须在 `DECISIONS.md` 中伴随一个 ADR，说明为什么不能通过纯标准库解决。

### 自建 vs. 采购决策

| 能力 | 推荐 | 理由 |
|---|---|---|
| 嵌入检索（v3） | **自建**：在 `Retriever` 接口后面使用 `net/http` 调用嵌入 API，再加上内存向量评分 | 没有现成的 Go 库适合该领域且零依赖 |
| 分布式锁 | **自建**：基于 `flock` 的锁带有 TTL 看门狗定时器 | 领域简单；外部 CRDT/协调库会引入数十个依赖 |
| 跟踪/可观察性 | **推迟决定**：v2 的 JSONL 跟踪是足够的。如果 v3 需要 OpenTelemetry，它应该是一个可选适配器 | OTel Go SDK 是轻量级的，但仍然是外部依赖 |
| 跨宿主适配器（CC、Codex、Gemini CLI） | **薄适配器模式**：每个宿主 1 个文件，使用 CLI 期望的接口 | 适配器不是共享库——它们每个宿主特定，所以没有"共享依赖"的问题 |

---

## 5. 实施路线图

### 优先级矩阵

| 方向 | 价值 | 风险 | 工作量 | 优先级 |
|---|---|---|---|---|
| Prompt I/O 超时（上下文感知的 I/O） | 高（修复生产级缺陷） | 低（受控签名变更） | 小（2-3 个包中的 ~10 个位置） | **P0** |
| 可配置的内存压缩 | 中（启用生产调优） | 低（提取+配置） | 小（`memory/config.go` + `evolve.go` 更新） | **P1a** |
| `Retriever` 接口提取 | 中（解锁 v3 检索） | 低（非破坏性接口提取） | 中（接口 + 包装器 + 调用者更新） | **P1b** |
| 跨进程协调（租约/锁） | 高（分布式限制） | 中（文件锁语义 + NFS） | 大（新 `coord` 包 + evolve 集成） | **P2a** |
| 检查点可观察性与 I/O 栅栏 | 中（恢复可见性） | 低 | 中（`persist` 增强） | **P2b** |

### 阶段划分

**阶段 1 （P0 ~P1, 当前 sprint 或下一个 sprint）**

目标：修复生产级缺陷，为 v3 可扩展性编码空间。

```
里程碑 M1：上下文感知的 I/O
  - 为 prompt.Gather + GatherCached 添加 (ctx, error)
  - 用 context-aware goroutine 包装器包装 os.ReadFile/os.ReadDir
  - 更新所有调用者（主要是在 orchestator 中）
  - 添加测试：超时 → 返回错误，取消 → 短路
  
里程碑 M2：可配置的内存压缩
  - 提取 memory.Config（所有当前常量为默认值）
  - 将 compactMemoryIfDue 移至 memory.CompactIfDue(*Config)
  - 添加集成测试：不同间隔下的压缩行为
  
里程碑 M3：Retriever 接口
  - 定义 prompt.Retriever 接口
  - 将当前函数重构为 *bm25Retriever
  - 更新 relevantADRs 通过接口接收 Retriever
  - 为接口添加测试：模拟 + BM25 满足
```

**阶段 2 （P2, 下一轮 sprint）**

目标：分布式准备，生产可观察性。

```
里程碑 M4：基于文件的锁与租约
  - 实现 coord.LockManager（flock/windows 等价物）
  - 将 evolve 启动包装在锁获取中
  - 添加 TTL 看门狗定时器用于租约到期
  - 测试：并发 evolve 进程，锁超时，优雅降级
  
里程碑 M5：检查点增强
  - 添加 CheckpointStats（持续时间，大小，条目计数）
  - 实现可选 IOCoordinator 用于 fsync 栅栏
  - 日志检查点元数据到单独的文件（不覆盖）
  - 测试：混乱关机后的恢复
```

### 风险与缓解

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| `prompt.Gather` 签名变更破坏外部消费者 | 低（v2 API 消耗量低） | 高（中断现有编排） | 添加 `GatherCompat` 用于过渡；在 `DECISIONS.md` 中记录中断 |
| 基于 `flock` 的锁在 NFS/CIFS 上行为不同 | 中 | 中（仅分布式场景受影响） | 记录已知的限制；为 NFS 添加回退到无操作锁 |
| `Retriever` 接口过于宽泛导致强制 v3 约束 | 低（如果有必要，可以给接口添加方法） | 低 | 保持初始接口最小：`Retrieve(ctx, docs, query, k) ([]Doc, error)`。以后如果需要，可以使用子类型 |
| 并发压缩损坏 memory.jsonl | 低（被紧凑互斥防止） | 高（知识丢失） | 在阶段 2 锁准备就绪之前默认禁用并发压缩；添加 `memory.Compact` 原子写入路径 |
| 工程师将接口提取视为"不必要的抽象" | 中 | 低（团队文化问题） | 动机：使 v3 语义检索成为可能，而无需突破 v2 包边界。将接口提取与 ADR 关联 |

### 关键架构序列图（高层）

```
┌──────────────────────────────────────────────┐
│  evolve loop (for evolve.go:evolveCmd)       │
│                                                │
│  for i:=0; ; i++ {                            │
│    ctx, cancel := context.WithTimeout(parent,  │
│                            phaseTimeout)       │  ← P0: 每个阶段一个 ctx
│    ctx = context.WithValue(ctx, "iteration",i) │
│                                                │
│    promptCtx := ctx                            │
│    ctxList := prompt.Gather(promptCtx, root,   │  ← P0: 现在使用 ctx
│                              query)            │
│                                                │
│    if i%cfg.CompactInterval == 0 {             │  ← P1: cfg 来自 memory.Config
│      memory.CompactIfDue(cfg, root, logln)     │  ← P1: cfg 包含锁定策略
│    }                                           │
│                                                │
│    // 稍后：                                    │
│    // coord.AcquireLock(root, ttl)            │  ← P2: 跨进程协调
│    // persist.CheckpointWithStats(stats)      │  ← P2: 可观察的检查点
│                                                │
│    cancel()                                    │
│  }                                             │
└──────────────────────────────────────────────┘
```

### 与现有路线图的一致性

这些方向与 `.agent/ROADMAP.md` 中的 v2→v3 过渡产生共鸣：

- **P0（上下文感知的 I/O）** 将 `forge-core` 的运行时从"在受控条件下可运行"提升到"在中断期间可生存"——这是使 v2 为自治运行做好生产准备所必需的。
- **P1 方向** 为 v3 特性（语义检索、分布式运行）提供编码空间，*无需*重构 v2 包。
- **P2 方向** 直接映射到路线图的门控项目："跨厂商模型池"在多个进程路由到一起时需要一个协调层。

没有方向需要改变路线图——它们增强了当前的结构以承载即将到来的工作负载。

---

## 结论

审查正确地识别了 **Prompt I/O 超时** 作为最干净、真正的"新方向"——`prompt.Gather` 缺少 `context.Context` 是一个可证明的缺陷，在代码级别得到验证，不受以前文档覆盖的影响。这是架构的 P0 修复方向。

**可配置的内存压缩** 作为方向二的替换候选是合理的，前提是重构承认压缩*已经*被连接（审查纠正了该点），但使其可配置且并发安全为生产实时代码带来了真正的价值。

方向一（跨进程协调）在没有新实现细节的情况下不应被重新提出——三份以前的文档已经覆盖了"什么"；现在需要的是"如何"（具体的租约机制、锁策略和文件格式协调）。我上面的 P2 提案提供了这些细节。

高价值的路线总结：**在 P0 修复 Prompt I/O → 提取 P1 可扩展点（压缩配置，检索器接口）→ 为分布式协调启动 P2 基础设施**。每个阶段都解锁下一阶段，而无需回溯。
