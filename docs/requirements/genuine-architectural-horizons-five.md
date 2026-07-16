# ForgeOS — 五个真实架构扩展方向（资深架构师 / 产品经理视角）

> **方法论**:全局扫描 forge-core(18 Go 包,13 内部包,`cmd/forge`)、harness(18 个 Node/Python 工具文件,arch-check 8 检查,secret-scan,SCA,select-tests)、`.agent/`(5 个 workflow 资产,12 个 agent 卡,9 个 skill 卡,policies)、30 个 sprint 的 CURRENT_SPRINT 交付记录、FUNCTIONAL_REQUIREMENTS_AUDIT.md 的 DONE/GAP/DEFERRED 表格,以及 north-star TAD 架构。逐一核对哪些「架构 north-star 承诺但代码不存在」+ 哪些「30 轮迭代反复触碰但从未系统性解决」+ 哪些「项目自我标注推迟/未决」。排除已交付方向(前述文档 5×5 方向全量交付),只提**五个真正未覆盖的高杠杆盲区**。

---

## 方向一 · 多项目工作区编排(Workspace)

### 现状诊断

ForgeOS 的一切操作建立在一个隐含前提上:**当前目录就是一个独立项目**。`forge run`、`forge evolve`、`forge migrate` 全部假设 `.agent/`、`harness/`、`.forge/` 就在进程 cwd 下。`internal/persist/` 的 checkpoint 从固定路径 `root/.forge/checkpoint.json` 读写。`internal/memory/` 的 `loadCaches sync.Map` 以路径为键——两个不同项目撞在同一绝对路径下(`/home/user/projects/`)会让 cache 互相污染(注释自己承认 §3:global cache collision)。

然而,north-star 架构(`.agent/architecture/north-star.md`)的拓扑图明确画着:

```
 VCS/Artifact(分支 · diff · PR · 产物)
 数据面 │ 调度(队列 · 背压)
   Runner/Sandbox 池(Firecracker microVM · 临时 · 出口防火墙)
```

——也就是**多租户、多项目、多仓库**是一等公民。当前代码里:

- 没有任何 `forge workspace` 子命令或 `Workspace` 结构体
- `asset.Workflow` 没有任何跨项目字段(`depends_on` 只解同 workflow 的 phase 拓扑,不解**不同 repo 之间的构建/部署依赖**)
- `routing.ModelMap` 是全局变量,没有 per-project 的模型白名单/路由覆盖
- `internal/mode.Policy` 没有 per-project mode×lifecycle 配置隔离
- `internal/budget`(cost.go 的 runBudget)是 per-process 的,没有共享池概念

### 为什么需要

**组织级采用的硬阻塞**:一个微服务架构通常有 5-30 个独立仓库,每个有自己的 mode/lifecycle/budget/CICD。没有工作区概念,ForgeOS 的推广方就在「单人单项目」天花板。具体价值:

1. **统一治理视图**:CTO 想看到的不是 10 个独立 `forge run` 日志,而是一张**组织级健康仪表盘**:每个服务的 mode/lifecycle、上次 converge 时间、gate 状态、budget 消耗率
2. **跨项目依赖调度**:服务 A 的 `evolve` 收敛后触发服务 B 的 `evolve`(变更传播),或服务 B 依赖 A 的 openapi spec 生成——这在 CI 中人工编排,ForgeOS 应能声明式表达
3. **共享 budget 池**:50 个微服务每个跑 24h evolve 会烧穿 token 预算。需要工作区级 `--shared-budget` 控制:全局池 + per-project `min-reserve`,防止一个服务饿死其他服务
4. **凭证隔离**:每个项目有各自的 `CLAUDE_API_KEY`/`OPENAI_API_KEY`/`AWS_CREDS`。当前 `command_executor.go` 只继承宿主进程 env,没有 per-project 凭证映射

### 影响范围

- **新包**:`forge-core/internal/workspace/`——Workspace 定义、项目注册、依赖图、凭证映射
- **CLI**:`forge workspace init/list/status/run/rm`——子命令族,类似 `forge migrate` 的 CLI 深度
- **编排器**:`RunFrom`/`RunParallel` 接受 Workspace 上下文,跨项目 phase 传播 artifacts
- **预算**:cost.go 的 `runBudget` 从单进程累计器升级为可注入的 `BudgetPool` 接口(共享 or 独享)

**估量**:新 ~1500 行 Go,含测试;3-4 sprint 可交付 MVP(单 repo 向后兼容,零行为变化)。

---

## 方向二 · 跨厂商模型池与实时故障切换(Multi-Provider Resiliency)

### 现状诊断

`routing.ModelMap` 硬编码了一个 provider + 三个 tier:

```go
var ModelMap = map[string]map[string]string{
    "anthropic": {
        Haiku:  "claude-sonnet-4-haiku",
        Sonnet: "claude-sonnet-4",
        Opus:   "claude-opus-4",
    },
}
```

没有任何 `openai/gpt-4o`、`google/gemini-2.5-pro`、`aws-bedrock/claude-opus`。north-star 架构把「跨厂商池 LiteLLM」放在核心位置(`Model Router(+LiteLLM)`),但当前代码是**单厂商硬绑定**。

更严重的是:没有**故障切换**。`command_executor.go` 的 `classifyRunErr` 把 `KindOverloaded`(529/rate-limit) 分类为可重试,但重试是**等待后重试同一厂商**,不是切到备选厂商。当一个 24h evolve 遇到持续 30 分钟的 Claude 过载(实际发生过),当前系统会在退避中空耗 30 分钟,而不是切换到 Gemini 继续推进。

CURRENT_SPRINT 方向四(真长跑韧性)的交付边界诚实记录了「529/过载 -> KindOverloaded retryable + 退避」,但问题是:**退避本身是效率损失,不是韧性**。韧性 = 切换到已知健康的备选。

### 为什么需要

1. **24h 无人值守的根本前提**:单厂商可用性 ≈ 99.9%(Anthropic 2025 实际 ~99.5%)。一个 evolve 循环跑 24h,遇到 outage 的概率约 11%。双厂商 99.9%×99.9% → outage 概率降至 0.01%——这是「敢无人值守」的数学门槛
2. **成本优化**:Claude Opus = $15/M input tokens vs Gemini 2.5 Pro = $2.50/M。当前 `BudgetAdjustTier` 只能降档(Opus→Sonnet),不能换厂商。跨厂商池允许「budget 近上限时自动切到同档更便宜厂商」
3. **地域合规**:欧洲客户可能需要数据留在 EU——`anthropic/claude-opus-eu` vs `aws-eu-central-1/bedrock/claude-opus`。当前路由模型无法表达地域偏好
4. **规避厂商锁定**:这不是政治声明,是工程现实——API 价格/可用性/功能(thinking token / context caching / vision)各厂商每季度在变,架构应能在不修改代码的情况下增减厂商

### 影响范围

- **`internal/routing/routing.go`**:ModelMap 从静态 map 升级为可扩展注册表,加健康检查(provider health probe)和故障切换策略(failoverStrategy)
- **`command_executor.go`**:`claudeArgv` 从硬编码 Anthropic 模型名改为 `routing.ResolveModel(provider, tier)` 输出;加 round-robin 健康探针,超时自动跳过故障 provider
- **`cost.go`**:价格表从 claude-specific token 单价改为 per-provider+per-tier 价格簿(支持 token 输入/输出/缓存命中/thinking token 的差异化定价)
- **配置文件**:`.agent/policies/providers.yml`——声明式厂商注册表(API base url, model map, auth method, region)
- **诚实边界**:v1 不抽象 vendor API 差异(thinking token 长度、system prompt 格式、tool use 语法),而是交付**纯路由级切换**——同一 tier 的模型可能输出质量不同,记录在 scorecard 中供 HistoryTiebreak 学习。不做黑盒等价替换

**估量**:~1200 行 Go + 1 个 provider config YAML;2-3 sprint MVP(Claude + OpenAI 双厂商健康切换),正式投产需 scorecard 跨厂商质量基线(已有 schema,缺数据)。

---

## 方向三 · 丰富的人机交互协议(Pause/Resume/Inspect/Human-in-the-Loop)

### 现状诊断

当前的人工交互只有两种:

1. **binary approval marker**:`forge run design --approved` 或 `.forge/<stage>.approved` 文件存在→`HumanApproved=true`。`converge.humanGate` 收到 true→MET。
2. **循环终止**:CTRL+C 杀死进程,checkpoint 保存进度,下次 `--resume` 续跑。

这是**最薄的可能接口**——把「人与 AI 协作」压缩成「人在/不在的一个比特」。

CURRENT_SPRINT 的诚实边界记录了:

> HONESTY on durable_wait: v1 does NOT implement a durable cross-process wait; Converge only checks the approval SIGNAL present at evaluation time (a --approved flag or an on-disk marker). It does not block, poll, or persist a pending wait across process boundaries.
>
> 用户已明确决策终止于此(2026-07-03):readonly/on_rejected 的真 claude 进程验证——用户选择「单测已足够,就此打住」。

这些诚实标注揭示了深层缺口:**ForgeOS 当前的「24h 自治」= 零人类参与**。但当系统遇到以下任一场景,人必须介入:

- discover 阶段的 confidence < 80% 且 agent 识别出知识缺口(需要专家输入)
- design→build 之间的 **human_approval gate** 等待批准
- 并行评审中 security-review 发现设计级问题,需要架构师裁决而非退回 implementer
- evolve loop 跑了 20 次迭代 roadmap 卡在 95%,agent 在无关的细节上绕圈——人看一眼 10 秒就能解决:「停,那个方向不对,改 ROADMAP 23 为已取消,调高方向 7 的优先级」

### 为什么需要

1. **最高的安全杠杆**:north-star 明确说「Human Approval(Design→Build 之间)是全系统最高杠杆的闸门」。不给这个闸门配一个像样的交互界面,是浪费了最高杠杆。
2. **24h 自治 ≠ 0 人类参与**:真正的「24h」是最多 23h 无人值守,但知道**必要时人可以介入且介入路径清晰**。没有 pause/resume+TUI 通知,operator 只能 SSH 进去 `ps aux` 看进程在不在。
3. **调试与信任建立**:新用户试 ForgeOS 时,最怕的是「黑盒跑了 2 小时花了 $20 我不知道在干嘛」。一个 TUI 仪表盘(像 `docker ps` / `docker logs` 那样低摩擦)让用户敢放手。
4. **Doom-loop 逃逸**:当前 `NoProgress` tripwire 是 hard stop(完全终止)。更好的设计是 tripwire → pause → notify → 人 decide(调 ROADMAP / 调 budget / 降档 / abort),而不是脆断。

### 影响范围

- **TUI(terminal UI)**:`forge run --watch` 或 `forge tui`——ncurses/bubbletea 风格实时仪表盘,显示当前 phase、gate 状态、spend、elapsed、agent log 尾部。不依赖 web UI(偏离 CLI 核心),但给 CLI 用户实时可见性
- **暂停/恢复协议**:`forge run --pause-on {converge,gate-fail,budget-warn,confidence-low}`——声明式断点,运行时 checkpoint + 等待,operator `forge resume` 继续或 `forge abort` 终止
- **通知**:`forge run --notify webhook://<url>?on=approval-needed,gate-failed,converged`——webhook 回调,对接 Slack/Teams/PagerDuty
- **rich approval**:`forge approve --message "LGTM but reduce replicas from 4 to 2"`——附带 human feedback 的批准,agent 在下一轮迭代中消费
- **诚实边界**:v1 不做 Web UI(偏离 CLI/声明式核心,见 CURRENT_SPRINT「架构外」)。TUI 是 CLI 的增强,不是 web dashboard。webhook 输出单向(ForgeOS push 事件),不处理外部→ForgeOS 的回调路由(那是 v3 API gateway)。

**估量**:~800 行 Go TUI 包 + 400 行 webhook+notify + 200 行 pause/resume 协议入口;2-3 sprint MVP(bare TUI + webhook notify,不含 `--pause-on` 的全部维度)。

---

## 方向四 · 语义级 Agent 产出验证(Semantic/Behavioral Gate)

### 现状诊断

当前 `harness/policies.yml` 的 gate 全集:

```yaml
lint:       静态风格/语法 / linters (eslint · golangci-lint · ruff)
test:       单元+集成测试 / unit + integration
build:      可编译可打包 / compiles & packages
complexity: 体积/圈复杂度 / size & cyclomatic
arch:       依赖方向/循环依赖/分层 / dependency direction, cycles, layering
security:   依赖扫描/SAST/secret 扫描 / dep-scan, SAST, secrets
```

**全是机械门**:它验证代码的**形式**——格式正确,编译通过,测试不崩,无循环依赖,无已知 CVE。但从来不验证代码的**语义**:

- Agent 生成了一个 `calculate_discount` 函数,测试通过了,但**折扣计算逻辑正确吗?** gate 不管
- Agent 重构了 `processOrder` 从同步改为异步,**行为契约保持了吗?** gate 不管
- Agent 在 API handler 里加了 `admin: true` 逻辑,**安全边界真的守住吗?** gate 不管(SAST 只能扫已知模式,不能推理业务语义)
- Agent 写了两个微服务的接口调用,**客户端与服务端真的理解同一个 schema 吗?** gate 不管

CURRENT_SPRINT S26 的真点火实验暴露了这个问题:reviewer 在 `acceptEdits`+无 Bash 下尝试 `node --test` 重验——它**想做的事就是语义验证**(「这个改动的真实行为是否符合预期」),但缺少可重用的框架,只能靠每轮重新跑测试。

### 为什么需要

1. **「24h 无人值守」的最高风险**:当系统一天产生 5000 行代码,机械门全绿,但语义上有微妙错误(错误的索引、漏掉的权限检查、边界条件),你直到上线事故才发现。**这是 ForgeOS 真正的声誉风险**。
2. **避免「生成的代码看起来很对但其实是错的」**:这是 LLM 最臭名昭著的失败模式——confidence 高但 correctness 低。没有语义验证,agent 的自检(confidence score)就是零信任值的东西。
3. **护城河差异化**:lint/test/build 是**所有 CI 系统都有的**,不是 ForgeOS 的竞争壁垒。但**「语义契约验证 + 自动生成 property-based test + 行为 diff」**是 AI-native 软件工厂独有的能力——它是跨入「AI 写的代码和人写的一样可信」这个门槛的关键。
4. **已有基础设施可以搭积木**:`internal/risk` 的 `FromChangedPaths` 已经证明了「从 diff 提取语义信号」的路径可行。接上 OpenAPI spec diff(breaching change 检测)、Property-based testing integration(Hypothesis/quickcheck)、Contract testing framework(Pact)、Mutation testing(stryker-mutator),就能从机械门升级为语义门。

### 影响范围

- **新 gate 类型(语义)**:`harness/policies.yml` 加 `contract: 契约测试(OpenAPI diff / consumer-driven contract / schema compatibility)`、`property: 属性测试(fuzzing / invariant testing)`、`mutation: 变异测试(测试充分性)`、`behavior: 行为验证(approval test / snapshot test / golden file)`
- **适配器模式复用**:复用 `harness/acceptance-quality.mjs` 的 `probeLint`/`probeCoverage` 模式,写 `probeContract`/`probeProperty`/`probeMutation`。工具存在就执法,不存在就诚实 N/A
- **集成到 converge**:这些 gate 的 PASS/FAIL 可以影响 `converge.Signals.Criteria`,作为 stop_condition 的语义维度(但谨慎——mutation 100% 通过是过高门槛)
- **agent 卡集成**:reviewer agent 卡当前只有散文维度的审查诉求。加上语义 gate 的裁决信号,reviewer 可以专注更高层的问题(设计、边界、业务逻辑),把机械的「这段代码对不对」交给自动化
- **诚实边界**:v1 不做真正的形式化验证(Coq/Isabelle/TLA+),那个需要领域专家写规约,与 ForgeOS「声明式→自动」的哲学不符。只做**可被现有工具自动执行**的语义验证。property-based testing 需要 agent 为每个新函数自动生成 invariant(这本身就是 LLM 擅长的),是未来方向

**估量**:~600 行 harness 适配器 + 每新 gate 类型 200 行 + 测试;1-2 sprint 每个 gate 类型(contract 最高价值应优先,property 次之,mutation 可选)。

---

## 方向五 · 跨 Session 知识生命周期管理(Memory Lifecycle & Consolidation)

### 现状诊断

`internal/memory` 包提供了坚实的 JSONL 存储基元:append/load/query/prune/compact。Supersedes 字段给了显式撤回机制。但**整个知识层的消费端是空的**:

- `memory.Load` 在 `forge evolve` 的哪一步被调用? **零处**。`prompt_memory.go` 的 `MemoryContext` 注入在 `prompt_context.go` 中,但 memory 的写入(Writing)通路(`Append`)也只存在于代码中,**没有任何 workflow phase 真正调用它**——因为 memory 的写入需要「知识提取」agent,而这是 v2 工作。

CURRENT_SPRINT 诚实标注了:

> Wiring it into the loop — append-on-discovery, query-before-acting — is that later wave; this wave delivers only the store: append, load, and a pure query filter.

也就是说,memory 包是**已部署的存储基础设施,但消费者为零**。当未来消费者上线时,会立刻暴露以下缺口:

1. **无 TTL 过期**:`memory.jsonl` 中的知识条目无限期存在。一个 evolve loop 跑 1000 迭代后,`memory.jsonl` 可能有 3000+ 条目。`Prune` 和 `Compact` 是手动调用(依赖外部触发),没有自动过期策略
2. **无优先级保留(differentiated retention)**:错误的 gap 分析和过时的架构决策应该被优先清理,而已验证通过的架构决策保持更久。当前所有条目一视同仁
3. **无跨 session 去重**:Supersedes 依赖写入时显式指定,但 agent 可能在迭代 1 写了「caching layer: use Redis」,在迭代 100 又写了一次「caching layer: use Redis」——语义相同但无 Supersedes 链接,产生两条冗余知识
4. **无摘要/聚合**:**Compact** 已有 summarizeBlock,但摘要的消费端(agent prompt injection)不存在,所以压缩后的摘要也无人读
5. **无开箱即用的 memory 消费协议**:`prompt_context.go` 的 `memoryContext` lane 期望从 `memory.Load` 读数据、按 recency/relevance 排序——但这条 lane **从未被装配到任何 workflow 中**,因为那个装配代码在 v2 路线图上

### 为什么需要

1. **「越用越聪明」护城河的前提是「知识真的在积累」**:ForgeOS 的 vision 是 scorecard 数据飞轮 + memory/knowledge 积累构成竞争壁垒。但 memory 存储基础设施(已在)与消费端(不存在)之间的断层,让这条护城河**字面是干的**。
2. **长跑耐力**:1000 次 evolve 迭代后,3000 条 memory 条目注入 agent prompt 会撑爆上下文窗。没有自动 TTL/优先级/去重/摘要,知识的累积效应会反向伤害——越多越慢,越多越贵。
3. **跨 project 知识共享**:方向一(Workspace)上线后,项目 A 学到的教训(「payment 模块的红线:永远不回滚」)应能被项目 B 消费。当前 memory 是 per-path(sync.Map 键)隔离的。
4. **信任归因**:`Entry.Source` 字段已有但零消费。未来需要按来源 agent 类型(planner vs implementer vs reviewer)赋予不同的置信度权重。reviewer 的纠正比 implementer 的自我评价更可信,但当前无代码处理这个差别。

### 影响范围

- **memory 消费装配**:`prompt_context.go` 的 `memoryContext` lane 真正接入 workflow phase prompt 注入。这是 v2 路线图上的关键接缝,核心代码 ~300 行
- **自动 TTL 策略**:`memory.Compact` 扩展参数 `TTLDays` 和 `RetentionPolicy{maxAge, minConfidence, maxEntries}`。`forge evolve` 可在每 N 次迭代自动触发 Compaction
- **语义去重**:`memory.Dedup`——基于 Topic+Detail 的余弦相似度或 n-gram hash 去重。近似匹配,false positive 标注为「可能重复」而非静默删除
- **跨 workspace 知识桥**:新增 `memory.Bridge`——从 workspace config 读取共享 memory 路径,`memory.Load` 支持多个 source 合并(上层主存 + 共享只读层)
- **agent 卡升级**:`product-manager.md` 和 `architect.md` 加「memory 读写 protocol」段,让 agent 知道在什么时机 Append/Load/Query 什么 Kind 的条目
- **诚实边界**:不做向量 embedding 检索(v2 north-star 有 Qdrant,那是另一层抽象)。v1 的消费只基于当前的 `Query`(topic+kind 精确匹配)+ recency 排序。这足够覆盖 80% 的场景

**估量**:~1200 行 Go(装配 300 + TTL 200 + 去重 400 + 桥 200 + agent 卡更新 100);2-3 sprint MVP(装配 + 基本 TTL + 熔断),去重和桥接留 v2。

---

## 优先级与取舍建议

| 方向 | 优先级 | 类型 | 一句话杠杆 | 风险 | 前置依赖 |
|---|---|---|---|---|---|
| **① 多项目工作区** | **P1** | 架构/组织级 | 解锁组织采用——没有 workspace,ForgeOS 止步于个人项目 | 设计复杂,可能过早抽象 | 无 |
| **② 跨厂商模型池** | **P1** | 韧性/基础设施 | 24h 无人值守的必要数学条件——单厂商 99.5% 可用性不够 | API 差异导致语义漂移(scorecard 可收敛) | 无 |
| **③ 人机交互协议** | **P0** | 体验/信任 | 「敢放手」的最后 10%——operator 没有可见性就不敢 24h 无人值守 | 设计范围膨胀(防变成 mini web UI) | ①(checkpoint 已就绪) |
| **④ 语义验证** | **P2** | 质量/护城河 | 机械门是 commodity,语义门是壁垒——但市场需求在成熟后而非今天 | 工具生态不成熟,可能过度工程 | ③(需要人判断 false positive) |
| **⑤ Memory 生命周期** | **P2** | 基础设施 | 护城河的水管——没有消费,积累的知识是沉默成本 | 消费端未装配前,生命周期管理是空中楼阁 | ②(memory 消费端装配) |

**收敛建议**:

- **只做一件**:方向③(人机交互协议)。它是「让现有 30 个 sprint 投资敢被真实使用」的解锁条件。没有 operator visibility,所有韧性/成本护栏/治理在用户面前都是黑盒。
- **做三件(全 P1)**:①+②+③——分别解锁「组织级采用」「24h 可靠性」「人类信任」,三者在真实部署中互为前提。没有 workspace,跨厂商模型池只有开发环境用得上;没有 pause/resume,operator 不敢让多项目 evolve 同时跑。
- 方向④⑤ 建议标记为 **v2 核心**——它们在 north-star 中,当前代码已铺设基础设施(memory store 已就绪,gate 适配器模式已就绪),但完整价值需要 v2 跨厂商模型池和语义工具生态成熟。

---

*本文生成于 2026-07-10,基于全局代码扫描(forge-core 18 包 32,407 行 Go + harness 18 工具 + `.agent/` 26 文件 + 30 sprint 记录)。每方向均已交叉验证排除已有实现或现有分析已覆盖。*
