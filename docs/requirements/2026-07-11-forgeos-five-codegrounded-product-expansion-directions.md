# ForgeOS — 五方向产品扩展分析（基于全局代码扫描）

> 本文以资深架构师/产品经理视角，对 ForgeOS 当前代码库（forge-core Go 运行时 + harness 治理层）进行全局扫描后，列出 5 个高价值、且在当前代码中确实尚不存在或仅存薄弱基础的产品扩展方向。每个方向均附「为什么需要它」的底线论证。

**前提声明**：docs/requirements/ 目录已有 70+ 篇分析文档，本文件力求与之不重复。如果有某个方向已在其他文档中被充分覆盖，则本文件会注明并另辟角度。

---

## 方向一：执行时间预算（Time-Budget Planning）—— 让自治运行真正可预测

### 当前状态
系统已有**成本预算**（`--run-budget-usd` 基于美元封顶）、**调用次数预算**（`MaxAgentCalls`）、**深度防护**（`FORGE_AGENT_DEPTH`）、**输出容量防护**（`--max-output-bytes`）。但**没有任何维度衡量或规划 wall-clock 时间**。

- `orchestrator.Engine` 有 `Timeout time.Duration`，但那是**单次 phase 的超时**，不是**整个 run/evolve 的时间预算**。
- `trace.Event.DurationMs` 记录了已完成 phase 的延迟——但这是**回顾性的**（后见），系统从不**预测性地**用它来规划后续应选什么模型、并行度、或迭代深度。
- `LoopEngine.MaxIter` 是安全上限，不是时间规划工具。
- `docs/analysis/strategic-expansion-v21.md` 和大量现有需求文档确实讨论过延迟/成本遥测，但都把延迟当作**观测指标**（scorecard p95），从未提出**执行规划输入**。

### 具体缺口
**没有「N 分钟内完成」的机制。** 一个 `forge evolve` 可能在 30 秒收敛（如果无 gap），也可能跑 60 分钟烧光配额。对真实用户来说，「花多少钱」和「等多久」是两大不可预测之源。当前 forge-core 只解决了前者。

### 建议实现方向（不做代码，只描述轮廓）
1. **Phase 级耗时估算器**：基于历史 trace 数据（`trace.jsonl` 中的 `duration_ms` + 模型名 + task_type），为每种 (model, task_type, phase) 组合维护一个移动平均耗时。新 run 起跑前，`forge preflight` 可以估算总耗时 = Σ(各 phase 估算值 × 预期迭代次数)。
2. **Time-budget 驱动机器选择**：如果估算总耗时超过 `--max-duration`，则自动选择更快的模型（Haiku 替代 Sonnet）、减少迭代深度、或启用并行（`--parallel`）来压缩时间。
3. **超时降级而非 abort**：到达时间预算后，不直接失败，而是进入「快速收敛模式」（完全走 cheap model + skip optional gates + 只检查已做 roadmap 项）。
4. **`forge preflight --time-estimate`**：在真跑之前输出 "Estimated: 8-12 min (opening buffer 3 min)"。

### 为什么需要
ForgeOS 的目标是「24h 无人值守」。如果用户只是吃个午饭回来检查结果，24h 和 6h 没有区别。但如果**总是比预期多跑 3 倍时间**，用户会失去信任。时间预算让自治系统像传统 CI 一样可预测——这是从「实验性工具」到「生产就绪基础设施」的必经之路。

---

## 方向二：跨工件一致性校核（Cross-Artifact Consistency Verification）

### 当前状态
ForgeOS 的治理体系是一套**逐阶段流水线**：Discover→Design→REVIEW→Build→Evolve。每个阶段产生工件（PRD / ADR / 设计方案 / 代码 / 测试），但**系统从不校核这些工件之间的一致性**。

代码观察：
- `converge.Signals` 检测 `RoadmapCompletion`（[x] 勾选率）、`GatesGreen`（闸门通过率）、`FileDelta`（改动匹配率）——但这些都是**过程指标**，不检查**工件内容一致性**。
- `harness/check.py` 校验 agent 卡引用、workflow 结构——但那是**治理层一致性**，不是**业务内容一致性**。
- 没有任何代码问：「PRD 中声明的功能点，是否真的在代码中实现了？」或「ADR 中的架构决策，是否与当前代码架构一致？」
- `internal/doctor` 有健康检查、`forge doctor --anomaly` 检测 checkpoint 趋势异常，但都不触及工件之间的内容一致性。

### 具体缺口
AI agent 可以在 PRD 中写了一堆功能、在设计文档中画了漂亮架构图、在 ADR 中做了决策——但最终代码只实现了其中一半。**系统完全意识不到这种偏离**，因为：
1. Roadmap 是 agent 自己给自己打勾（[x]），存在夸大倾向（`FileDelta` 交叉验证是改进但粒度太粗）。
2. 没有机制把 PRD 的需求清单 → 代码的 function/type/API 签名 → 测试覆盖范围 —— 这三者之间做双向追溯。

### 建议实现方向
1. **轻量级声明提取器**（非 LLM）：从 PRD markdown 中提取 `- [ ]`/`- [x]` 需求项（已有基础），从 Go/JS 代码中提取 export 的 function/type/API 签名（可利用现有 `harness/arch/scan.mjs` 的 AST 解析），从测试文件中提取被测函数名。三项对比得出「未实现的需求」和「无需求来源的代码」。
2. **架构漂移检测**：ADR 描述架构决策（如「shortcode 必须是纯函数无依赖」），`arch-check.mjs` 的 `checkLayering` 和 `checkPackage` 已能检测层间违规——但 ADR 中的约束目前**不是声明式规则**，而是散文。一个中间步骤是让写入 ADR 的 agent 同时输出一份机器可读的约束摘要(`docs/adr/<n>-<name>.constraints.json`)，后续 `forge validate --consistency` 将其与 `arch-check` 实测结果对比，差异即报警。
3. **跨阶段校验点**：在 Build→Evolve 边界插入一个一致性闸门（`consistency_violations == 0`），与 `complexity_violations`/`arch_violations` 并列，接受真正的声明式约束→代码验证流程。

### 为什么需要
流水线模型的最大隐患是**信息衰减**：100% 的 Discover 质量 → 80% 的 Design 精确度 → 60% 的 Build 完成度。没有一致性校核，系统会在用户不知情的情况下稳定偏离原始需求。这不是「未来优化」——这是 AI 生成代码的根本可靠性问题。人类开发者团队有 code review 来发现这些偏离；AI 自治系统需要等效的自动化机制。

---

## 方向三：多 Agent 协作式辩论/评审迭代（Collaborative Deliberation）

### 当前状态
当前 Agent 协作拓扑是严格的**管线（pipeline）**，不是**网络（mesh）**：

| 模式 | 机制 | 数据流 |
|---|---|---|
| 串行（默认） | `RunFrom` | Phase A → Phase B → Phase C |
| 并行（opt-in） | `RunParallel` + `Waves` | 同一波内独立 Phase 并发 |
| 评审（current） | `AgentVerdict` | Reviewer 输出 VERDICT→反馈给下一轮 implementer |
| 循环（evolve） | `LoopEngine` | Scan→Gap→Roadmap→Implement→Review→Evaluate→Scan |

这些都是有向、无环、单向的。**没有两个 agent 同时讨论一个共享工件并达成共识的机制。**

真实观察：
- `prompt_context.go` 的 `phaseOutputLedger` 是「前馈」而非「对话」——B 看到 A 的输出，但 A 看不到 B 的反馈（除非下一迭代）。
- `reviewFindingsLedger` + `verdictLedger` 支持 reviewer 的定向反馈进入下一轮 implementer——但这是跨迭代的，不能在**同一轮**内多轮协商。
- `internal/memory` 的 JSONL 是累积日志，不是对话媒介——没有引用/回复/共识的数据结构。

### 具体缺口
某些决策天然需要多角色迭代才能收敛：
- **架构决策**：Architect 起草方案 → Performance Engineer 指出性能瓶颈 → Architect 修改方案 → Security Engineer 追加威胁模型 → CTO 综合裁决。目前在一次 design.yml 的单趟中，每个角色只能看到前一个人写好的工件，无法来回迭代。
- **代码评审**：Reviewer 发现设计问题并 REQUEST_CHANGES → Implementer 修改 → Reviewer 重新审查。目前这通过 loop-back 跨迭代实现，但 loop-back 的代价是重跑整个 pipeline（包括无关 phase），且 reviewer 在下一轮会拿到全新的 fresh context，丢失前一轮的具体讨论线索。
- **需求优先级争议**：Product Manager 和 Architect 对某个功能在 MVP vs v2 有不同意见——系统没有结构化辩论机制。

### 建议实现方向
1. **结构化协作文件**（Deliberation Record）：每个需要多角色协商的 phase 可以产出 `.forge/deliberations/<topic>.jsonl`，格式类似 `{ round, from_role, content, references[], verdict? }`。各方追加条目而非覆盖。
2. **Spoke-Hub 仲裁者模式**：指定一个中性 agent（cto 或专用 reviewer）作为「仲裁者」，在每轮辩论后判断是否已收敛（deliberation rounds ≥ N AND last round 所有参与者无新异议）。
3. **有界辩论 budget**（防无限循环）：`--max-deliberation-rounds` + `--deliberation-timeout`，与 `MaxLoopBack` / `MaxAgentCalls` 并列的资源护栏。
4. **辩论结果固化**：达成共识的 deliberation 自动摘要为一则 `memory.Entry`，供后续迭代 consult 而不再重开辩论。

### 为什么需要
管线模型只能处理「已知正确的前置步骤」。「AI 自治软件开发」中最关键的工作恰恰是**不确定决策**——架构选型、API 设计、MVP 范围界定——这些都依赖多视角交叉验证。没有结构化的辩论机制，ForgeOS 只适合在「需求/架构已完全确定」的狭隘场景下做实现自动化，而无法处理更广泛的价值创造。

---

## 方向四：非二进制质量门控（Graduated Quality Scoring）

### 当前状态
ForgeOS 目前的所有 gate 都是**二值**的：PASS / FAIL / N/A。

```
gate.Result{Status: "PASS" | "FAIL" | "NA"}
converge.Signals{...} // 所有 bool / float64 最终汇聚到收敛: MET vs NOT MET
```

具体观察：
- `converge.evalOne` 中每个 metric 都是门限判断（≥ 阈值=通过）。
- `harness/acceptance.mjs` 的 `decide` 函数也是判 ACCEPTED / REJECTED。
- 甚至 `test_pass` 也是 `runCountedTest` exit 0 + count > 0。
- `risk.Classify` 输出离散四档（low/medium/high/critical）——但这是路由输入，不是质量评分。
- `scorecard` 的百分位延迟/成本数据被写入，但**不被任何后续决策消费**（除了 `HistoryTiebreak` 的粗粒度路由偏好）。

### 具体缺口
许多 AI 产出物不适合二值判断：
1. **PRD 质量**：不是「过/不过」，而是「覆盖度 70%，竞品分析 40%，市场研究 60%」——这些目前全无量化。
2. **架构设计**：不是「合格/重新设计」，而是「满足当前 lifecycle MVP 条件，但扩展性评级 B-，安全性评级 B+」——这些判断目前依赖 CTO 的人力裁决或 agent 自评（`CONFIDENCE: 85`）。
3. **代码质量**：不是只有「测试过的代码」，而是有「圈复杂度趋势、重复率、异常处理覆盖率」等渐变指标——`harness/arch/arch-check.mjs` 已经能测量一些结构性指标，但输出是二值 PASS/FAIL，而不是渐变评分。
4. **重构必要性**：不是「文件超 500 行了必须拆」的硬触发，而是「这个文件 400 行但圈复杂度高、频繁被改、是本迭代的热点」——这是渐变决策。

### 建议实现方向
1. **评分维基表（Quality Rubric）**：在 `.agent/policies/quality.yml` 中声明每个质量维度及其评分标准（如 `readability` 有 4 级：1=不可维护 / 2=只有作者能懂 / 3=有注释但散乱 / 4=规范且自文档化）。评分标准是散文但归一化为 1-4 整数。
2. **消费端渐变决策**：质量评分不直接阻断流水线，而是驱动：
   - 路由（低质量代码→更贵的 reviewer 模型来审稿）
   - 任务派生（`internal/migrate` 的补债任务类似：`test_coverage=63%`→派生 `backfill-tests` 任务）
   - 收敛条件变体（`forge evolve` 的 stop condition 可以接受 "quality ≥ 3 AND no gate red" 而非纯二进制）
3. **agent 自评与 harness 实测的交叉验证**：agent 自评 `readability: 4`，但 `arch-check` 测出 `function_length_violations > 0`→系统自动降级评分为 2 并标注「agent 自评与客观测量不符」。

### 为什么需要
软件工程中最重要的判断不是「做完了吗」，而是「做得怎么样」。如果 ForgeOS 只能在二值空间里运转，它会把大量「60% 就绪但可合并」的工作卡住，或把「99% 就绪但有一个 lint 违规」的工作误判为不可发布。渐变质量评分让系统做出更接近人类工程经理的判断——它不是一个新功能，而是让现有所有功能更智能的基础设施。

---

## 方向五：可观测性驱动的自适应治理（Observability-Driven Adaptive Governance）

### 当前状态
ForgeOS 的治理强度由 `mode × lifecycle` 静态决定：

```
mode.go:Effective(mode, lifecycle) → Policy{Gates, ReviewDepth, DiscoverDepth, ...}
```

这个策略在 run 开始时固定，**在整个 run 期间不变**。系统不根据 run 中间的实际表现动态调整治理强度。

具体观察：
- `orchestrator/mode_gating.go` 的 `skipByMode` / `gatesFor` / `reviewStageSkipped` 全部是**一次性决策**（在 `Engine.Run`/`RunFrom` 开始时查一次 `ModePolicy`）。
- `converge.Signals` 积累了真实数据（`GatesGreen`、`RoadmapCompletion`、`FileDelta`、`CodeTestRatio`），这些数据**只用于收敛判决**，不影响治理策略本身。
- `LoopEngine.OnIteration` 是一个 perfect hook 来注入自适应逻辑，但目前只用于 checkpoint + trace——没有谁检查「刚刚这轮花了 $3.50 但只推进了 5% roadmap，是不是治理太松/太严了？」。
- `internal/trace` 收集了完整事件流，`scorecard` 计算结果，但**没有反馈回路把这数据写回治理决策**。

### 具体缺口
**静态治理意味着：不是过严就是过松。** 在 `engineering` 模式下，所有 gate 全开、所有评审全跑。如果项目实际很健康（测试覆盖率高、历史缺陷率低），这种严格度是浪费（耗时、烧钱）。如果项目积累了大量架构债务，`balanced` 模式可能太宽松，导致缺陷放行。

一个人类工程经理会**根据最近趋势调整审查力度**：如果团队最近代码质量好就降低审查频率，如果出现了安全漏洞就提升审查强度。ForgeOS 完全有能力做同样的事，但完全不这么做。

### 建议实现方向
1. **治理反馈循环**：在 `LoopEngine` 中（`OnIteration` 后），注入一个可选的治理调节器，检查最近 N 轮收敛信号的趋势：
   - 趋势健康（连续 3 轮 `GatesGreen` + `RoadmapCompletion` 稳步上升 + `CodeTestRatio > 0.3`）：自动降低治理严格度一级（如果非 production）。
   - 趋势恶化（`FileDelta` 一直很低但 `RoadmapCompletion` 自报很高——即 agent 可能夸大）：自动提升治理严格度，增加 gate、强制使用更贵模型、追加安全评审。
2. **治理解耦 → 巡检频率而非 gate 开关**：与其把 `security-review` phase 设置为 `optional_for: [balanced]`（全有或全无），不如设置一个**采样率**：在治理宽松时，security-review 的触发概率为 20%而非 0%。
3. **异常检测告警 → 自动收紧**：`internal/doctor/anomaly.go` 的 `DetectAnomalies` 已能检测 checkpoint 序列异常（降级趋势）。将其输出接回 `mode.Effective()` 的上游——当检测到「连续 checkpoint 显示 roadmap 停滞且 gate 红色增加」时，自动升级治理等级。
4. **透明可观测性**：每次治理等级或采样率的自动调整，都写入 trace（`Event{Kind: "governance_adjust", Status: "tightened"/"loosened", Reason: "3-round gate-red>30%"}`），并可在 `forge status --history` 中查看。

### 为什么需要
静态治理是不经济的：它在所有场景下施加相同的成本/时间开销，而不区分风险高低。ForgeOS 的目标是 24h 自治，如果 24h 都在 `engineering` 全开模式下跑，对小项目来说成本和时间是灾难性的；如果一直跑 `explorer` 模式，对关键安全修复又不够。自适应治理是唯一能让系统在**广泛的项目类型和风险等级下都高效且安全**的方案——它是从「固定策略的工具」到「真正智能的自治系统」的质变。

---

## 总结优先级矩阵

| 方向 | 创新性 | 用户可感知价值 | 实现成本（估算） | 与现有架构吻合度 |
|---|---|---|---|---|
| ① 时间预算规划 | ★★★★ | ★★★★★（消除不确定性） | 中（基于现有 trace 数据） | 高（接现有超时/预算机构） |
| ② 跨工件一致性 | ★★★★★ | ★★★★（防 AI 夸大/偏离） | 高（需设计新扫描器+约束格式） | 中（利用 arch-check 基础） |
| ③ 多 Agent 协作辩论 | ★★★★★ | ★★★★★（解锁更高难度决策） | 高（全新协作基元） | 中（接现有 verdict/loop-back） |
| ④ 渐变质量评分 | ★★★★ | ★★★★（更精准的收敛判断） | 中（声明式 rubric + 评分消费） | 高（替换现有二值 gate 外部接口） |
| ⑤ 自适应治理 | ★★★★★ | ★★★★★（经济+安全兼得） | 中（反馈回路接 trace+converge） | 高（利用现有 mode 机制扩展） |

五个方向中，**① 和 ⑤ 的性价比最高**——它们利用已有的基础设施（trace/scorecard/converge/mode），提供显著的端到端价值改进。**③ 是最具雄心但实现成本也最高的**——它挑战了 ForgeOS 当前的核心执行模型（pipeline→mesh）。**② 和 ④ 则是治理深化的自然演化方向**，填补了从二进制治理到可量化治理之间的鸿沟。
