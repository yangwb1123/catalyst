现在我已对整个系统有了全面理解。让我提供架构分析。

---

# ForgeOS 架构分析：从深度到广度的跃迁

## 1. 架构评估

### 1.1 当前架构的核心优势

ForgeOS 的架构是一个罕见的组合：它有**成熟的北极星愿景**（north-star.md），同时以罕见的纪律进行了**增量交付**。31 个 sprint 中没有一个 sprint 为了性能而镀金。值得保留的关键架构优势：

| 属性 | 为什么它很强大 |
|---|---|
| **旋钮与拨盘分离**：`mode×lifecycle` 是一个单一的横切旋钮，同时控制 Router、Harness、Workflow 深度 | 避免正交轴爆炸。一个输入 → 三个输出，如同硬件拨盘 |
| **可证伪的收敛**：`converge.MET` = 带外 gate 布尔合取 + ROADMAP 完成度，非 LLM 自报 | 这是唯一可辩护的结构性 delta，区别于 ReAct/Reflexion/Devin/OpenHands。它不是一个更好听的终止条件——它是一个*不同类型*的终止条件。 |
| **诚实代数**：N/A ≠ PASS；vacuous-green guard；零-phase workflow 不报收敛；`provenCount==0 ⇒ false` | 这是维特根斯坦式的：「缺乏证明绝不等于满足」。对于 LLM 产出的系统，这是信任基础。 |
| **零外部依赖（Go 运行时）**：18 个包，空 go.mod，纯标准库 | 零供应链风险，零版本冲突，< 5 秒构建。对于宣称是「OS」的系统，这是基准线 |
| **上下文独立审查者**：一篇 CLAUDE.md 红线即为「审查者必须是 fresh-context 独立 Agent」 | 在 LLM 产出的系统中，这是*最重要的工程纪律*——且是最容易被违反的 |

### 1.2 架构债务与局限性

**债务一：Python YAML shim —— 仅存的强制性外部依赖**

forge-core 的 go.mod 为空，但 `forge run/evolve/gate` 若无 `python3` + `PyYAML` 在 PATH 中就无法运行。这不是一个可选的降级项——已有两条路径（Go 原生解析器遇到退化情况，Python shim —— Sprint 27 证明了 `consumeBlockScalar` 损坏工作流文件的模式），但 Go 解析器尚未通过 `diff-test` 发现的所有真实边缘情况。yaml2json 桥梁存在于一个无人认领的间隙中：它既不是被接受的永久依赖（未被打包），也不是被替换的过渡依赖（尚无可靠替换方案）。

**债务评估**：高紧迫性。每次 `forge run` 都依赖 Python + PyYAML。本仓库 7 个真实 YAML 文件中已有 6 个曾被损坏，且「差异测试」验证并未拦截——它仅有 `t.Logf` 无 `t.Errorf`。

---

**债务二：CLI-only 执行模型 vs 早已声明的事件驱动**

`evolve.yml` 声明了 `stop_condition.type: external` 和 `triggers: [human_pause, budget_exhausted, no_gaps_found]`。三个 trigger 中没有一个被实现为外部事件源——它们都是 LoopEngine 的内部终止条件。`gateway` 包不存在，`daemon` 模式不存在，没有 webhook 端点，没有 HTTP 健康检查。该系统只有一个激活模式：终端中的 CLI 命令。

这不仅是功能缺口——它是**收益递减曲线**。ForgeOS 对编排与治理的投资已足够深入，以至于 CLI-only 模型正在限制其效用。每增加一个 agent 卡、gate 或信号，边际收益增量减少，因为激活方式（人工 → 终端 → `forge run`）仍然是瓶颈。

**债务评估**：达到临界点的结构性债务。系统已准备好成为 daemon/平台，但构架依然停留在 CLI 工具。

---

**债务三：存储生命周期不对称 —— memory 可压缩，trace 不可压缩**

| 制品 | 增长 | 压缩 | 上限 | TTL |
|---|---|---|---|---|
| `trace.jsonl` | 线性增长，每次迭代 ~每 phase 一条 | 无 | 无 | 无 |
| `memory.jsonl` | 线性增长，每次迭代在 `recordMemory` 时追加 | `Compact` 每 10 轮调用 | `keepPerKind` 上限 | 无 |
| `checkpoint.json` | 每次迭代一次重写 | `retain` 存在但设为 0 | 1 活动 + N rotate | 无 |

在 24 小时 evolve 运行中（Sprint 25–26 经验证），trace 构成最大的存储足迹且完全不受治理。`trace.Emit` 从不返回错误——若写入失败（磁盘空间满），系统继续运行但审计消失。这对声称支持「不可篡改审计日志」的系统而言是不可接受的。

---

**债务四：无进程级文件锁**

零保护。无 PID 文件，无 `flock`，无 `O_EXCL`，无锁文件。两个 `forge evolve` 实例操作同一 `.forge/` 目录会损坏 trace（O_APPEND 行交错）、memory（同类原因）和 checkpoint（`.tmp` + `rename` 的 TOCTOU 竞争）。这不是理论问题——对于 24 小时无人值守的系统，`forge evolve` 在一个 CI runner 上运行，同时开发者本地又跑一个 `forge run`，这正是*设计上的预期用例*。

---

**债务五：`cmd/forge` 的反复边界问题**

Sprint 27 将文件数从 15+ 降至 14，Sprint 29 以允许 18 文件作为捷径，后被纠正至 16，Sprint 30 达到 16 文件零余量并更新至 17。每次突破被解释为孤立事件，但模式很明显：`cmd/forge` 吸收了超过 CLI 胶水职责的逻辑。`validate_agents.go` 本应进入 `internal/doctor`，`scorecard_rebuild.go` 本应进入 `internal/attribution`，`gate_resolve.go` 本应进入 `internal/gate`——所有这些都是在突破后*才*被纠正的。模式：「快，先写进 cmd/forge，以后再说。」

---

### 1.3 关键设计决策评估

**✅「一个旋钮控制全部」** —— 正确。这是系统的最高杠杆设计决策。与 Kubernetes 的单一 `spec.replicas` 控制 deployment、HPA、PDB 的方式异曲同工。

**✅「收敛判据 = 带外布尔合取」** —— 正确。这是唯一可辩护的 delta，针对 ReAct/Reflexion/AutoGPT/Devin/OpenHands。值得围绕它建立一门学科。

**✅「零外部 Go 依赖」** —— 正确。这强制进行干净的抽象边界。Python shim 令人遗憾但已正确标记为临时措施。

**⚠️「单二进制 monolith」** —— 尚可。对于 v0–v2 正确且必要，但架构的北极星（10+ 服务）与当前实现（1 个二进制）之间的鸿沟意味着服务边界从未被强制执行。`internal/orchestrator` 直接调用 `internal/memory`，而非通过接口。包级解耦已足够，但进程级解耦完全不存在。

**⚠️「JSONL 追加日志作为持久化格式」** —— 对 v1 尚可，但需警惕。JSONL 简单、可追加、可 grep，但无压缩、无索引、无 schema 演进。`persist` 包中的 `encoding/gob` 增加了跨 Go 版本的兼容性问题。

**❌「trace 无上限」** —— 欠考虑。对称性被破坏——memory 有 `Compact`，trace 无任何机制。对于声称支持「不可篡改审计日志」的系统而言，这是生产就绪度的缺口。

---

## 2. 扩展方向

从架构视角（而非产品视角），以下是对系统长期健康最重要的扩展方向。这些方向按**对系统长期健康的影响**排序，而非商业需求。

---

### 方向 A：事件网关 + Daemon 化（P0）

**为什么需要**：
ForgeOS 的北极星将自己定位为「自治软件工程控制平面」——但当前架构只有一个输入路径（CLI 命令），没有输出路径（无 webhook、无 daemon、无健康端点）。Gateway 引擎是北极星架构建模的 10 个引擎之一；其余 9 个引擎已部分集成到一个二进制文件中，但 gateway 连起步都没有。

矛盾很明显：系统的设计是让 AI 自治地 24 小时运行，但系统本身若不使用 tmux/nohup 包装就无法存活 24 小时。编排者本身不可编排。

**核心挑战**：
1. **激活模型假设的转变**：当前代码中的每一处推理都假设「用户通过 CLI 启动，等待结果，查看输出」。Daemon 模型意味着：无用户、无终端，只有进程 + 事件 + 文件系统。`Engine.Log`、`LoopEngine.Log`、`Logf` 需要结构化日志后端（非标准输出）。
2. **事件到 workflow 的映射**：GitHub webhook → 哪个 workflow？带什么参数？当前的设计不对事件分类，也无 `triggers` 实现。
3. **优雅关闭与恢复**：长时间运行的 daemon 需要 SIGHUP 处理、workflow 持久化（Temporal-ish checkpoint）、资源耗尽预防。

**预期架构变更**：
```
forge-core/internal/gateway/         # 新包，零外部依赖
  webhook.go        # HTTP handler ← 外部事件
  poll.go           # 基于定时器的轮询（GitHub API）
  dispatch.go       # 事件 → workflow 映射
  daemon.go         # signal 处理、优雅关闭
```

对 `Engine` 的变更：添加 `Serve()` 长期运行模式，替代当前的 `Run(ctx) error`。`LoopEngine` 获取 `OnEvent(event) bool` 方法，可被外部事件触发唤醒。

**影响**：现有 CLI 路径保持不变。`forge run --daemon` 是附加模式。所有现有行为向后兼容。

---

### 方向 B：跨进程并发模型 + 存储契约（P0）

**为什么需要**：
当前 `internal/persist` 实现假设单进程访问 `.forge/` 命名空间。多个进程的操作（两个 `forge evolve` 并行，或者 CI + 开发者并行）会损坏 trace、memory、checkpoint。

根本问题不是缺少 `flock` 调用——而是**存储契约未定义**。`.forge/` 的契约是什么？它是否支持两个同时读取者？一个写入者 + 多个读取者？互斥写入者？这些未被记录，因此每个写入者假设自己是唯一的写入者。

**核心挑战**：
1. **`trace.go` 的 `O_APPEND` + `sync.Mutex`**：Mutex 是进程内保护，`O_APPEND` 的内核保证（`PIPE_BUF` 写 > 4096 字节可能交错）仅在同一 kernel 线程串行化 I/O 时可靠。两个独立进程的写操作没有*内核保证*关于行原子性。
2. **`checkpoint.go` 的 `.tmp` + `rename` 竞争**：即使 `rename(2)` 是原子的，围绕它的逻辑（写入 tmp，fsync，rename）在存在两个写入者时存在 TOCTOU 条件。
3. **升级路径**：添加锁后，旧版本 forge（无锁感知）可能写入被锁持有者视为已锁的文件。

**预期架构变更**：
```
internal/persist/lock.go              # flock/LOCK_EX 抽象
internal/persist/lock_unix.go         # syscall.Flock 实现
internal/persist/stale.go             # 存活检测 + 超时

internal/persist/registry.go          # 集中式文件打开/锁定管理器
# 所有 OpenFile / Create 调用都走 registry 而非直接 os 调用
```

**关键设计决策**：锁是**强制性**还是**建议性**？建议性（flock(2) 默认）好——它不会破坏不理解锁的旧二进制文件，但会阻止理解锁的新版本相互冲突。

**对现有系统的影响**：
- `trace.Open`：获取共享（读取）或排他（写入）锁
- `memory.Append`：共享锁（写入者协调写入）
- `checkpoint.Save`：排他锁（互斥写入者）
- `forge status`：添加锁定状态显示

**向后兼容性**：新 forge 可读取旧 `.forge/` 目录（锁是可选的）；旧 forge 读取新 `.forge/` 目录时不感知锁，行为不变。

---

### 方向 C：结构化可观测性框架（P1）

**为什么需要**：
当前 observability 模型完全是文件形式的：自由文本日志到 `Engine.Log`，扁平的 JSONL trace 到 `trace.jsonl`。两种格式都无法被标准基础设施消费（Prometheus、OTel、Datadog、Grafana）。在 24 小时无人值守模式下，操作者无法 SSH 进入终端——他们需要端点。

此外，当前 trace 数据模型缺少 spans 层级。在典型的 evolve 迭代中，结构如下：

```
Iteration 3
  ├─ scan phase (agent)      — 1 trace event
  ├─ planner phase (agent)   — 1 trace event
  ├─ implementer (agent)     — 1 trace event
  ├─ harness gates (gate)    — 4+ sub-operations (gate/arch-check/secret-scan/NA)
  ├─ reviewer (agent)        — 1 trace event
  └─ converge check (engine) — 1 trace event
```

当前模型将这展开为 5–10 个扁平的 trace 事件，无父子关系。要将成本归因于「gate 失败 → loop back → 重试」链条，需要手动 grep。

**核心挑战**：
1. **跨度生命周期**：Go 程序不提供纯正 span 上下文传播的内置机制（如 OpenTelemetry 的 `context.Context`）。添加 spans 需要为每个 `Run`/`Emit` 调用添加 `context.Context` 参数——会影响整个代码库的 API。
2. **指标不是现有基准的附加项**：Prometheus 直方图需要类型、标签、桶。与当前无类型的 `Engine.Log` 集成不是透明的。
3. **与 `forge accept` 的集成**：闸门应消耗指标以检测退化（如「p95 延迟相较上次迭代翻倍」）。

**预期的架构变更**：
```
internal/telemetry/                 # 新包，可选依赖
  span.go           # Span 类型（ID, ParentID, Kind, Start, Duration）
  tracer.go         # Tracer 构建 spans，可选的 OTel 兼容
  export.go         # 写入 JSONL trace（当前格式）或 Prometheus 文本格式
  health.go         # /healthz + /readyz HTTP handler
```

对 `trace.Event` 的变更：添加 `SpanID`、`ParentSpanID`、`Kind`（agent/gate/loop/skip）字段。`trace.Tracer` 获得 `StartSpan`/`EndSpan` 方法。现有 `Emit` 签名保持不变，以可选 span 字段为后缀。

**影响**：JSONL 格式演进（添加 span 字段而不破坏现有消费者）。现有非 span 感知消费者忽略新字段。监控端点通过 `forge run --daemon` 提供。

---

### 方向 D：存储生命周期管理（P1）

**为什么需要**：
这是债务三的对应修复。Memory 有 `Compact`，trace 无任何机制，checkpoint 有 `retain` 但未暴露。`.forge/` 目录在长期运行中仅单向增长。需要主动存储管理，就如同任何数据库需要 WAL 归档。

**核心挑战**：
1. **何时进行压缩**：trace 压缩不能在每次迭代时进行（开销），也不能在展开紧急时进行（恐慌）。需要基于迭代计数的触发（memory 风格）与基于水位标记的触发相结合。
2. **压缩 vs 聚合**：trace 压缩可以只是删除旧条目（数据丢失），或聚合成小时级的统计摘要（数据保留）。哪种更适合审计？
3. **`forge state` 子命令**：新子命令会触发跨 `cmd/forge` 的边界问题（债务五）。考虑放入 `internal/state` 包。

**预期架构变更**：
```
internal/persist/ttl.go             # 按时间的 prune 逻辑
internal/persist/watermark.go       # 磁盘使用率监控

trace.go: 添加 Rotate(maxEvents)、PruneOlderThan(duration)
checkpoint.go: 暴露 retain CLI flag
cmd/forge: forge state prune --trace <days> 子命令
```

**设计决策**：默认自动开启（保护不知情用户），而非选择加入。若 `.forge/` 目录消耗超过总 FS 容量 10%，打印告警。若超过 20%，自动压缩 trace 至最近 2 天的数据。

---

### 方向 E：全局化治理分发（ADR 0003 实现）（P2）

**为什么需要**：
ADR 0003 设计了共享治理的完整机制：用于共享资产的 git submodule，用于覆盖解析的 `project.yml extends` 字段。设计已就绪，远程位置待定，代码零实现。ForgeOS 的核心承诺是成为元框架——但当前每个被治理的项目在 `forge-init` 时获得一份独立快照，然后与母体失去联系。

此方向的紧迫性取决于已治理项目的数量。目前本仓库是唯一的生产用户，故为 P2。当第二个项目加入时，升为 P0。

**核心挑战**：
1. **`extends` 解析器**：当前 `project.yml` 有 `extends: []` 字段但不影响任何行为。解析器需要从 submodule 配置加载覆盖。
2. **`forge upgrade` 的 diff-aware 合并**：当前 `forge upgrade`（`harness/scaffold/forge-upgrade.mjs`）是覆盖风格，而非合并。若本地项目修改了 gate.mjs，上游升级不应无条件替换。
3. **路径解析改造**：所有 `require()` 路径（harness 中的 Node.js），所有 `os/exec` 调用（Go），以及所有 shell 调用都需要解析「先查项目根，再查 shared submodule」的路径。

**预期架构变更**：
```
internal/config/extends.go          # extends 解析器
internal/config/resolver.go         # 路径解析（项目第一，共享回退）

forge-core/cmd/forge/upgrade.go     # forge upgrade 命令
```

---

## 3. 接口设计建议

### 3.1 关键接口设计原则

**原则 1：所有持久化操作走单个 Registry**

当前，`internal/trace`、`internal/memory`、`internal/persist` 各自打开、写入、关闭自己的文件。这导致：
- 没有集中式文件锁
- 没有跨制品的写入排序保证
- 没有原子 group commit

建议引入注册表：

```
internal/persist/registry.go

type Registry struct {
    root string          // .forge/ 路径
    lock *os.File        // 进程级文件锁
}
func OpenRegistry(root string) (*Registry, error)  // 获取锁
func (r *Registry) OpenWriter(name string) (io.WriteCloser, error)
func (r *Registry) OpenReader(name string) (io.ReadCloser, error)
func (r *Registry) AtomicWrite(name string, data []byte) error
```

`trace.Tracer`、`memory.Store`、`checkpoint.Checkpoint` 各自接收 `*Registry`，而非直接打开文件。Registry 持有进程级锁；制品获得文件级锁。

**原则 2：可观测性通过接口而非具体类型**

当前，`Engine.Log(format, args...)` 写入标准输出。应将其替换为：

```
internal/telemetry/logger.go

type Logger interface {
    Info(msg string, keysAndValues ...any)
    Warn(msg string, keysAndValues ...any)
    Error(msg string, err error, keysAndValues ...any)
}

type Metrics interface {
    Counter(name string, value int64, labels ...string)
    Gauge(name string, value float64, labels ...string)
    Histogram(name string, value float64, labels ...string, buckets ...float64)
}
```

Engine 接收 `Logger` + `Metrics`，而非写入标准输出。默认实现写入标准输出（向后兼容）；telemetry 实现写入 JSON + Prometheus。

**原则 3：事件网关通过适配器集成，而非 fork**

Gateway 不应修改现有的 `Engine.Run` 签名。相反：

```
type EventHandler interface {
    // HandleEvent 返回要调度的 workflow 名称，或 "" 表示无匹配
    HandleEvent(ctx context.Context, event Event) (workflow string, params map[string]string, err error)
}

type Gateway struct {
    handlers []EventHandler     // 已注册的适配器
}
func (g *Gateway) Serve(ctx context.Context, addr string) error
```

这与 Harness 工具（`adapters/*.yml` → probe → run/N/A）的模式相同。Webhook 轮询器、GitHub webhook、定时触发器各自是 `EventHandler` 实现。

### 3.2 是否需要新的抽象层

**需要的：网关抽象**。Gateway 被建模为北极星中的引擎，但未实现。网关是 daemon 模式的前提条件。

**需要的：持久化 Registry**。这是跨进程安全的前提条件。没有集中式 Registry，每个新的持久化制品都需要重复解决锁问题。

**可选的：Telemetry 接口**。非当前所需（现有日志可工作），但引入得越晚，迁移成本越高。若 `Engine.Log` 的 20+ 调用点变为 `Engine.Logger.Info`，涉及 20+ 处修改加测试。建议在添加第一个 telemetry 消费者时进行。

**不需要的：服务边界**。共识（方向 E 之前进程内 monolith）是正确的。在 1–3 个开发者的场景下，将 `internal/orchestrator` 拆分为独立微服务不会带来收益。北极星知道这一点——它将其建模为 10 个引擎，但所有引擎都可以且应该在 v2 中在进程内实现。

### 3.3 保持向后兼容性

**JSONL 格式演进**：当前 trace JSONL 消费者通常不会因新增字段而损坏。新增 `span_id`、`parent_span_id`、`kind` 作为可选字段。旧消费者忽略未知键。

**`.forge/` 格式演进**：Registry 添加文件锁后，`forge run --resume` 使用旧版 checkpoint 应继续工作。锁仅在新写入时获取，旧文件读取时无需锁。

**CLI 标志添加**：新标志（如 `--trace-retain`、`--daemon`）应默认匹配当前行为（`--trace-retain=0` 表示「无限」，`--daemon=false` 表示「前台 CLI 模式」）。

**YAML schema 演进**：若 budget policy 变为 YAML 文件，它应是可选引用（`--budget-policy` 标志），而非默认加载。未提供政策时，行为回退至当前 CLI 标志。

---

## 4. 技术选型

### 4.1 需要与无需引入的新技术

**无需引入**：

| 技术 | 讨论 | 结论 |
|---|---|---|
| OpenTelemetry SDK | Go OTel SDK 引入 ~10 个传递依赖（`go.opentelemetry.io/otel` + 导出器）。违反 forge-core 零外部依赖准则。 | **拒绝。** 自实现最小 span + metrics 结构体（~100 行），遵循 OTel 数据模型但不导入 OTel 库。消费端（Prometheus 抓取 / OTLP exporter）是轻量适配器，可选编译。 |
| OPA/Rego | 策略引擎评估。引入图灵完备策略语言。当前 `mode.Policy` 加上 YAML 条件匹配已够用。 | **拒绝。** 预算治理使用与 `mode.Policy` 相同的 YAML+Go 模式。 |
| LiteLLM | 跨厂商模型网关。已规划于 v3。 | **推迟至 v3。** v2 维持单一厂商（Claude）。 |
| Firecracker | 微 VM 沙箱。已规划于 v3。 | **推迟至 v3。** v2 维持进程级隔离（output-cap + recursion-limit 作为轻量替代）。 |
| 向量数据库（Qdrant） | 语义知识检索。已规划于 v3。 | **推迟至 v3。** TF-IDF + `internal/knowledge` 的结构化提取已够用。 |
| embedded Redis/NATS | 进程内事件总线。用于通知和背压。 | **拒绝。** Go channel + `sync.Mutex` 对于单进程架构已足够。 |

**需要引入（或应严格评估）**：

| 技术 | 用途 | 评估 |
|---|---|---|
| `golang.org/x/sys`（Unix 子包） | `flock(2)` / 跨进程锁的 `syscall.Flock`。stdlib 的 `syscall` 包是冻结的。`golang.org/x/sys/unix.Flock` 是推荐替代。 | 这是 `x/sys`——非标准库，但作为 Go 项目，事实上是标准库。引入会将传递依赖计数从 0 提升至 1。可接受。 |
| Go YAML 库（goccy/go-yaml 或 gopkg.in/yaml.v3） | 替换 Python shim。当前 Python shim 是唯一剩下的强制性外部运行时依赖。 | **高优先级评估。** 质量要求：① 通过 yaml2json diff-test 当前全部 7 个真实 YAML 文件；② 正确处理 block scalars、literal/flow 模式、缩进折叠；③ 零 C 依赖。 |
| 结构化日志库（slog/zerolog/zap） | 替代自由文本 `Engine.Log`。Go 1.21 的 `log/slog` 是标准库。 | **slog（stdlib）。** 零依赖，结构化，Level。迁移：`Engine.Log(format, args...)` → `Engine.Logger.Info(msg, fields...)`。 |

### 4.2 自建 vs 采购决策依据

| 组件 | 决策 | 理由 |
|---|---|---|
| **事件网关** | **自建** | 领域特定（ForgeOS workflow 调度非通用事件处理）。适配器模式（poll/webook/timer）简单。Go HTTP 监听器 + 事件循环约 300 行。无采购选项能理解 ForgeOS domain 模型。 |
| **文件锁** | **自建** | ~20 行 Go。无理由采购。 |
| **结构化日志** | **采购（标准库）** | `log/slog` 是标准库。零成本，零依赖。 |
| **Spans/Metrics 数据模型** | **自建** | ~100 行 OTel 风格的 Go struct。用标准的 OTel 出口协议（OTLP exporter、Prometheus Text Format 序列化）。 | 
| **YAML 解析** | **采购（Go 库）** | 正确解析 YAML 很难（spacing、indentation、block scalars、tag resolution、merge keys）。自建解析器在 Sprint 27 中暴露出细微缺陷（`consumeBlockScalar` 损坏真实数据）。该负担不值得。一个经过实战检验的库（yaml.v3 或 goccy/go-yaml）可在几天内消除 Python shim。 |
| **二进制发布** | **自建（CI pipeline）** | GitHub Actions 发布 workflow + `goreleaser`（可选）。`forge self-update` 是简单的 HTTP 获取。 |

### 4.3 关键决策：Go YAML 库注入时机

Python shim 是 forge-core 零外部依赖策略中唯一剩下的裂痕。替换它的收益：
- 消除 `python3` + `PyYAML` 作为运行时依赖
- 每操作减少一次进程派生（`yaml2json.py` fork）
- 消除跨语言调试负担（Go → Python → JSON → Go 数据路径）

风险：
- 自建解析器在两个场合（「裸 `-` items」和「block scalar corruption」）都产生了逐字节差异测试失败的 bug
- 错误替换（引入另一个有 bug 的 YAML 库）将 shim 替换从「一周的 sprint」变为「月度的寻宝」

**建议路径**：
1. 不替换 Go 原生解析器——使用经过实战检验的库
2. `gopkg.in/yaml.v3` 是最保守的选择（标准非标准 YAML 库，~2M Go 项目使用）
3. 替换分三步：（a）添加库依赖 + 新解析器路径，与 Python shim 并列（b）对新路径运行 diff-test 7 个真实文件（c）所有 diff-test 通过后，切换默认路径，Python shim 保留为备选

---

## 5. 实施路线图

### 5.1 优先级

| 优先级 | 方向 | 理由 |
|---|---|---|
| **P0** | **B：跨进程并发模型** | 数据损坏风险。无锁 → 两个并行进程主动破坏生产数据。每个其他方向都假定存储为可靠基座；唯一无法降级的设计约束。 |
| **P0** | **A：事件网关 + Daemon 化** | 结构性瓶颈。系统已准备好成为 daemon，但仍被限制为 CLI 工具。无 daemon 模式 → 无 CI/CD 集成 → 无平台采用。 |
| **P1** | **D：存储生命周期管理** | 生产可靠性。trace 在长期运行中仅单向增长，无法压缩或分层。若磁盘写满，审计消失。 |
| **P1** | **C：结构化可观测性** | 操作可见性。无 spans → 多阶段调试靠手动 grep。无指标 → 24 小时无人值守运行无仪表化。P1 而非 P0，因为运行仍可继续——只是不可观察。 |
| **P2** | **E：全局化治理分发** | 组织杠杆。ForgeOS 的「元框架」承诺在 >1 个已治理项目之前无法验证。当前为 P2，因为本仓库为唯一生产用户；第二个增量时升为 P0。 |

### 5.2 阶段划分

**第一阶段：可靠基础（1 个 sprint）**

目标：消除数据损坏风险，推出第一个与现有架构兼容的 daemon 原型。

- P0-B：实现 `internal/persist/registry.go` + `lock.go`（约 100 行）
  - 集中式文件打开 / 锁定
  - `trace.Open`、`memory.Append`、`checkpoint.Save` 迁移至 registry
  - 新 `.forge/` 目录锁定，向后兼容旧目录
  - `forge status` 显示锁定状态

- P0-A（子集）：实现 `forge run --daemon` 最小原型（约 150 行）
  - 起一个 `/healthz` HTTP 端点
  - 优雅关闭（SIGTERM → 当前迭代完成后关闭）
  - 无 webhook 消费者——仅 daemon 化 + 健康端点

**第二阶段：可观测的存储（1 个 sprint）**

目标：trace 不再单向增长；操作者可随时间观察系统健康。

- P1-D：存储生命周期
  - `trace.Rotate(maxEvents)`：trace 到达 N 条事件后切割
  - `trace.PruneOlderThan(duration)`：按 TTL 清理
  - `persist.WatermarkChecker`：磁盘使用率监控，超过阈值时告警
  - `forge state prune --trace <days>` 子命令

- P1-C（子集）：Span 数据模型
  - `trace.Event` 新增 `SpanID` / `ParentSpanID` / `Kind`
  - `trace.Tracer` 新增 `StartSpan` / `EndSpan`
  - JSONL 格式向后兼容（旧消费者忽略新字段）

**第三阶段：事件驱动（1–2 个 sprint）**

目标：ForgeOS 可响应外部事件（webhook、定时器、API），不再是纯 CLI 工具。

- P0-A：完整事件网关
  - `internal/gateway/` 包（webhook + poll + dispatch）
  - GitHub webhook 适配器（`push` / `pull_request` → workflow 调度）
  - 定时触发器（类 cron 调度）
  - `forge run --daemon` 升级至完整 gateway 模式

- P0-A：`forge webhook` 单次事件触发器（无 daemon）的独立子命令

**第四阶段：可观测的导出（1 个 sprint）**

目标：ForgeOS 指标可被 Prometheus、OTel、Grafana 等标准基础设施消费。

- P1-C：Metrics 端点
  - `internal/telemetry/metrics.go`：计数器、仪表、直方图
  - `/metrics` 端点的 Prometheus 文本格式序列化
  - LoopEngine 指标：每次迭代 gate PASS/FAIL/NA 计数、agent phase 耗时、预算消耗

- P1-C：结构化日志
  - `Logger` 接口替换自由文本 `Engine.Log`
  - `slog` 默认后端（结构化 JSON 写入标准输出，`ENGINE_LOG_LEVEL=info`）
  - 向后兼容：`Engine.Log(format, args...)` 保留为委托至 `Logger.Info` 的辅助函数

**第五阶段：分发与治理（2 个 sprint）**

目标：`forge` 可发布、可更新，治理共享机制可运作。

- P2-E：全局化治理分发
  - 建立共享治理仓库（`forgeos/agent-os` 或沿 ADR 0003 的等效位置）
  - `project.yml extends` 解析器连接 submodule
  - `forge upgrade` 升级至 diff-aware 合并
  - `forge init --from-template <org/project>`：基于共享治理初始化项目

- 发布工程
  - CI release workflow（`goreleaser` 或自定义）：构建 + 签名 + 上传
  - `forge version --check`：检查 GitHub 新版本
  - `forge self-update`：自动二进制替换
  - Docker 镜像（forge + node + python3）

### 5.3 风险与缓解策略

| 风险 | 阶段 | 可能性 | 影响 | 缓解策略 |
|---|---|---|---|---|
| **文件锁在 cifs/NFS 上不工作** | S1 | 中等 | 高——网络文件系统不实现 `flock(2)` | `flock` 在 Linux NFS（自内核 2.6.12+）上通过 `fcntl` 锁定模拟工作。若锁定失败（`ENOSYS`/`EOPNOTSUPP`），降级至 advisory 告警而非 fail-close。 |
| **Daemon 模式暴露了 CLI 设计中的竞态条件** | S1 | 中等 | 中——`Engine.Log` 的 stdout 写入在并发运行时可能交错 | 第一阶段 daemon 切换日志至 `slog`（互斥保护）。现有 stdout 写入仍可并发。 |
| **YAML 库替换损坏了生产 workflow** | S2 | 低 | 高——所有 workflow 文件无法解析 | 保留并行路径 2 个 sprint。在全部 7 个文件上运行 diff-test，要求逐字节相等。Python shim 保留为 fallback，直至新路径在 >2 个真实项目上坐实。 |
| **`forge upgrade` 因本地覆盖层而损坏脚手架** | S5 | 中等 | 高——用户丢失本地更改 | 实现 three-way merge（base template + local overlay + upstream change）。冲突时 fail-close（打印差异，不自动合并）。与 git merge-file 相同模式。 |
| **多项目治理增加 CLI 构建的认知负荷** | S5 | 低 | 中——`forge` 命令范围蔓延 | 保持 `forge` 为「当前目录下项目的 CLI」。工作区操作用于显式子命令（`forge --workspace list`、`forge --workspace status --all`）。不为全局项目注册表重新设计 CLI。 |

---

## 综合

ForgeOS 架构成熟的标志不是已实现的引擎数量，而是**治理的深度**——系统用 9 个代理卡、8 道硬性检查、mode×lifecycle 中枢旋钮、真正的收敛信号、诚实代数以及 31 个 sprint 的纪律来自我约束的程度。在这方面，它比我见过的任何 LLM 产出系统都更成熟。

但它作为平台仍处于早期阶段。上述五个方向解决了 CLI 工具与编排平台之间的鸿沟。阶段一解决了最严重的生产风险（无锁，无 daemon）。阶段二是持续运营层面（可观测性，存储生命周期）。阶段三是平台的真正承诺（事件驱动，治理共享）。

需要注意的关键标准是：方向 B（锁）和 A（daemon）是**路径依赖的**——若我们不现在处理，每增加一个新的持久化制品或编排路径都会使迁移成本更高。方向 E（全局化治理）是**人口依赖的**——第二个被治理项目到来前不做任何事。基于不同驱动因素及时做出正确响应，是架构纪律的体现。
