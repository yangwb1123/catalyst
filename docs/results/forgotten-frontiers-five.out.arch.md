# 架构分析报告：ForgeOS 扩展方向评估与设计建议

> **分析基准**：基于 forge-core Go 运行时（13 包，~7.3k 行）、harness 层（Node/Python ~13k 行）、`.agent/` 声明层、30+ 篇已有分析文档，以及输入的交叉核对评估。
>
> **角色**：资深架构师 — 不写代码，只做判断和设计决策。

---

## 1. 架构评估

### 1.1 当前架构的优势

**诚实架构（Honesty-by-Design）** 是 ForgeOS 最被低估的架构特征。这不是一个营销口号，而是可验证的代码级属性：

| 维度 | 代码证据 | 架构意义 |
|------|---------|---------|
| **N/A 永不冒充 PASS** | `orchestrator.go` `runGates()` — `NA` 分支独立，不计数为 ok | 审计追溯时不说谎 |
| **空收敛永不通过** | `converge.go` `Evaluate()` — `len(allOf) == 0` 时 `allMet = false` | 防遗漏 |
| **零值 = 向后兼容** | Engine 每一字段的零值语义逐一文档化（`// The default 0 means…`） | 增量引入不破坏已有行为 |
| **Dry-Run 默认** | `DryRunExecutor` 永远不调 LLM，`--executor command` 显式启用 | 安全默认，防止意外花费 |
| **Fail-Closed 原则** | `runGates` FAIL 返回 error → 中止；`checkAgentBudget` 超限拒绝 spawn | 没有静默绕过 |
| **接口注入而非继承** | Engine 所有变点都是函数字段（`Log`、`RunGate`、`AgentVerdict`、`BudgetExhausted`） | 极致可测试性 |

**分层纪律**：`domain` 包不 import 外层、`cmd/forge` 不 import `internal/orchestrator` 之外的东西、架构检查（`arch-check.mjs`）真解析 import 图。这不是口头承诺，是机器执法。

**两阶段增量路线**：北极星（Temporal + Firecracker + OPA）和当前（Go 状态机 + harness 脚本）之间有清晰的桥接。每一块组件都回答了「v0 形态是什么，北向目标是什么」。

### 1.2 关键设计决策评估

| 决策 | 正确性 | 权衡 |
|------|--------|------|
| Engine 用函数字段而非接口 | ✅ 当时正确 — 保持简单，17 个字段还能管理 | ⚠️ 超过 20 个字段就需要重构 |
| 收敛用代码 switch 而非规则引擎 | ✅ v0 阶段正确 — 6-7 种指标不需 OPA | ⚠️ 每新增一种指标需改代码 |
| 零外部 Go 依赖 | ✅ 安全收益巨大 — 供应链攻击面为零 | ⚠️ Python YAML shim 是代价 |
| RunFrom 是同步阻塞调用 | ✅ 当前使用模式正确 — CLI 一次性 | ❌ 扩展到 GitOps 需要异步化 |
| Checkpoint 用 JSON 无签名 | ❌ 短期可接受，但这是方向 3 要解决的根本问题 | 不签名 = 不可审计 |

### 1.3 架构债务

**P0 — 必须正视**：

1. **Engine 结构体膨胀**（17 字段，逼近 20）：这是「选项对象」反模式。每个新功能加一个字段，没有分组、没有子结构。症状：`orchestrator.go` 的 `Engine` 注释已经是该文件内容的最大块（远超函数的 50 行限制）。**建议**：引入 `EngineConfig` 子结构，将成本控制字段（`MaxRetries`、`MaxLoopBack`、`MaxAgentCalls`、`BudgetExhausted`）和回调解耦字段（`OnGateResult`、`AgentVerdict`、`OnPhase`）分组。

2. **Python YAML shim**：`python3 harness/yaml2json.py` — 这是 forge-core「纯 Go 零依赖」承诺的代价。它在构建时引入了一个**跨语言运行时依赖**（宿主必须有 Python + PyYAML），且 bypass Go 编译期类型检查。**建议**：引入 `gopkg.in/yaml.v3`，这是 Go 生态最成熟的 YAML 库，MIT 许可，被 Kubernetes、Docker 等核心项目使用。它不是「外部依赖膨胀」，而是**消除运行时断裂点**。

3. **converge.go 的 evalOne switch 语句**：7 种 metric 类型，每个是一个 case。这是「嵌入式规则引擎」的早期形态。**建议**：提取为 `Evaluator` 接口 + 注册表，使新增 metric 不需要改 converge.go。

**P1 — 需逐步解决**：

4. **Verdict 解析跨层**：`orchestrator.go` 定义了 `AgentVerdict` puller 接口（返回 `(verdict string, ok bool)`），但**实际的解析逻辑在 `cmd/forge`**。这意味着引擎知道「有 verdict 这个东西」，但不知道它的语义 — 这是好的分层。但 `reviewerRequestChanges` 常量被复制到 orchestrator 包里（被 `verdict_loopback_test.go` 确认），说明分层有条裂缝。

5. **双运行时维护**：v0（CC scripts + `.agent/`）和 v2（Go forge-core）在功能上有重叠，但维护两种路径的成本在增长。**建议**：明确定义 v0 路径的冻结边界 — 哪些场景必须走 Go 运行时，哪些场景可以保留 v0 脚本。

---

## 2. 扩展方向 — 架构分析

### 方向 1：GitOps 控制器 ⭐⭐⭐⭐⭐（最强）

#### 为什么需要

当前 forge-core 是「一次性 CLI」架构：`forge run` → 阻塞执行 → 退出。没有任何持久化状态、没有事件驱动、没有持续协调。对于一个 AI 软件工厂，这意味着：

- 无法自动响应 PR 创建、分支推送、Issue 变更
- 无法跨时间持续管理一个 repo 的治理状态
- 无法排队多个工作流（两个 `forge run` 同时操作同一个 repo 会冲突）

GitOps 控制器将 ForgeOS 从**「批处理编排器」转变为「持续协调器」**。

#### 核心挑战

1. **RunFrom 同步接口 ≠ 持续循环**：当前 Engine.RunFrom 是同步阻塞的，迭代完就退出。控制器需要一个包装层：
   ```
   当前: Engine.RunFrom(wf, mode, start) → error
   需要: Controller.Reconcile(desiredState) → (observedState, error)
   ```

2. **状态持久化升级**：`checkpoint.go` 的 Save/Load 是 JSON `os.WriteFile` — 没有事务、没有并发保护、没有版本化。一个 GitOps 控制器需要至少 `CAS（compare-and-swap）`级别的写入保证，否则两个并发 reconcile 循环会互相覆盖。

3. **事件源多样性**：
   - Git webhook（push/PR/merge）
   - 定时调度（cron）
   - 上游依赖变更（upstream repo release）
   - 手动触发（CLI 或 UI）

   每种需要不同的触发器适配器（adapter pattern）。

4. **工作队列 + 去重**：相同 commit 的两次 webhook 不应该触发两次 run。需要至少一个去重键（repo + commit SHA + workflow name）。

#### 预期架构变更

```
forge-core/
  internal/
    controller/          ← 新包
      controller.go      — 协调循环（Reconcile loop）
      queue.go           — 事件队列（内存版，未来可替换为 NATS）
      state.go           — 持久化工作流状态（包装 checkpoint，增加 CAS）
      triggers/          — 事件适配器
        webhook.go
        schedule.go
        manual.go
    webhook/             ← 新包（或单独二进制）
      server.go          — HTTP webhook 接收器
      handlers.go        — GitHub/GitLab 处理器
```

#### 与北极星的关系

GitOps 控制器是 Temporal 的**v0 替代品**。北极星中 Temporal 负责 durable workflow；在 v2.5，控制器 + 文件系统状态是这个功能的诚实 v0 版本。**架构兼容**：控制器的 `Reconcile` 循环结构可以直接映射到 Temporal Workflow 结构。

#### 影响评估

| 方面 | 影响 |
|------|------|
| 现有 `forge run` | 不变 — 控制器是补充，不是替换 |
| checkpoint.go | 需要升级到 CAS 语义 |
| 测试 | controller 包需要 mock webhook 的测试策略 |
| 安全 | webhook 处理器需要 secret 验证（HMAC） |

---

### 方向 2：工作流测试框架 ⭐⭐⭐⭐（强力候选）

#### 为什么需要

当前测试工作流编排的唯一方式是**跑真实工作流**（花钱调 LLM）或**手写 Go 测试**（需要懂 orchestrator 的内部接口）。没有一个用户可读的声明式方式来表达「当我运行这个 workflow，gate X 应该在第 3 阶段失败，然后 loop-back 应该跳转到阶段 1」。

这个方向的价值不在于「方便测试」，而在于**让工作流作者（而非 Go 核心开发者）能验证工作流行为**。

#### 核心挑战

1. **DSL 设计**：需要平衡表现力和简单性。`given/when/then` 是最自然的起点：
   ```yaml
   test_workflow: build
   given:
     gate_script: fake.sh  # always PASS on 1st call, FAIL on 2nd
     max_loop_back: 2
   then:
     - phase: implementer
       status: PASS
     - phase: gate_lint
       status: LOOP_BACK
       target: implementer
     - phase: implementer
       status: PASS
   ```

2. **现有基础设施已就位**：orchestrator 的 `orchestrator_test.go`、`verdict_loopback_test.go`、`loopback_test.go` 已经展示了**模式**。但这些是 Go 代码，不是用户可配置的 DSL。需要的是把相同的测试能力暴露到 YAML 层。

3. **测试编排器 vs 真实编排器**：需要一个新的 `TestOrchestrator` 实现，它用 fake gate/agent/verdict 而不是真实执行器。这可以完全复用 `Engine` 的注入接口 — 不需要改 Engine 本身。

#### 实施建议

```
forge test --workflow build.yml --test workflow-test.yml
```

`forge test` 是一个新的 CLI 子命令，它实例化 Engine，注入 fake gate runner（根据 test YAML 中的 `given` 配置行为），运行 workflow，然后断言 `then` 中的条件。

**不需要改 Engine 一行代码** — Engine 的所有 injectable 函数字段（RunGate、AgentVerdict、Log、OnGateResult、OnPhase）就是为这种场景设计的。

#### 风险点

| 风险 | 缓解 |
|------|------|
| DSL 复杂度过高，超过 80% 用例的需要 | 先从 3 个断言类型开始（PASS/FAIL/LOOP_BACK） |
| 测试编排器与真实编排器行为不一致 | 复用相同的 Engine，只替换函数字段 |
| 对已有分析的表层覆盖（方向 5 of genuinely-uncovered-frontiers） | 引用原文，强调「从单命令 → 声明式 DSL」的深化 |

---

### 方向 3：供应链信任与可验证治理 ⭐⭐⭐（高价值但需时机成熟）

#### 为什么需要

ForgeOS 的核心价值主张是**让 AI 自治产生代码**。但「自治」和「可信」之间存在天然张力：如果没有密码学证据链，你怎么知道你得到的代码确实经过了 gate、没有中间被篡改？

#### 核心挑战

1. **trace.jsonl 是纯文本 JSON**：任何人可以用任何编辑器修改它。没有任何防篡改能力。

2. **签名时机**：什么时候签名？每次 agent 写文件后？每次 gate 通过后？每次 approve 后？越频繁签名，防篡改能力越强，但性能代价越大。

3. **密钥管理**：签名密钥放在哪里？在 CI 中？在沙箱中？在 webhook 处理器中？密钥泄露的风险是什么？

4. **验证方是谁**：如果是 ForgeOS 自己验证自己，那 hash chain 就够了（每次运行的 trace 包含前一次运行的 hash）。只有在**第三方验证**（审计员、合规官、客户）的场景下才需要完整的 ed25519 + in-toto。

#### 推荐的最小可行方案

**不要从 full ed25519 开始。** 先做 hash chain：

```
checkpoint N: {
  hash: H(prev_hash || trace_N.jsonl)
  prev_hash: checkpoint N-1 的 hash
  phases: [...]
}
```

这提供：
- **防篡改**：修改任一 checkpoint 会让所有后续 checkpoint 的 hash 链断裂
- **无需密钥管理**：hash 是确定性的，任何人都可以验证
- **轻量级**：每次 checkpoint 保存时计算一次 SHA-256，O(1) 开销
- **方向正确**：当需要第三方验证时，ed25519 签名只需要对最后一个 checkpoint hash 签名一次，而不是对每个事件签名

**实施层级**：`internal/trace` 或 `internal/persist`，不是 orchestrator。Orchestrator 不关心 hash 链的存在，它只调用 `Save()`，hash 链是 `persist` 的实现细节。

#### 什么时候升级到 in-toto

当同时满足以下条件时：
- 有外部审计需求（合规/监管）
- 有第三方验证方（不是 ForgeOS 自己验证）
- 有多个 ForgeOS 实例需要交叉验证

---

### 方向 4：组织级成本治理 — 需要重新定位

**评估文档的交叉核对结论是正确的** — `expansion-directions-v6-novel-perspectives.md` 方向三已经以更高的架构精度覆盖了这个方向（接口设计、分阶段实施、边界情况分析）。

#### 如果保留，如何差异化

差异化点（v6 没有覆盖的增量）：

1. **成本预算的跨周期滚动**：`forfeit（use it or lose it）` vs `accumulate（未用完的继续累积）` vs `borrow（允许透支，下期扣回）` — 三种预算策略对应不同团队模式（startup vs enterprise vs platform）。

2. **成本优化建议引擎**：不只是报告「花了多少」，还能建议「这个 gate 在这个项目上跑了 N 次但从未失败，考虑移除吗？」或「这个 workflow 使用 Haiku 就能达到目的，但当前配置是 Opus」。

3. **成本告警集成**：当项目花费超过阈值 80% 时，自动创建 Jira ticket / 发 Slack 消息 / 触发 webhook。

#### 我的建议：诚实标注为增量，不声称独创

这个方向有明确的产品价值，但不应作为「全新架构方向」提出。应将其定位为 v6 方向三的**具体实施深化**。

---

### 方向 5：人类开发者协作接口 — 拆分并聚焦

**交叉核对结论准确**：pause/resume/notify 已在 `genuine-architectural-horizons-five.md` 方向三中覆盖。但两个子方向确实是空白且高价值。

#### 子方向 A：Diff Review（选择性批准改动）⭐⭐⭐⭐

这是一个**被低估的高难度架构问题**。

**问题**：当前 AgentVerdict 只有 `APPROVE` / `REQUEST_CHANGES` 两个状态。一个二进制信号无法表达「文件 A 的改动我接受，文件 B 的改动请重做」。

**架构挑战**：

```
当前: AgentVerdict("implementer") → "REQUEST_CHANGES"
      → loop-back 到 implementer → 重新生成全部

目标: AgentVerdict("implementer") → {
  approve: ["src/main.go", "src/utils.go"],
  reject: ["src/db.go"]
}
→ loop-back 到 implementer → 只重做 src/db.go
→ 保留 src/main.go 和 src/utils.go 的改动
```

这要求：
- **Verdict 协议升级**：从 `string` 到结构化类型（`map[string]string` 或类似）
- **选择性回滚**：部分应用 agent 改动（保留接受的，撤销拒绝的）
- **Engine 状态管理**：Engine 需要跟踪「哪些文件已批准」，以便下次迭代时不重新生成已批准的文件

**这不是「加一个字段」——它改变了收敛模型。** 当前模型是「最后一个完全通过的版本是结果」；新模型是「每个文件的批准状态独立演进」。

**估计工程量**：2-3 周的架构设计 + 2 周的实现 + 1 周的测试。

#### 子方向 B：交互式上下文注入 ⭐⭐⭐

**相对简单**。本质是 prompt 级别的修改：

```bash
forge run --context "不要改 auth 包" --context "使用 chi 而不是 gin"
```

**实现**：在 cmd/forge 中将 `--context` 参数注入到 agent prompt 的上下文块中。Engine 不需要知道这个参数的存在 — 它只在 prompt 装配层（cmd/forge 目前）起作用。

**工程量**：1-2 天。

#### 推荐

将方向 5 拆分为两个独立方向：
- **方向 5a：Diff Review（选择性审批）** — 保持为 P1 架构级扩展，标注「pause/resume/notify 已被覆盖」
- **方向 5b：上下文注入（`--context`）** — 降级为 P2 功能级扩展

---

## 3. 接口设计建议

### 3.1 核心原则

| 原则 | 理由 | 判断标准 |
|------|------|---------|
| **Engine 保持瘦接口，胖注入** | 架构的可测试性和可替换性 | 新增功能不应改变 `Engine` 的方法签名，而是通过注入字段 |
| **分层透明** | 引擎不知道 prompt、模型、成本单位 | `AgentVerdict` 返回 `(string, bool)` 而非 `(*claude.Verdict)` |
| **零值语义是契约** | 向后兼容是最重要的非功能属性 | 每个 `// The default…` 注释就是契约 |
| **不把 converge 做成胶水层** | converge 应该 eval，不应该编排 | `Signals` 是值对象，不是有状态对象 |

### 3.2 关键接口重构建议

#### 重构 1：Engine → Engine + EngineConfig（P1）

**当前状态**：
```go
type Engine struct {
    Exec           AgentExecutor
    RunGate        func(name string) gate.Result
    Log            func(string)
    OnGateResult   func(name, status string)
    AgentVerdict   func(phase string) (verdict string, ok bool)
    BudgetExhausted func() bool
    MaxRetries     int
    MaxLoopBack    int
    MaxAgentCalls  int
    ModePolicy     mode.Policy
    Sleep          func(time.Duration)
    OnPhase        func(phaseIdx int)
    Ctx            context.Context
    // ... 还可能继续增加
}
```

**建议状态**：
```go
type EngineConfig struct {
    // Cost & budget bounds
    MaxRetries    int
    MaxLoopBack   int
    MaxAgentCalls int
    BudgetExhausted func() bool

    // Callback injection points
    OnGateResult  func(name, status string)
    AgentVerdict  func(phase string) (verdict string, ok bool)
    OnPhase       func(phaseIdx int)
    Log           func(string)
    Sleep         func(time.Duration)
}

type Engine struct {
    Exec      AgentExecutor
    RunGate   func(name string) gate.Result
    ModePolicy mode.Policy
    Ctx       context.Context
    Config    EngineConfig  // default zero = back-compat
}
```

**好处**：
- 新功能加字段时不需要改 Engine 结构体本身
- 配置字段和回调字段分组
- 测试可以 `Engine{Config: EngineConfig{...}}` 而不是平铺

#### 重构 2：converge.Evaluator 接口（P2）

```go
// Evaluator evaluates one convergence metric. Register implementations to
// extend convergence without changing converge.go.
type Evaluator interface {
    Metric() string          // e.g., "roadmap_completion"
    Evaluate(crit Criterion, signals Signals) Result
}

// Register makes an evaluator available to evalOne.
func Register(evaluator Evaluator)
```

**好处**：
- 消除 `evalOne` 的 switch 语句
- 第三方/插件可以注册自定义收敛指标
- 每个指标的测试隔离在自己的包中

### 3.3 向后兼容性策略

ForgeOS 目前的零值向后兼容策略是**业界最佳实践**。我建议两处增强：

1. **契约测试**：每个 Engine 字段的零值语义应该有一个自动化的契约测试（`engine_backcompat_test.go`），验证 `Engine{}` 零值的 `RunFrom` 与上一个版本行为逐字节一致。

2. **弃用标记**：当从 Engine 中移除字段时，先标记为 Deprecated（保留 2 个版本），然后用 [deprecated field pattern](https://github.com/golang/go/issues/38974) 替换为 `noop` 实现。

---

## 4. 技术选型

### 4.1 需要引入的技术

| 技术 | 优先级 | 理由 | 风险 |
|------|--------|------|------|
| `gopkg.in/yaml.v3` | **P0** | 消除 Python shim 运行时断裂点 | 违反「零外部依赖」祖训，需架构审批 |
| `crypto/ed25519`（stdlib） | **P1** | 方向 3 供应链签名的基石 | 零风险（已在 stdlib 中） |
| `crypto/sha256`（stdlib） | **P1** | Hash chain 实现 | 零风险（已在 stdlib 中） |
| OPA/Rego | **P2** | 当 policy 维度超过 5 个时 | 引入重型依赖前需充分评估 |

### 4.2 不推荐引入的技术

| 技术 | 原因 |
|------|------|
| Temporal SDK | 北极星目标，但现在引入会过度设计。当前 Go 状态机 + checkpoint + GitOps 控制器是足够的 v0 版本 |
| Firecracker SDK | 数据面隔离是 v3- 的工作，v2 不需要 |
| Wire/DI 框架 | Engine 的注入模式已经足够，引入 DI 框架会增加认知负担而不带来可衡量的收益 |
| gRPC | 在 monolith-to-services 之前的单二进制阶段，gRPC 是负价值 |

### 4.3 自建 vs 采购决策

| 能力 | 决策 | 理由 |
|------|------|------|
| YAML 解析 | 采购（`yaml.v3`） | 45 行代码的 shim 不值得自建 |
| Hash 链/签名 | 自建 | 不超过 100 行代码，充分利用 stdlib |
| Git 操作 | 自建（`os/exec` + git CLI） | Go 的 git 库（go-git）体积大且边界行为多，CLI 调用更可靠 |
| Webhook 服务器 | 自建 | 标准 HTTP handler，不需要框架 |
| 规则引擎 | 暂不采购 | 当前 switch 语句 7 个 case 尚可管理，达到 15 个 case 再考虑 OPA |

---

## 5. 实施路线图

### 优先级排序

| 优先级 | 方向 | 价值密度 | 风险 | 工期估计 |
|--------|------|---------|------|---------|
| **P0** | 方向 4 定位修正 | 修复 credibility | 低 | 1 天 |
| **P0** | 方向 5 拆分 + 上下文注入 | 产品价值/实现成本比最高 | 低 | 2-3 天 |
| **P0** | Go YAML 库替换 Python shim | 消除断裂点 | 中（需架构审批） | 1-2 天 |
| **P1** | GitOps 控制器 Phase 1（watch mode） | 最高杠杆架构扩展 | 中 | 2-3 周 |
| **P1** | Engine 配置分组重构 | 阻止架构债务恶化 | 低（零值兼容） | 3-5 天 |
| **P2** | 工作流测试 DSL Phase 1 | 解锁工作流作者效率 | 低（复用 Engine 接口） | 1-2 周 |
| **P2** | Hash chain 供应链证明 | 为审计打基础 | 低（纯 stdlib） | 3-5 天 |
| **P3** | Diff Review 选择性审批 | 高难度架构变更 | 高（改变收敛模型） | 3-4 周 |
| **P3** | 组织级成本治理增量 | 与 v6 增量 | 低 | 视预算系统而定 |
| **P3** | in-toto 完整签名 | 只在有第三方审计需求时 | 中 | 2 周 |

### 阶段划分

#### Sprint N（当前）：Credibility 修复 + 低垂果实

```
已确认: P0 三项
- 方向 4：与 v6 文档合并或重定位
- 方向 5：拆分为 5a（diff review）+ 5b（context injection）
- Engine 文档诚实描述差异
```

#### Sprint N+1：架构债务还清

```
- Go YAML 库集成，删除 Python shim
- EngineConfig 分组重构（17 字段 → 7+子结构）
- converge.Evaluator 接口提取（消除 switch）
- Engine 零值契约测试
```

#### Sprint N+2~N+3：P1 架构扩展

```
- GitOps 控制器 Phase 1（--watch 模式）
  - 事件队列（内存）
  - checkpoint 升级到 CAS 写入
  - 单 repo 多 workflow 排队
- Hash chain 基础实现（persist 层不感知）
```

#### Sprint N+4~N+5：P2 扩展

```
- 工作流测试 DSL
  - forge test CLI 命令
  - 解析 test YAML
  - 复用 Engine 注入点
- GitOps 控制器 Phase 2（多 repo）
```

### 风险矩阵

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 方向 4 的零覆盖声明被质疑 → 整份文档 credibility 受损 | **高** | 中 | 立即修正（P0） |
| Engine 字段继续膨胀越过 25 → 认知负荷超限 | 中 | **高** | Sprint N+1 分组重构 |
| Go YAML 依赖 → 被质疑违反零依赖原则 | 中 | 中 | 架构 ADR 写明 trade-off，限制在 `cmd/forge` 不使用在 `internal/` |
| GitOps 控制器与 v0 auto-run 模式功能重叠 | 中 | 中 | 明确定义 v0 脚本冻结边界 |
| Diff Review 的 per-file 收敛模型 → 复杂度过高 | 低 | 高 | 先做可行性原型（1 周 spike），再决定是否 P2→P3 |
| 工作流测试 DSL 太复杂 → 没人用 | 中 | 中 | 从「1 个 gate + 1 个 loop-back」的最简 DSL 开始，80% 用例先覆盖 |

### 最终推荐的 3 个方向（评估后的排序）

基于架构价值 + 杠杆率 + 差异化程度，我建议作者将文档范围调整为以下**三个真正空白且有架构深度的方向**：

1. **🟢 GitOps 控制器** — 最高架构杠杆。它将 ForgeOS 从「批处理 CLI」转变为「持续平台」，是北极星 Temporal 路径上的诚实 v0 阶梯。**没有其他分析文档触及这个架构层。** ← 主推方向

2. **🟢 工作流测试框架** — 最高功能杠杆。借助 Engine 已有的注入能力（RunGate/AgentVerdict 等），可以用最小工程量（1-2 周）实现一个声明式测试 DSL。**已有分析只有表层提及。** ← 速赢方向

3. **🟢 供应链信任（hash chain 优先）** — 高合规价值。`crypto/sha256` 是 stdlib，实现 hash chain 不超过 100 行代码。它不改变任何行为，只增加可审计性。**已有分析关注点不同（签名审计日志 vs 运行时治理证据）。** ← 合规方向

**方向 4 撤销或重定位为增量；方向 5 保留 diff review 子方向但标注 pause/resume/notify 已覆盖。**

---

*以上分析基于 forge-core Go 运行时代码（`internal/orchestrator`、`internal/converge`、`internal/persist`、`internal/trace`）、harness 层（`gate.mjs`、`acceptance.mjs`、`check.py`）、`.agent/` 架构文档（`ARCHITECTURE.md`、`north-star.md`、`ROADMAP.md`），以及 30+ 篇 docs/analysis/ 分析文档的交叉核对。*
