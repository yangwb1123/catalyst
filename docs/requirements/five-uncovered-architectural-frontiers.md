# Five Uncovered Architectural Frontiers — ForgeOS 下一轮高价值扩展方向

> **状态**：基于 2026-07-10 代码库全局扫描的独立分析。
> **方法**：阅读 BOOTSTRAP / `.agent/` 全部文档 / `forge-core` 全部 18 个 Go 包 / `harness` 全部工具 / 现存 27 份需求分析文档，找出在已有分析中**未被充分覆盖**的高价值扩展方向。
> **排除**：已被现存架构文档覆盖的内容（满分卡回灌、执法器盲区、真点火韧性、learning loop、parallel 引擎、checkpoint phase 粒度、ReviewDepth 旋钮、SCA 框架、`readonly` 强制等——均视为已交付或已在路线图中）。

---

## 方向一 · Run Identity & Global State Isolation（运行标识与全局状态隔离）

**当前现状**：`forgeDir()` 返回硬编码 `<root>/.forge/`，所有运行时状态（checkpoint / trace / memory / approval markers）共享同一个全局目录。无 run ID、无会话边界、无并发隔离。

**具体代码证据**：
- `main.go:450-454` — `forgeDir()` 是简单的 `filepath.Join(root, ".forge")`，零命名空间
- `evolve.go:118` — checkpoint 路径硬编码为 `<root>/.forge/checkpoint.json`，单文件覆盖写入
- `evolve.go:469-475` — trace 以 `O_APPEND` 追加到 `<root>/.forge/trace.jsonl`，无 fork/version 概念
- `memory.go` — `loadCaches` 用 `sync.Map` 按 path 缓存，`invalidateLoadCache()` 全局删除——一个 evolve 循环的 Append 会清除另一个循环的缓存
- `loop.go:85-86` — LoopEngine 的 Parallel 模式只在单 run 内部做 wave 级并发，不涉及 run 级别的并行
- `memory.go:796` 的 `rewriteStore` — 两个进程同时调用 Prune 会相互覆写

**为什么需要**：
1. **并发 run 的数据损坏**：两个 `forge evolve` 同时运行（不同 terminal、不同 feature branch），都将 checkpoint 写到 `checkpoint.json`。`Save` 用原子 rename，但 A 写完后 B 立即覆盖，A 的进度永久丢失。trace 是 `O_APPEND` 写入，两股事件流交错在同一文件里——无法将事件解码回各自 run。
2. **分支盲区**：当前系统完全不知道 git branch。用户切分支后再跑 `forge run`，`.forge/` 中的 memory/checkpoint/trace 全部来自另一条分支，导致 context 污染（memory 注入上一条分支的 gap/lesson，checkpoint 指向不存在的 phase）。
3. **组织级扩展的先决条件**：ForgeOS 要成为多用户平台（north-star 的「多租户 + 成本治理是平台级一等公民」），运行隔离是最底层的结构前提。没有 run identity，所有后续的成本归属、trace 审计、用户隔离都是空中楼阁。
4. **灾难恢复盲区**：无 run ID → 无法回答「上个 run 什么时候跑的？谁跑的？在哪个分支？收敛了吗？」。`trace.jsonl` 事件缺少 `run_id` 和 `branch` 字段，无法按 run 过滤。

**方向建议**：
- `RunIdentity` 结构体：`{ID uuid, Branch string, StartTime, Workflow string}`，创建时注入，持久化到 checkpoint/trace/memory 的所有事件
- `.forge/` 目录按 run ID 分片或加前缀：`.forge/runs/<run-id>/checkpoint.json`、`.forge/runs/<run-id>/trace.jsonl`
- 添加 `forge run ls` / `forge run inspect <id>` —— 列出历史 run，查看单个 run 的 trace/checkpoint
- 添加分支感知：run 创建时读 `git rev-parse --abbrev-ref HEAD`，注入 trace 事件；切分支后自动选择该分支最后成功的 checkpoint
- memory 按 branch 隔离：不同分支的 knowledge 不应交叉污染

**优先级**：P1。不是 P0 因为当前单用户串行使用不受影响，但**首次接入并行 evolve 或多人协作时，数据损坏是结构确定的**，不是概率问题。

---

## 方向二 · Agent Output Veracity Gate（Agent 输出真实性闸门）

**当前现状**：ForgeOS 有 8 类代码质量闸门（test/complexity/lint/build/arch/security/secret/coverage），但**没有任何闸门检查 agent 输出的语言内容**——agent 写的 plan、claim、reasoning、promise 是否与它实际产生的代码变更一致。

**具体代码证据**：
- `command_executor.go:228-231` — `cappedBuffer` 捕获 agent 原始输出，`observe` 回调将其传递给 `cost.go` 做成本解析，**但输出的文本内容（agent 的 reasoning、commitment、claims）从不被结构化分析**
- `orchestrator.go:189-193` — `RunFrom` 对 agent phase 执行完 `runAgentPhase` 后只检查 `agentOutcome`（verdict 解析），**不检查 agent 说了什么是否兑现**
- `cost.go` — parseClaudeCostUsd / parseReviewerVerdict / parseConfidenceScore 全部是**结构化 token 解析**（末行匹配），不是**语义一致性验证**
- `prompt_context.go` — `buildPrompt` 注入大量上下文引导 agent，但**反向管道不存在**——agent 输出不流回 prompt builder 做自洽检查
- `converge.go` 的 `gatherSignals` — 信号全部来自外部客观测量（gate 状态、git diff、checklist 勾选），**从不包含 agent 输出的自评一致性度量**

**为什么需要**：
1. **hallucination 存在盲道**：agent 可以在 plan 里写「我重构了模块 X」，但实际只改了配置文件的缩进。当前系统中，只要 test 绿、complexity 不超、arch-check 过，这个虚假 claim 永远不会被检出——即使 plan 是 feeds_forward 给下个 phase 的核心输入。
2. **promise-keeping 无法衡量**：agent 在 planner phase 承诺「本 sprint 完成 P1+P2」，但 implementer 只做了 P1。当前 `FileDelta` 只做粗糙的关键词匹配（ROADMAP 勾选状态 × git diff），不做语义级别的「是否匹配 agent 自己的承诺」。
3. **reviewer 信号损失**：reviewer 产出的 findings 虽然被 `KindLesson` 记入 memory，但**没有自动验证**——reviewer 说「这段代码缺少错误处理」，没有后续机制检查 implementer 是否真的补了。当前的 loop-back 只检查 gate 结果，不检查 review finding 的逐条 closure。
4. **信任度无法量化**：没有 agent 的历史 truthfulness 指标。一个经常 hallucinate 的 agent 和另一个准确保守的 agent，当前 routing 一视同仁——只按 task_type + mode 路由，不按 truthfulness score。

**方向建议**：
- **Claim Extraction Gate**：在 agent phase 输出后添加一道轻量闸门，扫描 agent 输出中的可验证声明（「实现了 X」「修复了 bug Y」「增加了测试 Z」）并与实际变更做一致性匹配（git diff 模式匹配、文件存在检测、符号引用检测）
- **Truthfulness Score**：每个 agent phase 产生一个 `truthfulness_score ∈ [0,1]`，记录到 trace + scorecard，供下游 routing 参考（低 truthfulness agent 被降档或要求自证）
- **Finding Closure Tracking**：reviewer 的每条 `KindLesson` 关联一个 `expected_outcome`（如 `file:src/a.go, finding:"missing error handling"`），后续 implementer phase 结束后自动检查该 outcome 是否达成
- **Agent Consistency Check**：对 feeds_forward 的 phase 输出，在其下游 agent 执行完毕后,对比「上游声称要做的事」和「下游实际做的事」

**优先级**：P1。不是 P0 因为现有 code gate 体系已经提供了代码质量的客观验证——但 truthfulness 是决定**多 agent 协作的可信度**和**长时间自治运行中的 drift 检测**的关键能力。

---

## 方向三 · Workflow ROI Analysis & Adaptive Optimization（工作流 ROI 分析与自适应优化）

**当前现状**：系统已经有完整的 cost telemetry（`cost.go` 的 `runBudget`、trace 的 `cost_usd_micros`）和 latency measurement（`command_executor.go` 的 `runMeasured`），以及 scorecard 的 per-model 聚合。但**没有将这些原始数据转化为可指导决策的 ROI 分析**。

**具体代码证据**：
- `cost.go:80-120` — `feed()` 记录每个 billed phase 的实际 USD 花费
- `trace.go:78-92` — `Event` 有 `DurationMs` / `CostUsdMicros` / `Model`，但无 **ROI 或 value** 字段
- `loop.go:160-180` — `Run()` 每迭代调用 `reportConvergence`，只输出收敛状态，不输出**成本效率**或**趋势比较**
- `scorecard-update.mjs` — 聚合 p95 延迟和 avg_cost_usd，但无 **per-phase delta**（这轮 implementer 的产出比上轮多了还是少了？）
- `engine_build.go:236-247` — `phaseTierResolver` 决定档位后，**不追踪该档位决策的实际效果**——无法回答「今天用 Sonnet 跑 implementer 比昨天用 Haiku 跑，质量/速度/成本分别如何？」

**为什么需要**：
1. **无法识别浪费的 phase**：一个 `forge evolve` 跑 10 迭代，每个迭代的 `scan` phase 都 cos $0.18 + 90s，但 10 次中 8 次产生零 delta（没有任何 repo 结构变化需要扫描）。没有任何机制检测并建议跳过 redundant phase。
2. **模式选择盲目**：`--mode balanced` vs `--mode engineering` 的成本差异可以差 3-5x，但实际**没有工具告诉你「你的项目在 engineering 模式下 converge 所需的平均 phase 数 vs balanced 模式」**。用户只能靠感觉选 mode。
3. **无法做预算规划**：`--run-budget-usd` 已经可实现，但用户没有任何数据支持来设这个值。没有历史分析来回答「这个 workflow 上次 converge 花了多少钱？P50/P95 是多少？」——用户只能盲目试错。
4. **成本归因只到 phase，不到 roadmap item**：当前 trace 按 phase 记成本。但一个 implementer phase 可能同时推进 ROADMAP 的 P1 和 P2（写了两个文件），或者一个 phase 零产出（只改了 README typos）。成本按 phase 归因无法回答「P1 的成本是多少？这个 PR 的 ROI 是多少？」

**方向建议**：
- **Phase Delta Analysis**：在 trace 中增加 `files_changed` / `roadmap_items_touched` / `gate_delta`（本次 phase 使哪些 gate 从红变绿），使每个 agent phase 的「产出度量」结构化
- **ROI Dashboard (CLI)**：`forge analyze` 读取 trace 历史，输出 per-workflow/per-phase/per-mode 的成本效率报表（例如：`review phase: avg $0.18, p95 $0.25, avg findings 3.2, avg closure rate 68%`）
- **Redundant Phase Detection**：分析历史 trace，标记那些连续 N 次迭代都产生零 net change 的 phase，在启动前发出 advisory warning
- **Budget Recommendation**：基于历史数据，为用户建议合理的 `--run-budget-usd` 值（「此 workflow 历史 converge 成本的 P95 是 $1.80」）
- **Adaptive Phase Skipping (opt-in)**：允许 workflow 声明 `skip_if: {metric: files_changed, operator: "<", threshold: 1, for_N_iterations: 3}`——当某个 phase 连续 N 次迭代没有文件变更时自动跳过

**优先级**：P2。基础 cost telemetry 已够用（Sprint 26 已交付真 claude 成本数据），ROI 分析是增量价值，不影响核心功能。

---

## 方向四 · Prompt Construction Observability & Health Engineering（Prompt 构建可观测性与健康工程）

**当前现状**：ForgeOS 的核心产出是 prompt——它是 agent 看到的一切。但 `buildPrompt` 的输出完全不透明：不记录组件大小、不检测上下文窗口使用率、不验证各 lane 是否按预期注入、不跟踪 prompt 的组成随时间的变化。

**具体代码证据**：
- `prompt_context.go` — `buildPrompt` 将多 lane 内容拼接成最终 prompt（constraints lane / task lane / memory lane / artifacts lane），但**不记录每个 lane 的 token 数或总 token 数**
- `prompt/cache.go` — `ContextCache` 缓存 ADR/AGENTS 的预构建上下文，但**没有缓存命中率/未命中成本的数据**
- `retrieve.go` — TF-IDF 检索执行后**不记录检索质量**（top-1 的相关性得分、每 doc 的 score、检索耗时）
- `prompt.go` — `Gather` 从磁盘读文件构建 context，但**没有注入完整性检查**——一个期望 `uses_template` 的文件如果不存在或不可读，当前系统静默跳过，不产生告警
- `main.go:63-70` — `defaultAgentAllowedTools` 是硬编码的 node 验证 whitelist。对于非 node 项目，用户必须手动 `--agent-allowed-tools`。但 prompt 本身无法知道这个配置是否正确——一个 Python 项目用 node whitelist，`node --test` 会失败，agent 无法自测，不能诚实勾 ROADMAP

**为什么需要**：
1. **提示工程是盲飞**：ForgeOS 是一个 prompt construction 系统——它的大部分价值来自它提供给 agent 的上下文质量。但当前**prompt 是黑盒**：你只知道「我给了 agent ADR + AGENTS + memory + task」，但你不知道（a）每个组件占了多少 token，（b）检索是否真的找到了相关条目，（c）各注入组件的比分如何（是否 memory lane 占了 90% 而 constraints lane 只占 1%？）。
2. **上下文窗口 silently 接近极限**：随着 ADR 数量增长（>20）、memory 累积（>200 entry）、ROADMAP 变长，prompt 悄无声息地膨胀。当前无告警、无预算、无截断可见性。agent 在上下文窗口边缘的行为不可预测（随机忽略尾部内容、注意力稀释），但用户得到的是「看起来正常但输出质量下降」。
3. **注入退化无检测**：如果某个 ADR 文件被删除、某个 skill 模板路径变更、某个 memory 文件损坏，prompt builder 都静默跳过。没有机制验证「我预期注入的内容是否真的注入了」。
4. **无法做 A/B 比较**：当 prompt 逻辑变化时（比如调整了 memory lane 的顺序），无法量化变化的效果——「这个 prompt 变动让 agent 的 test 首次通过率提升了还是下降了？」

**方向建议**：
- **Prompt Snapshot**：每次 `buildPrompt` 执行后在 trace 中写入 `prompt_structure` 事件，包含：`total_tokens_est`（保守估计）、`lanes: [{name, chars, est_tokens, source_files}]`——使 prompt 的组成可追溯、可审计
- **Context Window Budget**：每个 mode 声明一个 `context_budget_ratio: 0.8`（最大 context window 的 80%），`buildPrompt` 在注入前检查总预算。超限时按优先级截断低价值 lane（约束 lane > 任务 lane > 记忆 lane），并记录截断事件
- **Injection Completeness Check**：对每个声明了 `uses_template` / `secondary_template` 的 phase，验证文件存在且非空，不存在则写入 `injection_miss` trace 事件并 advisory 告警（非阻断）
- **Prompt Version Fingerprinting**：`buildPrompt` 的 `Gather` 计算输入文件的 hash 集合，作为 prompt 版本指纹写入 trace。后续的 agent quality_score 可按 prompt 版本分桶，识别 prompt drift 对质量的影响
- **`forge prompt inspect`**：诊断命令，输入 workflow + phase，输出该 phase 的 prompt 结构预览——各个 lane 的内容摘要、token 估算、来源文件列表

**优先级**：P2。不影响核心功能，但在真点火长跑和项目规模增长后价值线性放大。建议在 ADR 数量超过 15 条或 memory 超过 200 条时开始实施。

---

## 方向五 · Multi-Branch & Parallel Lineage Workflow（多分支与并行谱系工作流）

**当前现状**：ForgeOS 完全假设单分支、单主线的演进模式。`.forge/` 状态全局共享，ROADMAP 是单份 checklist，checkpoint 是单文件覆盖。没有 git branch 感知，没有多谱系并行 evolve，没有「AB 两个分支各自跑 evolve，然后对比结果合并」的工作流。

**具体代码证据**：
- `main.go:450-454` — `forgeDir` 只按 repo root 分，不按 branch 分
- `evolve.go:118` — checkpoint 读/写完全忽略当前 git branch
- `evolve.go:469-475` — trace 写入单文件，两个分支的数据不可分
- `memory.go` — memory 没有 branch/lineage tag，两个分支的 Gap/Decision 混在一起
- `routing/scorecard.go:13-19` — scorecard 是 per-repo 的，不区分 branch，所以不同分支上的表现混合统计
- `file_delta_test.go` — FileDelta 的 git diff 只针对当前 HEAD，不跨分支比较

**为什么需要**：
1. **AI 平行宇宙无法比较**：团队想试两条 evolve 路线——分支 A 用 `--mode engineering` + Opus，分支 B 用 `--mode balanced` + Sonnet。当前系统无法隔离这两条路线的 state，结果是两个 loop 相互覆盖 checkpoint/memory/trace，或者必须 clone 两份仓库。
2. **无变更审查准备**：AI 在分支上工作完毕后，当前没有封装好的「对比 diff 摘要 + 提交 PR」流程。`forge run` 产出代码变更后，用户要手动 `git diff` 理解做了什么、手动写 commit message。对于 24h 自治运行，这个人在回路中的瓶颈是致命的。
3. **合并冲突放大**：两个 AI agent 独立修改同一个文件的不同部分（一个改业务逻辑，一个改测试），传统的 git merge 可以处理，但 context 碎片化——每个 agent 都不知道对方的存在。ForgeOS 作为编排层，应该感知并协调跨分支的工作。
4. **AB 测试不可能**：routing 的记分卡（scorecard）是按 (model, task_type) 聚合的，但无法按「方式」聚合——「方式 A（Opus + 完整 review）vs 方式 B（Sonnet + skip review）在下游绑定测试的表现」。这意味着 learning loop 只能在单一路线上优化，无法探索不同策略。

**方向建议**：
- **Branch-Aware State Store**：`.forge/runs/<branch-normalized>/` 目录结构，checkpoint/memory/trace 按分支隔离。切分支后自动选择该分支的状态（或提示用户选择）
- **`forge branch` 子命令**：列出所有已知的分支及其最后一次 converge 状态、最后运行时间、收敛状态
- **`forge diff` 增强**：`forge diff --branch <name>` 输出该分支相对主线的结构化变更摘要（文件、gate 状态、成本、收敛前的迭代数），为合并决策提供数据
- **Cross-Branch Scorecard**：记分卡增加 `branch` 维度，使不同策略的效果可量化比较（`forge route --compare`）
- **Parallel Lineage Executor**：允许用户提交一个「策略矩阵」——N 个 mode/model 组合跑相同的 workflow——自动隔离 run 状态、自动比较结果

**优先级**：P1。这是从「个人 AI 辅助工具」进化到「团队 AI 平台」的最大结构缺口。当前架构中加 branch 隔离是纯收益低风险的改造（不改核心编排逻辑，只改 state store 路径），但越晚做迁移成本越高。

---

## 优先级总览

| 方向 | 优先级 | 驱动力 | 一句话杠杆 |
|---|---|---|---|
| 一 · Run Identity & State Isolation | **P1** | 数据安全 | 并发 run 确定损坏共享 state，是多人协作/并行 evolve 的结构前提 |
| 二 · Agent Output Veracity Gate | **P1** | 可信度 | code gate 已经验证代码质量，但没有 gate 验证 agent 的 claim，多 agent 协作的信任链条是断的 |
| 三 · Workflow ROI Analysis | P2 | 成本优化 | 原始 cost telemetry 已就位，ROI 分析是增量价值，让数据飞轮从「可观测」进化为「可决策」 |
| 四 · Prompt Health Engineering | P2 | 质量保障 | prompt 是系统的核心产出，但完全不透明。项目规模增长后问题线性放大 |
| 五 · Multi-Branch & Lineage | **P1** | 团队扩展 | 单人单分支假设是整个架构最深的结构缺口，阻碍 ForgeOS 成为团队平台 |

**建议收敛**：如果本轮只能做两件，做**方向一 + 方向五**——两者都涉及 `.forge/` 的 state store 改造，可以合并实施（一次路径重构同时解决 run isolation 和 branch awareness）。方向二是独立于 state store 的纯逻辑层新增，可以与方向一/五并行开发。

---

> **写作诚实声明**：上述五个方向是基于 2026-07-10 代码库全局扫描的独立分析结果。本人尽力排除已被已有 27 份需求分析文档及 `.agent/` 文档覆盖的方向。如果你发现某个方向已在某份文档中被完整提出，请指正——不追求「必须新」，追求「确实有价值」。
