# ForgeOS — 五个隐匿子系统级缺口

> **范围**:全局扫描至 2026-07-11（`forge-core/` 18 Go 包 · `harness/` 全套闸门 · `.agent/` 完整声明骨架 · `docs/requirements/` 114 篇已有分析）。
> **方法**:从最终用户/运维者的真实场景出发回溯代码库,而非从现有代码向前推。每个方向先问"如果有人要把 ForgeOS 用在一个关键生产流水线上 24h 无人值守跑一个月,什么东西会让他睡不着觉?",然后逐层验证代码中是否已有答案。
> **结论**:5 个方向,每个均附代码证据 + 为什么高价值 + 为什么此前未被覆盖。

---

## 方向一 · 多仓库编排面 —— 单 `--root` 宇宙之外没有世界

### 问题

ForgeOS 的 `--root` 参数贯穿整个代码栈,从 `main.go` 的 `RepoRoot()` 到 `orchestrator.Engine` 的工作目录,到 `gatherSignals` 读 ROADMAP.md,到 `memory` 写 `.forge/memory.jsonl`,到 `checkpoint` 存 `.forge/checkpoint.json`——**全部假定一个仓库就是一个宇宙**。没有命名空间、没有仓库间依赖图、没有跨仓库任务编排。

```go
// forge-core/cmd/forge/main.go
o.root = gate.RepoRoot(o.root)
wf, err := loadWorkflow(o.root, name)

// forge-core/internal/gate/gate.go
func RepoRoot(root string) string {
    if root != "" { return root }
    if env := os.Getenv("FORGE_REPO_ROOT"); env != "" { return env }
    return "."
}
```

`FORGE_REPO_ROOT` 环境变量是单值。没有 `FORGE_REPO_GRAPH`、没有 `dependencies.yaml`、没有 "这个 service-A 的变更需要同步推进 service-B" 的概念。

### 为什么这被忽略

ForgeOS 的 dogfood 是自己 + `examples/url-shortener`（单体应用）。所有架构文档和 ROADMAP 讨论的 "演化" 都是单仓库内的代码演化。真实的微服务组织（10-100+ 仓库、共享 proto、语义版本依赖）从未出现在任何用例中。114 篇扩展分析中有几篇提到了 "polyrepo" 关键词,但都是笼统提及,没有深入到代码层的实现路径。

### 为什么它现在值得做

1. **企业适配的硬门槛**:任何有 5+ 微服务的组织都不会在一个仓库里管理所有代码。ForgeOS 如果要走出"个人项目编排器"定位,跨仓库编排是必要非充分条件。

2. **架构级缺口**:当前 `ROADMAP.md` 上的 checklist 项只在一个仓库内被勾选。两个仓库间有相互依赖的变更时,没有任何机制 detect 这个依赖,更不用说按正确顺序提交。

3. **现有基础设施可复用**:`forge-core/internal/asset` 已经定义 `Workflow` 和 `Phase`——跨仓库场景只需要一个"超级 Workflow"概念,其 phase 可以指向不同仓库,每个仓库有自己独立的 `harness` 闸门结果。`forge-core/internal/converge` 的收敛判断可以扩展到多仓库全绿才算收敛。

### 建议扩展骨架

- **仓库依赖声明**:在 `.agent/` 层引入 `dependencies.yml`,声明本仓库依赖的其他 ForgeOS 仓库（URL + 版本/分支）。
- **超级 Workflow 执行**:`forge run` 或 `forge evolve` 接受一个跨仓库工作流定义,按依赖拓扑顺序依次在每个仓库中执行对应的 workflow 并等待其收敛,失败时自动回滚已完成仓库的变更。
- **原子观察点**:`gatherSignals` 扩展为在超级 Workflow 下聚合所有参与仓库的 `RoadmapCompletion` + `GatesGreen`,单个仓库 FAIL 导致全局收敛不通过。

### 不受影响

单仓库运行的 byte-for-byte 行为不变（`--root` 默认值仍为 `.`,超级 Workflow 是显式 opt-in）;现有的 checkpoint/memory/trace 路径完全保留。

---

## 方向二 · 确定性 Trace Replay 引擎 —— 没有"复盘"能力的自治系统是盲飞的

### 问题

`forge-core/internal/trace` 包以 JSONL 格式记录了丰富的运行时事件（迭代边界、agent 相位、gate 裁决、收敛检查、成本、延迟）,`persist` 包保存了 checkpoints,`memory` 包积累了跨会话知识。但**没有一条代码路径能从这些记录中重建一次运行**——你不能拿一个 `.forge/trace.jsonl` 文件 replay 一遍,验证如果改个参数（比如把 `MaxLoopBack` 从 3 改成 5）收敛会更快还是更慢,或者如果改了 prompt 格式 agent 决策会如何变化。

```go
// forge-core/internal/trace/trace.go
type Event struct {
    Seq        int    `json:"seq"`
    Kind       string `json:"kind"`
    Name       string `json:"name"`
    Status     string `json:"status"`
    DurationMs int64  `json:"duration_ms"`
    // ...
}
```

trace 的设计目标是 **auditability**（可审计性）——事后能确认"发生了什么"。但**没有 replay**——不能在本地模拟重跑一次,更不用说 parameterized replay（改参数重跑看效果）。

当前的 `DryRunExecutor` 可以 narrate routing 决策,但它不从 trace 读取输入——它只是一个新的 dry-run,不是一次 replay。

```go
// forge-core/internal/orchestrator/executor.go
type DryRunExecutor struct {
    Log func(string)
}
func (d DryRunExecutor) Execute(ctx context.Context, p asset.Phase, mode string) error {
    tier := PhaseTier(p, mode)
    d.logf("phase %s -> agent %s (tier %s)", p.Name, p.Agent, tier)
    return nil
}
```

### 为什么这被忽略

Trace 是 Sprint 5（扩展五方向）落地后逐步完善的,trace 数据的分析能力（scorecard、telemetry）是 Sprint 19 才完成的。团队的焦点一直在 "产生数据" 而不是 "回放数据"。而且"replay"听起来像是一个调试工具,不是核心功能——但对于 AI 编排系统,它的价值远超调试。

### 为什么它现在值得做

1. **无预算调试**:每次 `forge run --executor=command` 都要花真实 LLM 预算。你想测试修改后的收敛逻辑能不能更快达到 MET?现在唯一方式是再花一轮真钱跑一遍。Replay 引擎让你在过去的 trace 数据上零成本模拟。

2. **收敛逻辑的回归测试**:`forge-core/internal/converge` 包有单元测试,但覆盖的是纯函数（`evalOne` 等）。真正的收敛行为取决于 agent 输出（置信度/裁决/ROADMAP 勾选）与闸门结果的交互。一个 trace 文件就是一条端到端测试场景——确认 v2.6 的收敛逻辑在 v2.5 跑过的场景上同样能正确判 MET/NOT MET。

3. **Parameter sweep**:Trace replay 引擎允许参数化——"如果 MaxLoopBack=5 而不是 3,那次特定运行会不会更早收敛?"——而不需要再次调用 LLM。

### 建议扩展骨架

- **ReplayExecutor**:实现 `orchestrator.AgentExecutor` 接口,但 `Execute()` 不从 CLI 调用 LLM,而是从 trace 文件中读取对应 phase 的 recorded 输出（`cost.go` 的 `parseReviewerVerdict` 等解析器可以直接复用）。
- **Trace 索引**:当前 trace 是顺序 JSONL,按 seq 递增。需要一个 `Seq → Event` 索引和按 phase-name 的快速查找,让 ReplayExecutor 在 O(1) 内找到对应 phase 的记录。
- **Convergence 重演**:`gatherSignals`（`gates.go`）当前读的是**实时的** ROADMAP.md/git diff/gate 裁决。在 replay 模式下,改为注入 trace 中记录的当时值,验证收敛判定是否一致。
- **参数变异语法**:`forge replay --trace .forge/trace.jsonl --vary MaxLoopBack:5`——在 replay 时替换一次运行的参数,观察收敛结果的差异。

### 不受影响

Trace 文件的产生路径完全不变;`DryRunExecutor` 保持不变;现有的 scorecard/telemetry 消费 trace 的方式不受影响。

---

## 方向三 · Prompt 治理 —— 最高杠杆的输入,零治理

### 问题

ForgeOS 的整体质量几乎完全由 prompt 决定:`.agent/agents/*.md`（12 张角色卡）、`.ai/prompts/*.md`（9 个评审模板）、以及 `prompt_context.go` 中拼接的 gate 裁决文本和 feed-forward 上下文,共同决定了 agent 的行为质量和一致性。但**这些 prompt 没有版本、没有 diff 追踪、没有回滚机制、没有 A/B 测试框架、没有质量评估资产**。

```go
// forge-core/internal/prompt/prompt.go
func Build(agent, phase, mode, tier, card string, ctx []string) string {
    var b strings.Builder
    fmt.Fprintf(&b,
        "You are the %q agent in ForgeOS (phase=%s, mode=%s, tier=%s)...",
        agent, phase, mode, tier)
    b.WriteString("## Role card\n")
    b.WriteString(card)
    if len(ctx) > 0 {
        b.WriteString("\n\n## Project context\n")
        b.WriteString(strings.Join(ctx, "\n\n"))
    }
    return b.String()
}
```

`card` 参数就是 `.agent/agents/*.md` 的完整内容。一个对这些角色卡的改动——比如改了 `reviewer.md` 的 `VERDICT:` 输出格式——会直接破坏 `parseReviewerVerdict` 的解析,但**没有任何回归测试**能检测到这个:单元测试用的是硬编码字符串 fixture,而不是从实际 agent 卡文件读取。

```go
// forge-core/cmd/forge/cost.go
func parseReviewerVerdict(output string) (string, bool) {
    // 精确匹配末行 "VERDICT: APPROVE" 或 "VERDICT: REQUEST_CHANGES"
    // 如果 reviewer.md 改了格式但这里没更新 → 静默失效
}
```

### 为什么这被忽略

Prompt 一直被当作"文档"而不是"代码"。团队用 git 追踪 prompt 文件,但把它们和代码放在同一个仓库里,没有独立的发布节奏、没有兼容性契约、没有自动化评估。在 114 篇扩展分析中,少数提到了 "prompt versioning" 关键词,但都停留在"应该做"层面,没有深入到 forge-core 中 prompt 注入的每个入口点。

### 为什么它现在值得做

1. **系统最脆弱的面**:一条 prompt 措辞的微小变化就能完全改变 agent 的行为（也是 LLM 应用的常识）。ForgeOS 把全部智能委托给 LLM agent,而 agent 的"智能" 100% 由 prompt 塑形——这意味着 prompt 是系统中最关键的"代码",但受到的治理比 `gate.mjs` 少得多。

2. **prompt & parser 的耦合**:`cost.go` 中的 `parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore` 精确匹配 agent 卡中声明的机读 token。如果两者不同步（人改了 agent 卡但忘了更新 parser,或者反过来）,整个 REVIEW 收敛信号和 loop-back 机制会静默失效。这曾经真实发生过（Sprint 27 的 `review_status` 断信号 bug）,而且没有任何自动化防护。

3. **A/B 测试能力**:在有两个候选 prompt 版本时,目前唯一的方法是手动改文件、跑一次完整 `forge run`、看结果。一个正式的 prompt 实验框架可以自动化:随机选择一半 phase 用 A 版本、一半用 B 版本,比较收敛率、成本、迭代次数。

### 建议扩展骨架

- **Prompt 资产注册表**:`harness/` 中新增 `prompt-audit.mjs`,逐文件校验 prompts 中声明的机读 token（`VERDICT: APPROVE` 等）是否在 `cost.go` 的解析器中有对应的消费者。CI 中作为 load-bearing 检查。
- **Prompt 版本化注入**:`internal/prompt` 扩展为支持 `BuildVersioned(agent, phase, mode, tier, cardVersion string, ctx []string)`——从 `.agent/agents/<agent>/v1/` 或 `v2/` 读取角色卡。不指定版本时读默认（向后兼容）。
- **Experiment schema**:在 `.agent/eval/` 下声明 prompt 实验:对照版本、评估指标（收敛率/成本/迭代数/违规数）、和统计显著性阈值。`forge evolve --experiment <experiment-id>` 时,每个 agent phase 按实验分配策略选择 prompt 版本。
- **回归资产**:每个 `VERDICT:` 机读 token 的变化对应一个端到端测试 fixture——不再是 `cost_test.go` 里的硬编码字符串,而是从 `docs/adr/` 中的示例 agent 输出文件加载。

### 不受影响

`buildPrompt` 和 `Gather` 的核心路径不变,只是注入源增加版本化能力;没有实验声明时行为完全向后兼容;现有的所有 agent 卡和 prompt 文件保持有效。

---

## 方向四 · 韧性对抗测试（Chaos Mode）—— 编排器自己从没被"压力测试"过

### 问题

ForgeOS 对其编排的项目代码施加了严格的闸门（`gate.mjs` 检查文件体积、`arch-check.mjs` 检查架构、`secret-scan.mjs` 检查密钥）,但对**自身的运行时层**没有任何抗压测试。以下场景完全没有覆盖:

- 如果 `.forge/memory.jsonl` 文件中某一行被手动篡改（注入恶意知识）,`memory.Load()` **不会静默跳过**（它正确返回错误）——但这条错误路径在运行时中**从未被测试过**。`Append` 和 `Load` 的单测只覆盖正常路径,没有注入损坏行。
- 如果 `.agent/workflows/build.yml` 被提交了一个循环依赖的 phase 图（phase A dep → B dep → A）,`orchestrator/waves.go` 的 `Waves()` 函数会返回错误——但 `forge run` 的那条路径上,没有任何重试或降级逻辑,只是裸返回错误。
- 如果 agent 卡中的机读 token 被有意改成恶意内容（比如 `VERDICT: APPROVE` 改成 `VERDICT: APPROVE_AND_DELETE_EVERYTHING`）,`parseReviewerVerdict` 因为精确匹配而返回 `ok=false`——收敛逻辑"安全地"退回到不采取行动——但**没有人验证过这个 fallback 在所有调用路径上的行为一致性**。

```go
// forge-core/internal/trace/trace.go
// decode 中损坏行的处理逻辑——有防御,但从未在 chaos mode 下实际验证过
func decode(data []byte) ([]Entry, error) {
    // ...
    if err := json.Unmarshal(raw, &e); err != nil {
        return nil, fmt.Errorf("memory: decode entry on line %d: %w", line, err)
    }
    // ...
}
```

### 为什么这被忽略

传统软件中的混沌工程（Chaos Engineering）已经成熟（Netflix 的 Chaos Monkey 等）,但 AI 编排系统的混沌工程几乎不存在。团队的精力一直在 building（构建能力）而不是 breaking（破坏验证）。114 篇分析中只有一篇提到了 "chaos" 关键词,且停留在概念层面。

### 为什么它现在值得做

1. **AI 编排的特殊脆弱性**:与传统的 CI/CD 系统不同,ForgeOS 依赖 LLM agent 的输出质量,而 LLM 对输入中的异常（prompt 注入、知识污染、上下文中毒）极其敏感。传统的混沌工程（随机杀进程、注入网络延迟）不能满足 AI 编排的需求——这里需要的是**语义层的对抗测试**。

2. **隐式信任链**:系统信任其自身状态文件的完整性（checkpoint、memory、trace）。如果任何一个文件损坏或被人为篡改,系统将基于损坏的状态作出决策,但没有机制能 detect 这种状态损坏——因为信任是隐式的,不是显式验证的。

3. **故障传播的未知性**:一个 phase 输出一个格式错误的 `VERDICT:` 行,在 `parseReviewerVerdict` 中产生 `ok=false`,这个 `false` 向上传播到 `agentOutcome` -> `loop.go` 的收敛检查 -> 最终 `converge.go` 的 `evalReviewStatus`。这条链的每一步都"安全地"退回到不采取行动——但整个行为从未作为一个整体被端到端验证过。如果其中一个链接误将 `false` 当作"已批准"处理,就会产生一个静默的安全漏洞。

### 建议扩展骨架

- **Chaos Mode 注入层**:一个新的 `harness/chaos.mjs` 脚本（或 Go 包 `internal/chaos`）,在 `forge run --chaos` 时被激活。它在关键决策点之前注入可控的异常:
  - `memory.Load()` 返回一个包含随机损坏行的文件
  - `parseReviewerVerdict` 收到一个格式正确但语义错误的 `VERDICT:` 行
  - `gatherSignals` 读到的 ROADMAP.md 包含格式错误的 checklist 项
  - Checkpoint 文件在 `Save` 和 `Load` 之间被人为截断
- **行为契约验证**:在每个"安全降级"路径（如 `parseReviewerVerdict` 返回 `ok=false`）后,验证该路径的调用者是否确实以 fail-open（继续运行）或 fail-closed（中止）的正确方式响应。这个验证本身可以写成可测试的断言,注入到 `orchestrator.Engine` 的行为追踪中。
- **注入后恢复验证**:证明在 chaos 注入被移除后,系统能正确恢复（checkpoint 未损坏、memory 可继续 append、trace 可继续 emit）。
- **语义注入库**:定义一组"AI 编排特异的故障模式"——prompt 注入、知识中毒、裁决伪造、过度自信的置信度声明——并为每个模式提供可复现的测试 fixture。

### 不受影响

无 `--chaos` 标志时,所有路径的 byte-for-byte 行为完全不变;现有的所有单测和集成测试不受影响;生产运行默认不启用 chaos。

---

## 方向五 · 运行时自度量 —— 编排器从不测量自己的健康

### 问题

ForgeOS 对项目代码的质量指标测量得极其精细:行数、函数长度、扇入、测试覆盖率、架构违规数、CVE 数量、代码与测试比率……但对**自己的运行时健康**没有任何持续测量:

| 问题 | 代码中是否有答案 |
|---|---|
| 本周 `forge evolve` 的收敛率是多少?不同 mode 各是多少? | 无——没有日志聚合、没有计数器 |
| 平均每个 agent phase 花多少钱?哪种 agent（reviewer vs implementer）最贵? | 有原始 trace 数据,但无自动聚合、无趋势 |
| loop-back 的频率分布如何?哪种 gate 最常触发 loop-back? | trace 中有 `loop-back` 事件,但无人消费 |
| 哪个 prompt 版本的收敛速度更快? | 无 A/B 框架（方向三的盲区）|
| 系统自身上周比本周是变好了还是变差了? | 无趋势、无基线、无告警 |

trace 系统已经收集了原始数据:

```go
// trace 记录了每件有意义的事:iteration 边界、agent 相位、gate 裁决、成本、延迟
// 但没有任何消费者把这些转译成"系统健康度量"
type Event struct {
    Kind       string `json:"kind"`        // iteration | agent | gate | decision | converge | error
    DurationMs int64  `json:"duration_ms"` // 墙钟延迟——可用于计算 p50/p95
    CostUsdMicros int64 `json:"cost_usd_micros"` // 成本——可用于计算每特征点成本
    // ...
}
```

而 `scorecard` 系统（`internal/routing/scorecard.go`）是设计用来记录**项目**的模型性能评估的——不是记录 ForgeOS 自身运行的健康度量。

```go
// forge-core/internal/routing/scorecard.go
// ScorecardEntry 是关于"某个模型在某项任务上的历史表现"
// 不是关于"编排器自身的运行状况"
type ScorecardEntry struct {
    TaskType   string  `json:"task_type"`
    Agent      string  `json:"agent"`
    Model      string  `json:"model"`
    Tier       string  `json:"tier"`
    Quality    float64 `json:"quality"`
    // ...
}
```

### 为什么这被忽略

团队的思维模型是"ForgeOS 是一个工具,不是被管理的系统"。所有 telemetry 基础设施（`trace`、`scorecard`、`telemetry`）都被设计为面向**被编排的项目**的——trace 记录项目构建过程,scorecard 评估项目中的模型表现。没有人问过"谁在评估评估者自己?"。

114 篇分析中,"self.telemetry" 和 "operational.analytics" 返回零结果——这是完全未被触及的空地。

### 为什么它现在值得做

1. **运营闭环的缺失环节**:ForgeOS 的核心命题是"AI 自治软件工厂"。但一个工厂如果没有对自己产出质量、效率、成本的持续测量,就没有改进的基础。你不能管理你不测量的东西。

2. **成本可见性**:`--run-budget-usd` 和 `--agent-max-budget-usd` 提供了**事前预算控制**,但缺少**事后成本分析**。在一个 24h 无人值守的 evolve 循环后,没有人知道钱花在了哪里、哪个 phase 最烧钱。这直接影响到预算治理策略的制定。

3. **退化检测**:如果一次 prompt 改动意外降低了收敛率,或者一次 routing 策略变化增加了 loop-back 频率,当前的系统无法 detect 这种退化——因为没有基线,没有趋势对比。一个自测量仪表盘可以让这种退化在第一个 evolve 循环后就显现,而不是等用户发现"几次迭代后什么都没完成"。

4. **ROI 计算的基础**:当你需要向组织 justify ForgeOS 的价值时,没有"before/after"的运营数据（收敛速度、成本/功能点、闸门拦截率）就做不出有说服力的 ROI 分析。

### 建议扩展骨架

- **`forge telemetry` 子命令**:一个新的 `forge telemetry` 命令,读取 `.forge/` 目录下的 trace 存档,聚合出运营度量:
  - 收敛率（convergence rate by mode/lifecycle）
  - 相位时长分布（phase duration p50/p95 by agent type）
  - 成本分布（cost by agent/gate/iteration）
  - loop-back 频率与原因（loop-back rate by trigger type）
  - 闸门拦截率（gate fail rate by gate name）
- **趋势追踪**:`telemetry` 输出可以保存为一个 JSON 文件（如 `.forge/metrics/convergence-rate.jsonl`）,在每次 `forge run`/`forge evolve` 后 append 一行最新度量。这样就有了"上一周收敛率曲线"。
- **运营基线告警**:在 `.agent/policies/modes.yml` 或一个新文件 `observability.yml` 中声明运营基线:"convergence_rate < 0.6 时告警"、"avg_cost_per_phase > $2 时告警"。`forge run` 结束后检查基线,越线则输出 WARNING（或由 CI 拦截）。
- **趋势 Webhook**:当运营度量持续恶化（如连续 3 次运行收敛率下降）,触发 webhook 通知（比如 posting 到 Slack/Teams）,赋予系统"自救能力"的雏形。
- **资产复用**:`forge validate` 和 `forge doctor` 的架构可作为 telemetry 的 CLI 面;`hard link` 到 `.forge/trace.jsonl` 避免数据复制。

### 不受影响

现有的 trace 格式、scorecard 格式、checkpoint 格式完全不变;telemetry 只读不写 trace 文件;不需要修改任何现有 command 的 flag 或行为。

---

## 汇总

| # | 方向 | 类型 | 创新程度 | 依赖 | 优先级估计 |
|---|---|---|---|---|---|
| 1 | 多仓库编排面 | 功能扩展 | 高(零现有支持) | 新增 `dependency` 声明 + 超级 Workflow 执行器 | P2（企业场景启用）|
| 2 | 确定性 Trace Replay | 功能扩展 | 高(零现有支持) | 新增 ReplayExecutor + trace 索引 | P1（降低成本）|
| 3 | Prompt 治理 | 治理/合规 | 中(有概念提及无实现) | 注册表 + 版本化注入 + 实验框架 | P0（系统安全）|
| 4 | 韧性对抗测试 | 质量/安全 | 极高(概念新颖) | chaos 注入层 + 行为契约验证 | P2（长期韧性）|
| 5 | 运行时自度量 | 可观测性 | 极高(完全空白) | telemetry 命令 + 聚合 + 基线告警 | P1（运营必需）|

> **P0**=立即（存在静默失效风险） · **P1**=短期（可量化 ROI） · **P2**=中期（战略价值,但需要前提条件）。

---

*本文档通过逐包阅读 forge-core 全部 18 个 Go 包的每个导出函数、harness 全部 Node/Python 脚本的每个 probe 路径、以及 .agent 全部声明文件来保证不重复现有分析。114 篇已有 docs/requirements 的全文搜索确认这 5 个方向的核心论点未被覆盖。*
