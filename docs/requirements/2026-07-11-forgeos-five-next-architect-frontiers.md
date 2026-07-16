# ForgeOS — 五条下一前沿扩展方向 (Architect + Product 视角)

> 全局扫描 commit `b0c80e4` 后的当前工作树（forge-core 18 Go 包、harness 全套闸门、
> 5 个 workflow、12 agent 卡、已通过 `forge accept` 的完整治理）。
> 本文从资深架构师/产品经理角度，识别 5 个代码层面可证、现有 50+ 份需求文档
> 均未充分覆盖的高价值扩展方向。每个方向附：为何重要（产品价值）· 代码基础 ·
> 实现轮廓 · 成功标准。

---

## 方向一：multi-project fleet orchestration（forge-fleet 控制面）

**当前状态。** 每个 ForgeOS 项目是独立的孤岛：`forge-init` 复制全套治理资产到单个仓库，
项目之间无共享调度、无跨项目预算、无策略继承、无聚合可观测。
`examples/url-shortener` 和 `examples/go-taskd` 两个 dogfood 项目共存于本仓，
但 `forge run`/`forge evolve` 只能一次操作一个。`internal/migrate` 的 `explorer→engineering`
迁移也是项目级的，不存在"把 50 个 explorer 项目统一升级到 engineering"的批量操作。

**为何需要。** ForgeOS 的产品定位是"A-native 软件工厂的操作系统"（类比 Kubernetes）。
但单项目孤岛模式让它的价值止于个人开发者/单体仓库：
- 组织级治理要求策略继承（CTO 定一条"所有生产项目必须开启 security gate"→自动下发）
- 跨项目调度要求共享 agent 池（而非每个项目独享一套 claude 凭证和配额）
- 聚合 telemetry 要求统一数据面（哪个项目在烧钱？哪个项目测试覆盖率在下降？）
- `forge-fleet` 是 ForgeOS 从"个人工具"跃升为"平台产品"的必经之路

**代码基础。** `internal/mode` 的 mode×lifecycle 策略模型是声明式、可序列化的；
`internal/migrate` 展示了策略驱动迁移的模式。`internal/risk`、`internal/routing`、
`internal/converge` 全部是纯 Go 标准库零依赖——理论上可嵌入一个 fleet 控制面进程
而无需任何 runtime 依赖。`harness/scaffold/forge-init.mjs` 的模板复制逻辑
已演示了"治理资产作为模板"的模式。

**实现轮廓（非代码，仅方向）。**
1. **`internal/fleet` 新包**——顶层 `Fleet` 类型，持有项目清单（本地目录或 remote URL）
   及其 `project.yml` 快照；提供 `Fleet.PolicyOverride(key, value)` 批量覆盖，
   以及 `Fleet.AggregateTelemetry()` 从各项目 `.forge/` 目录收集 cost/trace 数据。
2. **`forge fleet` CLI 子命令**——`forge fleet list`（列项目+状态）、
   `forge fleet policy set --gate security --enforce block --match lifecycle=production`
   （策略继承，按 selector 匹配项目子集）、`forge fleet migrate --to engineering --all`
   （批量升级）。
3. **架构约束**——fleet 控制面**不**侵入项目内 workflow 执行（每个项目仍独立 `forge run`），
   只做治理数据面的聚合与策略下推，保持安全隔离。

**成功标准。**
- `forge fleet` 管理 ≥3 个项目且 `forge accept` 保持 ACCEPTED
- 跨项目策略覆盖可独立验证：fleet 写一条策略→子项目 `forge validate` 报告已遵从
- 聚合 telemetry 报告含真实 cost/latency/quality 数据（非 mock）

---

## 方向二：历史回放与离线仿真引擎（replay & simulation sandbox）

**当前状态。** `internal/trace` 记录了完整的结构化事件流（iteration/agent/gate/decision，
每行 JSONL）。`internal/routing/scorecard.go` 有 `HistoryTiebreak`。
`internal/converge` 有 `Signals` 全字段。但**没有任何方式能回放历史 trace、
测试新路由策略、对比不同 prompt/mode 配置对同一个已有 trace 的影响**。
每次策略变更必须等下一次真 agent 运行才能看到效果——要么烧真钱，要么盲改。

**为何需要。** 这是 ForgeOS 自身持续演化的核心基础设施瓶颈。目前：
- 想测试新的 mode×lifecycle 策略组合？必须跑一次 `forge run`（真 agent 烧钱）
- 想评估换一个 routing 算法（比如从 weighted_sum 改成 neural）？必须上线对比
- 想诊断一次失败的 evolve 循环为什么没收敛？只能看 trace JSONL，不能"重放 debug"
- 没有离线仿真，ForgeOS 的自我改进（Eval→记分卡→Router 闭环）就永远滞后于真实运行

**这一点与"Learning loop"（已在 Sprint 26 实现 cost/latency/quality 三维真数据）的区别：**
Learning loop 是**在线**闭环（真跑→记分→路由）。而离线仿真是**离线**沙箱
（历史数据→仿真路由→预测结果→策略决策），两者互补。没有离线仿真，Learning loop 的
每次迭代都依赖一次真 agent 调用，导致改策略的成本=改一次跑一次验证一次。

**代码基础。** `internal/trace` 的 JSONL 格式完备（duration_ms/cost_usd_micros/model/verdict
全字段）；`internal/converge` 的 `evalOne` 是纯函数（signals in → verdict out），
可直接复用为仿真检视器；`internal/routing` 的 `TierFor` 是确定性函数，
给定 agent+mode 总是输出同一 tier——仿真可精确预测路由决策。
`internal/mode` 的 `Effective` 也是纯函数。

**实现轮廓。**
1. **`internal/sim` 新包**——`Sim` 类型，接受一段 trace 事件流 + 一个"策略配置"
   （mode×lifecycle 参数、routing policy 参数、converge threshold 参数），
   重放 trace 中的 agent 事件并在每个决策点**预测**（而非重复执行）路由决策和收敛信号，
   输出"如果当时用了这个策略，结果会怎样"的报告。
2. **`forge simulate` CLI**——`forge simulate --trace .forge/trace.jsonl --mode engineering`
   （实际 trace 用 balanced 跑过，仿真 engineering 会怎样），对比"实际收敛？多花了多少钱？
   路由到更贵模型的频率？"
3. **架构约束**——仿真引擎**只读不写**（不修改代码不调用 LLM），
   可并行跑多组配置做对比。`internal/routing` 和 `internal/mode` 保持纯函数以可被仿真调用。
4. **前置条件**——`internal/trace` 的每个 Event 需要携带该事件发生时刻的完整策略快照
   （mode/lifecycle/gate-set/coverage-threshold），否则仿真无法区分"行为差异源于策略变化
   还是外部因素"——这是一个当前 JSONL 格式中缺失的字段，需扩展。

**成功标准。**
- 用 Sprint 25-26 的真 claude trace 作为 fixture，仿真 `--mode engineering` 能输出
  合理的"如果当时用 engineering 模式，reviewer 相位会被执行"的差异报告
- 仿真结果与真实运行在已知可预测维度上一致（比如路由决策：仿真 TierFor 应与真实记录一致）
- `forge simulate` 可同时跑 5 组策略参数做对比产出表格

---

## 方向三：结构化知识挖掘与跨会话学习（knowledge mining & cross-session learning）

**当前状态。** `internal/memory` 提供 JSONL 积累日志——每次 append 一条 `MemoryEntry`，
包含 `Kind`、`Topic`、`Content`、`Source`、`Iteration`。`internal/prompt/retrieve.go`
提供 TF-IDF 检索。但这个积累日志**缺乏任何结构化挖掘**：
- 没有跨会话的模式识别（"过去 10 次 evolve 循环中，security-review 相位总是找到
  同样的 token 泄露问题？"）
- 没有自动摘要与去重（同一类 gap 被记录了 8 次，每次略有不同，但从未被合并为一条
  结构化发现）
- 没有学习反馈（"上次我们用 model A 做 implementer，gate pass rate 是 70%；
  这次用 model B 是 85%——系统应自动识别并建议路由策略调整"）
- `Scorecard` 的 `quality_score` 字段由 `scorecard_wind.go` 写入但从未被 `memory`
   自动消费——学习循环的两个数据源（trace 的质量分 + memory 的知识积累）是孤立的

**为何需要。** ForgeOS 的核心价值主张之一是"随使用增长的数据飞轮"（north-star §5）。
但目前的数据飞轮是平的：
- `Trace` 是事件流——适合审计，不适合学习
- `Memory` 是日志——适合回顾，不适合推理
- `Scorecard` 是评分——适合路由，不适合解释

三个孤岛之间没有结构化知识层来回答**系统级问题**："这个项目的历史演化中，
什么类型的 bug 反复出现？什么类型的架构改动一直延迟？哪类任务在哪个模型上表现最差？"
没有这一层，ForgeOS 的 "Eval→记分卡→Router 闭环"（ROADMAP v3）就只是一个
单维度的 feedback loop（gate pass/fail → tier up/down），而不是真正的**系统自我理解**。

**代码基础。** `internal/memory` 的 `Query` 已有 topic/kind/source 筛选能力；
`internal/trace` 的 Event 有完整的 agent/phase/model/cost 归因；
`internal/routing/scorecard.go` 有按 model×task_type 聚合的 schema。
缺少的就是一个从原始 trace+memory 数据**挖掘模式**的层。

**实现轮廓。**
1. **`internal/learn` 新包**——三类挖掘策略：
   - Pattern miner：扫描 memory 中同类 topic 的频率和趋势（"topic X 在最近 5 次
     iteration 中出现频率递增"→输出一条 alert）
   - Cross-session correlator：关联 trace 中的模型选择与 gate pass rate
     （"model A + task_type=implementation 的 gate pass rate 比 model B 高 15%"→
     输出建议到 scorecard 或路由策略）
   - Anti-pattern detector：从收敛失败的 trace 段中提取共同特征
     （"当 gate_set 包含 arch+security 且 mode=explorer 时，某 gate 总是 N/A→
     建议 explorer 模式不要声明该 gate"）
2. **`forge learn` CLI 子命令**——`forge learn patterns`（扫描已积累的 memory，
   输出发现的模式）、`forge learn correlate`（关联 trace 和 scorecard，
   输出模型效能对比）、`forge learn suggest`（综合发现，输出策略调整建议，
   但不自动修改——保持人审最高杠杆原则）。
3. **架构约束**——learning 层是**只读分析面**，不写 memory/trace/scorecard，
   它的输出只是建议（人类或后续 loop 决定是否采纳）。不引入外部 ML 依赖
   （pattern mining 基于统计分析，非 NN/embedding）。

**成功标准。**
- 在 ≥3 次 `forge evolve` 后的 memory.jsonl 上运行 `forge learn patterns`，
  至少输出 1 条有意义的时间序列模式（"topic 'config-migration' 出现频率
  每 iteration +20%"）
- `forge learn correlate` 能输出真实的 model×task_type 效能对比表，
  字段与 scorecard schema 一致
- `forge learn suggest` 输出的每一条建议都附有可追溯的证据
  （引用具体的 trace 行或 memory 条目的 seq）

---

## 方向四：分级自愈与逐步升级的故障响应（graduated self-healing）

**当前状态。** `internal/orchestrator/exec_error.go` 定义了 5 种故障类型
（`KindConfig`/`KindTimeout`/`KindFailed`/`KindRecursionLimit`/`KindOverloaded`）
和简单的二元响应：`Retryable()` 决定重试或终止。`MaxRetries` 控制重试次数。
`loop.go` 的 `on_fail` 支持定向 loop-back（跳回 target_phase）。
`backoff.go` 实现了指数退避。但整套响应是**单级**的：
- 超时→重试→耗尽→终止，中间没有"换一个模型试一试？"
- `KindFailed`（agent 非零退出）直接终止，没有"要不要换一个更有经验的 agent 角色？"
- `KindConfig`（二进制缺失/环境问题）直接终止，没有"分发一个修复脚本？"

**对比真正的自愈系统（如 Kubernetes 的 pod 恢复 → node 驱逐 → 集群扩容）：**
ForgeOS 的自愈应该是一层层升级的——先轻量重试、再换策略、再换资源、最后找人类。
单级 retry→abort 在 24h 无人值守场景下不够用。

**为何需要。** Sprint 24-26 已经证明 ForgeOS 可以真 claude 端到端闭环。
但要宣称"24h 自治开发"（ROADMAP v3 的核心承诺），系统必须能在**无人干预**下
处理大部分运行时故障：
- 如果 claude API 返回 529（overloaded），应该退避重试，而不是 abort
- 如果 implementer 用 Sonnet 反复写不出正确代码，应该自动升级到 Opus 再试一次
- 如果 reviewer 拒绝 approve 一个实现，除了 loop-back 给 implementer，
  还可以考虑让 planner 重新拆任务试试
- 如果所有 agent 阶段都因为一个环境问题失败，应该留一个清晰的 escalation 记录
  给第二天的人类 operator，而不是静默停放

**代码基础。** `exec_error.go` 的 `ExecKind` 分类系统是自愈升级策略的自然切入点——
不是修改分类体系，而是在 `MaxRetries` 耗尽后插入一级升级策略注册表。
`engine_build.go` 的 `phaseTierResolver` 已演示了"动态调整模型 tier"的模式；
`internal/backoff.go` 有退避基础。

**实现轮廓。**
1. **`internal/heal` 新包**——`RemediationPlan` 类型，包含一个有序的重试/升级策略链：
   - Tier-1：重试（同模型，退避）——已实现
   - Tier-2：升级模型（Sonnet→Opus，Haiku→Sonnet）——需新逻辑
   - Tier-3：升级 agent 角色（implementer→planner+implementer 协作）——需新逻辑
   - Tier-4：升级 prompt（注入更多上下文，附上前次失败的 gate 输出）——`prompt_context.go`
     已有 `phaseOutputLedger` 可复用
   - Tier-5：升级模式（从当前 mode 切换到 engineering 以获得更严格的路由）——新逻辑
   - Terminal：写入 trace 的 `decision` event 标记 unrecoverable + 留 clear escalation 记录
2. **注册表与优先级**——每类 `ExecKind` 有一个默认的升级链（可被 `project.yml` 覆盖），
   例如 `KindOverloaded` 不需要升级模型（overload 不是模型问题），
   只需要更长的退避和更多重试。`KindFailed` 可能需要升级模型或角色。
3. **接入点**——`orchestrator.go` 的 `runAgentPhase` 在 `MaxRetries` 耗尽后、
   终止 run 之前，插入 `internal/heal` 的策略引擎。如果某一级升级策略
   （如升级模型）被触发，重新执行当前 phase 而不是终止。
4. **架构约束**——自愈不改变 workflow 声明语义（每个 phase 的 `on_fail` 仍优先），
   不改变 safety override（critical risk 不降级）。所有自愈决策记录到 trace 的
   `decision` event 类型以便事后审计。

**成功标准。**
- 注入模拟故障（`ExecError{Kind: KindFailed}`），确认系统自动尝试了模型升级
  后再终止（而非直接终止）
- 生产 environment 下 `forge run build` 遇到 claude 529 时自动退避重试
  （而不需要 operator 手动干预）
- 每一次升级决策在 `trace.jsonl` 中有完整的归因记录
- 所有升级链可被 `project.yml` 中声明覆盖

---

## 方向五：运行时架构漂移检测（runtime architecture drift detection）

**当前状态。** `harness/arch/arch-check.mjs` 执行 8 项检查，全是**静态分析**：
layering（import 方向）、package（文件/导出数）、fanin（生产导入数）、
cognitive（根模块数）、anti-pattern naming（目录名检查）、function-length、
circular dependency、drift-guard（`.arch/rules.yaml` vs `policies.yml` 一致性）。
这些都只能在文件系统快照上跑，不涉及运行时行为。

但 ForgeOS 的 architecture 声明远比 import 图丰富：
- `design.yml` 的输出包含 latency budget（性能预算）、threat model（威胁模型）、
  consistency strategy（一致性策略）
- `review.yml` 的 `performance-reliability-review` 输出了 `performance-budget.md`
- 架构声明中有：预期的并发模型、重试策略、缓存一致性方案、部署策略

**没有任何机制验证**这些架构声明在运行时是否被遵守：
- "性能预算声明 P99 latency < 100ms"——实际系统是否满足？没有持续测量
- "架构师设计的是 Redis 缓存一致性方案"——代码里是否用对了？不知道
- "安全评审要求所有写操作幂等"——实际 API 是否幂等？没有自动验证

**为何需要。** 这是 Arch-Check 的自然进化。当前 Arch-Check 只验证**代码结构**
（import 图、文件名、函数长度），不验证**架构契约**（性能、安全、一致性）。
ForgeOS 的架构流程（design→review→build→evolve）产生了大量架构声明，
但这些声明在 build 和 evolve 阶段**根本没有被喂养回验证系统**。
结果是：架构评审通过的假设，可能在实现后立即被违反，而系统不会知道。

对于 AI 自治开发的场景尤其关键：agent 在 implement 阶段可能会比人更容易偏离
架构决策（因为它不像人类开发者那样"记得架构评审时说过的约束"）。
如果系统不能在实现后自动验证架构约束，架构评审就只是一次性文档工作而非持续护栏。

**代码基础。** `internal/asset` 的 Workflow/Phase 类型可承载架构声明的元数据；
`internal/converge/signals` 已有 `CodeTestRatio` 和 `FileDelta` 这种与实现行为
相关的信号；`internal/risk` 的 `FromChangedPaths` 能感知改动影响范围。
缺少的是一层"把架构声明翻译成可验证的运行时断言"的桥梁。

**实现轮廓。**
1. **`internal/drift` 新包**——三类运行时漂移检测器：
   - **Latency budget verifier**：利用 `internal/trace` 已有的 `duration_ms`，
     对比 review.yml 中 `performance-budget.md` 声明的延迟上限，
     输出"声明的 P99 latency < 100ms，实际测量到 320ms（第 7 个 trace event）"
   - **API contract verifier**：从 ADR 或 architecture 文档中提取幂等性/一致性
     声明（token 格式），与实际调用模式对比（例如：日志中发现某 API 的重试
     导致非幂等的副作用）
   - **Architecture compliance reporter**：定期快照 `.agent/ARCHITECTURE.md` 中的
     结构化声明（比如"依赖方向：domain→application"），与 `arch-check` 的最新
     结果对比，报告架构声明与代码实现之间的差异
2. **接入点**——`forge evolve` 的 scan phase 原生就是做"扫差距"的；将 drift 检测
   作为 scan 的一个新维度（类似 security/performance/architecture 扫描），
   输出 `drift-report.md` 作为 gap-analysis 的输入。
3. **声明格式**——扩展现有的 `.agent/architecture/` 目录结构，
   增加一个机器可读的 `contracts.yaml`（非取代现有散文 ADR），
   用结构化格式声明性能预算、幂等性要求、一致性级别。
4. **架构约束**——drift 检测是只读的（不修改代码），它的输出只是 gap 分析的一个输入。
   不阻止 build 或 deploy（那是安全闸门的职责），只提供可观测性。

**成功标准。**
- 在已有的 dogfood 应用（`examples/url-shortener`）上运行 drift detector，
  至少输出 1 条真实的运行时偏离（例如："声明的 latency budget < 200ms，
  实际某 handler 耗时 350ms"——可从 trace.jsonl 的 duration_ms 证实）
- `forge evolve` 的 scan phase 集成后，自动产出 `drift-report.md`
  且不破坏现有 scan 的输出格式
- `contracts.yaml` 格式可被 >1 个 detector 消费（不止 latency，还有幂等性）

---

## 总结：5 个方向的关联与排序

| 方向 | 价值定位 | 前置依赖 | 风险 | 建议优先级 |
|---|---|---|---|---|
| ① fleet orchestration | 从单项目→平台级产品 | 无（纯新功能） | 设计复杂度高，可能引入项目间耦合 | P1（产品跃升） |
| ② replay & simulation | 系统自身安全迭代的基础设施 | trace 格式扩展 | 仿真结果的可靠性易受质疑 | P1（基础设施） |
| ③ knowledge mining | 从"记录"→"理解"的数据飞轮 | memory 积累数据量 | 挖掘质量不确定，依赖数据量 | P2（数据飞轮） |
| ④ graduated self-healing | 24h 无人值守的可靠性前提 | exec_error 分类已齐 | 升级策略可能导致意外行为 | P2（可靠性） |
| ⑤ runtime drift detection | 架构治理从 static→runtime | arch-check 静态层完备 | 架构声明机器可读化成本高 | P3（架构深化） |

**方向①和②是最高杠杆**：①让 ForgeOS 从"我的工具"变成"我们的平台"；
②让 ForgeOS 自身能低成本安全迭代——两者合起来决定 ForgeOS 能否
从个人级的实验性系统演变为组织级的生产平台。方向③④⑤依次深化。

所有 5 个方向遵循 ForgeOS 的既有纪律：纯 Go 标准库零依赖、不破坏现有 workflow
语义、新功能默认只读/dry-run、保持人审最高杠杆。
