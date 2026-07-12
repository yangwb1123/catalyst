好的，现在我已经全面了解了 ForgeOS 架构。以下是我的综合架构分析。

---

# 架构分析：ForgeOS 运维成熟度前沿

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的架构在 v2 中做出了几个正确的根本性决策：

**纯净的层级隔离。** `forge-core/` 严格遵循 `interfaces → application → domain` 方向依赖，13 个包之间无循环依赖。这体现在代码结构上：`internal/persist` 不知道 `internal/trace` 的存在，`internal/orchestrator` 将 IO 注入为回调（`OnIteration`、`OnPhase`），而非直接导入 checkpoint/trace 包。这保证了每个包的可测试性——`orchestrator` 的 20+ 测试文件覆盖了循环重启、loop-back、verdict gating、并行竞争，全部使用模拟回调而非真实文件系统。

**零外部依赖的运行时核心。** `go.mod` 无 `require` 是一个被低估的优势。在供应链攻击的时代，核心编排器不依赖任何第三方代码，意味着零漏洞公告带来的紧急发布、零许可证漂移风险、零依赖树退化。代价——手写 YAML 解析器、无 ORM、无 HTTP 框架——对于一个预计在未来几年内不会为其核心逻辑引入外部依赖的系统来说，是值得的。

**上下文传播作为第一个基础设施。** 系统从一开始就构建在 `context.Context` 传播之上。Engine → LoopEngine → RunFrom/RunParallel 的每个公共方法都将 `ctx` 作为第一个参数。这意味着跨切面关注点（取消、超时、追踪传播）可以在不改变函数签名的情况下添加到系统中。事实上，审计提出的 `--max-wall-clock` 方案正是利用了这一点——在编排器入口处包装 `context.WithTimeout`，不需要向 `Engine` 结构体添加任何字段。

**检查点/恢复作为持久化原语。** `internal/persist` 的原子写入契约（写入 tmp → fsync → rename）是正确的。它保证崩溃永远不会留下一个截断的检查点文件。`rotateRetain` 在现有策略上保留了最多 N 个历史条目，使趋势检测和版本回退成为可能。`PhaseIndex` 字段提供了恢复粒度，从整迭代粒度提升到单阶段粒度。

**并行执行 opt-in 门控。** 值得称赞的经过深思熟虑的保守设计。`RunParallel` 默认不激活，需要同时满足 `--parallel` 标志和 `depends_on` 声明。锁排序契约（`trace.mu → runBudget.mu → loopProbe.mu → ...`）明确记录在文件头，设定了并发安全的 bar。诚实边界（loop-back 禁用、阶段粒度检查点禁用）被明确承认，而非悄然隐藏。

### 1.2 架构局限性

**存储生命周期分散在三套不一致的策略中。** 这是最显著的架构缺陷。三个存储子系统各自实现了自己的生命周期策略：

| 子系统 | 策略 | 可配置性 | 轮转 |
|---|---|---|---|
| `internal/persist` | retain=N，历史轮转 | 函数参数（从调用方传入） | `rotateRetain` |
| `internal/trace` | 10MB 大小触发轮转，保留 1 个备份 | 硬编码 | `evolve.go:469-473` 的 `os.Rename` |
| `internal/memory` | `DefaultCompactThreshold=500` 条目 | `internal/memory` 中的包变量 | 原地压缩 |

三者之间没有统一的策略层。operator 无法从单一位置配置保留策略。更重要的是，**这些包中没有一个是可插拔的**——`persist.Save` 直接写入磁盘，`trace.Tracer` 包装一个 `io.Writer`，`memory.Load` 读取一个固定路径。如果将来需要 S3/GCS 后端，每个包都需要自己的适配器。

**遥测管道是仅写的。** `trace.Tracer` 产生完美的结构化 JSONL 事件流——13 种事件类型，包含 `duration_ms`、`cost_usd_micros`、`model` 归因、`seq` 排序。但这条管道只有生产者，没有消费者。跨 run 聚合、趋势分析、成本归因报告——这些完全不存在。`forge scorecard` 只读当前 run。`internal/doctor/anomaly.go` 在历史启用时执行趋势分析，但该能力从未接入任何命令。

**健康检查是静态的，而非持续性的。** `internal/doctor` 包产生一个点状快照——运行一次就退出。没有 `--watch` 模式，无基于 ticker 的轮询，无阈值驱动的警告。对于一个承诺 24 小时无人值守运行的系统来说，operator 必须手动记住定期运行 `forge doctor`。磁盘可用空间、增长率趋势、孤儿进程、时间轴异常——这些健康的经典维度完全未被触及。

**进程假设是单体的，无强制手段。** 架构假设单进程到仓库的映射，但无任何机制执行它。没有 PID 文件、文件锁、或通过 `flock` 的进程间协调。当两个 `forge` 进程意外并行运行时——CI 矩阵重叠、operator 在 `forge evolve` 运行时打开第二个 terminal——checkpoint 的原子重命名产生了静默数据丢失（后写入者覆盖前写入者），而 memory/trace 的 O_APPEND 产生了交错条目。

### 1.3 架构债务

**Python YAML shim 作为永久性临时方案。** `harness/yaml2json.py` 被标记为 "temporary scaffolding"，但依然存在。`forge-core/internal/yaml2json` 包（9 个文件）是一个完整的原生 Go YAML 解析器，是取代 shim 的正确路径。shim 创造了不必要的 Python 运行时依赖 (`python3 on PATH` 是 doctor 的一个检查项)。替代路径应在内部解析器达到功能完备时优先移除 shim。

**`anomaly.go` 存在但不使用。** `internal/doctor/anomaly.go` 中的 `DetectAnomalies` 在启用了 checkpoint 历史的趋势时执行趋势分析。但其唯一消费者——`forge status --history` 和 `forge doctor --anomaly`——在 retain=0 的历史时从不调用它（现已修正为 retain=5）。该代码已交付但未经过实战验证。

**锁排序契约未机械化。** `parallel.go` 中的 8 级锁排序契约是一个头部注释。没有任何构建时工具或运行时断言来执行它。对于并行模式采用 opt-in 且目前只有少量工作流声明 `depends_on` 来说，这是可以接受的——但随着并行执行成为默认选项，锁排序违规将成为一类仅在竞争条件中出现的故障（"Heisenbug"）。

**预算门控与架构不匹配。** 有四个成本护栏（depth、calls、USD、timeout），但缺少第五个（总 wall-clock）。更具体地说，这些护栏是作为独立的标志实现的，而非统一的策略子系统。`--run-budget-usd` 和 `--max-agent-calls` 作用于不同的层次，但 operator 无法表达 "any threshold reached = graceful stop" 。这需要在 `internal/orchestrator/budget.go` 中有一个统一的预算监控器，将成本、调用次数和时间整合到一个流中。

---

## 2. 扩展方向

以下五个方向反映了组织变革——对核心架构的改变，而非增量功能。每个方向都从现有能力出发，延伸到下一个关注点。

### 方向 A：统一的运行时存储层

**为什么需要：** 当前有三个存储子系统，各有一致的生命周期逻辑。operator 无法从单个位置配置保留策略。如果添加新文件（例如 `event.jsonl`），operator 必须编写第四种生命周期策略。统一层可以在保留策略之上启用存储后端抽象（本地 vs S3 vs GCS）。

**核心挑战：**

- **抽象的边界。** 统一存储不应变成 "god storage interface"。`persist` 需要原子写入 + 历史轮转。`trace` 需要追加 + 大小轮转。`memory` 需要追加 + 压缩。这些语义是不同的。统一接口过于宽泛则会丧失语义，过于狭窄则会迫使每个子系统进行不必要的抽象。
- **向后兼容。** `.forge/` 目录格式是运行时合约。现有安装有 `checkpoint.json`、`trace.jsonl`、`memory.jsonl`。统一层必须读取旧格式或通过迁移路径进行升级。

**预期的架构变更：**

提取一个 `internal/storage` 包，包含：

```
internal/storage/
  manager.go       # StorageManager: 统一生命周期持久化策略
  retention.go     # RetentionPolicy: 支持 size/N 保留 + TTL + 轮转
  backend.go       # Backend 接口: local/s3/gcs 适配器
  metrics.go       # size/growth/age 指标的钩子 (健康检查用)
```

每个现有子系统保持其当前的 API 契约。`persist.Save` 内部委托给 `storage.Manager.Save` 和可配置的保留策略。`trace.Tracer` 接受一个 `storage.RotatingWriter` 而非裸 `io.Writer`。`memory.Compact` 阈值可通过 `storage.RetentionPolicy.MemoryCompactThreshold` 配置。

**对现有系统的影响：**

- 低：现有 API 不变——`persist.Save(cpPath, cp, 5)` 依然有效，但保留策略默认从 `.agent/project.yml` 读取。
- `internal/trace` 和 `internal/memory` 需要重构以使用 `storage.RotatingWriter`。这是纯内部重构——公共接口（`trace.NewTracer(w io.Writer)`）保持不变。

### 方向 B：基于上下文的运行生命周期管理

**为什么需要：** 缺少的 wall-clock 超时是一个跨切面关注点。应该添加为上下文链中的一层，而非 `Engine` 结构体中的另一个字段。更广泛地说，`context.Context` 链应成为运行生命周期的承载者——不仅取消，还包括进度、预算状态、以及取消时的清理 —— 称为 "run session"。

**核心挑战：**

- **上下文链的组合。** `context.WithTimeout` 和 `context.WithCancel` 很容易组合，但 `context.WithValue` 是类型不安全的。一个携带预算状态和进度信息的运行上下文需要一个结构化的键，并在 goroutine 边界上明确传播。
- **优雅关闭。** 仅仅是上下文取消是不够的——当超时触发时，系统应该编写最终的检查点、记录事件、并以错误信息而非 SIGKILL 痕迹退出。这意味着取消必须在编排器层次被捕获，而非传播到操作系统。

**预期的架构变更：**

```
internal/orchestrator/
  runcontext.go    # RunContext: 包装 context.Context + 预算 + 超时 + 元数据
  lifecycle.go     # 运行生命周期钩子 (OnStart, OnTimeout, OnCancel, OnComplete)
```

`RunContext` 添加在 `cmd/forge/run.go` 的入口处，将所有现有的预算检查（`--run-budget-usd`、`--max-agent-calls`）整合到一个统一的 checkpoint 中。每个 agent phase 的 `select { case <-runCtx.Done() }` 检查成为单一的 guard 表达式，而非在 loop.go 和 parallel.go 中分散的 budget 检查。

**对现有系统的影响：**

- 极小：实质上是 `cmd/forge/run.go` 和 `cmd/forge/evolve.go` 中的包装函数。`Engine` 结构体不变。现有代码路径（无 `--max-wall-clock`）逐字节不变。
- 应当这样做的好处：消除 `internal/orchestrator/loop.go` 和 `internal/orchestrator/parallel.go` 之间 budget 检查的重复。

### 方向 C：进程组与资源隔离框架

**为什么需要：** 当今 ForgeOS 假设单进程到仓库的范式。两个独立进程撞车的后果是静默数据损坏。这是分布式系统中的一个经典问题——共享状态的并发访问需要一个分布式锁。但该方向不仅仅是关于锁：它是关于将 `.forge/` 目录视为由单个 "run session" 租用的共享资源。

**核心挑战：**

- **层级锁 (L1/L2/L3)。** 审计正确地将锁分层：
  - L1：对 `.forge/run.lock` 的 `flock`，发现锁时直接退出。
  - L2：`forge run --force` 覆盖旧锁（记录覆盖操作到 trace）。
  - L3：孤儿锁检测（PID 存活？超过 24h 的不存活 PID 被清理）。
  但 L2 引入了竞争窗口——两个 `--force` 进程可能都读取相同的锁，都看到"自己的"覆盖。需要一个原子 CAS 操作或一个持有锁的租约周期。
- **跨会话隔离。** 锁本身对于并发写入是不够的，因为 trace 和 memory 以 O_APPEND 方式写入。即使加了锁，来自不同进程的交错条目也不会按照正确的顺序排列。锁必须与运行会话 ID 配对，写入时带有会话标记，使得读取者可以在读取时过滤掉不相关的条目。

**预期的架构变更：**

```
internal/lock/
  filelock.go      # L1: .forge/run.lock 上的 flock
  session.go       # 会话 ID 生成 + 追踪
  force.go         # L2: --force 覆盖（原子 CAS）
  orphan.go        # L3: 孤儿清理 (forge doctor)

internal/persist/session.go  # 运行时会话标识的标记
```

每个 trace/memory/checkpoint 条目携带一个 `session_id` 字段。`forge doctor` 默认只读取与当前运行相关的条目（最新会话或指定会话）。`forge doctor --all-sessions` 读取所有条目（代价是读取时的即时合并）。

**对现有系统的影响：**

- 中等：所有写入路径都需要一个 `session_id` 参数。这是向持久层添加一个字段，该字段也向下兼容（`session_id==""` 表示 "legacy session"，行为不变）。
- 乐观并发控制（读取时过滤）而非悲观方案（启动时锁定所有文件）意味着可以在不丢失数据的情况下添加锁——对于已经存在 `.forge/` 目录但无锁的部署来说，这是一个重要的属性。

### 方向 D：持续健康监测子系统

**为什么需要：** `forge doctor` 是一个点状快照。对于一个承诺 24 小时无人值守运行的系统来说，健康必须持续监测，而非按需检查。审计的 `--watch` 模式建议是正确的，但更深入：该系统需要一个健康状态机，跟踪随时间推移的转换（PASS → WARN → FAIL），并产生 operator 或上层编配层可以消耗的事件。

**核心挑战：**

- **阈值与灵敏度。** 什么是健康？800MB 的可用空间对于一个长期运行的 evolve 来说是 WARN 还是 FAIL？50% 的周增长率值得告警吗？阈值必须可配置且可分环境调整。过于敏感的默认值会生成告警疲劳——operator 忽略 `forge doctor`。过于宽松的默认值让损坏在静默中蔓延。
- **时间序列状态。** "健康"不是瞬时的。上周二的 30 秒超时可能没问题，但如果它每天都发生，就暗示了退化。健康子系统需要跟踪指标随时间的变化——不仅仅是状态，而是状态的导数。

**预期的架构变更：**

```
internal/doctor/
  watcher.go       # --watch 模式: 时间间隔 + 健康状态机
  threshold.go     # 可配置阈值 + 敏感度级别
  timeline.go      # 时间轴异常检测（时间戳范围不一致）
  metrics.go       # 指标收集: 文件大小、增长率、disk 使用

internal/doctor/check/
  disk.go          # statfs 检查 + 阈值
  orphan.go        # 孤立进程扫描
  permission.go    # 文件权限检查
  timeline.go      # 最后修改时间不一致检测
```

`forge doctor --watch 60` 以 JSON 形式输出每个间隔的健康状态。每个检查的类型也返回时间序列数据（过去 N 个检查的值），使 operator 工具可以绘制"可用空间 → 3 小时前 2GB，现在 800MB"的图表。健康状态机跟踪转换——一个 FAIL 不会重置为 PASS，直到操作员确认或问题自动解决。

**对现有系统的影响：**

- 低：新代码，无重构。`internal/doctor/doctor.go` 中的现有 `Run()` 函数成为 `Watch()` 帧内的一个间隔检查。新检查项（disk、orphan、timeline）是新功能，而非变更。

### 方向 E：遥测查询引擎

**为什么需要：** `trace.jsonl`、`checkpoint.json.N`、`scorecards.json` 和 `memory.jsonl` 包含丰富的结构化数据，但无人查询。operator 无法回答"上个月哪次运行的 cost 最高？"或"reviewer 的延迟趋势如何？"这类问题。这不是关于构建 Grafana——这是在已有数据和 CLI 之间缺失的查询层。低成本、无外部依赖、纯 CLI。

**核心挑战：**

- **非规范化与查询模型。** trace.jsonl 是平面的、逐行的事件。checkpoint.json.N 是序列化的快照。scorecards.json 是聚合的百分位数。没有一个包罗万象的数据库——每个文件都有自己的模式。查询引擎必须理解每种文件类型的语义，在一个过程中合并数据集（例如，将 checkpoint 时间戳与 trace 事件关联）。
- **日期范围与窗口。** 跨 N 天的查询需要按时间戳过滤行。`trace.jsonl` 没有固定的行序（它按序列号排序，不是时间戳）。按时间查询需要对文件进行完整扫描（除非构建了索引）。对于典型的 `.forge/` 大小（<100MB），完整扫描是可以接受的——但如果目录增长到 GB 级，就会失败。

**预期的架构变更：**

```
internal/report/
  query.go          # 查询执行: 过滤 + 聚合 + 分组
  trace_query.go    # trace.jsonl 查询（按 kind/name/status/date 过滤）
  checkpoint_query.go # checkpoint 历史查询（roadmap_completion 趋势）
  scorecard_query.go  # scorecard 查询（跨 run 聚合）
  memory_query.go     # memory 查询（决策/差距分类，随时间的增长）

cmd/forge/
  report.go         # forge report [--since 7d] [--metric cost|latency|quality]
  trend.go          # forge trend --metric cost --period 30d
```

`forge report --since 7d` 扫描 trace.jsonl（通过会话 ID 限制范围），提取所有事件，并产生：
- 总运行次数、成功/失败/取消统计
- 按阶段总 agent 耗时和成本
- 按agent 的平均延迟和 p95 延迟
- memory 增长率（`memory.jsonl` 中的条目/天）

`forge trend --metric cost --period 30d` 从多个检查点和 trace 文件读取，提取指定指标的周/日均值序列。

**对现有系统的影响：**

- 中等：纯新增代码，无重构。数据文件只读。主要风险是查询性能——一个没有索引的 100MB `trace.jsonl`，在 `--since 90d` 查询下扫描，可能会很慢。通过在 `trace.Emit` 中添加一个可选的辅助索引来解决，该索引是轻量级的（运行级别的时间范围映射），而非侵入性的。

---

## 3. 接口设计建议

### 3.1 存储后端的接口原则

`internal/storage` 包应最小化暴露。存储后端的接口应该是一个文件系统抽象，而非一个数据库抽象：

```go
// 最小化——只有持久化需要的操作。
type Backend interface {
    // WriteSync atomically writes data to path with fsync.
    WriteSync(path string, data []byte) error
    // ReadFile reads a complete file.
    ReadFile(path string) ([]byte, error)
    // Rename atomically renames oldpath to newpath within the backend.
    Rename(oldpath, newpath string) error
    // Stat returns file info.
    Stat(path string) (FileInfo, error)
    // Glob returns matching paths.
    Glob(pattern string) ([]string, error)
    // Remove deletes a file.
    Remove(path string) error
}
```

为什么这样最小化？`persist`、`trace`、`memory` 各自的 I/O 模式都不同，但都基于这三个原语（write+fsync、read、rename、stat、glob）。一个丰富的接口（带有 `Stream`、`Search`、`Query` 等）将超出具体存储实现（local vs S3 vs GCS）的能力，迫使各实现抛出 `UnsupportedOperationException` 风格的错误。

### 3.2 生命周期管理接口

保留策略应该是数据，而非代码。operator 可配置的策略在 `.agent/project.yml` 中：

```yaml
storage:
  retention:
    checkpoint: 10          # keep 10 rotated versions
    trace_rotate_mb: 50     # rotate trace files at 50 MB
    trace_keep: 5           # keep 5 rotated trace files
    memory_compact_threshold: 1000  # compact at 1000 entries
    auto_cleanup_days: 90   # delete data older than 90 days
```

在 Go 中，这映射为一个 `RetentionPolicy` 结构体，使用了包级别的默认值（当配置缺失时），因此零配置的行为与今天相同：

```go
type RetentionPolicy struct {
    CheckpointRetain        int   // default: 5 (current behavior)
    TraceRotateMB           int64 // default: 10 (current behavior)
    TraceKeep               int   // default: 1 (current behavior)
    MemoryCompactThreshold  int   // default: 500 (current behavior)
    AutoCleanupDays         int   // default: 0 (disabled — no auto-cleanup)
}
```

### 3.3 健康检查接口

每个健康检查应该被提取为一个 `Checker` 接口：

```go
type Checker interface {
    Name() string
    Check(ctx context.Context) (CheckResult, error)
}

type CheckResult struct {
    Status  CheckStatus // PASS | WARN | FAIL
    Message string
    Detail  map[string]any // 结构化数据供 JSON 输出使用
}
```

为什么这是一个接口而不是枚举？因为检查可以横向扩展——磁盘检查、孤儿进程检查、时间轴检查、内存压缩健康检查——且每项检查都可以独立测试。`internal/doctor/watcher.go` 中的 `Watch()` 函数持有 `[]Checker` 并以固定的间隔运行它们。新的检查项通过实现该接口来添加，而不是通过修改 `doctor.go` 中的一个大函数。

### 3.4 查询引擎的接口

查询应该通过过滤器和聚合器来构建，而非 SQL 字符串：

```go
// 过滤器的构建器模式
query := report.NewQuery(traceFile).
    Since(7 * 24 * time.Hour).
    Kind("agent").
    GroupBy("model").
    Aggregate("cost", report.Sum)

results, err := query.Run(ctx)
```

为什么不是 SQL？避免外部依赖。trace.jsonl 不是数据库，它是一系列 JSON 行。SQL 引擎需要对文件进行 ETL，这会产生外部依赖，并且对于 CLI 工具的每个运行来说过于重量级。一个 Go 原生的过滤器+聚合器管道，在文件上使用 streaming reader——内存效率高且零依赖。

---

## 4. 技术选型

### 4.1 保持：纯 Go 标准库用于核心运行时的决策

`forge-core` 的零外部依赖政策是架构正确的决定，不应妥协。理由：

- **供应链攻击面。** 核心编排器如果有一个受损的间接依赖（如 2024 年的 `xz utils`），将完全颠覆 ForgeOS 的安全模型——一个将代码直接写入仓库的工具，其依赖链暴露无遗。零依赖意味着零供应链攻击面。
- **构建和部署。** 一个单一的静态 Go 二进制文件，`go install` 即可部署。没有 `npm install`、`pip install`、`go mod tidy`。这是一个显著的运维优势。
- **约束作为收益。** 没有 ORM 意味着持久化代码保持明确且可审计。没有 HTTP 框架意味着 HTTP 服务器的延迟是可控的（当未来添加时）。对于系统来说，这是正确的约束集合。

**要抵抗的诱惑：** 引入 `go-yaml` 以避免手写 YAML 解析器，引入 `grpc-go` 以便利控制面通信。这些都是 v3 引入的合理需求。在 v2 中，标准库目前已经足够——`encoding/json`、`net/http`、`os`、`context`。

### 4.2 推迟：外部存储和可观测性基础设施

北极星架构（`north-star.md`）呼唤 Temporal、Postgres、Qdrant、NATS、OPA、Firecracker、OTel 和 Grafana。这些是 v3 的决策，不应在 v2 中引入。理由：

- **安装复杂度。** 这些基础设施组件中的每一个都增加了 operator 的部署复杂度。ForgeOS 的 v2 卖点是 "go install forge"，而非 "设置一个 Temporal 集群"。
- **数据量。** 当前的 `.forge/` 输出（单个运行数十 KB，一年数百 MB）不需要数据库。当项目在单个仓库中达到数百 GB 的 trace 数据时，就证明了 Temporal/Postgres 的合理性——这是一个雄心勃勃但非当前的状态。
- **架构迁移。** 从平面文件到 Temporal + Postgres 将是一次血统式的架构变革。这种迁移应该发生在 ForgeOS 到达生产阶段（v3）时，当流量增长迫使它发生时。预先构建抽象（`Backend` 接口、保留策略）降低了迁移成本，而不强制一个特定的时间表。

### 4.3 引入：用于 YAML 的可选 `gopkg.in/yaml.v3`

此处有一个特殊情况。`internal/yaml2json` 是一个 9 文件手写的 YAML 解析器，仅用于将 `.agent/workflows/*.yml` 转换为 JSON。YAML 是复杂的，手写解析器存在方言不一致的风险。

**选项 A：维持现状。** 手写解析器正确解析了当前的工作流 YAML。如果 YAML 需求保持稳定，它不需要外部依赖。但这在 operator 向 YAML 扩展添加边缘特性时存在脆弱性风险。

**选项 B：引入 `gopkg.in/yaml.v3` 作为核心依赖。** 这是一个经过验证、稳定、广泛使用的库。但如果引入核心依赖，就会打破 "零依赖核心" 的政策。

**建议：** 将 `internal/yaml2json/yaml2json.go` 重构为以下两者之一：a) 一个使用 `--stdin` 模式的独立的 `forge yaml` 子命令，作为 `harness/yaml2json.py` 的替代（零依赖，重用现有代码），或 b) 引入 `gopkg.in/yaml.v3` 作为构建依赖，但将其隔离在 `internal/yaml2json` 中——而且仅此一处。选项 b) 打破零依赖政策，但对于 YAML（一个行业标准、版本稳定、作为依赖无处不在）来说是一个合理的例外。

### 4.4 延迟：用于 CLI 查询的嵌入式 SQLite

如果 `forge report --since 365d` 需要在数 GB 的 trace.jsonl 上提供亚秒级的查询，嵌入式 SQLite 将是一个考虑方向。**但目前推迟。** 理由：

- SQLite 是一个 C 库。将其嵌入 Go 需要 `mattn/go-sqlite3` 或 `modernc.org/sqlite`（一个是 CGO，一个是纯 Go）。两者都会增加构建时间、二进制体积和维护负担。
- 在数据量达到数十万行之前，基于 streaming reader 的 Go 原生查询（`bufio.Scanner` + streaming JSON 解码）是足够的。在 v2 中不太可能达到这个量级。
- 如果确实达到，应先考虑轻量级替代方案（MMAP、每运行二级索引），然后再转向 SQLite。

**决策标准：** 当 `forge report --since 30d` 的 wall-clock 时间超过 5 秒，或者 trace.jsonl 超过 500MB 时，评估嵌入式查询引擎。在此之前，标准库 streaming reader 是最简单的正确的选择。

---

## 5. 实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 工作项 | 预估 | 评级原因 |
|---|---|---|---|---|
| **P0** | B | `--max-wall-clock` 标志 | 1-2 天 | 最低风险，最高财务收益。防止无限账单。零架构影响。 |
| **P0** | D | `forge doctor --watch` 模式 | 1-2 天 | 使其他所有运维流程成为可能。极低风险。 |
| **P1** | A | 统一保留策略 + `.agent/project.yml` 配置 | 5-7 天 | 修复分散的策略。启用 operator 配置。低风险。 |
| **P1** | C | L1 文件锁（`flock`） + L3 孤儿检测 | 3-5 天 | 防止静默数据损坏。中风险（需要解锁互斥 + 回退）。 |
| **P2** | E | `forge report --since 7d` | 5-7 天 | 高价值但非关键。不会让工厂停运。中等风险（查询性能首次暴露）。 |
| **P2** | E | `forge trend` + 成本归因 | 5-7 天 | 依靠 `forge report`。 |
| **P2** | A | 存储后端接口 + S3/GCS 适配器 | 7-10 天 | 当需求出现时（多仓库 / 异地部署）才有价值。 |
| **P2** | C | L2 `--force` 锁覆盖 + 会话隔离 | 3-5 天 | 依靠 L1。测试边界条件（竞争）。 |

### 5.2 阶段划分

**阶段 1——根基（7 天）：**

```
W1: --max-wall-clock (P0) + doctor --watch (P0)
```

这两个工作项为其他一切创造了环境。Wall-clock timeout 是成本控制的故事。`doctor --watch` 是可观测性的故事。两者都是独立完备的——每个都可以独立交付和验证。

*里程碑：* ForgeOS 可以保证在 8 小时后优雅停止（而非被 OOM 杀死），并且 operator 可以通过 `forge doctor --watch 60` 持续监测健康。这是一个承诺 "无人值守" 的系统的不同之处。

**阶段 2——数据完整性（10 天）：**

```
W2-W3: 统一保留策略 (P1) + L1 文件锁 (P1)
```

保留策略合并了三个分散的策略，使 operator 可以从一个位置配置 retention。文件锁防止了在并发进程场景下的静默数据损坏。

*里程碑：* `.forge/` 不再无限增长。operator 可以设置 "保留 90 天的数据" 并忘记它。两个 `forge run` 进程不能静默破坏对方的运行状态。

**阶段 3——可观测性（7 天）：**

```
W4: forge report --since 7d (P2)
```

这是来自已有数据的第一个消费端命令。利用阶段 1 中的 `--watch` 健康框架和阶段 2 中的保留策略。

*里程碑：* Operator 可以回答 "过去一周运行的健康吗？"、"哪个 agent 最贵？"、"memory 增长趋势如何？" ，都不需要 grep。

**阶段 4——弹性（7 天）：**

```
W5: L2 --force 锁 + 会话隔离
```

Layer 2 为进程锁增加了 `--force` 覆盖和会话隔离，使得并发写入是可以合并的，而不仅仅是阻塞。

*里程碑：* 两个 `forge for run` 命令可以安全地共存（trace/memory 条目按运行正确分组），operator 可以通过 `--force` 覆盖陈旧的锁。

### 5.3 风险与缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|---|---|---|---|
| `--max-wall-clock` 与 `--timeout` 交互错误（共同竞争 ctx） | 低 | 中 | 纯在 orchestor 之外包装上下文。在所有测试中同时设置两个标志进行测试。 |
| L1 文件锁在有遗留 `.forge/` 目录的现有安装上中断 | 中 | 高 | 实现优雅回退——锁文件缺失 = 警告而非失败。`forge doctor` 在下次运行时自动创建锁文件。 |
| `forge report` 扫描大型 trace.jsonl 时性能缓慢 | 低 | 中 | 添加运行级别的时间范围辅助索引（在 `trace.Emit` 中写入，trace.jsonl 旁边的单独文件）。在数据量大时回退到 O(N) 扫描。 |
| 统一保留策略改变了默认保留行为 | 高 | 中 | 零变更默认值：保留策略默认值=当前行为（checkpoint: retain=5, trace: 10MB rotate, memory: 500 compact）。现有安装逐字节不变。 |
| 会话 ID 向 trace/memory 添加字段破坏了现有下游工具 | 中 | 中 | 向后兼容编码：`session_id==""` 在读取时意味着 "legacy"。现有工具在大部分场景下无需额外处理即可工作。 |

### 5.4 关键架构决策记录

决定：

1. **不要创建统一的 "存储引擎"。** `persist`、`trace` 和 `memory` 各自的 I/O 模式是不同的（原子重命名 vs 流式追加 vs 压缩）。强行统一到一个通用的 "存储引擎" 中会在正确性和可理解性方面得不偿失。改为统一 *生命周期策略管理*（保留配置），而非 *I/O 执行*。

2. **在 orchestor 之外，而非其中，添加 Wall-clock timeout。** 审计给出的建议（在 `cmd/forge/run.go` 包装 `context.WithTimeout`）是正确的。原因：timeout 是一个每个运行属性，不是一个每个引擎属性。如果将 timeout 放入 `Engine` 结构体，它还需要向循环添加传播。在 orchestor 之外包装上下文可以保持传播沿着已经存在的 `ctx` 链路进行。

3. **文件锁使用乐观并发控制（读取时过滤），而非悲观的（启动时加锁）。** 悲观方案（在 `forge run` 启动时加锁所有文件）更简单，但在遗留安装上的启动时间会中断，并阻止 `forge status`（只读）在 `forge evolve` 运行时工作。乐观方案（以 `session_id` 标记写入，在读取时过滤）允许并发读取器，且对遗留数据向后兼容。代价是读取时的过滤 CPU 成本——对于典型的 `.forge/` 大小来说可以忽略。

4. **`forge report` 使用流式 JSON 读取器 + 内存聚合，而非嵌入式数据库。** 目前时间范围内。外部数据库引入了安装和打包的复杂性。流式读取器（`bufio.Scanner` + streaming `json.Decoder`）零外部依赖，且对于最多数百万行的数据量足够快。重新评估的时间点：当 `forge report --since 30d` 的 wall-clock 时间超过 5 秒时。

5. **健康严重性是分层的，而非二元的。** `forge doctor` 检查项返回 `PASS` | `WARN` | `FAIL`，而非二进制。WARN 不会触发预先退出——健康状态机跟踪随时间推移的转换（`PASS → WARN` 是信息性的，`WARN → FAIL` 是关键的）。这防止了"狼来了"的告警疲劳，同时放大了真正的失败信号。

---

## 总结

ForgeOS 的架构在 v2 中正确地将功能完备性置于运维成熟度之上。对于在一个空间里运行的平台来说，这是正确的优先次序——首先确保它能工作，然后确保它能工作 *很长时间*。

这五个运维成熟度前沿标记了从 "它能工作" 到 "它能自主运行 365 天" 的过渡。它们共同的架构模式值得注意：每一个前沿都利用已经存在的 *现有能力*（`context.Context` 链路、结构化 trace 事件、原子检查点、可扩展的 doctor 框架、保留策略框架），并将它们连接成面向运维者的工具。

最需要立即关注的方面是：

1. **wall-clock 超时（2 天，零风险）。** 架构上最清晰的工作项——作为一个正确定价的正确的跨切面关注点，利用现有的上下文传播链路。没有架构风险，只有财务收益。

2. **`forge doctor --watch`（1 天）。** 没有持续健康检查，"无人值守 24/7" 的承诺就不成立。`--watch` 模式通过将 `internal/doctor` 从静态检查器转换为持续健康状态机，完成从 "可检查" 到 "可观测" 的过渡。

这是一条将工程组织（确保它能工作）与运维成熟度（确保它能长时间工作）连接起来的桥梁，这是 ForgeOS 从个人工具走向平台的关键过渡。
