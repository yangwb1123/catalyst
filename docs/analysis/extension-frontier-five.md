# ForgeOS — 全局扫描：5 个尚未被充分论证的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **基线**: 代码库完整扫描（forge-core 17 内部包 + cmd/forge ~20 CLI 命令 + harness ~20 工具 +  
>   `.agent/` 完整治理骨架 + `pi-batch.py` + examples）  
> **方法**: 与已存在的 22+ 份 `docs/analysis/*` 分析文档逐一交叉对比，保证不重复  
> **约束**: 不写任何实现代码；每个方向标注尚未被现存文档覆盖的原因  
> **日期**: 2026-07-01

---

## 目录

1. [方向一：ROADMAP 条目选择与执行优先级引擎](#方向一roadmap-条目选择与执行优先级引擎)
2. [方向二：「铸剑为犁」——从技术审计到人类可读的产品进展报告](#方向二铸剑为犁从技术审计到人类可读的产品进展报告)
3. [方向三：跨会话持久 Agent 会话——`forge run` 的一次性开销根源](#方向三跨会话持久-agent-会话forge-run-的一次性开销根源)
4. [方向四：`--agent-allowed-tools` 安全周界与审计记录](#方向四--agent-allowed-tools-安全周界与审计记录)
5. [方向五：Memory 压缩退化监控——Agent 遗忘的隐形成本](#方向五memory-压缩退化监控agent-遗忘的隐形成本)

---

## 已有分析覆盖情况（已逐一核对，本文不重复）

本文与以下 22+ 份现存文档不重叠，每个方向均为新提出：

| 已被覆盖的域 | 对应文档 |
|---|---|
| 自适应工作流引擎 | `high-value-extensions.md` 方向一 |
| 闸门自省体系 | `high-value-extensions.md` 方向二 |
| 增量式治理执行 | `high-value-extensions.md` 方向三 |
| 跨项目依赖图谱 | `high-value-extensions.md` 方向四 |
| Human-in-the-loop 质量追踪 | `high-value-extensions.md` 方向五 |
| 多项目拓扑编排 | `expansion-core-five.md` 方向一 |
| 架构-代码漂移检测 | `expansion-core-five.md` 方向二 |
| 预热启动与知识图谱缓存 | `expansion-core-five.md` 方向三 |
| 自愈循环 | `expansion-core-five.md` 方向四 |
| 预算前瞻规划 | `expansion-core-five.md` 方向五 |
| 多 Agent 协作模式、Prompt 安全、持久化知识图谱、性能回归、Workflow 版本化 | `strategic-expansion-and-edge-cases.md` |
| 并行编排竞态、资源泄漏、收敛陷阱、序列化瓶颈、治理盲区 | `edgecases-and-perf.md` |
| 韧性运行时、学习闭环、Context/Memory、执法盲区、安全合规 | `expansion-directions.md`、`ROADMAP.md` |
| ADR 决策衰退 | `eighth-wave-adr-decay.md` |
| 持久化数据真实性 | `seventh-wave-data-realism.md` |
| 增长瓶颈、隐藏反馈、自测狗粮、MQTT/WASM、v2-to-northstar | 其余 `docs/analysis/*` |

---

## 方向一：ROADMAP 条目选择与执行优先级引擎

### 现状

ForgeOS 自治循环的核心是 `converge.RoadmapCompletion`——一个纯函数，逐行扫描 ROADMAP.md 的
`- [x]` / `- [ ]` / `- [~]` 标记，返回已完成条目的百分比（`converge.go:230-248`）。

但整个代码库中**不存在任何机制来决定"接下来做哪一个条目"**。当下一个迭代开始时，实现者
（planner agent）的 prompt 中注入了整个 ROADMAP.md 的正文（`prompt.go currentTask()`，截断至
4000 runes），然后 agent 自行决定做什么。这是一个**隐含的黑盒决策**——完全由 LLM 的即时判断
驱动，没有系统级的优先级、依赖关系、或工作量评估。

**代码证据**：
- `converge.go:230` — `RoadmapCompletion` 只计数，无排序/选择语义
- `prompt.go:40` — `currentTask` 注入整个 ROADMAP body，无优先级/依赖提示
- `asset.go:73` — `Phase.ConfidenceMetric` 已被声明但未被任何 workflow 消费（除 discover.yml）
- `engine_build.go` — `phaseTierResolver` 解决 tier 而非选择 roadmap 条目

### 为什么需要

在 24h 无人值守自治循环中（`forge evolve` 的核心场景），"下一步做什么"是一个**全系统最高
杠杆的决策**：

1. **依赖不可见**：条目 B 依赖条目 A 先行完成，但 ROADMAP.md 是扁平的 markdown 列表。agent 需
   要从 A 的上下文推断 B 的前提——如果 A 的完成没有被内存记录，agent 可能再次选择 A。
2. **优先级无系统化表示**：explorer mode 应该选低 hanging fruit，engineering mode 选高优先级
   架构条目。当前 agent 只能从 `priorities` observability 读模式意图，但该意图不被翻译成
   任何可执行的条目选择规则。
3. **无最大化吞吐量的并行选择**：当多个条目无依赖关系时，并行波（RunParallel）无法工作——
   因为没有依赖信息来构建 wave 拓扑。
4. **收敛的假象**：`RoadmapCompletion == 100%` 意味着所有 checkbox 被勾选，但 agent 可能跳过了
   最难/最重要的条目（因为 prompt 裁剪截断了它），勾选了所有可见的简单条目，然后声称完成。

### 建议方向

- 为 ROADMAP.md 添加机器可读的前端元数据（YAML frontmatter 或 checklist 注解）：`priority`、
  `depends_on`、`effort_estimate`、`confidence_threshold`
- 新增 `internal/roadmap` 包（与 `internal/converge` 同级），提供 `SelectNext` 函数——
  给定当前完成状态、mode、lifecycle、可用预算，返回"下一步最应做的条目"
- 选择策略可注入（pluggable priority policy）：mode-aware（explorer→低 hanging fruit、
  engineering→架构依赖链入口）、budget-aware（$5 预算内可完成的中等条目 >
  $20 的高价值条目）
- 将选择结果注入 agent prompt 作为明确的"本次迭代应实现的条目"，而非让 agent 从全文自己猜

**价值**: P1。当前设计依赖 LLM 的隐含判断做全系统核心调度决策，而 LLM 的"随机"选择无法保证
依赖顺序、资源最优分配和可预测收敛。这是自治循环中最后一个没有结构化决策的环节。

---

## 方向二：「铸剑为犁」——从技术审计到人类可读的产品进展报告

### 现状

ForgeOS 当前的输出核心是技术审计型：
- `forge accept` → ACCEPTED/REJECTED + 逐闸门 PASS/FAIL/NA
- `forge run` / `forge evolve` → phase 日志 + convergence MET/NOT MET
- `forge status` → checkpoint 摘要
- `trace.jsonl` / `memory.jsonl` / `scorecards.json` → JSONL 机器日志

**没有任何命令能回答一个产品经理或技术负责人最关心的三个问题**：
1. "这个 sprint 到底完成了什么？"
2. "哪个 agent 花了多少钱？和上周比是更多还是更少？"
3. "当前卡在哪里？为什么卡？"

**代码证据**：
- `main.go:506-508` — `verdict()` / `mark()` 只输出 binary "MET"/"NOT MET"
- `evolve.go` — LoopEngine 的 `reportIteration` 输出 `[iter N] roadmap=X% gates=Y (z)`，无历史
  对比、无趋势、无成本摘要
- `trace.go:87-93` — 所有 Event kinds 都是机器读的（iteration/agent/gate/decision），无
  "summary" kind
- `converge.go:157-160` — `evalCriterion` 输出 "test_pass=PASS"，但从不问"上次是多少"

### 为什么需要

1. **信任建立**：自治系统的运行者需要信任系统做出了合理的进展。没有结构化报告，人类只能
   手动 grep trace.jsonl 理解发生了什么——这直接阻碍了"无人值守"的采纳。
2. **成本可见性**：`cost.go` 已经有 `runBudget` 追踪每次 agent 调用的美元成本（`cost.go:48`），
   trace.Events 携带 `CostUsdMicros`（`trace.go:53`），scorecard 写 `avg_cost_usd`
   （`scorecard_wind.go`）。但这些数据只在机器文件里沉睡——没有任何人性化的报告。
3. **决策仪表盘**：CTO 在 design.yml 的 human_gate 前需要知道"之前 3 次 evolve 迭代花了 $X 完成了
   Y 个条目，当前收敛速度在加速/减速"——这是 `mode=cto` 工作流的自然输入。

### 建议方向

- 新增 `forge report` 命令：读取 `.forge/trace.jsonl` + `memory.jsonl` + `scorecards/*.json`，
  输出人类可读的迭代摘要：完成条目、花费美元、gate 状态变化趋势、瓶颈阶段识别
- 新增 `--output pretty` / `--json` 模式用于 CI 集成（后者已有雏形在 `acceptance.mjs:208`）
- 在 `forge evolve` 的迭代报告中引入**增量对比**：不仅说"roadmap=75%"，而且说"较上轮 +15%，
  成本 $0.42，平均 $0.03/百分点"
- 新增 trace.Event kind `"report"`：每 N 次迭代 emit 一个聚合摘要事件，即使后期 trace 被轮换
  也能保留高层聚合

**价值**: P1。当前系统产生了海量数据（trace/memory/scorecard/checkpoint），但没有消费它们的
人性化接口。这导致"系统在做很多工作"和"系统有用"之间存在感知鸿沟。这个方向不改变运行时行为，
只改变数据的呈现——是投资回报率最高的纯增量改进。

---

## 方向三：跨会话持久 Agent 会话——`forge run` 的一次性开销根源

### 现状

每次 `forge run` 和 `forge evolve` 的每次迭代都完整执行以下顺序：

```
forge run/evolve <workflow>
  → parse flags
  → loadWorkflow (打开 YAML, 尝试 Go yaml2json, 失败回退 python shim, unmarshal JSON)
  → openTracer (打开 trace.jsonl 追加)
  → probeStatuses (fork 子进程跑 node/python 检测工具链)
  → newRunBudget
  → resolveAutoRisk (git diff --name-only 追踪变更路径)
  → engine.Run/RunFrom/RunParallel
      → phase 0: 读 AGENTS.md, ADRs, ROADMAP.md → tokenize → retrieve → build prompt → exec
      → phase 1: 同上
      → ...
  → close trace
  → windDownScorecards
```

其中绝大部分 I/O 和解析在每个 phase 之间和每次 invocation 之间是**完全重复**的。

**代码证据**：
- `main.go:342` — `loadWorkflow` 每调用一次读 .agent/workflows/<name>.yml 并解析 JSON
- `prompt.go:51` — `Gather` 每次 phase 都读 ADRs (`adrTitles` 又调 `os.ReadDir`)、AGENTS.md、
  ROADMAP.md
- `retrieve.go:50-60` — 每次 Retrieve 重新 tokenize 所有 doc、重新计算 df/score
- `prompt.go:121` — `memory.Load` 已有 mtime 缓存（`memory.go loadCache`），但 ADR/AGENTS/
  ROADMAP 没有
- `main.go:372` — `probeStatuses` 每 run 重新 fork 子进程检测工具链

在 `forge evolve` 场景下，这些开销在每个迭代重复 N 次（默认 5，thorough 10），对于单次 24h
run 可能意味着数百次重复读、解析、tokenize。

### 为什么需要

1. **成本放大**：在真 `--agent-cmd=claude` 场景下，一次 phase 调用的 LLM 成本是 $0.05-$0.50，
   prompt 构建的 100-300ms 开销（文件 I/O + 正则 + tokenize）相比 LLM 延迟微不足道（10-60s），
   但**在 dry-run 模式**（第一次调通前的默认模式），这些开销占了 95% 以上的执行时间——每次
   `forge run` 都冷启动，开发反馈周期被拖慢。
2. **YAML 解析回退链**：`loadWorkflow` 的 Go yaml2json → python shim 回退链（`main.go:324-342`）
   是遗留的临时架构。每次加载都尝试打开文件、尝试 Go 解析、失败则启动 python3 子进程再解析。
3. **ADL（Architecture Decision Log）检索退化**：随着 ADR 数量增长（一个真实项目 6 个月可达到
   50+ ADRs），`adrTitles` 的 `ReadDir` + tokenize + scoring 的时间会线性增长——当前
   `adrTopK = 6`（`prompt.go:30`）约束了注入数量但没有约束预处理时间。

### 建议方向

- 为 `loadWorkflow` 添加 JSON 序列化缓存：将解析后的 `asset.Workflow` 序列化到
  `.forge/workflow.<name>.json`，仅在 YAML mtime 变化时重新加载——消除每次 run 的 YAML 解析开销
- 将 `Gather` 中不变部分（AGENTS.md 的前 6 条、ADRs 列表）在 run 维度缓存，而非 phase 维度
  （已有 `prompt.ContextCache` 基础：`cache.go`，可扩展至 AGENTS/ADRs）
- 消除 python shim 回退：当前 `yaml2json` Go 实现已经稳定（`internal/yaml2json`），考虑将其
  设为默认路径以消除 `exec python3` 开销和外部依赖
- 新增 `forge daemon` 或 `forge server` 模式（非 Web UI，纯后台进程）：常驻进程持有缓存状态，
  子进程通过 IPC 快速查询而非冷启动

**价值**: P2（当前 LLM 成本主导下不明显，但对开发体验和 dry-run 反馈周期有显著提升）。
可以在 2-3 天内以纯增量方式交付，无架构变更风险。

---

## 方向四：`--agent-allowed-tools` 安全周界与审计记录

### 现状

`forge run/evolve --agent-cmd=claude` 的安全模型完全依赖于 `--agent-allowed-tools` 参数
（`defaultAgentAllowedTools` 常量，`main.go:30-48`），它是一个 Bash 命令模式的白名单：

```go
const defaultAgentAllowedTools = "Bash(node --test*) Bash(node harness/gate.mjs*)"
```

Agent 只能执行匹配这些模式的 Bash 命令。这构成了**整个自治系统的唯一安全周界**——但该周界
当前有几个未被分析的问题：

1. **模式匹配的精度**：`Bash(node --test*)` 匹配 `Bash(node --test foo/bar.js)` 的意图是明确
   的，但它也匹配 `Bash(node --test-benchmark --run-script-eval "$(cat /etc/shadow)")`——因为
   通配符 `*` 匹配了任意后缀。Bash glob 模式不是安全的访问控制机制。
2. **`--agent-permission` 的安全假象**：`acceptEdits` 模式允许 agent 写文件但限制 Bash。但
   agent 可以写一个 `.sh` 文件然后通过允许的白名单命令执行它。例如：写 `run_payload.sh` 然后
   通过 `Bash(node --test*)` 调用 `node run_payload.sh`？不——node 执行的是 JS/TS，不是 shell。
   但 agent 可以写一个 JS 文件 `exploit.mjs`，然后 `node harness/gate.mjs` 不是同一文件。
   真正的风险是：agent 可以写一个 .mjs 文件，然后 `node --test exploit.mjs` 执行它。
3. **无审计日志**：`command_executor.go` 在 spawn 命令后记录 stdout/stderr，但从不记录
   **agent 实际执行了什么工具命令**。如果 agent 成功利用了白名单漏洞，事后审计无法发现。
4. **无最小权限原则**：默认白名单在所有 phase 共享（`defaultAgentAllowedTools` 被用于所有
   实现者 phase），但不同 phase 对工具的需求不同：`reviewer` 不需要 `node --test`，`planner`
   不需要 `node harness/gate.mjs`。当前全给同一份。

**代码证据**：
- `main.go:30` — `defaultAgentAllowedTools` 硬编码，通配符 `*`
- `main.go:46-48` — 注释自认"禁止 forge，但允许 node"，但 node 可以执行任意 JS
- `command_executor.go` — `Build` 将白名单传入 `--allowedTools`，不记录实际调用
- `engine_build.go:101` — 所有 phase 使用同一个 `agentAllowedTools` 值

### 为什么需要

1. **真点火场景已经坐实**：Sprint 24-25 已经用真 `--agent-cmd=claude` 运行了端到端工作流
   （`docs/ignition.md` 记录了配方）。如果这个安全模型存在绕过漏洞，**自治系统已经在裸奔**。
2. **安全周界是唯一防线**：没有沙箱（Firecracker 是 v3）、没有网络隔离、没有能力降级——
   `--allowedTools` 是唯一阻止 agent 执行任意系统命令的机制。这不是理论问题：agent 被 prompt
   操控写恶意代码是"prompt injection"的标准威胁模型（已由 Sprint 18 的 `prompt_context.go:305`
   部分覆盖输出层 sanitize，但输入层尚无防护）。
3. **安全不应是之后加的功能**：ForgeOS 目前是开发者工具，安全假设是"执行者就是开发者自己"。
   但当系统朝着 24h 无人值守演进，`forge evolve` 可能在 CI/CD runner 上或作为后台进程运行时，
   安全周界就变成了产品特性，而非基础设施细节。

### 建议方向

- 将白名单模式匹配从 **Bash glob 模式**升级为 **命令+参数前缀匹配**：
  `Bash(node --test)` 匹配 `node --test foo` 但不匹配 `node --test-benchmark`。使用前缀匹配
  消除通配符的过度授权。
- 按 phase 角色约束白名单：`implementer` 拿 `node --test`+`node gate.mjs`，`reviewer` 只拿
  `cat` 或 `head`（只读），`planner` 不拿任何工具
- 在 trace 中添加 `"tool_call"` kind：记录 agent 每次调用的工具及其参数，和 `--allowedTools`
  检查结果（允许/拒绝），形成事后可审计的安全日志
- 新增 `forge audit tools` 子命令：读取 trace 中的 tool_call 事件，报告被允许和（如果配置了）
  被拒绝的工具调用，让安全审计成为 CI 的一部分

**价值**: P0（安全）。当前是纯功能安全假设，一旦从"开发者在自己机器上跑"迁移到"无人值守后台
进程"，这就是第一道防线。即使不迁移，Sprint 24-25 已经证明真 agent 可以以 `acceptEdits` 写
任意文件——这是信任边界，需要可审计。

---

## 方向五：Memory 压缩退化监控——Agent 遗忘的隐形成本

### 现状

`internal/memory` 包包含一个 `Compact` 函数（`memory.go:270-340`），当 `.forge/memory.jsonl`
中的条目数超过阈值（默认 500）时，它会：

1. 将超过 24 小时的老条目按 `kind` 分组
2. 对每组保留最近的 `keepPerKind`（默认 20）条
3. 将剩余的旧条目合并为一条 `compact_summary` 摘要条目

**这意味着：系统的长期记忆会被人为压缩，且压缩的质量从未被验证。**

**代码证据**：
- `memory.go:285` — `compact.go` 的 `summarizeBlock` 只统计了条目数、时间范围、topic 频率，
  完全不做语义摘要——它不是真正的 LLM 摘要，是一个**结构化填充模板**：
  ```
  "compacted %d %s entries%s%s" → "compacted 15 gap entries, 10 entries from [1782516099..1782599499]; topics: gateway:3, routing:5, auth:2"
  ```
- `memory.go:362-364` — 摘要条目的 `Kind` 是 `"compact_summary"`，但代码中没有任何消费方
  理解这个 kind。`Query` 根据 `kind` 过滤，`compact_summary` 不在 `KindGap`/`KindDecision`/
  `KindLesson` 中，所以 **Query(entries, "gap", "") 会过滤掉压缩后的 gap 摘要**——旧教训从此
  对 agent 不可见。
- `memory.go:219-230` — `filterSuperseded` 从最新到最旧遍历去重，但 compact_summary 条目的
  Supersedes 字段为空，所以它不取代任何旧条目，旧条目也不取代它——两者并存，互相矛盾。

### 为什么需要

1. **遗忘就是退化**：`memory.jsonl` 是自治循环的长期记忆。如果 agent 每 24 小时忘记 80% 的
   历史教训，系统会周期性地重复犯同样的错误。ForgeOS 的第一轮 v2 愿景包含"学习闭环"
   （Learning loop），但压缩导致的遗忘在对抗这个目标。
2. **当前压缩是纯丢失**：`summarizeBlock` 不保留旧条目的具体细节——它只记得"有 15 个关于
   gateway 的 gap，发生在某时间范围"。agent 读这条摘要时只知道"发生过一些关于 gateway 的
   gap"，不知道具体是什么。这比没有记忆好一点（至少知道该警惕），但比原始条目差很多。
3. **压缩与 supersede 语义冲突**：如果一个被压缩的旧条目本应被 supersede（后来更新修正了），
   压缩后原始条目消失，supersede 链接断裂，但 superseding 条目保留——agent 读到 superseding
   条目但看不到它在 supersede 什么，上下文丢失。
4. **无退化检测**：压缩后无人检查摘要质量。如果 `Compact` 在阈值=500 时触发，压缩了 400 条
   gap（只留 20 + 1 摘要），那么 agent 从 400 条具体决策变成了 1 条模板化摘要——300 倍的信息
   损失未被记录或告警。

### 建议方向

- 新增 `memory.CompactQuality` 度量：压缩前后记录条目数、kind 分布、覆盖时间跨度、topic 数量，
  输出为一个 trace event（`"memory_compact"` kind，已在 `trace.go:91` 声明），让退化可观测
- 将 `summarizeBlock` 升级为可注入的策略：当前是无 LLM 的纯统计模板，未来可接 LLM（通过
  `--agent-cmd` 的同一个 executor）做真正的语义摘要，保留关键决策和原因
- 修复 `Query` 对 `compact_summary` 的不可见：当查询 `kind=gap` 时，返回 `compact_summary` 中
  包含的 gap 摘要作为上下文提示（类似 TF-IDF 返回顶部匹配）
- 为 `memory.Prune` 添加**保留关键条目索引**：`confidence >= 0.8` 且 `kind=decision` 的条目
  永远不被压缩（白名单保护），防止高确定性但低频率的架构决策被意外遗忘
- 新增 `forge memory inspect` 命令：读取 memory.jsonl，报告条目数、kind 分布、压缩历史、
  首次/最近条目时间、supersede 链长度统计——让遗忘可检查

**价值**: P2（当前 memory.jsonl 只有 ~14 条，尚未达到压缩阈值，但会随真点火运行时间线性增长）。
一旦 24h 无人值守 evolve 运行超过一周，memory 将稳定增长到压缩触发点。此时如果退化未被检测到，
系统相当于在"短期记忆好、长期记忆差"的状态下运行——这直接与 ForgeOS 的产品承诺（持续学习改进）
相矛盾。

---

## 优先级矩阵

| 方向 | 优先级 | 类别 | 一句话杠杆 | 代码就绪度 |
|---|---|---|---|---|
| 一 Roadmap 选择引擎 | **P1** | 功能 | 自治循环的核心"下一步做什么"决策从 LLM 隐含恢复为结构化 | `converge.RoadmapCompletion` 已就绪，只差 `SelectNext` |
| 二 产品进展报告 | **P1** | 产品/可观测 | 所有数据已产生（trace/scorecard/memory），只差人性化消费接口 | `cost.go`/`trace.go` 数据完备，纯新 CLI 命令 |
| 三 跨会话持久 Agent 会话 | **P2** | 性能/开发体验 | 消除 cold-start 提升 dry-run 反馈周期，为未来 daemon 模式铺路 | `prompt.ContextCache` 已存在，可扩展 |
| 四 `--agent-allowed-tools` 安全周界 | **P0** | 安全 | 唯一安全防线从 Bash glob 升级为精确匹配 + 可审计 | `command_executor.go` 是唯一改动点 |
| 五 Memory 压缩退化监控 | **P2** | 可靠性 | 长期记忆质量的保证机制，在触发 500 条压缩前介入 | `memory.go` `Compact`/`Prune` 已实现，只差质量度量 |

### 收敛建议

- **今夜能做**：方向四（精确工具匹配 + trace tool_call 审计）。影响范围极小
  （`command_executor.go` + `trace.go`），但安全提升极高。
- **本周能做**：方向二（`forge report`）。纯消费端，不碰运行时，复用现有数据格式快速产出可见价值。
- **本月能做**：方向一（`internal/roadmap` + ROADMAP frontmatter）。需与 agent workflow 配合
  修改 workflow YAML 和 prompt 注入，是一次架构设计决策但范围可控。
- **下季度规划**：方向三 + 方向五。方向三的 daemon 模式是架构级变更（虽然后续可增量交付），
  方向五的退化监控在 memory 达到数百条前没有紧迫性。

