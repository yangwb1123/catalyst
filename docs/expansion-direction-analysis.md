# ForgeOS 扩展方向分析

> 基于截至 Sprint 26 的全量代码库扫描（`forge-core` 13 Go 包 + `harness/` 全套执法器 + `.agent/` 声明骨架），
> 以资深架构师/产品经理视角识别 4 个高价值扩展方向。每个方向包含：为什么需要 · 现状基线 · 边界情况 ·
> 性能考量 · 建议的推进策略。

---

## 目录

1. [多 Agent 协商与仲裁协议](#1-多-agent-协商与仲裁协议)
2. [自适应测试自愈闭环](#2-自适应测试自愈闭环)
3. [跨项目知识迁移与全局学习](#3-跨项目知识迁移与全局学习)
4. [预测性资源管理与动态预算编排](#4-预测性资源管理与动态预算编排)

---

## 1. 多 Agent 协商与仲裁协议

### 为什么需要

当前 `forge-core` 的 agent 协作模型是**线性传递**：planner → implementer → [gate] → reviewer → [gate] → qa。Reviewer 发现问题时通过 `REQUEST_CHANGES` 触发定向 loop-back 跳回 implementer。但协作止步于此——如果 implementer 不同意 reviewer 的意见、或 reviewer 的判断本身就是误报（false positive），没有结构化仲裁机制。在 Sprint 26 的 dogfood 中，这个缺口真实出现了：implementer（acceptEdits 无 Bash）无法自验，被迫标记 ROADMAP 项完成，而 reviewer 在无 gate 信号下盲目重验，烧穿 budget。

- 真实自主运行需要处理**分歧**，而非仅单向「审→改」。
- 缺少仲裁意味着 reviewer 的 false positive 会让 loop 无限 oscillate（烧 loop-back budget 然后 abort），或 reviewer 的 false negative 直接放行有缺陷的增量。
- 随着 agent 数量增长（discover 阶段多 agent 并行调研、design 阶段多方案对比），分歧只会更频繁。

### 现状基线

- `orchestrator.go` 的 `agentOutcome` 实现了 reviewer → implementer 的**单向** loop-back（`REQUEST_CHANGES` → 跳回指定 phase）。
- `converge.go` 的 `review_status` 识别 "approved" 与否，但只用于收敛判断，不驱动协商。
- Pipeline 数据流（`phaseOutputLedger` / `gateLedger` / `reviewFindingsLedger`）为 reviewer 提供了客观闸门信号，但 implementer 没有渠道**回应** reviewer 的评审意见。
- `parallel.go` 的 fail-fast 机制在多 agent 并行时直接 cancel 整波，没有「部分完成、部分失败」的 graceful degradation。

### 建议的扩展方向

**结构化仲裁协议**——在 reviewer（或任何上游 agent）返回 REQUEST_CHANGES 时，提供三层 resolution path：

1. **自动 reconcilation**：当 reviewer 的修改建议是机械性的（命名、格式、测试遗漏），由 orchestrator 自动应用，无需再次调用 agent。这需要 reviewer 的输出包含**机器可读的修改指令**（而非纯自然语言）。
2. **结构化讨论回合**：当 implementer 和 reviewer 实质分歧时，引入有限轮次的「辩论」phase——两 agent 各自陈述理由，由第三个 agent（或 CTO agent）裁决。现有 `agentOutcome` 的 loop-back 已经可以复用为目标跳转，只需新增一个 `adjudicator` phase。
3. **人上升级**：当 N 轮协商仍无共识时，升级到 `human_gate`——但这不是 binary approve/reject，而是带着 implementer 和 reviewer 双方**结构化立场**请人类做定向决策。

### 边界情况

- **False positive cyclone**：reviewer 反复错误 reject。需在 loop-back budget 内加**按 agent 计费的否定计数**——同一 reviewer 连续 N 次 REQUEST_CHANGES 后被静默降级（不再 trigger loop-back）或升级给人。
- **协商淹没**：辩论回合中 agent 输出越来越长、越来越 defensive。需要**立场长度 cap** 和**相关性过滤**（超出 scope 的论点直接丢弃）。
- **循环依赖**：implementer A 的输出被 reviewer B 拒绝，B 的修改建议又被 implementer A 拒绝。死锁。需要 **max-debate-rounds** 硬上限 + 超时后默认维持 implementer 版本（fail-open 方向：代码先合入，争议留 ADR）。
- **并行协商**：多个 reviewer 同时审不同 phase 的输出。需确保仲裁上下文的 isolation（一个 phase 的 debate 不污染另一个）。

### 性能考量

- 每轮协商增加一次 LLM 调用（两 agent 各一次 + 仲裁一次）。`--max-agent-calls` 和 `MaxLoopBack` 已经提供总量护栏，但需要新增 `--max-debate-rounds` 独立控制。
- Trace 事件需要新增 `kind: "debate"` / `kind: "adjudication"`，带结构化元数据（立场摘要、裁决理由）。
- Memory 中应记录裁决结果（`KindDecision`），避免同一分歧在后续 iteration 中反复仲裁。

### 推进策略

1. 先在 `build.yml` workflow 中为 reviewer 新增 `writes_adjudication` 语义，让 reviewer 输出既包含自然语言评审、也包含结构化 diff 指令。
2. 实现 `AdjudicatorExecutor`——一个特殊的 AgentExecutor 实例，接收两份立场并输出裁决。
3. 扩展 `engine.agentOutcome`：在 `MaxLoopBack` 内增加辩论分支，而非仅单向 loop-back。
4. dogfood：用 reviewer 真实误报场景验证 false-positive cyclone 防护。

---

## 2. 自适应测试自愈闭环

### 为什么需要

ForgeOS 已经能检测测试缺口。`converge.go` 的 `CodeTestRatio` 在 roadmap 进度 > 30% 但 code-to-test 比例 < 10% 时发出 `⚠ test gap` 警告。但它**只报警不治**——缺口被记录在日志里，但系统不会自动补测试。

在 Sprint 23-26 的真 dogfood 中，这个缺口是真实的：implementer 写了功能代码但没写测试（或者测试写错了），reviewer 凭客观 gate 信号发现了问题，但修复只能靠 implementer 再次 loop-back 重写——而重写时 implementer 没有历史失败的上下文，可能重复同样的错误。

真正的自治系统应该能在**检测到缺口后自动诊断、生成修复方案、验证修复效果**，形成自愈闭环而非仅报警。

### 现状基线

- `harness/acceptance.mjs` 的 `test_pass` / `app_test_pass` probe 执行 `node --test` / `go test` 等，返回 PASS/FAIL/NA。
- `converge.go` 的 `Signals.CodeTestRatio` 从 git diff 统计 test lines / total changed lines。
- Pipeline 数据流中 `gateLedger` 将测试结果注入 reviewer prompt，让 reviewer 能做客观判断。
- `learning loop`（Sprint 26）已经收集了三维真数据（quality + latency + cost），但**没有回灌到测试生成**。
- `memory` 包能记录 gap/decision/lesson，但测试缺口不被记录为 Entry。

### 建议的扩展方向

**Test-Health Monitor + Auto-Healing Pipeline**，包含三阶段：

1. **Diagnosis Phase**（测试失败后自动插入）：不是简单 rebounce 给 implementer，而是运行一个专门的 `test-diagnoser` agent，分析测试输出 + 代码变更，输出结构化诊断报告：
   - 失败是测试本身的问题（flaky test、过时断言）还是实现的问题？
   - 如果是测试问题，修复方案是什么？
   - 如果是实现问题，需要改哪些文件？
2. **Remediation Phase**：根据诊断结果，分派给对应的 agent（implementer 改代码、或 QA agent 修测试）。
3. **Verification Phase**：修复后重新跑测试，验证 PASS + 确认 CodeTestRatio 改善。

### 边界情况

- **Flaky test oscillation**：测试 N 次中有时 PASS 有时 FAIL。需引入**重复执行**模式（`--flaky-retries 3`），仅有系统性的 FAIL 才触发诊断。diagnoser 应能识别 flaky 特征（timeout、race、顺序依赖）。
- **测试环境缺失**：`adapters/*.yml` 的 test 命令可能不存在（诚实 N/A）。诊断 phase 应跳过，不产生 false remediation。
- **测试本身被修改**：当 diagnoser 发现是测试断言过时（而非实现有 bug），修复方案可能导致「测试通过但功能错误」的假阴性。需要**契约验证**：diagnoser 确认测试修改不削弱覆盖率（`coverage_delta` 不应下降）。
- **空测试文件**：implementer 可能创建了测试文件但全是 skip/placeholder。diagnoser 应识别并标记。
- **跨语言测试**：`adapters/go.yml` 的 `go test` vs `adapters/typescript.yml` 的 `node --test`。diagnoser 需要语言感知。
- **无限修复循环**：diagnoser 提出修复 → 修复 FAIL → 再次诊断 → 再次修复 → ...。需硬上限 `--max-heal-attempts`（默认 2），超限后升级给人。

### 性能考量

- 每次测试失败触发 diagnoser = 一次额外 LLM 调用。在 `--max-agent-calls` 计数中单独分类 `kind: "diagnosis"`，便于成本归因。
- 诊断结果应写入 Memory（`KindLesson`），避免同一场景反复诊断（如「这个测试文件是 flaky，之前已确认」）。
- Trace 中新增 `kind: "diagnosis"` 和 `kind: "remediation"` 事件，带诊断摘要和修复文件的列表。
- 对于慢测试套件，diagnosis 阶段可能等待过久。需与 CommandExecutor 的 `timeout` 配合，diagnosis 设独立 timeout（如 120s）。

### 推进策略

1. 先在 `converge.go` 的 `CodeTestRatio` 警告后插入**建议诊断**的 log 输出（当前只打印 warning，不行动）。
2. 扩展 `build.yml` 增加可选的 `test_heal` phase（位于 qa 之后，当测试不通过时触发），用 `on_fail` loop-back 到 diagnoser（新 phase）。
3. 实现 `test-diagnoser` agent card（继承自 `qa`，但专注于测试诊断）。
4. dogfood 验证：故意引入一个测试 bug 看闭环是否自动修复。

---

## 3. 跨项目知识迁移与全局学习

### 为什么需要

当前 ForgeOS 的 `memory` 包是**项目隔离**的。每个项目的 `.forge/memory.jsonl` 只记录本项目内的 gap/decision/lesson。一个项目学到的东西（如「不要在某 ORM 版本用某查询模式」「某 lint 规则总是误报，需 suppress」）无法传递给其他项目。

但 ForgeOS 的愿景是**软件工厂**——一个控制平面管理多个产品。如果产品 A 发现了一个架构陷阱，产品 B 在相似场景下独立发现同一陷阱就是浪费。更严重的是，如果产品 A 做了一个架构决策（ADR），产品 B 在相似上下文中做了冲突的决策，长期就会产生「架构 drift」——两个同属 ForgeOS 生态的项目走向不可兼容的分叉。

`ADR 0003` 定义了 `agent-os` submodule 机制，但它是**治理资产共享**（agent cards / workflows / policies），不是**运行时知识共享**。下面这个扩展方向是运行时层。

### 现状基线

- `memory` 包：per-project JSONL，支持 KindGap/KindDecision/KindLesson 三种 Entry，`Supersedes` 机制支持修正，`Compact` 支持老化压缩。
- `internal/prompt/retrieve.go`：TF-IDF 风格检索器，但检索范围限定在**当前项目的 ADR**（`relevantADRs` 读 `docs/adr/` 目录）。
- `converge.go` 的 `RoadmapCompletion` 评估当前项目 ROADMAP。
- Checkpoint（`persist`）和 trace（`trace`）都是 per-process/per-project，没有跨项目聚合视图。
- 治理资产共享（ADR 0003）：submodule 机制设计就绪但未落地。

### 建议的扩展方向

**全局知识池（Global Knowledge Pool）**——一个可选的、跨项目共享的 Memory Store，分三层：

1. **模式库（Pattern Catalog）**：记录跨项目验证的架构模式、反模式、常见陷阱。每个 Entry 带**上下文签名**（project language / lifecycle stage / domain tags），供检索器按相似度匹配。当一个新项目的 `route --diff-files` 检测到某文件路径特征，全局检索器能返回「另一个项目在类似场景下遇到 X 问题，决策是 Y」。
2. **仲裁记录（Ruling Log）**：多 agent 协商的结果（方向 1）脱敏后写入全局池。当另一个项目触发相似分歧时，全局池直接提供前例裁决，减少 LLM 调用。
3. **演化预警（Drift Sentinel）**：跨项目对比 ADR 决策。当项目 A 在 ADR-004 中选了 PostgreSQL，而项目 B 在相似场景选了 MySQL，Drift Sentinel 在 discovery/design 阶段注入一个 advisory：「注意：姐妹项目 A 在此场景选了 Postgres（ADR-004），你们的技术选型不同，确认这是有意的 divergence？」

### 边界情况

- **隐私与敏感信息**：决策记录可能包含业务逻辑细节。需**脱敏层**（strip 项目特定名称、路径、API key）。默认不共享任何 Entry，仅显式 `--share` 标记的 Entry 进入全局池。
- **上下文不匹配**：项目 A 是 Go + event-driven，项目 B 是 Python + CRUD。A 的决策对 B 可能完全不适用。检索器需要**相似度阈值**（如 < 0.3 不返回），且下游必须标注「来自不同上下文，仅供参考」。
- **全局池膨胀**：100 个项目 × 每个 500 Entry = 50000 条。检索性能退化。需全局 Compact 策略 + 索引（按 tag/project/lifecycle 分片）。
- **冲突的前例**：全局池中一个场景有两种相反决策。检索器应返回多候选并标注置信度（如 70% 选 A，30% 选 B），让下游自行判断。
- **全局池可用性**：全局池是 optional 的外部依赖。不可用时 forge-core 必须 graceful degradation（回到仅本地 memory），不能硬阻塞。

### 性能考量

- 全局检索延迟：跨项目 TF-IDF 检索在 50000 条规模下应在 < 50ms（纯内存），否则需引入外部索引（SQLite / 嵌入式向量库——但这会打破 forge-core 的零依赖约束）。**推荐**：全局池不作为 forge-core 的一部分，而是 optional sidecar 进程或外部服务（Go 独立 binary，引入 `modernc.org/sqlite` 作为唯一依赖——属于 architect 的依赖决策）。
- 写入全局池应在后台异步进行（不影响主 workflow 延迟）。当前 memory.Append 是同步写文件；可扩展为写本地 + 异步复制到全局池。
- 全局池的 Compact 策略需支持跨项目协调（不能一个项目在 compact 时锁住全局池）。

### 推进策略

1. 不直接建全局池。先在 `memory` 包中增加 **Entry 元数据扩展**：`GlobalTags []string`、`ContextSignature map[string]string`、`ShareLevel`（private/project/anonymous）。这样即使全局池未落地，Entry 结构也准备好了。
2. 实现一个独立的 `forge-knowledge` CLI 工具（独立于 forge-core，可引入外部依赖），读取项目 memory.jsonl 并写入全局 SQLite 池。
3. 在 `prompt.Gather` 中增加可选的全局检索注入点：当 `FORGE_GLOBAL_KNOWLEDGE_POOL` 环境变量指向一个池地址时，检索并注入相关全局 Entry。
4. dogfood：用 url-shortener 和 go-taskd 两个 examples 验证跨项目模式匹配。

---

## 4. 预测性资源管理与动态预算编排

### 为什么需要

ForgeOS 当前的资源管理是**反应式硬边界**：

- `recursion guard`：FORGE_AGENT_DEPTH >= MaxDepth → 拒绝（硬 stop）
- `agent-call budget`：agent phase 计数 >= MaxAgentCalls → 拒绝（硬 stop）
- `output-size cap`：子进程输出 > MaxOutputBytes → 截断（hard limit）
- `cost budget`（run-level）：`BudgetExhausted` puller 在 spend >= cap 时 stop（硬 stop）
- `timeout`：CommandExecutor 的 context.WithTimeout（硬 stop）

这四个构成了**四维资源护栏**（Sprint 22 里程碑），但它们都是**阈值触发型的断崖式停止**。在真实 24h 自主运行中，硬 stop 是非常昂贵的——如果预算在 evolve 第 7 轮耗尽，前 6 轮的所有工作（agent 调用、gate 运行）全部浪费。更理想的行为是：系统在预算还剩 20% 时就开始**降级运行**（cheaper model、shallower evolve、skip non-critical gates），让有限的预算产出最大的价值。

`routing.go` 已经有 `BudgetAdjustTier`（spendRatio >= 0.80 → 降一级 tier），但这是**静态的、固定的**——它只看当前 spend ratio，不看这个 phase 的预期价值、不预测剩余工作所需资源、不区分「即将收敛」和「还有大量工作」。

### 现状基线

- `routing.BudgetAdjustTier`：spendRatio >= 0.80 时非 floor agent 降一级 tier。
- `routing.TierForScore` 的 budget_guard：spend >= 1.00 且 critical → escalate_to_human；spend >= 0.80 → 降一级。
- `orchestrator/budget.go`：`checkAgentBudget` / `checkRunBudget` 是纯计数/阈值检查。
- `cost.go`（cmd/forge）：`runBudget` 跟踪累计 spend，`BudgetExhausted` 返回 bool。
- `converge.go` 的零值设计：roadmap_completion 和 gates_green 是仅有的收敛信号，不反映**剩余工作量**。
- Memory 中的轨迹数据（trace.jsonl 的 duration_ms + cost_usd_micros + model）已包含预测所需的历史数据。

### 建议的扩展方向

**预测性资源编排器（Predictive Resource Scheduler）**——基于历史 telemetry 和当前状态，动态优化资源分配：

1. **剩余工作预估**：基于 ROADMAP 剩余项数 × 历史每项平均 cost（按 model tier 分桶），预估剩余 work 所需预算。trace 中的历史数据（`cost_usd_micros` + `duration_ms` × model tier）是输入。
2. **动态降级计划**：当预估剩余 work 超过剩余预算时，不等到 80% 才统一降一级。而是输出一个**分阶段降级计划**：
   - 前 20% 余量：成本最高的 phase（reviewer / architect）降一级。
   - 前 40% 余量：所有 agent 降一级 + evolve depth 减半。
   - 前 60% 余量：改为 advisory-only evolve（仅报告、不 action）。
   - 达 100%：改为 dry-run，不产生新 bill。
3. **价值导向的预算分配**：不是所有 phase 都有同等价值。reviewer 在低风险 CRUD 变更上的价值低于 security review。Scheduler 应能**按 phase 预期价值排序**，在预算紧张时优先保障高价值 phase（如 review + gate 的执行），跳过低价值 phase（如多余的 documentation-only agent）。
4. **预算借贷（Budget Borrowing）**：如果 evolve 第 2 轮非常 cheap（因为变更小），省下的预算可以「借」给后续更复杂的轮次，而非按 iteration 重置计数（当前 `forge evolve` 是 per-iteration 重置 agent-call 计数，但 run-level budget 是全局的——`BudgetExhausted` 是跨 iteration 累积的）。

### 边界情况

- **预估不准**：历史 cost per item 是 noisy 信号（简单 fix 和复杂 feature 的差距可达 10x）。需**置信度区间**（low/medium/high），低置信度时执行更保守的降级。
- **冷启动**：第一个 iteration 无历史数据。fallback 到静态规则（当前 `BudgetAdjustTier` 的 0.80 阈值）直到积累 N 个数据点。
- **预算突发耗尽**：某个 unexpected 的 phase（如 cluster startup timeout 导致 3 次 retry）突然烧掉大量预算。需**紧急降级**机制——立刻将剩余所有 phase 切到 Haiku + advisory-only，而非按原计划逐步降级。
- **零预算 run**：当 `--max-budget-usd 0` 时，所有预估都是 0。系统应直接转为 dry-run（不产生 bill），而非尝试执行然后 BudgetExhausted stop。

### 性能考量

- 预估计算是纯内存操作（读 trace.jsonl + ROADMAP md），应在 phase 之间异步完成，不阻塞主 workflow。
- 预估结果应写入 trace（`kind: "forecast"`），便于事后审计：为什么系统在 iteration 5 降级了？
- 分阶段降级计划应写入 checkpoint，确保 crash 恢复后不丢失降级状态。
- 当预估显示余量充足时，oversight low（不额外消耗性能）。

### 推进策略

1. 不直接做全量预估器。先在 `trace` 中确保每个 agent event 都有准确的 `CostUsdMicros` + `DurationMs` + `Model`（Sprint 26 已经为 claude 实现了，但 echo/dry/非 claude executor 还没有）。
2. 在 `converge.go` 增加一个可选的 `cost_forecast` criterion（不在 all_of 中执行，仅在报告中输出预估）。
3. 扩展 `BudgetAdjustTier` 为分阶段降级函数 `PredictiveTier(spendRatio, forecastRatio, phaseValue)`，接收两个比率（已花费 + 预估剩余）。
4. dogfood：在 url-shortener 的大幅变更上验证降级行为是否符合预期（先 cheap round、再 expensive round）。

---

## 总结

| 方向 | 复杂度 | 价值 | 前置依赖 | 核心风险 |
|------|--------|------|----------|----------|
| 1. 多 Agent 仲裁 | 中 | 高（解锁真正自治） | loop-back 已就绪 | False positive cyclone；无限辩论 |
| 2. 测试自愈 | 中高 | 高（提升产出质量） | acceptance gate + CodeTestRatio | Flaky 循环；测试环境缺失 |
| 3. 全局知识池 | 高 | 战略级（软件工厂核心） | Memory + ADR 0003 | 隐私；上下文漂移；网络依赖 |
| 4. 预测性预算 | 中 | 中高（节省 24h 运行成本） | Trace telemetry + BudgetAdjustTier | 预估不准；冷启动无数据 |

**建议优先级**：方向 1（仲裁）→ 方向 2（自愈）→ 方向 4（预算）→ 方向 3（知识池）。
仲裁和自愈直接提升 v2 的自治能力，且前置依赖最少；预算优化在 24h 运行场景的价值随运行时长线性增长；知识池是战略差异化，但复杂度最高、需要外部基础设施。
