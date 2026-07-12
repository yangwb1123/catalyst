现在我已完整阅读了输入文档和项目上下文（包括 `.agent/ARCHITECTURE.md`、`PROJECT.md`、`ROADMAP.md`、`CURRENT_SPRINT.md`、`AGENTS.md`），下面给出架构分析。

---

# 架构分析：ForgeOS 四个未被覆盖的扩展方向

## 1. 架构评估

### 当前架构的优势

ForgeOS 的架构决策整体上是高度纪律性的，值得肯定的关键设计包括：

**1.1 零外部依赖原则（Go 运行时）**
`go.mod` 纯标准库、零 `require` 是一条极具远见的决策。在当前阶段，它带来了：
- 构建确定性——没有传递依赖爆炸的风险
- 审计简单性——任何安全审查只需审查 Go stdlib + 第一方代码
- CI 可靠性——没有 `go mod download` 失败、依赖源不可用、supply-chain 攻击面
- 二进制体积可控——没有重型框架拖入

这条原则直接影响了方向一（文件锁方案受限）和方向三（没有 HTTP 客户端的超时封装），但利远大于弊。

**1.2 分层清晰的组织架构**
从 `forgae-core/internal/` 的 18 个包可以看出清晰的层次：
- **域层**：`asset`、`gate`、`mode`、`risk`、`converge`——核心领域模型
- **引擎层**：`orchestrator`、`prompt`、`memory`、`routing`、`trace`——运行时引擎
- **基础设施层**：`persist`、`doctor`、`migrate`、`attribution`、`yaml2json`、`yamlpath`——支撑设施
- **适配器层**：`cmd/forge`——CLI 胶水，薄薄一层

这种依赖方向（domain ← engine ← infrastructure ← CLI）与 `AGENTS.md` 规定的"interfaces → application → domain 单向向内"一致。

**1.3 诚实标记的文化**
大量 `NOTE:` 注释、`DEFERRED-BY-DESIGN` 分类、`honesty` 贯穿代码和文档——这是架构治理中极难得的品质。例如 Sprint 31 中 `blocking: false` 字段镀金被主动拦截。

### 当前的局限性

**1.4 单进程假设的架构天花板**
这是方向一的根源问题。从架构图看（ARCHITECTURE.md），ForgeOS 的"脊柱"（Idea→Production）和"中枢旋钮"（mode×lifecycle）都是单控制流设计的产物：

```
forge evolve → LoopEngine → runIteration → runAgentPhase → ...
```

整个编排器假设自己是 `.forge/` 目录的唯一写入者。但 `forge run` 和 `forge evolve` 是两个独立的 CLI 入口，没有设计层面的协调协议。这是**架构层面的单进程盲点**——不是"我们已经考虑过但推迟了"，而是从来没有被建模为架构问题。

**1.5 Context 传播链不完整**
Sprint 27-29 大量工作（信号处理、context 传播、`ctx context.Context` 参数化）集中在编排器核心路径，但传播链在 `prompt.Gather` 处断裂（方向三）。这不只是一个遗漏，而是**架构层面缺少"可取消 I/O 必须贯穿整个请求链路"的规范**——如果有一条架构原则要求"所有 I/O 操作都必须接受 `context.Context`"，方向三不会成为 Sprint 27 的遗留缺口。

**1.6 产品契约与实现的不一致**
方向二揭示的 `memory.go` 自述 "grow-only log，绝不 rewrite" 但 `memory_compact.go` 已是 rewrite 机制，说明**架构文档/注释与代码实现没有同步机制**。方向四的 `allGates()` 硬编码列表与 workflow YAML 中 `required_gates` 的引用是另一处此类问题。

### 架构债务清单

| 债务 | 严重程度 | 估计还债成本 |
|------|---------|------------|
| 缺少跨进程协调协议 | **高** — 数据损坏风险 | 中型（2-3 个新包 + 测试） |
| Context 传播链断裂 | **中** — 生产环境永久阻塞风险 | 小型（1 个 util 包装 + 签名修改） |
| 自动压实未接入循环 | **中** — 无界增长资源泄漏 | 小型（1 个检查点插入 + 并发锁） |
| Gate 名硬编码且无验证 | **低** — 拼写错误风险，概率低但影响大 | 小型（注册表 + 验证） |
| memory.go 自述与实现矛盾 | **低** — 只在代码审查时暴露 | 极小（注释修改） |

---

## 2. 扩展方向

### 方向 A（P0）：跨进程运行时协调层

**为什么需要**
这是当前架构中最接近"数据损坏"类风险的缺口。输入文档分析的三类损坏模式（checkpoint 回退、trace seq 破坏、memory 乱序）都真实存在。随着 ForgeOS 从单用户工具向 CI/CD 集成发展（方向一的"开发者 + CI 同时运行"高概率场景），这个问题的发生概率从"偶尔"变为"必然"。

**核心挑战**

| 挑战 | 难度 | 说明 |
|------|------|------|
| 零外部依赖约束 | 中 | `syscall.Flock` 是 Unix-only，Windows 需要 `LockFileEx`——需在两个平台抽象 |
| 读/写锁粒度选择 | 高 | 共享锁（读）vs 排他锁（写）的选择直接影响 `forge status/doctor` 的并发性 |
| 向后兼容 | 低 | 锁机制只在新版本启航时生效，旧版本 `.forge/` 目录无锁文件可优雅退路 |
| 锁释放可靠性 | 中 | 进程崩溃后锁是否自动释放（`flock` 由内核释放，OK）；但需要防范"僵尸锁" |

**预期架构变更**

```
internal/
  lock/               ← 新包：跨平台文件锁抽象
    lock.go           ← Lock 接口（共享/排他）
    lock_unix.go      ← syscall.Flock 实现
    lock_windows.go   ← LockFileEx 实现（可选 defer）
  runtime/            ← 新包：运行时上下文
    context.go        ← 进程标识符、Host 信息
```

影响现有包：
- `internal/persist`：`Save` / `Load` 增加锁获取/释放
- `internal/trace`：`Emit` 获取共享写锁
- `internal/memory`：`Append` / `Compact` / `Load` 均加锁
- `cmd/forge`：所有入口点（main.run/evolve/doctor）启动时尝试获取锁

**对现有系统的影响**
- **兼容性**：无锁的旧 `.forge/` 目录依旧可读写（带警告）
- **性能**：锁获取是微秒级操作，对性能基本无影响
- **行为变化**：并发 `forge run` 会有一个等待/失败，不再是静默损坏——这是改进而非退化

### 方向 B（P0）：上下文传播链完整化

**为什么需要**
方向三描述的"NFS/FUSE 场景永久阻塞"是生产级风险。ForgeOS 已经声明自己是"长期（24h）无人值守"系统——在这种环境下，任何一个同步阻塞点都可能导致整个循环永久挂起，且无人发现。

**核心挑战**

| 挑战 | 难度 | 说明 |
|------|------|------|
| 零依赖限制下的可取消 I/O | 中 | Go 1.26 标准库已有 `os.ReadFile`，不支持 context。需要自建 goroutine+select 包装 |
| API 签名变更 | 中 | `prompt.Gather` 等多个导出函数的签名需要新增 `ctx context.Context` 参数——影响所有调用者 |
| 降级策略的语义定义 | 高 | "部分文件读取超时"下，prompt 应：① 跳过该源 ② 注入部分内容 ③ 报错退出？三种策略各有适用场景 |
| 超时时间的选择 | 中 | 固定超时 vs 可配超时 vs 自适应——需要在 prompt 构建的上下游之间协调 |

**预期架构变更**

```
internal/prompt/
  context.go          ← 新文件：可取消文件读取包装 readFileWithContext
  gather.go           ← 重构：签名增加 ctx 参数，os.ReadFile 替换为 readFileWithContext
  cache.go            ← 修改：GatherCached 接受 ctx，缓存预热可取消
```

调用链路中需要修改的签名：
- `prompt.Gather(ctx, repoRoot, query)` 
- `prompt.GatherCached(ctx, repoRoot, runID)`
- `gatherContext(ctx, ...)` in `cmd/forge/prompt_context.go`
- `Engine.runAgentPhase` 已有 ctx，只需传递下去

**对现有系统的影响**
- **兼容性**：签名变更需要所有调用者同步修改——但链上调用都是 forge-core 内部代码，影响可控
- **行为变化**：增加超时后，慢文件系统会导致 prompt 内容减少而不是 agent phase 永远不启动——这是改进
- **回归风险**：需要确保超时路径下的降级输出不会导致 agent 幻觉（例如"ADR 内容为空"被解析为"没有 ADR 约束"）

### 方向 C（P1）：Gate Registry + 名称目录验证

**为什么需要**
方向四描述的"静默安全缺失"（gate 名拼写错误导致安全闸门被静默跳过）虽然在概率上低于方向一，但影响的严重程度同样高——安全闸门被跳过在合规审计中可能是致命问题。

**核心挑战**

| 挑战 | 难度 | 说明 |
|------|------|------|
| 区分"已知但被 mode 过滤"与"拼写错误" | 中 | 前者是预期行为（explorer 跳过 test），后者是配置错误。需要 registry 区分 |
| 与现有 `internal/gate` 的关系 | 低 | 当前 `internal/gate/gate.go` 只是按名调度，没有注册机制 |
| 自定义 gate 的扩展 | 中 | 当第三方/项目级 gate 被注册后，`allGates()` 不应只返回硬编码列表 |

**预期架构变更**

```
internal/gate/
  registry.go         ← 新文件：Gate 注册表（Register / Registered / All）
  gate.go             ← 修改：从硬编码 fullGates 改为由 registry 推导
```

`internal/mode/mode.go` 的 `allGates()` / `fullGates` 改为从 registry 推导。

`internal/doctor/models.go` / `harness/check.py` 增加验证：
- 读取 workflow YAML 的 `required_gates`
- 与 registry 交叉引用
- 输出警告 "unknown gate 'secutiry' in workflow build.yml"

**对现有系统的影响**
- **兼容性**：注册表增加 `init()` 注册调用，对已有 gate 零行为变化。`fullGates` 仍返回相同集合
- **行为变化**：新增 `forge validate --gates` 子命令，新警告日志
- **安全**：唯一的行为变化是"以往静默跳过的拼写错误现在会输出 WARN 日志"——是安全增强

### 方向 D（P1）：Memory 自动压实接入 Evolve 循环

**为什么需要**
方向二的问题本质是一个"产品契约缺口"——代码已有压实能力，但用户不知道何时需要手动触发。在 "24h 无人值守" 场景下，任何需要手动干预的机制都是不可接受的。

**核心挑战**

| 挑战 | 难度 | 说明 |
|------|------|------|
| `Append` 与 `rewriteStore` 并发安全 | 中 | 当前 `Append` 使用 `O_APPEND`，`rewriteStore` 使用 tmp+rename。如果没有写锁，rewrite 期间的另一进程 Append 可能丢失 |
| 压实阈值选择 | 低 | `DefaultCompactThreshold(500)` 是否合适——需要考虑小项目（50 条）和大项目（5000 条）的不同场景 |
| Load 性能退化 | 中 | 每次 Append 后 `invalidateLoadCache()` 导致下次 Load 重新全文读取——这在大 store 下是 O(n) 操作 |

**预期架构变更**

```go
// internal/memory/memory.go — 增加 AutoCompact 接入点
func (s *Store) AutoCompact(ctx context.Context) error {
    s.mu.Lock()
    defer s.mu.Unlock()
    if s.len() >= s.compactThreshold {
        return s.Compact(...)
    }
    return nil
}
```

```go
// internal/orchestrator/loop.go — 在 runIteration 尾部插入
func (e *Engine) runIteration(...) error {
    // ... existing logic ...
    if err := e.Memory.AutoCompact(ctx); err != nil {
        log.Warn("memory compact failed (non-fatal): %v", err)
    }
    return nil
}
```

**对现有系统的影响**
- **兼容性**：自动压实默认开启，但行为与手动 `forge memory-prune` 完全一致——用户不会感到变化
- **性能**：压实可能耗时数毫秒到数秒（取决于 store 大小），但只发生在超过阈值之后
- **行为变化**：`memory.jsonl` 不会无限增长——这是用户预期的行为，不算退化

### 方向 E（P2）：运行时健康自检与诊断闭环

**额外的方向**——从四个方向之外的综合分析得出的第五方向。

**为什么需要**
当前架构中有多个"无声失效"模式：方向一的跨进程数据损坏（无检测代码）、方向四的 gate 名拼写错误（无验证）、方向二的 memory 增长（无告警）。这些问题的共同模式是：**系统没有在运行时自我检测其内部状态的一致性**。`forge doctor` 存在但只做静态扫描，不做运行时诊断。

**核心挑战**

| 挑战 | 难度 | 说明 |
|------|------|------|
| 诊断的侵入性 | 中 | 运行时健康检查不能影响正常操作——需要异步、低优先级 goroutine |
| 健康指标的选取 | 高 | 太多指标是噪音，太少会漏掉问题——需要在"可观测性"和"可操作性"之间平衡 |
| 告警阈值 | 中 | 过于敏感导致告警疲劳，过于宽松导致漏报 |

**预期架构变更**

```
internal/health/
  check.go            ← 新包：运行时健康检查
  metrics.go          ← 运行时指标定义
```

诊断点：
- `.forge/` 目录锁状态检查（与方向一配合）
- Memory store 增长趋势告警（与方向二配合）
- Trace seq 连续性校验
- Gate 注册表一致校验（与方向四配合）

**对现有系统的影响**
- 新增包，不影响现有功能
- `forge doctor` 可扩展调用运行时检查（仅当 `.forge/` 目录存在时）

---

## 3. 接口设计建议

### 3.1 锁抽象层接口设计

方向一需要一个跨平台的文件锁接口。关键设计原则：

```go
// internal/lock/lock.go
package lock

type Type int
const (
    Shared  Type = iota  // 读锁：forge status/doctor
    Exclusive            // 写锁：forge run/evolve/memory-prune
)

// FileLock 代表一个已获得的文件锁。
// 调用者应保证在锁不再需要时调用 Close 释放。
// 如果进程崩溃，操作系统会自动释放锁（flock 语义）。
type FileLock interface {
    Close() error
}

// Acquire 尝试在指定路径上获取文件锁。
// 如果锁被占用且 block=true，则阻塞直到获得锁；
// 如果 block=false，则立即返回错误。
// timeout 为 0 表示无限等待（仅在 block=true 时有效）。
func Acquire(path string, typ Type, block bool, timeout time.Duration) (FileLock, error)
```

**设计决策权衡**

| 选项 | 优点 | 缺点 |
|------|------|------|
| **A: 全局排他锁**——任意写操作获取全局锁 | 实现简单，最安全 | `forge status` 无法与 `forge run` 并发 |
| **B: 文件级细粒度锁**——每个 `.forge/` 下文件独立锁 | 最高并发性 | 实现复杂，存在死锁风险（进程 A 锁 checkpoint，等待 memory；进程 B 锁 memory，等待 checkpoint） |
| **C: 推荐：文件级锁 + 一致的获取顺序** | 并发性好，无死锁 | 需要在文档中规定锁获取顺序（checkpoint → trace → memory） |

**推荐 C**：`./forge/checkpoint.lock`、`.forge/trace.lock`、`.forge/memory.lock` 三个锁文件，获取顺序为 checkpoint → trace → memory。

### 3.2 Context 传播的接口契约

方向三需要在整个 I/O 链上强制 context 传播。设计原则：

```go
// internal/util/read.go — 可取消文件读取包装
package util

// ReadFileWithContext 读取指定文件，受 context 控制。
// 如果 ctx 在读取完成前取消，返回 context.Cause(ctx) 的错误。
// timeout 为 0 表示不设置超时（仅响应取消）。
func ReadFileWithContext(ctx context.Context, path string, timeout time.Duration) ([]byte, error)
```

**关键决策**：`prompt.Gather` 的签名变更需要考虑向后兼容

| 选项 | 优点 | 缺点 |
|------|------|------|
| **A: 直接修改签名**：`Gather(ctx, repoRoot, query)` | 符合 Go 惯例，编译时检查 | 所有调用处需同步修改 |
| **B: 新增 WithContext 变体**：保留原 `Gather` + 新增 `GatherWithContext` | 向后兼容 | API 膨胀，两套路径需要分别维护 |
| **C: 推荐：版本化的包**：`prompt.Gather` 加 ctx 参数 + 在一 commit 中修改所有调用处 | 单一维护路径 | — |

**推荐 C**，原因：所有调用者都在 forge-core 内部，没有外部消费者需要兼容。

### 3.3 Gate Registry 接口

```go
// internal/gate/registry.go
package gate

// Runner 是 gate 执行的接口。每个已注册的 gate 必须提供一个 Runner。
type Runner interface {
    Run(ctx context.Context, dir string) (*Result, error)
}

// RunnerFunc 是 Runner 的函数适配器。
type RunnerFunc func(ctx context.Context, dir string) (*Result, error)

// Register 注册一个 gate 名称和其运行器。
// 如果名称已注册，panic（应在 init() 时间检测到冲突）。
func Register(name string, runner Runner)

// Registered 返回所有已注册的 gate 名称。
func Registered() []string

// Lookup 返回指定 gate 名称的运行器。error 表示未知 gate。
func Lookup(name string) (Runner, error)
```

**设计权衡**

| 选项 | 优点 | 缺点 |
|------|------|------|
| **A: init() 注册**——每个 gate 包在 init() 中自我注册 | 自动发现，新增 gate 无需手动注册 | 隐式依赖，测试中难以 mock |
| **B: 显式注册**——在 `cmd/forge` 的 `init()` 中统一注册所有 gate | 显式依赖，测试友好 | 新增 gate 需要手动添加注册行 |
| **C: 推荐：A + 测试辅助**——init() 注册 + registry 提供 `Clear()` 用于测试 | 生产代码简洁 + 测试可隔离 | `Clear()` 需要保护不被生产代码误用 |

---

## 4. 技术选型

### 4.1 文件锁：自建 vs 第三方库

| 对比项 | 自建（`internal/lock/`） | 第三方（如 `golang.org/x/sys`） |
|--------|--------------------------|-------------------------------|
| 依赖 | 无（纯 stdlib） | 增加一个外部依赖 |
| 平台支持 | Unix（`syscall.Flock`）+ Windows 可选 | 更完善的平台抽象 |
| 代码量 | ~100 行 | 已有实现 |
| 审计成本 | 低 | 需要审计第三方代码 |
| 与工程红线一致 | ✅ 完全一致（forge-core 零依赖） | ❌ 违反"纯标准库零依赖" |

**结论**：**自建**。`syscall.Flock` 是 POSIX 标准系统调用，Go stdlib 已暴露；Windows 的 `LockFileEx` 是核心 Win32 API，同样可通过 `syscall` 调用。100 行以内的抽象不值得引入外部依赖。

### 4.2 可取消 I/O：goroutine + select vs 第三方

方向三的可取消文件读取只有一种合理方案：**goroutine + select + context**。Go 标准库不提供可取消的 `os.ReadFile`，只能用 goroutine 包装：

```go
func ReadFileWithContext(ctx context.Context, path string) ([]byte, error) {
    type result struct {
        data []byte
        err  error
    }
    ch := make(chan result, 1)
    go func() {
        data, err := os.ReadFile(path)
        ch <- result{data, err}
    }()
    select {
    case r := <-ch:
        return r.data, r.err
    case <-ctx.Done():
        return nil, ctx.Err()
    }
}
```

这个模式是 Go 社区经充分验证的惯用法，无需外部依赖。

### 4.3 Registry 模式：使用 Go interface 而非框架

Gate registry 使用标准 Go interface（`Runner`）+ `sync.Map` 即可，没有任何框架需求。这与 forge-core 现有的零依赖原则完全一致。

### 4.4 技术选型总结

所有五个方向的技术选型都**不需要引入新的外部依赖**：

| 方向 | 技术方案 | 依赖影响 |
|------|---------|---------|
| 方向一（跨进程锁） | `syscall.Flock` / `LockFileEx` | 零新增依赖 |
| 方向二（自动压实） | `sync.Mutex` + 现有 `Compact` | 零新增依赖 |
| 方向三（可取消 I/O） | goroutine + context | 零新增依赖 |
| 方向四（Gate Registry） | 标准 Go interface + `sync.Map` | 零新增依赖 |
| 方向五（健康自检） | 现有 `log` 包 + 新 `internal/health` | 零新增依赖 |

这是 forge-core "零外部依赖"纪律的直接收益——架构扩展不需要纠结于引入什么框架。

---

## 5. 实施路线图

### 优先级矩阵

| 方向 | 影响严重度 | 发生概率 | 检测难度 | 综合优先级 |
|------|----------|---------|---------|-----------|
| 方向一 · 跨进程协调 | 高（数据损坏） | 中-高 | 极难 | **P0** |
| 方向三 · Prompt I/O 超时 | 中（永久阻塞） | 低-中 | 极难 | **P0** |
| 方向四 · Gate 名验证 | 高（安全跳过） | 低 | 高 | **P1** |
| 方向二 · 自动压实 | 低（资源浪费） | 高 | 中 | **P1** |
| 方向五 · 健康自检 | 中（检测延迟） | 中 | — | **P2** |

### 阶段划分

**阶段一（当前 Sprint 或 Sprint+1）— P0 两个方向**

```
里程碑：跨进程安全 + I/O 韧性
预计工作量：4-5 个 agent（2-3 方向一 + 1-2 方向三）
```

| 任务 | 包 | 工作量 |
|------|----|--------|
| `internal/lock/` 文件锁抽象 | 新建 | 小型 |
| `internal/lock/lock_unix.go` Flock 实现 | 新建 | 小型 |
| `internal/runtime/context.go` 进程标识 | 新建 | 极小 |
| `internal/persist/checkpoint.go` 加锁 | 修改 | 小型 |
| `internal/trace/trace.go` 加锁 | 修改 | 中型（需要原子性调整） |
| `internal/memory/memory.go` 加锁 | 修改 | 中型（需要解决 Append+Compact 并发） |
| `cmd/forge/main.go` 启动加锁 | 修改 | 小型 |
| `internal/util/readcontext.go` 可取消 I/O | 新建 | 小型 |
| `internal/prompt/gather.go` 签名+实现修改 | 修改 | 中型 |
| 调用链全面接线 | 修改 | 中型 |
| 测试：锁竞争并发测试 | 新建 | 中型 |
| 测试：NFS 模拟超时测试 | 新建 | 中型 |

**阶段二（下一 Sprint）— P1 两个方向**

```
里程碑：gate 名安全 + 无界增长消除
预计工作量：2-3 个 agent（1-2 方向四 + 1 方向二）
```

| 任务 | 包 | 工作量 |
|------|----|--------|
| `internal/gate/registry.go` 注册表 | 新建 | 小型 |
| 现有 gate 的 `init()` 注册 | 修改 | 小型 |
| `internal/mode/mode.go` 从硬编码到 registry | 修改 | 小型 |
| `internal/doctor/models.go` gate 名称验证 | 修改 | 小型 |
| `harness/check.py` gate 交叉引用检查 | 修改 | 小型 |
| `forge validate --gates` 子命令 | 新建 | 中型 |
| `internal/memory/auto_compact.go` | 新建 | 小型 |
| `internal/orchestrator/loop.go` 接入 | 修改 | 小型 |
| Append/Compact 并发安全（与阶段一重叠） | 修改 | 中型 |
| 测试：gate 拼写错误检测 | 新建 | 小型 |
| 测试：自动压实长运行模拟 | 新建 | 中型 |

**阶段三（规划中）— P2 一个方向**

```
里程碑：运行时健康自检能力
预计工作量：2 个 agent
```

| 任务 | 包 | 工作量 |
|------|----|--------|
| `internal/health/` 包骨架 | 新建 | 小型 |
| 方向一锁状态检查（与阶段一重叠） | 新建 | 小型 |
| Memory 增长趋势告警 | 新建 | 小型 |
| Trace seq 连续性校验 | 新建 | 中型 |
| `forge doctor` 扩展 | 修改 | 小型 |

### 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| **方向一锁死锁** | 中 | 进程 hang | 采用一致的锁获取顺序（checkpoint→trace→memory）；`Acquire` 支持 timeout；测试中注入死锁场景验证 timeout 释放 |
| **方向一 Windows 兼容** | 低 | Linux-only | 初期只做 Unix（Flock），Windows 实现列为 P2 defer。Go build tag 隔离 |
| **方向三超时值误设** | 中 | 正常文件系统下超时误触发 | 默认 timeout=30s（远超正常读取时间）；可配置；生产环境中日志记录每次超时事件 |
| **方向四现有 gate 注册遗漏** | 低 | `Registered()` 返回不完整 | 添加集成测试验证 Registered() 与 `mode.go` 硬编码列表一致；gate 验收条件之一 |
| **方向二压实与 Append 竞态** | 中 | 丢失 Append | 采用 `sync.Mutex` 互斥——`Append` 和 `rewriteStore` 不能并发。性能代价可以接受（Append 是微秒级） |
| **修改量超过文件行数上限** | 中 | 违反工程红线 | 预先估算每个文件的增长；在文件接近 500 行时先拆分再修改（"先拆分，再继续"纪律） |
| **上下文传播链修改影响范围超预期** | 中 | 编译错误连锁 | 所有调用者都在同一 repo 内；一次全量 `go build` 可发现所有断点；分步实现（先加 context 签名后再改实现） |

### 决策节点

有两个关键架构决策需要在实施之前做出：

**决策一：锁粒度——全局排他锁 vs 文件级细粒度锁**

| 标准 | 全局排他锁 | 文件级细粒度锁 |
|------|-----------|---------------|
| 实现复杂度 | 低 | 中-高 |
| 并发性 | 一个写锁阻塞所有读操作 | 读操作可并发 |
| 死锁风险 | 无 | 有（需一致获取顺序） |
| 测试工作量 | 低 | 中 |
| 推荐 | — | **推荐** —— 方向一的三个关键文件（checkpoint/trace/memory）的读写模式差异大，全局锁不必要地限制了 `forge status` 的可用性 |

**决策二：超时降级策略——prompt 内容降级 vs agent phase 失败**

| 标准 | 降级（跳过不可读源） | 失败（报错退出） |
|------|--------------------|-----------------|
| 健壮性 | 高（agent 能用残留 context 继续） | 低（每次 NFS 抖动的都会中断 workflow） |
| 风险 | agent 可能输出"没有 ADR 约束"的错误结论 | 操作员需要重新触发 workflow |
| 日志清晰度 | 需要清晰日志 "ADR unavailable due to I/O timeout" | 错误明显，操作员看到就知问题 |
| 推荐 | **推荐降级策略**——但必须在 prompt 中注入"⚠ 部分文件因 I/O 超时未能读取"的显式标记，让 agent 知道上下文中存在缺口 |

---

## 总结

文档识别的四个方向覆盖了 forge-core 架构的三个不同层次：

- **方向一（跨进程协调）** 触及架构的最底层——运行时存储的并发假设。这是当前架构最迫切的基础设施缺口。
- **方向三（I/O 超时）** 触及编排器的关键路径——prompt 构建的可取消性。这是一个生产级可靠性缺口，在"NFS/FUSE 环境+24h 无人值守"下会从"间歇性问题"演化为"永久阻塞"。
- **方向二（自动压实）** 和 **方向四（Gate 名验证）** 更接近"产品契约一致性"缺口——代码已有能力但未接入自动化，或缺少验证导致静默失败。

五个方向的共同启发是：**ForgeOS 需要一个显式的运行时健康模型**——不仅要有静态的架构检查和编译时验证，还要有运行时的状态一致性自检、跨进程协调、可取消 I/O。这是从"静态治理系统"向"自治运行系统"演进的必经之路。
