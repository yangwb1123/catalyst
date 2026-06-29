# Loop Engineering —— ForgeOS 的收敛控制方法论

> **状态(诚实分界)**:本文是对 ForgeOS **已落地控制平面**的方法论提炼,**不是**蓝图叙事。
> 「收敛控制」内核(LoopEngine + converge + 带外验证 + 诚实代数)已实现、已接线、已被测试与 dogfood
> 覆盖;但「学习飞轮 / 自适应装配 / 真 Reflect」**尚未建成**,本文用 §4 的「已建成 vs 蓝图」表**逐项**
> 标注,绝不假装已做(对照 [AGENTS.md](../AGENTS.md) honesty-first 立律)。本文是 [ARCHITECTURE.md](../ARCHITECTURE.md)
> 所述脊柱的**概念框架**,机制细节以代码为准。

## 1. 定义

**Loop Engineering = 自治 agent 的 SRE。** 它工程化的是**外层控制环** —— 收敛判据、带外验证器、
反 doom-loop 触发器、预算/韧性护栏、可崩溃续跑的状态 —— 使**多 fresh-context agent 的软件生产**
收敛到**机器可验证、与执行者无关**的判据,而非自报的「我做完了」,并能长跑而不熔毁。

- **Agent Engineering 拥有内层环**:单个 agent 的 reason→act→observe + 工具调用。
- **Loop Engineering 拥有外层治理环**:跨多个 agent 的「该不该继续 / 是否收敛 / 是否卡死 / 是否超预算」。

**诚实前提**:闭环这个**原语**是老的(控制论 / OODA / RL agent-环境环 / ReAct·Reflexion /
Devin·OpenHands 的 iterate-until-green)。本方法论**不声称发明了环**;它命名的是一个**实践焦点 +
对抗式验证立场**(见 §2、§5)。如同 SRE 没发明服务器、Platform Engineering 没发明 reliability ——
它们命名了一个值得严谨工程化的轨迹。

## 2. 可辩护的内核 —— trust-minimized feedback control

> ForgeOS 在一个**独立计算、可证伪**的收敛判据上闭环 —— `roadmap_completion==100% AND 所有必需
> harness gate PASS`,由一个**独立的零-LLM Go+harness 进程**测量、**执行中的 agent 无法撰写或伪造**
> —— 同时**结构性禁用**了其他 agent 环依赖的两个终止信号:模型自报「完成」(降级为**有界 fail-open
> 建议**),与轮数(显式声明 `anti_pattern: round_count`)。

为什么它能扛住与既有范式的对比:

- **vs 经典控制论(PID/MPC/OODA/控制论)**:ForgeOS **就是**一个离散反馈控制器(setpoint = 收敛判据,
  anti-windup = `staleCount`/`NoProgress` 触发器,saturation = `MaxIter`)。但控制论**假设传感器可信**。
  ForgeOS 的新意在于其**执行器是一个不可靠的生成式 agent、可能伪造自己的传感读数** —— 故把**执行器
  排除在自身误差测量之外**(验证器带外、异语言、跑在 LLM 从不触碰的进程里)。内核 = **对不可靠生成式
  执行者的 trust-minimized 反馈控制**。
- **vs agentic 环(ReAct/Reflexion/AutoGPT)**:它们以模型自判「done」或步数预算终止。ForgeOS 在代码里
  **两者皆禁**:三个 workflow 全带 `anti_pattern: round_count`;`MaxIter` 是 fail-safe(合取式的 MaxIter
  耗尽**报告为未收敛**);reviewer 裁决是 fail-**open** 建议(不跳转 ⇒ proceed,永不据此 converge)。
- **vs 图状态机(LangGraph)**:`RunFrom` 机制上就是带条件边的状态机,但 ForgeOS 把**终止判定从图里
  抽出**交给独立验证器,并对唯一受 LLM 影响的边设 fail-closed `MaxLoopBack` 上界。
- **vs RL episode**:终止是**可证伪 gate 的布尔合取**,非习得/塑形的标量 reward,且拒绝「horizon 即目标」。

**载重差异 —— 诚实代数**(这才是它区别于「朴素 setpoint 检查」的地方):N/A(无可执行检查)**永不**计为
PASS;零-phase workflow **永不**报收敛(`loop.go` false-clean 守卫);全-N/A 不算 green(vacuous-green
守卫,provenCount==0⇒false);探针损坏降级为**不可豁免**的 N/A。**「缺乏证明永不等于满足」** —— 这让
判据真正可证伪,而非打勾。

**二级 delta**:停止信号是**治理/架构 gate 的合取**(分层 / 文件大小 / 循环依赖 / 认知负荷 / secret /
复杂度,经 `acceptance.mjs` + arch-check 8 检查),故环收敛到**架构健康**,而非仅「跑得起来」—— 这是
对 Devin/OpenHands「iterate-until-green」的真实 delta。

证据:`converge.go:127`(Converge 分派)· `converge.go:149-175`(Evaluate 合取)· `cmd/forge/gates.go`
(gatherSignals 读 ROADMAP + ProbeAll,零 LLM)· `loop.go:11-22,121-166`(MaxIter=安全底线、tripwire)·
`orchestrator.go:281-319`(agentOutcome fail-open + loopBackTo)。

## 3. 最小诚实分层(7 层栈不成立)

用户提出的 7 层(Prompt/Context/Agent/Workflow/Loop/Model/Policy)**不能作为 7 个正交对等层成立**。
代码物理上分包的真实因子分解是 **4 个正交执行层 + 1 个横切配置轴**:

**Plane 1 —— 正交执行层**
1. **Context** —— 什么知识进窗口:检索/排序/限长/缓存(`prompt.Gather`/`Retrieve`/`ContextCache`)。真正正交轴。
2. **Agent** —— 角色/人设/工具授权/fresh-context/默认档(`.agent/agents/*.md`)。**吸收「Prompt」**(角色卡
   即 prompt 的实质)与「Model」的 agent-default 切片。
3. **Workflow** —— 静态编排:phase DAG、gate 布点、**有向回边**(`loop_back`)、阶段交接(`on_met`/`on_unmet`)。
4. **Convergence(真正的「Loop」)** —— 活体终止控制:对实测信号判停、MaxIter 安全底线、反 doom-loop
   触发器、checkpoint/resume。

**Plane 2 —— 一个横切旋钮**
- **Policy**(mode×lifecycle) —— 设定每个 Plane-1 层的**严格度**(modes.yml 头:「一个设置同时驱动三个
  子系统:Router 档 · Harness 严格度 · Workflow 深度」)。它是**旋钮**,不是栈里的一层。

**七层为何坍缩**:
- **Prompt 非对等层** —— `prompt.Build` 是 14 行装配缝(header⊕角色卡⊕context),自身独立内容只有一句
  framing;包文档自称「Context Engine v1」。**Prompt = Agent 卡 ⊕ Context**。
- **Model 非对等层** —— 是个 per-phase **解析器**,由 Agent-default ⊕ Policy-floor ⊕ run-budget 喂入。
  决定性反证:`forge run`(`main.go`)做 per-phase 模型路由时**根本没有 LoopEngine** —— 故模型调度是
  Workflow/Agent 执行关切,**不是**环的子组件。(用户「Model 是 loop 子组件」假设被证伪。)
- **Policy 非对等层,且反而 super-ordinate 于环** —— `mode.Effective(...).EvolveMaxIter()` **设定**
  `LoopEngine.MaxIter`。Policy 是栈**之上**的旋钮,不是环的子组件。(用户「Policy 是 loop 子组件」假设
  方向**正好相反**。)
- **Loop vs Workflow** —— **裸回边本身已是 Workflow 构造**:`on_fail:loop_back` 整个活在单趟
  `Engine.RunFrom` 里(`loopBackTo`,`i=target-1; continue`),不涉 LoopEngine。「带环的图」坍缩进
  Workflow。**不坍缩**的是 LoopEngine 在其上加的:活体判停、安全底线、反 doom-loop、跨迭代状态 ——
  **建议把「Loop」更名为「Convergence」**,使回边不再对 Workflow 双重计数。

**决定性结构事实**:`forge run` 直接建 Workflow Engine(无环);`forge evolve` 用 `NewLoopEngine(eng,…)`
包裹**同一个** `buildRunEngine` Engine —— 环在结构上**包含**了 workflow 引擎。

**「Loop 即大脑」** 只对**「该不该继续」**成立。**「下一步做什么」**的智能是一个 **Workflow phase**
(`gap-analysis`,agent=architect)。大脑是分裂的:Workflow 拥有 what-next,Convergence 拥有 whether-to-continue。

## 4. 已建成 vs 蓝图(载重诚实分项表)

| 组件 | 裁定 | 代码实际做了什么 | 证据 |
|---|---|---|---|
| **Loop Engine/Controller**(continue/stop/loop-back/re-plan) | **已建成** | 集中式迭代控制器 `LoopEngine.Run`,4 个优先级停止源(gate/agent 错误→converge→no-progress→MaxIter);迭代内有向 loop-back 是 `RunFrom` 里真状态机跳转;跨迭代 re-plan 重启于 planner。测试覆盖。 | `loop.go:121-166,174-183` · `orchestrator.go:267-319` · `converge.go:127` |
| **Loop Policy**(合取停 / human-gate / retry / budget / max-iter / checkpoint-resume) | **已建成**(opt-in 注意) | 声明式 YAML→typed `StopCondition` 端到端驱动;合取 `roadmap==100 AND gates green` 对活信号判定;human-gate 不可绕过;529 退避;两个 fail-closed budget cap;原子 + phase 粒度 checkpoint。 | `build.yml:101-116` · `converge.go:149-175` · `backoff.go` · `budget.go` · `persist/checkpoint.go` |
| **Loop→Model 调度**(phase 驱动档) | **phase+budget 已建成;risk 部分;一例 latent** | 一个共享 `tierOf` 喂 `claude --model`/prompt/cost-stamp(drift-guard 测试);architect/cto/reviewer opus 硬底线真。**但**动态 risk 评分只接入独立 `forge route` CLI、**未接** run/evolve 环;「README→廉价档」latent(`docs→Haiku` 存在但**无 workflow 声明 `agent:docs`**)。 | `executor.go:57-67` · `engine_build.go:236-247` · `routing.go:22-71` · `route.go` |
| **Loop Memory/Learning**(「学会哪种环最好」) | **部分 → 标题项是蓝图** | 落盘真但粗(`quality_score`=accepted/samples 二值,p95/cost 真)。**「学/选最优」是结构性 no-op**:`HistoryTiebreak`(唯一消费者)**只**用单元素列表调用、pick 被**丢弃**(`_, reason :=`);代码自述「picked==adj ALWAYS… 不假装 v1 在学」。**`scorecards.json` 仓内根本不存在**(永久冷启)。reviewer-score/rework/iterations 自动采集**未接线**。 | `scorecard.go:13-19` · `engine_build.go:256,279` · `scorecard_wind.go:146-149` · `.agent/routing/`(无 scorecards.json) |
| **自适应/动态环装配**(项目类型→专用 agent) | **缺失** | 4 个脊柱 workflow 是静态手写 YAML、phase 与 agent 固定。**全仓无项目类型概念**(grep 零命中);固定 9 张角色卡,所称专用 agent(Product/Performance/Evaluation)**不存在**。mode×lifecycle 是唯一变化,且**纯减法**(过滤 gate/跳 reviewer/discover、设 max-iter)+ **手动选**,非自动检测。 | `orchestrator.go:187-231` · `modes.yml` · `mode_gating.go` · `.agent/agents/`(9 卡) |
| **7-phase 认知环**(Observe→Think→Plan→Execute→Evaluate→**Reflect**→Evolve) | **部分 —— 6 映射,Reflect 缺失** | 6 个清晰映射到 LoopEngine 真迭代的 phase(Observe→scan / Think→gap-analysis / Plan→planner / Execute→implementer / Evaluate→harness+reviewer+qa〔最强、机器验证〕/ Evolve→环本身)。**Reflect 无真等价物**:环以**二值** converge 裁决 + no-progress tripwire 收尾;最接近的 reviewer findings 前传是**逐产物质量判断**,非「为何环失败/慢」自分析,且永不调整路由/流程。 | `evolve.yml` · `converge.go` · `loop.go` · `evolve.go:338-356` |

**横切诚实注**:出厂默认 `--executor=dry`(零 LLM)。开箱即用时环在**不变仓库**上测真 gate —— 收敛**机制**
真且测过,但自治**修复价值**只在 `--executor=command --agent-cmd=claude` 下兑现。已做过一次真点火
(~$3.84,proof of life),**非**已证的 24h 自治长跑。

## 5. 先行技术诚实

「Loop Engineering」**不能**当作 Agent 之上**新发明的第四层**叫卖:① 环是深厚先行技术(控制论/OODA/
RL/ReAct/Reflexion/Devin·OpenHands);② 2025「Agent」浪潮**本身就是环**(每个 agent 框架都是
reason-act-observe),把「Loop」提为接续第四层是叙事通胀 ——「Loop」**重叠**于「Agent」,非接续;
③ 该术语与 Prompt→Context→Agent→Loop 进阶在**本仓 grep 为空**,是提案 retro-frame,非代码已有声明。

**可辩护的真实 gap**:把环的**停止/反馈权威移到带外、机器可验证**,而非执行模型自判。这是 ReAct/Reflexion/
AutoGPT(同一模型既执行又自判完成)**结构上不具备**的。

## 6. 蓝图(明确标注,非现状)

按杠杆排序的下一步(**先建成、再宣称**,而非反过来):

1. **真 Reflect 步**(最高杠杆,也是把它从「CI 流水线」提升为「认知环」的关键):把真 reviewer/agent
   输出解析为结构化 `memory.KindLesson`(今 `recordMemory` 只写 dry-run trajectory),并把环级信号
   (rounds-to-green / rework / cost-latency)回喂下一迭代 prompt 或路由。底物已在(`memory.go` /
   `scorecard.mjs` rework_rate·avg_iterations / `prompt_memory.go`),**未接线**(`runScorecardUpdate`
   现省略 `--rework`/`--iterations`)。接上即让第 7 phase 从「缺失」→「部分」。
2. **兑现哪怕最小的学习环**:让真·多候选 `HistoryTiebreak` 真正做选择(今单候选 passthrough),或至少
   从一次真跑生成首个 `scorecards.json`,使观测路径产出过数据。在此之前,**飞轮必须显式标蓝图**。

*(自适应/动态装配是最大野心 gap 但**不是**下一步该押的:它全绿地、且对核心论题非载重。别宣称、别优先于 Reflect。)*

---

> **一句话方法论(可对外、且代码兑现)**:Loop Engineering = 自治 agent 的 SRE —— 工程化外层收敛控制,
> 让多 agent 软件生产收敛到**带外、与执行者无关、可证伪**的判据,而非自报完成。**领以已兑现的**收敛控制 +
> 带外验证 + 诚实代数 + 韧性护栏;**飞轮 / 自适应 / 真 Reflect 显式围栏为蓝图**。这样它是一门可辩护的
> **学科**,而非下一行文档漂移。
