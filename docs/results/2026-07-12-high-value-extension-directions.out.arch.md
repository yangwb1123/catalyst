# 架构分析报告：ForgeOS 从「基础设施验证」到「平台封装」的跃迁

---

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的架构布局展现出几个罕见的工程品质，值得先肯定再批评：

**优势一：分层清晰，依赖单向。** 从 `internal/orchestrator` → `internal/converge` → `internal/memory` → `internal/persist` 的调用链是单向且可追踪的。13 个 Go 包、零循环依赖、零外部依赖——这在同类项目中极为罕见。这为后续所有扩展提供了干净的基座。

**优势二：「声明 vs 实现」审计成为工程纪律。** Sprint 29-31 的系统性自审（`converge.Signals` 全字段审计、`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的编制）是整个架构最大的隐形资产。多数项目死在「不知道自己不知道什么」，ForgeOS 的 `honesty` 文化让架构债务显性化——这是平台规模化的前提。

**优势三：中枢旋钮（mode×lifecycle）的穿透力。** 一个设置同时驱动 Router 档位、Harness 严格度、Workflow 深度、 migration 行为——这是真正的架构杠杆点。10+ sprint 的增量建设没有把这个旋钮变成「又一堆 if-else」，而是保持了正交性。

**优势四：载重墙（Load-Bearing Wall）架构的诚实落地。** `harness/` 的 host-independent 设计、带外执法为真相之源、CLI 加速器为可选冗余——这个判断保护了 ForgeOS 不被任意一个上游 CLI 的 API 变更绑架。

### 1.2 局限性（架构债务）

**局限一：「单仓单项目」的隐含假设。** 当前整个代码库——从 `forge-init` 的 `COPIED_FILES` 列表到 `manifest-integrity` 检查——都假设被治理的对象是一个 Git 仓库、一个 `project.yml`、一个 `harness/` 副本。这与 ADR 0003 的 `extends` 机制之间存在结构性张力：当前架构的「单例假设」嵌入太深，不是改一个字段能解决的。

**局限二：事件驱动缺位。** 从 north-star 架构（Temporal、NATS、事件驱动持久化 workflow）到当前实现（同步 CLI 调用、`context.Background()`、无 signal handler）之间的差距是架构上最大的断层。`forge run` 是一个同步 CLI 进程——它不能持久等待、不能从崩溃中恢复、不能水平扩展。当前阶段这完全合理（v0-v2 的 CLI 单例模式），但所有五个扩展方向都被这个隐含约束限制着。

**局限三：单进程单例模式的无处不在。** 多版本并行冲突、checkpoint 覆盖写、trace.jsonl 写冲突——这些 edge case 的根因是同一个：架构假设「同一时刻只有一个 forge 进程操作工作目录」。这个假设在 CLI 单例模式下成立，到了 gateway/daemon 模式就会成为第一道裂缝。

**局限四：`memory` 包的附加值远低于其基础设施复杂度。** 输入文档正确地指出：`Append` 调用仅 1 处、`Query` 仅 1 处、中间的提炼管道是空的。当前 `memory` 是一个基础设施完整但业务信号稀薄的存储层。它不是错的——它只是在等消费方。

### 1.3 关键设计决策评估

| 决策 | 评价 | 理由 |
|---|---|---|
| 纯 Go 标准库零依赖 | ✅ 正确 | 编译单二进制、零供应链风险、适合控制面 |
| YAML 经 python shim 转码 | ⚠️ 短期合理，长期债务 | Go 标准库无 YAML 解析，当前零依赖承诺合理；但 python 是运行时依赖 |
| dry-run 为默认安全默认 | ✅ 正确 | 先叙述后执行，防预算烧穿 |
| 同步 CLI 单例而非 daemon | ✅ 当前合理 | 符合 v0-v2 的「基础设施验证」阶段；daemon 需要 signal handling + 持久化 + 重试 |
| TF-IDF 而非 embedding | ✅ 正确 | 知识规模 < 1000 条目时，embedding 的复杂度收益比不成立 |
| `forge accept` 的 N/A 诚实策略 | ✅ 卓越 | 防伪造通过，是组织级信任的基础 |

---

## 2. 扩展方向

### 方向一：运行时弹性（Runtime Resilience）← 当前权重最高的架构投资

**为什么需要：** 同步 CLI 模式的单进程假设即将被 gateway（P0）、daemon（P1）、多版本并行（跨领域）三个方向同时冲垮。不做运行时弹性，后续所有扩展都会在一个脆弱的地基上叠加复杂度。

**核心挑战：**
- **非中断式 checkpoint：** 当前 `saveCheckpoint` 是 overwrite 模式，两轮并行就冲突。需要从 overwrite → append-log → WAL 的演进路径。
- **优雅关闭与恢复：** `context.Background()` → signal handler (`SIGHUP`/`SIGTERM`) → 持久化状态保存 → 重新接入恢复。
- **文件锁的最小单元：** 不是「锁整个工作目录」而是「按资源粒度锁」（checkpoint 用 `flock`，trace/memory 用 `O_APPEND` 原子性）。

**预期的架构变更：**
```
internal/persist/
  flock.go         ← 新增：advisory lock 包装
  checkpoint.go    ← 改造：加 Version 字段 + 版本兼容检查
  wal.go           ← 新增（可选）：预写日志模式
orchestrator/
  signal.go        ← 新增：SIGHUP/SIGTERM/SIGINT 处理
```

**对现有系统的影响：** 最小。`persist` 包的接口不变，只是内部实现升级。`orchestrator` 新增信号处理，不影响已有 workflow。

**收益/风险：** 收益极高——解锁 gateway、daemon、多版本并行三个方向。风险低——改动集中在 `persist` 包，依赖方少（主要被 `orchestrator` 调用）。

---

### 方向二：学习闭环的真实化（Learning Loop Completion）← P0

**为什么需要：** 当前 learning loop 产生了 trace（含 scorecard）和 memory，但中间的提炼管道是空的。`memory.Append` 仅 1 处调用意味着「学到的经验不进入下一轮」。这不是知识引擎的缺失，而是 learning loop 在「感知→提炼→应用」三段中只有第一段。

**核心挑战：**
- **最小可行提炼：** 输入文档建议的 `harvest.go`（失败 iteration 自动 Append lesson）是成本最低的信号增益。但更大的问题是消费侧：memory 检索结果当前只注入 prompt，而 prompt 的 token 预算有限。
- **反馈回路的度量：** 如何衡量「知识被应用了」？不是只看 Append 次数，而是看在后续 iteration 中 gate FAIL 率是否下降。

**预期的架构变更：**
```
internal/memory/
  harvest.go    ← 新增：from trace to lesson（20-50 行核心逻辑）
  retrieve.go   ← 增强：可为 lesson 类型加更高权重
internal/converge/
  evaluate.go   ← 增强：测量「知识影响 gate 通过率」
```

**对现有系统的影响：** 极低。`harvest.go` 是一个新的消费者，不改变已有接口。

**风险提示：** 不要掉入「知识图谱」或「经验库」的工程浪漫主义。当前阶段只需要一条管道：trace[iteration-1].scorecard.quality_gate == FAIL → Append lesson for iteration N。20 行代码，不要做成 2000 行的知识中台。

---

### 方向三：多仓治理（Multi-repo Governance）← P0，但实施风险最高

**为什么需要：** 输入文档的判断完全正确——「ForgeOS 如果只治理 forge-core 自己，本质上不是 OS 而是单项目脚手架」。`extends` 字段空值一天，架构债务就累积一天。

**核心挑战：**
- **中间状态管理：** 输入文档指出的「Phase A 未完成时最危险」是真实风险。`extends` 存在但不被解析是静默破坏。解决方案：Phase A 的 first commit 就更新 `manifest-integrity` 检查，使其在 `extends` 非空但 `.forgeos/` 不存在时直接 REJECTED。
- **双层覆盖语义：** 白名单 + 本地覆盖层的合并策略需要确定性优先级——本地文件优先于共享资产、共享资产优先于缺失。这个合并逻辑需要清晰文档化，且 `forge-init` 的输出需要在 `forge migrate` 上可复现。

**预期的架构变更：**
```
harness/
  acceptance.mjs              ← 更新：manifest-integrity 检查扩展
  check.py                    ← 更新：extends 解析校验
forge-core/internal/persist/  ← 无变化（治理层在 harness 侧）
```

**对现有系统的影响：** 主要集中在 `harness/` 治理层，forge-core 不受影响。但 `forge-init` 的模板复制逻辑需要改——从「按白名单复制」变为「解析 extends → 读共享资产 → 按覆盖层合并 → 落地」。

**风险点：** Phase A 到 Phase B 的过渡期。建议的阶梯式上线：
1. Phase A.0: `manifest-integrity` 检查扩展（即使有 extends 也能反）
2. Phase A.1: `forge-init` 的 extends 解析（单层共享，无覆盖）
3. Phase A.2: 覆盖层合并逻辑
4. Phase B: submodule 自动管理（`forge upgrade`）

---

### 方向四：策略驱动的预算治理（Policy-driven Budget Governance）← P1

**为什么需要：** 预算从「CLI flag（开发时设置）」提升为「policy 文件（签入到 `.agent/` 的治理资产）」——如输入文档所说，这是组织级信任所必需的。当前 `--max-budget-usd` 和 `--max-agent-calls` 是运行时的逃生舱口，不是策略。

**核心挑战：**
- **条件表达式的设计：** 输入文档建议避免图灵完备表达式引擎，将条件限制为 `task_type in [set]` 的枚举匹配。我同意这个判断——但需要补充：如果未来需要 `when: task_type == "security" AND model_usage > $10` 这样的复合条件，枚举匹配就不够了。**建议留一个扩展点**：当前用集合匹配，但 schema 允许未来升级为表达式（保持向后兼容）。
- **预算域的定义：** 是按 iteration 预算、按 phase 预算、按时间窗口预算（day/week/month）、还是按任务类型预算？四个维度需要正交组合。

**预期的架构变更：**
```
.agent/
  budget.yml    ← 新增：预算策略文件
forge-core/internal/
  budget/       ← 新包：策略解析 + 执行（enum 匹配模式）
```

**对现有系统的影响：** 中等。当前 budget guard 散落在 `CommandExecutor`（`--max-agent-calls`）和 `engine.go`（`--max-budget-usd`）。需要统合到新的 `budget` 包中。

---

### 方向五：可观测性（Observability）← P2

**为什么需要：** 虽然「没有这个依旧能跑」，但如输入文档指出的——「evolve 跑了 8 小时，撞 budget cap，中间哪个 gate 引起了 loop-back」这类问题在 debug 事故时是真实痛点。

**核心挑战：**
- **Span 父子关系的缺失：** 当前 `trace.jsonl` 是扁平 event 流，没有嵌套结构。iteration A → phase B → gate C → loop-back → iteration A' 的因果链不可追踪。解决这个问题需要引入一个轻量级的 trace ID + parent span ID 机制，不需要 OpenTelemetry 那样的重量级 agent。
- **`forge stats` CLI 的设计：** 输入文档建议的「折叠 iteration 摘要树」是正确的方向。但需要明确：这个 CLI 的输出格式是什么？JSON（给机器）还是树形（给人）？建议两者都支持（默认树形，`--json` 输出结构化数据）。

**预期的架构变更：**
```
internal/trace/
  span.go           ← 新增：span 父子关系
  stats.go          ← 新增：摘要树生成
cmd/forge/
  stats.go          ← 新增（或并入现有子命令）
```

**对现有系统的影响：** 低。`trace` 包已有 event 写入，增加 span 关系不影响现有消费者。

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则一：Gateway 接口应设计为「协议适配器」而非「事件路由中枢」。**

Gateway 的核心职责是接收外部事件 → 标准化 → 提交给 Orchestrator。不应该在 Gateway 中做业务路由（判断事件类型应该触发哪个 workflow）。正确的分层：

```
外部事件 (GitHub webhook / Slack command / CLI trigger)
    ↓
Gateway (协议适配 + 认证 + dedup)
    ↓ 标准化事件 (Event{ID, Type, Source, Payload, Timestamp})
Orchestrator (业务路由 + workflow 调度)
```

**原则二：`persist` 包的接口应增加版本合约。**

当前 `Checkpoint` 结构体没有任何版本信息。建议：

```go
type Checkpoint struct {
    Version int              // 新增：序列化版本号
    // ... 现有字段
}
```

保存时写当前版本号，加载时检查兼容性。版本号递增规则：
- 向后兼容的字段添加 → 版本号+1，旧版本加载器忽略新字段（gob 默认行为）
- 向后不兼容的变更 → 版本号+1，旧版本加载器显式拒绝

**原则三：Gate 接口应增加 `degraded` 信号。**

输入文档的 Edge Case 3 建议极为重要。当前 `run()` 返回 `{status: 'pass'|'fail'}`，无法表达「我跑了但数据不可靠」。建议的接口合约：

```
GateResult:
  status: 'pass' | 'fail' | 'error'
  degraded: bool          // true 表示依赖不可达，数据可能不完整
  reason: string          // 人可读的原因
```

`acceptance.mjs` 聚合时：
- `status === 'error' && degraded === true` → warning，不阻断
- `status === 'error' && degraded === false` → 真正故障，阻断

### 3.2 是否需要新的抽象层

**需要引入的抽象层：**

1. **`internal/budget/` 包**：当前 budget 逻辑散布在多个文件中。新包集中管理策略解析、配额检查、熔断逻辑。接口：
   - `ParsePolicy(path string) (*BudgetPolicy, error)` — 读 `.agent/budget.yml`
   - `Check(phase *Phase, policy *BudgetPolicy) (*BudgetStatus, error)` — 检查当前 phase 是否在预算内
   - `Record(phase *Phase, cost *Cost) error` — 记录实际消耗

2. **`internal/persist/lock.go` 锁抽象**：不是重新发明锁，而是将 `flock` 系统调用包装为 `persist` 包内可见的接口：
   - `LockFile(path string) (func() error, error)` — 返回解锁函数

**不需要引入的抽象层：**

1. **事件总线抽象**：在 gateway 尚未引入前，事件总线的抽象是镀金。当前 `internal/trace` 的 event 流模式就是未来的事件模式的原型，不要提前发明一个「通用事件系统」。
2. **策略引擎抽象**：输入文档建议的 enum 匹配模式是正确的。不要引入 OPA/Rego。当前阶段的预算策略不需要图灵完备表达式。

### 3.3 向后兼容性策略

| 变更类型 | 策略 |
|---|---|
| 新增 `Checkpoint.Version` | 旧文件 Version=0，代码识别 0 为旧格式，兼容处理 |
| 新增 Gate 接口的 `degraded` 字段 | 旧 gate 不返回该字段，`acceptance.mjs` 视缺失为 `degraded=false` |
| 新增 `budget.yml` 文件 | 文件缺失视为无策略，向后兼容，不改变现有行为 |
| `trace.jsonl` 新增 span ID 字段 | 旧 trace 无 span ID，新代码读取时视作根 span |

---

## 4. 技术选型

### 4.1 不需要引入的新技术栈

| 候选技术 | 决策 | 理由 |
|---|---|---|
| OPA/Rego | ❌ 不引入 | 当前 enum 匹配已够；策略数量 < 10 条时 OPA 的开销超过收益 |
| OpenTelemetry SDK | ❌ 不引入 | 当前 `/metrics` 端点 + JSONL trace + Prometheus textfile collector 三件套足够 |
| 向量数据库 (Qdrant) | ❌ 不引入 | TF-IDF 在 < 1000 条目时够用；引入新存储是镀金 |
| gRPC | ❌ 不引入 | 当前单进程架构下 gRPC 引入序列化/网络/服务发现的全栈复杂度 |
| LiteLLM | ❌ 暂缓 | 跨厂商池 = v3 路线图 |

### 4.2 需要引入的最小技术增量

| 技术 | 为何需要 | 替代方案 | 决策 |
|---|---|---|---|
| Go YAML 库 | 消除 python shim 运行时依赖 | 继续用 python shim | ⏳ 暂缓（属 architect/cto 的依赖决策） |
| `golang.org/x/sys` 的 `flock` | checkpoint 并发安全 | 自建 syscall 包装 | ✅ 可引入（受控的 stdlib 扩展） |
| LRU cache 实现 | gateway dedup 的事件 ID 窗口 | 自建 map+time 窗口 | ✅ 标准库即可（`sync.Map` + 时间戳） |

### 4.3 自建 vs 采购决策

对于 gateway 的 webhook 接收，存在一个「写一个 HTTP handler vs 用现存框架」的决策：

- **自建 Go HTTP handler（推荐）：** 20-30 行标准库代码，零依赖。ForgeOS 的 gateway 不需要路由框架（编排在 orchestrator 侧，gateway 只是协议适配器）。
- **引入 Gin/Chi/Echo：** 增加依赖但获得路由/中间件/参数解析。**不推荐**——当前 forge-core 零外部依赖是重要资产，不应为一个 HTTP handler 打破。

---

## 5. 实施路线图

### 优先级排序

| 优先级 | 方向 | 理由 |
|---|---|---|
| **P0** | 多仓治理（方向三） | 外部队列最长，`extends` 空置一天债务一天 |
| **P0** | 运行时弹性（方向一·子集） | 仅需 `flock.go` + `Checkpoint.Version`，低成本高收益 |
| **P1** | 学习闭环（方向二·`harvest.go` MVL） | ~20 行代码，信号增益极高 |
| **P1** | 预算治理（方向四·enum 模式） | 组织级信任的前提，但需要运行时弹性先到位 |
| **P2** | Gateway（方向一·主体） | 依赖运行时弹性 + 需要用户授权 daemon 模式 |
| **P2** | 可观测性（方向五） | 高价值但非阻塞 |

### 阶段划分和里程碑

```
Phase A — 「地基加固」（P0）
  里程碑：checkpoint 并发安全 + extends 解析器就绪 + manifest-integrity 扩展
  关键交付：
    internal/persist/flock.go          ← 并发安全 checkpoint
    internal/persist/checkpoint.go     ← Version + 兼容检查
    harness/acceptance.mjs             ← manifest-integrity 扩展
    harness/forge-init.mjs             ← extends 解析（Phase A.1，单层共享）
  风险：forge-init 的解析器改造涉及两条路径（forge-init + forge-upgrade）
  缓解：先改 manifest-integrity（即使解析器没改完，也能检测到未解析的 extends）

Phase B — 「信号加固」（P1）
  里程碑：learning loop 第一条管道 + 预算策略文件就绪
  关键交付：
    internal/memory/harvest.go         ← trace FAIL → lesson
    internal/budget/                   ← 新包：策略解析 + 检查
    .agent/budget.yml                  ← 默认策略文件
  风险：budget 包的引入需要与现有 budget 逻辑（CommandExecutor/engine.go）合并
  缓解：先做 harvest.go（20 行，独立不依赖 budget 包）

Phase C — 「协议扩展」（P2）
  里程碑：gateway webhook endpoint + 可观测性
  关键交付：
    internal/gateway/                    ← 新包：HTTP handler + dedup
    internal/trace/span.go              ← span 父子关系
    cmd/forge/stats.go                  ← 摘要树 CLI
  风险：gateway 引入 HTTP listener，突破「零网络服务」的架构边界
  缓解：先做 stats CLI（不影响现有架构），gateway 作为独立二进制或子命令启动
```

### 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| Phase A 中间状态：`extends` 存在但不解析 | 高 | 静默破坏 | Phase A first commit 就更新 manifest-integrity 检查 |
| Gateway 的 webhook dedup 防线被绕过 | 中 | 多版本冲突 | LRU set + 事件 ID 去重窗口 |
| checkpoint 向后兼容忘了加 Version | 中 | 旧 gob 被新代码覆盖 | 代码审查的 checklist 项 |
| budget 策略变成了图灵完备的表达式引擎 | 低 | 过度工程 | 输入文档的 enum 匹配约束强制定期复审 |
| 多仓治理的共享资产版本冲突 | 低 | 被治理项目行为不一致 | `forge-upgrade` 的版本检查 + manifest-integrity |

---

## 补充：关于输入文档的回应

输入文档的整体分析质量极高，以下是几处需要回应的观点：

**关于方向一的「daemon 与 gateway 正交」**：完全同意。两个维度的依赖关系是：webhook endpoint（被动接收，无状态）先做，daemon（主动常驻，需要 signal handler + checkpoint 升级）后做。**建议在 roadmap 中明确标注这个依赖顺序**——当前没有文档记录这条解耦关系。

**关于 `harvest.go` 的最小可行方案**：赞同。20 行代码的 `harvest.go` 比一个完整的「知识引擎」更有价值。但需要补充一条消费侧的考虑：memory retrieval 的注入不能盲目 Append。当前 `prompt_context.go` 的 `buildPrompt` 已经有 token 预算约束（硬注入的 constraints + retrieved memory）。lesson 类型的知识在 prompt 中的权重需要高于一般 memory，但低于当前硬约束。

**关于 Edge Case 3 的 degrade 契约**：这是输入文档最重要的跨领域建议。当前 gate 的 `run()` 接口没有 `degraded` 信号，这意味着所有 gate 都是「黑了也 PASS（或 FAIL）」。引入 `degraded` 信号后，`acceptance.mjs` 的聚合逻辑需要相应调整。**建议将此列为 Phase A 的一部分**——它不是镀金，而是「诚实策略」的自然延伸。

**关于 Edge Case 5 的 gob 兼容性**：`Version int` 字段的引入是成本极低的 guard。建议在 Phase A 的首个 commit 就加入——不管先做哪个方向，checkpoint 的跨版本兼容性都是一个需要先解决的基础设施问题。
