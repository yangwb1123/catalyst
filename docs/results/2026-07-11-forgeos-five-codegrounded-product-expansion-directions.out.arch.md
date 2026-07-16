现在我掌握了充分的上下文。以下是完整的架构分析报告：

---

# ForgeOS 架构分析报告

## 1. 架构评估

### 1.1 当前架构的优势

**1. 架构完整性极高**

当前 ForgeOS v2（围绕 `forge-core` Go 运行时）已经完成了一个罕见的成就：一个**自洽、可验证、零外部依赖**的编排核心。它不是一个"朝不保夕"的快速原型——13 个 Go 包、零 `go.mod require`、8 项 arch-check 全绿、Sprint 1–31 已被 `forge accept` 锁定。架构骨架（中枢旋钮 mode×lifecycle、收敛引擎、Harness 闸门、Model Router）全都**真实运行且真实被测试**。

**2. "诚实第一"的文化已嵌入架构**

`forge-core` 中处处可见**无伪造**设计契约：
- 未知 metric → NOT MET（不以 PASS 伪造）
- 工具缺失 → N/A（不以 FAIL 或 PASS 伪造）
- trace 无数据 → omit 而非零
- YAML shim 缺失 → 错误信息清晰而非静默失败
- `stop_condition.type: external` 清晰约定退出条件而非假装收敛

这在 AI-native 系统中是一种罕见的架构纪律，也是未来信任模型的**地基**。

**3. 中枢旋钮驱动力强**

`mode×lifecycle` 一个设置驱动 Router 档位 + Harness 严格度 + Workflow 深度 + Migration——这种**单一旋转轴**设计避免了配置面膨胀为 `n 维笛卡尔积 {feature flags × thresholds × overrides × exceptions}` 的常见退化。当前的 fail-safe 模式（未知输入→全开，production→强制 block）是简单而安全的默认行为。

**4. 运行时-治理严格分离**

架构哲学正确：ForgeOS 是"控制平面 + 治理平面"，不是"在我们框架里写代码"。这意味着：
- Harness 闸门对所有项目 host-independent
- Agent 执行器是可替换的（dry-run / echo / claude）
- 模型路由是独立决策服务（不绑定到特定模型厂商）

### 1.2 当前架构的局限性

**1. 单仓库治理天花板（架构最严重的结构性限制）**

这是当前架构中最根本的局限性。`Engine.root` 指向单仓库根目录。`RepoRoot()` 返回单一路径。`arch-check` 扫描单仓库导入图。跨仓库依赖在代码库中**零存在**。

这不仅仅是"缺少一个特性"——它是一个**架构假设**，深嵌在 `forge-core/orchestrator`、`internal/risk`、`harness/arch` 等多个核心包中。要支持多仓库治理，不是"加一行配置"，而是需要引入新的抽象层（Dependency Graph Service、Cross-Repo Contract Registry）并重构 `Engine` 的作用域概念。

**2. 收敛信号全部基于 agent 自我声明**

当前 `converge.Signals` 的信号源几乎全部依赖**agent 自报**：
- `RoadmapCompletion`：agent 自己勾 `[x]`
- `ReviewStatus`：agent 自己写 `VERDICT: APPROVE`
- `RequirementConfidence`：agent 自己报 `CONFIDENCE: 85`

唯一机械计算的信号 `FileDelta`（git-diff 路径匹配）也已承认是"廉价代理启发式"。

这意味着整个"收敛"系统建立在**自我报告**的基础上——类似于学生给自己打分。Harness 闸门能拦截机械违规（行数、循环依赖、测试失败），但**不能验证语义正确性**（"代码是否满足 PRD 的需求？"）。这是 AI 自治系统中最根本的信任缺口。

**3. 平面事件模型不可诊断因果链**

`trace.Event` 是平坦列表。没有 span 树、没有 trace ID、没有 parent-child 关系。在 24h 无人值守的运行中，gate FAIL 无法被自动**归因**到引入它的 phase/iteration。`forge doctor` 能诊断当前状态的症状，但不能回答"这是什么时候开始的？"。

随着项目运行时间增长，平面 trace 的诊断效率急剧下降——operator 需要手动关联数百个 phase 事件。

**4. Memory 是 append-only 且无生命周期管理**

`memory.jsonl` 只增不删。`Load` 读取整个文件。Compact 只按最近 N 条做保留。TF-IDF 检索在**所有**条目上评分，包括过时的知识。

这意味着：
- 200 次 evolve 迭代后，每次 `Load` 是 O(全历史)
- 过时的知识（"我们在考虑 Redis"）仍然被注入到 agent context 中，与当前相关的知识无区别对待
- `.forge/` 目录线性增长

**5. 失败模式只有硬 abort**

loop-back 耗尽 → 硬 abort。NoProgress tripwire 触发 → 硬 abort。Human gate 拒绝 → 硬 abort。没有优雅降级路径（"换一个更贵的 agent 试试"、"标记为需人工处理并继续"、"以降级模式继续运行"）。

对于一个自称"24h 无人值守"的系统，失败模式的单一性是一个严重缺口。

---

### 1.3 关键设计决策评估

| 决策 | 评估 | 理由 |
|---|---|---|
| **零外部 Go 依赖** | ✅ 正确 | 对于编排核心是正确的——它应该是"可审计的确定性内核"。但 YAML shim 是一个合理的技术债 |
| **模式 pivot 在前门** | ✅ 正确 | `mode×lifecycle` 是用于所有子系统的单一决策点。避免 feature flag 膨胀 |
| **Harness 作为 host-independent 执法层** | ✅ 正确 | 这是长期最高杠杆的决策——它让 ForgeOS 不受宿主 CLI 的能力限制 |
| **stdout/stderr 的 cappedBuffer 10MiB** | ✅ 正确 | 资源上限是 v2 必须的护栏。但 10MiB 上限对于真 agent 输出可能过于保守 |
| **Go 运行时 + Node harness + Python check** | ⚠️ 合理但有成本 | 多语言运行时是 pragmatism（Node/Python 工具链丰富），但也意味着 3 套工具链和 3 种 deployment artifact。未来考虑全部编译为 Go 静态二进制 |
| **YAML 经 Python shim 转码** | ⚠️ 明确的技术债 | 已承认是暂时性的，项目也知道这是债务 |
| **收敛 = 计算而非声明** | ✅ 正确 | MET/NOT-MET 基于实时信号计算而非 agent 声称 |
| **平面 trace 事件模型** | ❌ 短期正确但长期不可持续 | 当前简单，但 24h 运行后根本不够用 |

### 1.4 架构债务清单

| 债务项 | 位置 | 严重度 | 建议干预时间 |
|---|---|---|---|
| YAML shim | `harness/yaml2json.py` | 中 | 下一次 Go 依赖决策窗口 |
| Memory append-only | `internal/memory/memory.go` | 中-高 | 项目运行达 100 次 evolve 前 |
| 平面 trace | `internal/trace/trace.go` | 中 | 首个多日无人值守运行前 |
| cmd/forge 包文件数紧贴上限(17/17) | `cmd/forge/` | 低 | 下一次自然增长时 |
| agent 阶段默认 dry-run | `cmd/forge/` | 低（设计的） | 产品化时转为默认真实执行 |

---

## 2. 扩展方向

基于验证文档中确认的 5 个方向 + 我的独立评估，以下是按优先级排列的高价值架构扩展方向：

---

### 方向一：语义输出验证层（Semantic Output Validation）

**为什么需要（业务价值/技术价值）**

当前 ForgeOS 的整个信任模型建立在一个脆弱的假设上："gate 全绿 + ROADMAP 全勾 = 实现正确"。但在 AI 自治开发的语境下，这是结构性盲区：
- **自我验证循环**：implementer 写代码，也写测试。测试会通过——因为 implementer 可以也必须让测试通过它自己的代码。这不是 malice，是机制。
- **需求回溯缺失**：PRD 中一条需求"密码必须 bcrypt 加密"，没有任何机械验证能让它连接到对应的代码实现。
- **跨 agent 验证中断**：reviewer 拿到的是**已经绿了 gate 的代码**。reviewer 在 prose 中能发现逻辑问题，但无法验证"所有 PRD 需求都已被覆盖"。

没有语义验证层，ForgeOS 永远不能在关键业务场景（金融、医疗、合规）中获得信任。

**核心挑战和技术难点**

1. **需求到实现的映射本身是开放问题**——即使对人类来说，"PRD 的第三条是否已被完全实现"也是代码审查的核心挑战。对于机器来说更难。
2. **声明的格式和粒度**——太粗（"实现认证系统"）无法验证，太细（"在第 42 行调用 bcrypt"）过度约束 implementer。
3. **AI-native 的验证粒度**——传统 traceability 需要形式化需求规格，在快速迭代的产品开发中太重型。需要找适合 AI 开发节奏的轻量级验证模式。

**预期的架构变更**

```
forge-core/internal/
  ├── converge/
  │   ├── converge.go         ← 新增 RequirementTrace 信号
  │   └── traceability.go     ← NEW: 需求-实现追溯矩阵
  ├── gate/
  │   └── semantic.go         ← NEW: 声明式不变式引擎
  └── invariant/              ← NEW: invariant 声明解析和执法
      ├── invariant.go        
      └── checker.go          
harness/
  └── invariants/             ← NEW: 每个语言的 invariant 检查器
      ├── check_invariant.mjs
      └── adapters/go.yml     ← 新增 invariant 工具
.agent/
  └── invariants/             ← NEW: 项目级不变式声明
      ├── security.yml
      └── data.yml
```

**对现有系统的影响**

- `converge.Signals` 需要新增字段但向后兼容
- Harness 闸门新增 family（invariant check），不改变现有 gate 行为
- agent 卡的 `emits:` 声明需扩展以包含"需求条目"，但现有 workflow 不受影响
- **新增的 `invariant/` 包不依赖于任何现有包的内部结构**——纯新代码

**实施选项与权衡**

| 选项 | 优点 | 缺点 |
|---|---|---|
| A. 轻量级：新增 `RequirementTrace` 信号 + 路径匹配 | 快速交付，在现有框架内工作 | 仍然是启发式，不是形式化验证 |
| B. 声明式不变式引擎 + 语言独立适配器 | 可验证、框架级；可扩展为所有语言 | 需要为每种语言编写 adapter；实现成本高 |
| C. 验收测试独立生成 + MD5 校验 | 语言无关，利用了既有测试基础设施 | 只能验证"实现通过测试"，不能验证"需求被覆盖" |

**建议**：走 B 路径（声明式不变式），但阶段划分：
- Phase 1：新增 `RequirementTrace` + 路径映射（两周交付）
- Phase 2：不变式引擎 + Go 适配器（三周）
- Phase 3：扩展到 polyglot（每个语言一周，按需求优先级）

---

### 方向二：多仓库依赖图治理（Multi-Repo Dependency Governance）

**为什么需要**

ForgeOS 自称"元框架/软件工厂"，但当前治理边界 = 单 Git 仓库。真实世界的产品由多个仓库组成——共享库、微服务、客户端、配置文件。一个共享库的变更可以打崩所有下游仓库的构建。ForgeOS 当前无法回答的基本问题：
- "这个 shared-lib 的 change 会 break 哪些下游 repo 的 test？"
- "需要跨 3 个 repo 协同发布一个 feature，如何编排？"
- "repo A 的 architect 设计了一个接口，repo B 的 implementer 如何自动得到通知？"

不突破单仓库天花板，ForgeOS 的"软件工厂"叙事无法闭环。

**核心挑战和技术难点**

1. **依赖图是分布式**——git submodule 是最直接的机制，但 ADR 0003 已指出 submodule 有"在 CI 中需要注册 token"等问题。跨仓库编排无论选择什么机制都会引入新的基础设施需求。
2. **跨仓库执行非原子**——repo A 变更触发 repo B 的构建，repo B 构建时 repo A 可能继续演进。需要版本锁定（参考 CRDT / 乐观并发控制思路）。
3. **工作目录管理的复杂性**——`Engine.root` 是单一字段。多仓库需要"仓库组"概念，每个仓库有自己的 `.agent/` + `.forge/` + root。

**预期的架构变更**

```
forge-core/internal/
  ├── orchestrator/
  │   └── orchestrator.go    ← Engine.root → Engine.roots ([]string)
  ├── dependency/            ← NEW: 依赖图管理和信号传播
  │   ├── graph.go           ← 依赖图数据结构
  │   ├── registry.go        ← 解析 .agent/dependencies.yml
  │   └── propagation.go     ← 跨仓库变更信号传播
  └── gate/
      └── cross_repo.go      ← NEW: 跨仓库 gate shim
.agent/
  └── dependencies.yml       ← NEW: 项目级依赖声明
harness/
  └── adapters/
      └── cross_repo_test.mjs ← NEW: 下游测试触发器
```

**对现有系统的影响**

- `Engine` 的 `root` 字段改为 `Roots []string`，对单仓库用户完全向后兼容（`Roots[0]` 行为不变）
- 依赖图解析器是新增的独立包
- Harness 闸门新增 cross-repo test runner，不影响现有单仓库 gate 流程
- ADR 0003 的 submodule 方案与多仓库治理是互补关系

**实施选项与权衡**

| 选项 | 优点 | 缺点 |
|---|---|---|
| A. 轻量级：`dependencies.yml` + 静态依赖图 | 快速交付，无运行时基础设施 | 依赖图是静态快照，不能响应实时变更 |
| B. 完整编排：`forge orchestrate` 跨仓库 run | 真正多仓库"软件工厂" | 需要多仓库状态协调，实现复杂度高 |
| C. git submodule 扩展 | 复用 ADR 0003 决策，利用 git 的成熟机制 | 子模块的 CI token 问题、多仓库原子性问题仍待解决 |

**建议**：走 A → B 路径
- Phase 1：`dependencies.yml` + `forge detect` 跨仓库变更信号（两周）
- Phase 2：跨仓库 test trigger + 聚合 gate（三周）
- Phase 3：版本协调 + 协同发布（四周）——建议延迟到有真实多仓库用户后再做

---

### 方向三：Agent 故障升级协议与优雅降级（Agent Escalation Protocol）

**为什么需要**

当前失败模式是二元的：要么重试，要么硬 abort。但 24h 无人值守必须处理真实世界中 N 种"既不是成功也不是崩溃"的中间状态：
- agent 反复尝试但做不出来（"我不会 Rust lifetimes"）→ 换人/降级
- reviewer 和 implementer 陷入审美死循环 → 换人
- gate N 次不绿但明确原因（"coverage 阈值需要人工配置"）→ 降级运行 + 标记
- budget 烧穿 → checkpoint-on-abort 而非硬 crash

**核心挑战和技术难点**

1. **区分"能力不足"和"任务难度"**——agent 说"不会"是 `KindInexpert`，但 agent 可能错误地认为自己能行（过度自信）或不能行（误判）。
2. **降级路由的预算控制**——escalation 总是升 tier（从便宜到贵），如果每次失败都升 tier，成本可以轻易膨胀。需要预算护栏。
3. **部分通过的语义**——`forge accept --allow-exceptions` 引入了一个新概念："可以接受部分 gate 不绿"。这个"例外"必须可审计、不被 agent 利用。

**预期的架构变更**

```
forge-core/internal/
  ├── orchestrator/
  │   ├── exec_error.go       ← 新增 KindInexpert
  │   ├── escalation.go      ← NEW: 升级协议状态机
  │   └── loop.go             ← LoopEngine 新增升级路径
  ├── routing/
  │   └── routing.go          ← TierFor 新增 escalation path
  └── converge/
      └── partial.go         ← NEW: 部分通过收敛信号
```

**对现有系统的影响**

- `ExecError` 新增 `KindInexpert`，不改变既有错误处理逻辑（向后兼容）
- `TierFor` 新增 escalation path，不影响正常路由
- `LoopEngine` 的 loop-back 逻辑新增降级分支选项
- 现有 workflow 默认不启用升级协议（opt-in）

---

### 方向四：知识生命周期管理（Knowledge Lifecycle Management）

**为什么需要**

当前 memory 是纯 append-only 的日志。项目运行 200 次 evolve 迭代后：
- `Load` 每次 O(n)，n 线性增长
- 过时知识（"我们在考虑用 Redis"）与当前知识（"我们最终选择了 PostgreSQL"）在 TF-IDF 检索中无区别
- 衰减权重只被 scorecard 使用，memory 条目无时间衰减
- `.forge/` 目录可轻易超过 50MB+

如果不解决，这是系统**随使用时间反向退化**的架构债务——越老越慢，越老越误导。

**核心挑战和技术难点**

1. **"过时"的判定**——简单按时间 TTL 衰减会删除仍在引用的历史决策。需要引用计数（"这个知识被加载了多少次？"）作为 TTL 的补充信号。
2. **无 LLM 语义摘要**——项目纪律是"纯 Go 标准库"，所以 Compact 的 summarization 必须不依赖 LLM。基于实体提取 + 决策树归纳是可以做的，但精度不如 LLM。
3. **分层 memory 的迁移**——现有 memory 是单文件 JSONL。引入 ephemeral/persistent 分层意味着需要迁移路径。

**预期的架构变更**

```
forge-core/internal/
  ├── memory/
  │   ├── memory.go           ← Load → lazy loading
  │   ├── memory_compact.go   ← 新增 Summarize 分支
  │   ├── memory_ttl.go      ← NEW: TTL 衰减
  │   ├── memory_index.go    ← NEW: 倒排索引
  │   └── memory_layers.go   ← NEW: ephemeral/persistent 分层
  ├── trace/
  │   └── trace.go            ← 新增日志轮转
  └── persist/
      └── checkpoint.go       ← 增量 snapshot（delta-based）
```

**对现有系统的影响**

- `memory.Entry` 新增 `TtlDays uint32` 字段（默认 0=永久），JSONL 向后兼容
- 新增的 index/layer/ttl 都是向后兼容的增强
- 唯一可能破坏兼容的是 `Compact` 的行为变化——需要在 mode 中加入"是否启用 summarization"的配置项

---

### 方向五：可观测性因果追踪与根因分析（Observability Causal Tracing & RCA）

**为什么需要**

当前 trace 是平面事件流。在 24h 运行中：
- gate FAIL → 需要 operator 手动翻阅数百行日志找到是谁引入的
- reviewer APPROVE 但 production gate FAIL → reviewer 是否 miss 了问题？
- budget 烧穿 → 是哪个 phase 最贵？
- `forge doctor` 能诊断当前症状，不能回答"从什么时候开始"？

这是从"能跑"到"能运维"的关键差距。

**核心挑战和技术难点**

1. **跟踪开销**——每个 trace 事件添加 `trace_id` / `parent_span_id` 对现有 trace 模型是向后兼容的，但会增加 log 大小 ~20%。
2. **因果关系的精确性**——gate FAIL 的上游是哪些 phase？这不简单是"前一个 phase"——gate 可能因为实现、测试、配置、依赖关系等多个因素 fail。需要从事件日志推断因果图。
3. **存储/查询**——trace 文件 O(n) 随时间增长。添加结构化查询 DSL 意味着需要某种索引而非纯线性扫描。

**预期的架构变更**

```
forge-core/internal/
  ├── trace/
  │   ├── trace.go            ← Event 新增 trace_id / parent_span_id
  │   ├── trace_tree.go      ← NEW: span 树构建
  │   └── trace_query.go     ← NEW: 结构化查询 DSL
  ├── orchestrator/
  │   ├── orchestrator.go     ← phase-level 注入 span_id
  │   └── loop.go             ← iteration-level 根 span
  └── blame/                 ← NEW: 差异归因
      ├── blame.go
      └── blame_test.go
cmd/forge/
  ├── diagnose.go            ← NEW: forge diagnose 命令
  ├── blame.go               ← NEW: forge blame 命令
  └── timeline.go            ← NEW: forge timeline 命令
```

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则一：新模块的接口必须向前兼容**

ForgeOS 已经有用户（至少是 `examples/url-shortener` + `forge-init` 脚手架）。任何接口变更必须：
- 新增字段具有安全默认值（0 → 禁用，空 → 不参与）
- 现有文件不需修改即可在新版本下工作
- 删除/重命名必须先 deprecate 再移除（当前仓库尚无 deprecation 机制，建议新增）

**原则二：治理接口与执行接口分离**

当前 `converge.Signals` 同时混合了两种角色：
- 治理信号：`RoadmapCompletion`、`GatesGreen`——驱动收敛决策
- 监控信号：`FileDelta`、`CodeTestRatio`——辅助 human 判断

建议分离为两个接口：
```go
// GovernanceSignal — 驱动收敛决策，agent 不能绕过
type GovernanceSignal struct {
    RoadmapCompletion float64
    GatesStatus       string    // "green" / "red"
    HumanApproved     bool
}

// ObservabilitySignal — 辅助人类判断，不驱动收敛
type ObservabilitySignal struct {
    FileDelta        float64
    CodeTestRatio    float64
    ReviewerAccuracy float64    // NEW
    RequirementTrace []RequirementLink  // NEW
}
```

**原则三：声明式配置先于编程式 API**

ForgeOS 现有的 `.agent/` 目录模式（YAML 声明 + 机读契约）已经被证明有效。新方向应尽可能遵循同一模式：
- `.agent/invariants/` → 声明的不可变规则
- `.agent/dependencies.yml` → 多仓库依赖图
- 避免引入新的 DSL 或编程式配置

### 3.2 是否需要引入新的抽象层

| 新抽象层 | 建议 | 理由 |
|---|---|---|
| **Dependency Graph Service** | ✅ 引入 | 多仓库治理需要独立的依赖图管理，不应嵌入 `orchestrator.Engine` |
| **Invariant Checker** | ✅ 引入 | 语义验证需要独立的检查器注册和调度机制 |
| **Escalation State Machine** | ✅ 引入 | 升级协议是复杂状态机，不应嵌入 `loop.go` |
| **Trace Span Model** | ⚠️ 扩展而非新层 | 在现有 `trace.Event` 基础上添加 span ID，不引入新包 |
| **Memory Index** | ⚠️ 扩展而非新层 | 在 `internal/memory` 包内新增 index 子包 |

### 3.3 向后兼容策略

```
版本策略:
  v2.current → v2.1 (add trace_id, TTL fields — backward compatible)
           → v2.2 (add invariant/ package — new feature, no existing change)
           → v2.3 (add dependency/ package)
           → v3.0 (if multi-repo Engine break backward compat)
```

每个新方向通过**可选开关**引入：
- YAML 中 `dependencies.yml` 缺失 → 单仓库模式（向后兼容）
- 无 `invariants/` 目录 → 不变式不启用
- trace 事件无 span ID → 平面模型回退
- 无 escalation config → 二进制 abort 模式

---

## 4. 技术选型

### 4.1 当前技术栈评估

| 组件 | 当前 | 评估 | 建议 |
|---|---|---|---|
| 编排核心 | Go 零依赖 | ✅ 正确的选择 | 维持 |
| 模型路由 | Go 内置 | ✅ 正确的选择 | 维持 |
| 收敛引擎 | Go 内置 | ✅ 正确的选择 | 维持 |
| YAML 解析 | Python shim | ❌ 技术债 | 下一周期解决 |
| Harness 闸门 | Node.js | ⚠️ 实用主义 | 维持，但保持 adapter 模式可替换 |
| 治理检查 | Python | ⚠️ 实用主义 | 维持，但监控是否增长过大 |
| Trace 存储 | JSONL 文件 | ✅ 当前正确 | 维持，添加轮转 |
| Memory 存储 | JSONL 文件 | ⚠️ 可接受 | 等 TTL/分层就绪后升级 |

### 4.2 新方向的技术选型

**YAML 解析**（影响方向一、二、四的配置声明）

| 选项 | 优点 | 缺点 | 成本 |
|---|---|---|---|
| A. 继续 Python shim | 零改动 | 技术债继续积累 | 0 |
| B. `gopkg.in/yaml.v3` | Go 原生的 YAML 解析 | 打破零依赖纪律 | ~3 天适配 |
| C. 自研手写 YAML 解析器 | 维持零依赖 | 成本高、风险大 | ~3 周 |
| D. yaml2json 从 Python 改为 Go（现有 `internal/yaml2json` 已启动此迁移） | 已经部分实现在 `internal/yaml2json/`，已通过 PyYAML 逐位吻合测试 | `yaml2json` 只做 YAML→JSON 转换，不直接解析 YAML 为 Go struct | 继续完善当前路径 |

**建议**：继续 D —— `internal/yaml2json` 包已经作为 Python shim 的 Go 替代品在开发中，Sprint 27 已修复 block-scalar bug 并通过 7/7 真实文件 vs PyYAML 逐位吻合测试。这条路径最符合项目「纯 Go 标准库、零外部依赖」的既有纪律，且当前投资已经启动。

**跨仓库通信**（影响方向二）

| 选项 | 优点 | 缺点 |
|---|---|---|
| A. git submodule | 成熟的 git 机制 | CI token 问题；多仓库原子性 |
| B. 文件系统链接（symlink） | 零基础设施 | 跨机器不工作 |
| C. HTTP event hooks | 真分布式 | 需要注册表服务 |
| D. 在现有 `Engine.roots` 扩展基础上通过本地文件系统共享契约 | 无基础设施；符合 ADR 0003 的双层覆盖模式 | 只适用于同机器的 co-located repos |

**建议**：Phase 1 选 D（同机器本地共享），Phase 2 根据真实用户需求评估是否升级到 A 或 C。

**不变式引擎**（影响方向一）

| 选项 | 优点 | 缺点 |
|---|---|---|
| A. 纯 shell 脚本 + grep | 零成本 | 表达力弱；不可扩展 |
| B. Go 内置 checker | 零依赖；与现有 gate 模式一致 | 每个 checker 需要手写 |
| C. OPA/Rego 集成 | 成熟策略引擎 | 引入外部依赖；过重 |
| D. 项目已有模式——`harness/adapters/{go,py,ts}.yml` 适配器框架扩展为 invariant 检查 | 复用现有模式；align 既有 `probeLint`/`probeCoverage` 等适配器模板 | 需要为每个 invariant 写 adapter，但框架已有 |

**建议**：走 D 路径——扩展现有的 adapter 框架以支持 invariant 检查。`probeInvariant` 镜像 `probeLint` 模式（探测→运行→结果），这是最符合项目「复用现有 harness 模式」纪律的选项。

### 4.3 自建 vs 采购

| 组件 | 建议 | 理由 |
|---|---|---|
| 编排核心 | 自建 | 核心差异化能力；简单（单仓库 500 行状态机） |
| 依赖图注册 | 自建 | 领域特定（软件工程） |
| 不变式引擎 | 自建 | 可以很轻（wrapper around shell tools） |
| 升级状态机 | 自建 | 领域特定；不可能采购 |
| Trace 存储 | 扩展现有 JSONL | 当前够用；OTel Collector 是以后的选项 |
| 根因分析 | 自建 | 领域特定（agent phase 因果关系） |

除非项目转向 Serverless 或 SaaS，否则不建议采购任何新组件。

---

## 5. 实施路线图

### 5.1 优先级排序

| 层级 | 方向 | 业务价值 | 技术风险 | 交付周期(人周) | 依赖 |
|---|---|---|---|---|---|
| **P0** | 语义输出验证 | 最高——信任模型缺口 | 中 | 6-8 周 | 无（完全正交） |
| **P0** | 多仓库依赖图治理 | 高——架构天花板 | 高 | 8-10 周 | 需要 ADR 0003 决策 |
| **P1** | 故障升级协议 | 中高——韧性 | 中 | 4-6 周 | 方向二部分设计（共享状态机概念） |
| **P1** | 可观测性因果追踪 | 中——运维瓶颈 | 低中 | 4-6 周 | 无（完全正交于现有 trace） |
| **P2** | 知识生命周期管理 | 中——长期退化 | 中高 | 6-8 周 | 需要先观察到实际退化（用户报告） |

### 5.2 阶段划分

```
Phase 1（P0 方向：信任 + 治理域）
├── 语义输出验证 v1（4 周）
│   ├── 第 1-2 周：RequirementTrace 信号 + 路径映射
│   ├── 第 3 周：声明式不变式引擎 + Go 适配器
│   └── 第 4 周：与 forge accept 集成 + 验收测试
├── 多仓库依赖图治理 v1（4-6 周）
│   ├── 第 1 周：dependencies.yml 格式设计
│   ├── 第 2-3 周：Engine.roots + 依赖图解析
│   └── 第 4-6 周：跨仓库 test trigger + 聚合 gate

Phase 2（P1 方向：韧性 + 可运维性）
├── Agent 故障升级协议（4 周）
│   ├── 第 1 周：ExecError 扩展 + KindInexpert
│   ├── 第 2-3 周：降级状态机 + 路由扩展
│   └── 第 4 周：checkpoint-on-abort + 配置
├── 可观测性因果追踪（4 周）
│   ├── 第 1 周：trace_id / parent_span_id
│   ├── 第 2 周：forge diagnose 命令
│   └── 第 3-4 周：forge blame + timeline

Phase 3（P2 方向：长期健康）
└── 知识生命周期管理（6 周）
    ├── 第 1-2 周：TTL 衰减 + 内存索引
    ├── 第 3-4 周：ephemeral/persistent 分层
    └── 第 5-6 周：trace 日志轮转 + 增量 checkpoint
```

### 5.3 关键里程碑

| 里程碑 | 时间 | 交付物 | 验证标准 |
|---|---|---|---|
| M1: 信任基线 | Phase 1 第 4 周末 | 语义输出验证 v1 可运行 | `forge run build` 包含 RequirementTrace + 不变式检查 |
| M2: 多仓库首个用例 | Phase 1 第 10 周末 | 依赖图 + 跨仓库 test trigger 在 2 个 demo repo 上验证通过 | repos A+B，改变 A 的接口，B 的 gate 自动 FAIL |
| M3: 弹性闭环 | Phase 2 第 4 周末 | Agent 在 "不会做" 时自动降级而非 hard abort | fake-agent 返回 KindInexpert → 验证降级路径 |
| M4: 24h 可运维 | Phase 2 第 8 周末 | `forge timeline` + `forge diagnose` 可用于诊断 24h 运行 trace | operator 用一次 trace 定位一次 gate FAIL 的根因 ≤5 分钟 |
| M5: 长期无退化 | Phase 3 第 6 周末 | memory 文件大小稳定（增长 ≤5% per 100 次迭代）| 200 次 evolve 后 Load 时间 < 100ms |

### 5.4 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| **多仓库治理过度设计** | 中 | 高（工程浪费） | Phase 1 严格限制到最小可行：只有 `dependencies.yml` + `Engine.roots`。复杂编排推迟到有真实多仓库用户后 |
| **语义不变式太弱/太强** | 中 | 高（信任幻觉或开发负担） | 先做轻量级 `RequirementTrace`（路径映射），不做复杂的 traceability 矩阵。等用户反馈再增强 |
| **EScalation 协议引入无限递归** | 低 | 高（预算耗尽） | MaxEscalationBudget + 一次 escalation 的"熔断"——升 tier 后失败则硬化 |
| **memory 分层迁移破坏现有数据** | 中 | 中（数据丢失） | 分层设计为 opt-in（新 memory 走分层，现有 memory 在 Compact 时迁移） |
| **YAML shim 成为长期依赖** | 中 | 低（技术债） | `internal/yaml2json` 已启动 Go 替代；决策窗口设为 yaml2json 完全达到 Python shim 功能等价后 |

---

## 总结

ForgeOS 当前的架构自洽、诚实、编写严谨——是我在 AI-native 工具中见到的最完整的编排治理层之一。

**最关键的结构性缺口**是信任模型（方向一）和治理边界（方向二）：
- 没有语义验证，ForgeOS 永远卡在"自动化 CI 编排器"而非"自治软件工厂"
- 没有多仓库治理，ForgeOS 的治理域被限制在单 Git 仓库内

这两个方向合在一起，是系统在**功能性自治**上的真正天花板。方向三（故障升级）让无人值守不再是脆弱的二进制 abort；方向四（知识生命周期）防止系统随时间退化；方向五（因果追踪）让 operator 能有效诊断。但前三者的优先级高于后两者——如果信任模型和治理域这两个天花板没有突破，后面的增强是在"正确工作的单仓库 CI 器"上做的锦上添花。

**最值得深思的发现**：验证文档确认这 5 个方向在 225+ 份现有文档中**零命中**。这说明当前的架构迭代虽然密集（31 个 Sprint），但一直聚焦于"把现有的脊梁走通"——收敛信号、中枢旋钮、mode gating、真点火验证——而非向外拓展系统的能力边界。现在，脊梁已经走通。**是时候向外拓展了。**
