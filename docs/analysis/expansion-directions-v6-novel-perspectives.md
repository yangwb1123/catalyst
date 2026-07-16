# ForgeOS — 第六轮扩展方向分析：从既有盲区外寻找新视角

> **扫描基准**：基于 forge-core Go 运行时 ~7.3k 行 + harness Node/Python ~13k 行 + 声明层 `.agent/` 全量扫描。
> **方法**：逐一阅读此前 16+ 篇分析文档的覆盖范围，然后专门寻找 **从未被任何既有文档深入分析过的角度**。
> **角色**：资深架构师 / 产品经理，不写代码，只做判断。
>
> **诚实声明**：前五轮分析已覆盖了 90% 的明显方向——沙箱/RAG/审批/多仓编排/变异测试/数据真实性/锁竞争/增长瓶颈/自测/三模型配置/管道盲区/韧性/八波回归等。本文聚焦那剩余的 10%——系统在架构层面最深层的、尚未被讨论的隐性缺口。

---

## 此前 16 篇 doc 已覆盖的领域（确认不重复）

为避免重复发明，我逐一核对了此前各篇并确认以下方向**已有深度分析**，本文不再展开：

| 已有分析 | 对应文档 |
|---------|---------|
| Agent 沙箱隔离（Firecracker/microVM） | `expansion-directions.md` 方向一 |
| 语义检索/RAG/Embedding pipeline | `expansion-directions.md` 方向二；`edgecases-and-perf.md` §4 |
| 持久化审批/通知/durable wait | `expansion-directions.md` 方向三；`edgecases-and-perf.md` §3.3 |
| 跨仓库编排/联邦治理 | `expansion-directions.md` 方向四 |
| 变异测试/测试生成/自演化质量 | `expansion-directions.md` 方向五 |
| Trace 事件完备性（8 种事件 vs 当前 1 种） | `seventh-wave-data-realism.md` 方向一 |
| Memory 压缩与归档 | `seventh-wave-data-realism.md` 方向二 |
| Checkpoint 历史追溯与趋势 | `seventh-wave-data-realism.md` 方向三 |
| 故障注入与持久层 Chaos Engineering | `seventh-wave-data-realism.md` 方向四 |
| 真实数据回放测试 | `seventh-wave-data-realism.md` 方向五 |
| 并行编排竞态/errgroup 短路的失败 | `edgecases-and-perf.md` §1.1 |
| trace.jsonl 轮换/无限增长 | `edgecases-and-perf.md` §2.1 |
| memory.Load 每相位全文件扫描性能 | `edgecases-and-perf.md` §2.2 |
| 收敛理论隐藏陷阱（门闩/零相位/假收敛/HumanGate 状态丢失） | `edgecases-and-perf.md` §3 |
| Prompt 构建序列化瓶颈（ADR 检索/卡片读取/YAML shim） | `edgecases-and-perf.md` §4 |
| 治理盲区（测试退化/代码无测试/零迭代 checkpoint/scorecard mode 偏见） | `edgecases-and-perf.md` §5 |
| cmd/forge 耦合过重（3831 LOC/52% 总代码） | `growth-bottlenecks-and-scalability.md` §2 |
| Engine 字段增长（15 个字段/无分组） | `growth-bottlenecks-and-scalability.md` §3 |
| 500 行红线逼近（mode.go 498 LOC） | `growth-bottlenecks-and-scalability.md` §4 |
| Node.js Python 运行时依赖风险 | `growth-bottlenecks-and-scalability.md` §6 |
| 三套测试套件覆盖分析 & dogfooding 真实度 | `self-testing-and-dogfooding.md` |
| 三条隐藏反馈回路（Memory→Prompt→Agent→Memory） | `hidden-feedback-and-pipeline-gaps.md` §1 |
| ForgeOS 对自己治理规则的自我认知 | `hidden-feedback-and-pipeline-gaps.md` §2 |
| CI/CD 盲区（旧版 CI 缺 race/端到端/单元测试） | `hidden-feedback-and-pipeline-gaps.md` §3 |
| 信号质量与垃圾进垃圾出（Memory→RoadmapCompletion→Convergence） | `hidden-feedback-and-pipeline-gaps.md` §4 |
| 长时间运行操作表面退化 | `hidden-feedback-and-pipeline-gaps.md` §5 |
| Meta-governance（自身治理 vs 产出治理的差距） | `expansion-forgeos-meta-governance.md` |

> **结论**：90% 的明显方向已被前人覆盖。本文的 5 个方向是 **「还从未有人问过的问题」**——它们不在表面，而藏在系统的架构假设和隐式契约中。

---

## 目录

1. [方向一：跨 Agent Prompt 注入防护与完整性层](#方向一跨-agent-prompt-注入防护与完整性层)
2. [方向二：置信度感知的决策引擎（替代二元收敛）](#方向二置信度感知的决策引擎替代二元收敛)
3. [方向三：多租户资源联邦与成本 Chargeback](#方向三多租户资源联邦与成本-chargeback)
4. [方向四：自愈层运行时——从检测到自动恢复](#方向四自愈层运行时从检测到自动恢复)
5. [方向五：架构度量趋势分析——从「交通警察」到「早期预警系统」](#方向五架构度量趋势分析从交通警察到早期预警系统)

---

## 方向一：跨 Agent Prompt 注入防护与完整性层

### 现状

现有安全体系覆盖了以下维度：

| 防护层 | 位置 | 防什么 |
|--------|------|--------|
| 资源四维护栏（深度/数量/时间/内存） | `CommandExecutor` | Agent 失控的资源消耗 |
| Secret 扫描（`secret-scan.mjs`） | Harness gate | 提交的硬编码凭证 |
| 风险分类器（`internal/risk`） | Routing | 高风险任务→Opus 强制 |
| 输出上限（`cappedBuffer`） | `CommandExecutor` | Runaway stdout OOM |
| 递归守卫（`FORGE_AGENT_DEPTH`） | `CommandExecutor` | Fork-bomb |

**但这全部是「宿主机→Agent」方向的安全。`Agent→Agent` 方向的完整性——即一个 agent 的输出如何被另一个 agent 消费——完全没有防护。**

### 问题：跨 Agent 污染路径

ForgeOS 有三条显式的跨 Agent 数据流：

```
路径 1: Memory → Prompt → Agent (cross-iteration, 持久化)
  Agent_A 的输出 → memory.Append("lesson") → memory.Load() → buildPrompt() → Agent_B 的 prompt

路径 2: GateLedger → Prompt → Agent (cross-phase, 一次 run 内)
  前序 gate 结果 → gateLedger.record() → gateLedger.context() → buildPrompt() → Agent_B 的 prompt

路径 3: PhaseOutput → Prompt → Agent (cross-phase, feed-forward)
  前序 agent 的产出 → phaseOutputLedger → buildPrompt() → 下游 agent 的 prompt
```

**每条路径都是单向、无验证、无签名的纯文本管道。** 如果一个 agent 的输出被（意外或恶意）构造为包含 prompt 注入载荷，下游 agent 会无条件信任它。

### 攻击场景

**场景 A：通过 Memory 持久化后门**

```
Evolve Iteration 3: Implementer 写入代码时输出包含隐藏指令
  → parser 提取为 KindLesson "we decided to use SQLite"
  → memory.Append("we decided to use SQLite")
  → 此后每次 Iteration: memory.Load() → Agent prompt 中出现
    "we decided to use SQLite (Lesson from Iteration 3)"
  → 下游 Agent 视其为合法上下文，据此决策
```

这不是"Agent 撒谎"的问题——而是 Agent 输出中的措辞可能被后续 Agent 解释为指令。如果 Implementer 输出"系统应该用这个连接字符串：...",后续 Agent 可能直接使用它。

**场景 B：GateLedger 的微妙误导**

```
Gate 结果：test: N/A（因为测试工具未安装）
→ gateLedger 渲染为 "前序闸门结果：\n- test: N/A"
→ Reviewer prompt 中出现
→ Reviewer 看到 "N/A"，可能理解为 "测试已通过"（因为当前 prompt 中 N/A 和 PASS 的渲染格式一致，
  都是绿色/中性条目。converge.go 的 greenDetail 正确区分它们，但 gateLedger.context() 不区
  分——render 格式是 "gate name: status"，没有视觉差异）
```

**场景 C：Feed-forward 的确认偏差**

```
Implementer 的产出包含 `// TODO: fix security hole`
→ phaseOutputLedger 原样传递给 Reviewer
→ Reviewer 看到后可能在 prompt 中产生 "已知安全问题" 的上下文
→ Reviewer 的判决可能被此上下文（而非实际代码）倾斜
```

### 为什么现在需要

| 维度 | 理由 |
|------|------|
| **OWASP Agentic Top-10 (2025-12)** | 排名 #2 即是 "Agent-to-Agent Contamination"，ForgeOS 目前的跨 Agent 管道完全无防护 |
| **ForgeOS 的自治深度** | 系统设计目标 24h 无人值守。无人值守意味着无人发现 5 小时后 memory 被污染。自动防护是必要非可选 |
| **非恶意、更危险** | 不需要恶意攻击者——一个格式错误的 Agent 输出就能产生意外指令。Haiku 模型的输出比 Opus 更容易出格式问题 |
| **现有基础设施已被"信任"** | `memory.Append` 不对内容做任何校验——只要是字符串就写入。`gateLedger.context()` 同样无校验。信任假设隐藏在每一行代码中 |

### 建议的架构方向

```
完整性层（internal/integrity, 纯 Go 零依赖）:

1. 内容起源标注（Provenance Tagging）
   type IntegrityTag struct {
     SourcePhase   string    // "implementer" | "reviewer"
     SourceModel   string    // "claude-sonnet-4" | "claude-opus-4"
     Confidence    float64   // 0.0-1.0, 由 source 类型决定基线
     Sanitized     bool      // 是否已通过净化过滤器
     InjectedBy    string    // "memory" | "gate_ledger" | "feed_forward"
   }
   - memory.Entry 增加 Integrity 字段
   - gateLedger 条目增加 Integrity 字段
   - Prompt 注入时携带 Integrity 上下文

2. 注入内容边界标记（Content Boundary）
   - 每条跨 agent 数据在 prompt 中用显式边界标记包裹：
     ──── 来自记忆（Iteration 3, implementer, 置信度 0.6）────
     [内容]
     ──── 记忆结束 ────
   - Agent 看到的不是纯文本，而是带来源标注的结构化信息
   - Agent 可以据此判断：这是一条来自 implementer（低可信）的
     旧记忆，不应高于当前 reviewer（高可信）的独立判断

3. 敏感模式过滤（Output Sanitizer）
   - memory.Append / gateLedger.record 写入前过一道过滤器：
     - 检测常见的 prompt 注入模式（"忽略前述指令"类模式）
     - 检测连接字符串/Credential 模式（在 secret-scan 之上加运行时的）
     - 对过长的单条内容做截断告警（防止通过记忆注入海量数据撑爆窗口）
   - 过滤不阻断写入——只标记 sanitized=true + 记录告警
   - 下游 agent 收到时知晓 "此内容经过净化但仍有风险"

4. 完整性校验（Cross-Agent Integrity Check）
   - 可选的 reviewer 阶段：在 agent phase 之后专门跑一个轻量检查，
     验证前序 agent 的输出是否产生非预期的 prompt 影响
   - 与现有 reviewer 的职责分离：一个检查代码质量，一个检查
     跨 agent 完整性
```

### 边界情况

- **误报**：敏感模式过滤可能将正常的技术讨论标记为注入（"忽略旧方案"是常见的重构描述）。需要白名单/调优
- **性能**：每次 memory.Append 过过滤器引入 ~1ms 延迟。50 次 iteration × 5 phase = 250ms 总开销——可接受
- **向后兼容**：现有 memory 无 Integrity 字段。新代码读旧 memory 时 Integrity=nil → 视为 "unknown origin"——不阻断但标记为低置信
- **来源标注的 token 开销**：每条标记约 50-100 token。20 条记忆 ~2k token——对 prompt 窗口影响可控

---

## 方向二：置信度感知的决策引擎（替代二元收敛）

### 现状

当前 ForgeOS 的收敛决策是**纯二元的**：

```
convergence: MET / NOT MET
Gate:        PASS / FAIL / N/A
Reviewer:    APPROVE / REQUEST_CHANGES
HumanGate:   approved / not approved
Roadmap:     [x] / [ ] / [~]
```

所有信号被硬聚合为 `AllOf`（与逻辑）或 `AnyOf`（或逻辑）。系统无法回答以下问题：

- "Gate 全绿但只有一个测试勉强通过（95% 代码未覆盖）——收敛有把握吗？"
- "Roadmap 自报 100% 但 Agent 从未写过测试——收敛可信吗？"
- "Reviewer APPROVE 了但 reviewer 是 Haiku 模型——这个批准的价值权重是多少？"

这是**信号质量的问题**（`hidden-feedback-and-pipeline-gaps.md` §4 已识别），但当前的设计
缺乏表达和处理信号置信度的**机制**——这限制了系统的容错性和早期故障检测。

### 问题：三个具体的脆弱性

**脆弱性 1：RoadmapCompletion 虚假 MET**

之前文档已指出 RoadmapCompletion 是 agent 自报的，无独立验证。但更深层的问题是：
系统将低置信信号（self-report）与高置信信号（gate result）放在同一个 `AllOf` 表达式中，
**赋予它们相同的权重**：

```go
// converge.go
allMet := true
for _, criterion := range stop.AllOf {
    met := evaluate(criterion, signals)
    allMet = allMet && met  // 一旦假 → 全假
}
```

一个 100% 自报但未验证的 RoadmapCompletion + 一个 gate green = **MET**。但如果
RoadmapCompletion 只有 70% 自信（agent 不确定是否完成了），而 gate 100% 自信地绿了——
当前系统会说 NOT MET（因为 Roadmap 未达 100%），**即使代码已经正确工作**。

**脆弱性 2：Gate 的 "灰区" 被忽略**

Gate 是二元 PASS/FAIL，但一个 gate 可以有真实的"灰区"：
- 测试通过但覆盖率从 80% 降到 79%（还未触发 coverage 门槛——如果 coverage 是 N/A 则完全无感）
- 架构检查通过但认知复杂度从 12 升到 14（趋近 15 的阈值）
- Secret 扫描通过但发现了一个已撤销的 API key（低风险但非零风险）

这些灰区信号当前**完全消失**——它们不触发任何告警或降级，直到硬阈值被突破。

**脆弱性 3：无法表达"部分收敛"**

当 4/5 指标满足时，系统输出 NOT MET。但真实场景中：
- 4/5 满足 + 第 5 个接近满足 = "我们几乎完成了，可以手工收尾"
- 4/5 满足 + 第 5 个完全没开始 = "还差很远"

当前系统无法区分这两种情况——都输出 NOT MET，exit 1。

### 为什么需要

| 维度 | 理由 |
|------|------|
| **操作信任** | 一个总是返回二元 PASS/FAIL 的系统，在 CI 中会"狼来了"。当 gate 总在边缘情况 FAIL 时，开发人员会学会忽略它。置信度信号让 gate 更 nuanced |
| **自治决策** | 24h 无人值守运行的 evolve 循环必须能在"不确定"时暂停并请求帮助，而不是在假 MET 上继续 |
| **路由反馈** | scorecard 已经记录了 quality_score — 0.0-1.0 浮点数——但收敛引擎只消费二元信号。置信度是 scorecard 数据与收敛引擎之间的缺失桥梁 |
| **人机协作** | Human 在 Design gate 处批准了架构，但如果在 Build 阶段发现意外问题，系统应该能表达"架构需要重新审议（置信度从 0.9 降到 0.6）"，而不是卡在二元 APPROVE/REJECT |

### 建议的架构方向

```
ConfidenceEngine (internal/confidence, 纯 Go 零依赖):

1. 信号置信度标注
   type ConfidenceSignal struct {
     Criterion   string    // "roadmap_completion" | "gates_status" | "reviewer_verdict"
     RawValue    float64   // 0.0-1.0（归一化）
     Confidence  float64   // 0.0-1.0（该测量值得信任的程度）
     Source      string    // "agent_self_report" | "harness_gate" | "human"
     Trend       float64   // 相较于上一次的变化率（可选，用于趋势检测）
   }

   每条收敛指标的置信度基线：
   ┌─────────────────────┬──────────┬──────────────────────────────┐
   │ 信号源              │ 基线置信  │ 理由                         │
   ├─────────────────────┼──────────┼──────────────────────────────┤
   │ gate result (test)  │ 0.95     │ 客观可重现                   │
   │ gate result (N/A)   │ 0.0      │ 未检查                       │
   │ RoadmapCompletion   │ 0.5      │ agent self-report            │
   │ Reviewer APPROVE    │ 0.7      │ 独立 agent 但可能有确认偏差   │
   │ Human approval      │ 0.99     │ 外部权威（但 human 也会犯错） │
   │ GateLedger context  │ 0.8      │ 客观但可能有 gap             │
   │ Memory entry        │ 0.4      │ 单 agent 输出，无验证        │
   └─────────────────────┴──────────┴──────────────────────────────┘

2. 置信度加权聚合

   当前：convergence = allOf(gate_green, roadmap_100%, review_approve)
   置信度版本：
     weighted_score = W_gate × 1.0 + W_roadmap × 0.5 + W_review × 0.7
                    = 0.6×1.0 + 0.2×0.5 + 0.2×0.7
                    = 0.84

     收敛阈值：
       ≥ 0.95 → 强烈收敛（可自动继续下一阶段）
       0.80-0.94 → 弱收敛（继续当前阶段但标记观察）
       < 0.80 → 未收敛（需要更多 iteration 或人工介入）

3. 趋势检测
   跟踪每条信号在连续 iteration 中的变化：
   - RoadmapCompletion: 0.5 → 0.6 → 0.6 → 0.6（停滞）
     ➝ 即使未达阈值也触发 "plateau detected" 告警
   - GatesGreen: true → true → true → true
     ➝ 稳定，高置信
   - CostPerIteration: $0.20 → $0.50 → $1.20（递增）
     ➝ "cost trend anomaly" 告警

4. Reviewer Verdict 置信度校准
   reporterHumanGate 的二元 APPROVE 改为：
   - 如果 reviewer 模型是 opus + 审查耗时 > 30s + 输出了具体代码问题 → 高置信 APPROVE
   - 如果 reviewer 模型是 haiku + 审查耗时 < 5s + 输出模板化语言 → 低置信 APPROVE
   - 置信度 = function(模型档位, 审查耗时, 输出长度, 具体引用数)
```

### 边界情况

- **阈值选择**：0.84 收敛是经验值。不同模式应该有不同阈值（explorer 0.7, engineering 0.9, production 0.95）
- **冷启动**：第一次运行时没有趋势数据，所有 Trend=0。需要默认初始值
- **Overfitting**：agent 可能学会"只要输出足够长就能获得高置信 APPROVE"。监管需要避免 agent 适应置信度模型
- **向后兼容**：所有现有 workflow 使用二元 stop condition。置信度引擎是可选覆盖——不声明时仍用二元 AllOf/AnyOf

---

## 方向三：多租户资源联邦与成本 Chargeback

### 现状

当前所有资源跟踪是**单租户的**：

| 资源维度 | 当前范围 | 粒度 |
|---------|---------|------|
| Agent 调用次数 (`--max-agent-calls`) | 单次 `forge run` | 全局累加 |
| 运行预算 (`--run-budget-usd`) | 单次 `forge run` / `forge evolve` | 全局累加 |
| 递归深度 (`--max-agent-depth`) | 单进程树 | 进程链 |
| Timeout (`--timeout`) | 单 agent 命令 | 单命令 |
| 输出上限 (`--max-output-bytes`) | 单 agent 命令 | 单命令 |

这些在单用户场景下完全足够。但 ForgeOS 的北极星架构描述的是**多租户的**——多个团队、多个项目、共享同一个 ForgeOS 实例。

```
North-star 拓扑（见 architecture/north-star.md）：
  IAM/Tenancy — OIDC/RBAC-ABAC/租户隔离/secrets
  Cost/Budget — token 计量/配额/熔断
  Agent Registry & Scheduler — 角色↔宿主映射, 调度/bin-pack/配额
```

当前代码中完全不存在以下能力的任何实现或框架：
- 团队级资源配额（"Team A 每月上限 $500"）
- 跨项目成本 chargeback（"项目 X 消耗了 $120.50"）
- 资源共享策略（"CI 任务用共享池，生产 evolve 用独占池"）
- 优先级抢占（"P0 产品发布可以根据需要从最佳 effort 任务借调配额"）

### 问题：三个具体缺口

**缺口 1：预算孤立，无总账**

当前预算机制：

```
forge run build --run-budget-usd 10.00
  → 这个 run 的上限是 $10.00，之后 fail-closed
  → 另一个用户可以同时跑 forge run build --run-budget-usd 50.00
  → 两个 run 互不知晓对方的存在
  → 总支出可以轻松超过任何团队/项目/组织的合理上限
```

没有中心化记账，没有汇率，没有"本月初至今消费"的查询接口。

**缺口 2：无配额预留或仲裁**

```
Team A 启动 24h evolve（10 iteration × 5 phase × $0.18 = ~$9.00 预算）
Team B 同时启动生产 build（3 phase × $0.50 = ~$1.50）
→ 如果共享 GPU/模型吞吐有限，谁先得到服务？
→ 当前：先启动的先占满，无公平调度
```

**缺口 3：成本归因只有 run 维度**

当前 `trace.jsonl` 的 cost 事件归因到 `(phase, model)`，但没有：

```
tenant_id / team_id / project_id 字段
charge_code / cost_center 字段
environment (ci / dev / production) 字段
```

所以即使实现 chargeback，数据也不够归因。

### 为什么需要

| 维度 | 理由 |
|------|------|
| **从工具到平台** | 单租户 CLI 不需要计费。多租户控制平面需要。ForgeOS 的宣言是"AI-native 软件工厂"——工厂有多条产线、多个团队 |
| **治理一致** | 当前 ForgeOS 治理代码质量（gate），但不治理自己的资源消耗。一个 24h evolve 循环烧掉 $200 但没有任何门控——这与它宣称的治理承诺矛盾 |
| **dogfood** | ForgeOS 自身如果管理多个项目（ADR-0003 的治理资产共享），就需要知道每个项目消耗了多少成本 |
| **预算可预测性** | 组织在采用 AI 编码工具时的首要问题是"这要花多少钱"。没有多租户计费框架就无法回答 |

### 建议的架构方向

```
Phase 1: 成本归因扩展（增量，不破坏现有）
  trace.Event 增加可选字段：
    TenantID    string  `json:"tenant_id,omitempty"`     // 项目/团队标识
    Environment string  `json:"environment,omitempty"`   // "ci"|"dev"|"production"
    ChargeCode  string  `json:"charge_code,omitempty"`   // 成本中心/计费代码

  这些字段从 forge run/evolve 的 --tenant / --env 标志传入。
  空值向后兼容（omitempty = 无租户信息）。

Phase 2: 配额管理器（internal/quotas, 纯 Go）
  type QuotaStore interface {
    Reserve(ctx context.Context, tenant string, amount MicroUSD, ttl time.Duration) (bool, error)
    Consume(ctx context.Context, tenant string, amount MicroUSD) error
    Remaining(ctx context.Context, tenant string, since time.Time) (MicroUSD, error)
    Reset(ctx context.Context, tenant string, period BudgetPeriod) error
  }

  BudgetPeriod: "monthly" | "per-run" | "lifetime"

  默认实现 FileQuotaStore：
    .forge/quotas/<tenant>.jsonl — 写前日志 + 当前状态快照
    （复用 trace/checkpoint 的原子写入模式，零外部依赖）

Phase 3: 配额在 Orchestrator 中的消费
  Engine 增加 Quota QuotaStore 字段（nil=无配额检查，向后兼容）
  checkRunBudget 之前先 checkQuota(tenant, estimatedCost)
  - quota 不足 → 记录告警 + 降档（hallt 或切到更便宜模型）而非直接 abort

Phase 4: 成本报告命令
  forge cost report --tenant TeamA --since 2026-06-01
    TeamA 本月消费: $342.15
    按项目:
      payment-api:   $187.20 (54.7%)
      user-service:  $120.95 (35.3%)
      shared-infra:   $34.00 (10.0%)
    按模型:
      claude-opus-4: $245.30 (71.7%)
      claude-sonnet:  $96.85 (28.3%)
```

### 边界情况

- **租户信息的真实性**：`--tenant` 是自报的。在没有 IAM 的情况下，恶意用户可以冒充其他租户。v1 是 advisory 级别的归因，v2 需要 IAM 集成
- **配额精度**：成本估算发生在 agent spawn 之前（准确值未知）。配额预留使用"上次类似 phase 的成本"或"模型均价 × max_tokens"的估算——预留过多则配额利用率低，过少则过度拒绝
- **文件配额存储的并发**：多个 `forge run` 同时写入配额 JSONL——需要文件锁或原子追加。与 trace 的并发写入问题相同
- **跨机器配额**：如果 `forge` 在 CI runner（临时 VM）上运行，本地配额文件不跨节点。v1 局限在单机，v2 需要中心化配额服务

---

## 方向四：自愈层运行时——从检测到自动恢复

### 现状

ForgeOS 有一些**检测**机制：

| 检测器 | 检测什么 | 动作 |
|--------|---------|------|
| `forge doctor` | trace/checkpoint/memory 完整性 | 报告问题，不修复 |
| `checkpoint.Save` 写前校验 | 编码前的结构完整性 | 返回 error——调用者决定是否重试 |
| `trace.Open` 读尾行校验 | JSONL 最后一行 | 仅记录 "forge doctor: trace.jsonl: last line appears truncated" |
| `cappedBuffer` 截断告警 | 输出超过上限 | 截断 + 记录告警 |
| `staleCount` | Roadmap 停滞 | 触发收敛停机 |

但**没有自动恢复**。当检测到问题时，系统停止前进（安全），但不尝试修复自身然后继续。

### 问题：四个需要自愈的场景

**场景 1：trace.jsonl 截断——收敛回退到 iteration 边界**

```
forge evolve 运行 47 次 iteration 后崩溃（断电/OOM）。
trace.jsonl 最后一行不完整（写了一半的 JSON）。
重启 --resume：
  → checkpoint 说 iteration=47, phase_index=2（已经过了 reviewer）
  → trace.Load 读到损坏行（当前行为未定义——可能跳过、可能 error）
  → scorecard_wind.go 的 windDownScorecards 在 run 结束时扫描全文件
  → 如果 trace 无法完整解析 → scorecard 无法更新 → 数据丢失
```

**自愈方案**：检测到最后一行损坏 → 自动截断最后一行 → 记录修复日志 → 继续。这不是数据丢失——损坏的行本来就没法用。

**场景 2：memory 被单条巨量内容撑爆**

```
Agent 输出了 500KB 的调试日志 → memory.Append 写入一条巨大的 Entry
→ 此后每次 prompt 构建：memory.Load → 解析 500KB → boundMemory → 筛选
→ 每次 500KB 解析产生 ~50ms 延迟
→ memory.jsonl 文件被这条无用的超大记录永久污染
```

**自愈方案**：`memory.Append` 在写入前检查 Entry.Detail 长度 > 10KB → 自动截断到 10KB + 附加 `[truncated: original 523KB]` 标记。已损坏的旧条目通过 `forge memory prune` 修复。

**场景 3：Phase 挂起无进展——自适应超时降级**

```
reviewer agent 启动了但 hang（claude 进程挂起）。
--timeout=5m 但没设置（默认 0=无限）。
orchestrator 卡在 runAgentPhase，永远不会继续。
```

**自愈方案**：Engine 增加隐式 Heartbeat 机制——如果没有配置 `Sleep`/`Timeout`，但某个 phase 的执行时间超过了同类 phase 历史 P99 延迟的 3 倍，自动弹出一个 goroutine 发送 SIGTERM，记录 "adaptive timeout: phase N exceeded P99×3"。这与现有 timeout 不冲突（显式 `--timeout` 优先）。

**场景 4：Checkpoint 损坏后 fallback**

```
checkpoint.json 被手动编辑破坏。
forge evolve --resume 读它 → JSON unmarshal error。
当前行为：abort with error "checkpoint corrupt"。
用户不知道怎么办。
```

**自愈方案**：检测到损坏 → 尝试读取 `.forge/checkpoint.json.1`（最近的备份）→ 如果也有问题 → 从 iteration=1 phase=0 开始全新 run + 打印 "checkpoint corrupt, starting fresh (backup also corrupt)"。always 前进，不阻塞等待人工。

### 为什么需要

| 维度 | 理由 |
|------|------|
| **24h 无人值守** | "24h 无人值守"意味着 24h 内无人处理故障。如果故障需要人工处理，它就不是真正的无人值守 |
| **操作债务** | 当前每个故障模式都需要人工干预（"删除 `.forge/` 目录重试"）。每个这样的操作都是系统脆弱的证据 |
| **LLM 不可预测** | 与确定性软件不同，LLM agent 会产生不可预测的输出。内存污染、输出格式错误、长延迟——这些不是 if，是 when。自愈是 LLM 原生系统的必备能力 |
| **自洽** | ForgeOS 治理其他系统使其"自治"，但自己不能自治地处理自身的运行时故障——这个不对称性最终会限制其适用范围 |

### 建议的架构方向

```
SelfHeal 接口（internal/heal, 纯 Go 零依赖）

1. 可恢复故障的分类
   type HealableError struct {
     Err       error
     Severity  HealSeverity  // HealInfo | HealWarning | HealCritical
     Strategy  HealStrategy  // HealTruncate | HealRetry | HealFallback | HealReset
     Detail    string        // 可读的诊断 + 修复记录
   }

   Severity 决定了在运行日志中的可见度：
   - HealInfo:     "trace: truncated last line auto-removed" (stdout)
   - HealWarning:  "memory: entry truncated to 10KB (was 523KB)" (stderr + trace event)
   - HealCritical: "checkpoint: all backups corrupt, starting fresh" (stderr + trace event + metric)

2. 修复操作
   HealTruncate: 删除文件的最后不完整行（trace.jsonl）
   HealCrop:     将单条记录裁剪到大小上限（memory.Append 时）
   HealFallback: 使用次优数据源替代损坏的源（checkpoint → checkpoint.1）
   HealRetry:    自动重试最多 N 次（非可重试错误用 fallback）
   HealReset:    重建文件从头开始（最后手段——记录重建理由）
   HealSkip:     跳过损坏的记录继续（memory.Load 跳过损坏的 JSON 行）

3. 治愈日志
   .forge/heal.jsonl — 每条自愈操作写入一行：
   {"time":"...","severity":"info","action":"truncate",
    "file":".forge/trace.jsonl","detail":"removed incomplete last line (seq=47)"}

   用途：事后审计、自愈率统计、趋势发现（某种故障是否在增加）

4. 集成点
   trace.Open:     检测截断 → HealTruncate
   memory.Append:  检测超限 → HealCrop
   memory.Load:    检测 JSON 解析错误 → HealSkip（跳过问题行）
   checkpoint.Load:检测损坏 → HealFallback → HealReset
   runAgentPhase:  检测超时未设但延迟异常 → HealRetry（自适应超时）
```

### 边界情况

- **过度自愈**：自愈可能掩盖根本问题。如果 trace 总是被截断（底层 bug），自愈会静默处理每次——用户认为系统健康但实际数据在丢失。需要告警阈值：如果同文件在 24h 内被自愈 > 3 次，升级为 HealCritical
- **修复的幂等性**：重复修复不应产生累积影响。`HealTruncate` 再次运行在同一文件上——如果上一次修复已截断，再次检测应不再截断
- **修复的记录**：自愈操作必须是可审计的。`heal.jsonl` 提供了这个功能，但如果 heal.jsonl 本身损坏了怎么办？可以将其设计为内存缓冲 + 周期性 flush，但内存中的数据在崩溃时丢失
- **用户信任**：如果系统自动截断了 trace，用户可能不知道成本数据丢失了——这将影响 scorecard 可靠性。严重性足够的自愈应该在 stdout 中可见，而不仅仅是 heal.jsonl

---

## 方向五：架构度量趋势分析——从「交通警察」到「早期预警系统」

### 现状

当前 `arch-check.mjs` 的 8 项检查是**点检测的**（point-in-time pass/fail）：

```javascript
// arch-check.mjs — 每次运行的结果：
checkLayering:    PASS    // 此刻依赖方向正确
checkPackage:     PASS    // 此刻包大小 < 阈值
checkFanin:       PASS    // 此刻扇入 < 20
checkCognitive:   PASS    // 此刻认知负荷 < 阈值
checkAntiPattern: PASS    // 此刻无反模式命名
checkFunctionLen: PASS    // 此刻函数 ≤ 50 行
checkCircular:    PASS    // 此刻无循环依赖
checkDriftGuard:  PASS    // 此刻架构规则匹配代码
```

**所有这些检查都是瞬时的、无上下文的。** 它们告诉我**现在**架构是否健康，但无法回答：

- "扇入从 5 涨到 19 用了多久？是 1 个 sprint 还是 10 个 sprint 的渐进积累？"
- "上周认知复杂度一直稳定在 12，这周跳到了 18——这个 PR 引入了什么？"
- "哪三个包的扇入增长最快？它们是相关的吗？"
- "按这个趋势，我们还有多少个 sprint 就会触碰扇入上限 20？"

### 问题：三个具体缺失

**缺失 1：无法检测渐进退化**

最危险的架构退化不是一次 PR 引入大问题——而是每次 sprint 扇入增加 1、认知复杂度增加 2、包大小膨胀 10 行。10 个 sprint 后：

```
sprint 1:  fanin=10, cognitive=15, pkg_lines=340
sprint 2:  fanin=11, cognitive=17, pkg_lines=355
sprint 3:  fanin=11, cognitive=18, pkg_lines=370
...
sprint 10: fanin=19, cognitive=29, pkg_lines=490  ← 所有指标都在红线下但都逼近红线
```

这个系统在 sprint 1-9 全部 PASS，sprint 10 仍然 PASS（因为 490 < 500, 19 < 20），但已经是一个健康警示了。**没有任何信号提醒团队架构在恶化。**

**缺失 2：没有趋势驱动的早期预警**

If `pkg_lines` 在 100 天内从 200 涨到 490（斜率 +2.9 行/天），系统应当能在 480 行时就预警：

```
当前行为：pkg_lines=480 → PASS (480 < 500)
趋势预警：pkg_lines=480, 斜率 +2.9/天, 预计 7 天后触碰 500 红线
         → "⚠️ pkg 'orchestrator' at 480 lines, trending +2.9/day,
            will exceed 500-limit in ~7 days"
```

**缺失 3：没有跨指标关联**

扇入增长可能与包大小增长相关，也可能不相关。当前独立检查每个指标，没有交叉分析：

```
orchestrator 包:
  - 扇入: 15 (PASS, < 20)
  - 包大小: 460 (PASS, < 500)
  - 依赖数: 5 (未检查)
  - 修改频率: 高频 (未检查)

→ 综合分析：
  "orchestrator 扇入 15 + 包大小 460 + 依赖数 5 + 高频修改
   = 高风险重构候选。建议拆分出 internal/workflow 和 internal/loop。"
```

### 为什么需要

| 维度 | 理由 |
|------|------|
| **治理预防性** | 当前治理是**反应性的**（红线被突破才告警）。趋势分析是**预防性的**（红线将在 N 天后被突破，今天告警）。预防性治理才是真正的治理 |
| **进化可见性** | ForgeOS 的核心是 Evolve 循环。如果 Evolve 让架构逐渐恶化而无人察觉，这与它宣称的"不让架构腐化"自相矛盾 |
| **20 sprint 效应** | 一个项目经过 20 个 sprint（约 5 个月）的持续演化。如果没有趋势跟踪，架构健康是"盲飞"——只知道此刻没撞墙，不知道墙正在靠近 |
| **与 converge 集成** | 如果趋势告警是收敛的信号之一，Evolve 循环可以自动分派一个"重构 sprint"来修正恶化趋势，而不等人发现 |

### 建议的架构方向

```
Phase 1: 度量序列化（数据采集，增量）
  每次 arch-check 运行时，输出结构化 JSON 到 .forge/metrics/arch/<timestamp>.json：
  {
    "timestamp": "2026-07-01T10:00:00Z",
    "repo": "forge-core",
    "checks": {
      "layering": "PASS",
      "package": {
        "status": "PASS",
        "details": [
          {"pkg": "orchestrator", "lines": 460, "limit": 500},
          {"pkg": "cmd/forge",   "lines": 3831, "limit": 500},
          ...
        ]
      },
      "fanin": {
        "status": "PASS",
        "details": [
          {"pkg": "orchestrator", "fanin": 15, "limit": 20},
          ...
        ]
      },
      "cognitive": { ... },
      "function_length": { ... }
    }
  }
  这个文件很小（< 2KB），每次 CI 运行产生一个，保留最近 100 个。

  集成点：arch-check.mjs 加 --json 输出模式，checkPackage/checkFanin 等
  返回结构化数据而非纯文本。

Phase 2: 趋势计算（internal/trend, 纯 Go）
  type Trend struct {
    Metric    string      // "pkg_lines.orchestrator" | "fanin.orchestrator"
    Values    []DataPoint // 最近 N 个数据点（已降采样）
    Slope     float64     // 线性回归斜率（单位/天）
    R2        float64     // 拟合优度（0-1, 指示趋势的可信度）
    Forecast  int         // 预测触及阈值的剩余天数（-1 = 不收敛）
  }

  func ComputeTrend(metrics []MetricSnapshot, metricName string, limit float64) Trend
    - 至少需要 5 个数据点才计算趋势（冷启动时不产生误报）
    - 使用简单线性回归（不引入外部统计库）
    - R² < 0.3 时标记为 "no clear trend"（避免噪声数据产生告警）

Phase 3: 趋势告警集成
  type TrendAlert struct {
    Severity    string  // "info" | "warning" | "critical"
    Metric      string
    CurrentVal  float64
    Limit       float64
    DaysToLimit int     // -1 = converging away from limit
    Slope       float64
    Message     string
  }

  告警触发条件：
  - warning:  天数到限 ≤ 30 天 且 趋势显著（R² > 0.5）
  - critical: 天数到限 ≤ 7 天 且 趋势持续

  告警输出位置：
  1. arch-check 的运行输出（stderr）
  2. .forge/metrics/trend_alerts.jsonl（持久化历史）
  3. 可选的 Engine.OnTrendAlert 回调（注入到收敛信号中）

Phase 4: 治理闭环（可选，高级）
  当趋势告警达到 critical 时：
  - forge accept 仍然 PASS（当前架构合规，只是趋势不好）
  - 但 converge 信号增加 `trend_health: false` 字段
  - evolve 循环可以识别 "架构正在变差" 并自动规划重构任务
  - 重构任务进入 ROADMAP → 下次 implementer 处理 → 架构趋势改善
```

### 边界情况

- **冷启动**：新项目没有历史数据。在积累 5 个数据点（约 5 次 CI 运行）前，趋势系统静默
- **数据点采样**：每天 10 次 CI 运行产生 10 个数据点但趋势变化很小。应该降采样——每天取最后一个数据点。太多数据会使线性回归对短期波动过于敏感
- **不显著的波动**：包大小从 340 到 342（+2 行）是噪声。趋势计算需要最小绝对值变化（如 > 5%）才计入趋势
- **人工干预**：如果团队做了大规模重构（包大小从 460 降到 250），趋势线会出现断点。需要检测 level shift 并重新开始计算趋势斜率
- **跨包关联**：真正的价值在于"orchestrator 扇入增长是因为它被太多 phase 类型引用"。这需要调用图级别的分析，超出简单趋势

---

## 优先级矩阵（新五方向）

| 方向 | 影响面 | 实施成本 | 依赖前序 | 推荐 |
|------|--------|---------|---------|------|
| **1. 跨 Agent Prompt 注入防护** | **安全**: 极高 | 中 | 无（纯增量，不改变现有行为） | **最高优先**：系统最深层的安全盲区。在 24h 无人值守场景下，cross-agent 污染是时间问题而非 if 问题。实现成本低（内容边界标记+敏感模式过滤），收益极高 |
| **2. 置信度决策引擎** | **质量**: 高 | 中 | 方向一（置信度可跨 agent 标注复用） | **第二优先**：将系统从二元 PASS/FAIL 提升到置信度感知，直接解决"假收敛"和"假失败"两个极端。需要设计但不需要引入外部依赖 |
| **3. 多租户资源联邦** | **平台**: 中-高 | 高 | 无（但需要设计决策） | 第三优先：这是平台级能力，但在团队规模 > 2 前没有实际需求。推荐框架先行（trace 增加 tenant_id 字段），实现后置 |
| **4. 自愈层运行时** | **可靠性**: 中 | 低-中 | 方向二（自愈可作为置信度降级的消费者） | 第四优先：单个自愈操作成本极低（截断一行、跳过损坏记录），但需要设计统一的 HealableError 框架。推荐先实现 trace 截断自愈（成本 ~20 行），其余后续扩展 |
| **5. 架构趋势分析** | **质量**: 中 | 中 | 无（arch-check 已产出所有原始数据） | 第五优先：价值高但非紧急（架构不是几天内崩塌的）。推荐先规范化 arch-check 的 JSON 输出，趋势告警作为 v2.x 的特性 |

### Sprint 建议

```
Sprint n （3-5 人日）：
  - 方向一 Phase 1-2（内容边界标记 + 敏感模式过滤）
    —— 不改变 memory schema，只在 prompt 渲染层做
  - 方向四 Phase 1（trace 截断自愈）
    —— ~20 行 Go，消除最常见的操作故障

Sprint n+1 （5-8 人日）：
  - 方向二 Phase 1-2（置信度信号模型 + 加权聚合）
    —— 与现有 converge 路径并存（opt-in）
  - 方向五 Phase 1（arch-check --json 输出）
    —— 规范结构化数据，为趋势打基础

Sprint n+2 （视需求）：
  - 方向三 Phase 1（trace 增加 tenant 字段）
  - 方向四 Phase 2-3（memory 截断 + checkpoint fallback）
```

---

## 与已覆盖领域的边界区分

为确认真是增量，此文五方向与已有分析的关系：

| 方向 | 与已有分析的区分 |
|------|----------------|
| **Prompt 注入防护** | `hidden-feedback` §1 识别了三条反馈回路的风险，但将其归为「系统依赖 Agent 诚实」的哲学问题，没有提出工程化防护手段。本文提出具体的完整性层（起源标注/边界标记/敏感过滤） |
| **置信度决策** | `hidden-feedback` §4 识别了信号质量差异（cost 是高质量，RoadmapCompletion 是低质量），但停留在「信号有优劣」的判断层面。本文提出将信号置信度作为收敛引擎的一等公民——加权聚合而非二元 ALLOF |
| **多租户联邦** | North-star 架构提到了 Cost/Budget 和 IAM/Tenancy 服务，但那是分布式微服务的目标架构。本文关注的是单二进制运行时中增量实现租户隔离的具体代码路径 |
| **自愈运行时** | `seventh-wave-data-realism` §4 建议了故障注入测试（Chaos Engineering for Data），但那是测试层面——"验证系统在故障下的行为"。本文提出的是运行时自修复——"系统遇到故障后自动恢复并继续"，方向不同 |
| **架构趋势分析** | `self-testing-and-dogfooding.md` §5 提到 arch-check 的结构化输出，但角度是 CI 中 forge accept 的可靠性。本文提出的是时间序列趋势预警——将点检测升级为趋势检测 |

---

*分析日期：2026-07-01 | 基于第六轮全量扫描（此前 16+ 篇覆盖外的 10% 盲区）*
