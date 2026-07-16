# ForgeOS: 五条未被触及的高价值架构扩展方向

> **角色**:资深架构师/产品经理  
> **方法**:全局扫描所有 Go 源文件(18 包,~40k LOC)、Harness 层(JS/Python,~6k LOC)、.agent 文档骨架、已有 docs/ 分析产物,排除「已实现」「已诚实推迟」及「已有十余份重复分析」的条目,只列**代码库中确实不存在且未被任何现有文档明确覆盖**的高杠杆方向。  
> **日期**:2026-07-10  
> **位置**:`docs/requirements/forged-architecture-five-fresh-horizons.md`

---

## 方向一:工作流定义合法性校验器(Workflow Static Analyzer)

### 当前状态

`asset.LoadWorkflowJSON` 是故意宽容的(拼写注释:missing fields => zero-valued-but-usable)。这使运行时对**结构错误**的 workflow 文件保持静默:
- **phase 名称冲突**:若两个 phase 同名(如两个 `"implementer"`),`phaseIndex()` 返回第一个匹配,第二个永不执行——无声丢失。
- **on_fail.target_phase 悬空引用**:若 target_phase 指向不存在的 phase,`loopBackTo` 返回 `jumped=false` → 降级为 abort-on-red,与作者预期的定向回跳产生行为差异,日志只有一个 `not found` 警告。
- **depends_on 循环引用**:`Waves()` 已经是 fail-closed,但循环只在**运行时**被发现(phase 已执行到一半),没有**起跑前**的静态告警。
- **stage gating contradiction**:若一个 workflow 同时声明 `stage: build` 和 `discover_gate: true`,mode gating 不会报矛盾——它只是忽略不匹配的字段。
- **feeds_forward + FreshContext 冲突**:若一个 phase 同时声明 `feeds_forward: true` 和 `fresh_context: true`,`FeedsForward` 永不输出(因为 FreshContext 抑制输出),无人告警。

### 为什么需要

ForgeOS 身份是**声明式治理运行时**——工作流定义是控制面文件。当前没有一层**在运行之前**回答「这个 YAML 是否自洽」。这在 v2 单仓单流程下可容忍;一旦 `forge-init` 复制给新项目,或者 `forge migrate --to engineering` 修改 workflow,错位的工作流引用就会变成**静默的行为退化**,非「运行失败」——更危险。

### 价值量化

- 🚨 **安全**:消除悬空 loop-back / on_unmet target 的**静默降级**——这是最危险的一类:作者声明了定向回跳,运行时降级为放弃,日志只一行 `target not found`。
- 🔧 **开发者体验**:在 `forge run/evolve` 之前给出一份完整的 validation 报告(类似 Go 的 `go vet`),而不是运行时才暴露。
- 🧱 **复用安全**:`forge-init` 的 copy-anywhere 不变量需要被复制的工作流在目标项目上也合法——当前只校验语法(JSON unmarshal),不校验语义。

### 具体范围(不做先发制人)

- 不实现**运行时可选的 TLA+/Temporal 模型检查**——那超出 v3。
- 不做**工作流是否被完整测试覆盖**的静态分析——那是 Eval 层的域。
- 只做**结构合法性**(悬空引用/命名冲突/阶段门互斥/不可达 phases),不做**正确性**(「这个 stop_condition 是否能被任何真实信号满足」需要动态分析,属于未来)。

---

## 方向二:多仓库编排——agent-os 继承层(Submodule 契约 Enforcement)

### 当前状态

ADR-0003 设计了 `agent-os` submodule 的**机制**(submodule + 双层覆盖 + 路径解析改造),但明确标注「**暂缓至触发条件**」。当前代码库的继承层是空的——`project.yml` 的 `extends: []` 说明了这一点。对 `forge-init` 而言,全部 `.agent/` 资产都是原样复制,没有任何「上游基线→项目覆盖」的分层概念。

### 为什么需要

这是**唯一一个从 v0 路线图就声明、设计就绪、但代码零落地**的架构层。随着 ForgeOS 从单仓治理扩展到多项目组织(一个 org 下 N 个 microservice 仓库共享一套 agent 卡 + skill 卡 + workflow 模板 + routing policy),当前模型暴露出:
- **策略漂移**:每个仓库独立复制 `.agent/policies/modes.yml`,无集中升级路径。
- **rolodex 碎片**:agent 卡、skill 卡在各项目里独立演化,修复了一个 agent 卡 bug 需要在 N 个仓库重复同样的编辑。
- **check.py 无法交叉引用**:`check_workflow_agent_refs` 只查本地 `.agent/`,不能验证「这个 workflow 引用的 agent 卡在上游 agent-os 仓库里是否存在」。

### 价值量化

- 🏛️ **架构完整性**:兑现 ADR-0003 的「submodule + 双层覆盖」设计——目前这是**唯一一个设计就绪但代码零实现的架构组件**。
- 🔁 **可升级性**:一个上游策略更新(`modes.yml` 加新维度,agent 卡补新机读契约)通过 `git submodule update --remote` 传播到所有子项目,无需逐仓手动同步。
- 📐 **治理一致性**:`check.py` 的 `check_workflow_agent_refs` 可以跨仓库验证——引用上游 agent 卡的项目不需要自己的副本。

### 具体范围(不做先发制人)

- 不实现**在线策略分发**(Temporal / etcd)——v2 够用,submodule + `forge upgrade` 足够。
- 不做**覆盖冲突的语义合并**(两层都改了同一行怎么做 3-way merge)——当前 **submodule 的 git merge** 已经提供了三层合并,forge-core 不需要自己的合并引擎。只需要路径解析:上游 `<submodule>/.agent/agents/architect.md`,项目覆盖 `project/.agent/agents/architect.md`。
- 不做**热重载**——每次 `forge run` 重新读取文件路径,no stateful cache。

---

## 方向三:运行态健康自检与退化路径(Operational Health Checks & Graceful Degradation)

### 当前状态

`forge doctor` / `forge status` / `forge validate --models` 是**诊断命令**——人执行、看结果。运行时(engine_build.go / execEngine / buildLoop)在每次起跑前只做一件事: `quickDoctorCheck`(写几个 trace 事件)。具体来说:
- 如果 trace.jsonl **不可写**(权限/磁盘满),`openTracer` 返回 error → 整个 `forge run` 失败 fail-closed。
- 如果 checkpoint.json **损坏且 --resume**,`resumeStart` 返回 error → 整个 evolve 失败。
- 如果 memory.jsonl **有一行 JSON 损坏**,`Load` 返回 error → `compactMemoryIfDue` 失败 → 一个 log warning。
- 但如果 `.forge/` 目录**被意外删除**(`rm -rf` 后 evolve 还在跑),`memory.Append` 重新创建文件(有 MkdirAll),但 checkpoint 历史链断裂(rotateRetain 的源文件已经没了),`trace.jsonl` 从空文件继续写——**部分状态被静默重置,无人知晓**。

更重要的是,**没有运行时健康检查响应机制**:
- `BudgetExhausted` 是一个 bool 闭包——但 budget 到达 100% 后,当前 run  直接 abort 而非尝试**降级到耗尽前的最优状态再停止**。
- `ClassifyOverload` 是一个纯识别器——不跟踪**最近 5 分钟内的 overload 率**来动态调整 backoff;每次 overload 独立对待。
- `trace.Emit` 失败只是 `WARNING`——但如果 trace 持续不可写(连续 10 次 emit 失败),`forge evolve` 还在无观察性地跑,失去审计能力。

### 为什么需要

ForgeOS 是**24h 自治运行时**——不是 cron job。当前的状态是「核心路径 fail-closed,辅助路径 fail-loud-and-continue」,但没有**系统性的健康状态机**:一次磁盘满导致 trace 不可写→持续 silent emit failure→循环还在跑→烧真实 API 预算但审计流断裂——直到有人发现 `forge doctor` 报告 trace 损坏。

**需要的是**:
1. 运行时健康**等级**(HEALTHY / DEGRADED / FAILED),由多维度投票(磁盘 IO、记忆体损坏率、budget 水位、overload 趋势)决定。
2. **降级路径**:DEGRADED 时自动收紧 `--max-agent-calls`(从 100 -> 20),降低 `--max-output-bytes`,禁用可选 trace/memory 写入;FAILED 时触发**有序关闭**(`SIGTERM` -> 最终 checkpoint -> 关闭子进程)。
3. **恢复检测**:在 DEGRADED 状态下每 N 秒重探,健康恢复后自动回到 HEARTY 模式扩回参数。

### 价值量化

- 🛡️ **自治韧性**:当前ForgeOS的「失控保护」全在**起跑前**(`--max-agent-calls`,`--timeout`)。运行中的故障只有 `WARNING` 日志和 panic-and-crash 两种模式,缺乏中间态。
- 💰 **成本安全**:一次 trace 写入持续失败但 evolve 继续烧钱——这是真实风险,超过了「诚实标注」的范畴(标注了但未防御)。
- 📊 **可观测性**:运行态健康等级可以写入 checkpoint(当前无健康字段),`forge status` 可以问「最后一次 evolve 是健康跑完还是退化跑完」。

### 具体范围(不做先发制人)

- 不实现**自动恢复操作**(重启容器、回滚部署)——那是 k8s 的域。
- 不实现**远程报警**(PagerDuty/Slack webhook)——那是外部 observability 平台的事(v3 Web UI 可以消费健康事件,但不是报警引擎)。
- 只实现**本地健康状态机 + 资源参数自适应 + 有序关闭**——一个 struct 加几行决策逻辑,复用既有的 `BudgetExhausted`/`sleep` 回调模式。

---

## 方向四:跨阶段数据依赖追踪(Data Dependency Graph & Artifact Provenance)

### 当前状态

ForgeOS 目前有一个**隐式阶段依赖链**:Discover → Design → REVIEW → Build → Evolve。但**跨阶段数据流完全是手动的**:
- `design.yml` 的 solution-architect 产出应当被 `build.yml` 的 implementer 读到,但当前没有机制保证——build.yml 的 Gather 只查 `.agent/ROADMAP.md` + ADRs,如果 architect 写了一个 `docs/design/architecture.md`,下游 implementer **不会自动看到它**。
- `feeds_forward` 只在**同 workflow 内**有效:build.yml 的 planner 产出可以前传到 implementer,但 **build.yml→evolve.yml** 之间没有数据继承。
- `emits` 字段**声明**了文件产出,但 forge-core **不验证**这些文件是否真的存在,也不验证其内容是否被下游消费。
- 跨阶段依赖全部靠 YAML 注释和 agent 卡散文约定(「implementer 需读 PROPOSAL.md,SPRINT.md,ROADMAP.md」)——无法被机器验证。

### 为什么需要

ForgeOS 的脊柱是 **5 个 workflow 文件的时序链**,但 forge-core 的 `LoadWorkflowJSON` 加载的永远是**一个 workflow**,不是整个脊柱。`forge run discover` 的输出不自动传递给 `forge run design`,每个阶段是**信息孤岛**。当前填补这一空白的唯一代码是 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 NOTE 标记——这是文字,不是代码。

具体后果:
1. **假设暴露**:agent 卡声称「读 X、写 Y」但无机器验证,reviewer 只能靠人肉检查,在真 agent 场景下不可靠。
2. **跨阶段的收敛无法校验**:evolve.yml 的 stop_condition 可以引 `roadmap_completion`,但**不能**引 discover 阶段的 `requirement_confidence`——因为 converge.Signals 不跨 `forge run` 调用传递。
3. **重复检索**:build.yml 的 implementer 重新检索 ADR/Gather(ADRs/AGENTS),即使这些在 design 阶段已经查过——每次重复丢失了阶段上下文。

### 价值量化

- 🔗 **脊柱连贯性**:脊柱上不同阶段之间有了**机器可读、可校验的数据流契约**,不再依赖散文约定和 agent 的 luck。
- 🛡️ **诚实性**:`emits` 声明的文件如果在下游阶段不存在,forge 可以给出**可观察的警告**而非沉默。
- 📉 **减少重复**:下游检索可以直接复用上游已计算的 context 而不是重新 disk read。

### 具体范围(不做先发制人)

- 不实现**全局 DAG 调度器**(分阶段执行所有5个 workflow)——那是编排架构的未来。
- 不做**数据版本化**(多轮迭代后,下游该用哪个上游版本的数据)——那是 Temporal/durable execution 的域。
- 只做**声明的 artifact 路径追踪**:在 `gatherSignals()` / `prompt.Gather()` 中加入「本阶段期望的上游产物清单」,检查存在性、注入内容、记录缺漏到 converge.Signals 的一个新字段(`MissingArtifacts []string`)——这样 stop_condition 就可以配置一条 `missing_artifact_count == 0` 的判据(在 `evalOne` 中加一个新 metric 分支)。

---

## 方向五:Agent 运行时契约自检(Contract Self-Verification by the Runtime)

### 当前状态

ForgeOS 的大量「约定」是 agent 卡中的散文,由 LLM 自行遵守:

- **VERDICT: APPROVE 格式**(reviewer.md + cto.md):运行时只能**被动解析**agent 输出——如果 agent 忘了写 `VERDICT: ` 行,`parseReviewerVerdict` 返回 `ok=false` → fail-open(无 loop-back),无警告说「agent 未遵守契约」。
- **CONFIDENCE: <N> 格式**(product-manager.md):同款契约,同款被动解析——agent 写了才认,不写就 silently 0。
- **emits 声明**(架构师应写 `docs/design/architecture.md`):运行时解析了 `Emits` 字段,但**不验证 agent 是否真的生成了该文件**。
- **readonly 声明**(reviewer 不应写产品代码):运行时的 `readonlyToolScope` 构造了正确的 claude argv,但除非真 claude 进程报错(permission denied),否则 forge-core 无法知道 agent 是否遵守——因为 `CommandExecutor.Execute` 只看退出的 exit code,而不是检查变更的文件列表。
- **requires_tools 违反**:`requiresToolsGuard` 只在**起跑前**检查工具白名单。如果 agent 跑起来后用了未授权的工具(whitelist 没覆盖到但 claude 有),运行时不感知。

### 为什么需要

ForgeOS 的「治理」哲学是一半通过代码(harness gate / mode gating / on_fail),**一半通过 agent 卡契约**——后者目前完全靠 LLM 的诚信。在 v2 探索模式下(explorer mode,skip reviewer),如果 agent 卡文档写「reviewer 必须输出 VERDICT: 行」但 agent 没有,没有任何防御。

**真正的治理运行时需要在 agent 阶段结束后做验证**:
1. **契约遵守检查**(post-phase):若 phase 声明 `confidence_metric: requirement_confidence` 但 agent 输出中没有 `CONFIDENCE: ` 行——**记录一个 observable warning** 到 trace 和记忆体,而非静默放过。
2. **产物完整性检查**(post-phase):若 phase 声明 `emits: [task-plan.md]` 但运行后目标文件不存在——**记录一个 `KindGap`** 到 memory,让下一迭代知道「上一轮没写产物」。
3. **只读违反检测**:可在 post-phase 对 `readonly: true` 的 phase 做 `git diff --stat HEAD`——若有非声明 emits 路径的修改,记录到 honesty 信号(`FileDelta` 的镜像)。
4. **工具使用验证**:对 `requires_tools` phase,在 post-phase 检查 claude JSON envelope 中 `tool_use` 实际调用记录——若有未授权工具被调用,记录违反信号。

### 价值量化

- 🛡️ **契约治理**:从「被动解析」升级为「主动验证」——agent 不遵守契约被**可观测地记录**而非默默降级。
- 📊 **信任评分**:收敛到一个 **contract_fidelity** 度量(每 run 记录一次),积累到 memory 中。长期来看这是判断「哪个模型更守约束」的输入(与 quality_score 正交——一个模型可能写的好代码但不守契约)。
- 🔁 **自纠能力**:当 memory 中 `KindGap` 条目(contract violated)积累到阈值时,`forge evolve` 的下一迭代可以在 prompt 中注入额外警告而非仅靠 agent "自觉"。

### 具体范围(不做先发制人)

- 不实现**实时违反阻断**(kill agent thread)——agent 已跑完,事后验证,非侵入。
- 不做**语法级的 prompt 注入验证**(检测 agent 输出是否包含 `[system]` 试图劫持下一轮 prompt)——那属于上下文安全的专属域(当前已有 `sanitizeAgentOutput` + `contextMarker` 防线)。
- 只做**post-phase 契约验证 + 可观测记录**——复用现有的 `OnPhase`/`OnIteration` 回调点,验证结果注入 `converge.Signals` 的一或多个新字段,让 stop_condition 可以表达「必须先有 contract_fidelity >= 50% 才能继续」。
