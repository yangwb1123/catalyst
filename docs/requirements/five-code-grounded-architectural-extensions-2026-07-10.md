# ForgeOS: 五条代码级高价值扩展方向

> **视角**:资深架构师/产品经理。基于对 forge-core 全部 18 个 Go 包 + harness 全套 + `.agent/` 声明的完整通读。
> **不重复**:102 份已有 `docs/requirements/*` 中反复出现的同质陈述。每条直接关联到某段**当前代码**中存在的一个确定缺口。
> **不写代码**:只分析 why + what,不给出实现。

---

## 方向一:跨项目舰队治理 —— 从「单仓 OS」到「控制面」

### 当前状态
ForgeOS 今天的全部治理能力绑定在一个仓库内:
- `.agent/` 目录是每个项目自己的治理事实源。
- `forge-init` 是唯一的「全局化」机制:它复制一套模板到新仓库,但复制后两个仓库**彻底独立**——修改中央策略不会自动传播到已有项目。
- `harness/` 中的每个执法器(gate/arch-check/secret-scan/SCA/scorecard)都假设 cwd 是某个项目根,没有任何「跨项目汇总」视角。
- ADR-0003(`agent-os` submodule)至今是 Proposed 状态,位置 + 批准未拍板,零代码落地。

### 为什么需要
ForgeOS 的北极星是「AI-native 软件工程的 Kubernetes」。但 K8s 管理的是**一群节点上的容器**,不是单台机器上的一个容器。今天 ForgeOS 管理的是**一个仓库里的 AI agent 活动**——它没有「集群」概念。

如果你有 5 个微服务仓库、2 个共享库、1 个 infra 仓库:
- 你无法定义一条「所有仓库必须通过 secret-scan」的全局策略并自动传播。
- 你无法在一处 dashboard 看到全舰队 8 个仓库的 converge 状态/成本/分数。
- 你需要手动在每个仓库里升级 `harness/` 模板。
- 一个仓库的架构决策(ADR)不会自动被其它仓库的 agent 检索到。

### 核心缺口(代码级)
1. **无跨仓策略传播层**:`harness/policies.yml` 和 `.agent/policies/modes.yml` 都是本地文件,没有「中央策略 → 项目策略」的继承/覆盖/审计机制。
2. **无跨仓信号聚合**:`scorecard*.mjs` 每次处理一个 `.forge/` 目录；`forge status` 只看当前仓库。没有任何扫描 N 个 repo 的 `forge fleet status` 或 `forge fleet scorecard`。
3. **无远程治理 Registry**:ADR-0003 设计的 submodule 机制只是把 `.agent/` 模板化,不是真正的 registry(push/pull/versioning/access control)。

### 边界情况
- **策略漂移**:当中央策略收紧但某个项目因特殊原因需要豁免,如何 audit 这个豁免?今天零审计。
- **异构项目**:一个仓是 Go 服务,另一个是 Python 数据处理。全局策略如何优雅按语言/类型分支?
- **先有鸡还是先有蛋**:全局治理应设计为先有中央 registry 再接入项目,还是从 2-3 个项目临时拼接到正式 registry?

---

## 方向二:并行编排的崩溃恢复 —— per-phase checkpoint 在 wave 语义下断裂

### 当前状态
`forge run/evolve --parallel` 通过 `orchestrator/parallel.go` + `waves.go` 实现依赖波次并行执行。但**关键安全特性被显式禁用**:

```
// 来自 loop.go:
if l.Parallel && startPhase > 0 {
    l.logf("parallel mode: per-phase resume not supported — iterating from phase 0")
    startPhase = 0
}
```

同时 `Engine.OnPhase`(per-phase checkpoint hook)在并行模式下不设置。后果:如果在并行波次中(例如 discover.yml 的 scan/market/capability 三路并发),crash 后 `--resume` 从 phase 0 重启——**所有已完成并发 phase 的 LLM 调用成本全部浪费**。

### 为什么需要
串行模式下,per-phase checkpoint 是 Sprint 26 上线的关键成本控制:crash 后只重放当前 phase。并行模式下这个保护断裂了,且随着 `--parallel` 被更广泛使用(目前 5 个 shipped workflow 均未声明 `depends_on`,但设计文档已预留),这个问题会越来越痛。

更隐蔽的问题:**wave 中的部分 phase 可能已完成并产生 side effect**(写文件、批准等),crash 后重播这些 phase 可能导致重复执行。`command_executor.go:102-118` 的 SandboxConfig 注释提到「v1 placeholder」——真实沙箱不到位时,重复执行的副作用是真问题。

### 核心缺口(代码级)
1. `parallel.go:67 RunParallel` 不接受 `startPhase` 参数(硬编码从 phase 0 开始)。
2. `waves.go` 的 `groupIntoWaves` 把 phase 按依赖分组,但 checkpoint 存储的 `PhaseIndex`(int)是线性下标,与 wave 的并发分组无映射关系。
3. 没有「已完成的 phase 名称集合」的概念——checkpoint 格式只有 `PhaseIndex` 一个 int,无法表达「wave 0 的三个 phase 中两个已完成、一个未完成」。

### 边界情况
- **部分完成 + crash**:wave 0 的 phase A 和 B 已完成(产生 cost + side effect),phase C 运行中 crash。resume 后 wave 0 整体重播,phase A 和 B 的 side effect 重复执行——幂等性完全依赖 agent 自身的容错。
- **依赖链断裂**:wave 1 的 phase D 依赖 wave 0 的产出。如果 wave 0 只部分完成(phase A 写了一个文件,phase B 没来得及写),resume 后 wave 1 可能读到不一致的状态。

---

## 方向三:上下文缓存一致性 —— agent 写入物触发的失效传播

### 当前状态
`internal/prompt/cache.go(ContextCache)` 提供了精密的运行期上下文缓存:
- ADR 标题集(`adrDocs`)和 AGENTS.md 硬约束(`constraintsBlock`)被懒惰构建一次,后续所有 phase 复用。
- ROADMAP **不在缓存中**(设计正确:agent 写 ROADMAP checkbox,所以每次 phase 重新读取)。
- 提供 `Invalidate()` 方法,但 v1 **从未调用它**——注释明确说「v1 NEVER calls this」。

**问题**:当前没有任何 agent 写入物会导致缓存失效——因为 agent 根本不写 ADR。但 `design.yml` 声明 `writes_adr: {target: docs/adr/, condition: ...}`(且标注了「v2 起启用」)。一旦 ADR 真正被 agent 写入,`ContextCache` 中缓存的 ADR 标题集会变成**陈旧数据**,后续 phase 的 prompt 中将缺少新写入的 ADR。

### 为什么需要
这不是「v2 特性推迟」的问题——ADR 写入是 ForgeOS 架构脊柱的核心环节(Design → HUMAN APPROVAL → Build)。如果 agent 写了 ADR 但同一 run 的后续 phase(例如 design 阶段内的 proposal-generator)看不到这个 ADR,架构推导就会基于不完整的决策记录进行,等于让 agent 在部分失忆状态下做高杠杆判断。

更一般化的问题:ForgeOS 正在逐渐增加 agent 写入治理文件的模式(ROADMAP checkbox、scorecard 更新、memory entries、trace 事件),但**缺少一个统一的「agent 写入 → 运行时状态失效」的发布-订阅机制**。

### 核心缺口(代码级)
1. `cache.go` 的 `Invalidate()` 方法是纯手动调用——没有任何自动化检测「对应文件是否被修改」的 watcher。
2. `prompt_context.go` 中构建 prompt 的各条 lane(task/ADR/constraints/memory/gate-verdict/phase-output)各自独立读取文件,不存在统一的「文件更改→缓存失效」总线。
3. `persist.Save` 的原子写入模式(rename 覆盖)使常规的 `fsnotify` 式监控不可靠——rename 新文件在同一 inode 上发生,watch 可能漏掉。

### 边界情况
- **并发写入**:在并行模式下,phase A 写 ADR 的同时 phase B 读 ADR。在没有失效传播时,phase B 读到的是 stale 集。有失效传播时,phase A 的写入可能正好在 phase B 构建 prompt 的「写入完成但 Invalidate 未执行」的窗口内。
- **写入失败粒子**:agent 声称写了 ADR 但文件实际因为权限/磁盘满未写入。系统应检测到文件不存在,不失效也不报错,而不是静默认为「ADR 已生效」。

---

## 方向四:观测数据的质量维度 —— 当前 telemetry 只回答「花了多少」,不回答「做得好不好」

### 当前状态
Sprint 26 完成了 Learning loop 的三维真数据:quality(lint/test gate 结果)+ latency(p95 墙钟)+ cost(美金)。但这「质量」维度**只到 gate 级别**:test PASS/FAIL、lint PASS/FAIL。它不包含:

1. **代码评审评分**:reviewer agent 的 `VERDICT`(APPROVE/REQUEST_CHANGES/REDESIGN) 已被解析,但从未进入 scorecard——它驱动 loop-back 后就被丢弃。
2. **回归率**:一个之前的 implementer phase 写的代码被当前 reviewer 要求重改,这个「前功尽弃」的事件没有被跟踪。
3. **Agent 级别质量**:哪个 agent(haiku vs sonnet vs opus)产出的代码被 reviewer 打回的比例更高?当前没有任何跨 agent 的质量对比数据。
4. **测试覆盖率的变化**:`CodeTestRatio` 已被 `converge.Signals` 收集,但仅作为 warning 日志输出,从未进入 scorecard 回灌路由决策。

### 为什么需要
G3(自动模型调度)的核心承诺是「贵模型只用在该用处」。但如果没有质量维度的反馈,路由决策只能基于**成本**(BudgetAdjustTier 降档省钱)+ **禁区**(risk critical → Opus),无法做「质量-成本帕累托优化」。这使得 ForgeOS 的路由本质上是在黑暗中省钱——你不知道降档到 Haiku 后代码质量是降低了 5% 还是 50%。

更长远:scorecard-driven `HistoryTiebreak`(v1.5 标记为「non-floor agents」)的设计意图是让路由「随使用数据自我优化」,但今天的数据只有 cost 一个维度能支撑这个反馈环。

### 核心缺口(代码级)
1. `scorecard.go` 的 schema 有 `task_type`(agent 角色)字段,但 `scorecard-update.mjs` 在写 scorecards.json 时从不记录 agent 的身份——它只记录 phase name,不记录 `routing.TierFor` 输出的实际 tier。
2. `scorecard_wind.go` 中的 `scorecardPair` 可以从 trace 重新推导 agent 角色,但 `avg_cost_usd` 和 `p95_latency_ms` 是仅有的两个数值——没有任何质量数值字段供路由算法消费。
3. `engine_build.go` 的 `phaseTierResolver` 在调用 `HistoryTiebreak` 时,传入的 `history` 只包含 cost/latency 统计——质量分默认为 0。

### 边界情况
- **样本量太小**:一个新 agent(例如切换了模型版本)在头几次调用时质量数据不足,如何避免路由算法过早下结论?
- **质量归因困难**:一个 `REQUEST_CHANGES` 是因为 implementer 写得差,还是因为 reviewer 太严?今天的二分 verdict 无法区分——需要人审 cross-review。

---

## 方向五:自治循环的确定性回放调试能力

### 当前状态
ForgeOS 的核心价值主张是「24h 无人值守」。但当自治循环产出一个意外结果时(烧了过多 budget、agent 陷入了无限 loop-back、产出了错误代码),调试方式只有:

1. 阅读 `trace.jsonl`(JSONL 日志,人类可读但难以因果追溯)。
2. 阅读 `checkpoint.json`(仅记录迭代边界状态)。
3. **重新跑一次**——花真实的 LLM budget 复现问题。

没有任何办法把一次真实 run 记录成一串**确定的、可重放的脚本**:「在 phase 3 使用 agent 输出 X,在 phase 5 gate 返回 Y……」然后离线回放,验证修复是否有效,而不消耗二次 LLM 成本。

### 为什么需要
- **成本**:每次调试跑真实 agent 调用,调试成本 ≈ 生产成本的 N 倍(你每做一次「试试这个 fix」就烧一次钱)。
- **确定性**:LLM 输出是非确定的。一次 run 中出现的 bug 在下一次 run 中可能因为 LLM 输出的微小变化而不复现(heisenbug)。
- **测试**:`LoopEngine`/`orchestrator`/`converge` 当前靠 fake agent(返回硬编码输出的 mock)做单元测试。但 fake agent 的行为与真实 agent 有质的差距——你不能在 fake agent 上测试跑 24 小时后 converge 系统是否还稳定。

### 核心缺口(代码级)
1. `trace.Event` 已经捕获了每次 agent phase 的 `Status`/`Detail`/`DurationMs`/`CostUsdMicros`——所有重放所需的 agent 输出都在 trace.jsonl 中。但没有任何代码能把 trace 重放为一次 `LoopEngine.Run` 的 mock executor。
2. `command_executor.go` 的 `CommandExecutor` 和 `DryRunExecutor` 是仅有的两个 `AgentExecutor` 实现。缺少第三个实现:`ReplayExecutor`——从 trace JSONL 读取预录输出,按 phase name 匹配返回,phase 不存在时 fail-closed。
3. `loop.go` 的 `Run` 方法是纯副作用——它不返回序列化的 trace,也不接受一个「trace replay oracle」作为输入。
4. `backoff.go` 的 `Sleep` 注入点(测试用 fake clock)已经存在,`Now` 注入点(trace 用 fake clock)也已经存在——表示框架已经考虑了确定性测试,但缺少「把真实 trace 转成可重放 fixture」的入口。

### 边界情况
- **版本漂移**:回放一个月前的 trace 时,workflow/agent-card/ADRs 已经变化。ReplayExecutor 应检测到 phase 定义不匹配并诚实降级(拒绝重放而非静默用错)。
- **非确定性 gate**:gate 检查(test/lint)在真实环境可能因为系统状态不同而给出不同结果——重放时应允许 gate 结果被 trace 覆盖,还是应当真实执行?选择意味着要么接受非确定性(真实跑 gate),要么可能掩盖环境相关的 gate bug(用 trace gate 结果)。
- **部分 trace 损坏**:如果 trace.jsonl 中间某行损坏(e.g. crash 导致最后一行截断),重放应从最后一个完整 event 优雅停止,而不是 panic 或静默跳过。

---

## 对照:这些与已有 102 份 `docs/requirements/*` 的区别

| 方向 | 现有覆盖 | 本文差异 |
|------|----------|----------|
| ① 跨项目舰队 | 多份文件提「全局化」,但都停留在 forge-init 模板复制 + ADR-0003 submodule;无人触及「中央策略自动传播 + 跨仓信号聚合」的运行时架构 | 指向`harness/`和`scorecard.mjs`的本地单仓假设;指出无远程 registry 的架构空白 |
| ② 并行崩溃恢复 | 大量文件讨论 checkpoint/resume(方向一、Sprint 5 等),但无人指出**并行模式显式禁用了 per-phase checkpoint** | 直接引用 `loop.go:89-91` 的具体代码注释和 `parallel.go` 缺失的参数 |
| ③ 缓存一致性 | 多文件讨论 Context Engine/Memory/知识持久化,但无人把 agent 写入 ADR 和 ContextCache 失效传播链接起来 | 指向 `cache.go:`Invalidate` v1 零调用 + `writes_adr` 声明但无运行时检测 |
| ④ 质量维度 telemetry | 「学习闭环」「Eval→记分卡→Router」被大量讨论,但无人点出 scorecard 中**完全没有质量数值字段** | 直接引用 `scorecard.go` schema 只有 cost/latency、`HistoryTiebreak` 质量分恒为 0 |
| ⑤ 确定性回放 | `trace.jsonl` 的格式和 replay.go 的恢复机制被广泛讨论,但无人提出**把真实 trace 转成可重放 mock 的 ReplayExecutor** | 指向缺少第三种 `AgentExecutor` 实现、trace 数据已足够但零消费代码 |

