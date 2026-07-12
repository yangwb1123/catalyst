Now I have comprehensive context. Let me provide the full architectural analysis.

---

# 架构分析报告:ForgeOS「验证结论」深化评估

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的架构在同类系统中表现出了罕见的**工程诚实度**和**可验证性**。核心优势体现在:

**信任最小化的收敛判据**(Largest Delta over Devin/OpenHands)
- 收敛判据(`roadmap_completion==100% AND gates green`)由**零-LLM Go+harness 进程**独立测量,执行 agent 无法撰写或伪造。这是对 ReAct/Reflexion/AutoGPT「同一模型既执行又自判完成」范式的结构性突破。
- `N/A` 永不等于 PASS,`provenCount==0` 不构成 green,vacuous-green 守卫等诚实代数已在 `converge.go` 代码中兑现——这套**可证伪性工程**是 ForgeOS 最深的护城河。

**分层清晰的控制器架构**
- `forge run`(串行 workflow 引擎)和 `forge evolve`(LoopEngine 包裹 workflow 引擎)的**包含关系**验证了「收敛环 ≠ workflow 图」的设计:裸回边(`on_fail:loop_back`)活在单趟 `RunFrom` 里,收敛引擎在其上叠加活体判停、安全底线、反 doom-loop、跨迭代状态。代码中 `LoopEngine` 的确包裹而非替换 `Engine`,这是教科书级的关注点分离。

**模式严格的带外执法**
- 以 `harness/` 为真相之源的载重墙设计,使 All ForgeOS 工程红线(函数 ≤50 行、循环依赖 =0、单向依赖层、secret 扫描等)可被**不带任何宿主知识**的独立进程执行。architecture/north-star.md 中的「能力契约/适配器」原则已在 v2 代码中具象化为可验证实体的`arch-check.mjs`+`secret-scan.mjs`+`gate.mjs`。

**单一旋钮(mode×lifecycle)驱动三子系统**
- Router 档位 + Harness 严格度 + Workflow 深度,三者正交但不重复。已实现 production 一票否决、fail-safe 全开不减配等 fail-closed 特性。这是 k8s 式「控制面 vs 数据面」分离的微小但诚信的落地。

### 1.2 局限性(当前架构债务)

| 领域 | 现状 | 债务类别 |
|------|------|----------|
| **并行执行安全** | `--parallel` 已交付,但 `CommandExecutor` 直接写工作树,wave 内无任何写冲突检测或锁 | **安全债**(并行写 = 竞态数据) |
| **YAML 处理** | 通过 `harness/yaml2json.py` shim 的子进程 shell out,Go 运行时无原生 YAML 解析 | **基础设施债**(Go 标准库无 YAML,零依赖策略导致此 shim) |
| **内存语义检索** | `boundMemory`(关键词 BM25-lite)顶了语义检索的缺;`filterSuperseded` 只能精确 topic 匹配,无自动矛盾检测 | **功能债**(假语义检索可能导致 24h 运行中上下文污染) |
| **Control Plane 韧性测试** | `trace.Tracer.Now()` 和 `Engine.Sleep` 有注入点,但**无系统化故障注入测试** | **测试债**(混沌工程从未引入) |
| **cmd/forge 文件数** | 反复顶到上限(14→16→17),每次靠自然拆分消化 | **结构债**(CLI 包仍偏厚) |
| **phase 间产出物契约** | `emits:` 只是文件路径列表,无 schema 声明,格式正确性依赖 LLM 顺从 | **接口债**(无 schema 校验的 phase 边界) |
| **Scorecard 冷启动** | `scorecards.json` 需真跑产生,永久冷启到首跑 | **启动债** |
| **Agent 默认 Dry-run** | `--executor=dry` 是真跑的安全默认,但 dry-run 下阅读代码路径的人可能误以为系统工作 | **认知债**(诚实但信息不对称) |

### 1.3 关键设计决策评估

**D1:Go-核心 polyglot 时序**
已于 v2 落地,纯标准库、零外部依赖,13 包 140+ 文件。**决策合理**——Go 的类型安全、并发原语、工具链一体性让其作为编排层/控制面底座是正确选择。Python 留作智能层(Rust 沙箱再晚一轮)的退让也务实。

**D6:forge-core 启动时机(2026-06-19)**
当 `url-shortener` dogfood 被 `forge accept` 实际 gate 后,ADR-0001 的取代条件触发。**这一时序决策是模式级的正确**——用最小胶水验证核心循环后,再用真实数据指导运行时设计,避免了过度工程。

**D3:带外 gate 是真相之源,CC hook 是加速器**
已在代码中得到严格遵循。`gate.mjs` 在三处执法(edit-time / Stop / CI),且 host-independent。此决策对多宿主(Claude Code / Codex / Gemini CLI)的未来支持是前提条件。

**Lock Order Contract(8 级锁序)**
`parallel.go:31-52` 的锁序契约是**必要的技术债承认**。代码诚实文档化了「共享可变状态的并发安全属于最危险的那类 bug」,但在 v1 中这是合理折衷——并行执行的边际价值 vs 全 lock-free 重构的成本。

**`Supersedes` 精确 topic 而非语义匹配**
`memory.go:130-134` 的显式撤回机制是务实的最小可行。在无 embedding 基础设施下做语义矛盾检测会镀金;但`memoryCap=32`下的硬截断+BM25-lite 排序在 24h 持续积累下一定会丢失信息。**这是一个需要跟踪到 v3 的功能缺口**。

---

## 2. 扩展方向

基于验证报告和代码库审计,我给出 5 个扩展方向,按**价值/风险比**排序:

### ⭐ 方向 A:并行写冲突检测基础设施(P0,高价值·中等风险)

**为什么需要**:`--parallel` + `waves.go` 拓扑排序已交付,`CommandExecutor` 直接写工作树且 wave 内无协调——两个独立 agent 同时修改同一文件的同一区域必然导致数据竞争。这不仅是工程质量问题,是**安全合规问题**:在一天 24h 无人值守的自治场景下,静默写冲突可能导致用户丢失代码。

**核心挑战:**
1. **Go 用户的文件锁不是跨进程的**:`flock`(BSD)/`LockFileEx`(Windows)是整个 POSIX 的约定,但跨语言工具链(Go agent 调用 `claude`,claude 调用 `sed`/`git`/`node`)的写入锁需要 **OS 级租约**而非用户态互斥。
2. **git tree 粒度的冲突检测 vs phase 粒度的文件级锁定**:细粒度锁(每文件)与 wave 的「独立」假设矛盾——如果两个 phase 真的独立,锁应是空操作;加锁得证明「非独立」。
3. **向后兼容**:已有所有非 parallel 路径必须零行为变化,`--parallel` 下串行演退必须完整。

**设计考量:**
- **选项 A(推荐):Git-based conflict detection**——wave 结束后,在 phase 边界做 `git diff --check` 或定制的冲突检测(`git merge-file` 的 `--diff3`)。代价:冲突只在 wave 结束时检测,agent 已浪费;好处:不需要侵入 CommandExecutor。
- **选项 B:Worktree copy-per-phase**——每 phase 拿到工作树的 git snapshot(soft copy 或 `git worktree add`),写完后 diff→merge。隔离性强,但 IO 和磁盘开销高;v2 的纯 stdlib 限制下难实现。
- **选项 C:Lock server in-process**(慎用)——在 `Engine` 里加一个文件级 Write Locker,`CommandExecutor` 的 pre-exec hook 持有所需文件集的锁。v1 中镀金,适合 v3 分布式沙箱场景。

**预期架构变更:**
- `internal/orchestrator/parallel.go` 加冲突检测阶段,在每 wave 后跑 `git diff` 解析冲突
- 新包 `internal/conflict`(或放在 `internal/git` 下)封装 diff 冲突解析
- `CommandExecutor` 可能需要 post-exec callback 或 `OnWrite` hook(用于轻量非锁式观察,不是锁)

### ⭐ 方向 B:上下文毒性自动检测(P1,中等价值·中等风险)

**为什么需要**:`memoryRecencyFloor=8/32`硬截断而非智能选择、无时间衰减的 BM25-lite 排序(filterSuperseded 依赖显式 opt-in 无自动矛盾检测),在 24h+ 的自主运行中必然导致上下文污染——agent 被自产自销的过去错误决策干扰,且不知如何否决。**自动矛盾检测是自治系统长期正常运行的前提条件**。

**核心挑战:**
1. **句法级否定检测(你称之为 v2 目标)的定义**:两个 memory entry 何时才算矛盾? "port 8080" vs "port 9090"? "use PostgreSQL" vs "use SQLite"? 前者是精确字符串匹配的题,后者是**架构决策层面**的语义矛盾。
2. **矛盾检测的位置**:在 `boundMemory` 时做,在 `Retrieve` 时做,还是在 `Append` 时做? 时序决定了方案。
3. **性能**:每轮 query 对所有 entry 做 O(n²) 的矛盾检测,在 24h 积累后可能影响编排延迟。

**设计考量:**
- **选项 A(实用):Supersedes 启发式提升**——让 `filterSuperseded` 支持**通配符或路径前缀**(`Supersedes: "port-*"`),而不是精确 topic 匹配。不改 `memory.Entry` 的结构,只改过滤逻辑。这是最小的 MVP。
- **选项 B(推荐):跨 entry 结构矛盾检测**——在 `memory.Compact` 阶段,对同 kind(KindDecision/KindGap/KindLesson)的 entry 做定式矛盾检测:Key-Value 映射(如 `port:8080` vs `port:9090`)、决策方向相反(use-X vs use-Y)。需要新字段 `Contradicts []string` 或统一在 Compact 时推导。
- **选项 C(蓝图,不现在做):Embedding-based**——需要向量数据库嵌入模型,是 v3 的内容。不推荐在纯 stdlib 范围内做。

**预期架构变更:**
- `internal/memory/memory.go`:`Entry` 可能加 `Contradicts []string` 字段(`Supersedes` 的对称补)
- `internal/memory/memory_compact.go`:加矛盾检测逻辑,输出 `ConflictEntry`(新 kind)
- `cmd/forge/prompt_memory.go`:`boundMemory` 在超过 `memoryCap` 选择 relevant 时,优先排除被矛盾标记的旧 entry
- 时间衰减进入 memory query:在 `memory.Query` 加 `WithDecay` 选项,在 `boundMemory` 消费

### ⭐ 方向 C:阶段间产出物形式化契约(P0,低风险·高价值)

**为什么需要**:验证报告确认 `emits:` 仅为文件路径列表,无 schema 声明,`parseConfidenceScore` 只在 ledger 消费时解析而非在 phase 边界校验。当前格式正确性依赖 LLM 顺从——在自治场景下,**一个 format error 可能导致下游 phase 静默吞掉有价值输出**。与 `on_fail` 和 loop-back 结合,这是收敛可靠性关键。

**核心挑战:**
1. **Schema 语言**:是用 JSON Schema、Go struct tag + 反射、还是自定义的轻量表达式? JSON Schema 是标准但引入外部 dep,违反零外部依赖红线。Go 反射是 stdlib 但冗长。
2. **向后兼容**:已有 workflow 的 `emits:` 路径列表需要继续工作,不能要求所有 history 之前的数据加上 schema。
3. **校验时机**:起跑前校验(lint)→校验速度,运行时校验→保正确性,收产物时校验→最小阻抗。

**设计考量:**
- **选项 A(推荐):.artifact 侧车文件**——在每个 `emits:` 指定的目录放 `.artifact/emit_schema.yaml`(或 JSON),描述该阶段产出的 schema(字段名、类型、必需/可选)。消费方在 `Gather` 时验证;schema 缺失则不校验(向后兼容)。
- **选项 B:Workflow YAML 内联 schema**——直接在 `emits:` 旁加 `emit_schema:` 块,如 `emit_schema: {type: object, required: [confidence], properties: {confidence: {type: number, min: 0, max: 100}}}`。更紧凑但 YAML 内复杂度上升。
- **选项 C:Go reflect + tag-based**(0 依赖推荐)——`emits:` 路径读入内存后映射到 Go 结构体,用 `json.Unmarshal` + struct tag 校验。最简单,但 schema 本身只能用 Go 类型系统表达,不能运行时扩展。

**预期架构变更:**
- 新包 `internal/artifact` 或放在 `internal/workflow` 下
- `orchestrator.RunFrom` 在 phase 切换处加 hook `validateArtifact(phaseName, emits)`
- 延迟:仅校验 `emits:` 路径被声明的条目;不在已有全量数据上 backfill validator

### ⭐ 方向 D:选择性相位执行(`--phase-from`/`--phase-to`)(P1,低风险·高价值)

**为什么需要**:犬儒论证:开发者花费 3 小时迭代 `implementer` 阶段,不想每次从头跑 discover+design+review;但 `forge run` 总是从 phase 0 开始。验证报告确认无此 flag,也无 `--skip-gates`。`forge migrate --to` 已有先例(`migrate.go:17`),证明团队已经理解 `--to` 模式的价值。

**核心挑战:**
1. **语义**:`--phase-to` 是到某个 phase 停(last inclusive/ exclusive?)、还是到 stop_condition 停?
2. **依赖**:如果跳到 phase 3,phase 0-2 的产出物可能缺失——`--phase-from` 需要与 `resume`(checkpoint) 协同,确保跳过的前置 phase 的产出物已在本地。
3. **回边**:有 loop-back 的 build.yml 中 `--phase-to=implementer` 的语义是什么——loop-back 定向跳是否被允许?

**设计考量:**
- **选项 A(推荐):`--phase-from` + `--phase-to` 为软边界**——低于 `--phase-from` 的 phase 被跳过(不执行,不算失败),`--phase-to` 后的 phase 被跳过。若 `--phase-from` 的 phase 依赖缺失,则 fail-fast 提示跑 `forge run --phase-check` 验证依赖就绪。
- **选项 B:复用 checkpoint resume 路径**——`--phase-from 3` 等价于从 checkpoint 3 resume。更自然,但 checkpoint 路径要求之前有 checkpoint——从零开始就不能直接用。
- **选项 C:组合 `--skip-phases` 和 `--only-phase`**——更细粒度,但与 `--phase-from/to` 重复。建议先做前者,若用户明确请求再补后者。

**预期架构变更:**
- `main.go` 加 `--phase-from`/`--phase-to` flag
- 在 `RunFrom` 的循环里加 `skip` 判断,与 `stageSkipped` 模式一致
- 依赖校验:在 phase 跳过后,若检查点不存在则报错而非静默失败

### ⭐ 方向 E:控制平面系统化故障注入框架(P2,中等价值·高风险)

**为什么需要**:验证报告确认 `trace.Tracer.Now()` 和 `Engine.Sleep` 有注入点但无系统化故障注入测试。在 24h 自治运行的场景下,**编排器必须对以下故障具有确定性韧性**:checkpoint 写入失败、LLM 调用超时、子进程 OOM、网络分区、时间跳跃。当前只有零散的测试覆盖这些场景,没有系统化的混沌工程框架。

**核心挑战:**
1. **幂等性**:故障注入必须在**测试**时生效,在生产路径上零开销。Go 的 `interface` 注入在测试时是好的,但需要确保生产编译不会被优化掉。
2. **覆盖率**:当前只有 `trace.Tracer.Now`,`Engine.Sleep`,`backoff.Backoff`(通过 Engine.Sleep)有注入点——要系统化做故障注入,需要**列出所有可能的故障类别**(IO、超时、取消、OOM、子进程 crash、时间扭曲)并对每个类别暴露 injectable hook。
3. **并行测试**:故障注入与 `--parallel` 模式相互作用——wave 内的并发故障更难复现和断言。

**设计考量:**
- **选项 A(推荐):测试用故障注入接口**——在每个 resilience-critical 接口加 `FaultInjector` hook(如 `Checkpointer.FaultInjector`、`CommandExecutor.FaultInjector`)。不在生产路径上加分支,只在测试时通过 `WithFaultInjector` 设置。模式:Go 的 `http.Handler` 中间件包装。
- **选项 B:独立混沌编排层**——类似 LitmusChaos 的测试层面,在集成测试中随机 kill/延迟子进程。不修改 forge-core 代码,但需要外部依赖。
- **选项 C:不做,依赖系统级容错**(不推荐)——假设 OS 级超时/取消/重试已足够。在 24h 无人值守场景下,这些假设已被证明成立(Sprint 24-26 真跑)。**但如果加入 read-only 强制、复杂 loop-back 等后,不做故障注入是假信任。**

**预期架构变更:**
- 新包 `internal/fault`(极小,只定义 `Injector` 接口 + `NoopInjector`)
- 在 `internal/orchestrator/backoff.go`、`internal/persist/checkpoint.go`、`internal/trace/trace.go` 加可选的 `fault.Injector` 字段
- 现有测试扩展:为每个包的 resilience 路径加随机故障注入测试

---

## 3. 接口设计建议

### 3.1 关键模块接口设计原则

**1. 接口的最小化原则**
ForgeOS 已经做得很好——`trace.Tracer` 只有 `Emit`/`Now`/`Close` 三个方法。`Engine` 的导出符号也保持适度。新增基础设施时应严格遵循:**一个接口暴露少于 5 个方法,如果超过则拆解**。

**2. Injectable 优先于子类化**
当前 `Engine.Sleep` 被 `CommandExecutor` 和 `backoff` 间接调用——若需修改定时行为,通过 `Engine.Sleep` 接口注入而非创建 `SleepyEngine` 子类。这个模式应推广到所有时间、clock、IO 敏感逻辑。

**3. 可观测性是一等接口,不是日志装饰**
`trace.Tracer` 已经是独立接口。`Engine.OnGateResult`/`costSink`/`Observe` 等回调模式应作为**默认模式**而非 afterthought。新子系统(collision 检测、artifact schema 校验)应该默认暴露 `Observer` 式的回调接口,方便 scorecard 消费。

### 3.2 是否需要新的抽象层

**需要:Artifact Contract 层**
当前 `emit:` 是纯文件路径列表。从 RunFrom→RunParallel 的 phase 切换需要知道:① 产出物是否已就位,② 是否符合下游预期的 schema,③ 是否被依赖 phase 声明了写入锁。建议在 `internal/artifact` 下封装:

```
type Artifact struct {
    Path   string          // emit 路径
    Reader io.ReadCloser   // 读取器(phase 完成后可消费)
    Schema *Schema         // nil=无 schema(向后兼容)
    Hash   string          // 内容哈希(冲突检测用)
}
type Contract struct {
    Emits   map[string]*Schema  // phase 声明的产出 schema
    Expects map[string]*Schema  // phase 依赖的输入 schema
}
```

**不需要:独立「Convergence」层作为运行时包**
`converge` 当前在 `internal/converge` 中作为纯逻辑包独立存在,这是一个正确的设计决策。但**不应提取为独立的运行时服务**(像 north-star 中的「Eval Engine」),直到 v3 的分布式架构落地。

**可能不需要:统一 Lock Manager**
在 v2 纯 stdlib 约束下,一个用户态的 Go Lock Manager 无法跨进程锁定(无法锁住 `claude` 的写操作)。与其创建一层假锁,不如把冲突检测的权威放到 git 层面(`git diff --check`),让版本控制基础设施天然解决写冲突。

### 3.3 向后兼容性策略

已验证的 ForgeOS 模式(基于 Sprint 审计历史):
1. **零值兼容**:新字段/flag 的零值等价于旧行为(`MaxAgentCalls=0 → 无限`)。已在 budget 和 agent-call guard 中验证。
2. **文件存在性作为 feature gate**:checkpoint 文件存在→走 resume,不存在→走全量。artifact schema 文件缺失→无 schema 校验。
3. **env over CLI over config**:`FORGE_MAX_AGENT_DEPTH`→`--max-agent-depth`→`project.yml`。已有模式,适用于新引入的 flag。
4. **N/A 兼容**:不校验 schema 时,产出物走 N/A,与 SCA/lint/coverage 的已有 pattern 一致。
5. **fail-closed 而非 fail-open**:新模式下的未知配置值一律取最保守(全开或全关视上下文)。已有模式(Sprint 7 fail-safe→全开)。

---

## 4. 技术选型

### 4.1 是否需要引入新技术

| 领域 | 是否需新技术 | 理由 | 建议 |
|------|------------|------|------|
| YAML 解析 | **是** | Python shim 是开发时充分但运行时不可靠的桥接——Go 原生 YAML 包免 shell out。但需打破零外部依赖红线 | **有条件引入**:评估 `gopkg.in/yaml.v3`——市场标准、MIT,需要 architect 决策拍板打破依赖红线 |
| 语义检索 | **否** | BM25-lite 对 v2 够用;v3 引入 Qdrant 时再桥接 | 保持现状,`boundMemory` 关键词检索在 32 条上限下合理 |
| 冲突检测 | **否** | 可用 `git diff --check` 已有组件 | 不需要新库,需要新集成 |
| Schema 校验 | **推荐纯库** | 零依赖约束下可用 Go `encoding/json.Unmarshal` + struct tag 或自定义 schema DSL | 自研轻量 schema 解析器(几十行 reflect);不使用 JSON Schema 库 |
| 故障注入 | **否** | 接口注入模式已存在(`Now`/`Sleep`),无需新框架 | 不需要第三方库 |
| 进程级文件锁 | **考虑** | `flock` syscall 是 POSIX 标准,Go 标准库 `golang.org/x/sys/unix` 但也是外部 dep | v2 内不做——跨进程锁的设计复杂度超过当前并行执行的实际 usage;等真实多 agent 竞态 bug 出现再引入 |

### 4.2 零外部依赖红线的评估

`go.mod` 无 `require` 的状态对 v2 forge-core 是**正确但有代价的约束**。纯 stdlib 降低了供应链攻击面、构建部署简化、可审计性最高。但是:

**建议修正**:引入 `gopkg.in/yaml.v3`(或等效,最少依赖)作为**唯一的、明确的、经过架构决策批准的**外部依赖。理由:
1. 当前 `yaml2json.py` shim 是 shell out + 子进程,是**比一个 dep 更大的风险面**(无类型检查、子进程失败覆盖不完全)
2. python shim 可被替换后仍保留为可选降级路径(没有安装 Python 的环境 fallback 到 `gopkg.in/yaml.v3`)
3. 已有大量测试 YAML 黄金文件,迁移后完全相同

这是**锚定**级别的决策——不再新增第二个外部依赖(严格维持 v2 的极简供应链),但批准这一个。

### 4.3 自建 vs 采购

ForgeOS north-star 已清晰地划界:采购 Temporal/LiteLLM/Qdrant/NATS/OTel/Firecracker/OPA/Vault/PG,自研编排逻辑/治理模型/路由决策+记分卡/Context/角色体系/适配器/Eval/UI。

与验证报告中的方向对应的判断:
- **并行冲突检测**:自研(无商业产品满足需要)
- **语义检索**:v3 采购 Qdrant;v2 自研 BM25-lite
- **Schema 校验**:自研轻量 DSL(L 计)
- **故障注入**:自研接口模式(不算独立方案)
- **Phase-from/to 执行**:自研(cli flag + RunFrom 扩展)

**没有任何方向需要采购做决策**。

---

## 5. 实施路线图

### 5.1 优先级排序

```
P0 (下一轮):  方向 C (产出契约) + 方向 A (并行冲突检测)
P1 (近期):    方向 D (选择执行) + 方向 B (自动矛盾检测)
P2 (中期):    方向 E (故障注入) + YAML 依赖决策
```

**理由**:P0 的两个方向是**收敛可靠性的必要条件**——没有 schema 校验,格式错误会导致下游静默吞数据;没有冲突检测,并行执行的结果不可信。P1 增强开发者体验(D)和长期自主鲁棒性(B)。P2 的故障注入和 YAML 依赖是系统韧性/基础设施债,韧性上真跑已验证稳定、YAML 依赖需要架构决策拍板。

### 5.2 阶段划分

**Phase 1:「phase 边界完整性」(2-3 sprints)**
- 方向 C(产出契约):设计 schema 语法(极小 DSL)→实现 `internal/artifact` →接 `RunFrom`/`RunParallel` →向后兼容 N/A → 写 `FUNCTIONAL_REQUIREMENTS_AUDIT.md` GAP 收口
- 方向 D(选择性执行):`--phase-from`/`--phase-to` flag→`Engine.SkipPhasesBefore`/`SkipPhasesAfter` →依赖校验→checkpoint/resume 感知

**Phase 2:「并行安全」(2-3 sprints)**
- 方向 A(冲突检测):`git diff --check` post-wave hook→ `internal/conflict` 包→冲突报告格式→冲突恢复策略(自动三方 merge? OR 标记冲突→回退→重跑? OR 直接 abort?)→ `--parallel` 集成

**Phase 3:「上下文质量」(2-3 sprints)**
- 方向 B(矛盾检测):`Supersedes` 通配符→`Contradicts` 字段→`Compact` 矛盾检测→时间衰减进入 memory query →测试 Memory 一致性

**Phase 4:「韧性工程」(2-3 sprints)**
- 方向 E(故障注入):`internal/fault` 接口→`WithFaultInjector` 在 resilience 路径→系统化随机故障测试
- YAML 决策:architect/CTO 拍板引入 `gopkg.in/yaml.v3` →替换 shim →保留降级路径

### 5.3 风险点和缓解策略

| 风险 | 等级 | 缓解 |
|------|------|------|
| 并行写冲突引入竞态 | **高**——并行执行的安全核心 | 先做 post-wave 检测(观察模式),确认冲突频率后再决定是否需要 pre-wave 锁(干涉模式) |
| Schema DSL 设计过渡工程 | **中**——JSON Schema 引入外部 dep,自研 DSL 可能重复造轮 | MVP = 只 `json.Unmarshal` + struct tag,不做 DSL;看到真实 schema 模式后再提炼 |
| `--phase-from` 跳过依赖导致静默失败 | **中**——跳过前置 phase 可能缺产出 | 依赖校验 + 显式警告,如果缺依赖则 fail-early;不要让用户在运行中才发现 |
| 矛盾检测的假阳性/假阴性 | **中**——启发式可能误判或漏判 | `Compact` 阶段的矛盾检测只写 `ConflictEntry` 做推荐,不删除原 entry;`boundMemory` 时可以按权重排除受矛盾影响的旧 entry |
| 抗性工程「为测试而测试」 | **低**——做假故障注入的可能性 | 遵循「每个 resilience 点已有可注入接口(如 `Now`/`Sleep`/`OnGateResult`),扩展模式而不是发明新框架」 |
| YAML 依赖评估耗时过长 | **低**——架构决策环节 | 预研 `gopkg.in/yaml.v3` 在 forge-core 当前 13 个包中全部依赖关系,给有量化证据的决策 |

---

## 总结

ForgeOS 的架构质量在同代 AI 编排系统中显著领先——其信任最小化的收敛判据、带外执法的载重墙、单一旋钮驱动三子系统的正交性,以及贯穿代码库的工程诚实文化(诚实代数、N/A 不伪造、零值兼容),构成了一组在实践中罕见的**架构可信度**。

验证报告中确认的五个方向——并行写冲突检测、上下文毒性检测、选择性相位执行、阶段间产出物形式化契约、控制平面故障注入——全部是真实的 gap,且每一个都有清晰的、已知模式的可扩展路径。最可贵的是,在当前代码库中,每个方向都已经有实现的先例(`wires.go` 的 waves、`filterSuperseded` 的显式撤回、`migrate --to` 的 `--to` flag、`check.py` 的 schema 校验框架、`Now` 和 `Sleep` 的注入点)可以借鉴。

推荐的启动顺序(C→D→A→B→E)比验证报告的建议(③→②→①→④→⑤)更偏重**相位边界完整性先行**,因为:
- 产出契约(C)和选择执行(D)不修改既有路径、可并行起步,**风险最低**
- 产出契约直接补了当前最危险的收敛缺口:phase 间格式正确性依赖 LLM 顺从
- 选择执行在 dogfood 中将被高频使用——架构师迭代 `implementer` 而不想从头跑 discover/design,**这个需求每天都会撞到真实 developer**

等 C + D 落地并 dogfood 几轮后,再开工 A(并行安全)时,团队对 forge-core 的熟悉度和测试基础设施完备度会更高。
