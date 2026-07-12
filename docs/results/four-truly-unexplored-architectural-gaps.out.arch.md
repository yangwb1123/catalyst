现在我已全面阅读项目代码和架构文档。以下是我的架构分析。

---

# ForgeOS 架构评审与技术设计分析

**角色**：资深架构师  
**评审对象**：用户提供的 `four-truly-unexplored-architectural-gaps.md` 评审文档  
**交叉验证基准**：仓库源码 + 80+ 篇已有分析 + `.agent/` 架构骨架 + `forge-core` 全包

---

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 在 v2 阶段展现的架构质量在同类项目中非常罕见，值得明确记录：

**1.1.1 零依赖内核（Architecture Win — Foundational）**

`forge-core` 的 `go.mod` 无 `require` 块——这是*有意识的工程决策*，不是运气。它强制团队自己解决每个正交问题，从而：

- 保持了对核心抽象（Workflow / Engine / Convergence）的完全控制权
- 消除了传递依赖注入的间接性（没有 Spring 式魔法）
- 使 `go build` 成为瞬时操作，CI 缓存友好
- 让架构边界不可绕过（你不能偷懒引入一个 ORM 然后跨层调用）

**1.1.2 带外执法（Architecture Win — Correctness）**

「真相之源 = out-of-band gate，host hook = 加速器适配器」的设计区分了 enforcement plane 和 acceleration plane。这意味着：

- 即使宿主（Claude Code / Codex）的 hook 机制改变，执法不降级
- 每个宿主只需一个薄 adapter，核心逻辑不重复
- CI 与本地执行共享同一套 gate，不存在「CI 绿了本地红了」的不一致

**1.1.3 收敛信号纯函数化（Architecture Win — Testability）**

`internal/converge` 将收敛判定建模为 `Signals → []Result` 的纯函数——无副作用、无 I/O、可独立测试。这是系统中架构最清晰的模块之一。Sprint 28-29 对 `Signals` 全字段的审计正是在这个干净的 seam 上完成的。

**1.1.4 Central Knob 的统一旋转（Architecture Win — Cohesion）**

`mode × lifecycle` 同时驱动 Router 档位 · Harness 严格度 · Workflow 深度 · Migration 策略——这是单一职责在系统设计层面的体现。一个旋钮改变 4 个维度的行为，降低了操作复杂度。

### 1.2 架构局限性

**1.2.1 `cmd/forge` 的包级别内聚性债务——已确认、需行动**

数据：12513 行、17 个非测试文件、全部在 `package main`、涵盖 12+ 责任域（run/evolve/gate/check/accept/route/migrate/detect/validate/scorecard/doctor/preflight/approve）。

这已经超出了「胶水 CLI 包」的合理范围。当前拆分为时未晚——Sprint 27-31 已将多块逻辑下沉到 `internal/{doctor,attribution,gate/resolve}`，但 `cmd/forge` 仍保留了太多纯逻辑（如 `reportConvergence` 的两次独立实现——`main.go:399` 和 `orchestrator/loop.go:346`，验证了评审文档的证据）。

**这不是紧急风险**——Go 的 `package main` 编译后是单一二进制，运行时没有额外的 memory/cpu 成本。但它是**架构信号**：当新来者需要理解 12 个子命令的路由而入口文件长达 499 行时，认知负荷已经越过了可持续的阈值。

**1.2.2 双执行轨道孤岛——已确认、需协调**

| 维度 | `forge run/evolve` | `pi-batch.py` |
|------|-------------------|---------------|
| 宿主 | forge-core Go 运行时 | 独立 Python 脚本 |
| 工作流表达 | YAML workflow + phases + gates | YAML tasks（无 gates/convergence） |
| 治理整合 | 全 harness 闸门 | 零治理 |
| 模型路由 | routing.TierFor（多维） | 每个 task 声明 model |
| 收敛 | converge.Converge 纯函数 | 无——执行完即结束 |

两个系统共享了「YAML 驱动的 agent 批处理执行」的概念，但当前没有共享抽象。评审文档将此定位为**集成机制问题**，而先前分析（`expansion-directions-v3`）定位为**架构策略问题**。两者互补：

- 策略问题先决：forge-core 应该成为 pi-batch 的执行引擎吗？
- 机制问题才可落地：如果答案是肯定的，YAML schema 兼容性、task→phase 映射、pi-batch 当前独有的 `ThreadPoolExecutor` 并行模式如何适配？

**1.2.3 进程级健康检查的缺失——已确认、视容器化路线而定**

当前 forge 进程在 `evolve.go:495` 只处理 SIGINT/SIGTERM。无 SIGUSR1（触发状态 dump）、无 HTTP 端点、无 socket 文件。这不是今天的问题——`forge run`/`evolve` 是短期进程，健康检查在产品级部署中才是必要的。

**但 north-star 架构（`north-star.md`）的描述已经预设了容器化部署**（Firecracker microVM in data plane）。当 v3 引入 `forge serve`（常驻网关进程）时，健康检查端点的缺失将成为 blocking gap。

**1.2.4 Prompt ContextCache 可观测性盲点——真正的新发现**

评审文档方向四的洞察完全准确：

- `cache.go` 的 `builds` 字段只在测试中读取（`cache_test.go:64,176,183,219`），生产路径零消费
- 无 `hitCount`/`missCount`/`entryCount`/`memoryUsage` 度量
- `cardText` map 无大小限制（随 `.agent/agents/*.md` 数量线性增长）
- `Invalidate()` 注释明确说「v1 NEVER calls this」

这构成一个**可观测性债务**：当前 cache 的大小和命中率完全是黑箱。虽然 cache 本身的 I/O 成本是微秒级（注释诚实声明），但：

1. **没有度量意味着无法验证「它仍然是微秒级」**——agent 卡数量增长 10 倍后，`filepath.Glob` + 每个文件的 `os.ReadFile` 开销是否仍可忽略？无法回答。
2. **没有度量意味着回归不可检测**——如果未来某次修改意外导致 cache miss 率飙升，没有告警会触发。
3. **更广泛地**，这是系统「度量的最内层缺口」：trace 系统覆盖引擎层（事件持续时间），但数据结构内部状态不被度量。

---

## 2. 扩展方向

基于以上评估，我提出 5 个高价值架构扩展方向，按优先级排序。

### 方向 A：「可观测性基础设施」作为平台服务 —— P0

**为什么需要**

当前的可观测性架构是**嵌入式的**（每个组件自己记录 trace 事件），但缺少**度量基础设施**（metrics) 和**健康契约**（health probes)。评审文档的方向三（健康端点）和方向四（缓存可观测性）指向同一个根因：系统没有「组件必须暴露度量和健康状态」的契约。

三个缺口其实是一个问题的不同表现：

| 表现 | 根因 | 影响的 north-star 组件 |
|------|------|----------------------|
| 无 HTTP `/healthz`/`/readyz` | 无「进程必须暴露 liveness」契约 | Gateway, Orchestrator, Model Router |
| Cache 无 hit/miss/size 度量 | 无「数据结构必须暴露内部健康」契约 | Context Engine, Memory Engine |
| Trace 无因果链（无 TraceID/SpanID） | 无「事件必须关联因果上下文」契约 | 所有引擎 |

**实施建议**

在 `internal/infra/` 下建立三个正交设施：

1. **Health registry**：每个组件在启动时注册一个 `HealthProbe`（`func() health.Status`），由统一端点聚合。`health.Status` 包含 `{Alive bool, Ready bool, Detail string}`。默认实现报告 `true`，v2 不需要容器化时零开销。

2. **Metrics registry**：一个 `Gauge(name string) Gauge` / `Counter(name string) Counter` 工厂，内部用 `atomic` 原语实现（零外部依赖），trace 事件可引用 metric 快照。ContextCache 注册 `cache.card_count` / `cache.build_count` / `cache.hit_ratio`（用 `sync/atomic`）。

3. **TraceID 传播**：当前 `trace.Event` 无 `TraceID`/`SpanID`。这是 architectural debt——跨组件因果链无法重建。有两种成本路线：
   - **廉价路线**：`trace.NewEvent` 自动生成 64-bit random TraceID，不传播（同一进程内 trace 可关联，跨进程不行）
   - **完整路线**：TraceID 通过 context.Context 传播到 `CommandExecutor` 的子进程环境变量——这需要修改 `orchestrator` 和 `CommandExecutor` 的接口

**技术难点**

- 零依赖约束下实现 metrics registry：Go 的 `sync/atomic` 足够（Counter/Gauge），但直方图（Histogram）需要 float64 atomic——当前 `sync/atomic` 不直接支持。曲线解法：用 `math.Float64bits` + `CompareAndSwapUint64` 实现一个 `AtomicFloat64`。
- TraceID 生成：`crypto/rand` 是标准库的一部分，可以用于生成 128-bit ID。或者用时间戳 + 进程 PID + 递增序列号的 64-bit 组合（更快、无熵池压力）。

### 方向 B：`cmd/forge` 的渐进式包架构重构 —— P0

**为什么需要**

这是「先拆分，再继续」纪律对 forge-core 自身的应用。当前 17 个文件的 `package main` 已经接近认知负荷临界点。

**分层策略**

```
cmd/forge/main.go          → 入口 + 子命令表 + usage（~100 行）
cmd/forge/run.go           → run/evolve CLI flag 解析 + 参数收集
cmd/forge/gates.go         → gate/check/accept CLI 胶水
cmd/forge/workflow.go      → loadWorkflow + transcode + resolveLifecycle
cmd/forge/signals.go       → gatherSignals + reportConvergence（从 main.go 提取）
cmd/forge/options.go       → runOpts + bindRunOpts + flag 常量
```

**关键约束**

- **纯提取、零行为变化**：重构只移动符号，不重命名包、不改变导出签名
- **渐进式**：每个 PR 移 1-2 个责任域，不「大爆炸重写」
- **不与已有的 `internal/` 下沉冲突**：`internal/{doctor,attribution,gate/resolve}` 已经展示了正确的下沉方向——纯逻辑下沉，CLI 胶水留在 `cmd/forge`

**6 个月后的目标态**

```
cmd/forge/           ← 纯 CLI 胶水：flag 解析 + 参数传递 + 错误渲染
  7-9 个文件
  每文件 ≤ 400 行
internal/cli/        ← CLI 共享类型：flag 定义、option 结构体、常量
  2-3 个文件
```

### 方向 C：统一批处理执行抽象 —— P1

**为什么需要**

pi-batch 和 forge-core 当前是两个平行的 agent 执行系统，共享「YAML 驱动的批处理」概念但没有共享抽象。这导致：

- pi-batch 的 499 行 Python 零测试覆盖（Sprint 27 已确认）
- pi-batch 的 `ThreadPoolExecutor` 并行模式与 forge-core 的 `RunParallel`（Kahn 拓扑 + wave 编排）不兼容
- pi-batch 的任务级 `model` 声明与 forge-core 的 workflow 级 `model_tier override` 不兼容
- pi-batch 产出无 gates / convergence 检查

**三个选项的权衡**

| 选项 | 工作估算 | 对 pi-batch 的影响 | 对 forge-core 的影响 |
|------|---------|-------------------|---------------------|
| A：pi-batch 的 task→forge-core workflow 桥接 | 2-3 sprint | pi-batch 作为薄 CLI 层，实际执行委托给 forge-core | 需扩展 `Emits` 为通用 `Outputs` 契约 |
| B：forge-core 内置「快速 task」模式（非正式 workflow） | 3-4 sprint | pi-batch 标记为 legacy | 需引入新的简化 phase 类型（task phase），无 gate 要求 |
| C：不做统一，仅保证 schema 兼容性 | 1 sprint | 零变化 | 加一个 `pi_batch_task:` 字段标记以兼容 pi-batch schema |

**我的推荐**：选项 B。理由：

- 选项 A 把 pi-batch 变成薄代理层，但保留了一个额外进程跳转（python CLI → forge-core），弊大于利
- 选项 C 只是推迟了问题
- 选项 B 让 forge-core 获得一个「轻量 task 执行」模式，这对未来 onboarding 新项目也有用（无需写完整 workflow 文件）

### 方向 D：契约化 Phase 副作用声明 —— P1（走完方向 A 后可降级为 P2）

**为什么需要**

当前 `Phase.Emits` 声明零消费（`engine_build.go:198` 只用于 narrate readonly 提示，编排器未读取）。结合 review 文档中 `reportConvergence` 的重复实现（`main.go:399` vs `orchestrator/loop.go:346`），暴露了一个更大的缺口：**系统不知道一个 phase 产生了什么输出**。

方向 A（可观测性基础设施）落地后，这个方向可以自然生长出来——每个 phase 执行前声明预期输出（文件路径模式），执行后验证预期 vs 实际输出是否一致。

**实施路线（0 行为变化的基础设施先行）**

1. 先让 `Emits` 被编排器**读取并记录**（不执行，仅 trace）
2. 再让 `Emits` 被**验证**（执行后检查文件是否存在）
3. 最后让 `Emits` 被用于**成本归因**（每个 phase 输出文件 = 对 ROADMAP 进度的贡献）

### 方向 E：Cross-Agent 因果追踪体系 —— P2

**为什么需要**

当前 trace 系统记录事件序列（顺序保证由 `trace.Tracer.mu` 提供），但缺少因果链——每个事件不知道「我因哪个事件而生」。

当 `forge evolve` 运行 20+ 迭代、100+ phase、多次 loop-back 后，回答「这个 gate 失败的原因链是什么？」需要人工逐事件阅读。

**技术方案**

- `trace.Event` 增加 `CausedBy EventID`（可选字段）：在 `orchestrator.go` 中，当 `runAgentPhase` 因为 gate FAIL 触发 `loopBackTo` 时，loop-back 的 target phase 创建的事件设置 `CausedBy` 指向触发 gate 的事件
- 这不需要修改事件序列化格式——当前 `Event` 的 JSON tag 模式已经是 `omitempty`，新字段空值时和平共存

---

## 3. 接口设计建议

### 3.1 健康检查接口契约

如果采用方向 A 的 health registry，健康检查接口应该：

```go
// internal/infra/health.go

type Status struct {
    Alive  bool   // 进程在运行（OS 级）
    Ready  bool   // 可以处理请求（组件级）
    Detail string // 人类可读的原因（可选）
}

type Probe func() Status

type Registry struct {
    probes map[string]Probe // 原子性
    mu     sync.RWMutex
}

func (r *Registry) Register(name string, probe Probe)
func (r *Registry) Liveness() Status  // AND 所有 Alive
func (r *Registry) Readiness() Status // AND 所有 Ready
```

**设计原则**：
- 零依赖：纯 Go 标准库
- 无网络监听：Registry 只是状态聚合器，HTTP 端点由调用者额外绑定
- 默认安全：新注册的组件默认 `Alive=true, Ready=true`，不因缺失 probe 而误报不健康
- 向后兼容：当前 forge run/evolve 是短期进程，不绑定 HTTP 端点，则 health registry 零运行时开销

### 3.2 Cache 可观测性接口

对 `ContextCache` 的扩展应该保持其当前简洁性：

```go
// 新增字段（在 internal/prompt/cache.go 中）
type ContextCache struct {
    // ... 现有字段
    hitCount  atomic.Int64
    missCount atomic.Int64
}

// 新增方法
func (c *ContextCache) Metrics() CacheMetrics  // 线程安全快照
type CacheMetrics struct {
    HitRatio float64
    EntryCount int
    BuildCount int
}
```

**设计决策**：`Metrics()` 返回快照而非指针，避免调用者在读取后继续修改。

### 3.3 pi-batch → forge-core 的桥接接口

如果选择方向 C 的选项 B（内置「快速 task」模式），新增 phase 类型：

```yaml
# 与现有 workflow 兼容的快速 task 模式
tasks:
  - task: "analyze project"
    model: claude-sonnet
    output: docs/analysis.md
    # 无 gate、无 depends_on、无 on_fail
```

在 forge-core 侧，这映射到一个 `Phase{Type: "task", Agent: "default", Gates: nil}`——无 required_gates 的 phase 跳过 harness，直接执行 agent。

---

## 4. 技术选型

### 4.1 是否需要新的技术栈？

**不需要。** 评审文档的四个方向都可以在当前技术栈内解决：

| 方向 | 需要新技术 | 解决方案 |
|------|-----------|---------|
| cmd/forge 拆分 | 否 | 纯 Go 标准库重构 |
| pi-batch 整合 | 否 | 用 forge-core 的 `asset.Workflow` 表达 task |
| 健康检查 | 否 | `net/http` + `os/signal`（都是标准库） |
| 缓存可观测性 | 否 | `sync/atomic` 计数 |
| 可观测性基础设施 | 否 | `sync/atomic` + `math.Float64bits`（直方图） |

**结论**：forge-core 的零依赖约束没有成为这些扩展的障碍，反而保证了所有方向都可以在不引入攻击面或传递依赖的前提下完成。

### 4.2 何时考虑引入外部依赖？

forge-core 的零依赖纪律应该在以下条件下才打破：

1. **功能无法在合理工程成本内用标准库实现**（例如：Temporal 的 durable wait）
2. **依赖本身是零传递依赖的**（例如：golang.org/x/sync 仍然是 x 仓库，没有外部传递依赖）
3. **依赖仅在可选路径中引入**（例如：YAML 解析器在 `yaml2json.go` 中，不影响核心引擎包）

当前没有任何方向满足条件 1。

### 4.3 自建 vs 采购

评审文档中没有涉及外部采购决策。所有提出的扩展方向都是纯自建。

---

## 5. 实施路线图

### 优先级排序

| 优先级 | 方向 | 估计 | 前置条件 | 独立可交付 |
|--------|------|------|---------|-----------|
| **P0** | A: 可观测性基础设施 | 2-3 sprint | 无 | 健康 registry + metrics registry + CacheMetrics |
| **P0** | B: cmd/forge 重构 | 3-4 sprint | 无 | 每 sprint 1-2 个责任域下沉 |
| **P1** | C: task 模式（方向 C 选项 B） | 3-4 sprint | 方向 A（trace/health 先就绪） | YAML task phase + CLI `forge task` |
| **P1** | D: Phase 副作用契约 | 2 sprint | 方向 A（metrics registry 就绪） | Emits 被 trace + 被验证 |
| **P2** | E: 因果追踪 | 1-2 sprint | 方向 A（TraceID 就绪） | CausedBy 字段 + loop-back 因果链 |

### 阶段划分

**阶段 1（Sprint 32-34）— 可观测性就绪**

```
Sprint 32:
  - internal/infra/health.go: Registry + Probe 类型
  - internal/infra/metrics.go: Counter + Gauge（atomic）
  - internal/prompt/cache.go: CacheMetrics + atomic hit/miss 计数
  - trace.Event: TraceID 自动生成

Sprint 33:
  - cmd/forge: "signals" 责任域提取（reportConvergence → new file）
  - cmd/forge: "options" 责任域提取（runOpts → new file）
  - health registry 接 SIGUSR1 dump

Sprint 34:
  - cmd/forge: gate/check/accept 胶水提取
  - cache metrics 在 gatherSignals 中输出（forge run 可观测）
  - 回归检查：cmd/forge 文件数 ≤ 12
```

**阶段 2（Sprint 35-37）— Task 模式 + Phase 契约**

```
Sprint 35:
  - asset.Phase 扩展 PhaseType "task"
  - 无 gate 的快速 agent 执行路径
  - pi-batch.py 更新为可选用 forge-core 执行

Sprint 36:
  - Emits 声明被编排器读取 + trace
  - Emits 执行后验证（文件存在性检查）

Sprint 37:
  - CausedBy 因果关系关联
  - LoopEngine.onIteration 因果链记录
```

### 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| cmd/forge 拆分打断 sprint 交付节奏 | 中 | 中 | 每次拆分后跑完整 `forge accept` + 所有集成测试；每个拆分独立 PR，不超过 1 sprint |
| 可观测性基础设施在健康检查上「做了但没人用」 | 高 | 低 | phase 1 只做 infrastructure，不绑定 HTTP 端点；phase 2 当需要容器化时才绑定 |
| TraceID 传播到子进程引入安全/隐私问题 | 低 | 中 | TraceID 默认不传播到子进程 env，通过显式 `--trace-id` flag 可选启用 |
| pi-batch 整合被 stakeholder 拒绝 | 中 | 中 | 保持 pi-batch 当前独立运行能力，forge-core 的 task 模式作为**可选替代**；不强制迁移 |

---

## 总结

| 维度 | 评估 |
|------|------|
| **当前架构质量** | ⭐⭐⭐⭐⭐（在同类项目中出类拔萃——零依赖内核、out-of-band 执法、收敛信号纯函数） |
| **评审文档质量** | ⭐⭐⭐⭐（方向四是真 gap；方向一至三是「已有确认的核心题 → 实施方案」的深化；差异化声明的修正建议公正） |
| **真正的新发现** | 方向四（ContextCache 盲点）是 80+ 篇分析中唯一从未被审视的 |
| **最高优先级行动** | 可观测性基础设施（P0）+ cmd/forge 渐进重构（P0） |
| **长期架构债务** | 双执行轨道孤岛（P1）、Phase 副作用契约缺失（P1）、跨-agent 因果追踪（P2） |

**一句话建议**：先建可观测性地基（health + metrics + CacheMetrics），再在干净的地基上重构 cmd/forge 和整合 pi-batch——从「知道你做了什么」到「知道你做得怎么样」再到「让你做得更好」。
