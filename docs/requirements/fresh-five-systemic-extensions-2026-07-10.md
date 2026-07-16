# ForgeOS — 五方向系统性扩展分析

> **角色**:资深架构师 / 产品经理,全局深扫代码库(forge-core 18 Go 包 + harness 30+ 脚本 + 31 个 sprint
> 的完整演进轨迹)后,**从已落地能力的边界向外看**。
>
> **方法**:不同于"扫出所有缺口然后排优先级",我刻意寻找那些**跨已有 sprint 边界、跨 package 边界、
> 需要在多个子系统间新增数据流或控制流**的方向——它们不适合单 sprint 消化,但一旦做成会根本性地
> 提升 ForgeOS 作为平台的可信度、可观测性、和规模化能力。
>
> **每条包含**:问题诊断(代码证据) → 为什么值得做(ROI 判断) → 建议方向(不写代码,只描述边界)。

---

## 方向一 · 确定性仿真测试框架(Orchestrator 状态机形式化验证)

### 问题诊断

`internal/orchestrator` 是整个 ForgeOS 最复杂、最多状态的子系统:它有 6× 阶段状态机(`RunFrom`)、
4 种定向跳转(loop-back / on_unmet / on_rejected / resume)、5 个独立引擎参数
(MaxLoopBack / MaxRetries / MaxAgentCalls / budget / mode-policy)的组合爆炸、
以及一条**诞生时就带着锁顺序合约**(parallel.go LOCK ORDER CONTRACT)的并发路径。
此外它还有**一条声明但从未被任何 CLI 路径触发的代码路径**(`loop.go:nextStartPhase` 的
`OnRejected` 分支,注释自认「intentionally dormant」)。

当前测试策略完全是**单元 + 有限集成**——每个 fixture 构造一个具体场景、断言特定输出。
这能覆盖**已知**路径,但无法回答以下问题:

- **并发死锁**:`RunParallel` 持有 8 级锁顺序合约,但只有 `-race` 检测数据竞争,
  没有测试证明「在所有可能的 goroutine 交错下都能推进」。锁顺序合约被违反时,
  死锁是调度依赖的 Heisenbug,CI 测试 100 次全绿不代表不存在。
- **loop-back + resume 组合爆炸**:当 `MaxLoopBack=3`、`MaxAgentCalls=5`、`startPhase=2`、
  且 `agentOutcome` 返回 `REQUEST_CHANGES` 时,引擎是否能保证不超过任何预算?没有单一测试覆盖
  这三种计数器的交互。
- **on_rejected 死代码退化**:注释说它不会在当前 CLI 架构下触发,但未来如果有人写新 CLI 入口
  调用了 `LoopEngine.Run` 带 `human_gate` + `OnRejected`,没有测试证明它在那种上下文下行为正确。
- **checkpoint 崩溃一致性**:`phaseCheckpointHook` 写 checkpoint 后引擎崩溃,`resumeStart` 读到
  PhaseIndex 但 RoadmapCompletion 来自上一轮迭代——没有测试验证这个跨恢复的不变量。

### 为什么值得做

ForgeOS 的核心价值主张是「24h 无人值守自治运行」。无人值守系统**最怕的不是功能缺陷,而是
偶发死锁/活锁/预算耗尽/状态不一致**——这些 bug 在单元测试中从不出现,只在长跑第 7 小时触发。

一个**确定性仿真引擎**(类似 FoundationDB 的 `SimulationTesting` 或 TigerBeetle 的 `vsr` 式
状态机仿真)可以:
- 穷举调度交错来证明锁顺序合约无死锁
- 注入随机故障(disk write failure / OOM / signal)验证 checkpoint 恢复的最终一致性
- 在模拟时钟下跑 1000 次收敛迭代来验证 `staleCount` 的稳定性和 `MaxIter` 的 fail-safe 属性
- 对所有「声明但未触发」的路径(如 `on_rejected`)做形式化可达性分析

**ROI 判断**:这是基础设施类投资——不做时,项目依赖「人类的测试直觉 + CI 多次重跑」来保证
正确性;做成后,所有未来对 orchestrator 的修改都有安全网。对于一个要用 24h 自治的治理 OS,
这比任何功能特性都更能防止灾难性失败。

### 建议方向

1. 将 `LoopEngine.Run` / `Engine.RunFrom` / `Engine.RunParallel` 包装进一个
   `SimulationHarness`,使其运行在**确定性模拟时钟 + 可注入故障**的环境中
   (只需将 `time.Sleep`、`os.WriteFile`、`os.ReadFile` 等替换为可模拟接口,
   `backoff.go` 的 `Engine.Sleep` 已示范这个模式)
2. 核心不变量:**每个 checkpoint 的 PhaseIndex + SpentUsdMicros 组合在恢复后
   既不丢失已完成工作也不跳过未完成工作**(精确一次语义)
3. 证明锁顺序合约没有违反:在模拟器中跑 10K 随机交错,用 `-race` 检测 + 超时检测死锁

---

## 方向二 · 跨 Agent Prompt 注入检测与证据链溯源

### 问题诊断

`internal/memory` + `prompt_context.go` + `prompt_memory.go` 构成了一个**多 agent
共享知识层**:一个 agent 写的 memory 条目(`KindGap`/`KindDecision`/`KindLesson`)被
后续迭代的所有 agent 读取(`memoryContext` 注入到 prompt)。系统还有 `feeds_forward`
机制,把一个 phase 的输出直接注入下游 phase 的 prompt。

这份设计里有一个结构性风险:**信任链是扁平的**。

- **证据**:`prompt_memory.go:85-112` 把 `memory.Load` 的全部 `KindLesson` 条目
  注入 prompt,只按 `recency+relevance` 排序,**从不验证条目的来源可信度**。
  如果一个恶意(或被 prompt 注入污染的)agent 写入了一条虚假的 `KindLesson`,后续 agent
  会在多个迭代中把它当作事实。
- **证据**:`prompt_context.go:467-476` 的 `readCard` 读取 agent card 文本,但
  card 本身存储在 `.agent/agents/*.md` 中——项目文件,理论上可被 agent 修改。
- **证据**:**没有差分隐私或信任衰减**——`memory.Entry.Confidence` 字段存在且当 `confidence < 0.3`
  时 prompt 加 `[unverified]` 前缀,但没有任何机制自动降低从不可信来源(如从失败 phase 的
  输出中提取的条目)的 confidence。

更广泛地说,**整个观测链路缺少加密证据链**:`trace.jsonl` 是明文文件,任何有文件系统
写入权限的 agent 都可以篡改。`gateLedger` 和 `verdictLedger` 是内存结构,没有持久化的
不可否认证明。对于一个声称是「治理 OS」的系统,这是根本性的可信度缺口。

### 为什么值得做

这不是抽象的安全问题——在多 agent 自治系统中,prompt 注入是**已证实的攻击面**
(2024-2025 年多个研究:multi-agent prompt injection 可以在协作 agent 间传播)。
ForgeOS 的架构恰恰创造了一个高效的传播路径:
1. Agent A 被 prompt 注入 → 写入恶意 memory 条目
2. 下一迭代所有 agent 读 memory → Agent B 被间接注入
3. Agent B 写代码或评估结果 → 影响最终产出

对于企业采用 ForgeOS,可审计的证据链(不可否认的 trace)可能比任何功能特性都更重要:
「这个 gate 真的 PASS 了,不是 agent 伪造的」需要能独立验证。

### 建议方向

1. **信任层级标签**:每个 memory 条目和 trace 事件携带 `trust_level`:
   - `HARDWARE` — 来自独立仲裁者(如 `gate.HarnessRunner` 的 exit code)
   - `GOVERNANCE` — 来自带外执法器(`arch-check`/`secret-scan` 的机器裁决)
   - `AGENT_REPORTED` — 来自 agent 自报(如 `CONFIDENCE: 85`、reviewer verdict)
   - `INFERRED` — 来自启发式推导(如 `risk.FromChangedPaths`)
   - `UNVERIFIED` — 来自未经验证的 agent memory
2. **证据链持久化**:gate 结果带 `gate.Result` 的 `Evidence` 字段(exit code + stdout hash +
   timestamp),签入 trace,使其可被第三方独立验证
3. **记忆自动信任衰减**:memory 条目随迭代次数增加自动降低 `Confidence`,除非被新证据
   重新确认——防止「一条错误的知识永不消退」

---

## 方向三 · Workspace 级多项目和跨 Session 学习

### 问题诊断

当前 ForgeOS 的所有持久化状态都是**单项目、单 workspace 的**:

- **Checkpoint**:`.forge/checkpoint.json`(一个文件,原地覆盖)
- **Trace**:`.forge/trace.jsonl`(一个文件,10MB 轮转)
- **Scorecards**:`.agent/routing/scorecards.json`(一个文件,只在这个项目里)
- **Memory**:`.forge/memory.jsonl`(一个文件,随 evolve 单调增长)
- **Gate state**:无全局 gate 缓存,每个 `forge run`/`forge evolve` 重新 `ProbeAll`

证据:
- `engine_build.go:288-308`:`logPhaseHistory` 读 `routing.LoadScorecards(scorecardPath(o.root))`,
  传入 `o.root`——永远只读**当前项目**的 scorecard
- `evolve.go:305`:`recordMemory` 调用 `memory.Append(memoryPath(o.root), ...)`——永远写
  当前 `.forge/memory.jsonl`
- `routing/scorecard.go:HistoryTiebreak` 对所有候选模型做 quality 择优——但如果只有
  一个项目的 scorecard(冷启动),永远没有足够样本来让 haiku 凭质量证据胜出 sonnet

这个架构有一个根本性的限制:**ForgeOS 越用越聪明,但聪明只限于这个项目**。如果你用
ForgeOS 管 10 个项目,第 1 个项目学到的东西(「对于 Python CRUD 任务,haiku 的质量和
sonnet 一样好但便宜 3 倍」)完全不能帮助第 2 个项目做更好的路由。

更精细的问题:
- **scorecard.json 冷启动**(`loop-engineering.md` 承认):直到第一个真跑写完才有数据,
  冷启期间所有非安全底限 agent 都走 tier default,没有历史数据帮助做更优选择
- **没有模型质量退化检测**:如果 Claude Haiku 在某次更新后质量下降,系统不会自动发现
  并降权——因为 scorecard 没有跨项目的时间序列聚合
- **没有跨项目治理策略继承**:`forge migrate` 是单项目操作,没有「组织级策略→项目级
  覆盖」的层级

### 为什么值得做

ForgeOS 的护城河(per north-star)`治理 + 路由 + 随使用增长的记分卡/模板/策略数据飞轮`。
数据飞轮**必须跨项目**才能产生网络效应:

- 单项目的 scorecard 需要 20-30 次真跑才能产生统计显著的 `p95_latency` 和 `avg_cost_usd`
- 10 个项目共享同一套 scorecard,7 天内就能达到同等统计能力——**学习速度提升 10 倍**
- 跨项目路由数据还能发现「模型退化」这种单项目永远看不到的模式

**ROI 判断**:这是从「单机工具」进化到「平台 OS」的关键一跃。不做时,ForgeOS 永远是一个项目一个
独立实例;做成后,组织级的数据飞轮产生真正的网络效应——用的人越多,路由越智能。

### 建议方向

1. **可选的全局 scorecard 存储**:`scorecardPath(root)` 改为可配置为 `~/.forge/scorecards/`
   或 `$FORGE_SCORECARD_DIR`,不同项目可以向同一仓库贡献/查询。本地 scorecard 优先于
   全局 scorecard(项目特定覆盖率局部知识)。
2. **跨 session memory 导入/导出**:`forge memory export --format jsonl --global` 允许
   将 learnings 从一个项目导出、另一个项目导入,由 CTO 角色仲裁哪些 lessons 是通用的。
3. **workspace 概念**:`.forge/` 下预留给未来 `workspace.json`,声明此项目所属的组织单元、
   继承的策略来源(哪个 team 的 scorecard 可供参考)。

---

## 方向四 · Harness 执法器自身的 SLO 监控与退化检测

### 问题诊断

ForgeOS 把「带外 gate 为真相之源」立为宪法。但**谁守护守护者**?

当前的执法器(harness 中的 gate.mjs / arch-check / secret-scan / check.py 等)是整个系统
是否可信的**锚点**。但:

- **证据 1 —— gate 的 SLO 完全不可观测**:没有指标记录 `gate.mjs` 自身的运行时间、
  失败率、假阳性/假阴性率。如果有一天 `arch-check.mjs` 的某个正则因为 Node.js 更新
  而静默失效(类似 Sprint 27 修复的 `block-scalar 损坏` 在测试中隐匿了 6 轮 sprint),
  系统没有任何机制发现——gate 仍然报 PASS,但实际已不再抓住真正的架构违规。
- **证据 2 —— gate 的依赖链没有健康检查**:`harness/secret-scan.mjs` 依赖 `grep`+`tr`+
  `git`;`harness/arch/arch-check.mjs` 依赖 Node.js `fs`/`path` 模块。如果这些依赖
  被破坏(如 CI 环境升级导致 Node.js API 变更),gate 可能静默降级为总是返回 PASS 的
  空转。
- **证据 3 —— SCA 框架已就绪但无自身的漏洞扫描**:`harness/sca.mjs` 可以为项目代码
  做 CVE 扫描,但**它自己不扫描自身的依赖**。如果一个 harness 脚本的依赖有漏洞,
  谁会报告?
- **证据 4 —— gate 输出没有自检签名**:`forge accept` 的 ACCEPTED/REJECTED 裁定全部
  在内存中完成,没有「此 gate 在此时间对彼 commit 输出了彼结果」的不可否认证据。
  这意味着一个 CI 重跑可能因为环境差异得到不同结果,但无法追溯。

### 为什么值得做

ForgeOS 把治理可信度视为一切的基础。Sprint 29 修复了执法器假阴盲区(export-from 不可见、
async def 不匹配、.env 遗漏),但这些是**静态 bug 修复**——修一个就少一个。真正需要的是
**运行时退化检测**:在 gate 自身出错时,能区分「是真 PASS」还是「执法器坏了所以报 PASS」,
并且把这个区分信号暴露给上层(operator dashboard、CTS)。

**ROI 判断**:这不是功能特性,是 ForgeOS 自身 SRE 的基础设施。类似于「监控系统需要被
监控」——ForgeOS 作为治理 OS,它的治理执法器需要有自身的健康状态管理。不做时,执法器退化
是**不可见的信任侵蚀**(用户逐渐不信任 gate 结果);做成后,系统能自己报告「今天我可能
不可靠」。

### 建议方向

1. **gate 自检 `forge doctor --gates`**:扩展 `internal/doctor` 包,加入对每个 harness
   脚本的 smoke test——确认每个 gate 可以实际运行、输出格式可解析、且对已知样本产生预期输出。
   这不是 `forge validate`(校验 workflow 声明),而是运行时的**执法器健康检查**。
2. **对每个 gate 的 SLO 做 telemetry**:`acceptance-quality.mjs` 已经可以测量 lint/
   coverage,但 gate 自身的执行时间/退出码/stdout 没有被记录。扩展现有的 `trace.Event`
   来包含每个 gate 调用的元数据,使退化可观测。
3. **gate 输出的签名和可验证性**:每个 `gate.Result` 携带 `ProvenBy` 字段(如
   `"harness/arch-check.mjs@sha256:abc123"`),trace 写入时附带 commit hash 和时间戳,
   形成一个可追溯的证据链。

---

## 方向五 · 成本/预算的多维可视化和自动优化引擎

### 问题诊断

ForgeOS 的成本控制已经是业界最佳实践——run-level hard cap(--run-budget-usd)、per-call cap
(--agent-max-budget-usd)、budget-aware tier down-grade(BudgetAdjustTier)、
跨-resume 持续 costing——但所有这些能力在当前都是**静态配置的防御性护栏**,
而不是**主动优化的引擎**。

具体缺口:

- **证据 1 —— 成本数据没有结构化消费管道**:`cost.go:468-475` 已经能把每次 agent call 的
  `Model`、`CostUsdMicros`、`LatencyMs` 写入 trace,`scorecard_wind.go` 也能把 per-iteration
  cost 归入 scorecard——但这些数据**只消费于 `forge route --scorecard` 的可视化**,
  从未喂入一个**自动优化决策回路**。
- **证据 2 —— 没有「预算-质量」权衡界面**:当前系统要么全价跑、要么 budget guard 触发紧急
  降档。没有中间状态——比如「我今天的预算只有 $5,但 build 有 10 个 phase,请优化分配:
  哪 3 个 phase 值得 Opus、哪 7 个 Haiku 就够了」。
- **证据 3 —— no-progress tripwire 没有 cost awareness**:`LoopEngine.staleCount` 检测
  到 roadblock 后触发 no-progress tripwire 停止循环。但它**不区分「烧了 $20 后无进展」
  和「烧了 $0.01 后无进展」**——前者应该立即 escalate 给人、后者可以再试一轮不同方法。
- **证据 4 —— budget 消耗是「事后」的,不是「事前预测」的**:`BudgetAdjustTier` 在
  `spendRatio >= 0.80` 时才触发降档。但更智能的做法是在 run 开始前预测总成本:
  「这个 workflow N 个 phase,每个大约 $X,总预算 $Y,建议降 2 档或减少迭代数」。

### 为什么值得做

ForgeOS 的真点火已经如实证明:一个真实的多 agent build(`planner→implementer→gate→reviewer→qa`)
每次迭代大约 $0.18,一个 `forge evolve` 10 次迭代大约 $1.80。在企业规模下,如果每天跑 50 个项目,
每月的 LLM 预算会迅速成为组织的首要关切。

**当前的预算系统足以防止失控,但不能帮用户做「如何在有限预算下获得最大质量」的优化决策。**
这恰恰是 ForgeOS 的护城河机会:不仅是一个治理 OS,还是**企业 AI 开发预算的控制面**。
没有竞品(Claude Code / Codex / Gemini CLI 各自为政)提供跨项目、跨模型的预算优化。

**ROI 判断**:这是产品化的高杠杆方向。成本控制是企业采购「AI 软件工厂」时的第一问。
如果 ForgeOS 能回答「给我 $100 预算,我帮你建一个可验收的 MVP,并给你一份成本质量权衡
报告」,它就从工程工具升级为预算所有者的决策工具。

### 建议方向

1. **Run 前成本预估器**:在 `forge run` / `forge evolve` 的预检阶段(类似 `preflight.go`),
   增加基于 scorecard 历史数据的成本预估:「此 workflow 预计 12 个 agent phase,上次 3 次
   类似 build 平均 $0.14/phase,预估 $1.68,低于你设定的 $5 cap——放心跑。」
2. **预算优化建议**:当 `--run-budget-usd` 低于预估成本时,主动建议优化方案:
   「预算 $1 但预估 $1.68,建议:(1)将 implementer 降为 sonnet 省 $0.42
   (2)将 max-iter 从 5 减到 3 省 $0.50 (3)或增加预算到 $2。」这不是计算器,而是从
   scorecard 历史中学习的优化器。
3. **成本-time-to-value 仪表盘**:在 converge report 中加入成本维度的收敛曲线
   (类似「每 $1 买到的 roadmap 完成度百分比」),帮助企业回答「我的 AI 开发预算
   花得值不值」。

---

## 总结:五方向的对比矩阵

| 方向 | 类型 | 风险/回报 | 估算规模 | 依赖前置 |
|---|---|---|---|---|
| ① 确定性仿真测试 | 基础设施/质量 | 低风险,极高回报——防止死锁/状态不一致的灾难性 bug | 2-4 sprint(主要是测试框架,不碰业务逻辑) | 无 |
| ② 跨 Agent 信任链 | 安全/可信度 | 中风险(涉及 memory 和 prompt 的侵入式改造),极高回报——企业采购的前提条件 | 3-5 sprint(跨 memory/prompt/trace 三个包) | 无 |
| ③ 多项目 Workspace | 平台/扩展 | 低风险(增量扩展现有存储层),高回报——网络效应开启 | 4-6 sprint(subtle——涉及 scorecard/memory/routing 三处数据流的选址和迁移策略) | 方向②的记忆信任标签完成度越高,跨项目数据越可信 |
| ④ 执法器 SLO 监控 | 基础设施/治理完整性 | 低风险,中回报——主要是可观测性增量,不易用户感知但工程纪律质变 | 2-3 sprint(主要扩展 `internal/doctor` + harness 适配器) | 无 |
| ⑤ 成本优化引擎 | 产品/商业化 | 中风险(新规划),极高回报——企业直接买单的差异化功能 | 3-5 sprint(成本数据管道已就绪,主要是决策引擎 + UI) | 需要有足够的 scorecard 历史数据(方向③加速) |

**优先级建议**:① → ④ → ② → ⑤ → ③,理由:
- **① 和 ④** 是地基:先保证系统不会悄无声息地坏掉,再谈扩展
- **②** 是信任的工程基础:没有信任链,多项目数据共享(③)也不可信
- **⑤** 是面向用户的最高杠杆产品特性,但基于前二者提供的数据可信度和管道
- **③** 是平台化的最终形态,但依赖前序方向构建的可信数据基础设施
