Now I have a thorough understanding of the project — its architecture, history (31 sprints), existing analysis corpus (400+ docs, 137K lines), and the specific validation report. Let me compose the comprehensive architectural analysis.

---

# 架构师评估：ForgeOS 扩展方向验证报告

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 体系已具备以下结构性优势：

**a) 控制面/数据面分层已成型**

从 v0 的纯声明式（Claude Code 原生编排）到 v2 的自研 Go 运行时，`forge-core` 已构建起 `Orchestrator · Model-Router · Context-Engine · Memory-Engine · Evaluation-Engine` 五个核心引擎。这些引擎的接口设计遵循了 north-star 架构中「控制面分离」的原则——Orchestrator 不直接管理子进程生命周期路由决策，Router 不直接调度 agent。

**b) 带外执法（Out-of-band Enforcement）是正确且难得的架构决策**

`gate.mjs` / `arch-check.mjs` / `check.py` / `secret-scan` 运行在宿主独立的 harness 层，执行结果被 Orchestrator 消费但不被 Orchestrator 控制。这避免了「自己执法自己」的结构性矛盾——正是 k8s 的 Admission Controller / OPA sidecar 模式在 AI-编排领域的适配。

**c) 中枢旋钮（mode × lifecycle）是高质量的抽象**

将三个独立维度（Router 档位 · Harness 严格度 · Workflow 深度）统一到一个声明点（`project.yml`），是系统架构中「策略与机制分离」的典范。验证报告中的方向四（策略合规漂移）之所以成立，正是因为这套抽象已足够成熟，使 YAML 声明层与 Go 实现层之间的偏差有了业务意义。

**d) 诚实（honesty）作为工程纪律内建在架构中**

从 `forge accept` 的 N/A 诚实标注、到 `cappedBuffer` 的截断标注、到数据不存在时不伪造零值，诚实不是文档层面的修辞，而是架构层面的显式设计模式。这使 ForgeOS 在需要「故障模式推理」时比同类系统更有底气——因为当系统说「我不知道」时，它确实不知道。

### 1.2 当前架构的局限性

**a) 事件/数据流是单向的，缺乏回调通道**

当前所有输出路径都是 **push** 模式：`trace.Tracer` push 到 JSONL，`reportConvergence` push 到 stdout，`cost.go` push 到 trace。没有任何 **pull** 或 **subscribe** 机制让外部消费者主动收集系统状态。这正是方向三（外向通知总线）成立的架构根因——架构层面缺少一个 `EventBus` 或 `EventSink` 接口。

**b) 状态模型在单进程假设下设计**

`persist/checkpoint.go` 使用 `rename(2)` 保证单文件原子性，`memory/memory.go` 使用 `O_APPEND` 追加写入，`trace/trace.go` 使用 `sync.Mutex` 防护——这三者都是在「同一主机、同一进程」假设下工作的。随着 multi-agent 并行执行（`RunParallel`）和即将到来的分布式部署（north-star 的 Temporal 集群），这些假设会系统性失效。方向二（状态外部队改防护）的核心价值就在这里。

**c) 消费端缺失不亚于供应端缺口**

trace 基础设施（`trace.go` 的 `Event` 结构含 6+ event kinds、`Seq`/`DurationMs`/`CostUsdMicros` 等字段、constructor helpers 全部就绪）在生产侧已非常成熟，但**没有任何代码消费它**来做聚合呈现或运维告警。信号链是「产生→落盘→静止」，而不是「产生→落盘→消费→决策→行动」。方向一的离线回放引擎和方向三的外向通知总线，都是消费端缺失的表现。

**d) YAML↔Go 的桥梁是脆弱的 python shim**

`harness/yaml2json.py` 作为临时脚手架已在 Sprint 27 暴露出 `block-scalar` 损坏等真实 bug。Runbook 中关于 `blocking:` / `mode_gating:` 字段的漂移注说明，YAML 声明层与 Go 运行时层之间缺少类型安全的契约（protobuf / flatbuffers / Go struct tag schema）。这直接影响方向四的合规漂移检测——如果运行时根本无法可靠地知道自己「应该做的事」，漂移检测就是空中楼阁。

### 1.3 架构债务与技术债

| 债务类别 | 具体表现 | 严重度 | 建议期限 |
|----------|---------|--------|---------|
| **YAML shim** | `yaml2json.py` 无 type safety, 真实 bug 已爆 | 中度 | v2.5 |
| **没有事件总线** | stdout + JSONL 是唯一输出通道 | 中度 | P1 |
| **单进程状态假设** | `rename(2)` / `O_APPEND` / `sync.Mutex` 假设单进程 | 低度 | P1（分布式的 prerequisite） |
| **trace 数据零消费** | 丰富的 trace 数据落盘后无人问津 | 中度 | P0 |
| **`cmd/forge` 包文件数反复触限** | 多次拆包后仍反弹（Sprint 27/30/31） | 低度 | v3 包重组时 |
| **无需求清单的衍生问题** | `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 已补但需持续维护 | 低度 | 常态化 |
| **test 的 copy-anywhere 不变量** | 虽已加固（Sprint 16），但每加新文件就需同步 COPIED_FILES | 低度 | 需自动化 |

---

## 2. 扩展方向

基于验证报告和项目现状，我给出 5 个架构级扩展方向。这只是我作为独立架构师的分析——验证报告中的 5 个方向是另一方产出的分析，我基于自己的理解重新排列优先级和角度。

### 方向 A（P0）：事件驱动可观测性总线 —— EventBus 与 Sink 抽象

**为什么需要**

ForgeOS 的真实运行模式是无人值守自治循环。一次 `forge evolve` 可能跑数十分钟到数小时，跨越多个 agent phase、gate 检查、reviewer 循环。但是当前只有两种输出机制：
- stdout（人读，无人值守时零价值）
- JSONL 文件（事后读，但不能驱动实时行动）

当 evolve 循环在凌晨 3 点卡在某 phase 上时，没有任何机制通知运营商。这和验证报告的方向三（外向通知总线）一致，但我更倾向于将其定位为「可观测性总线」而非仅仅是「通知」——因为消费端不仅是人类告警，还包括自动化决策（自动中止、自动回退、自动扩容）。

**核心挑战和技术难点**

1. **接口设计**：需要一个 `EventSink` 接口使 producer 与 consumer 解耦。当前 producer（trace、cost、gate、orchestrator）各自定义了自己的输出格式和通道，没有统一的 `Event` 抽象。
2. **背压与降级**：如果 webhook 目标不可达，不应阻塞主循环。需要 `fire-and-forget` / `retry-with-backoff` / `circuit-breaker` 三级降级策略。
3. **事件分类与路由**：不是所有事件都应该触发通知。需要分类系统（`debug` / `info` / `warning` / `error` / `critical`）和过滤器，使不同 sink 订阅不同等级。
4. **与现有 trace 的关系**：trace 是**记录性**的（persistent、结构化、完整）；EventBus 是**响应性**的（实时、可筛选、可投递）。两者不应合并但应共享事件分类 schema。

**预期的架构变更**

```
// 当前
orchestrator → stdout/JSONL

// 目标
orchestrator → EventBus → stdout sink (继续保持现有行为)
                    ├── JSONL sink (trace 记录)
                    ├── Webhook sink (Slack/PagerDuty)
                    ├── Health check endpoint (K8s liveness probe)
                    └── Metrics sink (Prometheus counter/histogram)
```

**对现有系统的影响**

- 对现有 producer 是**零侵入**的：现有 stdout 调用保留，新 EventBus 作为平行通道追加。
- `reportConvergence` 从 `fmt.Printf` 改为通过 EventBus 发布收敛事件，stdout sink 保持向后兼容。
- 所有现有 trace 写入作为 JSONL sink 的一个实现保留，不破坏 `.forge/` 目录格式。

---

### 方向 B（P0）：多进程/多主机状态一致性保护

**为什么需要**

验证报告方向二的论证很扎实。当前状态模型的设计假设是「每项目一个 forge-core 进程」。但：
- `RunParallel`（`parallel.go`）已经证明多 phase 并行是真实需求
- `forge evolve` 的 LoopEngine 在多 iteration 间 checkpoint/resume 需要读写下一次 iteration 能看到完好的文件
- north-star 的分布式架构（Temporal 集群）要求状态文件可被多个主机安全访问

当前没有这些保障。三个具体风险：
1. **`persist/checkpoint.go`**：`rename(2)` 是单文件原子操作，但不是跨文件的——checkpoint 写入顺序在崩溃时可能导致 checkpoint 与 trace 时间戳不一致。
2. **`memory/memory.go`**：`O_APPEND` 写入在两个并发进程间是操作系统原子的，但 `Load` 读不到其他进程刚写的内容（无 fsync、无读锁）。
3. **`trace/trace.go`**：`sync.Mutex` 只保护同进程；多进程写 trace 会产生交错事件流。

**核心挑战和技术难点**

1. **最低可行保护**：ForgeOS 的纯 stdlib 零依赖纪律禁止引入 etcd/ZooKeeper。最小可行保护是 **file lock（`flock` + `LOCK_NB`）** + **内容校验（`sha256` 摘要）** + **跨文件时间戳一致性验证**。
2. **向后兼容**：现有 `.forge/` 目录没有锁结构，v1 checkpoint/trace 文件在 v2 升级后必须仍可读。
3. **配置化 vs 默认 safe**：flock 在多主机 NFS 上不可靠。默认应开启（单主机 safe），但在已知不可靠文件系统上允许降级为 advisory 模式。

**预期的架构变更**

```
// 当前
persist.Checkpoint: rename(2) → 单文件原子但跨文件不一致
memory.Store: O_APPEND → Load 可能读到不一致快照
trace.Tracer: sync.Mutex → 只保护单进程

// 目标（增量添加，非重写）
persist.Checkpoint: add flock + sha256 content hash + cross-file timestamp alignment
memory.Store: add read-verify loop + optional fsync-after-write
trace.Tracer: migrate from sync.Mutex to file-level flock for multi-process safety
```

**对现有系统的影响**

- 新增的 `persist/filelock.go` 和 `persist/checksum.go` 是纯新增叶子包，零依赖、零侵入。
- 现有 checkpoint/trace/memory 消费者（`evolve.go` 的 resume、`doctor/anomaly.go` 的异常检测）不受影响——它们只读数据，读锁对它们透明。
- 唯一行为变化：首次写入时获取锁，失败则错误上报（而非静默覆盖）。这本身就是对「外部篡改」的检测。

---

### 方向 C（P1）：声明-实现漂移持续检测（Policy Compliance Sentinel）

**为什么需要**

验证报告方向四揭示了项目内 YAML（`.agent/policies/modes.yml`、`routing/policy.yml`）与 Go 实现之间的不一致风险。这不是理论风险——Sprint 14 的 `per-phase model_tier`、Sprint 15 的 `workflow_depth.*`、Sprint 29 的 `on_rejected` 死代码，全部都是「声明在 YAML 中已写但 Go 运行时从未消费」的真实案例。

**但从架构角度看**，这个问题的本质不是「发现漂移后一次修复」，而是需要建立一个**持续检测机制**——因为：
- 每次 sprint 都可能引入新的 Go 实现与旧 YAML 声明的偏差
- 每次 `mode×lifecycle` 组合变化都可能暴露未接线的已声明字段
- YAML shim（`yaml2json.py`）本身就可能引入二次漂移（如 `block-scalar` bug）

**核心挑战和技术难点**

1. **检测 vs 修复的边界**：漂移检测引擎应该只**报**不**修**。Go 实现与 YAML 声明之间可能有合理的偏差（如 `blocking:` 字段从未被使用但也不想用——验证报告中已论证「镀金」），检测引擎不应自动同步。
2. **声明提取**：需要可靠的 YAML 解析器（最终淘汰 python shim，换 Go YAML 库，或者用 Go 原生的结构化读取）。
3. **实现提取**：需要从 Go 源码（结构体字段、接口方法、函数签名）自动提取「已实现」清单。Sprint 29 的「声明 vs 实现」审计已证明这可以手动做，但自动化需要静态分析。

**对现有系统的影响**

- 可作为 `harness/check.py` 的新检查实现（`check_policy_compliance`），复用既有框架。
- 不需要修改 forge-core 的任何 Go 代码——这是纯 harness 层的增强。
- 首次运行可能产生大量基线偏差：需要一个「已知偏差清单」（类似 `n/a` 但明确记录在案）。

---

### 方向 D（P1）：trace 数据消费层 —— 离线回放与分析引擎

**为什么需要**

验证报告的方向一被判定「已有覆盖」是因为离线回放的概念在之前已有展开。但我认为**概念层面的展开 ≠ 架构设计层面的就绪**——当前没有任何代码能回答以下问题：

- 「上次 deploy 失败的那次运行，gate 失败在哪？」
- 「最近 10 次运行的趋势是什么？成本上升了吗？延迟变长了吗？」
- 「这次 review 迭代和上次比，gate 裁决变好了还是变坏了？」

这些是 **P0 运营问题**，在没有离线回放引擎之前，「自治运行」对于运营商来说仍然是一个黑箱。

**核心挑战和技术难点**

1. **trace 格式演进**：`trace.go` 的 `Event._format` 字段已为版本兼容预留，但消费者需要能处理多版本的 trace 文件——目前没有版本感知的 decoder。
2. **大型 trace 处理**：24h 运行可能产生数万事件。回放引擎需要分页/过滤/聚合能力，不能全部加载到内存。
3. **跨数据源关联**：trace（事件流）+ checkpoint（快照）+ memory（知识）+ scorecard（路由学习）是四个独立的 JSONL/JSON 文件。回放引擎需要按 iteration Seq 关联它们。

**对现有系统的影响**

- 不修改任何 producer 代码（trace/checkpoint/memory 写入不受影响）。
- 新增 `forge replay` 命令（cmd/forge 层），和 `forge doctor` 对称——两者共享一个分析后端。
- `doctor/anomaly.go` 的 `DetectAnomalies` 可重构为回放引擎的分析插件。

---

### 方向 E（P2）：YAML→Go 的类型安全契约

**为什么需要**

这是我补充的验证报告未涵盖的方向。当前系统中 YAML 声明层与 Go 运行时层之间的桥梁是 **`yaml2json.py` + 手动反序列化**。这带来了三重风险：

1. **格式损坏**：Sprint 27 的 `block-scalar` 损坏 bug 是活生生的例子——所有 workflow 文件中的 `description:` 字段被注入 `"> "` 前缀，且测试本身失效导致全绿通过。
2. **类型不安全**：YAML 的 `on_fail` / `loop_back` 等控制流字段在 Python 中转后到 Go 的 `map[string]interface{}`，没有 schema 校验，运行时才能发现类型不匹配。
3. **声明漂移检测无法自动化**：方向 C 的漂移检测需要可靠地读取 YAML 声明——如果读取过程本身就可能出错，检测结果就不可信。

**核心挑战和技术难点**

1. **零依赖约束**：Go 标准库没有 YAML 解析器。选项：
   - **选项 A**：内嵌手写 YAML 解析器（已有 `internal/yaml2json` 雏形）。代价：维护一个 YAML 子集解析器，需要覆盖 block scalar / anchors / aliases / multi-doc。
   - **选项 B**：`go.mod` 加 `gopkg.in/yaml.v3`——这打破「零外部依赖」红线。但这可以用「仅接入 harness-yaml 转换层，核心引擎仍零依赖」来缓解。
   - **选项 C**：从 YAML 迁移到 TOML / JSON / HCL（更简单、Go 原生或易内嵌的格式）。代价：破坏现有 `.agent/` 格式的向后兼容性。
2. **版本化 schema**：引入 protobuf 或 flatbuffers 定义 `.agent/workflows/*.yml` 的 schema，自动生成 Go 结构和校验代码。但 protobuf 依赖 protoc 且需 Go 代码生成流程。

**对现有系统的影响**

- 这是基础设施层变更，影响范围广但大多是**透明的**（消费者看到的还是 Go 结构体）。
- 对现有 `.agent/` 文件格式应保持 100% 向后兼容（`yaml2json.py` 仍可作为 fallback 保留一段时间）。
- `check.py` 的治理检查可受益于类型安全的 schema——不再需要手写 JSON key 路径。

---

## 3. 接口设计建议

### 3.1 EventBus / Sink 接口设计原则

```
// Backward-compatible minimal interface
type Event struct {
    Kind      string            // "gate.status" / "phase.complete" / "converge.result"
    Severity  string            // "debug" / "info" / "warning" / "error" / "critical"
    Timestamp time.Time
    Source    string            // module path + name
    Payload   map[string]any    // structured, schema-per-Kind
}

type EventSink interface {
    Name() string
    Emit(context.Context, Event) error  // fire-and-forget / retry decisions are impl detail
}

// No interface change for existing producers:
//   Option A: existing stdout calls → stdout sink via EventBus (transparent)
//   Option B: dual-path (fmt.Printf + EventBus) in key decision points
```

设计原则：
- **最小承诺**：producer 只需构造 `Event` 结构体并调 `bus.Publish(ctx, event)`。不关心谁在消费。
- **背压由 Sink 实现负责**：EventBus 本身是非阻塞的（chan buffer 或 ring buffer），写满时默认 drop（可配置为 block）。
- **Sink 是可插拔的**：stdout、JSONL、Webhook、Prometheus 都是 `EventSink` 的实现。forge-core 核心引擎不 import 任何 sink 实现（通过函数注入或 `options.go` 模式）。

### 3.2 状态一致性保护接口设计

不需要新接口——这是对现有 `persist` / `memory` / `trace` 包的行为增强，通过以下方式向后兼容：

```go
// New, optional configuration
type LockStrategy int
const (
    LockNone    LockStrategy = iota // no locking (existing behavior)
    LockAdvisory                    // try lock, log warning on failure
    LockRequired                    // fail if lock unavailable
)

type CheckpointConfig struct {
    LockStrategy LockStrategy
    VerifyHash   bool
    // ...
}
```

### 3.3 声明-实现漂移检测接口

不需要新接口——这是纯 harness 层的检查，复用 `check.py` 扩展框架。唯一需要注意的设计决策是「已知偏差清单」格式：

```yaml
# .agent/policies/known-drifts.yml (new, optional)
known_drifts:
  - field: workflow.required_when
    reason: "Declarative-only, no runtime consumer needed. See docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
    since: "2026-07-02"
  - field: workflow.mode_gating.blocking
    reason: "Never used as false; implementing would be gold-plating. See Sprint 31 decision."
    since: "2026-07-03"
```

这确保漂移检测引擎不把「已知合理的偏差」误报为违规——这是从 `forge accept` 的「N/A is honest」模式继承的经验。

### 3.4 trace 消费层接口

```go
// Replay engine — reads .forge/ directory and provides structured views
type Session struct {
    Trace      []Event       // sorted by Seq
    Stages     []StageSummary
    Checkpoint *Checkpoint
    Memory     []memory.Entry
    Anomalies  []Anomaly
}

type ReplayEngine interface {
    Load(ctx context.Context, forgeDir string) (*Session, error)
    Timeline(session *Session) TimelineView
    RootCause(session *Session) []RootCause
    Compare(a, b *Session) ComparisonResult
}
```

这是纯新增接口，不修改任何现有代码。

---

## 4. 技术选型

### 4.1 需要引入的新技术栈

| 组件 | 推荐 | 候选 | 不推荐 |
|------|------|------|--------|
| YAML 解析 | 内嵌手写解析器（已有 `internal/yaml2json` 雏形） | `gopkg.in/yaml.v3` | 保持 python shim（已爆 bug） |
| 事件总线 | 自研（Go chan + ring buffer + sink 注册） | NATS（north-star 阶段） | Kafka（太重） |
| 指标暴露 | Prometheus `expvar` + `/metrics` endpoint | OpenTelemetry Go SDK | 自研指标引擎 |
| Webhook 通知 | 自研薄层（`net/http` post + retry + circuit-breaker） | Slack SDK | 买第三方通知即服务 |
| 状态锁 | `flock(2)` via `golang.org/x/sys/unix` | `os.Create` + pid file | etcd（外部依赖） |
| file checksum | `crypto/sha256`（标准库） | blake3 | MD5 |

### 4.2 第三方依赖评估标准

ForgeOS 的「零外部依赖」红线是合理的——它确保：
- `go build` 即工作（无 `go mod download` 失败）
- 审计简单（13 个包，纯 stdlib）
- 安全面小（无 CVE-patched C 库间接链接）

但这一红线不应无限期阻碍所有扩展。我建议的评估框架：

| 标准 | 权重 | 说明 |
|------|------|------|
| 打破红线后能恢复吗？ | ⭐⭐⭐ | 依赖应是「叶子层可以剥离」的，不是核心基础设施 |
| 依赖的稳定性？ | ⭐⭐⭐ | v1.x 以上、API 稳定、Go 社区标准 |
| 依赖的安全记录？ | ⭐⭐ | 无 CVE 历史、fuzzing 覆盖率 |
| 内嵌等效的维护成本？ | ⭐⭐ | 自写 YAML 子集解析器可能比 `gopkg.in/yaml.v3` + 定期 CVE 扫描更贵 |

**我的建议**：`gopkg.in/yaml.v3` 可以引进，但条件如下：
- 它不出现在 `forge-core/` 核心包的 `go.mod` 中
- 它仅作为独立工具（如 `forge-yaml-to-json` 静态二进制）存在
- 核心引擎运行时仍无外部依赖
- 这个工具在 CI 中由 `forge accept` 调用（同步哈ness adaper 模式）

### 4.3 自建 vs 采购

对于这个项目阶段（v2 工程验证 + dogfooding），**自建是唯一正确的选择**：

- 没有成熟的「AI 编排控制面的可观测性框架」可采购——每个系统的拓扑和事件 schema 都不同
- 没有现成的「AI agent 输出质量评估引擎」市场存在——这是开拓性产品阶段
- 核心目标不是构建通用可观测平台，而是**让 ForgeOS 团队自己能运营 ForgeOS**

«采购»在 north-star 阶段的 Temporal / NATS / Qdrant 等基础设施服务上合理，但在 v2 的控制面扩展上自建更合适。

---

## 5. 实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 理由 | 依赖 |
|--------|------|------|------|
| **P0** | 方向 B：状态一致性保护 | 防数据损坏→防错误决策；多进程执行已是现在时 | 无 |
| **P0** | 方向 A：EventBus + Sinks | 自治运营无法承受黑箱；trace 空有数据无消费 | 无 |
| **P1** | 方向 D：trace 回放引擎 | 利用已有 trace 数据基础设施；补运营可观测性基座 | 方向 A（事件 schema）可选 |
| **P1** | 方向 C：声明-实现漂移检测 | 安全合规补线；在已有检查框架中扩展 | 可靠 YAML 解析（方向 E） |
| **P2** | 方向 E：YAML→Go 类型安全契约 | 消除脆弱 shim；但不可干扰现有功能 | 无 |
| **P2** | 验证报告中的方向五（多仓库） | 高价值但已有覆盖；不阻塞单仓库用户 | 方向 A + B |

### 5.2 阶段划分

**阶段一（~2-3 sprints）：安全基座**
- 实现 `persist/filelock.go` + `persist/checksum.go`（flock + sha256）
- `memory.Store` 的 read-verify loop（可配置）
- `trace.Tracer` 的多进程安全 guard
- 为方向 A 做接口铺垫：定义 `Event` 结构体 + `EventSink` 接口，但不做实现

**阶段二（~2-3 sprints）：可观测性基座**
- 实现 stdout / JSONL 两个 sink（保持现有行为 100% 兼容）
- 实现第一版 EventBus（chan buffer + 注册机制）
- `reportConvergence` 迁移到 EventBus（stdout sink 保持向后兼容）
- `forge replay --timeline` 基本回放命令

**阶段三（~2-3 sprints）：自动化运维**
- Webhook sink（Slack webhook 集成）
- 在 `forge evolve` 中加入关键事件（循环卡死、预算逼近、质量恶化）的告警触发
- `forge replay --root-cause` 根因分析
- `check.py` 的漂移检测检查

**阶段四（~2-3 sprints）：基础设施加固**
- YAML 解析器重写（自建或审慎引入 `yaml.v3`）
- 基于可靠 YAML 解析的声明-实现漂移全自动化检测
- 跨文件时间戳一致性校验成为默认启用而非可选

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 方向 A 的 EventBus 使核心循环变重（goroutine + chan 开销） | 低 | 中 | 不阻塞 hot path；EventBus 为 optional channel，不在热路径上 |
| 方向 B 的 flock 在 Docker/容器中行为不确定 | 中 | 高 | `os.MkdirTemp` + fallocate-based lock 作为容器兼容的后备；默认检测容器并降级为 advisory |
| 方向 C 的漂移检测产生大量误报（开发者因此 ignore 它） | 高 | 高 | 「已知偏差清单」是必须的随附产物；首次运行必产生基线 → 必须在 PR 审查中同时更新清单 |
| 方向 D 的回放引擎变成维护负担（trace 格式每 sprint 变一次） | 中 | 中 | `_format` 字段已预留版本号；版本兼容测试（`TestReplay_V1Trace`、`TestReplay_V2Trace`⋯）需随每个 trace 变更添加 |
| YAML→Go 类型契约（方向 E）变成重写项目（scope creep） | 中 | 高 | 严格限定在「解析→Go 结构体」层，不改为「验证→审计→代码生成」；在 2 sprint 内完成，超出则回退为 python shim + 额外测试补强 |

---

## 总结

| 维度 | 判断 |
|------|------|
| **验证报告质量** | 🟢 高质量。代码证据扎实（少量位置偏差不伤大局），差异化验证方法正确，3/5 方向（二、三、四）经得起交叉检验 |
| **真实新颖方向** | 方向二（状态外部队改防护）、三（外向通知总线）、四（声明-实现漂移检测）是**真正的架构缺口**，且与 ForgeOS 当前的工程红线（零依赖、纯 stdlib）和阶段约束（v2 工程验证）一致 |
| **现有覆盖的方向** | 方向一（执行溯源/回放）概念已有覆盖但**架构实现层仍为零**——概念展开远早于实现就绪；方向五（多仓库）已有坚实展开 |
| **我补充的关键发现** | YAML→Go 类型安全契约（方向 E）是验证报告未覆盖但影响所有方向的基础设施缺口——只要 python shim 存在，基于 YAML 声明的一切（方向 C 的漂移检测、方向 E 的一致性）都不可信 |
| **架构健康度** | 🟢 良好。架构债务明确、诚实标注、已在渐进偿还。当前没有任何需要「大规模重写」的问题——全部扩展方向都是增量式、可并行的 |
| **建议优先行动计划** | P0 打基座（状态锁 + EventBus 接口定义）→ P1 补消费端（回放引擎 + 漂移检测）→ P2 加固基础设施（YAML 契约）|
