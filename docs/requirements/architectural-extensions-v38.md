# 高价值扩展方向分析（v38）

> 基于 2026-07-10 代码库全局扫描，以资深架构师/产品经理视角提出 3-5 个高价值扩展方向。
> 范围涵盖核心功能点、边界情况（Edge cases）处理和性能优化点。
> 每个方向均说明「为什么需要它」——从产品价值、架构完整性和工程约束三维论证。

---

## 方向一：多项目治理联邦（Federation Governance）

### 问题描述

当前 ForgeOS 的所有治理配置（`.agent/`、`project.yml`、`policies.yml`）强绑定**单个仓库根目录**。`gate.RepoRoot()` 假定一个 repo 一个世界。但在真实组织中：
- **Monorepo** 内含多个独立服务/库（如 `services/gateway/`、`services/payment/`），各自有不同的 lifecycle（一个 MVP、一个 production）、不同的 mode、不同的路由策略。
- **Polyrepo** 微服务架构下，20 个 repo 需要 20 次手动 `forge-init`，且策略极易漂移——没有「组织级基线 + 项目级覆盖」的层次化治理。
- **共享库**（shared lib / private package）被多个服务引入，但其治理策略与消费方无关，目前无法独立声明。

### 为什么需要

1. **产品价值**：ForgeOS 的目标是「AI-native 软件工厂」。一个工厂管理多条产线是基本要求。不支持多项目联邦，ForgeOS 只能管理「一个玩具项目」，无法进入真实企业场景。
2. **架构完整性**：中枢旋钮（mode×lifecycle）当前是单 repo 全局的。monorepo 内不同子项目应有独立的 mode/lifecycle，但共享 harness gate 策略。这需要引入**策略继承链**（organization policy → team policy → project policy），类似 Open Policy Agent 的层次化决策树。
3. **边际成本低**：已有 `Policy` 结构体、`lifecycleMod` 表、`projectYAMLValue` 读 project.yml 的脚手架。扩展点在 `internal/mode` 包加一个 `PolicyStack` 解析链（org → team → project），在 `harness` 层加 `--include-policy` 或目录级 `.forge-policy.yml`。当前 `gate.RepoRoot` 只需改为可指定子目录根。

### 关键边界与难点

- **跨 repo 共享策略**：需要决定策略事实源放在哪——单独 `forge-policy` repo？嵌入每个 repo 再同步？「策略即代码」的推拉模型需设计。
- **继承冲突裁决**：org 说 `enforce=block`，team 说 `enforce=warn`——winner 是 stricter（同 lifecycle floor 的 only-tighten 原则），但必须显式记录在案而非静默裁决。
- **monorepo 内执行隔离**：harness gate 跑在 repo 根，但 gate 结果需要按子项目归因。当前 `walk()` 全仓扫文件，改为按子项目边界扫。

---

## 方向二：自适应循环组装（Adaptive Loop Assembly）

### 问题描述

ForgeOS 现有 **5 个静态 workflow YAML**（discover/design/build/review/evolve），`forge detect` 能做**项目类型检测**（语言/生命周期/测试/CI 有无）并**建议**一个 workflow，但从不**动态调整** workflow 本身的结构。这意味着：

- 一个 Rust + Redis 的项目和纯 Python CRUD 应用走**完全相同的 build.yml**（相同的 phase 序列、相同的 agent 卡、相同的 gate 集）。
- `evolve` 的迭代深度只有 `--max-iter` 一个旋钮，不能根据「扫描发现的 gap 数量和类型」动态决定是 deep-dive 还是 quick-fix。
- 「生命周期」的推进（idea → mvp → growth → production）依赖 `forge migrate` 的手动触发，而不是自动感知项目成熟度后调整 workflow。

### 为什么需要

1. **产品价值**：静态 workflow 对于「AI 自治 24h 无人值守」是根本性限制。机器不应该在每轮迭代里跑与上一轮完全相同的 phase——它应该根据**当前状态**动态决定「这次该做什么」。这是扫描→分析→行动闭环的核心。
2. **架构完整性**：BOOTSTRAP.md 的 Evolve 阶段描述是「Scan → Gap → Roadmap → Implement → Review → Evaluate」，但 `evolve.yml` 实际上就是这 6 个 phase 的死循环。真正的「Gap 驱动」应该是：扫描结果决定下一轮 phase 列表——发现架构违规则插入 architecture-review phase，发现测试缺失则插入 test-backfill phase。目前需要用户或 agent 手动在 ROADMAP 里加 item。
3. **性能优化**：每轮迭代跑全部 6 个 phase 是浪费的。如果上一轮 scan 发现「零 gap」，则下一轮不应该再跑 implement → review → qa，而应该直接走 evaluate → stop。自适应组装可大幅减少空转迭代（当前靠 `NoProgress` tripwire 硬杀，浪费了至少 2 轮）。

### 关键边界与难点

- **phase 间依赖声明**：动态组装需要 phase 的输入/输出契约（当前 `Emits` 字段已声明产出文件，`DependsOn` 已声明先后顺序）——但缺少「gates required before this phase」的动态版本。需要 phase 注册自己的前置条件和输出 artifact。
- **收敛定义的挑战**：静态 workflow 的 stop_condition 写在 YAML 里。动态组装时，stop_condition 本身也应是动态的——「修补所有已知 gap」vs「修补高优先级 gap」vs「修补安全 gap」。需要 stop_condition 的组合子（AND/OR/THRESHOLD）能在 phase 层面声明。
- **可审计性**：每一轮的 phase 列表必须被 trace 捕获（`forgeos.trace.v1` 格式已支持），以便事后复盘「为何这次跳过了 security review」。不能静默改变行为。
- **实现路径**：从 `forge detect` 输出 profile → 一个 `internal/composer` 包（与 `internal/mode` 平级）将 profile 映射为 phase 列表 + gate 列表 + model tier。当前 `internal/routing.TierFor` + `internal/mode.Policy` 已提供大部分构建块。

---

## 方向三：知识引擎与语义检索（Knowledge Engine + Retrieval-Augmented Generation）

### 问题描述

`.agent/ARCHITECTURE.md` 列出了 **10 大引擎**，其中「Knowledge-Engine」和「Context-Engine」是两座并列的支柱。目前：
- **Context-Engine**（`internal/prompt`）已实现：从 ADR/AGENTS/ROADMAP/约束中构建 prompt 上下文，支持 feed-forward 和 fresh-context。
- **Memory-Engine**（`internal/memory`）已实现：JSONL 日志式知识持久存储，支持带 supersede 语义的查询。
- **Knowledge-Engine**（语义检索 / RAG）**从未实现**：`memory.Query` 只是精确匹配 kind+topic，没有 TF-IDF、没有 embedding、没有跨 session 相关性排序、没有自动将「过去决策」注入当前 agent prompt 的机制。

这意味着：当前 agent 只能通过 `phaseOutputLedger`（前一个 phase 的产出）+ `memory.Load`（精确 topic 查询）+ `retrieve`（按约束查找）获得上下文。它无法做「语义相似搜索」——例如 reviewer 无法自动找到「与本次修改最相似的上一轮 review 裁决」。

### 为什么需要

1. **产品价值**：长时间自治运行的核心挑战是**非遗忘性**。Memory-Engine 解决了「记住」（append-only log），但没解决「检索」（find the relevant past decision）。没有语义检索，agent 每轮迭代都在「近似失忆」状态下工作——它记得「有件事发生了」但找不到具体内容。对于 24h+ 运行，这是根本性瓶颈。
2. **架构完整性**：10 大引擎列表里 Knowledge-Engine 是显式缺口。它被列为与已经实现的 Context-Engine / Memory-Engine 同等重要的引擎，但零代码。
3. **合理的轻量级实现**：不需要 LLM-heavy embedding 模型。一个**TF-IDF 倒排索引**（基于 `internal/memory` + ADR 文本 + agent 卡内容构建）在 v1 范围内足够好：embedding 维度是可控词汇表大小（项目专有名词有限），而非开放域。Go 纯标准库可实现（`internal/knowledge` 包，依赖已有的 `internal/memory` 和 `internal/asset`）。
4. **性能优化**：当前 `memory.Load` 每次都全量读文件+解析+过滤。语义检索可附加摘要缓存（`memory.Prune` + `Compact` 已有雏形），大幅降低长运行时的 I/O 开销。

### 关键边界与难点

- **注入时机与容量控制**：语义检索结果不能无限制灌进 prompt。需要像当前 `retrieve` 的 `taskCap` 一样，加一个 `knowledgeCap`（token budget），按相关性排序截断。当前 `buildPrompt` 的多 lane 注入机制（约束 lane / 任务 lane / 记忆 lane）已给出注入点。
- **跨 session 一致性**：`loadCache`（mtime 缓存）在写入后 `invalidateLoadCache()`，但 Knowledge-Engine 的索引需要在每次 `memory.Append` 后增量更新（而非全量重建）。当前已有 Append→invalidate 的模式。
- **Honesty 边界**：语义检索是「近似搜索」，可能返回不相关结果。agent 必须被提示「检索结果来自知识库，可能不精确」，且必须有 fallback（元信息自带精确引用）。当前 `prompt_context.go` 的 `injectKnowledge/retrieveKnowledge` 是天然接入点。

---

## 方向四：生产级并行编排安全网（Production-Grade Parallel Safety）

### 问题描述

`forge-core` 已实现**可选并行执行引擎**（`orchestrator.RunParallel`，Sprint 25+），支持依赖波（dependency waves）内 phase 并发执行。但该实现停留在一个「功能演示」的成熟度，缺少多条生产级安全特性：

| 缺口 | 当前状态 | 风险 |
|---|---|---|
| 成本追踪竞态 | `cost.go` 注释明确说「mutex 待加」 | 并行 phase 同时 `feed()` → spend 竞态 → 预算失控或锁死 |
| 无 per-phase 并行超时 | 共享 `--timeout`，波内一个 phase 超时 cancel 整个波 | 一个慢 phase 拖死其他独立 phase |
| 无资源感知调度 | 没有并发上限（goroutine 无界），没有 CPU/内存约束 | 20 个 phase 同时 spawn 20 个 agent → OOM |
| 无波级重试 | 一个 phase 失败 → 波取消（fail-fast）。波内已成功的 phase 重跑浪费 | 瞬态故障不应丢失已完成的工作 |
| 无渐进降级 | 并行模式要求所有 phase 声明 `depends_on`，否则静默退回串行 | 用户期望并行但没声明 deps → 完全串行，无提示 |

### 为什么需要

1. **产品价值**：并行是 `forge evolve` 从「演示速度」到「真实速度」的关键。目前的并行只对 fan-out（多个独立 implementer）有价值——这是对 discover 阶段（scan/market/capability 可并行）和 build 阶段（多个独立 feature 可并行）的直接加数。不给安全网，这些场景无法投入真实使用。
2. **性能优化**：并发 phase 带来的加速是线性的（最多到 CPU 或 API 瓶颈）。但缺乏资源感知调度会导致**负加速**：超过 API 速率限制后，并发退化为排队+重试，比串行更慢。已有 `backoff.go` 的 529 重试支持，但缺并发仲裁。
3. **架构完整性**：`parallel.go` 文件头的锁顺序契约（LOCK ORDER CONTRACT）是优秀的设计文档，但只覆盖了锁顺序这一个维度。需要扩展为**完整的并发安全契约**，覆盖资源预算、trace 写入、checkpoint 持久化等。
4. **边际成本适中**：
   - `runBudget.mu` 已声明（`sync.Mutex`），只在 `feed` 和 `SpendRatio` 处加锁——改动是加锁范围调整，不是新设计。
   - `internal/orchestrator/parallel.go` 已有 `runWave` 的 per-wave ctx 和 WaitGroup，加 semaphore-channel 的 goroutine 限流是纯加法。
   - 波级重试可复用 `backoff.go` 的 `overloadBackoff` 逻辑（interface 已解耦）。

### 关键边界与难点

- **checkpoint 与并行不兼容**：当前设计明确说「parallel 无 per-phase checkpoint，因为并发 phase 无线性 index」。这意味着 crash→resume 在并行模式下只能从 iteration 边界恢复，损失已完成的波的工作。一个折中是「per-wave checkpoint」——波完成时而非 phase 完成时存 checkpoint。
- **成本公平性**：并行 phase 共享 run budget。当一个 phase 烧光预算，它应该只取消自己而不影响同一波的其他 phase。当前 `BudgetExhausted` 是全局的（engine-level puller），不是 per-phase 的。需要预算的 per-phase 分配（类似容器的 cgroup）。
- **trace 线序**：`trace.Tracer.Emit` 在 mutex 下分配 Seq，并发 phase 的 trace 事件 Seq 交错——这是预期的，但下游工具（scorecard）需处理交错。

---

## 方向五：自动变更影响分析与智能门控（Automated Impact Analysis + Intelligent Gate Bypass）

### 问题描述

风险分类器（`internal/risk`）当前完全依赖**显式声明信号**（`TouchesPayment`、`BlastRadius` 等由调用者手工传入——`forge route --touches-payment` / `--blast-radius`）。`risk.FromChangedPaths`（Sprint 9）做了一次轻量自动化：从 git diff 的文件路径做子串匹配（检测 `payment`、`auth`、`secret` 等关键词）。但：

1. **只读路径，不读内容**——一个文件叫 `auth.go` 但只改了注释，依然被标记 `TouchesAuth=true`（假阳性高）。
2. **不计算真实影响范围**——`BlastRadius` 被硬编码为文件数，不是调用图可达函数数。改一个公共库函数，所有 import 者都是 blast radius。
3. **不驱动智能门控**——风险高→强制 Opus（已实现）、风险低→skip security review（**未实现**）。当前的 mode gating 只能根据 mode×lifecycle 跳过整个 review stage，不能根据单次变更的风险**选择性地跳过某个 gate**。
4. **无调用图分析**——改 `UserService.Login()`，不知道它被 `AuthMiddleware` 引用，也不知道 `AuthMiddleware` 被 `Gateway.HandleRequest` 引用，所以 blast radius 被严重低估。

### 为什么需要

1. **产品价值**：这是「用一个昂贵的审查代替所有审查」与「跳过所有审查」之间的第三条路。真实情况是：90% 的变更是小修小补（改 typo、加 log、文档更新），不需要跑安全审查+分布式审查+性能审查+CTO 综合裁决。但 10% 的变更（改 auth、改支付、改核心数据模型）必须全部通过。没有自动化影响分析，就必须让所有变更都走完整 review → 这是 v1 的 safe-by-default，但也是**无谓的成本**。
2. **性能优化**：`forge run review` 跑 4-5 个 agent phase（security/distributed/performance/CTO），每个都是 Opus 档调用。如果 90% 的变更可以跳过其中 3 个，成本降低约 70%。
3. **架构完整性**：
   - `internal/risk` 已有 `FromChangedPaths` 的改名（已导出、已测试）。加内容级扫描（AST 提取函数调用关系）是自然演进。
   - `internal/routing` 已有 `TierForScore` 的多维打分。把 blast radius / impact 纳入 score 的 weight 计算已在 `Score()` 函数中得到支持（`weights` 参数），只是没有自动化的 input source。
   - `determineGateSet`（目前不存在）可以从 risk level 推 recomended gate 子集（低风险→只跑 lint+test，高风险→全 gate）。当前 `Policy.Gates` 的 intersection 机制已是现成的组合子。
4. **与现有系统无缝集成**：
   - 影响分析结果可以注入 `Signals.FileDelta` + `Signals.CodeTestRatio` + 新增的 `Signals.ImpactBlastRadius`。
   - 不需要新 workflow，只需要 `forge run / forge evolve` 在起跑前做一次快速影响分析，然后传参给 `mode.Effective` 或一个新的 `gateSetForRisk(risk.Level)` 函数。

### 关键边界与难点

- **Go 调用图分析在零依赖限制下**：`forge-core` 零外部依赖，意味着不能引入 `golang.org/x/tools/go/callgraph`。替代方案是轻量级 AST 扫描：只查同一包内的函数引用（`import` 关系 + 函数名匹配），这是 v1 范围——不求精确调用图，只求比「文件路径子串匹配」更好。跨包分析需要解析 `import` 路径（Go 标准库 `go/parser` + `go/ast` 是 stdlib，可用）。
- **假阴性 vs 假阳性权衡**：任何近似分析都会有误判。架构原则是「当不确定时，放过（fail-open for gate bypass）但标记」。即：如果影响分析不确定，不走 gate bypass，走全量 review 但**在 trace 里记录「无法评估影响，回退全量」**。这与当前 `classifyClaudeOverload` 的「rather miss than mis-fire」原则一致。
- **与 lifecycle mode 的交互**：即使影响分析说「低风险」，production lifecycle 的一票否决（`applyLifecycle` 的 production floor）仍然强制全量 gate——安全 override 高于效率优化。这是非协商原则。
- **增量分析的持久化**：`BlastRadius` 结果应被缓存（map 路径→影响得分），避免每次 `forge run` 都重新扫描整个 AST。`internal/persist` 的 checkpoint 机制或 `internal/memory` 的 key-value 模式可复用。

---

## 总结对比

| 方向 | 产品价值 | 架构完整性 | 性能收益 | 实现成本估计 | 风险 |
|------|---------|-----------|---------|------------|------|
| 一：多项目联邦 | 高（进入企业场景） | 高（补引擎声明缺口） | 中（复用 gate 缓存） | 3-4 周 | 策略继承链设计需审慎 |
| 二：自适应循环 | 高（真正自治） | 高（闭环缺失环节） | 高（减少空转迭代） | 4-6 周 | phase 依赖声明需严谨 |
| 三：知识引擎 | 高（解决非遗忘性） | 高（10 引擎缺口） | 中（减少 I/O） | 2-3 周 | 检索质量难保证 |
| 四：并行安全网 | 中（加速收敛） | 中（补生产缺口） | 高（线性加速） | 2-3 周 | 锁顺序维护成本 |
| 五：智能门控 | 高（成本降低 70%） | 高（risk→gate 闭环） | 非常高 | 4-5 周 | AST 分析精确度不足 |

**推荐优先级**：方向三（知识引擎）→ 方向一（治理联邦）→ 方向五（智能门控）→ 方向四（并行安全）→ 方向二（自适应循环），按「当前已有脚手架最多 + 边际成本最低」排序。
