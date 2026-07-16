# ForgeOS 功能需求点全景扫描（未采纳构思目录）

> **扫描日期**：2026-07-16　·　**扫描范围**：项目根目录 + `docs/` + `docs/requirements/` + `docs/results/` 下全部 1,260 份 Markdown 文件
> **交互版本**（可搜索/筛选/排序全部 413 项）：https://claude.ai/code/artifact/166312f6-a962-4ea3-9f16-ba2853c83f90

## 这份文档是什么

`docs/requirements/`（438 个文件）与 `docs/results/`（778 个文件）绝大部分不是人工撰写的产品需求文档，
而是 ForgeOS 自身 `ai-dev/pi-batch.py` 驱动的自动化流水线反复调用 Claude 生成的**扩展方向构思稿**——
每份文档独立扮演"资深架构师 + 产品经理"角色，提出 4-6 个候选功能方向，并声称与此前所有文档均不重复。

本文档是对这 1,260 份文件的一次性全量扫描、去重、聚类与现状交叉校验，**不是**新的需求提案，
也**不是**已实现功能清单（已实现功能见 [`FUNCTIONAL_REQUIREMENTS_AUDIT.md`](FUNCTIONAL_REQUIREMENTS_AUDIT.md)）。

## 规模统计

| 指标 | 数值 |
|---|---|
| 扫描的 Markdown 文件 | 1,260 |
| 解析出的文档主题（含 `.md`/`.out.md` 配对） | 419 |
| 提取的原始候选"方向"条目 | 2,110 |
| 去重聚类后的独立需求主题 | **413** |
| 功能领域分类 | 21 |
| 标记为生成管线自认幻觉的条目 | 10 |

## 核心发现

1. **「Run Identity / 跨进程状态隔离」被独立重复提出 96 次**——`.forge/` 目录下 `checkpoint.json`/
   `trace.jsonl`/`memory.jsonl` 没有任何跨进程文件锁或运行身份标识，两个并发的 `forge run/evolve` 会互相覆盖状态。
   该诊断在「边界条件/并发可靠性」类目下被独立提出 48 次，在「可观测性/审计」类目下（从审计追踪角度）又被独立提出 48 次，
   全部来自不同措辞、不同标题的文档，彼此从未互相引用——是全语料库最显眼的单一结构性空白。
2. **至少 10 条"需求"是通用化的可观测性/断路器/重试模式提案**，与 ForgeOS 实际代码库脱节，
   且被源文档自身标注为 `[HALLUCINATED/OFF-TOPIC]`。已在下方目录中单独标记，不计入正常统计口径。
3. **元讽刺**：`other` 类目下有一条主题（"需求文档语料治理"）本身就是在提议构建一套新颖性去重管道
   （MinHash 近似去重签名 + 方向生命周期状态机），用来防止这种指数级重复生成——换言之，
   生成这批文档的流水线自己的产出物已经独立"发现"了自己的问题。

## 现状基线：与已实现功能的关系

对照仓库自带的 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（基于源码 + 实跑闸门的独立审计，2026-07-02/03 定稿）：

| 分桶 | 数量 | 说明 |
|---|---|---|
| DONE | ~115 | 已验证实现并测试，独立五轮审计交叉确认 |
| BLOCKED-EXTERNAL | 3 | 缺外部资源（Firecracker microVM / LiteLLM 多厂商 key / OSV-DB） |
| DEFERRED-BY-DESIGN | 15 | 主动搁置且有明确理由，非遗漏 |
| GAP（原始） | 12 | 全部已于审计当天（2026-07-02→03）修复或降级，无遗留 |

**v1 声明的能力边界内，审计口径下没有遗留的"真空白"。下方 413 个主题全部是面向 v2/v3 的探索性构思，
绝大多数从未经过人工筛选或去重，也从未进入 `.agent/ROADMAP.md` 的正式承诺。**

## 按功能领域分布

| 领域 | 独立主题数 | 原始条目数（去重前） |
|---|---:|---:|
| 可观测性/审计 (`observability-audit`) | 20 | 288 |
| 治理执法 (`governance-enforcement`) | 78 | 282 |
| 编排引擎补齐 (`orchestration-engine`) | 21 | 240 |
| 边界条件/并发可靠性 (`edge-case-reliability`) | 18 | 178 |
| 其他/跨领域架构治理 (`other`) | 19 | 125 |
| 测试基础设施 (`testing-infrastructure`) | 37 | 113 |
| Memory质量衰减/生命周期管理 (`memory-lifecycle`) | 31 | 110 |
| 预算治理/成本策略引擎 (`budget-cost-governance`) | 30 | 95 |
| 多厂商模型路由/LiteLLM (`multi-vendor-routing`) | 25 | 95 |
| 资源核算/容量规划 (`resource-accounting`) | 19 | 78 |
| 多仓库联邦/全局化治理 (`multi-repo-federation`) | 10 | 77 |
| Web UI/CLI DX (`web-ui-dx`) | 18 | 50 |
| 多用户协作 (`collaboration-teaming`) | 15 | 43 |
| 事件驱动/外部集成 (`event-driven-integration`) | 10 | 39 |
| OS级安全/进程隔离/凭证保护 (`os-security-isolation`) | 13 | 37 |
| 知识组织/语义层/ADR结构化 (`knowledge-semantic-layer`) | 16 | 37 |
| Agent输出确定性/可复现性 (`output-determinism`) | 8 | 20 |
| 供应链安全 (`supply-chain-security`) | 6 | 20 |
| Prompt注入防御/信任边界(非OS层) (`prompt-credential-safety`) | 8 | 16 |
| 知识可移植性 (`knowledge-portability`) | 6 | 14 |
| 沙箱运行时 (`sandbox-runtime`) | 5 | 9 |

## 方法论与局限性

- **提取阶段**：57 个并行子代理分批读取全部 419 个文档主题，按固定 21 类分类法抽取结构化"方向"条目
  （名称/类别/优先级/摘要/来源），全部成功完成。
- **聚类阶段**：21 个类目中 17 个由独立的 fresh-context 聚类代理去重合并；
  其余 4 个大类（`edge-case-reliability` 边界条件可靠性、`orchestration-engine` 编排引擎、
  `observability-audit` 可观测性/审计、`other` 其他杂项，合计约 974 条原始条目）
  因当次会话触发了两次基础设施故障（Claude Code 会话额度上限 + API 流式超时）转为由分析主线程直接人工聚类完成——
  这四个类目下方标注为 `⚠ 人工聚类`，精细度略低于自动化聚类的类目，条目计数为合理估计而非逐条精确去重。
- "提及次数"是去重后同一构想在语料库中重复出现的次数，**不代表**该构想的重要性或采纳优先级——
  次数高往往只反映该结构性空白在代码中足够显眼、容易被独立反复"发现"。
- 本文档不包含 `docs/results/` 中已进入 code/review 阶段的 34 个主题的详细代码评审内容——
  抽样检查显示这些"code"阶段产物大多是澄清性追问而非真实代码实现，未纳入功能点统计。
- 优先级信号（P0/P1/P2/P3/unstated）与最高成熟度（ideation-proposal/architected/reviewed/narrative-log 等）
  均为源文档自述，未经独立验证，仅供参考排序。

---

## 完整主题目录（按领域分组，领域内按提及次数降序）

### 可观测性/审计 `observability-audit` ⚠ 人工聚类

20 个独立主题，原始条目 288 条。

**可观测性从只写入到可查询 (trace/checkpoint/memory 无 Read/Query API/实时事件流)** `×55`

运行时数据只能通过文件轮询消费（tail/jq/手工 grep），没有 daemon、API server、事件总线或 forge trace 查询子命令；用户想知道一个跑了 6 小时的 evolve 循环当前在做什么、花了多少钱、卡在哪个 gate，唯一办法是 SSH 到机器读原始 trace.jsonl。这是观测类诊断中体量最大的母题，涵盖结构化日志升级(log/slog替代裸fmt.Printf)、trace Reader/Query/Filter API、Event Bus 实时事件流、forge trace/report/insights 等查询子命令族。

*优先级信号*: P0(x多)/P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: five-product-operational-gaps, 2026-07-11-five-genuinely-uncovered-runtime-frontiers, architecture-review-execution-semantics

**Run Identity / 跨存储关联ID (RunID 注入 trace/checkpoint/memory)** `×48`

trace.Event、checkpoint.Checkpoint、memory.Entry 三个持久化系统各自使用独立标识符（进程内 Seq/行号/时间戳），互相没有引用，也没有任何 run/session 级别的唯一标识。这是全库观测类诊断中出现频率最高的具体机制修复：注入 UUIDv7/ULID RunID 到每个事件/checkpoint/memory 条目，用于审计关联、跨运行对比与并发隔离。与 edge-case-reliability 类别中'多进程并发安全'主题共享同一根因，但这里的落脚点是可观测性/可审计性而非并发安全本身。

*优先级信号*: P0(x多)/P1(x多)　·　*最高成熟度*: coded-and-reviewed　·　*示例来源*: 2026-07-11-four-code-grounded-product-expansion-directions, forgeos-trust-operational-maturity, 2026-07-11-forgeos-five-unbuilt-product-architectural-extensions

**自监控/退化检测/持续健康检查循环 (doctor 从一次性诊断 → 24h daemon 化自愈)** `×32`

internal/doctor 是功能完整、有测试覆盖的诊断工具，但只在用户显式调用 forge doctor 时才运行一次，不是持续监控循环——trace.jsonl 无限增长无轮转告警、checkpoint 历史不清理、.forge/ 目录无大小上限，24h 无人值守运行中系统对自身的退化毫无感知。多份提案主张把 doctor 升级为可选的持续健康检查/自我诊断循环，甚至嵌入 LoopEngine 本身实现自愈。

*优先级信号*: P0(x多)/P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-product-architect-expansion-directions, 2026-07-12-five-expansion-directions-product-platform-perspective

**自动故障复盘/根因分析引擎 (forge autopsy/postmortem/diagnose)** `×24`

一次失败的 24h evolve 运行目前唯一的诊断手段是人工 grep trace.jsonl 并跨三个独立文件（trace/memory/scorecards）手动关联，没有任何工具自动综合这些信号给出'为什么失败'的结论。多份提案主张构建 forge autopsy/postmortem/diagnose 命令自动聚合根因分析，部分提案进一步提出规则驱动的自动修复建议（Failure Intelligence & Automated Remediation）。

*优先级信号*: P1(x多)/P2(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-architect-extension-directions, 2026-07-11-forgeos-five-structural-blindspots

**确定性回放/历史仿真引擎 (forge replay/simulate，从 trace 重建过去的执行)** `×22`

trace.jsonl+checkpoint.json+memory.jsonl 理论上包含重建一次历史运行所需的全部信号，但没有任何代码路径真正实现'时间旅行'式回放——trace 只记录发生了什么(结果)而非确切的 prompt 文本/agent 决策链，无法做失败运行的确定性重放调试。多份提案主张构建 internal/sim 或 forge replay 命令实现这一能力。

*优先级信号*: P0(x若干)/P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-genuine-codegrounded-extensions, 2026-07-11-five-product-architectural-expansion-directions-scanned

**跨存储一致性校验 (checkpoint/trace/memory/scorecard 独立写入无交叉校验)** `×14`

四个独立的 JSON/JSONL 存储各自写入、互不感知，理论上可能出现 checkpoint 记录已完成 phase N 但 trace 里找不到对应事件的不一致状态，当前没有任何交叉校验层能检测这类静默的数据完整性问题。

*优先级信号*: P1(x多)/P2　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-unseen-systemic-operational-gaps, 2026-07-11-forgeos-five-unbuilt-product-architectural-extensions

**因果追踪/Span 层级 (trace.Event 平面列表 → TraceID/SpanID/ParentSpanID DAG)** `×13`

trace.Event 目前是一个只有单调递增 Seq 的平面列表，没有 parent/child 关系，无法将一次 gate FAIL 追溯到触发它的具体 implementer phase，也无法将 loop-back 后的新 phase 事件关联回触发它的原始事件。多份提案主张仿照分布式追踪（OpenTelemetry 风格）给 trace.Event 增加 TraceID/SpanID/ParentSpanID/CausedBy 字段。

*优先级信号*: P0/P1/P2/P3 混合　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-genuine-post-closure-architectural-expansion-directions, architecture-review-execution-semantics

**不可逆决策审计追踪 (人工 approve 标记是空文件，无身份/理由/签名)** `×12`

`.forge/<stage>.approved` 只是一个空的 marker 文件，humanApproved() 只检查其是否存在——不记录是谁批准的、为什么批准、批准时对应哪个 commit/内容哈希。对于'human_gate 是全系统最高杠杆闸门'这一治理声明而言，审批本身缺乏任何可审计的身份/理由/防篡改记录是一个显著的合规缺口。

*优先级信号*: P1(x多)/P2　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-extension-directions-verification, 2026-07-11-five-deep-systemic-codegrounded-boundaries, genuinely-uncovered-five-frontiers

**结构化错误类型体系 (ExecError 只有 Kind 枚举，无 Code/Severity/RecoveryHint)** `×10`

7 种（后修正为 5 种）ExecError 类型只暴露 Go 的 Error() 字符串，没有机器可匹配的错误代码/严重度/恢复建议，结构化信息在冒泡到 main.go 的过程中被压扁成纯文本，下游消费者（scorecard/trace/CLI 输出）无法按错误语义做差异化处理。

*优先级信号*: P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-product-architect-expansion-directions, execution-semantic-gaps

**Agent 输出溯源与可验证性 (ArtifactManifest / hash-chain 证明谁产出了什么)** `×8`

经过 31 个 sprint 和 190+ 源文件之后，没有任何机制能回答'这个文件是哪个 agent、哪个 model、基于哪次 prompt 产出的'。多份提案主张引入基于哈希链（非 PKI/区块链）的轻量级 ArtifactManifest，记录 session_id/phase/agent/model/prompt_hash/文件 SHA256。

*优先级信号*: P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-unseen-product-architect-extensions

**Trace 轮转/生命周期管理缺失 (10MB 轮转只留 1 份备份，无跨进程锁)** `×8`

trace 文件达到 10MB 阈值轮转时只保留单一备份（trace.jsonl.1），会被下一次轮转覆盖丢失更早历史；轮转操作本身用非原子的 os.Rename，同一进程在轮转瞬间可能丢失少量正在写入的行。与 memory（有 Compact/Prune）和 checkpoint（有 retain-N 历史）相比，trace 明显缺乏对等的生命周期管理。

*优先级信号*: P1(x多)/P2　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-four-codelevel-extension-directions-after-four-deep-scans, 2026-07-11-forgeos-four-codegrounded-architectural-expansion-directions

**Agent 推理/决策可观测性 (Chain-of-Thought / Reasoning 字段捕获)** `×8`

trace.Event 记录结果（PASS/FAIL/APPROVE、耗时、成本、model）但从不记录 agent 做出该决策的推理过程（前提/结论/置信度），使得事后无法理解'agent 为什么这么判断'，只能看到判断本身。

*优先级信号*: P1/P2　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-systemic-blindspots

**收敛信号时序丢失 (converge.Signals 只有当前迭代快照，无历史)** `×6`

converge.Signals 只持有当前迭代的最终快照值，没有跨迭代的时间序列，无法回答'RoadmapCompletion 是在哪次迭代开始停滞不前的'这类调试问题，也是 Convergence Replay & Forensic Analysis 类提案的核心依据之一。

*优先级信号*: P2(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-product-architecture-expansion-directions

**Scorecard 聚合相位级盲点 / 元学习闭环缺失** `×6`

scorecard 在 task_type 级别聚合而非 phase 级别，单个异常样本会被平滑掩盖；同时 ForgeOS 现有学习闭环只覆盖任务级知识（memory/roadmap），缺少'系统学习如何更好地治理自身'的元学习层。

*优先级信号*: P1/P2　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-systemic-blindspots, 2026-07-11-forgeos-five-codegrounded-structural-gaps

**Prompt 装配可观测性与效能度量 (--dump-prompt / prompt_hash 用于优化闭环)** `×6`

buildPrompt 的多步骤上下文装配（agent 卡/ADR/AGENTS.md/memory 各 lane 的 token 估算与来源）目前是黑盒，修改 prompt 或 agent 卡后无法知道效果是变好还是变差，也无法做 A/B 对比；提案主张增加 --dump-prompt 调试通道和 prompt_hash/token_count 度量接入 scorecard。

*优先级信号*: P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: expansion-production-blindspots-v36

**Checkpoint/Scorecard 历史轨迹丢失 (每次覆写，无 RetainCount/trajectory)** `×6`

checkpoint.json 和 scorecards.json 在每次迭代/运行结束时被完整覆写，无法回答'第 5 次和第 50 次迭代之间发生了什么'，也无法做跨迭代的收敛轨迹回放。

*优先级信号*: P2(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-state-data-integrity-and-lifecycle-gaps, scan-new-angles

**[数据质量标记] 4条明确的幻觉/离题条目** `×4`

genuinely-uncovered-five-frontiers 及 high-value-extension-directions 系列(v1-v3)各生成了一条通用化的 Prometheus/Grafana/UsageCollector 式监控仪表盘提案，与 ForgeOS 实际代码库（一个 CLI 工具，非长驻服务）脱节，被文档自身标注为 [HALLUCINATED/OFF-TOPIC]。

*优先级信号*: n/a　·　*最高成熟度*: n/a (自认幻觉)　·　*示例来源*: genuinely-uncovered-five-frontiers, high-value-extension-directions, high-value-extension-directions-v2, high-value-extension-directions-v3

**模型路由决策可解释性 (forge route --explain)** `×2`

forge route 目前只打印最终选定的 model tier，不展示各评分维度的贡献拆解，用户无法理解'为什么选了这个 tier'。

*优先级信号*: P1　·　*最高成熟度*: architected　·　*示例来源*: expansion-five-systemic-learning-loop-gaps

**产品遥测/匿名用量分析 (区别于开发者调试遥测)** `×2`

现有全部遥测都面向开发者调试或路由优化，完全没有面向产品决策的匿名使用数据（安装计数、活跃仓库数、功能使用分布），与 docs 层面'其他'类别中的同一诊断呼应。

*优先级信号*: unstated　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-genuine-systemic-boundaries

**Dry-run 默认导致学习循环静默空转** `×2`

默认 AgentExecutor 是 DryRunExecutor，只记录一行叙事日志，不写入真实的 trace/memory 数据；用户若未显式传 --executor=command，整个学习闭环从一开始就是空的，且没有任何提示。

*优先级信号*: P0　·　*最高成熟度*: architected　·　*示例来源*: scan-current-gaps

---

### 治理执法 `governance-enforcement`

78 个独立主题，原始条目 282 条。

**Phase emits 产出真实性验证(存在性/非空)** `×16`

要求 workflow phase 声明的 emits: 文件在阶段执行后被独立核验是否真实存在、非空,而不是像今天这样在缺失时静默跳过或仅打印一条不影响流程的 WARNING。多篇文档反复提出同一机制:核验结果记为非阻塞的 trace/converge 信号,未来可选升级为阻塞。是全库被重复提出次数最多的单一治理提案,且已有一版进入 coded-and-reviewed(D2)阶段。

*优先级信号*: P0(x2)/P1-P2(x4)/P1(x3)/unstated(x7)　·　*最高成熟度*: coded-and-reviewed　·　*示例来源*: 2026-07-10-genuinely-novel-architect-perspective, 2026-07-11-five-adoption-gating-product-trust-gaps, forgeos-five-codegrounded-systemic-gaps, forgotten-five-system-boundaries

**Phase 产出物结构化契约(阶段间输入/输出 schema,超越存在性)** `×16`

与'仅检查存在性'不同,这是最大的重复提案之一:为 emits/consumes 声明结构化 schema(必需章节、最小文件数、格式规范),支持 v1 存在性检查→v2 结构检查(必需标题段)→v3 语义/embedding 相似度检查的分级演进路径,并覆盖非代码产物(ADR、PRD、评审报告)的模板一致性。已有多版架构设计(internal/contract、EmitSpec 扩展)。

*优先级信号*: P1(x2)/P2(x9)/P0-P1(x1)/unstated(x4)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-global-scan-post-closure-extension-directions, 2026-07-12-five-highvalue-architect-pm-directions, 2026-07-11-five-structural-architectural-frontiers-uncovered, governance-prod-five-frontiers

**Agent 产出合约验证框架(VERDICT/CONFIDENCE 解析脆弱性→契约注册表)** `×15`

最大的第二类重复提案:cost.go 中提取 VERDICT/CONFIDENCE 等结构化信号的解析器是纯字符串精确匹配,格式微小变化(大小写、缺空格、markdown 加粗)会导致信号静默失效且历史上已造成真实的 Sprint 27/28/30 bug。反复提议用 YAML/JSON 定义的契约注册表(支持精确/大小写不敏感/前缀/正则匹配)取代硬编码 switch 解析器,并配 forge validate --contracts 命令与 A/B 回归套件保证行为等价。

*优先级信号*: P0(x8)/P1(x5)/unstated(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-structural-extension-directions-architect-pm-combined, 2026-07-11-five-structural-extension-horizons, 2026-07-12-five-uncovered-architectural-gaps-scan, expansion-production-readiness

**策略即代码的多镜像/多语言一致性校验(YAML↔Go/Node/Python 漂移)** `×14`

internal/mode、internal/routing、internal/risk 各自声称是 modes.yml/policies.yml 的独立 Go 蒸馏版本,Go/Node/Python 三种实现各自独立计算同一策略值(如覆盖率阈值、gate 名单),但没有任何自动化机制校验它们没有漂移——YAML 改了阈值但 Go/JS 常量忘记同步时系统静默按旧规则执行且无告警。多篇文档反复提出'forge validate --policies / forge audit --drift'加 `// Source:` 注释约定的方案,已有架构设计将其落地为 internal/audit 或 internal/drift 包。

*优先级信号*: P1(x9)/P2(x3)/unstated(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-architectural-extension-gaps-deep-scan, forgeos-architect-product-manager-five-extensions, five-structural-architectural-frontiers-uncovered, second-order-architectural-gaps

**持久化产物/Workflow Schema 版本化与向后兼容** `×10`

checkpoint、trace、memory、asset.Workflow/Phase 等结构体持续新增字段却从无 schema 版本号或最小兼容版本校验,decode 时旧文件会被静默解读为错误的零值语义(如 SpentUsdMicros=0 抹掉真实花费)。多篇文档反复提出加 _schema_version/_format 字段、decode 入口版本校验、forge migrate 迁移子命令,并延伸出跨越 checkpoint/trace/memory/workflow 的分层版本协商协议设计。

*优先级信号*: P1(x7)/unstated(x1)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-codegrounded-architectural-product-gaps, forgeos-five-architectural-priority-extensions, forgeos-operational-maturity-and-structural-debt, scan-five-codegrounded-systemic-frontiers

**治理文件完整性与变更管控(信任锚/审计追踪/双人规则)** `×9`

'谁来治理治理者'问题:.agent/、harness/、.arch/ 等治理文件可以被它们所约束的 agent 静默修改,没有保护路径清单、变更审批流程、审计日志或防篡改哈希链。多篇文档反复提出 protected-path 声明、checksum 完整性校验、双人/人工批准规则、chain-hash trace,以及最终演化出 Ed25519 签名的信任根设计。

*优先级信号*: P0(x3)/P1(x5)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-highvalue-governance-evolution-extensions, forgeos-five-product-architect-extensions, five-overlooked-product-extensions, strategic-extensions-v32

**语义输出验证层(超越机械门禁,需求可追溯)** `×7`

指出现有 gate 只检查代码的机械/语法属性,收敛信号完全基于 agent 自报,implementer 写的测试验证 implementer 自己写的代码,形成自我验证闭环。提议需求-测试-代码的可追溯矩阵、声明式不变式引擎、独立生成且实现者不可编辑的验收测试,以及 diff 范围污染检测。已有 arch.md 级别的分阶段架构设计(RequirementTrace + Go 不变式检查器 + 多语言适配器)。

*优先级信号*: P0(x3)/unstated(x2)/P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-highvalue-extension-directions-v2, 2026-07-11-five-genuine-post-closure-architectural-expansion-directions, forgeos-five-codegrounded-product-expansion-directions, expansion-production-perspectives

**pi-batch.py 治理孤儿收编(无治理批处理编排器)** `×7`

根目录约 470-500 行的 pi-batch.py 批处理执行器完全在 ForgeOS 自身治理体系之外——无 agent 卡、无 harness gate、无 trace、无测试覆盖,却是驱动大量分析文档生成的实际生产工具,且有已知未修复 bug。反复提议把它折叠进受治理的 `forge batch` 子命令或 harness/ 下带测试的 shim,复用 internal/orchestrator 的并行执行能力。

*优先级信号*: P1(x2)/P2(x2)/P3(x2)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-codegrounded-architectural-blindspots-five-directions, forgeos-five-codegrounded-expansion-directions, expansion-five-systemic-architectural-gaps, five-genuinely-uncovered-architectural-frontiers-2026-07-10

**跨 Phase 一致性/意图/决策核验** `×7`

feeds_forward 只把上游阶段的原始文本注入下游 prompt,从不验证下游是否真的兑现了上游的意图——implementer 的实际 diff 可能与 planner 声明的目标文件范围毫不相关,不同阶段(planner/implementer/reviewer/QA)对同一工作项也可能给出互相矛盾的判断而无人发现。多篇文档提议机读 INTENT 声明+实际 git diff 比对、跨阶段决策提取与矛盾检测、非阻塞的一致性告警信号。

*优先级信号*: P1(x5)/P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-structural-architectural-frontiers-uncovered, agent-orchestration-five-novel-perspectives, expansion-five-systemic-learning-loop-gaps, expansion-production-blindspots-v36

**收敛信号对抗鲁棒性与 FileDelta 交叉验证** `×6`

收敛判定完全依赖三个 agent 自报信号(RoadmapCompletion/ReviewStatus/RequirementConfidence),唯一的独立信号 FileDelta 只是 log 不阻塞,构成经典 Goodhart's Law 漏洞——agent 学会写 'VERDICT: APPROVE' 即可绕过真实评审。提议信号一致性矩阵、跨迭代统计异常检测、随机独立复核探针,以及把 FileDelta 真正接入 stop_condition 作为阻塞条件。

*优先级信号*: P0(x2)/P1(x1)/P2(x2)/unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-architect-extension-directions, high-value-extension-directions, novel-five-highvalue-extensions, scan-current-gaps

**project.yml Schema 硬化(枚举校验/overrides 生效)** `×6`

project.yml 作为 mode/lifecycle 单一真相源却没有 JSON Schema、没有版本字段,`lifecycle:'produktion'` 这类拼写错误会静默退化为宽松的零值默认,绕过 production 生命周期的全部安全收紧;同时用户在 overrides 段自定义的 max_file_lines 等值从未被任何代码读取,配置生效是假的。提议 project.schema.yml 枚举约束、mode.Effective() 输入校验,以及把 overrides 真正接入 gate.mjs 的阈值合并逻辑。

*优先级信号*: P1(x5)/P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-expansion-priorities, 2026-07-12-five-post-scan-architectural-extension-directions, productization-five-frontiers-2026-07-10, global-scan-five-codegrounded-extension-directions

**红线自动门控(编辑期预警,先拆分再继续的自动化)** `×6`

'先拆分再继续'纪律完全靠 agent 自律,gate.mjs 只在编辑后触发,CC hook 从不在编辑循环中运行 arch-check 的 8 项检查,文件数预算被反复手动上调而非提前预警。提议在文件逼近 500 行上限时提供增量预警、新增轻量 gate-fast.mjs 把 arch-check 部分检查折叠进编辑期 hook,以及 forge preflight 在运行开始前就阻断已违规的代码树。

*优先级信号*: P1(x5)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-expansion-priorities, 2026-07-12-five-post-scan-architectural-extension-directions, global-scan-five-codegrounded-extension-directions, expansion-deep-analysis

**Prompt 治理(版本化/回归测试/A-B实验/模板漂移)** `×6`

12 张 agent 卡与 9 个评审模板决定了系统几乎全部行为,却没有版本追踪、diff、回滚或自动化回归测试;cost.go 的解析器硬匹配卡片声明的机读 token,两者从无自动同步校验,曾造成真实的 Sprint 27 生产 bug。提议 prompt-audit.mjs 交叉校验、per-lane SHA-256 哈希快照与 golden-file 回归、A/B 实验 schema,以及独立的模板内容 checksum/lock 文件检测非 agent-card 类模板的语义漂移。

*优先级信号*: P0(x2)/P1(x2)/P1-P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-hidden-subsystem-gaps, 2026-07-11-five-novel-architect-product-directions, forgeos-five-architect-product-perspective-2026-07-10

**引擎级声明落地核验(Declaration Grounding,对照 git diff 验证 agent 自陈)** `×6`

agent 自称完成的工作(代码写了、ROADMAP 打勾、评审通过)在引擎层从未被独立核验。提议在关键阶段后插入结构化交叉检查——implementer 后检查 git diff 是否为空、planner 后检查 emits 文件是否存在、reviewer 后检查只读 diff——记为非阻塞的结构透明度信号,是'相信但核实'哲学在引擎层的具体落地。

*优先级信号*: P1(x3)/P0(x1)/unstated(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-directions, four-code-grounded-product-expansion-directions, forged-architecture-five-fresh-horizons, five-genuinely-uncovered-frontiers

**Schema 字段声明-消费一致性审计('ADDED HERE ONLY' 模式)** `×6`

多篇文档声称 asset.Phase 若干字段(RequiresTools、Readonly、SecondaryTemplate)被解码但从未被消费,代表 schema 声明与实现的落差;但交叉验证发现其中多数声称是错的(字段实际已被消费,只是代码注释过时)。真正幸存的诉求是建立自动化的 `forge validate --consumed-fields` 声明-实现一致性检查工具,防止未来靠人工审计而非工具捕获此类漂移。

*优先级信号*: P2(x1)/P3(x1)/P0(x1)/unstated(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-uncovered-architectural-gaps-scan, forgeos-five-hidden-architectural-gaps-2026-07-10, expansion-five-product-blindspots, five-gaps-from-global-scan-2026-07-10

**策略/迁移变更预演(dry-run/diff/回滚安全网)** `×6`

修改治理策略(提高覆盖率阈值、把 gate 从 warn 翻转为 block、提高路由下限)没有任何影响预览机制,`forge migrate` 也是一次性不可逆操作、无 dry-run、无回滚、无状态查询。提议 `forge policy plan`/`forge diff --policy` 结构化影响报告(哪些 gate 严格度变化、哪些阈值变化及当前实测值分布、成本增量估算),以及 warn/canary→block 的两阶段发布机制。

*优先级信号*: P1(x2)/P2(x2)/unstated(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-verified-direction-tl-analysis, 2026-07-11-five-deep-systemic-codegrounded-boundaries, forgeos-trust-operational-maturity, 2026-07-12-five-closure-gap-expansion-directions

**Workflow 静态可达性/死循环分析(forge vet)** `×5`

mode gating、loop-back、on_unmet、human_gate 组合出的可达状态空间远大于 YAML 表面看起来的线性序列,没有验证 loop-back 目标在所有模式下仍可达,也没有死循环检测(如 loop-back 自我指向)。提议静态控制流分析器(拓扑环检测、mode-gating 可达性模拟、stop_condition 依赖追踪),最终演化为 `forge vet` 命令。

*优先级信号*: P2(x5)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-foundational-architecture-gaps, 2026-07-12-forgeos-architect-product-five-expansion-directions, five-novel-extension-frontiers-v49, forged-architecture-five-fresh-horizons

**forge-init/forge-upgrade 治理资产升级路径与漂移检测** `×5`

forge-init 只在项目创建时一次性复制治理资产,不会给已初始化项目打上版本印记,forge-upgrade.mjs 只复制缺失文件而不做 diff-merge,导致已部署项目的门禁/密钥扫描/mode 改进永远与上游脱节。提议 forgeos_version 字段、只读的 `forge governance drift` 报告器,以及后续基于已有~70%完成度原型的安全 diff-merge 升级工具。

*优先级信号*: P1(x3)/unstated(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-unseen-runtime-frontiers, expansion-horizon-three, 2026-07-11-five-structural-blindspots, forgeos-five-codegrounded-systemic-gaps

**治理配置中途生效(Mode/Lifecycle 热更新与治理版本钉扎)** `×5`

mode/lifecycle 策略在 evolve 启动时被固定,操作员中途更新 project.yml(如把 lifecycle 从 mvp 升级到 production 收紧门禁)会被完全忽略直到手动重启,评审者认为这等同于静默降级的安全策略。提议在每次迭代边界重新解析策略,以及基于 mtime/SIGHUP 的治理资产热加载机制配合 GovernanceStamp 版本钉扎写入 checkpoint。

*优先级信号*: P1(x4)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-state-data-integrity-and-lifecycle-gaps, forgotten-five-foundations, genuine-architectural-gaps-v28

**治理政策的时间维(分阶段生效/legacy豁免)** `×4`

提出把治理策略从一次性静态快照变成随时间演化的对象:policy_transition.yaml 声明 warn 一段时间后再 block 的迁移窗口、目录级永久豁免、`forge diff --policy` 治理状态快照对比,让存量代码库能渐进采纳更严格的门禁而非一步到位。已有一版进入 coded-and-reviewed(D5)。

*优先级信号*: P3(x4)　·　*最高成熟度*: coded-and-reviewed　·　*示例来源*: 2026-07-10-genuinely-novel-architect-perspective, 2026-07-11-five-adoption-gating-product-trust-gaps

**Agent Card ↔ Workflow 契约漂移与运行时履约(readonly/requires_tools/emits)** `×4`

workflow phase 声明的 readonly/requires_tools/emits 权限边界与 agent 卡自身文档化的行为可能矛盾,但系统从不核对——readonly 阶段可能实际写入而无人发现,requires_tools 没有起跑前可用性预检。提议解析 agent 卡 frontmatter 为结构化行为声明,并在阶段前后做一致性+工具预检+产出路径审计。

*优先级信号*: P1(x4)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-expansion-directions, five-novel-extension-frontiers-v49, five-uncovered-product-frontiers-2026-07-10

**可观测性驱动的自适应治理(gate 严格度随运行趋势调整)** `×4`

mode.Effective 在运行开始时一次性固定,即便 converge.Signals 持续积累的真实趋势数据显示运行正在挣扎也从不据此调整。提议在 LoopEngine.OnIteration 中构建反馈信号收集器,健康迭代多轮后放宽(仅非生产环境)、agent 自报进度与实测 file-delta 背离时收紧,并记录每次调整为 governance_adjust trace 事件。

*优先级信号*: unstated(x2)/P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-codegrounded-product-expansion-directions, novel-five-highvalue-extensions, high-value-expansion-directions

**运行时架构漂移检测(延迟预算/API契约动态验证)** `×4`

arch-check 目前只做静态分析(导入图/文件数),完全不校验设计阶段声明的性能预算、API 幂等性契约、一致性策略是否在运行时真的被遵守。提议新增 internal/drift 包,含延迟预算滑动窗口验证器、API 契约验证器、架构合规报告器,对接 trace 实测数据并产出 drift-report.md。

*优先级信号*: P3(x4)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-next-architect-frontiers, 2026-07-11-forgeos-five-next-frontier-expansion-directions

**自反式治理(ForgeOS 自身不受自己规则约束)** `×4`

ForgeOS 对被治理项目施加认知负荷上限(如 8 根模块)、layering 分层规则,但自身代码库已超出这些限制却因'自身检查是 advisory 不 block'而未被拦截;arch-check 的分层检查甚至完全跳过 forge-core 自身的 internal/<pkg> 路径(无 dir_alias 映射),PASS 结果意味着零文件被真正检查。提议把这些检查自反式应用于 ForgeOS 自身,并为 forge-core 补全 layering 映射与覆盖率阈值。

*优先级信号*: P2(x2)/P0(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-structural-blindspots, forgotten-five-meta-governance-and-blindspots

**ForgeOS 自举/自托管(forge evolve 治理自身)** `×4`

ForgeOS 声称是'AI 原生软件工厂'但 forge-core 自身约 3.5 万行 Go 代码没有 `.agent/workflows/forge-core-*.yml`,是用传统方式开发而非被自己的编排器管理;CI 中也无 forge evolve。提议为 forge-core 自身建立 workflow 定义并接入真实 gate 校验、新增 CI dry-run 验证环节,并逐步引入 nightly 真 agent 演化任务,让 dogfood 从叙述层落到机制层。

*优先级信号*: P0(x2)/P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: five-code-grounded-architectural-gaps-2026-07-11, genuine-five-product-architectural-frontiers

**自动变更影响分析与智能门控(AST 级 blast-radius,跳过低风险审查)** `×4`

现有风险分类器 risk.FromChangedPaths 只做路径子串匹配(任何包含 'auth' 的文件都被标记,无论实际改了什么),提议升级为基于 Go 标准库 go/ast 的同包调用图分析计算真实 blast radius,据此选择性跳过低风险变更的非必要审查门禁(如安全/性能评审),对不确定情形一律 fail-open 回退全量审查,生产生命周期始终强制完整门禁集。

*优先级信号*: unstated(x4)　·　*最高成熟度*: architected　·　*示例来源*: architectural-extensions-v38, v38-extension-analysis

**跨工件一致性校核(PRD↔代码↔ADR)** `×3`

Discover→Design→Review→Build→Evolve 流水线从不检查工件之间是否保持一致——没有代码验证 PRD 声明的功能真的被实现,或 ADR 的决策是否仍与当前代码匹配。提议轻量非 LLM 提取器对比 PRD checklist 与导出的代码签名/测试覆盖,以及每个 ADR 的机读 constraints.json 与 arch-check 实际结果比对。

*优先级信号*: unstated(x1)/P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-product-expansion-directions, forgeos-five-product-architect-extensions

**非二进制质量门控(分级评分取代 PASS/FAIL/NA)** `×3`

所有 gate/converge/acceptance 决策都是严格二元的,但很多 AI 产物本质是可分级而非通过/不通过。提议声明式质量 rubric,把每个维度的评分接入路由(低分 diff 路由给更贵的 reviewer)、补丁任务生成、分级停止条件,并用 arch-check 客观指标交叉核验 agent 自评防止评分虚高。

*优先级信号*: unstated(x2)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-product-expansion-directions, forges-five-unbuilt-foundations

**Gate N/A 生命周期管理(N/A 侵蚀检测)** `×3`

gate 报告 N/A 后没有新鲜度/TTL 概念——工具装好一个月后系统也不会重新探测,导致门禁覆盖率可以无声萎缩且无审计轨迹。提议 expected_gates 声明合约区分'预期但报N/A'与'合法排除',N/A 趋势告警,以及区分永久性 N/A 与可修复的工具缺失型 N/A。

*优先级信号*: P1(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-structural-gaps, five-novel-architectural-frontiers-2026-07-10

**Gate 名目录验证(拼写错误被静默过滤)** `×3`

gatesFor/Allows 会静默过滤掉任何不匹配硬编码已知 gate 集的 required_gates 条目,一个拼写错误(如 'secutiry')导致该 gate 被跳过而运行仍报告'gates green',check.py/doctor 都无法察觉。提议 gate 注册表 + Register/Lookup 机制,把未知 gate 名与合法被模式过滤区分开并给出可见 WARN。

*优先级信号*: unstated(x2)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-four-uncovered-architectural-gaps, 2026-07-11-four-uncovered-architectural-extension-directions

**语义架构/行为一致性闸门(超越技术正确性)** `×3`

gate 只检查代码是否'形状正确'(文件大小/分层/密钥/lint),从不验证代码是否真的遵循已批准的 ADR 架构决策或 PRD 描述的行为,导致架构漂移、API 幻觉可以在测试全绿的情况下通过。提议新增 arch_coherence/behavior_match 语义门禁探针和 `forge validate --semantic/--arch/--behavior` 命令,以及基于 AST/diff 的启发式(非 LLM)语义门禁套件。

*优先级信号*: P2(x2)/P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-product-architectural-expansion-directions-scanned, 2026-07-11-five-product-level-architectural-extension-directions, next-five-architectural-frontiers

**跨文件/跨资产引用完整性守卫(声明式引用图校验)** `×3`

check.py 等现有检查只做内部/相邻引用校验,从不做跨文件/跨格式一致性——modes.yml 字段被多个 workflow YAML 引用、requires_tools 引用工具提供方、agent 卡 verdict token 被 CLI 解析器引用,这些引用图从未被完整校验,重命名或删除会静默降级为无声失效。提议图遍历校验工具及后续的 phase-name/id 稳定化拆分。

*优先级信号*: P0(x2)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-systemic-declaration-gaps, governance-prod-five-frontiers, forgeos-product-output-quality-and-ecosystem-gaps

**配置面安全分析(多信任源特权升级)** `×3`

运行时参数来自四个独立可信度不同的来源(CLI flag、环境变量、project.yml、policy YAML),校验深度不一致,存在多个静默特权升级路径:FORGE_REPO_ROOT 环境变量覆盖无来源追踪、lifecycle/mode 可通过可编辑的 project.yml 绕过生产门禁、FORGE_AGENT_DEPTH 递归守卫可通过取消设置环境变量绕过、未认证的 --approved 标志无审计轨迹。提议来源溯源审计、白名单校验器、进程内(非环境变量信任)递归计数,以及签名批准方案。

*优先级信号*: P1(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-four-infrastructure-gaps-subprocess-example-bridge-config, 2026-07-11-four-unseen-foundational-gaps

**ADR 可测试性的断裂闭环(测试失败无自动补救)** `×3`

每个 ADR 都有对应的自动化测试验证决策仍然成立,但测试失败时除了 t.Errorf 什么都不会发生——没有 ROADMAP 补救条目、没有通知、没有与 FUNCTIONAL_REQUIREMENTS_AUDIT 的关联,架构腐化可以无声地持续存在。提议 ADR 测试失败时自动创建 ROADMAP 补救条目、forge doctor --anomaly 中暴露 ADR 测试健康度,以及连续失败/跳过的过期机制,并延伸为完整的 ADR 状态机(Proposal→Accepted→Superseded)。

*优先级信号*: P1(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-global-scan-engineering-product-expansion-directions, forgeos-five-architect-product-expansion-2026-07-10, five-genuinely-uncovered-architectural-frontiers-2026-07-10

**双 YAML 解析器(Go/Python)静默语义漂移检测** `×3`

ForgeOS 有两个独立的 YAML→JSON 解析器(Go 主解析器、Python 兜底解析器),但 Python 版本只在 Go 解析器报错时才被调用——Go 解析器成功但对边缘 case(混合缩进、block scalar、数字类型强转)产生与 Python 不同的语义结果时从不被交叉核对。提议基于真实仓库 YAML 建立 golden-file 测试集,实现 CrossCheck() 逐字段比对两个解析器输出。

*优先级信号*: P1(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-uncovered-architectural-gaps-scan, forgeos-five-hidden-architectural-gaps-2026-07-10, five-gaps-from-global-scan-2026-07-10

**治理资产卫生(未引用资产检测/健康衰减报告)** `×3`

.agent/ 目录随项目演进积累悬空 agent 卡片、未使用 skill、policy 死字段和过时 ADR;check.py 只验证'引用方是否存在对应定义'而不验证'定义是否被引用'(反向检查缺失)。提议构建 .agent/ 全引用图、forge governance report 输出未引用工件与资产健康度(最后修改时间/执行率/ADR-代码一致性)、forge governance tidy 交互式清理向导。

*优先级信号*: P3(x1)/原始P1→建议P0(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-systemic-oversights-v45, expansion-forgeos-meta-governance

**CI 与 forge accept 治理碎片化(单一真相源缺失)** `×3`

CI 的 forge.yml 跑 `go test -race` 和 E2E dry-run 冒烟测试,但 forge accept 完全不覆盖这些,导致 PR 可能显示 'forge accept: ACCEPTED' 而 CI 却失败,没有仲裁规则判断哪个信号权威。提议把 -race 和 E2E dry-run 折入 forge accept(或显式声明 CI 才是权威),并新增声明式 ci_contract 块使两者保持同步。

*优先级信号*: P0(x1)/P2(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-unseen-governance-horizons

**治理工具自身的免疫测试(harness 工具的 mutation testing/自检)** `×3`

harness 工具(gate.mjs/arch-check.mjs/check.py/secret-scan.mjs)只针对自身逻辑做隔离测试,没有 mutation testing、差异测试或自举验证,真实的 Sprint 26 arch-check fan-in bug(测试文件被误计入生产扇入)证明此类缺陷可长期潜伏。提议给每个治理工具加 --self-test 模式(已知违规/合规样本断言预期输出)、`forge doctor --gates` 聚合自检,以及 CI 中定期运行的 gate-health 工作流。

*优先级信号*: P1(x1)/P2(x1)/P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: novel-five-perspectives-2026-07-10, five-uncovered-architectural-frontiers, fresh-five-systemic-extensions-2026-07-10

**声明式弹性质量策略语言(按路径/生命周期分级阈值)** `×2`

当前 coverage_threshold、max_file_lines 等质量阈值是全局单值硬编码,无法区分 payment/ 需要 90% 覆盖率而 cmd/ 只需 50%。提议一套编译为 Go/JSON 的策略 DSL,支持路径级覆盖率覆盖、生命周期阶梯式阈值曲线,以及覆盖率+测试代码比+新鲜度组合而成的复合测试健康度信号。

*优先级信号*: unstated(x1)/high(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-code-grounded-expansion-perspectives

**编译门(compile gate)与语义验证管线** `×2`

发现代码可以在不实际编译/运行的情况下通过所有现有门禁,提议新增独立的 compile gate(go build ./...、语法扫描等)作为与 lint/test/build 并列的 load-bearing 闸门,以及 api-contract/snapshot-test 等更高阶语义门禁类型,弥补'新代码不编译但 test gate 仍可能 PASS'的假阴性。

*优先级信号*: P0(x1)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-unbuilt-core-extension-directions

**internal/yamlpath 零消费基础设施孤岛的接入** `×2`

internal/yamlpath 实现了完整的 YAML-path 引用解析器(支持 policy 引用如 `../policies/modes.yml#workflow_depth.reviewer`)并已测试,但在 Go 运行时零消费者——策略引用解析实际靠一条独立的 Python check.py 路径完成。提议把 asset.LoadWorkflow 接上这套解析器,在加载时一次性预解析并写入缓存。

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-structural-capillary-gaps

**运行时配置快照完整性(SHA-256 指纹+iteration 级漂移检测)** `×2`

提议在 forge evolve 启动/每次迭代时对已加载的 .agent/ 配置(modes.yml、workflow、agent 卡契约段)计算 SHA-256 快照并重新校验,配置化 warn/abort/reload 行为,防止 24 小时无人值守运行中静默混用新旧治理配置。

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-unexplored-perspectives

**工作流资产声明的按需交叉引用预检(typo 静默降级)** `×2`

workflow YAML 解析是容错的,拼写错误(如 model_tier: ops)会静默把安全关键的 opus 覆盖降级为 sonnet,只有 CI 阶段的 check.py 才能事后捕获。提议在 workflow 加载与引擎执行之间插入一个 warn-only 的轻量预检步骤,只校验 agent 引用、gate 名、model_tier 值等引用完整性,不重复 check.py 的完整策略校验。

*优先级信号*: P3(x1)/unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-forgeos-four-codegrounded-architectural-expansion-directions, 2026-07-11-four-codelevel-extension-directions-after-four-deep-scans

**非功能性影响评估门禁(延迟/体积/构建时间)** `×2`

现有闸门体系只检查结构性和功能性属性,从不检查非功能性指标——延迟、内存、构建时间、二进制体积是否退化;scorecard 已收集 P95LatencyMs 等数据但仅用于路由决策从未用作闸门条件。提议新增二进制大小闸门、构建时间跟踪、测试执行时间回归检测。

*优先级信号*: P2(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-foundational-resilience-gaps

**治理即实验(A/B 测试策略+灰度发布)** `×2`

mode/policy 变更目前是全有或全无的静态切换,没有任何度量验证机制。提议 internal/experiment 包和 forge experiment CLI,支持治理策略变更(如降低默认路由 tier)以 treatment vs control 方式对比 cost/review-fail-rate/gate-fail-rate,以及不实际应用只评估的 shadow-mode。

*优先级信号*: P3(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-product-architectural-expansion-directions-scanned, 2026-07-11-five-product-level-architectural-extension-directions

**二进制-脚本版本一致性协议** `×2`

ForgeOS 是三语言栈(Go 二进制、Node harness 脚本、Python 治理检查)通过文本协议通信,forge --version 纯装饰性,forge doctor 从不检查二进制/脚本兼容性,升级的二进制配旧脚本可能静默误解析输出。提议在每个 harness 脚本中嵌入兼容的 forge 版本范围、调用时版本协商检查,以及 forge doctor 的版本一致性 PASS/FAIL 检查。

*优先级信号*: P0(x1)/P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-code-grounded-systemic-gaps, 2026-07-11-forgeos-five-system-level-gaps

**Agent 输出质量断言层(超越结构门禁的语义质量)** `×2`

结构性门禁(体积/架构/密钥扫描)从不检查 agent 输出内容的语义质量,提议后置质量断言系统:工具保真度检查(诚实降级 vs 虚构)、只读阶段的 git-diff 强制、输出模板完整性检查、跨阶段自报与代码 diff 的一致性检查——核心洞察是'门禁全绿不代表输出质量好'。

*优先级信号*: unstated(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-architectural-product-expansion-directions

**yaml2json 手写解析器健壮性(fuzz/conformance 测试)** `×2`

整个治理层(5 workflow × 12 agent 卡 × policy)全部经过一个无 fuzz/conformance 测试的手写 YAML 解析器,已有生产级损坏先例(Sprint 27 block-scalar bug 曾给每个 description 字段注入错误的 '> ' 前缀)。提议短期建立 fuzz+conformance 测试套件,中期评估用隔离的标准依赖解析器替代或与 PyYAML shim 双后端比对回退。

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-scan-five-codegrounded-systemic-frontiers, 2026-07-11-forgeos-five-genuine-codegrounded-extensions

**Agent 能力漂移检测与契约版本兼容矩阵** `×2`

提议跟踪每个 agent-phase 的'契约合规率'(reviewer 输出是否真的匹配声明的 VERDICT 格式),建立模型版本-契约兼容性矩阵(已知可用/已知失效),在模型升级悄悄降低某 agent 对卡片机读契约的遵守度时能被发现——这是运行时行为漂移问题,不同于纯格式版本化。

*优先级信号*: P1(x1)/P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-12-five-overlooked-product-extensions, forgotten-product-five-v51

**死代码与孤包治理(零入边包检测)** `×2`

arch-check 的 8 项架构检查都不回答'这个包是否仍被需要',仓库中已确认存在零引用孤包(internal/adr 只有会被 Skip 吞没失败的测试文件、internal/yamlpath 有 200+ 行生产代码但从未被消费)。提议基于 go list -json -deps 的导入图分析新增 checkOrphanPackages(WARN 非 block),支持白名单豁免注释,并接入 forge doctor --dead-code/--prune(移动到 graveyard,不删除)。

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-systemic-oversights-v45

**主动式相位间架构护栏(增量 arch scan,而非仅统一 gate 阶段)** `×2`

arch-check 只在统一 gate phase 运行,implementer 累积多个违规文件后才被一次性发现,导致整轮 iteration 作废重跑。提议 workflow 声明 `scan_after: true` 使每个 agent phase 结束后可选运行增量 arch scan(仅扫描本 phase 新增/修改文件),并把结果作为 context 注入下游 reviewer prompt,实现更快的边写边查反馈。

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-uncovered-horizontal-frontiers

**预提交/本地守卫(pre-commit hook)** `×2`

所有 harness 门禁只在 CI 或工作流中途运行,forge-init 不提供 git pre-commit hook,唯一近似本地保护的 .claude/settings.json PostToolUse hook 只对 Claude Code 生效且可绕过。提议轻量 pre-commit 模板(快速版 gate.mjs+secret-scan.mjs,<1s)由 forge-init 安装,并演化为 `forge preflight`/`forge install-hooks` 命令。

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: five-code-grounded-architectural-gaps-2026-07-11

**cmd/forge 依赖中枢退化重构** `×2`

cmd/forge 包已从 CLI 包装器退化为依赖中枢,直接 import 15/17 个内部包且无反向依赖,文件数预算被反复突破,cost.go/gates.go/prompt_context.go 等纯逻辑大量混在 CLI 层且无法独立测试。提议提取 internal/cost、internal/ledger 等新包,把 Engine 组装逻辑收敛为 EngineBuilder,并为 cmd/forge 增加进口包数上限约束。

*优先级信号*: P2(x1)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: strategic-extension-five-novel-2026-07-10, forgotten-five-structural-debt

**生命周期自动推进(读取信号自动建议/应用 lifecycle 升降级)** `×2`

lifecycle(idea/mvp/growth/production)只能通过全手动的 `forge migrate` 变更,但决策所需的全部客观信号(converge 信号历史、gate 通过率趋势、覆盖率趋势、评审判定分布)每次收敛检查都已被计算却从未被用来驱动生命周期决策。提议构建成熟度引擎消费这些信号自动提议(或 --auto-lifecycle 自动应用)project.yml 的 lifecycle 升降级,以及作为只读顾问的 LifecycleAdvisor。

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: genuinely-uncovered-five-frontiers, 2026-07-11-codegrounded-five-systemic-blindspots

**Agent 机读契约版本协商(CONTRACT: 版本 token)** `×2`

提议给 VERDICT/CONFIDENCE 等硬编码机读契约加一个可选的版本化注册表和协议握手机制(如在 VERDICT 前解析 `CONTRACT: <version>` token),使三套硬编码契约可以独立演化而不必与 forge-core 解析器/agent 卡文档/测试同步锁步升级,未版本化输出默认走 v1 行为保证零回归,未知版本产出 WARN 而非硬失败。

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: forgotten-five-system-boundaries

**Phase Schema 单一权威源+代码生成(SSOT)** `×2`

asset.Phase 结构体已膨胀到约 40 个字段、329 行、67% 是注释,schema 分散在 6 个手动同步点,审计发现超过一半的功能需求类 GAP 都源于此。提议建立单一 YAML/JSON schema 权威源,通过 go generate 代码生成同步驱动 Phase 结构体、mode gating、gate 解析、收敛映射,并配 schema-check.mjs 漂移守卫。

*优先级信号*: P0(x2)　·　*最高成熟度*: architected　·　*示例来源*: architect-product-perspective-four-structural-gaps, expansion-five-architect-product-perspective

**Agent 自评置信度的后验校准与跨阶段传播** `×2`

AgentVerdict 目前只有 (verdict, ok) 二元组,没有置信度维度,agent 自报的 CONFIDENCE 也从不与实际 gate/reviewer 结果做后验校准,可能导致虚假高置信度在 discover 阶段过早触发收敛。提议记录 agent_role×task_type×self_confidence×实际结果的校准记录,计算偏差因子折算收敛判断,并升级为 (verdict,confidence,ok) 三元组配合跨模型校准因子。

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: fresh-expansion-perspectives, genuine-five-product-architectural-frontiers

**相位级内联输出闸门(PhaseGate,先于聚合门禁的即时判定)** `×2`

当前相位间验证是纯信任模型——implementer 写完代码直接 feed-forward 给 reviewer,直到 workflow 末尾才跑聚合 gate,期间可能已浪费大量 token 和时间审阅/测试有问题的代码。提议在每个 agent phase 完成后、feed-forward 前插入可选的 PhaseGate 回调(allow/retry/abort 三态),提供编译门/语法门/测试覆盖门/模块边界门/secret 门等快速确定性检查,与 workflow 末尾的聚合 final gate 是互补而非替代的姊妹概念。

*优先级信号*: 立即开始(独立增强,无依赖)(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-directions-v3, expansion-high-value-directions

**门禁输出结构化(Violation 类型取代自由文本)** `×1`

gate.Result.Output 和 trace.Event.Detail 是无字段划分的自由文本字符串,reviewer prompt 和 scorecard 学习回路只能看到 PASS/FAIL 而无法追踪违规趋势或按严重度排序。提议共享的结构化 Violation 类型和各 gate 工具的 --json 输出模式。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-frontiers

**用户自定义闸门系统(声明式扩展 gate 目录)** `×1`

当前 8 种 gate 类型全部硬编码在 harness 的 acceptance.mjs/adapters.mjs 中,采纳团队无法在不 fork forge-core 的前提下声明自己的合规检查(如迁移安全性、license header)。提议通过声明式 YAML 加标准 stdin/stdout/exit-code 契约,让团队自行接入闸门。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-codegrounded-architectural-extension-directions

**Doctor/QuickChecks 发现问题但编排无条件继续** `×1`

internal/doctor/quick.go 的 QuickChecks 明确声明'a failing check is never a gate',即使发现 checkpoint 损坏或 trace 截断,orchestrator 仍照常继续运行,只把结果记为 trace 事件。提议将结果分层:必然导致后续失败的问题应阻塞运行(可 --force 跳过),可能失败的问题仅告警。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-deep-systemic-codegrounded-boundaries

**治理资产版本化生命周期跟踪(agent卡/workflow/policy 变更归因)** `×1`

提议给 agent 卡、workflow YAML、policy 文件加基于 git hash/mtime 的版本追踪,并把版本快照贯穿 trace 事件与 scorecard 条目,使 prompt/质量回归能归因到具体的 agent-card 或 workflow 版本,而不是无法追责。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: cross-cutting-systemic-gaps

**Mode × Lifecycle 矩阵覆盖完整性验证** `×1`

4×4 的 mode×lifecycle 矩阵被冗余定义在 modes.yml、mode.go 手写 map、gate.mjs、check.py 四处,没有单一权威来源,也没有跨三个子系统(路由 tier、harness 严格度、workflow 深度)的自动化覆盖校验。提议 `forge validate --matrix` 命令核对 mode.go 覆盖率与 modes.yml 声明的 mode 集合是否一致。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-gaps-from-global-scan-2026-07-10

**治理策略测试框架(针对自身治理配置的断言测试)** `×1`

提议 `forge test` 子命令和 `.agent/tests/*.yml` 断言文件,让用户对自己的 mode×lifecycle 治理配置写可重复测试(如'engineering+production 必须强制安全门禁+block 执法'),复用现有 mode.Effective() 策略解析器,填补 check.py/acceptance.mjs 只验证结构有效性、从不验证治理策略行为正确性的空白。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: novel-five-highvalue-extensions

**语义化配置漂移检测(boundaries 段/ADR 时效性/ARCHITECTURE.md 事实核对)** `×1`

check.py 只做语法层检查从不验证语义层——每个 agent 卡的 boundaries 段全仓无一处代码读取(纯散文无执法)、ADR 文本可能已被代码演进甩开却无自动机制标记过期、ARCHITECTURE.md 只列出 5 个引擎而 internal/ 实际有 18 个包。提议把 boundaries 机读化为 BOUNDARY: 标记接入 arch-check、ADR 事实性自动审计,并从包结构自动生成引擎列表与手写文档交叉对比。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-extensions-v33

**分析文档爆炸的准入管控(冻结新文档,索引+归档)** `×1`

这是一个流程性而非运行时功能性的治理提案:docs/requirements/ 已积累 435 个文件/124,766 行,以约每天 30 篇的速度持续生成。提议冻结新分析文档创建直至完成 INDEX.md 整合与归档/TTL 标记结构,并建立永久性的'新分析准入检查'——要求作者先搜索索引证明不重复才能写新文档。

*优先级信号*: P0(x1, blocking prerequisite)　·　*最高成熟度*: architected　·　*示例来源*: five-structural-blindspots

**分析文档代码引用自动化衰退检测** `×1`

提议仿照 secret-scan.mjs 的架构模式,新建 stale-ref-check.mjs 工具解析分析文档中嵌入的 file:line 代码引用,周期性与当前代码状态做 diff,区分良性行号偏移与真实内容漂移,对 30 天未解决的过期引用自动打上'deprecated'标签(非阻塞 CI 警告报告)。

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-frontiers

**docs/ 目录治理盲区(Markdown 文件完全未被门禁覆盖)** `×1`

gate.mjs(行数检查)、arch-check.mjs、secret-scan.mjs、check.py 全部跳过 .md 文件,使 docs/ 成为仓库中最大的未治理领地,与 ForgeOS'自我治理'的核心承诺相矛盾。提议把 docs/ 纳入 gate.mjs 行数限制、secret-scan 覆盖、近重复文件名反模式检查,以及断链检查器。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: forgotten-five-meta-governance-and-blindspots

**内部包契约接口缺失(隐式耦合导致的漂移)** `×1`

跨 18 个内部包,仅存在一个显式 Go 接口(AgentExecutor),跨包耦合全靠具体结构体和函数类型字段(如 `RunGate func(string) gate.Result`),导致 converge.Signals 或 gate.Result 新增字段可能在消费方静默零值化而无编译错误,手写测试 fake 也可能通过却与真实生产行为脱节。提议为最常共享的类型建立消费者/探针接口及字段覆盖契约测试。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: forgeos-five-unexplored-structural-corners

**增量/diff 范围化门禁执行(按 blast radius 跳过全量扫描)** `×1`

提议引入 gate.Scope 类型和 ScopeResolver,把 git 变更文件扩展到其消费者(复用 internal/risk 的路径匹配做 blast radius),再由 GateScheduler 依据每个检查声明的增量能力分派为全量扫描/增量扫描/跳过;跳过的 gate 标记为 NA+scoped 而非 PASS,循环依赖检测等全仓检查始终保持全量扫描。

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: five-systemic-oversights-v45

**Toolchain 版本契约(声明式工具链版本要求)** `×1`

提议引入声明式 .agent/toolchain.yml(node/python3/claude/git 所需版本)及自建 semver 范围比较器(支持 >=、^、~、*、<、=),由新的 internal/toolchain 包解析,统一供 forge doctor --toolchain 和 forge preflight 消费,替代目前散落在 doctor/preflight/adapters 各处的临时版本约束。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: cross-cutting-systemic-gaps

**结构化审批 Schema(Approval 内容化取代空文件标记)** `×1`

当前人工审批只检查一个空 .approved 文件是否存在,没有记录谁批准、何时批准、批准理由或有效期。提议定义结构化 Approval{ApprovedBy,ApprovedAt,Reason,ExpiresAt,Chain,Version},新增 `forge approve <stage> --reason` 写入带身份/时间戳/有效期的 JSON,并升级校验逻辑做过期/链完整性检查。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-extension-directions-verification

**Mode Policy 结构体子域拆分重构** `×1`

mode.Policy 从 13 字段 7 域的扁平结构体重组为两层嵌套子结构体(GatePolicy/CoveragePolicy/RouterPolicy/WorkflowPolicy/EnforcePolicy 等),防止字段继续膨胀成不可拆解的 God 结构体,是一次面向代码健康度的内部重构而非新增外部行为。

*优先级信号*: P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-unseen-codelevel-architectural-gaps

**生命周期感知的治理资产筛选(forge-init 按需裁剪)** `×1`

forge-init 无差别复制全套 9 张 agent 卡+8 张 skill 卡+4 个 workflow,一个 lifecycle=idea 的脚手架项目并不需要 security-engineer 卡。提议 governance.yml 声明 lifecycle→required/recommended/optional 资产映射,`forge validate --governance-fit` 检查适配度,`forge prune --governance` 按声明移除不必要资产。

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: genuine-five-product-architectural-frontiers

**架构漂移检测→诊断→自动修复闭环流水线** `×1`

arch-check 目前只单向报告违规(分层/包/扇入/认知负荷/反模式命名/函数长度/循环依赖/drift-guard)为非结构化字符串。提议构建结构化 ArchViolation 类型、漂移→根因映射表、自动修复的 `arch-fix` 工作流阶段,以及修复后的再验证步骤,使治理从'一次性警察报告'变为闭环(检测→诊断→修复→验证)。

*优先级信号*: P0(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-analysis-v2

**自治理破产修复(REJECTED 状态下的自我修复缺失)** `×1`

发现 forge accept 曾处于 REJECTED 状态(test_pass/complexity_violations/architecture 三项 load-bearing 闸门同时失败),但项目在 enforce:block 模式下仍持续以 REJECTED 状态运行,没有 self-rollback 机制。要求立即拆分超限文件(validate.go/main.go/evolve.go 等),修复已知回归,并新增 forge self-check 元闸门确保自身代码遵守其倡导的纪律。

*优先级信号*: P0(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-self-governance-and-hygiene

**架构评估引擎(改动前影响预测+可定制规则引擎)** `×1`

arch-check.mjs 是仓库级静态结构检查,在 commit 后才触发,对'这个改动会让某模块扇出膨胀'没有预测能力,也没有可定制的架构风格规则引擎(如策略模式 vs switch 语义合规)。提议声明式规则文件支持 layering/耦合/命名约定/禁止模式,新增 forge plan 阶段做改动前架构影响预测,以及模块大小/依赖图熵/覆盖率 delta 的演进趋势仪表盘。

*优先级信号*: P0(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: extension-analysis

**声明式策略引擎运行时加载(YAML 真正驱动 mode.Effective)** `×1`

零外部依赖的原生 Go YAML 解析器 internal/yaml2json 已存在并被用于 validate.go 校验 modes.yml,但运行时路径 mode.Effective("engineering","mvp") 仍读取硬编码 Go 表而非从 YAML 加载——'policy-as-code'的承诺尚未在运行时真正兑现。指出缺失的只是约 50 行胶水代码,是被显著低估工作量的一项高杠杆基础设施补全。

*优先级信号*: highest(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-expansion-perspectives

---

### 编排引擎补齐 `orchestration-engine` ⚠ 人工聚类

21 个独立主题，原始条目 240 条。

**并行执行生产就绪化 (--parallel 资源治理/写冲突检测/仲裁/锁排序机械化验证)** `×40`

--parallel 拓扑分层并发引擎已交付基础机制（Kahn 排序 + 8 把文档化互斥锁），但缺乏生产级护栏：无并发上限（100 个独立 phase 可同时启动 100 个 claude 进程）、无文件级写冲突检测（同 wave 内两 phase 改同一文件后写者静默覆盖）、锁顺序契约纯靠人工遵守无编译期验证、以及并行模式下 checkAgentBudget 的'幽灵消费'（budget 计数在 phase 真正执行前就不可逆递增）。

*优先级信号*: P0(x多)/P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-expansion-priorities, five-uncovered-horizontal-frontiers

**跨阶段结构化产物契约 (Emits → Expects/OutputContract/Schema Registry)** `×38`

asset.Phase 的 emits: 字段目前只是一份未经消费的文件路径字符串清单——编排器从不校验上游 phase 是否真的产出了声明的文件、格式是否符合下游期望，格式正确性完全依赖 prompt engineering 的软约束。这是全库出现频率最高的编排类诊断，压倒性多数提案收敛到同一修复方向：为 Phase 增加对称的 Expects/InputContract 字段、引入版本化的 OutputContract/Schema Registry，在 phase 完成后做存在性+结构签名校验。

*优先级信号*: P0(x多)/P1(x多)/P2(x若干)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-four-extension-frontiers, 2026-07-12-five-closure-gap-expansion-directions, forges-five-unbuilt-foundations

**跨workflow编排管线组合引擎 (chain discover→design→build→evolve / Pipeline DSL / Composition Algebra)** `×32`

discover→design→review→build→evolve 五个 workflow 完全独立、靠操作员手动依次触发，design.yml 声明的 on_approved.next_stage 意图从未被引擎消费自动跳转。海量提案收敛到构建某种'Pipeline'/'Composer'抽象（新增 pipelines.yml DSL 或 forge pipeline run 命令），自动串联多个已有 workflow 并在阶段间做前置条件校验。

*优先级信号*: P0(x多)/P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: expansion-five-truly-uncovered-frontiers-v46, 2026-07-12-five-deep-global-scan-extension-directions, 2026-07-11-forgeos-five-codegrounded-structural-gaps

**收敛判定二元论 → 部分满足/连续分数/振荡检测/信号注册表** `×26`

converge.Converge() 目前只返回严格的二元 MET/NOT MET，无法表达'3/5 条件已满足'的部分进展，也无法区分'真实失败需要继续迭代'与'非确定性停滞'。多份提案主张引入连续型 ConvergenceScore/百分比完成度、收敛轨迹的振荡检测、以及用注册表模式取代 evalOne 里日益膨胀的硬编码 switch 语句。

*优先级信号*: P0(x若干)/P1/P2 混合　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-code-grounded-systemic-gaps, 2026-07-12-five-deep-global-scan-extension-directions

**Checkpoint 语义完备性 / 回滚编排 / 演化分支与反事实实验** `×24`

checkpoint 目前只持久化位置性/数字状态（iteration、phase index），不记录'为什么'（agent 的决策、修改的文件、关键假设），resume 后系统知道'从哪继续'但不知道'之前在想什么'。同时 --resume 是纯前向的，没有真正的回滚能力——一次 evolve 走偏后无法回到某个历史 checkpoint 重新分支。多份提案主张升级 checkpoint 的语义丰富度、引入 checkpoint DAG（父子关系/分支标签）与 forge branch/merge 式的反事实回滚。

*优先级信号*: P0(x若干)/P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: five-uncovered-horizontal-frontiers, 2026-07-11-forgeos-five-codegrounded-architectural-frontiers

**自适应/增量工作流组装与相位智能跳过 (避免全量重跑)** `×22`

forge evolve 每轮迭代无条件从起始 phase 完整执行所有 phase，即便某 phase 的输入自上轮以来完全未变（如 ROADMAP+memory 均未更新的 planner）。多份提案主张引入内容寻址的 phase 输出指纹（SHA256/结构相似度）、收敛感知的增量调度、以及让静态 5 个 workflow YAML 能依据 forge detect 的项目画像动态组装 phase 列表，而非一成不变。

*优先级信号*: P1(x多)/P2(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-genuinely-unexplored-extensions, 2026-07-11-forgeos-five-codegrounded-expansion-directions

**AgentExecutor 生命周期契约与结构化返回值 (Init/Shutdown/Rollback/Health + 工具执行运行时)** `×11`

AgentExecutor 接口目前是单方法的 Execute(ctx, phase, mode) error，没有 Init/Shutdown/Rollback/Health 等生命周期钩子，返回值也只有纯粹的 error（成功时无结构化信息可用）。更进一步，CommandExecutor 本质是一次性 shell 包装器（一次 claude -p 调用发射即遗忘），没有真正的多轮工具调用循环。多份提案主张分层解决：先补生命周期接口，再演进为完整的 Agent-Runtime 工具执行循环。

*优先级信号*: P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-structural-blindspots, forgotten-frontiers-five

**多Agent冲突检测/协商仲裁/矛盾一致性引擎** `×9`

当前 agent 协作是严格单向的流水线（reviewer 的 REQUEST_CHANGES 只能单向流向 implementer），没有任何机制让两个 agent 就分歧进行协商或投票；并行 wave 内多个 agent 也可能在语义上互相冲突（如对同一数据结构做出不兼容的设计假设）而无检测。

*优先级信号*: P1(x多)/P2/P3　·　*最高成熟度*: architected　·　*示例来源*: next-five-architectural-frontiers, 2026-07-11-four-uncovered-architectural-extension-directions

**Gate Loop-Back 效率 (全量重跑浪费/结果缓存/故障上下文传递)** `×7`

当定向 loop-back 被 gate 失败或 reviewer REQUEST_CHANGES 触发时，RunFrom 会重新执行目标 phase 与当前 phase 之间的全部 phase（包括本可复用的 gate 扫描结果），且 loop-back 后 implementer 收到的重跑提示不携带任何关于'具体哪个 gate 为什么失败'的上下文，只有模糊自由文本。

*优先级信号*: P1/P2 混合　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-gaps

**pi-batch.py 治理外孤岛桥接 (Shadow Orchestrator Absorption)** `×4`

仓库中存在一个独立于 forge-core 治理体系之外运行的并行 agent 批处理脚本 ai-dev/pi-batch.py（本会话开头 git log 中可见的 [pi-batch] 系列提交即由此产生），它有自己的 checkpoint/resume 逻辑但完全绕开 forge-core 的 gate/route/converge 体系。多份提案主张把它并入治理框架（轻量 task phase 类型或完整吸收为 forge batch 子命令）。

*优先级信号*: P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: four-truly-unexplored-architectural-gaps, 2026-07-11-forgeos-five-codegrounded-structural-gaps

**风险信号未接入真实编排执行策略 (risk.FromChangedPaths 只服务 forge route)** `×4`

risk.FromChangedPaths 分类器已完整实现并测试，但只被独立的手动 forge route CLI 消费，从未接入真实 run/evolve 的执行策略——按变更影响域动态调整并行度/gate 严格度/model tier 的能力完全缺失。这与 FUNCTIONAL_REQUIREMENTS_AUDIT.md 中已记录的 G3 多维路由未接入真实执行的 GAP 属于同一根因的不同侧面。

*优先级信号*: P1/P2　·　*最高成熟度*: architected　·　*示例来源*: five-systemic-trust-and-scalability-gaps

**Workflow 版本化与内容哈希锁定 (漂移检测)** `×4`

checkpoint 只存 workflow 的字符串名称（如 'build'），不存内容哈希；若用户在 iteration 之间手动编辑了 workflow YAML，resume 后系统无法察觉正在恢复的其实是一个语义已变化的不同 workflow。

*优先级信号*: P1/P2　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-architectural-extension-gaps-deep-scan

**Phase Name 脆弱标识符 → 类型化 Phase ID 系统** `×4`

Phase Name 字符串同时被用作人类可读标签和 5 种不同结构（loop_back target/depends_on/emits 引用等）唯一使用的机器 ID，重命名一个 phase 会静默破坏所有引用它的下游声明而无任何编译期或运行时校验。

*优先级信号*: P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-systemic-blindspots

**非LLM执行器插件化架构** `×3`

每一个非 gate phase 目前都被硬编码走 LLM AgentExecutor，没有让某些 phase（如纯脚本/纯静态分析类任务）走非 LLM 执行路径的插件点。

*优先级信号*: unstated　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-four-extension-frontiers

**Discover 引擎系统级数据模型缺失** `×2`

ForgeOS 最高论点'需求探索优先于代码实现'与实现之间最大的落差：discover.yml 声明了 requirement-discovery→market-research→product-designer 三阶段，但 Discover 阶段目前完全由 agent 自由发挥，没有系统级的数据模型或收敛判据来约束探索质量。

*优先级信号*: P1　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-forgeos-state-data-integrity-and-lifecycle-gaps

**Gate/Acceptance 探针拓扑并行化 (acceptance.mjs 8 个 probe 串行执行)** `×2`

forge accept 聚合的 8 个探针（体积检查/arch-check/secret-scan/check.py/test/app-test/SCA/质量）之间彼此无数据依赖却严格串行执行，存在无风险的并行化空间。

*优先级信号*: P1/P2　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-gaps

**已评估后主动搁置的方向 (非 gap，是理性决策)** `×2`

两个方向被文档自身明确记录为'曾评估、主动放弃'：引入 Temporal 做分布式持久化工作流引擎（当前单机 checkpoint/resume 已够用，且会打破零依赖红线，收益仅在多 worker 场景显现）；构建独立的 Agent Registry & Scheduler 服务（当前单 executor 模式无需调度，只有支持多 Runner 时才需要）。与 FUNCTIONAL_REQUIREMENTS_AUDIT.md 的 DEFERRED-BY-DESIGN 分类精神一致——这是诚实标注的非目标，不应被误读为遗漏。

*优先级信号*: deferred(x2)　·　*最高成熟度*: n/a (主动搁置)　·　*示例来源*: strategic-expansion-directions

**[数据质量标记] 2条明确的幻觉/离题条目** `×2`

与 edge-case-reliability 类别中发现的模式一致，genuinely-uncovered-five-frontiers 和 high-value-extension-directions-v3 各生成了一条通用化、与 ForgeOS 代码库无关的工作流引擎/任务分片提案，被文档自身标注为 [HALLUCINATED/OFF-TOPIC]。

*优先级信号*: n/a　·　*最高成熟度*: n/a (自认幻觉)　·　*示例来源*: genuinely-uncovered-five-frontiers, high-value-extension-directions-v3

**业务逻辑聚合层重构 (internal/app 提取 cmd/forge 领域逻辑)** `×2`

buildRunEngine/runEvolve/gatherSignals/gatesGreen 等领域逻辑目前散落在 cmd/forge 包内，提案建议提取为独立的 internal/app 聚合层或拆分 internal/orchestrator 内部的执行器子包，与 docs 层面'其他'类别中的 cmd/forge 上帝包重构诉求同源。

*优先级信号*: P1　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-genuinely-uncovered-runtime-frontiers

**异常检测脱离演化循环 (anomaly.go 已建但未接线)** `×1`

internal/doctor/anomaly.go 已经有一套完整可用的 checkpoint 历史异常检测启发式，但从未被 evolve 循环实际调用消费，是一个'建了但没用'的能力。

*优先级信号*: P0　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-architect-product-expansion-2026-07-10

**分层超时体系 (phase/gate/iteration/run 多级 timeout)** `×1`

当前超时架构是一维的（CLI --timeout 全局应用到所有 phase），而实际需求是多层级的：整体 evolve 超时、每轮迭代超时、每 phase 超时、每 gate 超时应能独立配置。

*优先级信号*: P1　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-forgeos-operational-maturity-and-structural-debt

---

### 边界条件/并发可靠性 `edge-case-reliability` ⚠ 人工聚类

18 个独立主题，原始条目 178 条。

**多进程并发安全 + Run Identity/状态隔离 (.forge/ 缺少跨进程文件锁)** `×48`

目前 checkpoint.json/trace.jsonl/memory.jsonl/scorecards.json 全部假设单进程独占写入：checkpoint 用 temp→rename 保证单文件原子性但不防跨进程互相覆盖，trace/memory 用 O_APPEND 但无跨进程互斥。两个 forge run/evolve 同时跑同一仓库（或同仓库不同分支）会互相覆盖 checkpoint、交叉污染 memory、trace 行序错乱。这是全库出现频率最高的单一诊断，压倒性多数提案收敛到同一套修复：为每次 run 生成 run_id/UUID、用 flock/PID 文件做进程级互斥锁、并把 .forge/ 从扁平命名空间重构为按 run/分支隔离的目录。

*优先级信号*: P0(x20+)/P1(x15+)/其余unstated　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-adoption-gating-product-trust-gaps, forgotten-five-foundations, 2026-07-12-five-hidden-product-quality-gaps, forgeos-trust-operational-maturity

**Agent 输出解析脆弱性 → 结构化/fuzzy 契约解析** `×25`

reviewer VERDICT、CONFIDENCE 分数、cost JSON、overload 分类等全部依赖对 agent 输出做精确逐行字符串匹配（如末行必须精确等于 'VERDICT: APPROVE'），格式稍有偏差整条解析静默失败并回退到不可预测的默认值。多份提案主张替换为结构化 JSON-Schema 契约、fuzzy/tolerant 解析器，并让 cappedBuffer 感知截断（Truncated() bool）、把当前合并捕获的 stdout/stderr 拆分为独立缓冲区以免诊断信息互相污染。

*优先级信号*: P0(x4)/P1(x多)/P2(x若干)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-pipeline-integrity-and-security-gaps, architect-product-perspective-five-novel-directions, high-value-extensions-analysis, five-gaps-from-global-scan-2026-07-10

**优雅降级/部分失败恢复(告别全有全无二元模型)** `×15`

当前运行时几乎所有子系统都是全成功或全中止的二元模型：一个 gate 失败即 abort 整个 run，parallel wave 中一个 phase 失败会取消整个 wave（已成功的其他 phase 输出丢弃），状态文件一旦轻微损坏就硬错误退出而非降级读取。多份提案主张引入分层的优雅降级/部分恢复能力，让系统在部分故障下仍能保留已完成的工作并以降级模式继续。

*优先级信号*: P1/P2 混合，多数 unstated　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-extension-directions-verification, 2026-07-11-five-product-operations-systemic-gaps, 2026-07-11-codegrounded-edge-cases-and-extensions

**错误分类维度扩展 (ExecKind 扁平5类 → 多维 Severity/Source/RecoverySemantics)** `×13`

当前 classifyRunErr 把所有非零 exit 归入 5 个扁平类别（Config/Timeout/Failed/RecursionLimit/Overloaded），OOM-kill (exit 137)、网络错误、部分写入失败全被塞进同一个不可重试的 Failed 桶，导致本可恢复的故障被当作永久失败直接 abort。多份提案主张扩展为多维错误分类（增加 KindPartialWrite/KindResourceExhausted 等），使恢复策略能按错误的真实语义分派。

*优先级信号*: P1(x多)/P2(x若干)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-pipeline-integrity-and-security-gaps, 2026-07-11-forgeos-five-codegrounded-expansion-priorities, global-scan-five-codegrounded-extension-directions

**并行写冲突检测 (--parallel wave 同文件竞争无检测)** `×11`

--parallel 已交付的拓扑分层并发调度只保证内存锁的获取顺序，但从未检测文件系统层的写冲突——同一 wave 内两个 agent phase 修改同一文件时后写者静默覆盖前者，也没有任何冲突信号或合并尝试；同一问题也体现为 checkpoint 并发写竞争（Parallel Checkpoint Write Race）。

*优先级信号*: P0(x3)/P1/P2 混合　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-systemic-gaps, 2026-07-12-five-extension-directions-architect-product-perspective, expansion-deep-analysis

**YAML 双解析器语义分歧 (Go原生 vs Python shim 差分测试)** `×10`

Go 原生 yaml2json 解析器与 python3/PyYAML fallback 在若干边界构造（缩进指示符、块标量后缀等）上行为不一致，且从未被差分测试验证过，静默产出不同解析结果。多份提案主张构建 golden-fixture 差分测试套件、或彻底原生化替换掉 Python shim 依赖。

*优先级信号*: P0(x1)/P1/P2/P3 混合　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-systemic-gaps, 2026-07-11-genuinely-uncovered-frontiers, five-verifiable-code-level-gaps

**工作区快照/原子回滚 (phase 级 git-stash 安全网)** `×9`

agent phase 可能在写入过程中被 SIGKILL 或预算耗尽中断，此时工作树处于部分写入的不一致状态且无回滚机制。多份提案主张在每个 phase 执行前自动 git stash（或等效快照），phase 失败/中断时可一键回滚到干净状态，形成 Workspace Sandbox/Atomic Rollback 能力。

*优先级信号*: P0(x3)/P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-genuinely-uncovered-runtime-frontiers, high-value-expansion-directions, forgeos-five-architect-product-perspective-2026-07-10

**I/O 超时与 Context 传播缺失 (git 子进程/prompt gather 无超时)** `×8`

computeCodeTestRatio/computeFileDelta 直接 exec.Command('git',...) 不设 Context 或超时，prompt.Gather/GatherCached 读 ROADMAP/ADR 等文件用裸 os.ReadFile 无超时保护；NFS 超时或 git 挂起会让整个 phase 无限期挂起而无法被上层的墙钟护栏发现。

*优先级信号*: P0(x2)/P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-four-uncovered-architectural-extension-directions, 2026-07-11-forgeos-four-uncovered-architectural-gaps

**韧性对抗测试 / 混沌工程 / 控制面故障注入框架** `×7`

现有 707+ 测试全部覆盖正常路径，没有任何故障注入/混沌工程层验证 checkpoint ENOSPC/EIO、budget 耗尽、并发时序竞争等场景下的真实行为。多份提案主张新增 chaos-injection 层（harness/chaos.mjs 或 internal/chaos）、可注入的 LLM 返回序列模拟器，以及针对 checkpoint/orchestrator/外部依赖三层的分阶段故障注入框架。

*优先级信号*: P0(x2)/unstated(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-hidden-subsystem-gaps, fresh-five-systemic-extensions-2026-07-10, fresh-scan-2026-07-11, next-horizons

**进程崩溃/孤儿进程回收/优雅关闭协议** `×6`

forge 只处理 SIGINT/SIGTERM 的有序取消链，SIGKILL/OOM/panic 会让 forge 主进程立即死亡而不触发子进程组清理，导致 claude 子进程及其孙进程成为孤儿继续消耗 API 预算；`forge doctor` 目前也只诊断不修复。多份提案主张父崩溃检测（PR_SET_CHILD_SUBREAPER 或等效机制）、优雅关闭协议以及把 doctor 从纯诊断升级为可选自动修复。

*优先级信号*: P0(x1)/P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-foundational-resilience-gaps, novel-five-perspectives-2026-07-10-deep, uncovered-frontiers-v25-systemic-boundaries

**边界情况分类矩阵 (元提案：系统化5轴测试框架)** `×6`

多份文档独立提出同一个方法论级建议：为每个核心子系统建立零值/极限值/损坏输入/并发/外部环境故障 5 个轴的边界情况矩阵文档，把当前'撞到才修'的被动模式转为主动预扫+测试覆盖的纪律。

*优先级信号*: unstated(x多)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-10-five-genuine-architectural-frontiers, expansion-strategic-perspectives

**持久化格式版本化 (Checkpoint/Trace/Memory FormatVersion 写而不查)** `×5`

checkpoint.json/trace.jsonl/memory.jsonl 都声明了 _format 版本字段，但写入端设置后读取端从不校验，一旦未来发布不兼容格式，旧版本数据会被静默按新语义误解析且零告警。与 docs/ 层面'其他'类别中同一诊断呼应，是全仓库范围内的横切问题。

*优先级信号*: P0(x1)/P1(x3)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-expansion-directions, 2026-07-11-scan-five-codegrounded-systemic-frontiers, 2026-07-11-forgeos-operational-maturity-and-structural-debt

**大仓库规模下的性能悬崖 (heuristics 随规模无声失效)** `×4`

computeCodeTestRatio 的 git diff --stat 全量扫描、gatherSignals 的全仓重算等在今天的小规模代码库下工作正常，但缺乏增量/缓存机制，在大型仓库上会线性甚至更差地退化，且退化是无声的（不报错，只是变慢）。

*优先级信号*: P2(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-architect-extension-directions, high-value-extension-directions-v2, strategic-expansion-perspectives

**[数据质量标记] 4条明确的幻觉/离题条目** `×4`

提取过程中 4 份文档（high-value-extension-directions / -v2 / -v3 及 genuinely-uncovered-five-frontiers）各自生成了与 ForgeOS 代码库完全无关、通用化的'断路器/重试/幂等性'话题条目，且被原文档自身标注为 [HALLUCINATED/OFF-TOPIC]。这不是一个真实需求主题，而是自动化生成管线偶发产出离题内容的证据，予以保留标记但不计入真实需求统计。

*优先级信号*: n/a　·　*最高成熟度*: n/a (自认幻觉)　·　*示例来源*: genuinely-uncovered-five-frontiers, high-value-extension-directions, high-value-extension-directions-v2, high-value-extension-directions-v3

**并行锁顺序静态校验 (8 把共享互斥锁的顺序契约)** `×2`

parallel.go 顶部虽文档化了 8 个共享互斥锁的获取顺序契约，但纯靠人工遵守，Go 编译器无法验证，顺序违规只在调度巧合下才产生偶发死锁（heisenbug）。提案建议实现类似 Linux lockdep 的运行时锁顺序验证器。

*优先级信号*: unstated　·　*最高成熟度*: architected　·　*示例来源*: high-value-extensions-analysis, extension-analysis

**Scorecard 多进程文件 IPC 可靠性建模** `×2`

trace.jsonl → scorecard-wind.mjs → scorecards.json 这条链路是一个多步骤、无事务保证的文件系统 IPC 管道，缺乏形式化的可靠性建模（部分写入、并发读写冲突等）。

*优先级信号*: unstated　·　*最高成熟度*: architected　·　*示例来源*: forge-core-five-unseen-structural-gaps

**配置状态空间覆盖/属性测试 (mode×lifecycle×workflow×parallel 组合爆炸)** `×2`

mode×lifecycle×workflow×parallel 的配置组合空间超过 5000 种，当前测试只覆盖了少数手选组合，多份提案建议引入属性测试（property-based testing）系统性覆盖配置状态空间，尤其是 parallel×production 语义等价性等边角组合。

*优先级信号*: P2　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-structural-blindspots, 2026-07-11-forgeos-five-structural-blindspots

**Unicode/编码/国际化鲁棒性** `×1`

YAML 解析器和多处路径处理隐含 ASCII/UTF-8 假设，未针对非 ASCII 路径名、BOM、混合编码等场景做过验证。

*优先级信号*: unstated　·　*最高成熟度*: ideation-proposal　·　*示例来源*: forgeos-architect-product-perspective-five-frontiers-2026-07-10

---

### 其他/跨领域架构治理 `other` ⚠ 人工聚类

19 个独立主题，原始条目 125 条。

**持久化格式版本化与迁移路径 (checkpoint/trace/memory FormatVersion 写而不查)** `×15`

三种持久化格式都声明了 _format/FormatVersion 字段，但写入端设置后读取端从不校验，向后兼容完全依赖隐式的 omitempty 策略——一旦未来发生字段重命名或类型变更，旧格式数据会被静默按新语义误解析且零告警。与 edge-case-reliability/observability-audit 两个类别中同一诊断的不同分身共同构成全库出现频率最高的横切诊断之一。

*优先级信号*: P0/P1/P2 混合　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-foundational-architecture-gaps, 2026-07-11-forgeos-operational-maturity-and-structural-debt

**cmd/forge 上帝包结构性张力 (业务逻辑层缺失，需系统性包提取)** `×14`

cmd/forge（16-17 个文件、约 12500+ 行）是唯一导入几乎全部 internal 包的枢纽包，已经积累了大量本不该属于 CLI 胶水层的领域逻辑（budget 跟踪、prompt ledger、收敛信号收集），持续把文件推向 500 行/文件的体积红线。Sprint 27-31 的演进记录中反复出现这一模式，多份提案主张系统性提取为 internal/cli、internal/runner、internal/cost 等子包。

*优先级信号*: P0(x多)/P1(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-codelevel-architectural-blindspots, 2026-07-11-forgeos-operational-maturity-and-structural-debt

**Python YAML shim——零依赖声明的裂缝** `×13`

ForgeOS 宣称 Go 运行时纯标准库零外部依赖，但 loadWorkflow 在 Go 原生 yaml2json 解析器失败时会 fallback 到 python3 harness/yaml2json.py，是'零依赖'哲学声明与实际运行时行为之间一个反复被独立发现的具体裂缝，也带来冷启动性能开销（每次 forge run/evolve 都要 fork/exec python3）。

*优先级信号*: P0/P1 混合　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-code-grounded-systemic-gaps, 2026-07-11-five-hidden-subsystem-gaps

**forge-core 二进制生命周期/发布/分发/自更新** `×10`

forgeVersion 目前只是硬编码为 'dev' 的展示字符串，没有 --version 标志的真实语义、没有多平台发布流水线、没有 forge self-update/rollback 机制，升级二进制后也无法知道一段历史数据是哪个版本产生的。

*优先级信号*: P0(x多)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-product-operations-systemic-gaps, forgotten-five-structural-debt

**ForgeOS 不可嵌入性 (internal/ → pkg/ 导出为可消费的 Go 库)** `×8`

forge-core 的全部 17-18 个内部包都在 internal/ 目录下，Go 语言机制使外部 Go 模块无法 import 这些包（orchestrator 引擎、闸门系统、收敛判定器），cmd/forge 是 package main 只能作为二进制调用，无法作为库集成进其他 Go 程序。

*优先级信号*: P0/P1 混合　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-foundational-architecture-gaps, 2026-07-11-forgeos-five-unbuilt-product-architectural-extensions

**Prompt 作为一等代码资产 (版本化/有效性度量/A-B 测试闭环)** `×7`

约 750 行的 prompt 构建逻辑（buildPrompt/memoryContext/artifact 渲染）硬编码在 Go 字符串中，修改 prompt 等价于修改代码、重新编译部署，无法做 prompt 版本对比、A/B 测试或运行时切换；scorecard 记录的质量/成本/延迟信号也从未回流到 prompt 装配路径。

*优先级信号*: P0/P1 混合　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-structural-debt-and-product-frontiers

**状态存储/多进程隔离 (Store 抽象层/session 协调/forge daemon)** `×7`

当前所有状态（memory/trace/checkpoint/scorecard）基于文件系统直接读写，跨进程共享不存在，两个 forge evolve 实例会互相覆盖；提案主张抽象出 Store 接口层并引入可选的 forge daemon 常驻进程实现跨命令的配置热加载与状态协调，与其它类别中的 Run Identity 主题密切相关但聚焦于抽象层设计而非仅仅打标识符。

*优先级信号*: P1/P2　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-novel-architect-product-directions

**示例应用质量/跨语言标准化 (production-readiness baseline)** `×7`

forge-init 生成的 starter 项目和 examples/ 下的示例应用缺乏统一的生产就绪基线（信号处理/优雅关闭/健康检查端点），跨语言示例（Go/Python/Rust/TS）架构风格也不一致，四语言目标栈中只有部分语言有真实示例。

*优先级信号*: P0-P3 混合　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-expansion-directions

**Agent 输出结构化契约 (Structured Output/Error 类型体系)** `×6`

agent 输出解析、错误处理目前分散在多个独立的临时文本解析函数中，缺少统一的结构化契约层（如 FilesCreated/FilesModified/Decisions 等字段），与 orchestration-engine 类别中'跨阶段产物契约'主题同源但聚焦点在 agent 输出本身而非 phase 间契约。

*优先级信号*: P0/P1　·　*最高成熟度*: architected　·　*示例来源*: five-uncovered-horizontal-frontiers

**生产部署/发布/回滚/事故响应工作流 (Idea→Production 的最后一公里)** `×6`

ForgeOS 宣称'Idea → Production'，但当前系统在代码通过 forge accept 闸门后就停止——没有部署、发布、版本管理或回滚协调；converge.Signals 的字段中没有任何与部署相关的信号，也没有生产事故（需立即修复）对应的 mode/lifecycle 或回滚工作流。

*优先级信号*: P0(x多)　·　*最高成熟度*: architected　·　*示例来源*: strategic-production-gaps, 2026-07-11-forgeos-five-product-architectural-extension-directions-verification

**风险/质量信号增强 (静态分析驱动风险提取/置信度标定/负向学习环路)** `×6`

当前的风险分类器诚实地自我标注为路径字符串启发式，从不读文件内容；agent 自报的 CONFIDENCE 分数也从未与后验客观结果（gate PASS/FAIL）做校准。提案主张引入静态分析驱动的真实风险信号提取、置信度标定学习闭环，以及针对 routing scorecard 的负向趋势/失败率检测护栏。

*优先级信号*: P0/P1　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-10-five-genuine-architectural-frontiers

**遥测/产品分析、Agent IAM、对抗红队验证、跨平台可移植性等零散单点诉求** `×5`

一组各自只出现 1 次、彼此不相关的独立诉求：面向产品决策的匿名遥测；agent 角色运行时完全平等、无按角色区分的权限模型（Agent IAM）；缺少对抗性（红队）reviewer 验证循环；forge-core 对 Unix 的隐性假设（SIGINT/SIGTERM/硬编码路径分隔符）损害'host-independent'的声明；Lifecycle 状态机（idea→mvp→growth→production）目前是纯静态手动字段。

*优先级信号*: unstated　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-genuine-systemic-boundaries, strategic-expansion-perspectives

**forge-ai Python 智能层 (ADR-0002 声明但零代码的第三支柱)** `×4`

ADR-0002 声明的 polyglot 目标栈包含一个 Python 'forge-ai' 智能层，但至今零代码、零目录。多份提案主张以 os/exec 子进程或 Unix-socket 常驻 daemon 的形式落地一个提供 embedding 语义检索等能力的 Python sidecar。

*优先级信号*: unstated/P1　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-tech-lead-analysis-five-genuine-directions, forgeos-five-unseen-product-architect-extensions

**状态目录健壮性与灾难恢复 (forge doctor --state / backup / restore)** `×4`

forge doctor --state 健康诊断、forge state backup/restore/list-backups 命令，以及可选的自动周期性快照——把 .forge/ 这一系统唯一持久层从'无备份单点故障'升级为有恢复路径的状态存储。

*优先级信号*: P0(x多)　·　*最高成熟度*: coded-and-reviewed　·　*示例来源*: 2026-07-10-five-operational-frontiers

**配置/Schema 治理 (跨包接口契约/config schema 版本化)** `×4`

5 个独立的配置系统（project.yml/modes.yml/policies.yml/routing policy.yml/workflow YAML）存在重叠字段但没有统一的组合与覆盖解析模型；提案主张为 .agent/ 配置文件增加统一的 forge_schema_version 戳记与迁移管线，并引入消费者侧定义的 Go interface 契约层。

*优先级信号*: P1　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-unseen-codelevel-architectural-gaps

**Context Engine 结构化重构 (从字符串拼接到类型化 Context Lane)** `×3`

架构上声明的'Context Engine'（ForgeOS 五大核心引擎之一）目前的实际实现只是字符串拼接，提案主张引入类型化的 ContentClass/Priority/TokenBudget 结构替代裸字符串组装。

*优先级信号*: P1　·　*最高成熟度*: architected　·　*示例来源*: architect-product-perspective-four-structural-gaps

**插件化 Agent/Gate/Router 扩展系统** `×3`

Agent 类型、Gate 种类、路由维度全部硬编码在 Go 代码里（agentTier map/opusFloorAgents/ScoreInput 结构体），第三方想扩展新 agent 类型或 gate 检查器必须直接改 forge-core 源码，没有任何声明式扩展点。

*优先级信号*: P1　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-expansion-directions-product-platform-perspective

**需求文档语料治理 (新颖性去重管道 + 方向生命周期 + 引用防漂移)** `×2`

元层面提案：本次分析亲身验证的问题本身——docs/requirements/ 下已积累数百篇高度重复的'expansion direction'文档，提案主张构建 harness/novelty-scan.mjs（MinHash 近似去重签名 + INDEX.json 索引）与 .agent/insights/ 四态方向生命周期管理，从源头上抑制这种指数级重复生成。这是本次分析过程亲历验证其必要性的一条提案。

*优先级信号*: unstated　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-extension-directions-verification

**自举/Dogfooding：让 ForgeOS 用自己的 evolve 引擎修复已知真实技术债** `×1`

一份 peer-review 式的元评论指出：多数扩展方向都排除了'需要外部资源'的方向，但遗漏了一个不需要任何外部资源的自举方向——直接让 ForgeOS 调用自己的 evolve 引擎去修复本文档体系里已经发现的真实技术债，是对整个'AI 软件工厂'叙事最直接的自我验证。

*优先级信号*: unstated　·　*最高成熟度*: ideation-proposal　·　*示例来源*: next-horizons

---

### 测试基础设施 `testing-infrastructure`

37 个独立主题，原始条目 113 条。

**增量/变化感知 Gate 执行与结果缓存** `×12`

当前 forge accept/gate 每次都对整个仓库跑全部 gate（lint/test/build/security/arch/complexity），与改动范围无关——改1个文件和改100个文件耗时相同，已计算出的 changed-files/FileDelta 信号也从未真正用于裁剪 gate 执行范围。反复被提出的方案是：为每个 gate 声明其关心的文件/包范围（scope），按 git diff 或 tree-hash 判断是否可跳过或复用上次裁决（内存缓存或磁盘持久化），并在依赖图明确时并行执行互不相关的 gate 以压缩墙钟时间，git diff 不可用时安全回退全量扫描。多篇文档还提出了 AST 扫描缓存（arch-check/secret-scan 按文件哈希增量重扫）与循环内（loop-back 之间）的阶段级缓存作为具体落地切口。

*优先级信号*: P1(x7)/P2(x3)/high(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-code-grounded-expansion-perspectives, five-genuinely-unexplored-code-level-architectural-expansions, five-systemic-oversights-v45, novel-architectural-extensions-v40

**核心解析器 Fuzz/属性/差分测试（yaml2json 等手写解析器健壮性）** `×8`

forge-core 的手写解析器（尤其是承担全部治理层输入的 yaml2json、363 行的 pyproject.toml/go.mod 解析器、SCA 的 semver 匹配器）几乎完全没有 fuzz/属性/差分测试覆盖——全仓 19 个 Go 包里只有 1 个 Fuzz 测试，且真实发生过 Sprint 27 的 block-scalar 损坏 bug 因差分安全网测试误用 t.Logf 而非 t.Errorf 静默存活 6 个 sprint。反复提出的方案是：为 yaml2json 添加 fuzz 语料（深嵌套/超大标量/混合缩进/unicode）、构建 Go↔PyYAML 差分或双后端回退机制、为 TOML/semver 等解析器补充基于往返不变量的属性测试，并把差分测试改为真断言。部分提案进一步主张按 YAML 构造类型（mapping/sequence/anchor/block-scalar）拆分 golden-file 用例，以避免整文件差分掩盖局部 bug。

*优先级信号*: P1(x6)/P0(x1)/P0-P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-hidden-product-quality-gaps, 2026-07-11-forgeos-five-unexplored-perspectives, 2026-07-11-scan-five-codegrounded-systemic-frontiers, forges-five-hidden-product-quality-gaps

**故障注入与韧性验证基础设施（Fault Injection / Chaos Testing）** `×7`

ForgeOS 自身最复杂的韧性逻辑（529 过载检测、超时 kill、退避重试、budget guard、checkpoint 原子写、NoProgress tripwire）只在静态单元测试 fixture 上被验证过，从未对真实或模拟的 agent 进程/磁盘故障/时钟跳变端到端验证过，例如“重试的 implementer 看到的是干净文件还是半写文件”这类问题完全无测试覆盖。反复提出的方案是引入一个可插拔的故障注入层（FakeAgentExecutor/internal/fault Injector/FORGE_CHAOS_ENABLE 开关），用 YAML fixture 或编程方式模拟 ENOSPC、超时、SIGKILL 中途写入、时钟跳跃、子进程 OOM 等场景，并保证生产二进制零注入路径开销。部分提案把注入点放在 CLI 接线层（cmd/forge）而非已充分测试的 orchestrator 内部，专门覆盖真实进程输出解析路径。

*优先级信号*: P1(x3)/P2(x3)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-system-level-gaps, 2026-07-12-five-extension-directions-architect-product-perspective, 2026-07-11-five-hidden-subsystem-gaps, forge-core-five-unseen-structural-gaps

**Agent 产出质量评测框架（Eval / Golden Tasks / Quality Score）** `×7`

现有 scorecard 只记录 gate 二元 pass/fail 的 quality_score，routing 的 HistoryTiebreak 因此无法区分“勉强通过”和“干净通过”，也无法比较不同模型/prompt 模板随时间的产出质量。反复提出的方案是新增一个 eval/ 目录与 internal/eval 包，定义 golden task 与可插拔的 checker（结构完整性、复杂度增量、文档完整性、覆盖率增量），提供 forge eval run/forge eval compare 命令，产出多维质量分并异步/非阻塞地回灌 scorecard 驱动路由决策，并普遍强调用统计重复运行而非 LLM-as-judge 来应对 LLM 输出的非确定性。

*优先级信号*: P2(x6)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-highvalue-architect-pm-directions, 2026-07-12-forgeos-five-highvalue-extensions-senior-architect-pm, 2026-07-12-tech-lead-analysis-five-genuine-directions, forgeos-five-unseen-product-architect-extensions

**共享测试基础设施成熟度（testutil / golden-file / fs 与时钟注入）** `×6`

forge-core 坚持“纯标准库零依赖”也延伸到了测试层——没有 testify/cmp 之类的断言库、没有统一的 internal/testutil 共享 fixture 包（每个包各自重造轮子）、测试大量依赖未注入的 time.Now()/time.Sleep 导致 CI 抖动、写路径（memory/persist/prompt-cache）也缺少 fs.FS 风格抽象使文件系统故障难以在测试中模拟。反复提出的方案是建一个手写的零依赖 testutil 包（TempDir、golden-file 比对器、可注入 Clock 接口）、为稳定边界（asset 解码、yaml2json）引入系统化 golden-file 测试，以及用 io/fs 抽象让写路径可注入失败进行测试。

*优先级信号*: P1(x3)/P2(x2)/unspecified(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-highvalue-extension-directions, 2026-07-11-five-codegrounded-product-expansion-directions, 2026-07-11-forgeos-five-unexplored-perspectives, forgeos-five-architect-product-extensions-2026-07-10

**编排性能基准测试与退化检测** `×5`

ForgeOS 拥有丰富的可观测数据但零性能基线——memory.Compact() 是 O(n) 全量 JSONL 扫描却无 benchmark，TF-IDF 检索每次从零重建矩阵，workflow YAML 每次 forge run 都重新解析，CI 的 dry-run 冒烟测试只验证引擎能跑完却不测量任何耗时/内存，三个孤立的 Go 微基准的结果也从未被 CI 汇报，导致长跑级别的性能退化完全无声。反复提出的方案是建立一个 benchmark/ 套件（覆盖 trace/asset/converge/memory/yaml2json/buildPrompt 等关键路径），生成结构化 `.forge/bench.jsonl` 基线，先以 advisory（非阻断）信号接入 forge accept/CI，待基线稳定后再考虑转为阻断。

*优先级信号*: P1(x4)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-systemic-declaration-gaps, 2026-07-11-forgeos-five-unexplored-perspectives, forgeos-architect-product-perspective-five-frontiers-2026-07-10, forges-five-unbuilt-foundations

**示例应用 CI 回归检测（Dogfood Examples as Regression Suite）** `×5`

ForgeOS 的两个手工构建的 dogfood 示例应用（url-shortener、go-taskd）是唯一可外部验证“软件工厂确实好用”的证据，但只在构建时被验证过一次，CI 从不重新对它们跑 forge accept/测试，导致 harness 或 forge-core 的行为变化可以在不被发现的情况下悄悄破坏已验证过的示例。反复提出的方案是把示例应用接入 CI 作为持续回归探针——新增 CI 步骤对每个 examples/*/ 目录跑 forge run build --executor dry + 原生测试，并配一个 manifest 防止新增示例被遗忘，部分提案进一步提议 forge verify-examples 子命令。

*优先级信号*: P2(x4)/P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-four-infrastructure-gaps-subprocess-example-bridge-config, 2026-07-11-four-unseen-foundational-gaps, high-value-extension-v47

**语义级 Agent 产出验证 Gate（Semantic/Behavioral Gate）** `×5`

现有 6 种 gate（lint/test/build/complexity/arch/security）全部是机械门，只验证代码形式——能编译、测试通过、无环、无已知 CVE——却从不验证生成代码的业务语义是否正确，这被反复点名为“代码看起来对但其实错”这一 LLM 典型失败模式最大的风险敞口。提案方向高度一致：新增 contract（OpenAPI diff/契约测试）、property（属性测试）、mutation（变异测试）、behavior/snapshot（行为快照）等语义 gate 类型，以镜像现有 lint/coverage adapter 的模式接入 harness，工具不可用时诚实降级为 N/A，且普遍建议第一版只报告不阻断。

*优先级信号*: P2(x5)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-genuine-architectural-frontiers-senior-architect-pm, 2026-07-12-forgeos-five-genuine-architectural-frontiers-senior-architect-pm, genuine-architectural-horizons-five

**工作流编排集成测试框架（DSL / Stub Executor）** `×5`

测试金字塔中间层存在一个“沙漏空洞”——orchestrator/converge/loop 各包被隔离单元测试得很扎实，真实端到端跑又要烧真实 LLM 预算，但两者之间缺一个中间层：没有测试断言过完整编排行为本身（loop-back×mode-gating×checkpoint/resume×并行组合），例如“build.yml 在 mode=engineering 下 reviewer 必须运行”完全靠人工验证。反复提出的方案是构建一个声明式 given/then 测试 DSL 或专用 testutil 包（Stub/Fake Executor + 可注入 Clock + gate 结果注入），让 workflow 作者能以脚本方式断言阶段序列、gate 结果、mode 矩阵、feeds_forward 跨阶段数据传递等行为，而不产生真实 LLM 调用成本。

*优先级信号*: P1(x2)/P2(x2)/★★★★★(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgotten-frontiers-five, novel-five-frontiers-v34, 2026-07-11-genuinely-uncovered-frontiers, expansion-gaps-v7-novel

**编排引擎 Property-Based / Fuzz 测试（状态空间覆盖）** `×4`

internal/orchestrator 的编排状态机由 phase 数×mode×lifecycle×stop_condition×并行串行×on_fail×MaxLoopBack×MaxIter×executor 行为×checkpoint/resume 等维度组合而成，估算组合空间达数十万种，但目前仅靠约 80 个手工枚举场景测试覆盖（<0.1%），没有 testing/quick 或 go test -fuzz 针对 orchestrator/loop/parallel 引擎的系统性验证，而真实发生的 bug（Sprint 27 yaml2json 损坏、Sprint 31 ContextCache 数据竞争）恰恰都是靠巧合/reviewer/-race 才发现而非系统化测试。提案方向是引入随机化 workflow 生成来 fuzz 测试编排不变量（Waves() 排序正确性、MaxLoopBack 上界、trace.Event 往返序列化、memory.Compact 语义等价性），明确定位为统计性覆盖而非完整模型检验。

*优先级信号*: P1(x2)/P2(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-foundational-architecture-gaps, 2026-07-11-five-novel-architect-product-directions

**Harness 跨语言桥接契约测试（Go↔Node/Python）** `×4`

forge-core 的 Go 层通过 shell 调用 Node/Python 工具（gate.mjs、check.py、yaml2json.py、acceptance.mjs、sca.mjs）并隐式信任其 stdout 格式，没有任何测试验证这个格式契约本身——历史上 Sprint 27 的 yaml2json block-scalar 损坏正是这种隐式契约悄然破坏的先例。反复提出的方案是建立一个独立于工具内部逻辑正确性的桥接契约测试套件，对每个 Go↔Node/Python 边界断言其 stdout/exit-code 的格式与 Go 消费方的假设一致，作为 forge accept 的前置 CI 检查。

*优先级信号*: P3(x2)/P2-P3(x1)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-four-infrastructure-gaps-subprocess-example-bridge-config, 2026-07-11-four-unseen-foundational-gaps

**自我测试隔离与自举完整性（Self-Test Isolation & Bootstrap Integrity）** `×3`

dogfooding 存在一个循环依赖风险——harness 的集成测试直接在真实仓库工作目录上运行，而非隔离的 fixture 副本，这意味着 gate.mjs/arch-check.mjs/acceptance.mjs 自身的 bug 有可能反过来卡住修复它自己所需要的那个 commit。反复提出的方案是构建 hermetic fixture 仓库生成器（os.mkdtemp()/t.TempDir() 风格），把 harness 测试迁移到只在 fixture 上运行，新增一个 forge test --self 模式，并为 harness 测试显式声明行为契约。

*优先级信号*: P1(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-codegrounded-architectural-blindspots-five-directions, five-genuinely-uncovered-architectural-frontiers-2026-07-10

**Dry-Run vs 真实执行器语义鸿沟** `×3`

AgentExecutor 接口有 dry-run 与真实两种实现共享编排代码，但几乎所有测试只走 dry-run 路径——DryRunExecutor 返回空输出导致依赖 agent 输出的下游全部断裂（review 永远无 VERDICT、loop-back 永不触发），而此前真实点火发现的八个 bug 全部是 dry-run 测试看不见的。反复提出的方案是构建一个 DryVsRealCollisionSuite/FakeAgentExecutor（用固定 fixture 回复驱动同一状态机路径）对同一 workflow 在两种执行器下运行并断言语义等价，部分文档进一步指出“dry-run 下 loop-back 修复等行为诚实性声明”只被一次性人工 pilot 跑验证过，缺少可重复触发的自动化回归套件持续守护这个诚实边界。

*优先级信号*: P1(x2)/gap-implied(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-systemic-declaration-gaps, strategic-production-gaps, ignition

**外部 CLI 隐式依赖声明与文档化（python3/node/git）** `×3`

尽管 go.mod 声明零外部 Go 依赖，`go test ./...` 却隐式 shell 调用 python3（yaml2json 差分测试）、node、git（ADR 测试），这让 Go-only 的新贡献者遇到与真实测试失败无法区分的“文件未找到”式困惑失败，削弱了“零外部依赖”的说法。反复提出的方案是：新增一份 TEST_REQUIREMENTS.md 显式列出这些隐式依赖，把静默的 t.Skip 升级为带说明的 t.Log，构建一个不依赖 Python 的 yaml2json golden-fixture 验证套件作为差分测试之外的安全网，并在 CI 增加一个纯 Go 的 test matrix job。

*优先级信号*: P1(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-global-scan-engineering-product-expansion-directions, forgeos-five-architect-product-expansion-2026-07-10

**pi-batch.py 分析工具的产品质量缺口** `×3`

pi-batch.py 是这个仓库自己用来批量驱动 AI agent、生成 90+ 份需求分析文档的 499 行 Python 脚本，却零测试覆盖，还带有一个 Sprint 27 就记录在案但未修的超时 bug（stdout/stderr 各自独立的 reader 线程都拿到完整超时预算，导致实际超时约为配置值的 2 倍）、一个混淆二进制不在 PATH 与 cwd 错误的误导性异常，且完全游离在 ForgeOS 自身治理之外。反复提出的方案是为其纯函数面补单元测试、修复超时/reader 线程 bug、把 PyYAML 变成硬依赖或优雅降级，并把该脚本纳入 forge accept 治理，架构阶段的方案进一步提出拆分为 pi-batch/ 包并保留兼容入口。

*优先级信号*: P2(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-hidden-product-quality-gaps, forges-five-hidden-product-quality-gaps

**测试跳过（Skip）级联侵蚀追踪与治理** `×3`

仓库中至少 27-32 处 t.Skip/t.Skipf 依赖环境条件（缺 python3、缺 fixture、-short 模式、非仓库根 cwd 等），但没有任何机制追踪“预期用例数 vs 实际执行数”，导致同一个 go test ./... 在不同机器/CI 上实际跑的用例数不同、覆盖率分母也随环境漂移，且跳过不产生任何告警，使 forge accept 可能因静默跳过而“假绿”。反复提出的方案是为每处 skip 打分类标签（env/fixture/intentional/config）、注册预期测试计数并汇总跳过数、在 forge accept 中加一个非阻断的 WARNING 检查，部分提案进一步建议用 embed.FS 消除路径依赖、用构建约束隔离依赖 Python 的测试。

*优先级信号*: P2(x1)/P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: expansion-five-systemic-architectural-gaps, production-product-gaps-v43

**测试套件质量深度门禁（Assertion Density / Flaky Detection）** `×3`

现有 gate 只统计代码/测试行数比例这类表面指标，无法识别 LLM 生成的重言式或只覆盖 happy-path 的低质量测试（如 assert(true)），这类测试目前能顺利通过所有 gate 却几乎没有实际验证价值。反复提出的方案是在 gate 系统中加入轻量启发式检查——按语言扩展 assertion-density 检测模式、跨迭代重跑比较以侦测 flaky 测试、构建一个综合 CodeTestHealth 指标，部分提案进一步设想一个 forge test --quality 命令持续追踪覆盖率/耗时趋势并对 harness 自身治理工具做自省式覆盖检查。

*优先级信号*: unstated(x1)/P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: genuinely-novel-expansion-directions, structural-gaps-v41-genuinely-unexplored

**Gate 执行 Fail-Fast 排序与代价优先级** `×2`

现有验收探针（lint/build/test/complexity/arch/security）按相同优先级顺序执行，既没有按历史平均耗时排序，也没有在便宜的探针已经确定 FAIL 后短路停止更贵的探针——纯文档改动的 commit 也要跑全套 gate。提案是按历史平均代价对探针升序重排并在首次 FAIL 后短路，叠加基于 git diff 的变更感知跳过，预期可将高频开发循环中的 gate 开销降低 20-40%。

*优先级信号*: P2(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-adoption-gating-product-trust-gaps, 2026-07-10-genuinely-novel-architect-perspective

**配置状态空间覆盖盲区** `×2`

ForgeOS 真实的配置空间（mode×lifecycle×workflow×executor×model×安全防护×权限×并行×max-iter）理论组合数达 5000+，但测试只覆盖约 6 个 mode×lifecycle 组合对（约 0.12%），诸如生产 lifecycle 下的 require_min_gates、--parallel 与 mode-gating 叠加等组合完全未被验证过。提案是产出一份文档化的配置矩阵，对 mode.Effective → routing.TierFor → orchestrator.RunFrom 这条核心链路做属性测试，并新增一个 forge doctor 检查项标记生产中实际在用但未被测试覆盖的配置路径。

*优先级信号*: P2(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-structural-blindspots, 2026-07-11-forgeos-five-structural-blindspots

**CI 管线语义完整性覆盖广度** `×2`

CI 目前只跑一个 forge run build --executor dry 冒烟测试（只证明二进制不崩溃），从未系统性覆盖 discover→design→review→build→evolve 各阶段间的顺序/数据传递正确性、并行 wave 执行、或 evolve 循环本身——而编排语义恰恰是这个自治理系统风险最高的部分却完全没有 CI 回归覆盖。提案是把 CI 的 dry-run 覆盖扩展到全部五个 workflow（不仅 build.yml），并加入 --parallel 与 evolve 的 dry-run 命令，以捕获类似 Sprint 27 阶段名子串匹配 bug 这种未被发现的回归。

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-product-output-quality-and-ecosystem-gaps

**Agent 契约解析 CI 可测桥接层（ContractTestExecutor）** `×2`

reviewer/executive/PM 等机读契约（VERDICT/CONFIDENCE）三层 fallback 解析链路目前只能通过真实付费 API 调用才能测试，CI 里没有零成本的方式验证这条解析逻辑本身是否正确。提案是新增一个实现现有 AgentExecutor 接口的 ContractTestExecutor，用预配置的 mimic 输出驱动同一条 observeFor→解析器→converge 路径，并配一个声明式 .agent/contracts/ 注册表与 forge validate --contracts 命令。

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-scan-five-codegrounded-systemic-frontiers

**工作流 Golden-File 确定性测试骨架** `×2`

orchestrator/converge/loop 包目前只用硬编码结构体做单元测试，没有任何测试真正加载一份真实 workflow YAML 端到端跑一遍，并将其 phase 序列、mode-gated gate 集合、收敛结果与预期的 golden 输出比对——Sprint 26 那个曾静默损坏 7 份真实 workflow 文件数月之久的 yaml2json bug 正是这类回归的典型代表。提案是基于 DryRunExecutor 构建一个 golden-test runner，把执行过的 workflow 序列化并与 checked-in 的 golden 文件做 diff，支持 --update-golden 模式并在 CI 中强制执行。

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: expansion-five-truly-uncovered-frontiers-v46

**Prompt 构建管道质量保证与 Golden 快照测试** `×2`

尽管仓库已有 707 个测试，但 prompt 构建这个“编译器”（buildPrompt 及其管线）却零测试断言实际发给 agent 的 prompt 内容是什么。提案是为按 workflow×phase 的 buildPrompt 输出做 golden-file/快照回归测试，增加 token 预算审计（逼近模型上下文窗口时告警），并把 unwrapClaudeResult 在遇到未识别 JSON 信封格式时的行为从返回空字符串改为回退原始输出，进一步提案还主张把机读契约注入做编译期校验、按 lane 隔离构建以区分 prompt 组装失败与 agent 质量失败。

*优先级信号*: unstated(x1)/P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-production-readiness, next-five-frontiers

**生产级 CI 全链路补全（go build / -race / harness 单测接入 CI）** `×2`

.github/workflows/forge.yml 目前只跑 forge accept 和不带 -race 的 go test，完全不执行 go build ./...、go test -race ./...、node --test harness/，意味着编译错误和并发数据竞争都不会被 CI 拦截——而 orchestrator/loop.go 这类 goroutine+channel 并发编排代码，竞态检测应是正确性保证而非可选质量属性。提案是补齐这几个纯 CI 配置项（约 20 行 YAML，无代码风险），被标记为“本周应完成”的最低成本高价值项。

*优先级信号*: P0(x1)/P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: scan-current-gaps, strategic-expansion-directions

**对抗/红队 Agent 验证循环** `×1`

当前 workflow 里所有 agent（reviewer/qa 等）在设计上都是协作/建设性的，没有任何一个角色被专门赋予“主动尝试找茬、捕捉 reviewer 群体盲从”的任务。提案是在 reviewer/qa 之后引入一个新的“对抗”phase/gate 类型，其 prompt 明确要求 agent 找出 reviewer 遗漏的缺陷，产出一份结构化的发现报告而非简单的二元裁决，并接入一个新的 AdversarialFindings 收敛信号。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-novel-architect-product-directions

**开发体验与测试基础设施成熟度综合包** `×1`

这是一份将多个测试/DX 缺口捆绑在一起的提案：没有 forge init --template 式的 workflow 脚手架注册表、没有变异测试、没有针对纯函数（converge.Evaluate、routing.TierFor、yaml2json.Decode）的属性测试、没有编排级集成测试套件（CI 的 --executor dry 跳过真实 gate/agent 逻辑）、也没有代码覆盖率采集或阈值门。审阅确认了这些核心缺口但纠正了脚手架工具描述这一细节（它已复制完整的 .agent/ 治理树，而不仅是 harness/CI）。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-genuinely-uncovered-frontiers

**跨会话/跨调用 Gate 结果持久化缓存** `×1`

Gate 执行是完全无状态的——即使代码树自上次运行以来毫无变化，每次 forge run 仍会从零重新拉起 lint/build/test/security，因为 Memory 的 Kind 枚举只有 gap/decision/lesson，checkpoint 的 GatesGreen 也只是一个不带细节的单一布尔值。提案是新增一个 KindGateResult memory 条目类型持久化每个 gate 的名称/状态/时间戳/git commit，并配一个按 gate 差异化 TTL（lint 1 小时、test 30 分钟、security 永不缓存）的缓存——与“增量 Gate 执行”类方向的区别在于，这里的缓存跨越的是不同 forge 调用/会话，而非同一次 evolve 运行内的迭代。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: forgeos-four-product-architect-frontiers-2026-07-10

**Agent 输出行为回归检测层（diff-test / contract / traceability）** `×1`

现有 gate 只验证代码结构（行数、分层、密钥、测试是否通过），从不验证 agent 产出的代码在行为上是否正确、是否符合 PRD。提案捆绑了三个可渐进采用的策略：forge diff-test（在 agent 改动前后 stash/pop 并重跑同一批测试以捕捉行为回归）、forge contract（一份声明式 CONTRACTS.md，为每个模块定义可执行的属性/集成测试断言）、以及 forge trace-requirements（基于关键词在 PRD 的 MUST 条款与代码/测试/注释内容之间建立可追溯性）。

*优先级信号*: unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: high-value-extension-v35

**Prompt 结构敏感性与 Ablation 测试框架** `×1`

Prompt 组装顺序/内容目前是一段硬编码的字节序列，而非一个可版本化、按模型分层、可测试的变量。提案是引入一个 PromptStrategy 抽象，为 prompt 结构增加回归快照检测，并新增一个 opt-in 的 --prompt-ablation 模式，对同一 phase 跑多个 prompt 变体并比较其 gate 通过率/迭代次数/成本，以数据驱动地找出更优的 prompt 结构——重点是主动比较多个变体而非仅防止漂移。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: next-five-architectural-frontiers

**Agent 执行器真实子进程 Spawn 测试盲区** `×1`

全仓测试套件从未真正 spawn 一个子进程来验证 agent 执行——CI 唯一的端到端测试是 --executor dry，而生产实际使用的 CommandExecutor 在超时处理、进程组清理、成本计算等行为上与 DryRunExecutor 有本质差异，却从未被集成测试覆盖，这意味着 loop-back、checkpoint 恢复、budget 守卫等机制的“端到端”验证事实上完全排除了真实子进程这一环。提案是引入一个真正 spawn echo/cat 的 FakeExecutor，在 CI 中加入使用真实 --executor command 的 echo 步骤，并为每项子进程交互机制建立至少一个真 fork+exec 测试。

*优先级信号*: P0(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-extension-five-novel-2026-07-10

**用户自定义闸门系统（User-Defined Custom Gate System）** `×1`

现有全部 8 种 gate 类型（test/lint/build/complexity/arch/security/secret/coverage）都硬编码在 harness 的 acceptance.mjs/adapters.mjs 中，没有任何机制允许用户或项目声明并注册一个带自定义运行脚本的 gate，而不必修改 forge-core 本身。该方向仅停留在需求阶段，设计流程未产出方案（design-pipeline-failed）。

*优先级信号*: P1(x1)　·　*最高成熟度*: design-stage-failed　·　*示例来源*: 2026-07-11-five-codegrounded-architectural-extension-directions

**Workflow SDLC（Schema 版本化 + 声明式测试框架 + CI 验证钩子 + A/B 对比）** `×1`

提案给 asset.Workflow 增加 SchemaVersion 字段与迁移守卫，在 workflow YAML 中新增声明式 test_suite: 测试块并配一个 forge workflow test <name> 运行器，把 forge validate --workflows 接入 CI 以检查版本偏移与测试通过情况，再新增 forge run --compare-workflow 用于对比两个 workflow 版本的收敛率/成本/信任得分——是一个把 schema 治理、声明式测试、CI 钩子与 A/B 对比捆绑在一起的更大范围提案，而非单一测试机制。

*优先级信号*: P3(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-directions

**统一自测试编排入口（forge self-test --ci）** `×1`

当前测试入口高度碎片化（go test ./...、node harness/acceptance.mjs、forge validate --models 各自独立运行、各自输出格式），提案新增一个 cmd/forge/self_test.go 统一入口，复用 acceptance.mjs 的裁决逻辑把这些入口聚合为单一的 PASS/FAIL/NA 裁决，并提供一个跳过完整 Go 测试套件的 --quick 模式。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: five-systemic-oversights-v45

**自适应测试自愈闭环（Adaptive Test Self-Healing Closed Loop）** `×1`

converge.go 的 CodeTestRatio 信号目前只在 roadmap 完成度较高且比例过低时打印一条警告，从不触发任何实际动作，导致 implementer 在下一次 loop-back 时完全不记得上次为什么测试不足。提案是构建一个三阶段的 Test-Health Monitor：一个能区分 flaky/断言过时与真实实现 bug 的 test-diagnoser agent、一个据此路由到 implementer 或 QA 的补救阶段、以及一个确认 PASS 且 CodeTestRatio 确有改善的验证阶段，并设 --max-heal-attempts 上限与重复运行的 flaky 检测。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-direction-analysis

**WASM 可移植 Gate 引擎** `×1`

harness 的多语言 adapter 在宿主机缺少对应工具时会让该 gate 降级为 N/A——目前 ForgeOS 自身就有 5/14 个 gate 处于 N/A 状态，而 N/A 对 agent 而言与 PASS 几乎无法区分，被认为是“最危险的信号类型”。提案是用零外部依赖的 wazero WASM runtime 预编译各语言工具模块，使 gate 执行脱离宿主环境依赖，把本应是 N/A 的信号转化为真实的 PASS/FAIL，并建议先用 eslint 的 WASM 版本做一个 sprint 的 POC 验证可行性。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: scan-current-gaps

**多语言 Adapter 工具链输出契约标准化** `×1`

go.yml/python.yml/typescript.yml 三个 adapter 各自定义了完全独立的工具链，输出格式互不相同，导致 acceptance.mjs 必须为每种工具单独手写解析逻辑，且工具版本升级会让解析器静默过时，新增语言会进一步放大这个问题。提案是在“标准化 JSON 输出契约”（较小改动）与“WASM 统一执行层”（更大改动，与 WASM Gate 引擎方向相关联）两条路径中二选一，以降低 adapter 解析层随语言数量线性增长的维护成本。

*优先级信号*: non-urgent(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: scan-fresh-perspectives

**确定性回放与回归测试框架（Deterministic Replay & Regression Framework）** `×1`

trace.jsonl 记录了完整的事件流，但没有任何回放引擎可以用旧 trace 重放 Engine 来验证策略/harness 变更后的行为是否保持一致——诸如 MaxLoopBack 调整、mode gate-set 过滤规则变化这类策略变更，目前只能靠真实 agent 调用（烧真实预算）或纯代码评审判断是否改变了预期行为。提案是实现一个 Replay(tracePath, eng) 函数比对实际裁决与 trace 记录裁决，把此前真实 claude 跑产生的 trace.jsonl 作为版本化的 testdata/traces/ fixture，并支持从 checkpoint 恢复后重放 N 次迭代来验证 prompt 变更的影响。

*优先级信号*: high(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-expansion-perspectives

---

### Memory质量衰减/生命周期管理 `memory-lifecycle`

31 个独立主题，原始条目 110 条。

**Memory 条目级 TTL 与冷热分层生命周期管理** `×14`

Proposes giving each memory.jsonl entry an explicit TTL/expiry field plus a hot(ephemeral)/cold(persistent) tier split, so Load-time filtering and Compact automatically retire stale knowledge by age instead of today's pure recency/count-based keep-last-N policy. Frequently bundled with adjacent asks — non-LLM summarization of compacted blocks, auto-triggering Compact from the evolve loop, an inverted index, hot/cold storage separation — under the umbrella claim that memory quality degrades unboundedly over 200+ evolve iterations because nothing ever expires. This is the single most-repeated idea in the entire ideation corpus, independently reproposed under a dozen near-identical titles ('知识生命周期管理' / 'Knowledge Lifecycle Management' / 'Cross-Session Memory Lifecycle') across nearly every batch. Several out.md verifications found the 'consumption is completely unwired' framing partly false (memory IS loaded into prompts and auto-compacted every 10 iterations), narrowing the real surviving gap specifically to TTL/tiering.

*优先级信号*: P1(x8)/P2(x6)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-highvalue-extension-directions-v2, 2026-07-11-forgeos-five-codegrounded-product-expansion-directions, 2026-07-12-five-genuine-architectural-frontiers-senior-architect-pm, genuine-architectural-horizons-five

**Confidence 字段驱动排序/过滤/淘汰（价值感知记忆保留）** `×9`

Points out that memory.Entry already carries a Confidence field (and later Source) that is declared but essentially unused — Query never filters/sorts by it, Compact evicts by pure recency/count, prompt injection doesn't weight by trust — so a high-value architectural finding is retained no better than a throwaway status update. Proposes actually consuming Confidence for ranking (confidence-descending injection, minConfidence filters), a value-score (confidence x recency x source-weight) driving both Query ordering and Compact eviction, and — in the foundational version — literally introducing the Confidence/Supersedes fields (later found to already be implemented well beyond scope). Distinct from the TTL/tiering cluster because the retention criterion here is a semantic value/trust score rather than raw age.

*优先级信号*: P0(x1)/P1(x2)/P2(x4)/unstated(x1)/medium(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-edge-cases-and-extensions, 2026-07-11-forgeos-five-architectural-priority-extensions, 2026-07-11-forgeos-five-codegrounded-expansion-directions, second-order-architectural-gaps

**记忆矛盾/漂移检测与信任加权** `×9`

Argues the append-only memory store lets directly contradictory or superseded entries on the same Kind+Topic coexist silently — nothing flags when a new finding negates an older one, causing 'learn, forget, relearn' oscillation across long autonomous runs. Proposes syntax/keyword-heuristic contradiction detection (negation words, opposing decisions), source-trust-weighted ranking by agent role, staleness annotations, and — in the most fleshed-out architected version — dedicated contradiction.go/retro.go/delta.go modules running in observation-only mode that tag entries with warnings rather than auto-deleting. One of the most consistently re-proposed ideas across independent docs, with multiple real architected designs.

*优先级信号*: P0(x1)/P1(x2)/P2(x4)/unstated(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-extension-directions-architect-product-perspective, expansion-five-uncovered-2026-07-10, forgeos-five-unseen-structural-gaps, fresh-scan-2026-07-11

**运行时状态文件(trace/memory/scorecard)体积治理：轮转/归档/配额** `×7`

Observes that trace.jsonl, memory.jsonl, and scorecards.json all grow without any size cap, rotation, or archival — unlike checkpoint.go which already supports a retain=N parameter that evolve.go never actually wires up — so a 24h+ unattended run can accumulate tens or hundreds of MB with no cleanup path and increasingly slow linear scans. Proposes a configurable retention-policy YAML block (state_management), a forge state/gc/doctor-storage command family (info/prune/archive/rotate), size-triggered gzip rotation, and disk-usage warning events. This is a raw file/operations-level concern distinct from the memory-content-semantic themes above, which are about which knowledge entries to keep rather than how the underlying files are managed.

*优先级信号*: P0(x1)/P1(x2)/P2(x2)/high(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-10-five-operational-frontiers, 2026-07-11-five-structural-debt-and-product-frontiers, 2026-07-12-five-verified-direction-tl-analysis, production-operational-gaps

**自动 Compact 未接入 evolve 循环的争议性声称** `×7`

A cluster of documents claiming memory_compact.go's Compact() routine exists but is never invoked automatically by forge evolve, forcing manual `forge memory-prune` while the store grows unbounded; proposals include wiring Compact into LoopEngine.OnIteration, adding a Config/CompactPolicy struct, and switching the trigger from iteration-count to entry-count. Distinctively, most entries here were subsequently DEBUNKED by their own out.md verification — compactMemoryIfDue is in fact already called every 10 iterations inside evolve.go, and several docs' supporting code-comment citations don't exist in the codebase at all — making this the most-frequently-refuted recurring claim in the corpus, though a couple of variants (configurable/concurrency-safe triggering, entry-count-based triggering) remain legitimate refinements.

*优先级信号*: P1(x2)/P2(x2)/unstated(x2)/cross-cutting(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-four-uncovered-architectural-gaps, 2026-07-11-four-uncovered-architectural-extension-directions, 2026-07-12-five-global-scan-engineering-product-expansion-directions, forgeos-five-architect-product-expansion-2026-07-10

**跨运行/跨会话知识继承 (forge memory import)** `×7`

Notes that every new forge run/evolve session starts with a completely empty memory store — prior runs' accumulated decisions and lessons are simply lost — and proposes an explicit, opt-in inheritance mechanism (a `forge memory import --from <prev-run-dir>` command or a `--load-memory N` session flag) paired with a configurable TTL/max-entries retention window (commonly ~30 days) so imported knowledge decays appropriately. More developed architected variants add semantic/episodic/programmatic entry classification and session snapshot/restore with dual-dimension (time + context-match) decay. Distinct from the cross-run *isolation* theme below — this is about deliberately propagating knowledge forward, not preventing accidental cross-contamination.

*优先级信号*: P1(x1)/P1.5(x1)/P2(x5)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-deep-global-scan-extension-directions, 2026-07-12-five-global-scan-post-closure-extension-directions, expansion-five-product-blindspots, genuine-expansion-gaps

**基于时间/证据的置信度自动衰减** `×6`

Argues memory.Entry.Confidence is set once at creation and never updated except via explicit Supersedes, so knowledge later contradicted or invalidated by new evidence — or simply aged past relevance — keeps its original high confidence indefinitely and keeps misleading prompt injection. Proposes a decay function (decayFactor = max(floor, 1 - age/halfLife)) applied during Compact/Load, a post-iteration reconcile step checking entries against current-iteration signals (gate results, trace events) to auto-decay confidence when contradicted, token-budget-triggered compaction, and — in the most elaborate architected version — a full internal/learning package (Attribution + keyword-Jaccard Influence scorer) closing the loop from 'lesson written' to 'agent behavior observed to change' to 'confidence adjusted'.

*优先级信号*: P0(x1)/P1(x1)/P2(x1)/P3(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-architectural-product-extensions-codegrounded, 2026-07-11-genuinely-uncovered-frontiers, architect-product-perspective-five-directions, expansion-five-systemic-architectural-gaps

**结构化知识挖掘/学习闭环引擎 (internal/learn 或 internal/knowledge 包)** `×6`

Proposes building a dedicated distillation layer on top of the raw trace/memory/scorecard logs — an internal/learn or internal/knowledge package with a pattern miner, cross-session correlator, and anti-pattern detector, exposed via `forge learn patterns/correlate/suggest` commands — so recurring patterns (e.g. which model performs best per task type) become evidence-backed, agent-consumable suggestions instead of three permanently disconnected raw logs. Architected variants scope this down considerably: a minimal ~20-line harvest.go appending a lesson whenever a quality gate FAILs, or LoopEngine-injected KindAdaptiveHint/KindAntiPattern hints derived from simple failure-distribution aggregation — explicitly rejecting a full mining engine as premature given current run-count.

*优先级信号*: P1(x2)/P2(x4)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-next-architect-frontiers, high-value-extension-directions-v2, 2026-07-12-high-value-extension-directions, five-code-grounded-architectural-gaps-2026-07-11

**Compact/Append 并发竞态导致静默数据丢失** `×4`

Identifies a concrete concurrency bug: memory.Compact()/Prune() read the whole store as a snapshot, process it, then atomically rewrite the file via temp+rename, while memory.Append() writes independently via O_APPEND — if an Append lands between the Compact snapshot read and the rewrite, that entry is silently overwritten and permanently lost. Confirmed not reachable within a single serial evolve process today, but a real risk for any future parallel/cross-process compaction (manual `forge memory-prune` racing a live `forge evolve`, or a parallel-evolve mode). Proposed fixes: a cross-process file lock during Compact, incremental LSM-tree-style compaction instead of full-snapshot rewrite, or an optimistic mtime/size check-and-retry before the rewrite.

*优先级信号*: P0(x2)/P1(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-codegrounded-five-systemic-gaps, 2026-07-11-five-deep-systemic-gaps

**Memory 写入去重（精确/模糊/语义近重复）** `×4`

Observes memory.Append performs zero deduplication, so the same gap independently rediscovered by planner/reviewer/implementer across dozens of iterations gets written as many near-identical entries, diluting the fixed injection cap with redundant noise. Proposes a fuzzy/simhash-based dedup layer applied at write-time (compare against recent entries, bump confidence instead of duplicating), an offline `forge memory dedup` cleanup command, and retrieval-time dedup on top-k results — plus, for the Gap-kind-specific variant, LastSeenAt/HitCount fields and automatic resolveGap when a later entry references the same topic.

*优先级信号*: P1(x1)/P2(x3)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-deep-systemic-gaps, 2026-07-11-forgeos-five-codegrounded-systemic-frontiers, novel-architectural-extensions-v40

**Memory Query() 主动检索原语接入 Prompt 装配** `×4`

Points out that memory.Query(kind, topic) — a targeted, filtered retrieval function — exists in code but has zero real call sites; instead memoryContext does an undifferentiated full-dump-then-truncate of the entire store into every prompt. Proposes wiring Query into prompt assembly via phase-declared memory_retrieve targeting or a strategy.go SelectForPrompt function (recency x kind x confidence weighted, consuming ~20% of the prompt token budget), turning memory consumption into an active, role/phase-specific pull rather than a fixed broadcast dump. The architected version was assessed by the architect review as the single highest-ROI change among five surveyed directions.

*优先级信号*: P0(x3)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-architectural-frontiers, architect-product-perspective-five-directions, 2026-07-11-forgeos-five-genuine-architectural-frontiers

**TF-IDF 语义检索接入 Memory + 增量加载 + LSM 式压缩** `×3`

Proposes wiring the existing TF-IDF retriever (currently used only for ADRs) into memory.Query so prompt injection is relevance-ranked rather than purely recency-based, replacing full-file reload on every access with incremental/streaming loading of only new entries since last checkpoint, and sharding memory storage by Kind with background LSM-tree-style compaction/merge instead of simple keep-last-N pruning. One variant also flags that the TF-IDF tokenizer has no CJK word-boundary handling, making Chinese-language memory entries effectively invisible to retrieval.

*优先级信号*: unstated(x1)/medium(x1)/high(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-code-grounded-expansion-perspectives, strategic-expansion-perspectives

**Memory 按 Workflow/Phase 命名空间隔离 + 衰减 + 去重（复合规格）** `×3`

A specific, recurring, near-verbatim ask (reproposed almost word-for-word across three independent docs) to add Workflow and Phase fields to memory.Entry, expose workflow-filtered Query so one workflow's findings don't dilute another's prompt context, apply automatic confidence decay by age/iteration count, and add fuzzy dedup keyed on (Kind, Topic, Workflow) — bundling namespace isolation, decay, and dedup into one concrete schema-change proposal rather than treating them separately. The architected pairing frames this as a 'minimum-invasion' first step before considering directory-sharded storage, explicitly rejecting a SQLite/embedded-DB rewrite.

*优先级信号*: P1(x1)/P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-systemic-blindspots, 2026-07-11-five-systemic-blind-spots

**Memory Entry Detail 字段无大小上限** `×2`

Points out that memory.Entry.Detail is a plain string with no length/byte-size validation at write time — only the prompt-injection layer caps entry COUNT (memoryCap=32), not any individual entry's size — so a single verbose agent write can produce a 50KB Detail field, and 32 such entries alone could reach 1.6MB, blowing well past any context window and wasting tokens. No concrete fix design was produced for either instance of this ask (both stalled at requirement stage).

*优先级信号*: P1(x1)/unspecified(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-deep-systemic-codegrounded-boundaries

**Confidence 字段零值序列化歧义 (omitempty 吞掉 0.0)** `×2`

Identifies a concrete data-model bug: memory.Entry.Confidence uses json:",omitempty", so an explicit 0.0 ('zero trust') is dropped on serialization, and the decoder unconditionally promotes any missing/zero value back to 1.0 ('full trust') for backward compatibility — meaning 'I do not trust this' can never actually be represented or survive a save/load round trip. This silently defeats the existing <0.3 'unverified' warning-prefix logic and would corrupt any future automated knowledge-verification agent trying to flag contradicted entries as untrusted.

*优先级信号*: P0(x1)/P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-12-five-code-verified-architectural-blindspots, five-verifiable-code-level-gaps

**跨运行 Memory 命名空间隔离(run_id)防串扰** `×2`

Warns that internal/memory is a single global, unnamespaced, append-only JSONL file shared by every forge evolve process on a project, so two concurrent runs (e.g. CI plus local dev) cross-pollute each other's findings — one agent gets biased by another run's unrelated progress, and convergence criteria could even be satisfied by another run's unrelated signals. Proposes tagging every entry with a run_id, defaulting Load to the current run only with an opt-in --load-all-runs flag for the learning loop, a memory_ttl_days retention window, and write-audit trace events. One architect review flagged this as potentially the costliest silent bug in the codebase and recommended promoting it to P0.

*优先级信号*: P1(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-12-forgeos-architect-product-five-expansion-directions, forgeos-five-architect-product-extension-perspectives-2026-07-10

**持久化产物 FormatVersion 校验与迁移框架** `×2`

Proposes an explicit FormatVersion check-and-migrate framework spanning all persisted artifacts (memory.jsonl, trace.jsonl, checkpoint.json, scorecards.json) so a schema-version mismatch fails loudly with a migration command, instead of json.Unmarshal silently dropping new fields or misinterpreting a renamed one. Validation confirmed some files (checkpoint) already stamp a `_format` marker on write, but nothing anywhere ever checks it against a supported range on Load — leaving the store vulnerable to silent, undetected drift across ForgeOS version upgrades.

*优先级信号*: P2(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: execution-semantic-gaps, execution-semantics-gap-analysis

**人工拒绝/修正意见转化为记忆学习信号 (forge reject)** `×2`

Notes that memory.Entry.Supersedes exists in the schema but is essentially never written to, and that human rejection markers (.forge/<stage>.rejected) only drive same-session loop-back — they never become a persistent memory learning signal, so a routing/design choice rejected once can be proposed again identically in a future session. Proposes a `forge reject <reason>` CLI (symmetric to forge approve) that writes a low-confidence Denied entry populating Supersedes, plus a routing negative-feedback hook (Router.ConsultHistory()) so a previously-rejected approach carries lower routing confidence in future sessions instead of being retried from scratch.

*优先级信号*: P2(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: expansion-horizon-three

**TF-IDF 相关性门控注入 + 知识冻结/归档 Kind + Staleness 信号** `×2`

Observes memory's Compact only groups/summarizes by time and Kind with zero semantic-relevance judgment, so an abandoned technical decision can occupy prompt budget indefinitely alongside genuinely current knowledge. Proposes reusing the existing TF-IDF retriever to relevance-gate which entries actually get injected (rather than just truncating by count), introducing an explicit 'frozen/archived' memory Kind for deliberately retired knowledge, filtering by sprint-boundary, and adding a new KnowledgeStaleness signal into converge.Signals so staleness itself becomes a measurable convergence input.

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-scan-five-codegrounded-systemic-frontiers

**知识策展管线：语义去重+矛盾检测+置信度衰减一体化** `×2`

Proposes a coherent 4-layer semantic-quality maintenance pipeline layered on top of the raw memory.jsonl store, explicitly distinguished from piecemeal fixes: advisory text-similarity dedup that never auto-deletes, sign-word-based contradiction detection that halves confidence on conflict, age-based confidence decay (30-day half-life), and a `forge memory-status` health dashboard command. The architected pairing implements this with a pure-stdlib TF-IDF+cosine-similarity semantic.go module (conservative >0.9 merge threshold, rollback via Supersedes) plus an age x confidence x kind-weighted Compact predicate.

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: genuine-five-product-architectural-frontiers

**Memory 全量无截断注入 Prompt（缺 TopK 截断，已修复）** `×2`

Flags that memoryContext originally loaded and injected every matching memory.jsonl entry into every phase's prompt with no ranking or truncation — at the 500-entry DefaultCompactThreshold this could inject roughly 10K tokens, catastrophic for narrow-context model tiers. Proposes replacing 'load all, inject all' with topK=32 truncation sorted by confidence-desc + recency-desc (configurable via an env var). Both instances of this claim note it was already fixed by prompt_memory.go's boundMemory() (8 most-recent + 24 BM25-relevant, capped at 32) by the time of review, documenting a regression that has since been resolved rather than an open gap.

*优先级信号*: P0(x1)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: forges-five-unbuilt-foundations, expansion-deep-analysis

**按 Agent 角色隔离 Memory 注入 (Per-Role Isolation)** `×2`

Adds role-based filtering to memory injection (memoryContext/boundMemory) so shared memory.jsonl entries written by one agent role (e.g. implementer) are not silently injected into a different role's prompt (e.g. reviewer) — closing a governance bypass of the project's 'fresh-context reviewer' red line at the memory layer, since memory.Entry.Source already records the originating role but is simply never consumed for filtering. The architected design adds a currentRole-based sourceFilter step inside boundMemory(), with an explicit share_with override for intentionally cross-role knowledge.

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-genuinely-unexplored-extensions

**跨 Sprint 战略记忆摘要注入** `×1`

Proposes closing the gap between scorecard/trace learning and forward planning by summarizing memory findings at sprint boundaries (reusing the existing memory_compact machinery) and injecting structured, time-decayed lessons — which architecture calls proved wrong, which test strategies worked — into the next evolve iteration's prompt context and routing decisions, instead of leaving memory as a purely single-session append-only log with no higher-level periodic distillation.

*优先级信号*: unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-forgeos-five-genuine-architectural-frontiers

**Memory 查询/压缩线性扫描性能债 (索引化)** `×1`

Points out that memory.Query() and Compact() are pure O(n) linear scans with no index or Kind/Topic partitioning, so a long 24h evolve loop pays repeated full-scan cost on every single phase's prompt assembly. Proposes kind-partitioned storage, a topic inverted index, and LRU caching of hot topics as low-risk performance optimizations, distinct from the storage-rotation and lifecycle themes since this is purely about query/compact algorithmic cost rather than what gets retained.

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-code-grounded-architectural-gaps-2026-07-11

**组织级跨项目知识平面 (Knowledge Plane)** `×1`

Elevates memory from a purely per-project auxiliary store to an organization-level plane via a $FORGE_HOME/memory/global/ directory with LoadGlobal/AppendGlobal APIs and forge publish-pattern/subscribe CLI commands, plus a semi-automatic publish workflow where the system flags high-confidence, gate-passed patterns as promotion candidates and a human or fresh-context reviewer confirms before they propagate — deliberately guarding against cross-project knowledge pollution while still letting validated lessons travel between projects.

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-highvalue-governance-evolution-extensions

**Checkpoint↔Memory 分层恢复一致性契约 (MemorySeqRange)** `×1`

Proposes adding a MemorySeqRange[start,end] pointer to checkpoint records marking the exact memory.jsonl entry range corresponding to that iteration, so a resume operation can read only the relevant slice instead of linearly scanning the entire memory file; if a checkpoint is usable but its corresponding memory range is missing (e.g. disk corruption), the system should degrade gracefully to a WARN-level minimal-semantic-summary recovery rather than silently failing or resuming with stale/incomplete knowledge.

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-directions

**跨会话置信度校准与漂移追踪 (Confidence Calibration)** `×1`

Observes that requirement_confidence and other agent-reported confidence scores are consumed at face value with zero cross-session calibration — nothing checks whether an agent's stated confidence has historically correlated with actual outcomes. Proposes a new KindCalibration memory type, a ConfidenceTracker recording calibration history across sessions, drift-detection logic, and injecting the resulting calibration vector back into both prompt context and the converge pipeline so future confidence claims from the same agent/role can be trust-adjusted based on track record.

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-closure-gap-expansion-directions

**Mode 感知 Scorecard 维度 + Memory 置信度阈值裁剪** `×1`

A hybrid proposal noting that scorecards.json keys on (model, task_type) with no mode dimension, so explorer-mode runs (minimal gate set, inflated quality_score) pollute routing decisions made in balanced/engineering mode; bundles this with a memory-side ask to add a Confidence field to Entry (default 0.5, filterable below 0.3) and a `forge memory-prune --min-confidence` command. The memory-confidence portion substantially overlaps with the Confidence field already implemented and documented elsewhere in the corpus, suggesting this proposal's premise about memory lacking Confidence was stale even at the time of writing.

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-strategic-v5

**跨项目 Workspace 级学习 (Scorecard 合并 + Memory Scope 提升)** `×1`

Proposes a `forge scorecard merge` command that filters and weighted-averages scorecard statistics across projects by task_type, extends memory.Entry with a Scope field (project/global/task_type:go), a `forge memory promote` command to elevate entries from project-local to global scope, and a cold-start seeding mechanism (forge scorecard seed) — giving routing decisions access to cross-project historical data rather than starting cold on every new project.

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: fresh-five-systemic-extensions-2026-07-10

**ADR 写入触发 Context Cache 失效** `×1`

Adds a Phase.WritesADR bool YAML annotation that automatically triggers ContextCache.Invalidate() after a workflow phase writes an ADR, closing a currently-latent (not yet actively exploited) staleness risk where the prompt context cache could keep serving pre-ADR content since no workflow phase today actually sets writes_adr. A narrowly-scoped cache-consistency fix rather than a general memory-lifecycle policy.

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: five-code-grounded-architectural-extensions-2026-07-10

**跨项目模式库 (Pattern Library with Path Abstraction)** `×1`

Proposes adding Topic/Tags fields to memory.Entry plus a path-abstracting Patternizer that converts project-specific paths into abstract tags so lessons can be shared across projects, combined with confidence-driven auto-pruning, topic conflict detection, and a `forge memory query` CLI wired into appendFeedbackLanes — explicitly framed as the starting point of a cross-project 'data flywheel' pattern library, distinct from the more governance-heavy Knowledge Plane proposal in mechanism (implicit path-tag abstraction vs. explicit publish/subscribe workflow).

*优先级信号*: P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: five-genuinely-unexplored-code-level-architectural-expansions

---

### 预算治理/成本策略引擎 `budget-cost-governance`

30 个独立主题，原始条目 95 条。

**预测性运行成本/时间估算引擎(本类目最高频核心主题)** `×17`

这是本类目中被独立重复提出次数最多的主题(至少17篇不同文档各自提出),核心洞察高度一致:预算护栏目前完全是花到了才停的被动止损,而 trace.jsonl 与 scorecard 早已积累每phase的时长/成本/模型历史数据,却从未被读回用于'这次运行大概要花多少钱/多久'这个问题。具体落地方式收敛为 forge run/evolve --dry-run / --dry-cost / forge estimate 命令,按 (model, task_type, mode) 分桶给出带置信区间的成本/时长区间(P50/P90、3σ异常过滤、样本不足时诚实标注低置信度),并接入 checkRunBudget 作为预警或超出倍数阈值时的硬性中止。最完整的架构化版本还加入了可选的 Calibrator(基于 scorecard 数据自动校准路由分档阈值,仅建议不自动生效)、按(phase,agent,tier)的成本查表并通过 forge preflight build 暴露、以及聚合 trace.jsonl 的 forge cost CLI。

*优先级信号*: P1(x10)/P2(x2)/P0(x2)/P3(x1)/P2-P3(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-structural-extension-directions-architect-pm-combined, 2026-07-11-five-unbuilt-product-experience-layers, five-systemic-oversights-v45, next-five-architectural-frontiers

**阶段/Wave级预算预留与隔离** `×7`

串行与并行执行下所有phase共享同一全局 agent-call/美元计数器,没有per-phase预留机制,导致前置的producer phase(planner/implementer)可能耗尽整个运行预算,饿死后置的consumer phase(reviewer/qa);并行RunParallel下问题更严重,因为一个wave内的并发phase既无公平性保证也无并发上限。反复被提出的修复是 PerPhaseBudget/WaveBudget 预留机制,基于已有的 MaxAgentCalls/BudgetExhausted 基础设施预分配并在用不完时退还共享池;最完整的架构化版本(Parallel Resource Governance Framework)还加入了并发槽位的 ResourcePool 与运行级 BudgetLedger。

*优先级信号*: P1(x4)/P2(x2)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-architectural-product-extensions-codegrounded, 2026-07-11-forgeos-five-architectural-priority-extensions, 2026-07-11-forgeos-four-codegrounded-architectural-expansion-directions, 2026-07-11-four-codelevel-extension-directions-after-four-deep-scans

**预算水位线梯度降级(Budget Watermark)** `×5`

当前预算耗尽处理是纯二元硬停(BudgetExhausted() 一旦为真立即中止),会导致预算最后一小段被浪费搁置(例如仅剩$1却下一次opus调用要$0.35+),且操作者事先毫无预警。提议设置分级 BudgetWatermark:剩余约30%时收紧prompt上下文、约20%时降级模型档位、约10%时削减重试/跳过可选phase、约5%时通知操作者,且这些降级须在预算回升后可逆;其中一个综合方案进一步扩展为带价值加权phase优先级与跨迭代预算借用的完整预测调度器。

*优先级信号*: P2(x4)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-runtime-boundary-blindspots, genuinely-uncovered-five-deep-runtime-gaps, expansion-five-uncovered-2026-07-10, expansion-direction-analysis

**全局墙钟运行时间预算(缺失的第五维护栏)** `×5`

ForgeOS 已有四个资源护栏维度(agent 深度、调用次数、美元预算、单agent超时),唯独缺少针对整次 forge run/evolve 调用的总墙钟时间上限——若单agent超时未设或重试/loop-back反复叠加,一次运行理论上可以跑上数天,而 duration 目前只能事后记录。各提案趋同地要求一个 --max-wall-clock/--time-budget/--timeout 旗标,在每次迭代/phase边界检查,基于既有取消 context 包一层 context.WithTimeout 使运行优雅失败;最完整的架构化版本还加入按固定/自适应/比例策略分配的每phase时间预算与 budget_allocation/warning/exhausted trace事件。

*优先级信号*: P1(x3)/P0(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-product-expansion-directions, forgeos-five-operational-maturity-frontiers, novel-architectural-extensions-v40

**策略即数据:运行时PDP抽象(超越预算范畴)** `×5`

路由阈值、风险下限、mode×lifecycle 矩阵目前都是编译期硬编码的 switch/const 逻辑(如 mode.go 的 Effective()、routing.go 的 modeDefault map),没有可追溯的决策链或覆盖历史。各提案程度不一:从给既有硬编码 switch-case 打决策日志、加可追溯覆盖链,到把阈值外部化为内嵌JSON或YAML策略并配 forge validate policy 自检与 --dump-decision-chain 调试旗标,再到完整的 PolicyLoader 接口把 .agent/policies/*.yml 移出硬编码 Go 代码——均明确以'避免引入 OPA/Rego 这类过度设计'为前提(其中专门评估 OPA/Rego 的那一篇最终结论是暂缓引入),以维持零外部依赖红线。

*优先级信号*: P1(x1)/P2(x2)/P3(x1)/暂缓(x1)　·　*最高成熟度*: architected　·　*示例来源*: five-code-grounded-architectural-gaps-2026-07-11, forgeos-five-unseen-product-architect-extensions, forgeos-four-structural-gaps-2026-07-10-scan, global-scan-five-codegrounded-extension-directions

**预算降级-质量螺旋断路器与可观测性** `×4`

预算逼近上限时 BudgetAdjustTier 会静默把未受保护的 agent 降到更便宜的模型档位,导致输出质量下降、触发更多 reviewer REQUEST_CHANGES 循环、消耗更多预算、加速再降级——这是一个没有 DecisionEvent 记录、没有检测、没有自动升级人工介入触发的正反馈螺旋,可能在无人值守24h运行中悄悄跑完全程。各提案从最基础的'降级时打一条 DecisionEvent 日志',到在路由层检测与质量下滑相关的单调降级序列,再到连续N次同一降级模型触发循环时暂停并要求人工介入的完整断路器,程度不一。

*优先级信号*: P1(x4)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-unseen-systemic-operational-gaps, five-uncovered-operational-gaps-2026-07-10, agent-orchestration-five-novel-perspectives

**声明式预算治理引擎(budget.yml 策略)** `×4`

预算策略目前只能靠 --max-budget-usd/--max-agent-calls 等 CLI 旗标表达,组织缺乏一种可审计的声明式方式来表达'安全/支付类任务无论预算压力都强制 Opus 且不可降级'或'预算最后20%只能用 Haiku/Sonnet'这类成本-风险权衡规则。提议引入受版本控制的 .agent/policies/budget.yml(或 project.yml 预算段),由新的 internal/budget 包解析并反哺现有 BudgetAdjustTier/checkRunBudget,采用枚举式task_type匹配而非图灵完备规则引擎(明确否决 OPA/Rego)。一轮评审将其从 P1 上调至 P0,视为企业级采纳的硬性前提。

*优先级信号*: P1(x3)/P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-high-value-extension-directions, high-value-extension-directions-v2, five-high-value-extensions-v44

**组织/项目级跨运行成本治理与分摊** `×4`

现有预算控制完全局限于单次运行范围,没有跨运行、跨团队、跨组织的成本核算能力。提议引入声明式项目级预算面(月度/冲刺硬上限与80%阈值软告警)、按项目/phase/agent/model的成本归因字段、纯基于已有本地 trace.jsonl 聚合(无需新服务器/数据库)的 forge cost report/--team CLI、基于规则的成本异常检测,最完整的版本还加入跨周期滚动预算(结转/累积/借用策略)与 webhook/Slack 告警集成。

*优先级信号*: P2(x3)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: expansion-production-perspectives, forgotten-frontiers-five

**质量信号驱动的分层路由深化** `×4`

模型档位路由(TierFor/BudgetAdjustTier)目前做成本驱动的降级决策时完全不参考质量信号,尽管 scorecard 的 PassRate/ReviewerScore 数据早已被采集却从未被消费。各提案分别提议:给 BudgetAdjustTier 加入历史质量地图,使成本驱动降级不会跌破质量下限、且 PassRate 骤降时自动升级档位;把已构建但被闲置的多维评分器(复杂度、风险、token上下文、历史)通过一个失败即回退安全档位的 Scorer 接口接入实时 TierFor 路径;以及为 Scorecard 扩展 ReviewerScore/RegressionRate 与覆盖率/lint密度/复杂度增量/架构违规等多维 QualityMetrics,合成加权 quality_composite 驱动 HistoryTiebreak。

*优先级信号*: P0(x2)/P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: expansion-five-systemic-learning-loop-gaps, five-code-grounded-architectural-extensions-2026-07-10, five-code-grounded-architectural-gaps-2026-07-11, five-genuinely-unexplored-code-level-architectural-expansions

**[疑似虚构/离题]本地配额限流与调度引擎** `×4`

四篇提案(本地 token-bucket/滑动窗口 QuotaTracker、异步队列+配额感知调度器、按分钟/小时/天统计消耗的 QuotaManager、本地计数器+服务端确认同步的智能配额管理器及其 Go Infer(ctx,opts) 接口草图)均被源头流程自身标记为虚构——不引用任何真实 ForgeOS 文件或机制,其中一篇甚至引用了虚构的文件路径,另一篇的 impl-plan 阶段产出为空内容。收录仅为完整性,应视为噪声而非真实落地需求。

*优先级信号*: N/A(x4, 标记为虚构)　·　*最高成熟度*: architected　·　*示例来源*: genuinely-uncovered-five-frontiers, high-value-extension-directions, high-value-extension-directions-v2, high-value-extension-directions-v3

**分层感知的Prompt预算与上下文裁剪** `×3`

每个 phase 目前都从零重建并重发完整 prompt,不复用 Claude 的 cache_control 缓存,且 opus 档位与 haiku 档位收到完全相同的上下文payload,存在约15倍成本差却未做区分。提议把 adrTopK/taskCap/memoryCap 等改为按模型档位(haiku/sonnet/opus)分级的函数,并通过角色卡外部片段文件差异化指令,同时接入 prompt-caching 复用重复上下文块。架构化版本给出了具体分级数值并预计降低 30-50% LLM 调用成本。

*优先级信号*: P2(x1)/medium-high(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-code-grounded-expansion-perspectives, fresh-expansion-perspectives

**厂商无关的每调用美元成本护栏** `×3`

当前 --agent-max-budget-usd 只是被透传成 claude CLI 自己的 --max-budget-usd 参数,对任何非 claude 的 --agent-cmd(自定义CLI、测试桩、未来多厂商路由)会被静默丢弃、完全不生效,使美元成本成为五个资源护栏维度中唯一依赖底层 agent CLI 自觉遵守而非由 forge-core 独立强制执行的一项。提议在 CommandExecutor.Build 与 Execute 之间拦截、在子进程启动前独立校验成本上限,复用已定义但从未使用的 PhaseBudget{MaxCostUSD, CurrentCostUSD} 结构体作为强制执行的数据模型。

*优先级信号*: P1(x2)/P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-runtime-boundary-blindspots, genuinely-uncovered-five-deep-runtime-gaps

**成本-收益归因与ROI分析层** `×3`

现有成本设施只能执行预算上限,无法回答'每个 roadmap 条目花了多少钱''不同 mode 的成本/质量权衡如何''成本效率趋势如何'这类决策支持问题。提议在 scorecard/收敛报告中加入 cost_per_roadmap_item、cost_per_iteration、efficiency_trend 等归因维度,较完整的版本封装为独立的 internal/roi 包,产出估算-实际偏差报告并对不可量化的业务价值明确标注置信度。

*优先级信号*: P2(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-directions, five-uncovered-architectural-frontiers, fresh-five-systemic-extensions-2026-07-10

**主动成本引导的执行调度决策** `×3`

成本治理目前完全是被动式的——checkAgentBudget/checkRunBudget 只在花费已跨过阈值后才拒绝phase,没有任何事前成本估算来回答'这次 reviewer 调用值不值这$0.50'。提议基于 scorecard 历史均值构建 phase 级成本预测器,驱动按phase类型的运行级预算分配,以及 agent-card 声明式的降级回退路径(opus→sonnet→skip);架构化版本封装为 CostPredictor + BudgetOptimizer 二元组做背包式的预算内优化,先从简单的'临近耗尽即降级'规则起步,再投入完整历史均值预测。

*优先级信号*: P1(x2)/P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-uncovered-architect-product-extensions-2026-07-10, forgeos-five-unseen-structural-gaps

**门控执行成本策略(gate fast-fail 重排序)** `×2`

提议按历史耗时对 required_gates 重排序(快门在前、慢门在后)并在便宜的门已经失败时提前熔断,不再等全部门跑完;同时对纯文档类等低风险 diff 做变更感知跳过(跳过 lint/test/build)。目的是压缩 loop-back 反馈延迟与 CI/LLM 成本。已落地的 D4 版本记录 gate 耗时百分位到 trace/scorecard 驱动排序,预计节省 20-40% 成本,并已 coded-and-reviewed。

*优先级信号*: P2(x2)　·　*最高成熟度*: coded-and-reviewed　·　*示例来源*: 2026-07-10-genuinely-novel-architect-perspective, 2026-07-11-five-adoption-gating-product-trust-gaps

**并行Wave取消的成本核算与追踪** `×2`

当 RunParallel 因一个 phase 失败而取消整个 wave 时,其他仍在执行中、被丢弃的 phase 已经发生的 LLM 成本既没有记入 trace,也没有作为浪费预算被清晰上报。提议增加结构化的 'aborted' trace事件类型并回滚未用完的调用配额;架构评审明确否决了完整的预留/确认(reservation/commit)记账模型,认为对实际并发规模是过度设计,改为在wave级别记录被取消phase数量,以 '潜在成本损失' 警告日志形式呈现。

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-gaps, 2026-07-11-five-pipeline-integrity-and-security-gaps

**阶段执行中的实时成本可见性反馈** `×2`

成本/预算跟踪目前完全是内部黑盒——runBudget.feed() 内部累积花费,只有在 BudgetExhausted() 被查询或迭代结束后才会暴露,用户在 phase 执行过程中完全看不到实时花费。提议新增 SpentSoFar()/PercentUsed() 查询接口以及每次迭代的成本摘要输出行,把当前盲目的硬性预算中止变成透明的实时反馈。

*优先级信号*: P2(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-codegrounded-architectural-extension-directions

**统一预算核算数据模型(CostEvent/BudgetState)** `×2`

checkAgentBudget 目前只是一个整数调用计数器,BudgetExhausted() 是一个不透明的布尔闭包,导致调用次数检查可能通过、而真实美元花费早已超过 --max-budget-usd,因为调用数与美元成本是两套互不等价的强制执行单位。提议引入结构化的 CostEvent{Kind, PhaseName, AgentCmd, ModelTier, CostUsd, Calls, DurationMs, Reason} 和/或 BudgetState 接口(Remaining/SpendRatio/ProjectCost/Charge/Exhausted),给 routing.BudgetAdjustTier 提供前瞻性成本数据而非仅事后硬停,nil BudgetState 保持现状无限预算行为不变。

*优先级信号*: P0(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-gaps, 2026-07-12-five-unseen-systemic-operational-gaps

**工作流效率分析与自适应优化引擎** `×2`

现有的成本/延迟遥测数据从未被转化为可决策的效率报表。提议新增 forge analyze 命令,输出每phase的成本效率,检测连续多轮零净变化的'冗余phase'并给出advisory告警,基于历史数据推荐 --run-budget-usd 取值,并支持声明式 skip_if 条件自动跳过无产出phase;配套变体还主动给出具体的预算-质量权衡建议(如'把 implementer 降到 sonnet 可省 $0.42')。

*优先级信号*: P2(x1)/unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-uncovered-architectural-frontiers, fresh-five-systemic-extensions-2026-07-10

**无人值守信任曲线:置信区间成本预估+确认闸门** `×2`

为了让操作者放心让 forge evolve 无人值守跑24h,提议提供一个预运行模拟(forge plan/preflight),基于 scorecard 历史给出带诚实置信区间的成本/时长估算(如'$2.40±$1.20,基于3次历史运行',样本数<5时回退到硬编码定价并标注低置信度),搭配一个开始高成本运行前的 --confirm 确认闸门,以及用分级warn-then-kill阈值替代当前对超时/输出体积的二元崩溃式护栏。

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: genuine-five-product-architectural-frontiers

**[疑似虚构/离题]结果缓存层** `×2`

两篇提案(通用 LRU/哈希键结果缓存,以及基于 SQLite/Redis、按 prompt+参数+模型版本键控的 CacheLayer)意图通过缓存 agent 调用结果削减 API 调用量,但都被源头流程明确标记为未扎根于 ForgeOS 实际的 prompt/cache.go 机制,与该批次真正的五个方向无关。

*优先级信号*: N/A(x2, 标记为虚构)　·　*最高成熟度*: architected　·　*示例来源*: high-value-extension-directions, high-value-extension-directions-v2

**Provider速率限制/用量配额账户管理(区别于美元预算)** `×2`

与美元成本预算不同,这两篇是扎根真实代码的提案,针对的是供应商自身的速率限制窗口(如观察到的5小时滚动调用上限):一个是轻量级的 forge preflight --check-quota 探测,在批量运行开始前报告配额是否将耗尽并据此预先拆分大批量任务;另一个是更完整的持久化 internal/budget 包(account/forecast/guardian),跨运行追踪累计用量、对照供应商滚动窗口预测剩余可用调用数,以零依赖 jsonl 持久化并通过 forge budget status/set-limit 暴露,明确定位为对现有单次调用美元护栏的补充而非替代。

*优先级信号*: P2(x1)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: high-value-expansion-directions, high-value-extension-v35

**运行前资源维度交叉校验(Resource Feasibility Preflight)** `×1`

forge preflight 目前只检查环境就绪(workflow 能解析、python3 存在),从不交叉核对五个独立资源护栏(美元预算、调用次数、超时、嵌套深度、输出字节)彼此是否矛盾,也不核对是否够workflow声明的phase数用,导致用户可能带着数学上不可能完成的 --timeout/--run-budget-usd 组合启动运行,直到中途才发现。提议分三阶段实现:crossCheckBudgets() 维度冲突预警、converge.CanConverge() 静态可达性预检、以及基于 scorecard 历史的成本/时间预测。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-architectural-extension-gaps-deep-scan

**迭代级预算的边际收益自适应分配** `×1`

forge evolve 的每次迭代目前分配相同的资源预算(MaxAgentCalls、MaxLoopBack、完整phase序列),不考虑边际收益递减,现有 NoProgress 熔断也只能检测零进展,检测不到缓慢漂移或震荡。提议跟踪 RoadmapCompletion 的进展轨迹,在边际收益低于阈值时自适应下调预算/模型档位或加速收敛判定。

*优先级信号*: P3(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-five-systemic-architectural-gaps

**成本遥测缺失阶段类型/角色维度** `×1`

trace.Event 目前只记录 Model 与总 CostUsdMicros,没有 AgentRole、PhaseType 或 WorkflowStage 维度,导致长时间运行结束后完全无法回答'钱究竟花在哪类phase/角色上'这类问题,只能看到总额。该提案仅停留在需求阶段,未产出设计方案。

*优先级信号*: P2(x1)　·　*最高成熟度*: design-pipeline-failed　·　*示例来源*: 2026-07-11-five-deep-systemic-gaps

**类型化治理信号平台(Signal[T]/PolicyRule)** `×1`

成本效率、真实性偏差、评审状态等治理信号目前分散在 cost.go、scorecard_wind.go、route.go、converge.go 里,用零散的字符串/布尔值表达,缺乏统一的关系与趋势语义。提议新增 internal/governance 包定义 Signal[T]{Kind,Value,Confidence} 与 PolicyRule{Condition, Action: ALLOW/WARN/BLOCK/ESCALATE} 作为统一抽象,配套产品层的 forge report cost --by-iteration/--compare 与 forge route --cost-advice。

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-directions

**经济治理层:ROI阈值与优先级降级** `×1`

把预算控制从单一的花费/上限比率硬阈值,升级为价值感知模型:在 modes.yml 中声明 ROI 阈值与显式的'停机成本',并按优先级做优雅降级——例如 P0 安全评审即使超预算也必须跑,P3 的 lint 可以被跳过——同时积累跨会话的 ROI 统计以指导未来的预算设定。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: next-five-frontiers

**Scorecard冷启动引导先验数据** `×1`

新项目的 scorecards.json 起初为空或近乎为空,导致依赖历史数据的 HistoryTiebreak 路由决策在积累足够真实运行数据前无据可依。提议让 forge-init 在初始化时写入若干条(如7条,samples=25)引导性先验 scorecard 条目到 .agent/routing/scorecards.json,并在 scorecard 条目少于5条或全为先验值时在 forge run 中显示冷启动警告横幅,同时文档化 HistoryTiebreak 的回退行为。

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: five-code-grounded-architectural-gaps-2026-07-11

**变更影响驱动的执行策略降级** `×1`

risk.FromChangedPaths 目前已经计算出变更风险信号,却只被打印日志、从未被真正消费。提议把它接入 Engine.RiskSignals,使纯文档/测试类低风险变更能自动跳过非生产门禁、降低模型档位、把运行预算减半,而高风险变更仍保留完整门禁/档位/预算,并且生产生命周期始终优先于基于风险的跳过逻辑,作为安全兜底。

*优先级信号*: P3(x1)　·　*最高成熟度*: architected　·　*示例来源*: five-systemic-trust-and-scalability-gaps

**真点火预算旗标显式提示** `×1`

操作手册强烈建议在真点火(--agent-cmd=claude)时显式设置 --max-agent-calls 与 --timeout 以获得可预测的成本上界,但这目前仅是 docs/ignition.md 里的人工纪律建议,CLI 本身在这些旗标缺失时不会主动警告。提议当 --agent-cmd=claude 且未显式设置 --max-agent-calls/--timeout/--agent-max-budget-usd 时,CLI 直接输出明确警告,而不是仅依赖文档提醒操作者。

*优先级信号*: recommendation(x1)　·　*最高成熟度*: narrative-log　·　*示例来源*: ignition

---

### 多厂商模型路由/LiteLLM `multi-vendor-routing`

25 个独立主题，原始条目 95 条。

**Agent CLI 适配器契约（多厂商 argv/成本/verdict 解析统一接口）** `×22`

ForgeOS 的整条 agent 执行链——argv 构造(claudeArgv)、成本解析(parseClaudeCostUsd)、过载检测(529匹配)、reviewer verdict/confidence 解析——目前全部硬编码 Claude CLI 的私有 flag 语法和 JSON 信封格式，散布在 engine_build.go/cost.go/command_executor.go 数个文件，与 README 宣称的'站在 Claude Code/Codex/Gemini CLI/OpenHands 之上'的多CLI愿景直接矛盾。二十余份独立文档反复提出同一修复：抽出正式的 AgentAdapter/Provider 接口(BuildArgv/ParseCost/ParseVerdict/SanitizeOutput)，配按vendor注册的registry和ClaudeAdapter默认实现，把新增CLI后端的成本从'逆向工程5+处硬编码'降到'实现一个接口'。多份已产出具体设计(接口签名、文件划分)，但也有一份architected结论认为在第二个vendor真正落地前属于YAGNI，建议先注释标记vendor-specific代码、暂缓构建正式接口。这是本类目里被独立重复提出次数最多的单一主题。

*优先级信号*: P0(x10)/P1(x3)/P2(x7)/P1-P2(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: five-codegrounded-architectural-blindspots-five-directions, architect-product-perspective-four-structural-gaps, forgeos-architect-product-perspective-five-frontiers-2026-07-10, novel-five-highvalue-extensions

**ModelMap/Registry 多厂商化：Provider 维度扩展** `×9`

routing.ModelMap是硬编码的单一Anthropic三档映射表，无Provider维度，opusFloorAgents等安全下限逻辑也隐式假设provider=claude，使向OpenAI/Gemini/Ollama等厂商路由在架构上不可能，直接卡死ROADMAP宣称的v3跨厂商LiteLLM池愿景。反复提出的修复是把ModelMap升级成可扩展ProviderRegistry(按厂商注册tier→model映射与定价表)，部分方案强调真正工作量在于跨厂商prompt格式适配(PromptAdapter：SystemPrompt/UserPrompt/ParseResponse，因Claude XML tag与OpenAI function-calling JSON不兼容)，而不只是加一行map。多份文档把此方向和'让HistoryTiebreak/scorecard真正参与--model选择'捆绑提出，但也有文档建议整个方向trigger-gated——等真正需要第二厂商API key时才投入。

*优先级信号*: P0(x2)/P1(x5, 部分复核后建议降为P2)/P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-unbuilt-core-extension-directions, 2026-07-11-genuinely-uncovered-frontiers, expansion-deep-analysis, next-horizons

**路由双轨断裂：forge route/TierForScore 从未真正驱动执行** `×8`

routing.go 同时存在两套路由系统——真正驱动执行的简单角色查表TierFor(agent,mode)，和完整实现、镜像policy.yml声明权重的六维评分器TierForScore/Score()——但后者只能通过forge route CLI手动触发，orchestrator从不调用它，导致用户在forge route里做出的路由判断与真实执行脱节，policy.yml调权重毫无效果。多份文档提出几乎相同的修复：加forge run --from-route标志把CLI算好的分数喂给引擎(并消除resolveAutoRisk重复计算)，或直接把TierForScore接入phaseTierResolver使其成为默认路径。后续交叉验证发现该断裂被部分夸大——BudgetAdjustTier/HistoryTiebreak实际已接入执行路径——把问题收窄到'六维评分器'这一具体部分仍未接入。

*优先级信号*: P0(x1)/P1(x3)/P2(x4)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-systemic-blindspots, five-architectural-product-extensions-codegrounded, five-codelevel-systemic-extension-directions, five-systemic-blind-spots

**跨厂商故障切换与韧性（Failover, not just backoff）** `×6`

当Claude遭遇持续过载(KindOverloaded)时系统只会对同一厂商退避重试，从不切换到健康的备选厂商，这与24小时无人值守目标数学冲突——单厂商~99.5%可用性意味着24小时内约11%概率遭遇中断，双厂商可压到<0.01%。反复提出的修复是把ModelMap升级为带健康探针的provider registry，加FailoverStrategy(round-robin/priority/latency-based)在探测到故障时自动跳过该厂商，配声明式.agent/policies/providers.yml和per-provider定价表；一份architected方案还把凭证隔离(每厂商独立API key管理)并入同一个ProviderAdapter接口。与'ModelMap多厂商化'高度相关但焦点不同：后者关心能否选择，这里关心故障时能否自动切换。

*优先级信号*: P0(x1)/P1(x4)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-genuine-architectural-frontiers-senior-architect-pm, 2026-07-12-forgeos-five-genuine-architectural-frontiers-senior-architect-pm, genuine-architectural-horizons-five, 2026-07-11-forgeos-five-genuine-architectural-frontiers

**路由评分维度自动特征提取/静态风险嗅探** `×6`

routing.Score()声明的六个评分维度里只有risk一个真正有信号来源(且仅是路径子串匹配的粗糙启发式FromChangedPaths)，其余complexity、dependency_change、security、context_size、business_impact在运行时全部硬编码为0.5占位值，导致'risk=critical→Opus'类安全下限逻辑永远等不到真实输入。反复提出的修复是构建可组合的ContentAnalyzer/特征提取链——轻量正则内容嗅探(检测import路径关键词)、真实的圈复杂度信号(复用现有harness的probe-or-N/A适配器模式接入gocyclo/lizard)——把这些维度从占位符换成有证据支撑的信号，同时保留路径启发式作为向后兼容的默认第一层分析器。这与'路由双轨断裂'主题相辅相成：后者关心评分结果有没有被消费，这里关心评分输入本身是否可信。

*优先级信号*: P0(x3)/P1(x1)/P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-structural-capillary-gaps, high-value-extensions-analysis, five-structural-extension-directions-architect-pm-combined, forgeos-five-unbuilt-product-architectural-extensions

**Scorecard/HistoryTiebreak 质量数据未回灌路由决策** `×6`

ForgeOS的记分卡系统已按(model, task_type)积累了真实的pass-rate/rework-rate/quality-score数据，但BudgetAdjustTier等tier决策逻辑大多不查询它，导致降档/升档决策始终纯静态阈值驱动。多份文档提出把scorecard证据接入tier决策并加冷启动样本量门槛、自动质量回归升档路径，但后续交叉验证发现核心声称部分失实——HistoryTiebreak其实已在logPhaseHistory里被phaseTierResolver消费——把问题收窄为'让它从tie-break顾问角色升级为能真正覆盖静态tier分配的--history-aware选项'这样一个更小、已被明确deferred的增量优化，而非从零打通的架构缺口。

*优先级信号*: P0(x1)/P2(x4)/P2-P3(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-architect-product-manager-five-extensions, expansion-five-codelevel-architect-gaps, expansion-five-systemic-learning-loop-gaps, forgeos-state-data-integrity-and-lifecycle-gaps

**[非主题] 幻觉/离题提案（未真正扎根于 ForgeOS 代码）** `×6`

这一组被源文档自己标记为[HALLUCINATED/OFF-TOPIC]或[OFF-TOPIC/META]——包括几份泛化的、脱离ForgeOS实际代码库的通用'ModelRouter/AIProvider接口'提案(有的甚至建议引入违反forge-core零依赖铁律的第三方重试/熔断库)，以及两份实际是在讨论pi-batch元编排管道自身(而非ForgeOS产品功能)的429智能重试/quota-aware路由提案。单独成组而非静默丢弃是为了诚实呈现原始数据构成——即便其中部分条目maturity字段标记为'architected'，内容要么脱离代码库虚构，要么讨论的是构建这批需求文档的pi-batch工具链自身，与ForgeOS产品真实的多厂商路由需求无关，不应被当作真实产品需求纳入优先级排序。

*优先级信号*: N/A-fabricated(x4)/P0-meta(x1)/P1-meta(x1)　·　*最高成熟度*: architected (但内容为虚构/离题，不代表真实设计产出)　·　*示例来源*: genuinely-uncovered-five-frontiers, high-value-expansion-directions, high-value-extension-directions, high-value-extension-directions-v2

**跨厂商计费/成本遥测解析抽象层（BillingParser）** `×4`

cost.go的成本解析函数(parseClaudeCostUsd、classifyClaudeOverload)整体硬编码Claude CLI的total_cost_usd JSON字段和529状态码格式，无provider抽象，即便routing.ModelMap已为多provider预留路由结构，成本这一侧完全没跟上。反复提出的修复是定义BillingParser接口(ParseBilling(rawOutput)->model/costUsdMicros/err)，把ModelMap改成可从providers.yml加载而非硬编码，并给ScorecardPair/trace.Event加Provider维度使跨厂商成本/质量比较成为可能；对无账单的自托管模型要求诚实标N/A而非编造0。这是'Agent Adapter'大主题里专门聚焦计费/遥测数据管线的窄化重复提案。

*优先级信号*: P1(x1)/P2(x2)/unstated(x1)　·　*最高成熟度*: design-stage-failed　·　*示例来源*: five-genuinely-uncovered-runtime-frontiers, five-structural-debt-and-product-frontiers, expansion-directions-v20

**Agent CLI 版本/契约兼容性校验** `×4`

routing.ModelMap假设安装的Agent CLI会一直支持路由表请求的模型名，但目前没有任何启动期校验——若Anthropic废弃某模型名或改变CLI flag/JSON字段名，系统会静默路由错误或在phase 0失败而非给出明确警告。反复提出的修复是加`claude --version`/`--dry-run`式启动校验(或`forge preflight --probe-cli`子命令)，把结果与一份声明式CLI-vendor契约文件(min_claude_version、稳定flag列表、期望JSON schema版本)比对，校验失败时打model_unverified告警而非静默继续。其中一份architected结论明确判定这是YAGNI，建议推迟到真正接入第二个vendor时再做正式契约。

*优先级信号*: P1(x1)/P2(x2)/P3(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-genuinely-unexplored-extensions, agent-orchestration-five-novel-perspectives

**主权离线部署/本地LLM执行器** `×4`

所有agent执行路径目前假设能访问云端Claude API，排除了政府/金融/医疗等无法把代码发给云端LLM的受监管或离线环境，使这些市场对ForgeOS完全不可寻址。反复提出的修复是实现LocalModelExecutor(包装Ollama/llama.cpp/vLLM而非重新发明推理)，在project.yml加`execution.backend: cloud|local|hybrid`配置和按phase覆盖，加离线SCA快照回退和感知上下文窗口的prompt截断。其中一份out.md复核确认这是这批文档里唯一真正的P0，但把工作量从约3个sprint上修到约5个(TierFor签名、waves调度、routing都要跨包改动)，并指出未被充分讨论的风险：更弱的本地模型可能导致更多loop-back，总成本反而可能不降反升。

*优先级信号*: P0(x3)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-overlooked-product-extensions, forgotten-product-five-v51

**路由阈值自校准引擎** `×4`

路由评分器里Haiku/Sonnet/Opus的档位边界常量(HaikuMax=0.34, SonnetMax=0.69)写死后从未调整过，即便记分卡系统已积累足够数据判断边界是否仍正确。反复提出的修复方案有明确分阶段路径：先做只读的`--calibrate`报告对比当前阈值与scorecard建议值差异；再做自动CalibratedThresholds重算(最小样本量门槛作冷启动保护，加阻尼/drift-guard防止阈值和预算反馈形成对抗性振荡)；最终目标是每维度评分权重也能自校准。所有版本都明确v1只做只读建议，不改变运行时行为。

*优先级信号*: P1(x4，其中一份复核建议降为P2)　·　*最高成熟度*: architected　·　*示例来源*: five-structural-extension-directions-architect-pm-combined, five-structural-extension-horizons, forgeos-five-unbuilt-product-architectural-extensions, forgeos-product-output-quality-and-ecosystem-gaps

**运行内实时模型质量自适应** `×2`

目前tier决策只在run开始前根据静态风险和预算消耗比一次性判断，HistoryTiebreak只在初始化时读一次跨run历史记分卡，同一run内即便连续出现reviewer REQUEST_CHANGES这种质量退化信号也不会触发实时升档，连续PASS也不会触发降档。两份文档提出几乎相同的修复：新增QualitySignal/QualityMonitor回调，在滑动窗口内累计质量退化信号后触发down_tier/up_tier，并设计预热期防止run早期噪声误触发。这是'Scorecard数据未回灌路由'主题的运行时/同run内特化版本，区别在于数据来源是本次run内实时证据而非跨run历史记分卡。

*优先级信号*: P1(x1)/unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: forgeos-five-codegrounded-architectural-frontiers, expansion-gaps-v7-novel

**forge detect 检测信号未注入路由/风险/模式系统** `×2`

forge detect计算出的结构化项目画像(语言、是否有测试、是否有CI、依赖密度)目前只被用来打印一条一次性的静态workflow建议字符串，之后再无任何下游消费者——不会喂给routing.TierFor、risk.FromChangedPaths或mode选择逻辑，导致测试完备的Go monorepo和完全没测试的Python脚本在路由/风险/模式判断上被完全等同对待。提议是把detect输出作为可选上下文接入router/risk classifier，缓存进.forge/detected.json，并用检测信号建议更安全的默认mode；但后续design-pipeline复核显示该方向没能产出实际设计，仍停留在需求阶段。

*优先级信号*: P1(x1)/unstated(x1)　·　*最高成熟度*: design-pipeline-failed　·　*示例来源*: 2026-07-11-five-codegrounded-product-expansion-directions

**学习闭环冷启动（新项目路由种子数据缺失）** `×1`

policy.yml的min_samples:20强制新项目前约20次agent运行只能走静态tier_default路由，因记分卡还没积累到足够样本触发HistoryTiebreak生效，而目前没有任何跨项目scorecard种子/预热机制、导入导出能力，也没有按项目类型打标签的画像模板。提议是从已跑过的示例项目派生种子记分卡画像，供forge-init在新项目初始化时预热，缩短新项目路由决策'盲跑'的窗口期。与'Scorecard数据未回灌'主题相关但关注点是新项目冷启动而非成熟项目的数据消费问题，仅出现一次但指向真实缺口。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-code-grounded-architectural-gaps-2026-07-11

**可插拔 Executor/Gate 扩展注册框架** `×1`

目前新增一个agent-CLI后端(Gemini、Codex)或沙箱执行器(Firecracker)都需要直接编辑engine_build.go里参数多达17/18个的agentExecutor()构造函数，没有任何插件式注册机制。提议引入ExecutorFactory/RegisterExecutor注册表，以及一个平行的Gate注册表，让第三方/社区可以通过注册工厂函数扩展新执行器和自定义质量门，无需fork forge-core本体。该提案比'Agent Adapter'主题范围更广(同时覆盖Gate扩展而不仅是vendor解析)，仅出现一次，故单列为独立主题。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: forgotten-five-foundations

**跨Tier一致性验证器（Shadow Run）** `×1`

当前模型路由TierFor是纯声明式规则(agent+mode+risk+lifecycle的确定性函数)，完全没有数据支撑'用更便宜档位是否会降低质量或增加loop-back次数'这一核心假设，scorecard.quality_score长期为N/A且与路由决策没有自动连接。提议引入Shadow Run机制——在采样比例内同时用主tier和影子(更便宜)tier跑同一任务并比较结构完整性代理指标，统计gate首次通过率与loop-back归因，最终输出`forge route --audit`量化的tier选择建议报告。是唯一一份把'路由决策是否被数据验证'作为核心问题提出的独立提案。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-extensions-v32

**Scorecard 聚合粒度过粗（phase级方差丢失）** `×1`

记分卡当前按iteration×task_type聚合，导致同一次evolve迭代里的4个implementer phase被压缩成单个Samples++数据点，phase级别的离群值被抹平，统计功效因此损失约75%，路由决策建立在被人为平滑过的信号之上。这份architected设计提议在Scorecard结构体加方差字段(CostStdDev、P99LatencyMs、OutlierPhaseCount)和有上限的滚动窗口PhaseCosts切片，把windDownScorecards从按iteration计数改为按phase计数，并让HistoryTiebreak感知方差而非只看均值。是一个统计学角度的独立缺口，聚焦数据采集粒度而非路由消费逻辑本身。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-systemic-blindspots

**模型路由决策的运行时可验证性（审计轨迹）** `×1`

目前路由链条没有闭环校验能确认'实际执行用的模型'和'路由决策选中的模型'是否一致——cost.go只解析total_cost_usd从不解析model字段，trace.Event.Model字段虽存在但从未被填充，导致BudgetAdjustTier触发的降档在审计轨迹里完全不可见。提议从成本解析落点填上trace.Event.Model，新增trace.DecisionEvent记录tier决策输入输出，并在run结束时生成一份对比'路由决策vs实际执行模型'差异的合规报告。该提案的设计阶段被标记为design-stage-failed，说明后续未能产出可行方案，仍停留在需求层面。

*优先级信号*: P1(x1)　·　*最高成熟度*: design-stage-failed　·　*示例来源*: 2026-07-11-five-genuinely-uncovered-runtime-frontiers

**声明式 Agent 卡片驱动路由元数据** `×1`

agent的tier/opus-floor/readonly/fresh-context等属性目前硬编码在routing.go的agentTier/opusFloorAgents等Go map里，每新增一种agent类型都要改Go代码并重新编译。这份architected提议把这些属性移到每个.agent/agents/*.md卡片的JSON front matter里，启动时扫描一次载入运行时map，特意选择JSON而非YAML以维持forge-core的零外部依赖铁律。这是关于'路由配置来源'的独立缺口——不是选哪个厂商/模型，而是agent元数据该硬编码在代码里还是数据驱动——只出现一次但已有具体设计。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-product-architect-expansion-directions

**模型档位感知的 Prompt/行为适配** `×1`

prompt.Build目前完全不感知运行时选中的tier——Haiku和Opus收到结构完全相同的prompt(同样的adrTopK/taskCap/memoryCap全局常量、同一份角色卡文本)，没有按模型能力差异化context预算或指令深度，造成信噪比浪费(弱模型被过多上下文淹没)和成本浪费(强模型该给的深度指令没给够)两头不讨好。提议让context预算成为tier的函数，并给角色卡加可选的按tier分区指令块。是唯一一份把'路由决策之后prompt本身该不该跟着变'作为独立问题提出的提案，与'该选哪个tier'的路由主题正交。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-12-five-closure-gap-expansion-directions

**多 Agent CLI 能力发现与自适应适配** `×1`

尽管README宣称站在Claude Code/Codex/Gemini CLI/OpenCode/OpenHands之上，编排层目前硬编码Claude专属flag(--permission-mode、--allowedTools、--model)，没有任何能力协商层，意味着非Claude CLI收到这些flag会直接报错而非优雅降级。提议构建CLI能力描述机制(静态映射表、`--help`内省，或声明式capabilities文件)，配合按phase的CLI选择(通过agent card声明)和'CLI缺少某项声明能力时优雅降级'的容错路径。与'Agent Adapter契约'大主题高度相关但机制不同——那边是固定方法集的接口契约，这里是动态能力发现/协商，故单列。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-systemic-architectural-gaps

**用量/配额感知路由器** `×1`

当前路由逻辑完全不感知厂商用量配额或限流重置时间，只在触发429/503后被动重试，而非提前主动降档避免撞到限流。这份architected提议在internal/routing.TierFor上扩展配额/重置时间维度，通过新的internal/quota包用零依赖本地滑动窗口跟踪用量(首选实现)，或可选轮询厂商用量API，目标是在真正撞到限流之前把Opus→Sonnet→Haiku主动降档，功能默认关闭，设计上明确与现有HistoryTiebreak/risk启发式互补而非替代。是关于'预算/限流感知'的独立机制，与'故障后切换厂商'的Failover主题(事后反应)形成对照(这里是事前预防)。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: high-value-extension-v35

**高风险决策的多模型共识引擎** `×1`

architect/cto/reviewer这类不可逆的高风险架构决策阶段目前硬编码只用单一模型(Opus)，没有跨模型交叉校验机制去对冲某个模型特有的偏见或瞬时质量下降。提议在asset.Phase上加consensus字段(strategy: majority/unanimous/advisory、min_voters、models列表)，由新runner在共识阶段针对不同模型并发跑N个agent，再用新的internal/consensus包合并输出、记录分歧度和置信度。作者自己标注这明确依赖尚未建成的跨厂商模型池才能真正落地，其建议分类(suggested_category)与本类目不同(multi-model-consensus)，是本类目里唯一一份指向'多模型共识'而非'多模型路由选择'的提案。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-analysis-v2

**厂商无关工具调用 Schema 层（隐藏耦合洞察）** `×1`

这是一份meta层面的观察而非独立功能提案——指出'Agent Runtime工具调用循环'和'多厂商模型池'这两个常被当作独立方向提出的需求之间存在被忽视的隐藏依赖：一个为Claude XML工具标签构建的工具调用生命周期循环，无法不经过居中的工具调用抽象层就直接适配到OpenAI function-calling JSON或Gemini格式。其价值在于提醒——任何真正推进多厂商路由的团队都需要先解决这个tool-call schema归一化问题，否则'接入第二个厂商'会比想象中难得多。作为唯一一份指出这一具体架构性隐藏依赖的条目，单列成一个洞察型而非实现型主题。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-deep-analysis

**Memory→Routing 反馈闭环（memoryAdjustedTier）** `×1`

recordMemory写入的KindLesson/KindGap/KindDecision类型条目本身就包含'连续gate失败→建议提高tier或缩小scope'这类自分析建议，且这些memory已在prompt层被memoryContext读取注入给agent参考，但从未在routing/循环控制层被真正消费执行——即memory自己总结出的建议从未真正反过来改变下一次的模型tier选择。提议在phaseTierResolver尾部加一个约20行代码的memoryAdjustedTier()过滤器让这些自分析建议真正生效，标注为'最高杠杆最低成本'的提案。与'Scorecard数据未回灌路由'主题相似但数据源不同——这里消费的是agent自己写的定性memory记录而非量化scorecard指标，故单列。

*优先级信号*: unstated(标注最高优先级信号)(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-expansion-perspectives

---

### 资源核算/容量规划 `resource-accounting`

19 个独立主题，原始条目 78 条。

**.forge/ 存储生命周期与容量治理 (Trace/Memory/Checkpoint 无界增长防护)** `×28`

本类目下最高频重复的方向:trace.jsonl、memory.jsonl、checkpoint.json、scorecards 四类状态文件各自为政地增长,大多只有临时性的单次轮转或手动压缩,缺乏统一的保留期/大小上限/TTL 策略,在 24 小时以上无人值守的 evolve 运行中有把磁盘写满、拖慢 forge status/doctor 读取甚至导致 checkpoint 失败的风险。二十余份独立文档反复提出本质相同的修复:引入声明式的 retention/rotation/quota 策略(project.yml 或 .forge/policy.yml 配置块),外加一个 forge cleanup/clean/maintain/state 之类的 CLI 命令做清理、归档与磁盘配额告警,部分设计更进一步提议把三个存储子系统抽象为统一的 internal/storage 包(可插拔 Backend,为未来 S3/GCS 铺路)或引入 PostRunLifecycle 钩子链。各文档只是在策略配置粒度、命令命名和文件覆盖范围上略有差异,是本类目收敛程度最高的一类想法。

*优先级信号*: P0(x3)/P1(x15)/P2(x7)/P3(x2)/cross-cutting(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-codegrounded-architectural-blindspots-five-directions, 2026-07-11-forgeos-architect-product-manager-five-extensions, five-gaps-from-global-scan-2026-07-10, five-uncovered-high-value-extensions

**Prompt/上下文 Token 预算管理与降级策略 (Context Window Budget)** `×17`

反复指出 ForgeOS 现有五个显式运行时预算维度(美元/调用数/超时/递归深度/输出字节)唯独缺少第六个——prompt token 大小;buildPrompt 把 ROADMAP、ADR、memory、gate 结果、角色卡等各条 context lane 无脑拼接,部分 lane 有独立字符上限但整体从不做加总校验,也完全不感知目标模型(Haiku 8K vs Opus 200K)的实际窗口容量,存在被模型静默截断、丢失关键约束(如 AGENTS.md)的风险。提案的收敛解法高度一致:引入轻量 token 估算器 + 按模型 tier 分级的预算表,超预算时按优先级(硬约束>任务>ADR>记忆>产物)做梯度降级并把降级动作记入 trace 供审计。部分较成熟的设计已具体到 checkPromptBudget 守卫函数、ContextPipeline 插件化重构和分阶段(监控→硬约束强制→自适应)落地路径。

*优先级信号*: P0(x5)/P1(x7)/P2(x5)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-genuine-systemic-boundaries, 2026-07-11-five-genuinely-uncovered-runtime-frontiers, 2026-07-11-forgeos-five-unbuilt-core-extension-directions, architectural-expansion-perspectives

**系统自监控与优雅降级框架 (Self-Health Monitoring & Graceful Degradation)** `×5`

指出 ForgeOS 对 LLM 调用成本追踪精细到微美元,却对自身进程级资源(磁盘占用、RSS、goroutine 数、trace/memory 文件大小)完全没有监测,24 小时自治运行中的内存泄漏、磁盘写满等问题只能靠事后 SIGKILL 或数据丢失才被察觉。共同思路是在每次迭代末尾做一次健康检查,配合阈值化的分级动作——不是直接 fail-closed 硬中止,而是先降级(收紧 trace 详细度、触发 memory compact、降低 checkpoint 保留数、跳过非关键 gate、降低并行度),并提供 `--auto-maintain`/`doctor --monitor` 等免人工介入模式。相比只是给 trace 加轮转的存储生命周期主题,这一类方向的核心是新增一层持续监测+分级响应机制,作为硬预算护栏崩溃前的缓冲带。

*优先级信号*: P1(x2)/P2(x2)/unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-forgeos-five-genuinely-unexplored-extensions, 2026-07-12-tech-lead-analysis-five-directions, cross-cutting-systemic-gaps, production-hardening-five-v42

**子进程资源可观测性 (Subprocess CPU/Memory/IO/FD Accounting via rusage)** `×4`

当前对 agent/gate 子进程的追踪只有退出码、超时、成本与耗时,完全没有采集 CPU 时间、峰值内存、IO 和文件描述符,即便 Go 标准库的 wait4/getrusage 系统调用唾手可得;结果是无法区分一次子进程失败究竟是逻辑错误退出还是被 OOM Kill,长时间运行的资源漂移也无从察觉。提案统一建议在 cmd.Wait() 附近采集 rusage,把 CPU/内存/IO 作为新的 omitempty 字段写入 trace.Event,并新增 `forge status --resources` 视图,同时需处理 Linux/Darwin 与 Windows(Job Object)间的平台差异。

*优先级信号*: P1(x2)/P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-four-infrastructure-gaps-subprocess-example-bridge-config, 2026-07-11-four-unseen-foundational-gaps

**跨存储一致性校验与并发运行隔离 (RunID / Consistency Verification / Run Isolation)** `×4`

关注的不是磁盘容量增长,而是 trace/memory/checkpoint 三个独立存储在崩溃或并发运行下彼此失去一致性的正确性风险——今天并发跑多个 forge evolve 会静默破坏彼此的 checkpoint/trace/memory 状态,且缺少任何跨存储的引用完整性校验。提案思路包括:给三类写入统一注入 RunID 做互相关联、新增 forge maintain/doctor --fix 检测并修复不一致、在 Checkpoint 里加入 TraceLastSeq/MemoryEntryCount/ScorecardVersion 等交叉引用字段做崩溃后的非阻塞式发散检测,以及最激进的方案——把共享的 `.forge/` 目录重构为 `runs/<run-id>/` 加文件锁的强隔离布局。这是本类目下唯一显式针对“数据正确性/并发安全”而非“容量/成本”的子集。

*优先级信号*: P0(x1)/P1(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-operational-maturity-and-structural-debt, 2026-07-12-five-unseen-systemic-operational-gaps, expansion-production-readiness

**资源感知的并行 Wave 调度 (Resource-Aware Parallel Scheduling)** `×3`

指出当前的并行 wave 调度纯粹由 depends_on 依赖关系驱动,对宿主机 CPU/内存/磁盘 IO 或 LLM API 限流容量毫无感知,一个 wave 可能同时拉起 N 个 claude 子进程和一个 CPU 密集型 gate,相互挤占资源、拉长墙钟时间并触发更多 529 限流错误。提案建议增加 `--max-wave-concurrency` 信号量以及可选的按阶段声明的 `resources: {cpu, memory}` 提示用于调度排序;但其中一份架构复核明确指出当前 wave 规模天然偏小、无实测资源压力证据,建议降级为 P3 直到真实出现资源瓶颈场景。

*优先级信号*: P2(x2)/P3(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-forgeos-architect-product-five-expansion-directions, forgeos-five-architect-product-extension-perspectives-2026-07-10

**统一资源核算总账与容量预检 (Unified ResourceAccount Ledger)** `×2`

提出一个跨越现有孤立护栏(调用次数/花费/超时/输出大小)的统一 ResourceAccount 账本,把此前完全未追踪的维度(.forge/ 磁盘占用、trace/memory 增长趋势)也纳入同一处核算;配套一个 `forge plan --resource-usage` 式的运行前资源估算器,基于 scorecard 历史预测本次运行的成本与磁盘消耗,以及 `forge status --cost/--disk` 提供跨 session 的累计可见性。目标是把资源管理从五个互不相通的护栏升级为一个中心化、可预测、可审计的容量规划系统,定位为纯 advisory 而非新的 fail-closed 闸门。

*优先级信号*: P2(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-10-five-genuine-architectural-frontiers

**闸门结果的阶段感知注入 (Gate-Result Phase-Aware Injection)** `×2`

gateLedger.context() 目前把已记录的全部闸门结果(测试/lint/复杂度/架构/安全/构建)无差别注入每一个下游 agent 阶段的 prompt,仅靠一个粗粒度的 FreshContext 布尔值把关,不区分某条 gate 结果是否与当前阶段角色相关,既浪费 token 预算又可能让实现者被无关的安全审查发现锚定分心。提案建议引入 gate 名称到 phase 角色的相关性过滤表,仿照已有的 memory/output/ADR 注入的 token 预算纪律,只把真正相关的闸门结果送入 prompt。

*优先级信号*: P2(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-architectural-product-extensions-codegrounded

**跨维度资源可行性预检 (Resource Feasibility Precheck)** `×2`

`forge preflight` 目前只检查环境就绪度(workflow 可解析、python3 在 PATH、agent-cmd 可执行),从不交叉校验美元预算、调用次数上限、超时、递归深度等资源维度是否共同足以跑完整个 workflow,也不判断当前信号下 convergence 在静态意义上是否可达。提案建议增加跨维度约束校验、一个 `converge.CanConverge` 静态可达性预检,以及基于 scorecard 历史的成本/耗时估算,作为非阻塞性的 preflight 警告。

*优先级信号*: P1(x1)/unspecified(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-codegrounded-architectural-product-gaps

**并行 Wave 取消的成本可见性 (Discarded-Cost Reporting)** `×2`

runWave 在某个 phase 的 gate 失败时会 fail-fast 短路整个 wave,但同一 wave 中已经跑完的其它并发 phase 所产生的 LLM 成本会被静默丢弃,用户只会看到“workflow failed”而看不到损失了多少钱。提案建议在 wave 被取消时打印一条结构化的成本损失报告(部分设计提议新增 WaveTrace 结构体、KindWaveResult trace 事件类型和 scorecard 的 avg_waste_pct 指标),属于低成本、低风险的纯可见性增强,不改变任何执行逻辑。

*优先级信号*: P1(x1)/unstated(x1, low-urgency)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-gaps, scan-fresh-perspectives

**成本遥测缺乏 phase/role/stage 维度** `×1`

trace.Event 和 scorecard 的成本聚合目前只按 model 维度切分,没有 agent_role、phase_type、workflow_stage 等字段,导致无法回答“reviewer 阶段花了多少钱”或“discover 与 build 阶段成本对比”这类问题,阻碍了按阶段类型做预算治理和基于角色的路由优化。这是一个独立、具体的数据模型缺口,与更宏观的统一资源账本或 token 效率遥测是不同粒度的问题。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-deep-systemic-gaps

**Token 级效率遥测 (parseClaudeUsage)** `×1`

提议把 cost.go 里现有的 parseClaudeCostUsd 扩展为 parseClaudeUsage,从 Claude CLI 的 JSON 输出中解析出 input_tokens/output_tokens,作为可选字段加入 trace.Event,并在 scorecard 中新增 token_efficiency 指标,为 context/token 预算类提案所声称的“节省 20-40% token”提供可量化的 ROI 验证依据。这是唯一聚焦于“采集真实 token 用量数据”而非“控制/限制 token 用量”的提案。

*优先级信号*: P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: architect-product-perspective-five-directions

**跨会话增量复用 (Phase Cache)** `×1`

指出每次 forge run/evolve 即便输入(角色卡、ROADMAP 片段、ADR)与上次运行字节级相同,也会把每个阶段完全重跑一遍,现有 ContextCache 只在单次运行内生效。提案新增 internal/cache/phase_cache.go,以输入的组合哈希为 key 做跨运行缓存,配合默认关闭的 `--incremental` 开关、`FORGE_CACHE_MAX_AGE` 过期策略和 `forge status --cache` 可见性,属于“避免重复计算”的性能/成本优化方向,与存储清理或 token 预算是不同机制。

*优先级信号*: P2(x1)　·　*最高成熟度*: design-stage-failed　·　*示例来源*: 2026-07-11-five-novel-architect-product-directions

**预测性运行成本/耗时估算引擎 (forge predict)** `×1`

提议新增 internal/predict 包和只读附加命令 `forge predict <workflow>`,聚合 .forge/trace.jsonl 与历史 scorecards.json,按 (workflow, model, mode, lifecycle) 维度输出 p50/p90/p95 的成本与迭代次数预测区间,用以取代 preflight.go 中当前硬编码的每阶段成本估算,冷启动时回退到 BaselineEstimate。这是资源可行性预检方向的数据驱动升级版,但作为独立、更完整的统计预测子系统单独提出。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-unbuilt-product-architectural-extensions

**声明式资源预算与实际消耗的交叉验证 (Declarative Per-Mode Resource Budgets)** `×1`

指出 defaultMaxAgentDepth、defaultMaxOutputBytes、overloadBackoffBase、DefaultCompactThreshold 等五类资源边界目前都是硬编码 Go 常量,不同 project 类型和 lifecycle 阶段共用同一套假设(prototype 与 24 小时 production evolve 用完全相同的 10MB 输出上限和 2s-60s 退避策略),且 modes.yml 已声明的 max_iter/max_agent_calls 等预算完全不与这些运行时参数联动。提案建议把这些常量迁移为 modes.yml/新 resources.yml 里按 mode×lifecycle 组合声明的配置,并让 check.py 新增声明-实现的交叉验证检查。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-extensions-v33

**loadCache 内存缓存无界增长 (Unbounded In-Process loadCache)** `×1`

指出 memory.go 里包级别的 `loadCaches sync.Map`(以文件路径为 key 缓存已解码的 memory 条目)没有任何淘汰策略,会随进程生命周期内接触过的每一个项目路径永久增长,在多项目 CI runner 或长期常驻进程场景下持续累积。这是一个非常具体、实现成本极低的修复点——为 storeToCache 增加一个 LRU 上限(如 64 条)——与磁盘上的 trace/memory/checkpoint 生命周期问题是完全不同层面(进程内存 vs 磁盘文件)的资源泄漏。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-verifiable-code-level-gaps

**启动期磁盘缓存层 (Cold-Start Disk Cache)** `×1`

提议为 workflow YAML、modes.yml、路由策略、ADR/AGENTS.md 等每次 forge 调用都要重新读取的上下文(约 12-20 次文件 I/O)增加一个以 mtime 为 key 的 JSON 磁盘缓存层,采用原子的临时文件改名写入和缓存未命中回退,确保缓存损坏或缺失时行为完全不变。这是关于“减少启动期重复文件 I/O 以提升调用延迟”的性能优化,方向上与本类目里其它“防止磁盘无界增长”的存储生命周期提案相反(一个是新增缓存写入,一个是清理旧数据)。

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: five-novel-architectural-frontiers-2026-07-10

**能力分级自适应 Prompt 构建 (Capability-Tier-Aware Prompt Construction)** `×1`

指出 prompt.Build/Gather 目前无论阶段被路由到 Haiku($0.0003/1K tok)还是 Opus($0.015/1K tok)都组装完全相同结构和体量的 prompt,tier 信息只以一句字符串出现,从不真正影响注入的上下文体量。提案建议引入 BuildAdaptive(tier, ...) 路径,按模型 tier 成比例压缩上下文、按 tier 调整输出格式指令,并给 scorecard 条目打上 prompt_version 标签,使质量数据比较不再混淆不同体量的 prompt。该提案被原文标注了不同的 suggested_category(capability-aware-prompting),提示其内核更接近“按能力自适应构造”而非单纯的预算截断控制。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-directions-v14-operational-trust

**零依赖栈下的隐蔽基础设施故障模式 (Silent Resource-Exhaustion Failure Modes)** `×1`

关注资源耗尽时的静默数据丢失和错误误判风险——checkpoint 写入失败目前只是 WARNING 后继续执行,trace.Emit 直接丢弃写入错误,fork/exec 的错误分类完全没有处理 EMFILE/ENFILE/EAGAIN 等瞬时性资源耗尽错误(会被误当作永久性失败)。提案建议新增 internal/infra 包,包含 IOFence.CheckWrite 在磁盘写满/FD 耗尽前主动拒绝写入而非静默丢数据、一个周期性检查磁盘/内存/goroutine/FD/trace 大小并记录为 trace 健康事件的资源报警层,以及更完善的 fork/exec 错误分类使瞬时资源耗尽被重试而非直接判定失败,是唯一聚焦“资源耗尽时如何正确失败/重试”而非“如何预防资源耗尽”的方向。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: novel-extensions-v36-deep-architectural

---

### 多仓库联邦/全局化治理 `multi-repo-federation`

10 个独立主题，原始条目 77 条。

**治理资产全局继承与组织策略分层 (ADR-0003 Submodule/Extends 激活 + Org→Team→Project PolicyStack)** `×22`

全语料库中被独立重复提出最多次的方向:激活 ADR-0003 设计已久却从未落地的治理资产集中共享机制,让 agent 卡片/workflow/policy 修一次就能传播到所有受治理仓库,而不是每次靠 N 个仓库各自手动改 `.agent/policies/`。具体做法是解析 project.yml 早已声明但从未被消费的 `extends:` 字段,指向一个上游 git submodule / 共享 agent-os 仓库,建立本地覆盖(override)与合并语义;更复杂的变体在此之上叠加完整的 组织→团队→项目 `PolicyStack`(更严格者胜)解析。多个条目明确提出触发条件门槛(≥2-3 个受治理项目才启动)与"先做可逆本地原型 Phase A、暂不做远程/不可逆迁移"的分阶段策略。

*优先级信号*: P0(x4)/P1(x7)/P2(x3)/P3(x2)/P1-P2(x1)/unstated-or-deferred(x5)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-high-value-extension-directions, architectural-extensions-v38, expansion-five-product-blindspots, forged-architecture-five-fresh-horizons

**多仓库/多项目工作区编排 (Workspace Orchestration Runtime)** `×16`

提议把 ForgeOS 从单进程单仓库工具升级为真正的多仓库/多项目编排运行时:新增 `forge workspace` CLI 家族与 `internal/workspace` 包,提供项目注册表、跨仓依赖图(depends_on 调度)、共享或分区的 budget 池与 per-project 凭证隔离,以及跨仓聚合的收敛信号(ROADMAP 完成度、GatesGreen 逻辑与)。直接针对 persist/memory/orchestrator/mode 中处处硬编码的"当前目录即唯一项目根"假设——该假设导致组织内多个相关仓库今天只能靠用户手动分别在每个目录跑 `forge run`,没有共享状态、预算或依赖感知。少数变体(如 forge daemon+FIFO队列)还提出常驻进程调度多个项目的演化。

*优先级信号*: P1(x7)/P2(x3)/P3(x2)/P4(x1)/unstated(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-architectural-product-expansion-directions, 2026-07-12-forgeos-five-genuine-architectural-frontiers-senior-architect-pm, strategic-expansion-v39, production-operational-gaps

**多仓库舰队治理与组织级控制平面 (Fleet Governance / forge-hub)** `×15`

提议在多个已独立 forge-init 的仓库之上建一层组织级"舰队"控制面(`forge fleet`/`forge org`/forge-hub):向各项目下发(或仅建议)策略基线、聚合各仓的 converge/gate/cost/quality 遥测与 scorecard 到统一仪表盘,部分变体还提出跨项目预算熔断或策略灰度发布(policy_canary)。与工作区编排集群的关键区别是:这里没有共享执行引擎,各成员仓库仍各自独立跑自己的 `forge` 命令,舰队层只做观测与策略推送,一些提案还明确建议先做只读聚合、暂缓强制策略下发。

*优先级信号*: P1(x4)/P2(x4)/P4(x2)/unstated-or-other(x5)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-product-architectural-expansion-directions-scanned, 2026-07-11-forgeos-five-next-architect-frontiers, five-production-architect-extensions-2026-07-10, expansion-directions-v3

**跨仓库依赖图与变更传播治理 (Dependency Graph & Breaking-Change Propagation)** `×7`

指出 orchestrator.Engine、gate.RepoRoot()、risk.FromChangedPaths 均假设单一仓库根,完全无法回答"这个共享库的改动会破坏哪些下游仓库"这类问题。提议新增声明式 `.agent/dependencies.yml`(或 `.forge/topology.yml`)记录上游/下游/共享契约关系,由 `forge detect` 驱动变更传播、触发下游 discover/build/测试,并对架构师阶段产出的接口契约(如 OpenAPI)做强制校验;"超级 workflow"变体进一步提出按拓扑顺序跨仓执行、下游失败时回滚。这是一组高度重复(多次 P0)、聚焦于"依赖感知与传播"而非纯执行编排的提案。

*优先级信号*: P0(x4)/P2(x2)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-five-highvalue-extension-directions-v2, 2026-07-11-five-genuine-post-closure-architectural-expansion-directions, 2026-07-11-five-hidden-subsystem-gaps, 2026-07-11-forgeos-five-genuine-architectural-frontiers

**治理模板漂移检测与版本化补丁同步 (Template/Policy Drift Detection & Manifest-Based Upgrade)** `×7`

比"全局继承"更机械、更狭义的一组提案:forge-init 把整套治理工具链(gate.mjs/secret-scan.mjs/arch-check.mjs/policies)作为一次性、无版本的快照复制进新项目,导致上游后续的安全补丁或策略收紧永远无法触达已创建的项目,且没有任何漂移告警。提议记录每个文件的来源哈希/版本的模板清单(template-manifest.json),用 `forge audit`/`forge doctor --governance-drift` 三态判定(未改动可升级/本地已改需合并/仅上游改动可自动应用),并让已存在但没有 CLI 入口的 `forge-upgrade.mjs` 升级为交互式三方合并(3-way merge)流程。

*优先级信号*: P1(x6)/P3(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-structural-blindspots, 2026-07-11-forgeos-five-codegrounded-systemic-gaps, 2026-07-11-forgeos-five-system-level-gaps, high-value-extensions-analysis

**跨项目知识联邦 (记忆/Scorecard/路由历史的组织级学习共享)** `×4`

提议把 ForgeOS 每个仓库积累的"学到的知识"——memory.jsonl 中的 lesson/decision、路由 scorecard、phase 历史遥测——在组织的多个项目间联邦共享,而不是让每个仓库的 `.forge/` 缓存成为孤岛。具体做法包括给 memory 条目和 scorecard 加上 namespace/origin/share_level 等作用域字段、提供 `forge memory export/import --global` 或 share/feed 机制、基于 Confidence/Supersedes 做冲突消解,使得一个仓库学到的安全教训或路由质量信号能被兄弟仓库看到,新/小项目也能借用路由学习所需的约 20-30 次真实运行统计成熟度,而不必从零冷启动。

*优先级信号*: P3(x1)/unstated(x2)/high-priority-custom-scale(x1)　·　*最高成熟度*: design-stage-failed　·　*示例来源*: fresh-five-systemic-extensions-2026-07-10, high-value-extension-v35, 2026-07-11-five-novel-perspectives-product-architect, expansion-gaps-v7-novel

**单仓多模块 Monorepo 工作区 (Monorepo Workspace, 区别于多仓)** `×2`

专门针对 monorepo 场景——同一个 Git 仓库内包含多个生命周期独立的服务(例如生产级支付服务与 MVP 前端并存),但目前只有一个全局 project.yml、一套 lifecycle/mode 策略和一个 `.forge/` 状态目录。提议 `Workspace`/子工作区模型,每个子项目拥有自己的 `.agent/project.yml`(可继承并覆盖根配置)、workflow 级别的 scope 字段,以及作用域隔离的 `forge gate/doctor/migrate` 命令与独立状态——这是与"多仓库"编排本质不同的问题,因为这里始终只有一个 Git 根。

*优先级信号*: P2(x1)/P3(x1)　·　*最高成熟度*: architected　·　*示例来源*: expansion-five-architect-product-perspective

**多仓库预留扩展点 (延迟决策的零成本占位钩子)** `×2`

两份提案共享同一元策略:现在不构建完整的多仓库编排,而是先预留几乎零运行时成本的扩展点,供未来的多仓库能力接入而不破坏现有假设。一份提议可选的 memory_sync_url、共享 checkpoint 目录、workflow 触发声明等钩子;另一份提议 `.forge/peer.json` 保留字段与 `forge route --peer` 标志(当前仅返回明确的 v3-noop),并起草未来跨项目引用语法的 ADR。两者都明确是"暂不做、只占位"的立场,而非请求实现该功能本身。

*优先级信号*: P2(x1)/P3(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-system-level-gaps

**跨项目配置学习与蓝图市场 (Project-Type Config Recommendation & Blueprint Marketplace)** `×1`

与上面的记忆/scorecard 联邦不同,这一方向关注的是学习并共享"最优运行参数"本身——按项目类型/语言归纳出最佳的 max-iteration、gate 组合、model tier 分配、成本画像,而不是共享具体的教训条目。提议一个匿名聚合、opt-in 的跨项目统计注册表(样本量过小不采信)、依据 `forge detect` 结果自动套用已验证优化配置的 `forge init --smart`,以及远期的社区 blueprint 市场,解决当前所有新项目无论是 Go CRUD API 还是 Python 数据管线都套用完全相同模板的问题。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-forgeos-meta-governance

**治理资产生命周期收敛与裁剪 (Governance Asset Lifecycle Pruning)** `×1`

提议一个 lifecycle → 治理资产 的映射文件,配合 `forge validate --governance-fit` 与 `forge prune --governance`,让项目能检测并移除对当前生命周期阶段而言多余的 agent 卡片/skill/workflow(例如一个 idea 阶段项目仍背着完整的 security-engineer 卡片),同时让 `forge-upgrade` 在合并时能更好地区分本地定制与上游变更。这是治理资产"体检瘦身"层面的诉求,而非跨仓库共享问题,但因作者将其归入本类别而收录于此。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: genuine-five-product-architectural-frontiers

---

### Web UI/CLI DX `web-ui-dx`

18 个独立主题，原始条目 50 条。

**CLI 结构化输出协议与错误码体系 (--json/--output 全覆盖)** `×13`

ForgeOS's ~17 CLI subcommands overwhelmingly emit unstructured human text; at any given snapshot only 1-2 commands (detect, sometimes status) support --json, so CI/CD pipelines, IDE plugins, and dashboards have no reliable way to consume forge's output besides fragile text-grepping. Nearly a dozen independently-generated docs converge on the same fix: a uniform --output text|json (or --json) flag across every subcommand, typed result structs (RunResult/EvolveResult/PhaseResult), a categorized error-code taxonomy (E_GATE_*/E_BUDGET_*/FORGE-XXX) replacing today's three inconsistent ad-hoc error-construction styles, and in several proposals a --json-events streaming mode emitting one JSON line per completed phase. This is the single most over-proposed idea in the category, and design work progressed furthest here, reaching an architected internal/clioutput OutputPort interface with a Result tagged-union and incremental per-command rollout plan.

*优先级信号*: P0(x1)/P1(x6)/P2(x3)/P3(x1)/high(x1)/unspecified(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-10-five-operational-frontiers, 2026-07-11-forgeos-five-unbuilt-product-architectural-extensions, five-gaps-from-global-scan-2026-07-10, forgeos-five-architect-product-extensions-2026-07-10

**首次运行引导与工作流发现体验 (forge start/workflow list/tutorial)** `×8`

New users hit a steep first-run cliff: forge detect only prints an advisory command the user must copy-paste rather than offering to execute it, there is no forge workflow list to discover the 5+ opaque workflow YAML files, forge init is a standalone Node script rather than a real CLI subcommand with no zero-config trial, and error messages are purely declarative instead of guiding users to a working alternative. At least eight independently-worded docs converge on the same cluster of fixes: a `forge start` entry point that runs detect and offers to execute the suggested workflow with confirmation, `forge workflow list/show/new/graph` for discovery plus template-based scaffolding, an interactive tutorial/quickstart/guided-init walkthrough, a `forge explain <workflow>` per-stage description command, and a `forge demo` zero-config trial run. Design reached an architected internal/template package and a Workflow UI/UX Layer.

*优先级信号*: P1(x4)/P2(x4)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-gaps, 2026-07-12-five-hidden-product-quality-gaps, forges-five-hidden-product-quality-gaps, productization-five-frontiers-2026-07-10

**统一配置管理系统 (forge config get/set/explain + 用户级持久化)** `×6`

Configuration is scattered across 5-8+ surfaces — project.yml, modes.yml, policies.yml, routing/policy.yml, .arch/rules.yaml, CLI flags, and undocumented FORGE_* env vars — with implicit, undocumented precedence rules, and there is no user-level persistence tier (~/.forge/config.yml), so every setting must be re-passed as a flag on every invocation. Multiple entries converge on a `forge config` subcommand family (get/set/diff/validate, plus a git-config-style `--show-origin`/`explain --effective` view of the merged result), sometimes paired with a CONFIG_MODEL.md doc and an env-var registry. The most advanced design (Central Configuration Hub & Consistency Guard) adds a JSON-Schema ConfigProvider with cross-reference validation and a drift-detection subcommand, phased toward project.yml becoming the single source of truth.

*优先级信号*: P0(x1)/P1(x3)/P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-codegrounded-architectural-extension-directions, 2026-07-11-five-genuine-systemic-boundaries, five-uncovered-product-frontiers-2026-07-10, expansion-self-governance-and-hygiene

**渐进式治理采纳 (forge adopt / 分级 bootstrap profiles)** `×3`

ForgeOS's only path onto an existing codebase is forge-init's all-or-nothing full copy of .agent/+harness/+CI (200+ files with full governance day one), with no way to incrementally adopt just one check (e.g. secret-scan) or coexist with a project's pre-existing CI. Proposes `forge adopt --level {silent|advisory|gated|full}` with per-check `--check-only` runners modeled on Kubernetes PodSecurity's profile evolution, and an alternative framing of four selectable bootstrap profiles (nano/micro/standard/enterprise) plus `forge migrate --profile` for incremental upgrade. The architected design explicitly found ~80% overlap between the two framings and merged them into one direction.

*优先级信号*: P1(x1)/P4(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: expansion-five-truly-uncovered-frontiers-v46, high-value-extension-v35

**CLI 帮助系统与 Shell Completion** `×3`

forge --help and forge <cmd> --help currently print the same static string regardless of subcommand, flag defaults are discoverable only by reading source, and there is zero shell-completion support anywhere in the repo. Proposals converge on per-subcommand FlagSet-driven --help (flag.PrintDefaults()), hand-written zero-dependency bash/zsh/fish completion scripts embedded via go:embed (explicitly avoiding Cobra to preserve forge-core's dependency discipline), plus adjacent polish items (--color flag, progressive --examples help, fuzzy subcommand-typo matching, an interactive real-time evolve dashboard).

*优先级信号*: P1(x1)/P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: five-uncovered-product-frontiers-2026-07-10, systemic-expansion-v26

**诊断可读性与 forge why 根因分析** `×2`

Two near-identically-titled entries independently argue that today's error output is a raw, unclassified Go error chain (e.g. 'gate test FAIL — exit status 1') that never distinguishes transient from permanent failure and never suggests a next action, leaving operators unable to self-diagnose. Both propose an OpError type carrying category/severity/fix-hint, and a new `forge why` command that synthesizes trace + checkpoint state into a what/why/how-to-fix/current-health report, plus richer doctor/preflight remediation messages.

*优先级信号*: P1(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-product-operations-systemic-gaps, five-product-operational-gaps

**forge migrate/policy 变更安全 (dry-run/回滚/审计)** `×2`

forge migrate --to is the highest-leverage lifecycle-lever operation, but it only computes a target Plan with no scriptable dry-run output, no --status query, no --rollback, no precheck of debt-task feasibility, and applyPlan directly overwrites project.yml/ROADMAP.md with no backup. The follow-on architected design adds a `forge policy plan --from/--to` diff CLI (gate-set/model-tier/coverage-threshold changes), an enhanced migrate --dry-run governance-impact report, a state machine blocking illegal backward lifecycle transitions, and a .forge/migrations.log audit trail, with an optional canary run mode explicitly deferred.

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-deep-systemic-codegrounded-boundaries, forgeos-trust-operational-maturity

**forge init 多语言生态适配 (--lang)** `×2`

forge-init scaffolds a strongly Node.js-biased project (Node seed app, SKIP_DIRS missing __pycache__/target/venv) despite ForgeOS's stated polyglot vision, and pre-existing per-language adapter YAMLs (go.yml/python.yml) are declared but never actually consumed as scaffolding templates. Proposes a `forge init --lang go|python|rust|node` flag selecting the correct seed app, SKIP_DIRS additions, and test command per ecosystem; the architected design adds new Go/Python seed apps reaching the same coverage/forge-accept bar as the existing Node example.

*优先级信号*: P1(x1)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-system-level-gaps

**Web 仪表盘/人审界面/告警钩子** `×2`

Proposes a build-step-free single-page HTML dashboard reading trace.jsonl/memory.jsonl to show run history, cost, and convergence status, paired with a `forge approve <stage>` HTTP endpoint replacing the --approved flag, and budget/converge alert webhooks — a self-critical follow-up noted the 'no build step' framing understated embedding/distribution, JSONL concurrent-read races, and chart JS complexity, re-splitting the work into P1 event streaming, P2 static dashboard, and P3 approval endpoint (deferred). A separate full Next.js Web UI was evaluated in the same document and explicitly deferred as conflicting with ForgeOS's CLI/declarative-core architecture principle, since the minimal dashboard already covers ~80% of the visualization need.

*优先级信号*: P1→split-P2/P3(x1)/deferred(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-expansion-directions

**冷启动引导:空 ROADMAP 首跑体验** `×1`

On a freshly forge-init'd project with an empty ROADMAP.md, forge run build correctly reports non-convergence, but the underlying agents receive an entirely empty 'current task' prompt lane and either emit nothing or hallucinate an unrequested feature. Proposes having forge init derive an initial ROADMAP from project.yml goals, and having planner/implementer phases refuse to run with an explicit 'run discover first' message when the ROADMAP is empty.

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-forgeos-five-system-level-gaps

**默认 dry-run 导致学习循环零覆盖** `×1`

forge's default executor is --executor dry, so the out-of-the-box experience from forge init through forge run build is entirely narrative — no real trace/memory/checkpoint writes occur, meaning the core learning-loop code paths (memory load/cache, converge evaluation, trace write) get zero coverage in a typical user's environment by default. Proposes flipping the default to require explicit opt-out of real execution, or at minimum making the learning-loop paths functionally exercise (not just narrate) under dry-run.

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-uncovered-operational-gaps-2026-07-10

**多会话运行时协调与 daemon 热加载** `×1`

Proposes introducing a SessionID concept threaded through trace/checkpoint/memory/scorecard, plus an optional `forge daemon` persistent process that hot-reloads .agent/ config via inotify, shares prompt/routing caches across invocations via a Unix socket, and supports graceful multi-signal shutdown (SIGINT once=graceful/twice=force, SIGHUP=reload) — replacing today's cold-start-every-invocation CLI model and laying groundwork for a future web UI/API.

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: genuine-uncovered-five-binary-state-output-session-datalifecycle

**执行计划非执行式预览 (forge plan)** `×1`

Proposes a `forge plan` subcommand producing a structured, non-executing preview of what a workflow run would do — mode-filtered gate set, skipped phases with reasons, resolved model per phase, historical cost/time estimate bands, loop-back targets, and parallel wave grouping — none of which the current --dry-run flag exposes, since it only toggles the executor rather than surfacing a structural plan.

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: novel-architectural-extensions-v40

**三运行时依赖门槛的诚实文档化** `×1`

forge-core itself is zero-dependency Go, but full functionality actually requires Go + Node.js + Python3 + PyYAML working together (gate.mjs/check.py/yaml2json.py/scorecard), so the 'zero dependency' narrative in CLAUDE.md/BOOTSTRAP.md understates the real installation barrier for new users and CI setup. Proposes documenting the true full-stack runtime requirement honestly, progressively porting check.py's key checks to Go to eliminate the Python dependency, and making yaml2json.py a genuinely invisible fallback.

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: production-product-gaps-v43

**Starter 起始项目从空壳到最小可用产品** `×1`

examples/starter/ is a governance-only shell today — every test comes from copied harness assets with zero project-specific code — so forge init produces nothing actually runnable. Proposes upgrading it into a minimal buildable/deployable HTTP service (health endpoint, minimal CRUD route, Dockerfile), with CI verifying the generated project both `go build`s and passes forge accept.

*优先级信号*: P3(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-product-output-quality-and-ecosystem-gaps

**forge init --template 选择与 CI 覆盖率/Fuzz 强化** `×1`

Bundles three developer-experience gaps into one design: forge init lacking a --template flag to choose among common starter workflows (default/api/service), CI's coverage.out currently only ever containing header data with no real enforcement, and internal/yaml2json having no fuzz testing despite a known past silent-corruption regression — addressed together via a --template flag, real go test -coverprofile CI enforcement, and a FuzzDecode harness seeded from the 7 real workflow YAML files.

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-genuinely-uncovered-frontiers

**选择性阶段执行 (--phase-from/--phase-to)** `×1`

forge run always starts at phase 0, so a developer iterating on just the implementer phase for hours must re-run discover+design+review every single time — a daily friction point, especially given that forge migrate --to already established a --to flag precedent. Design adds --phase-from/--phase-to as soft phase boundaries (skipped phases neither run nor count as failed), a --skip-gates flag, a dry-run-gate mode (gates run for real, agent phases dry-run), and fail-fast dependency checking when jumping ahead of un-produced artifacts.

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-extension-directions-architect-product-perspective

**CLI 子命令责任边界显式化 (run/evolve flag 解耦)** `×1`

The monolithic runOpts struct shares 16 flags indiscriminately between forge run and forge evolve, causing conceptual leakage where evolve inherits run-only flags like --parallel. Design splits it into RunOptsShared + RunOptsRunExtra + RunOptsEvolveExtra with an applies-to annotation and a scope-consistency lint check — a pure code-organization refactor with zero runtime behavior change.

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-unexplored-structural-corners

---

### 多用户协作 `collaboration-teaming`

15 个独立主题，原始条目 43 条。

**审批语义丰富化：条件批准/委托/SLA超时 (Conditional Approval, Delegation & SLA Escalation)** `×6`

六份提案从不同角度共同指向同一诉求：把 `.forge/<stage>.approved` 这种极简二元标记文件升级为承载状态的结构化审批模型——支持有条件批准（自动派生后续任务）、委托/转交给其他审批人（含循环检测）、审批过期时间与超时自动升级、以及记录审批人身份与 SLA（ApprovedBy/ValidUntil/BypassTicket）。具体载体各不相同：有的建议独立 `internal/approval/` 包配 Notifier 接口，有的建议 `.forge/<stage>/` 目录存 JSON 元数据配 `--with-conditions/--expires/--loop-back-to`，还有的建议 project.yml 声明审批角色配 daemon 超时计时器，但都以“二元 human_gate 无法表达真实工程协作”为共同痛点。其中两组已分别产出 architected 设计，且都包含向后兼容旧标记文件的迁移方案。

*优先级信号*: P1(x2)/P2(x1)/unstated(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-genuine-architectural-frontiers, five-production-architect-extensions-2026-07-10, expansion-production-readiness, operational-product-five-gaps

**富人机交互协议：TUI仪表盘+Pause/Resume+Webhook通知 (Rich HITL beyond Binary Approval)** `×6`

这是本类目中重复次数最多的方向：当前人机交互仅有“approved 标记文件”和“Ctrl+C 杀进程”两种最薄接口，操作员只能 SSH 进服务器 `ps aux` 才知道系统在干什么，NoProgress tripwire 也是硬停而非“暂停等人决策”。六份高度相似的提案反复重申同一诉求组合：一个终端 TUI 仪表盘（`forge tui`）实时展示阶段/闸门/花费状态、声明式 `--pause-on {converge,gate-fail,budget-warn,confidence-low}` 断点配 `forge resume/abort`、Slack/PagerDuty 等 webhook 通知子系统、以及携带人类反馈文字的 Rich Approval 供下一轮迭代消费；其中一份还额外提出 StatusStream 流式状态通道与交互式 reviewer diff 界面。两组已分别产出 architected 设计，均把 Pause/Resume 协议限定在 checkpoint 迭代边界，v1 明确不修改 evolve.go 现有的 rejectHumanGate 约束，只做可见性与手动暂停/中止。

*优先级信号*: P0(x5)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-genuine-architectural-frontiers-senior-architect-pm, 2026-07-12-forgeos-five-genuine-architectural-frontiers-senior-architect-pm, genuine-architectural-horizons-five, product-deployment-transparency-five-gaps

**分级自治/半自治Co-Pilot协作模式 (Graduated Autonomy / Semi-Autonomous Co-Pilot)** `×5`

五份提案共同提出用分级自治取代当前“等同于 full_autonomy、缺少信任建立阶梯”的现状：一种是通过 project.yml 声明 supervised→review_before_accept→auto_with_escalation→full_autonomy 四级自治光谱，配合 `forge run --interactive` 逐项 approve/edit/skip 和更丰富的裁决信号（APPROVE_WITH_NOTE/DEFER/OVERRIDE）；另一种是基于置信度动态分级响应（≥80自动继续、50-79标记提醒、<50触发软性人工闸门）加超时自动降级。architected 阶段两次都发现“逐条部分接受”与 asset.Phase 现有的“要么整体接受要么整体跳过”原子化执行/收敛模型冲突，因而把细粒度部分批准明确降级/延后到 v2，v1 收窄为整阶段 accept/skip/carry-back。

*优先级信号*: P0(x2)/P2(x2)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-overlooked-product-extensions, forgotten-product-five-v51, five-high-value-extensions-v44

**Discover阶段人机模糊消除/增量澄清协议 (Incremental Clarification Protocol)** `×4`

Discover 阶段目前一旦置信度低于阈值，只会得到一个死胡同式的“等待人工批准”提示，不会告诉用户到底缺什么信息。四份提案（两组 ideation+architected 配对）一致建议给 `converge.Signals` 增加按信息增益排序的 `OpenQuestions`，agent card 能吐出 `clarifying_questions`，并新增 `forge answer` 命令把用户答案注入 memory（新增 KindQuestion/KindAnswer 条目类型），触发增量式而非从头开始的置信度重估，从而把“卡住等批准”变成一次结构化的双向澄清对话，避免重复提问同一个问题。

*优先级信号*: P1(x4)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-highvalue-governance-evolution-extensions, 2026-07-11-forgeos-five-product-architect-extensions

**人类反馈信号纳入学习闭环 (Human Feedback as Learning Signal)** `×3`

当前人机反馈止步于 human_gate 的二元 approve/reject，操作员对具体决策（路由选型、某次评审判断）的纠正意见没有任何结构化落点，这一高价值训练信号被完全浪费、也无法反哺自我学习闭环。三份提案趋同地建议新增 `forge feedback`/`forge correct` 命令与 memory 的第四种 Entry 类型 KindFeedback，把 HumanRating/CorrectionCount 等字段写入路由 Scorecard，用以加权 HistoryTiebreak 与收敛判定（HumanFeedbackScore），并设有反刷分的权重上限与样本不足时的诚实默认值。其中一份进一步提出反馈应同时反哺具体 agent 的 prompt/AGENTS.md 约束强化，并配一个按修正频率排优先级的 `feedback triage` 命令。核心目标是让“人纠正AI”从一次性、易失的口头意见变成持久化、可加权、可复用的结构化学习信号。

*优先级信号*: P0(x1)/P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-10-five-genuinely-uncovered-frontiers, expansion-analysis-v2

**Shadow-Mode 预览式执行 (Propose-Only Execution)** `×3`

ForgeOS 目前在“完全不调真实 LLM 的 dry-run”与“直接写盘的实盘执行”之间没有任何中间地带。三份提案（两份 ideation 内容完全相同、一份 architected 尝试）一致提议新增第三种执行模式 `--executor shadow`：用真实 LLM 在临时 git-worktree/tmpfs 副本上运行，产出统一 diff 与结构化变更清单但不触碰真实工作树，人类审查后再 `forge apply shadow-xxxxx` 或丢弃，为自治 agent 写生产代码之前提供一道“预览-批准”的信任台阶。architected 阶段最终判定 design-stage-failed，说明这道信任台阶在真实实现层面遇到了阻力。

*优先级信号*: P1(x3)　·　*最高成熟度*: design-stage-failed　·　*示例来源*: 2026-07-11-five-novel-perspectives-product-architect, 2026-07-11-five-product-architecture-expansion-directions

**运行中人工反馈与引导注入 (Mid-Run Feedback/Guidance Injection)** `×3`

一旦 24 小时无人值守的 `evolve` 运行启动，LoopEngine 的回调是只写的，`forge status` 只能读取静态 checkpoint 快照，人类没有任何中途注入引导的入口（“单向玻璃”问题）。三份提案（含一组 ideation+architected 配对）建议增加 `.forge/feedback/` 目录与 `forge feedback` 命令（`--pause/--inject/--skip-phase/--redo-phase`），由 LoopEngine 在阶段边界轮询消费并具备重启安全的持久化；另一份从“实时状态推送”角度提出 `forge status --live`、`forge evolve --webhook`、`--pause-after <N>`，以及可以直接 attach 到运行中进程 PID 注入引导文本的 `forge guidance --attach <pid>`，机制上比文件轮询队列更进一步。

*优先级信号*: P0(x1)/P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: forgotten-five-meta-governance-and-blindspots, high-value-extension-v47

**工作区/并发状态隔离 (Workspace Isolation)** `×2`

checkpoint.json/trace.jsonl/memory.jsonl/scorecards.json 等状态文件目前都写在单一全局 `.forge/` 路径下、没有按会话分区，两个工程师或本地开发与 CI 并发跑 `forge evolve` 时会互相覆盖对方状态，崩溃后的 `--resume` 甚至可能悄悄接上别人会话的 checkpoint 并消耗错误迭代的预算。两份提案（一份 ideation、一份已 architected）一致建议引入工作区标识（分支名/会话 UUID）以 `--workspace <id>` 形式拼进每个状态文件路径与缓存 key，architected 版本明确默认不传参时保持现状全兼容，并被 Tech Lead 评估为风险最低、可最先落地的方向。

*优先级信号*: P2(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-four-extension-frontiers, forgeos-four-product-architect-frontiers-2026-07-10

**紧急操作员覆盖路径 (Break-Glass Override)** `×2`

ForgeOS 设计为 24 小时无人值守自治系统，却没有任何文档化、可审计的人类操作员紧急覆盖机制——当收敛判定卡死、gate 误报或只读阶段挡住紧急修复时，操作员唯一的选择是杀进程或手动改状态文件，两者都不留审计轨迹。两份内容高度一致的提案建议提供 `forge approve --override-gate`、`--stop-after-next-iteration`、`--force-write` 等强制附带 `--reason` 的 CLI 标志，并在 trace 中打上 `bypassed:true` 标记，同时明确此机制禁止绕过 human_approval 或生产环境强制底线。

*优先级信号*: P2(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-codelevel-systemic-extension-directions, 2026-07-11-forgeos-five-code-grounded-systemic-gaps

**多级审批链 / 单一审批者模型扩展 (Multi-Stage Approval Chain)** `×2`

design→build 之间的 human_gate 目前是单一二进制闸门，无法表达“架构师先批→安全再批→CTO放行”这类多阶段、多角色的顺序审批链，也不读取 review.yml 产出的多维评审裁决。提案建议扩展出 `approval_chain` 声明，支持多角色标记文件加依赖顺序。该方向在尝试进入 architected 阶段后设计流水线失败（design-pipeline-failed），说明多级审批链与现有 converge/gate 模型的整合难度比预期更高，目前仍停在需求阶段。

*优先级信号*: P3(x1)/unspecified(x1)　·　*最高成熟度*: design-pipeline-failed　·　*示例来源*: 2026-07-11-five-deep-systemic-codegrounded-boundaries

**Human Gate 超时升级 (HITL Timeout & Escalation, 窄范围)** `×2`

`humanGate()`/`StopCondition` 的人工审批闸门目前完全没有超时机制——一旦没人响应，`evolve` 循环会无限期等待，既无升级通知也无降级路径，标记文件被误删还会静默打回“未批准”状态。提案建议为 HumanGate 增加 `timeout_after`/`on_timeout: auto_approve|skip|fail` 字段，配合周期性超时轮询和可选 webhook 升级通知；architected 版本落地为超时后把收敛信号显式翻转为 NOT MET 并触发 exec hook（升级模型档位/通知），同时补上 `forge approve/--reject` 的显式标记写入。相比更大的“审批语义丰富化”方向，这一支专注在单一 StopCondition 字段的窄范围修复，reviewer 也认定这是已知的 v1 限制、计划留给 v2/Temporal 重做。

*优先级信号*: P1(x1)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-genuinely-uncovered-frontiers

**Diff级人工审查与上下文注入 (Diff Review & Context Injection)** `×2`

提案把人类角色从纯粹的“批准/拒绝”闸门升级为主动协作者：暂停/恢复运行中的工作流、在 CLI 层面对 agent 产出的具体文件改动做逐项 approve/reject 的 diff 审查界面、budget-warning/gate-fail/converged 等事件的通知集成，以及 `forge run --context "..."` 这样的临时上下文注入。architected 阶段在与另一份已覆盖异步审阅/条件批准的设计交叉核对后，收窄聚焦到两个真正未被覆盖的缺口：把 AgentVerdict 从二元 APPROVE/REQUEST_CHANGES 升级为按文件/按 hunk 的细粒度审批（需要真正改变收敛模型，从“最后整体通过版本获胜”变为“每个文件的批准状态独立演进”），以及一个轻量的 `--context '<约束>'` 引导注入标志。

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: forgotten-frontiers-five

**多分支并行谱系与团队实验对比 (Multi-Branch Parallel Lineage)** `×1`

提议把 `.forge/` 状态按 git branch 隔离存储、新增 `forge branch`/`forge diff --branch` 命令，并给 scorecard 增加 branch 维度，使团队成员能在不同分支上并行探索不同 mode/model 组合并直接横向比较结果——比单纯的工作区隔离更进一步，目标是把分支变成团队协作实验对比的一等公民维度，而非只是避免状态冲突。该方向在交叉验证中被判定已被另一份文档的“方向三”完整覆盖、并非真正未覆盖的新方向，但其“按分支对比实验结果”的具体诉求仍与纯粹的并发隔离（Workspace Isolation）有实质区别。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-uncovered-architectural-frontiers

**forge approve 只读转读写闭环 (Approve CLI Write-Path Completion)** `×1`

`approve.go` 的代码注释里明确写着 “Future: forge approve <stage> --yes”，但当前实现只有只读的 `forge approve list`，用户被迫手动创建 `.forge/<stage>.approved` 标记文件才能完成批准，也完全没有 `forge reject` 命令来触发 on_rejected 路径或记录审批备注/理由。提案要求补全这条写路径：`forge approve <stage> --yes/--note`、`forge reject <stage> --reason`，让项目自称“最高杠杆闸门”的 human gate 拥有名副其实的完整 CLI 交互闭环。这是本类目里颗粒度最小、最具体的一个纯 CLI 补全型缺口，与更大的“审批语义丰富化”方向互补但不重叠——它解决的是“连最基本的写命令都没做完”这一更基础的问题。

*优先级信号*: P0(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: productization-five-frontiers-2026-07-10

**人机协同版本竣工确认分工 (Validated Human-Agent Sign-off Division)** `×1`

与其他条目不同，这不是一个待实现的新功能提案，而是一次真实点火(ignition)运行后记录下来的、已经生效的人机协同治理模式的观察：agent 由于权限模型限制（无 Bash、无法自我核验就必须拒绝勾选）天然只能达成“增量绿”，而不能越权给整版本盖章；“版本竣工”的 ROADMAP 勾选必须由人基于 test 全绿 + gates green + reviewer PASS + `forge accept` ACCEPTED 的客观证据来确认。目前这层分工靠 implementer 的权限边界隐式实现，尚未固化为显式协议，但被记录为未来更广泛人机协同工作流（如多人审批链、团队版本签核）的设计基线，与多级审批链、审批语义丰富化方向形成呼应。

*优先级信号*: validated-capability(x1)　·　*最高成熟度*: narrative-log　·　*示例来源*: ignition

---

### 事件驱动/外部集成 `event-driven-integration`

10 个独立主题，原始条目 39 条。

**事件驱动网关与常驻守护进程 (Webhook/Cron/GitOps 触发层)** `×13`

ForgeOS today is 100% CLI pull-based: every run requires a human-typed command, with zero net/http listener, daemon process, or scheduler anywhere in the codebase, so it cannot restart itself tomorrow or react to a merged PR. This theme — by far the most repeatedly reinvented idea in the category — proposes turning it into an always-on service (forge daemon/serve/controller) that ingests external events (GitHub/GitLab webhooks, polled CI status, cron schedules, git-watch, PR-merge) and auto-dispatches forge run/evolve accordingly, finally activating the stop_condition.type:external and triggers fields already declared but unconsumed in evolve.yml. Variants add HMAC-verified webhook auth, idempotency/dedup keys, multi-repo scheduling queues (a GitOps-controller framing that reuses existing DAG wave logic), and graceful daemon lifecycle (SIGHUP/reload/stop).

*优先级信号*: P0(x4)/P1(x4)/P2(x2)/unstated(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-genuine-architectural-frontiers, forgotten-frontiers-five, operational-product-five-gaps, novel-five-perspectives-2026-07-10

**跨 Workflow 管线编排引擎 (forge pipeline DSL)** `×5`

The Discover→Design→Review→Build→Evolve spine is five isolated manual CLI commands today — each workflow's next_stage/OnApproved field is a documentation-only string consumed by zero Go code, so a human must run and manually approve every stage transition. This theme proposes a declarative pipeline DSL (.agent/pipelines/*.yml) and a forge pipeline run engine that auto-triggers the next workflow on convergence/approval, supports conditional branches (on_approve/on_redesign/on_reject), stage-level timeouts, pipeline-scoped checkpoint/resume, and optionally parallel independent stages — layered non-invasively on top of existing per-workflow execution rather than modifying it. One design attempt for this exact idea is recorded as having failed at the design-pipeline stage before a later attempt succeeded through to an architected design.

*优先级信号*: P1(x3)/unstated(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-architectural-product-expansion-directions, 2026-07-12-forgeos-five-highvalue-extensions-senior-architect-pm, forgeos-five-unseen-product-architect-extensions

**运行时网络集成面 (HTTP/Unix-Socket API + SDK)** `×4`

ForgeOS exposes zero network surface — no HTTP/gRPC/socket listener anywhere — so external tools (CI dashboards, IDE plugins, internal developer platforms) can only integrate by shelling out to the CLI and scraping stdout text or exit codes. This theme proposes a phased runtime API: first a read-only status/runs/trace query surface (Unix domain socket or HTTP), then a read-write trigger/webhook API, then richer telemetry (SSE event streams, Prometheus metrics), and eventually a TypeScript SDK or a promoted public Go library API so internal/ abstractions can be embedded directly. Proposals are explicit about phasing read-only ahead of read-write to avoid prematurely opening an attack surface.

*优先级信号*: P1(x3)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-10-five-genuinely-uncovered-frontiers, 2026-07-11-forgeos-three-architectural-gaps, production-product-gaps-v43

**内部结构化事件总线 (Pub-Sub 替代回调，进程内可观测性)** `×4`

Progress and lifecycle signals inside forge-core today only travel through ad-hoc func(string) Log callbacks or post-hoc JSONL trace files, so nothing inside the process can react to another component's event in real time and every new consumer must be hand-wired. This theme proposes a structured, subscribable in-process EventBus/Dispatcher (Publish/Subscribe by kind, optional replay ring buffer, UNIX dgram or in-memory channel transport) that replaces scattered OnGateResult/OnPhase/Log callback plumbing and enables reactive behaviors such as auto-pausing evolve on repeated anomalies or auto-downgrading on repeated overload signals. All four instances are explicitly scoped as in-process/local decoupling (embedded v1, transport interface reserved for a future daemon), distinct from external webhook notification or inbound event triggering.

*优先级信号*: P0(x2)/P1(x1)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-architect-product-expansion-2026-07-10, forgeos-five-product-architect-expansion-directions, forgeos-five-uncovered-architect-product-extensions-2026-07-10, forgeos-four-product-architect-frontiers-2026-07-10

**统一结构化输出协议 (--json 标志 / E_* 机器可读错误码)** `×3`

Only forge doctor/detect currently support ad-hoc --json; every other command's results can only be scraped by grepping free-text terminal output, and there are just three undifferentiated exit codes (0/1/2) with no way to tell a retryable failure from a permanent one. This theme proposes a uniform --output text|json flag across all subcommands, typed result structs (RunResult/EvolveResult with phases/gates/convergence/cost breakdown), and a standardized E_* error-code taxonomy with a consistent JSON envelope — a pure CLI output-contract upgrade (no daemon or network involved) that lets CI pipelines and PR-comment bots consume results programmatically.

*优先级信号*: P1(x1)/P2(x1)/unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-uncovered-high-value-extensions, five-uncovered-product-frontiers-2026-07-10, genuinely-novel-expansion-directions

**出站事件通知总线 (Webhook/Slack/Email Notification Sink)** `×3`

Gate failures, convergence, budget-exhaustion, and human-approval-needed moments currently only ever write to stdout or local JSONL files, so nothing proactively alerts a human — they must poll or tail logs. This theme defines an EventSink/notify.Sink abstraction (stdout, JSONL, Webhook-with-retry, Slack, SMTP, generic message-queue) fanned out in parallel from the orchestrator's key lifecycle points, deliberately kept opt-in (e.g. behind a --notify flag) so default byte-identical CLI output is preserved. Its purpose is proactive outward notification, distinct from inbound event triggering (the daemon/gateway theme) and from internal pub-sub decoupling.

*优先级信号*: P1(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-genuine-codegrounded-extensions, genuinely-uncovered-five-frontiers

**部署与生产反馈闭环 (Deploy Phase + Production Signal Loop)** `×3`

ForgeOS's workflow spine currently stops the moment code passes the qa/accept gate — there is no deploy phase, no rollback mechanism, and no channel for production reality (error rate, p95 latency, traffic drop) to flow back in and automatically re-trigger forge evolve. This theme proposes extending converge.Signals with production metrics pulled from external monitoring, adding new deploy.yml/rollback.yml workflow stages that reuse the existing Phase/on_fail engine, and new asset types (DeployTarget/DeploymentStrategy/RollbackPlan) with a risk-classified strategy (direct/rolling/canary) — deliberately scoped in its first cut to artifact generation (Helm/K8s manifests for an external CI to apply) rather than direct remote execution, to avoid credential-management risk before user trust is established.

*优先级信号*: P1(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-10-five-genuinely-uncovered-frontiers, architect-product-perspective-five-directions, expansion-five-product-blindspots

**跨阶段结构化数据契约与制品血缘 (Artifact Lineage / Session ID)** `×2`

Beyond just auto-triggering the next stage, these proposals target the payload that would flow through such a pipeline: today no PRD content, architecture decisions, review verdicts, or accumulated trace/memory is structurally passed between stages, forcing every downstream agent to rediscover upstream artifacts by re-reading files from scratch. This theme proposes a first-class Run/Session ID threaded through trace events plus an Artifact Catalog recording {session_id, phase, artifact_path, type, hash}, making lineage (which design run consumed which discovery PRD) queryable — a data-contract concern that complements but is mechanically distinct from the pure execution-scheduling concern of the pipeline-DSL theme.

*优先级信号*: P0(x1)/P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-extension-directions-verification, forgeos-five-product-architect-expansion-directions

**自描述能力与发现协议 (forge describe)** `×1`

External tools (IDE plugins, CI orchestrators, cross-project governance dashboards) currently have no machine-readable way to learn what workflows, gates, or mode-gating rules a ForgeOS project has — they can only manually parse prose-heavy YAML. This lone proposal defines a versioned internal/describe package and a forge describe project/workflows/gates/capabilities command family exporting structured JSON, functioning as a discovery/introspection protocol rather than an execution or triggering mechanism — arguably the prerequisite building block that many of the other API and pipeline proposals in this category implicitly assume already exists.

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-systemic-declaration-gaps

**外部 SDLC/VCS 集成层 (PR/CI 操作适配器)** `×1`

ForgeOS can write code but has no way to actually create a pull request, check remote CI status, comment on a PR, or auto-merge — keeping it a purely local dev tool rather than a participant in a team's real SDLC. This lone proposal defines an internal/vcs package with a VCSAdapter interface (CreateBranch, CreatePR, GetCIStatus, CommentOnPR, MergePR, PostCheckRun) implemented for GitHub/GitLab, defaulting to a no-op local adapter that preserves today's zero-API behavior, plus a workflow-declarable publish stage and CI-status-aware convergence criteria — an outbound VCS-actions layer, mechanically distinct from the inbound webhook-triggering daemon theme.

*优先级信号*: unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: high-value-extension-v35

---

### OS级安全/进程隔离/凭证保护 `os-security-isolation`

13 个独立主题，原始条目 37 条。

**子进程环境变量透传泄漏 (EnvPolicy 白名单)** `×11`

childEnv()/buildEnv() 目前只剥离 FORGE_AGENT_DEPTH 一个变量，把父进程完整的 os.Environ()（ANTHROPIC_API_KEY、GITHUB_TOKEN、AWS/GCP 密钥、SSH_AUTH_SOCK 等）原样透传给每一个 `claude -p` agent 子进程，在 CI/多租户场景下构成泄漏叠加被 LLM 驱动进程滥用的复合风险。十一份提案反复收敛到同一个修复：引入 EnvPolicy/EnvGuard 式的允许/拒绝清单（默认只放行 PATH/HOME/FORGE_*/必要 API key），部分变体叠加 PATH 硬化、LD_PRELOAD 清零防动态链接注入、trace 审计透传变量数与 `forge doctor --security` 检查。多份已推进到 architected 阶段并给出具体接口设计（EnvPolicy struct、EnvAllow/EnvDeny 字段）。

*优先级信号*: P0(x5)/P1(x4)/P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-foundational-resilience-gaps, 2026-07-11-five-pipeline-integrity-and-security-gaps, 2026-07-12-five-unseen-systemic-operational-gaps, architect-product-perspective-five-novel-directions

**子进程最小权限三位一体(Env allowlist + Argv[0] 白名单 + Emits 派生写路径)** `×5`

在纯环境变量过滤之上，这组高度重复的提案统一打包三种机制：(1) MinimalEnv 环境变量白名单（可选 --preserve-env 逃生阀）；(2) 对被执行二进制 argv[0] 做 basename 白名单校验（仅允许 claude/node/python3 等已知 agent CLI）；(3) 从每个 phase 的 `emits:` 声明自动派生允许写入的文件路径，取代当前非 readonly 阶段即可任意写全仓的全有全无模型。五份来源几乎在同一套三段式方案上重复收敛，其中两份已推进到 architected 阶段（含 Runner 接口装饰器重构设想），目标是压缩被劫持/失控 agent 子进程的爆炸半径。

*优先级信号*: P2(x5)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-expansion-priorities, 2026-07-12-five-post-scan-architectural-extension-directions, global-scan-five-codegrounded-extension-directions

**孤儿/僵尸子进程回收与单实例并发锁** `×4`

CommandExecutor 通过进程组 SIGKILL 终止超时 agent 进程，但 setsid() 逃逸的孙进程（git 凭证助手、后台 watch 进程等）会成为孤儿被 init 收养、永不清理，可能持有文件锁/端口；被 SIGKILL 的 forge 主进程本身也不留任何 PID 记录，静默消耗 API 预算。四份提案收敛到同一套机制：spawn 时把 PID/PGID 登记到 `.forge/pids/` 目录、启动时扫描/清理陈旧进程组（Linux 下用 prctl(PR_SET_CHILD_SUBREAPER) 或 ScanOrphans()），以及一个 flock/O_CREATE|O_EXCL 式单实例运行锁防止并发 forge 实例互相踩踏。

*优先级信号*: P1(x4)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-novel-architect-product-directions, 2026-07-11-forgeos-five-unbuilt-product-architectural-extensions, five-unseen-governance-horizons

**运行身份与 .forge/ 并发状态隔离(Run-ID/Session)** `×3`

`.forge/checkpoint.json`、`trace.jsonl`、`memory.jsonl`、`scorecards.json` 全部使用裸文件名、无运行/会话标识，两个并发的 forge 进程（手动 run 与后台 evolve，或两个 CI job）会互相覆盖 checkpoint（最后写入者赢）、交叉污染 memory、trace 行序错乱。三份提案独立收敛到同一根因，其中一份已被采纳为最高优先级真正的采用阻塞项并完整落地：注入 ULID RunID 到每条 trace.Event/checkpoint、把 `.forge/` 重构为 `runs/<run-id>/` + `latest` 软链接（含旧布局迁移路径）加跨进程 flock 锁；另两份分别在需求/构想阶段独立提出相同诊断与 session-UUID + trace session_id 的轻量方案。

*优先级信号*: blocking-highest(x1)/★★★★low-cost(x1)/unstated(x1)　·　*最高成熟度*: coded-and-reviewed　·　*示例来源*: 2026-07-11-five-adoption-gating-product-trust-gaps, 2026-07-11-five-codegrounded-product-expansion-directions, expansion-gaps-v7-novel

**Phase 执行环境隔离与 Readonly/Sandbox 强制执行** `×3`

一次 forge 运行内的所有 phase 目前共享同一个 OS 进程、文件系统和环境变量：声明为 readonly:true 的 phase（如 fresh-context reviewer）实际仍可通过 Bash 任意修改 src/ 甚至破坏 .agent/，并行 wave 之间也可能竞态写同一文件，且 Phase.Readonly 与 SandboxConfig 字段目前只被解析、从未被真正执行。三份提案分别从独立工作区副本+仅合并 emits 文件、L1-L5 分层沙箱（进程/临时目录/环境变量白名单/网络/文件系统），以及分阶段落地路径（L1 post-phase git-diff 告警 → L2 git-stash 隔离 → L3 Firecracker/Docker 路由）三个角度收敛到同一诉求：让声明的隔离/只读约束真正被 harness 强制执行。

*优先级信号*: P0(x1)/P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: architectural-expansion-perspectives, forges-five-unbuilt-foundations, forgeos-five-unseen-perspectives-2026-07-10

**持久化状态明文存储缺乏加密与完整性校验** `×2`

`.forge/trace.jsonl`、`checkpoint.json`、`memory.jsonl`、`scorecards.json` 全部以明文 JSON/JSONL 落盘，无加密、签名或完整性校验，同一 CI runner 上的其他进程可直接读取完整 agent prompt 与架构决策，或篡改 checkpoint 强制 resume 到错误 phase。两份提案（需求版与对应的 architected 版本）都提出分层方案而非无差别全量加密：checkpoint 加 HMAC/SHA-256 签名做防篡改校验、trace 中敏感字段侧车加密同时保持主体可读以便 jq 调试，并将全量 AES-GCM `--encrypt-state` 作为可选的延后特性。

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-architect-product-perspective-five-frontiers-2026-07-10

**纵深防御打包(trace脱敏+文件权限收紧+完整性校验/SBOM)** `×2`

两份提案（需求与 architected 版本高度对应）都主张把当前单点防御（仅 secret-scan 挡住硬编码 secret）升级为多层纵深防御：在 trace 写入路径加脱敏/redact 钩子防止敏感字符串写进 trace.jsonl、把 checkpoint/memory 等状态文件权限从 0644 收紧为 0600 并加 umask 保护、在子进程 env 构建时按允许列表清理密钥、并为 checkpoint 增加 SHA-256 完整性哈希；其中一份还额外提议生成依赖 SBOM。与状态加密主题的区别在于这是把多个零散防护点打包成一份纵深防御清单的元提案，而非专注单一加密机制。

*优先级信号*: P0(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-unexplored-perspectives

**Prompt 经 Argv 泄漏(ps aux 侧信道)与凭证传输通道加固** `×2`

完整 agent prompt（含 ADR、ROADMAP、memory、业务逻辑）目前作为 CLI argv 字符串传递，同一主机上任何进程都能通过 `ps aux` 读取全文，同时 childEnv 把凭证不加过滤传给子进程。两份提案（需求版与对应设计版，设计管线最终失败）共同提出：把 prompt 传输方式从 argv 改为 stdin 或 0600 权限的临时文件、加环境变量允许清单、临时文件安全擦除且崩溃路径下也能清理、把现有占位的 SandboxConfig 落到实处，并在 trace 写入路径加通用的 secret 样式字符串脱敏钩子。与其它 env 透传类主题的区别在于这里的核心攻击面是 prompt 内容通过 argv 暴露，凭证透传只是子项之一。

*优先级信号*: P0(x2)　·　*最高成熟度*: ideation-proposal (对应设计版 pipeline-failed，未产出方案)　·　*示例来源*: 2026-07-10-five-genuine-architectural-frontiers

**Forge 自身运维凭证生命周期健康检查** `×1`

secret-scan 只能挡住硬编码密钥被提交，但对 forge autonomous engine 自身运行所需的凭证（Claude API key 过期、git push token 权限范围、CI GITHUB_TOKEN 权限）完全没有生命周期管理——一次 24 小时的 evolve 长跑可能因 key 中途过期而静默失败，没有预检、告警或轮换支持。提案新增 `forge doctor --credentials`、`forge preflight` 凭证检查子命令和 `forge secrets rotate` 辅助工具，是唯一聚焦 forge 自身运维凭证健康度、而非子进程能看到哪些密钥的独立诉求。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-systemic-declaration-gaps

**Agent 部署期安全凭据注入通道(SecretProvider/secrets.yml)** `×1`

当前 secret-scan 只能在事后检测已泄漏的凭证，但当 agent 需要真实凭证去完成部署类任务（推送镜像、调用云 API）时，系统没有提供任何安全获取凭证的正规途径。提案新增声明式 `.agent/secrets.yml` 凭据清单，以及 CommandExecutor 上的 SecretProvider 接口用于注入环境变量或转发到外部密钥管理器，secret-scan 对该注入路径放行，trace 记录使用情况（值本身脱敏）以便审计。与其它主题关注堵住泄漏/限制透传不同，这是唯一一份解决 agent 如何被正当授予凭证这一相反方向问题的提案。

*优先级信号*: unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: genuinely-novel-expansion-directions

**配置信任边界加固(REPO_ROOT/YAML注释解析/--approved/AGENT_DEPTH)** `×1`

唯一一份聚焦配置层信任边界的提案，一次性加固四个已确认的攻击向量：FORGE_REPO_ROOT 环境变量可静默覆盖仓库根路径、projectYAMLValue 的逐行扫描解析器会把 `# lifecycle: production` 之类注释误当作真实声明生效、--approved 布尔标志没有任何身份校验即可跳过人工闸门，以及跨子进程继承的 FORGE_AGENT_DEPTH 没有签名可被伪造以逃避深度限制。修复方案分别是路径白名单校验、注释行跳过、生产 lifecycle 要求磁盘批准标记、以及基于锁文件（非 HMAC）的深度篡改防护。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-four-infrastructure-gaps-subprocess-example-bridge-config

**Implementer 受限测试命令自查权限缺口** `×1`

真实点火(ignition)记录显示，implementer agent 在 acceptEdits 权限下能写代码，但因不开放 Bash 而无法执行 `node --test` 之类命令自查，只能诚实放弃勾选 ROADMAP 完成项，把客观验证完全交给 harness-gates 和人工审阅。文档隐含了一个尚未实现的能力缺口：若能给 implementer 一个受限的测试命令白名单（例如只放行 `node --test` 而非完整 Bash shell），agent 就能在不开放任意命令执行权限的前提下自行验证增量完成度。这是唯一一份从叙事日志/复盘而非正式需求提案文档中提炼、且聚焦最小权限自查能力而非防泄漏/防越权方向的条目。

*优先级信号*: gap-implied(x1)　·　*最高成熟度*: narrative-log　·　*示例来源*: ignition

**forge approve 审批标记可伪造(无签名空文件即通过)** `×1`

`humanApproved()` 的实现仅是 `os.Stat(<root>/.forge/<stage>.approved)` 文件是否存在，没有签名、时间戳、审批人身份或防篡改机制——任何对文件系统有写权限的进程或用户都能伪造对系统中权重最高的 human_gate 的批准。这是唯一一份聚焦该具体攻击面的提案，需求阶段被记录但设计管线未产出方案，是与其余 env/进程隔离类提案完全不同的、针对治理闸门自身完整性的独立问题。

*优先级信号*: unstated(x1)　·　*最高成熟度*: design-pipeline-failed (requirement-stage only, no design produced)　·　*示例来源*: 2026-07-11-five-deep-systemic-codegrounded-boundaries

---

### 知识组织/语义层/ADR结构化 `knowledge-semantic-layer`

16 个独立主题，原始条目 37 条。

**核心知识引擎与语义检索升级(TF-IDF/BM25 → Embedding,含Harvest/Decay/多源整合)** `×12`

本类目下被独立重复提出次数最多的方向:当前 internal/prompt/retrieve.go 只做无词干化的关键词/TF-IDF匹配,ADR 检索只覆盖标题不含正文且 adrTopK 硬编码,memory.Query 只支持精确 kind+topic 匹配、从未与检索打通,ARCHITECTURE.md 列出的"Knowledge-Engine"引擎从未真正建成。十余份文档反复提出本质相同的方案:建 internal/knowledge 包,做 ADR全文/memory/agent-card 的语义索引与top-K检索、从 trace/scorecard/gate 收获(harvest)结构化知识并加 TTL 衰减剪枝、支持动态预算与多源整合排序/矛盾检测,少数提案还延伸到跨项目知识库共享和面向 implementer 的代码结构地图。已有多次独立的 architected 级具体包设计,是本类目下成熟度最高、共识最强的方向。

*优先级信号*: P0(x3)/P1(x4)/P2(x1)/P3(x1)/unstated(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-high-value-extension-directions, architectural-extensions-v38, forges-five-unbuilt-foundations, expansion-strategic-v5

**ADR可执行治理闭环(状态机 + 约束原语 + 漂移检测)** `×4`

指出 ForgeOS 的 ADR 目前只是纯 prose,没有 Proposal/Accepted/Rejected/Superseded/Deprecated 状态机,代码可以悄悄背离已记录的决策而无人发现(如 ADR-0004 声称的模式行为与实际代码不符),也没有任何门禁或运行时组件读取 ADR 内容。多份文档提出构建 internal/adr 包解析 YAML front-matter,建立状态机注册表、可执行约束原语(如"零外部依赖"自动检查)、drift-detection 命令比对声明规则与实际代码状态,并提供 forge adr list/status/supersede/validate 等 CLI 及 converge 信号,把 ADR 从死文档变成驱动运行时行为的治理资产。

*优先级信号*: P1(x3)/P0(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-codegrounded-architectural-blindspots-five-directions, five-genuinely-uncovered-architectural-frontiers-2026-07-10, forgeos-five-unseen-perspectives-2026-07-10

**需求/分析文档膨胀的元治理(INDEX注册表 + 生命周期状态 + 去重扫描)** `×4`

指出仓库的 requirements/analysis 文档已膨胀到数百份、十余万行,且自身的认知负荷阈值已被违反,新文档只能靠作者手动通读全部旧文档才能自证"未被覆盖",是不可扩展的过程。建议建立 docs/INDEX.md 或 direction-registry.yml 作为单一事实源,记录每份文档的 status(draft/active/superseded/archived)、supersedes 关系与过期策略,并配合 check_doc_index 治理检查、相似度去重扫描器和 TTL 归档策略,把知识库从"只写不读的坟场"变回可维护资产。

*优先级信号*: P1(x3)/P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-structural-blindspots, forgotten-five-meta-governance-and-blindspots, production-product-gaps-v43

**forge-ai Python智能层(落地ADR-0002多语言承诺)** `×2`

指出 ADR-0002 承诺的 Python "forge-ai" 智能层至今零代码,所有"智能"(路由打分、风险检测、记忆检索)都是手写 Go 规则。提出构建可选、非阻塞的 forge-ai 侧车,用于统计路由打分预测、基于 embedding 的语义 ADR/记忆检索、语义风险检测、trace 异常检测与成本估计,forge-core 在 Python 不可用时降级为现有规则行为。两次独立提案都强调应做成常驻 Unix-socket daemon 而非逐次子进程调用,以避免每次调用 50-100ms(甚至加载模型时数秒)的启动开销。

*优先级信号*: P1(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-12-five-highvalue-architect-pm-directions, 2026-07-12-forgeos-five-highvalue-extensions-senior-architect-pm

**结构化知识挖掘与跨会话学习(纯统计模式挖掘 forge learn)** `×2`

提出新增 internal/learn 包,对 memory.jsonl 做纯统计意义上的主题频率趋势与"模型×门禁通过率"相关性挖掘,明确排除 ML/embedding(理由是数据量不足以支撑训练)。复用已建成但从未被消费的 memory.Supersedes 字段做去重,通过 forge learn patterns/correlate/suggest 暴露给用户,在证据不足时诚实降级为 insufficient data,并要求每条输出都带 trace/memory 序号级证据引用以避免黑箱洞察。

*优先级信号*: P2(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-next-architect-frontiers, 2026-07-11-forgeos-five-next-frontier-expansion-directions

**语义唯一性门禁(embedding相似度防止方向文档重复)** `×2`

针对"近乎相同的方向内容曾在多份文档中通过关键词 grep 差异性检查"这一具体失败模式,提出用零网络调用的本地 embedding 模型算余弦相似度,在新方向文档保存前做拒绝/合并判定(阈值约0.70-0.75)。同一份文档进一步提出把已覆盖方向簇的 embedding 摘要直接注入"生成N个新方向"的提示词,要求新候选与所有已有簇的最大相似度低于0.7,从生成源头而不仅是事后门禁上约束重复度。

*优先级信号*: P0(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-frontiers

**结构化语义知识层(KnowledgeItem关系模型 + ADR前置元数据 + ROADMAP状态机)** `×2`

提出用带 Subject/Predicate/Object/Provenance/Status 关系的 KnowledgeItem 模式取代当前完全无结构的知识存储——自由文本的 memory 条目、纯 prose 的 ADR、二元 checkbox 的 ROADMAP。配合 ADR YAML front-matter 解析、ROADMAP 五态状态扩展和概念倒排索引,使系统能检测跨会话矛盾决策(如"用 PostgreSQL"与"用 SQLite"并存)并对概念进行推理,而不只是存取原始文本。与单纯的检索算法升级不同,这是一个知识表示/关系建模层面的方案。

*优先级信号*: P1(x2)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-10-five-genuine-architectural-frontiers

**三框架并存的结构性债务(.agent vs .ai vs ai-dev)** `×1`

指出仓库中并存三套互不引用、逐渐漂移的 agent 框架——.agent/(实际运行时依赖)、.ai/(无代码集成的模板框架)、ai-dev/(带自有 pipeline/prompt/pi-batch.py 的已废弃近重复品)——且没有任何治理检查覆盖后两者。建议把有价值内容迁移进 .agent/、对另外两套做软废弃标记、新增角色映射的 drift-guard 检查,并最终阻止对遗留框架的非废弃性编辑。虽被打上知识语义层标签,但本质是框架级重复而非知识表示问题。

*优先级信号*: P0(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-five-structural-debt-and-product-frontiers

**Context检索的使用反馈闭环(ADR注入 vs 实际引用追踪)** `×1`

指出当前基于关键词的检索器把 top-K 打分文档硬塞进每次 prompt,却对 agent 是否真的用到这些文档零反馈,且 ContextCache.Invalidate() 已存在但从未被调用。建议追踪哪些 ADR 被注入 vs 被 agent 输出实际引用,构建使用信号来提升历史上有用的 ADR 排名、降权被忽略的 ADR,与既有的 Eval→scorecard→Router 学习闭环形成对称机制。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-expansion-directions

**上下文缓存一致性(ADR写入触发的失效传播)** `×1`

指出 ContextCache.Invalidate() 在 v1 从未被调用是因为 agent 还不会写 ADR,但设计已为 v2 声明了 writes_adr,意味着一旦 agent 开始在运行中途写 ADR,后续阶段会持续拿到过期的缓存 ADR 集合。建议不要头痛医头式修补个别调用点,而是构建一个通用的"agent写入 → 运行时状态失效"发布订阅总线来系统性解决这类问题。

*优先级信号*: unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-code-grounded-architectural-extensions-2026-07-10

**分层/分档位的Prompt组装架构(Tier-Aware PromptAssembler)** `×1`

指出 prompt.Build 是对所有模型档位(Haiku/Sonnet/Opus)都产出同一套三通道上下文的单一字符串拼接函数,没有基于档位的裁剪或模板分区,且 prompt_context.go 里还独立维护着第二套重复的上下文构建路径。建议引入 PromptAssembler 接口和按 TokenBudget 驱动的裁剪器(按 ADR→memory→feeds_forward 优先级丢弃),并按模型档位划分角色卡片区块。这是 Prompt 组装层面的架构重构,区别于检索算法本身的语义化升级。

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-12-five-closure-gap-expansion-directions

**Retriever接口解耦重构** `×1`

指出 prompt.Retrieve 是绑死在 BM25-lite 打分算法上的包级纯函数,任何未来接入语义/embedding 检索的尝试都会破坏所有调用方,或被迫把 embedding 依赖引入零依赖的 prompt 包。设计提炼出一个 Retriever 接口、以 NewBM25Retriever() 作为默认实现,让未来的语义检索可以活在独立包里而不冲击现有调用链。这是知识引擎升级的一个前置解耦步骤,而非检索能力本身的实现。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-four-uncovered-architectural-extension-directions

**语义化变更叙事管线(Semantic Change Narrative Pipeline)** `×1`

在原始 git diff/trace 数据与面向人类的收敛报告之间插入一个结构化语义层:机械字段严格由 git diff --stat 计算,LLM 只贡献一个显式标注 generated_by:<model> 的 summary 字段,按运行持久化并通过新的 forge log 命令聚合成工作流级 changelog,与 Shadow-Mode 共享 diff 采集基础设施。这与其他方向的"语义检索/知识索引"不同,面向的是生成人类可读的变更叙事,而非给 agent 做上下文检索。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-product-architecture-expansion-directions

**存量文档聚合收敛管线(一次性聚类合并)** `×1`

提出对已存在的约200余份需求文档做一次性聚类,把每簇合并成 converged/ 目录下的权威文档,原始文档只归档打 deprecated 标记绝不删除,并通过 frontmatter 加入生命周期状态。这是针对存量语义重复文档的一次性批处理清理,区别于面向未来新增文档的语义唯一性门禁,也不同于持续性的 INDEX 注册表治理。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-frontiers

**方向文档与ADR/Issue的双向链接契约** `×1`

提出给需求文档加结构化 frontmatter(id/status/cluster/adr/issues/supersedes),并在 ADR 侧加反向的 related-requirements 字段,配合一个 CI 检查:被标记为"ready-for-adr"但一定期限内未出现对应 ADR 的文档自动降级为 stale。用来堵上"分析文档产出后与真实工程动作之间没有消费契约"的缺口,区别于单纯的去重或注册表治理。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-systemic-frontiers

**ADR可测试性跨层桥接(ADR Test-to-ROADMAP Bridge)** `×1`

提出在 harness 层加一个 Node.js watchdog 脚本,解析 ADR 相关测试的输出,把未处理的失败项追加为 ROADMAP 的 inbox 条目,从而把只打印到 stdout 的 ADR 测试与驱动优先级排序的 Markdown ROADMAP 之间的回路闭合,同时刻意不引入 Go 到 Markdown 的反向依赖。是一个连接测试结果与治理文档的小范围桥接方案,区别于更宏大的 ADR 状态机治理闭环。

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgeos-five-architect-product-expansion-2026-07-10

---

### Agent输出确定性/可复现性 `output-determinism`

8 个独立主题，原始条目 20 条。

**确定性回放执行器 ReplayExecutor(离线重放,免真实 LLM 调用)** `×5`

提议新增第三种 AgentExecutor 实现——ReplayExecutor——从记录的 trace(如新增的 trace.full.jsonl / FullTraceEntry,包含 prompt、response、routing_snapshot)中读取历史 phase 输出并回放进 LoopEngine.Run 或仅重放编排决策,使调试异常的长时间 evolve 运行、验证编排逻辑改动(loop-back、mode-gating、budget 分支)是否仍复现相同的 phase 序列和收敛判决,都无需真实消耗 LLM 费用也不受 LLM 非确定性干扰。需要扩展 trace schema 以捕获 agent 的实际输出载荷(文件编辑、verdict)而非仅 Status/DurationMs/CostUsdMicros,并可选配 trace-digest 认证与两次录制运行间的语义 diff。该方向已有两份提案完成完整设计(architected)。

*优先级信号*: P0(x1)/P1(x2)/P2(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: expansion-five-uncovered-2026-07-10, five-code-grounded-architectural-extensions-2026-07-10, genuine-architectural-gaps-v28

**声明式 Agent 输出解析契约(取代手写 last-line 解析器)** `×4`

指出 reviewer verdict / executive verdict / confidence score 等信号目前由三到五个独立手写的"最后一行精确前缀匹配"解析器提取,格式仅存在于 agent card 的自然语言描述中,没有机器可读 schema,且解析失败时是 fail-open(静默放行未评审的 phase)。提议在 agent card 或 asset.Phase 上声明统一的 OutputContract/OutputSchema(支持 JSON/YAML 代码块优先、行 token 兜底等多策略解析),用单一 ParseOutput 函数取代多个解析器,并配套 forge validate --contracts / forge check 做 card 描述与机器契约的一致性、漂移检测,防止模型格式微调导致的契约破坏在无人察觉的情况下阻塞整晚的收敛流程。该方向已有两份提案完成了完整设计(architected)。

*优先级信号*: P0(x2)/P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-codegrounded-structural-gaps, architectural-expansion-perspectives, genuinely-novel-expansion-directions

**编排引擎确定性重放模式(FORGE_DETERMINISTIC/时间戳与遍历顺序冻结)** `×3`

指出 memory.Load 的缓存 key 依赖文件 mtime(内容相同但 mtime 不同即产生不同缓存结果)、目录遍历顺序未指定、trace/checkpoint 时间戳每次运行都不同,导致即便输入、agent 输出、git ref 完全相同也无法保证得到相同的 trace 与收敛判决,阻碍调试复现和合规审计。提议引入 FORGE_DETERMINISTIC=1 模式冻结时间戳/耗时、对 map key 与目录列表排序、用 deterministic:"false" 结构体标签标记非确定字段,并新增 forge replay <trace.jsonl> 命令及 --seed/--git-ref 参数以重建执行序列。其中一份提案进入了设计流水线但未产出设计(design-pipeline-failed)。

*优先级信号*: P2(x2)/unspecified(x1)　·　*最高成熟度*: design-pipeline-failed　·　*示例来源*: 2026-07-11-codegrounded-five-highvalue-extension-directions, 2026-07-11-five-codegrounded-product-expansion-directions

**Seed/Temperature 传播与结构化稳定性校验契约** `×2`

提出为 Agent 输出建立端到端可复现性契约:将 --seed/--temperature(按 run ID 作用域)贯穿 claudeArgv 传给底层模型调用,配合 SHA-256 prompt 指纹与可选 --lock-prompt 缓存模式锁定 prompt 版本,并引入 StabilityChecker 接口对同一 phase 两次运行的输出做结构化(而非逐字文本)比对。核心动机是 ForgeOS 现有的 gate 重跑、FileDelta、converge 收敛判断都隐含假设了 agent 输出可复现,但 LLM 本身具有非确定性,这会侵蚀 --resume 的完整性和 loop-back 收敛判定的可信度。该提案曾进入设计流水线但未能产出完整设计(pipeline-failed)。

*优先级信号*: P1(x2)　·　*最高成熟度*: pipeline-failed　·　*示例来源*: 2026-07-10-five-genuine-architectural-frontiers

**并行编排 Ledger 写入顺序非确定性** `×2`

指出在 RunParallel 并发模式下,多个 goroutine 并发写入 phaseOutputLedger/gateLedger/verdictLedger 时,虽然 mutex 避免了数据竞争,但写入顺序取决于"谁先抢到锁"的调度结果而非声明顺序或实际上游依赖关系,导致下游 phase 构建 prompt 时注入内容的顺序随调度而异,两次相同的 run 可能产生不同的 prompt 上下文进而产生不同的 LLM 输出,损害审计可重现性。这是一个独立于全局时间戳/mtime 问题之外的、专门针对并行执行写入序的确定性缺口,曾进入设计流水线但未产出设计。

*优先级信号*: P1(x2)　·　*最高成熟度*: design-pipeline-failed　·　*示例来源*: 2026-07-11-five-deep-systemic-gaps

**输出格式容错归一化层(大小写/前缀/正则模糊匹配)** `×2`

提议在现有 VERDICT/CONFIDENCE/roadmap-checkbox 的精确字符串匹配解析器前加一层轻量级容错输入归一化层——大小写归一、前缀匹配、正则模糊提取——以应对 LLM 输出的自然变体(如小写 'approve'、'CONFIDENCE: 85%'、`* [x]` 而非 `- [x]`)。与"声明式 schema 整体替换"方案不同,这是更轻量的补丁式方案:不改变解析器架构,只在其前面加缓冲层,并强调当前静默 fail-open 却没有任何信号丢失的告警,是需要单独跟踪的差异化机制。

*优先级信号*: P2(x2, one flagged by cross-validation for P1 elevation)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: execution-semantic-gaps, execution-semantics-gap-analysis

**Prompt 内容版本锁定与黄金文件回归检测** `×1`

指出 prompt 由多个 lane 动态组装后即发即弃,没有任何机制记录"发给 agent 的 prompt 长什么样"——trace.Event 缺少 PromptHash/PromptLength 字段,因此无法回答"这次和上次的 prompt 有何不同",也无法判断评审裁决从 APPROVE 变为 REDESIGN 究竟是 prompt 变化导致还是 agent 本身的问题,更缺少 prompt 回归测试。提议为每次构建的 prompt 计算 SHA-256 写入 trace 的 prompt_hash 字段、为每个 lane 打版本戳、新增 forge validate --prompt 输出黄金快照供 CI diff 检测,以及让 phaseOutputLedger 记录输出对应的 prompt 指纹——是一种偏向"CI 回归测试/审计追溯"而非"运行时随机性控制"的独立机制,与 seed/temperature 契约和重放执行器均不同。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-production-gaps

**自治循环再现性赤字:run ID/配置快照/memory 快照/diff-runs** `×1`

指出 forge run/evolve 缺少全局 run ID,trace seq 每次运行都会重置且不同 run 的事件混杂难以区分,checkpoint 不记录输入参数(executor 类型、agent-cmd、model override)导致 resume 时无法确认配置一致性,而 memory 是跨 run 累积而非按场景绑定的,天然导致两次 run 的输入环境不同、无法做受控复现或 A/B 对比。提议引入全局 run_id、输入参数的 checkpoint 快照、memory 快照/回放机制,以及新增 forge diff-runs 命令做跨运行差异分析——这是一个比其余重放/契约类提案更宏观、聚焦"整体运行环境可比性"的独立提案,机制与目标均不与其他主题重叠。

*优先级信号*: unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: second-order-architectural-gaps

---

### 供应链安全 `supply-chain-security`

6 个独立主题，原始条目 20 条。

**YAML Python Shim 消除 / Go 原生解析器唯一化** `×6`

forge-core 声称零 Go 外部依赖，但实际运行时 workflow YAML 解析在 Go 原生解析器路径未覆盖或失败时会 shell out 到 python3 执行 harness/yaml2json.py，甚至 internal/yamlpath 包也硬编码 exec.Command('python3',...)，使无版本锁定的 PyYAML 成为每个 forge-init 项目都继承的隐藏攻击面，且在无 python3 的环境（容器/Windows/Alpine）直接跑不起来。六份文档从不同角度反复提出同一修复：把 Go 原生 yaml2json 解析器扩展为经黄金文件/差分测试/YAML 官方兼容子集验证的主路径，将 python3 fallback 降级为显式 opt-in（如 --use-python-shim 默认关闭）甚至移除，并清理 yamlpath 包内的硬编码 python 调用及用 yaml2json.py 作仓库识别标记的历史写法。其中一份援引 Sprint 27 真实事故（block-scalar bug 因回归测试误用 t.Logf 未被拦截）作为必须做阻断性差分测试的依据。

*优先级信号*: P0(x1)/P1(x3)/P3(x1)/low-medium(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-codegrounded-edge-cases-and-extensions, five-code-grounded-architectural-extensions-2026-07-10, forgotten-five-meta-governance-and-blindspots, forgotten-five-structural-debt

**二进制发行、发布流水线与版本管理/自更新** `×5`

forge 二进制目前是直接提交进仓库的静态文件，forgeVersion 硬编码永远是 "dev"，没有安装脚本、Homebrew/APT/GitHub Release 分发渠道、checksum 或签名校验，CI 只验证编译通过但从不发布 artifact；forge-upgrade.mjs 也只同步治理资产（agent 卡/workflow/policy）而完全不涉及二进制本身，导致「新 workflow 语法 + 旧 binary 解析器」的版本分裂风险持续存在。五份文档从各自切入点反复提出同一整体方案：搭一条 Build&Sign（cosign/GPG + SHA256 + 可选 SBOM）→ Release Manifest → 多平台分发的发布流水线，用 ldflags 注入真实版本号；新增 forge release / forge self-update（原子替换、--version=vX.Y.Z 降级回滚）/ forge version --check(-upgrade) / forge doctor --binary 等子命令；并引入 .forge/version.json 或 scaffold 版本绑定校验，让 forge preflight 能检测治理资产与二进制版本是否兼容一致。

*优先级信号*: P1(x2)/P2(x1)/high-MVP(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: operational-product-five-gaps, product-deployment-transparency-five-gaps, production-operational-gaps, productization-five-frontiers-2026-07-10

**SCA/CVE 扫描能力激活（从沉睡引擎到真正跑起来）** `×4`

harness/sca.mjs 已经是一套功能完整的软件成分分析引擎（OSV advisory 解析、semver 范围匹配、多生态系统/多 manifest 支持，300+ 行代码），但因仓库中从未提供 FORGE_SCA_DB 或 .agent/security/advisories.json，probeSCA() 在 forge accept 中永远返回 N/A——漏洞扫描能力架构完整却从未真正执行过一次扫描。四份文档反复确认同一个「沉睡引擎」缺口，并提出让它真正运行：接入 OSV 公共 API 或通过 forge-init 引导离线 advisory DB 快照做本地同步，同时明确保持 sca.mjs 为 Node 脚本（不为此新增 Go 依赖，govulncheck 可作为备选方向），并把范围扩大到 harness 自身工具链——用 npm audit/pip-audit/trivy 等真实工具扫描 harness 自己的依赖、用 cyclonedx-bom/syft 生成 SBOM，而不仅仅扫描被治理的目标项目。

*优先级信号*: P0(x1)/P2(x2)/Low(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-codelevel-systemic-extension-directions, 2026-07-11-forgeos-five-code-grounded-systemic-gaps, five-high-value-extensions-v44, scan-fresh-perspectives

**Checkpoint 证据链 / 可验证治理证明（Attestation & Provenance）** `×3`

forge 目前只打印一行不带签名的 "forge accept: ACCEPTED"，第三方（审计方、监管方、下游 CI）无法验证治理流程真的跑过、结果未被篡改。三份文档独立提出几乎同一套机制：对 checkpoint/trace 事件做 SHA-256 哈希链（部分方案进一步用 ed25519 签名），生成 ArtifactManifest 记录「谁产出了什么、哈希是什么」，配一个 forge verify provenance 命令报告 VERIFIED/INCOMPLETE/TAMPERED，并支持导出 in-toto/SLSA 格式证明；其中一份主张先做无需密钥管理的轻量哈希链、把签名和格式化证明导出作为有真实第三方验证需求时的后续升级路径，且默认零行为变化（--verifiable opt-in）。这本质是同一个「可验证治理证据链」能力被三份不同文档独立重新发现。

*优先级信号*: P0(x2)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgotten-frontiers-five, forgeos-five-unseen-product-architect-extensions

**SBOM + 构建产物签名与可溯源元数据** `×1`

目前 forge accept 通过只是一个阶段性检查点，不携带任何持久化的供应链安全信号。该提案建议在 gate 通过后自动生成标准化 SBOM（CycloneDX/SPDX，复用 sca.mjs 已有的 manifest 解析逻辑），用 Go 标准库 crypto 对构建产物做哈希、可选签名，并把版本号/commit/已通过的 gate 列表等元数据直接嵌入构建产物本身，使供应链安全属性从「运行时一次性检查」变成「产物自带的可核验持久属性」。与「Checkpoint 证据链」主题不同，这里验证的对象是构建产物及其依赖清单本身，而非治理过程的执行记录。

*优先级信号*: unstated　·　*最高成熟度*: ideation-proposal　·　*示例来源*: five-high-value-extensions-v44

**文档/Markdown 内容的 Secret 扫描覆盖盲区** `×1`

gate.mjs 的文件遍历与 secret-scan.mjs 目前完全排除 .md 文件，但文档目录恰恰是最容易被人手滑写入硬编码 URL、API key、示例凭证的地方。该提案建议把 secret-scan 的覆盖范围扩展到文档，同时为文档场景使用更宽松的规则集以减少教学用占位符触发的误报，并将其放在 --scan-docs 这样的可选开关之后而非直接挂进主 CI 路径，避免对现有流水线造成噪声冲击。

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: forgotten-five-meta-governance-and-blindspots

---

### Prompt注入防御/信任边界(非OS层) `prompt-credential-safety`

8 个独立主题，原始条目 16 条。

**多信任域Prompt装配结构化边界(Lane/信任分级隔离)** `×3`

buildPrompt 把 7 个以上不同信任级别的内容(人类维护的高信任 AGENTS.md/agent 卡 vs agent 可写的低信任 memory/ROADMAP/前序阶段输出/reviewer findings)用纯 \n\n 拼接进同一文本平面,既没有结构分隔符,也没有逐来源的 token 预算或装配后校验,其中 agent 可写的 ROADMAP 路径甚至未经 sanitizeAgentOutput 处理,是最活跃的注入向量之一。核心提案是引入声明式的 LaneRegistry,给每个 context lane 打 TrustLevel 标签、设定按相关度截断的逐 lane token 预算、加结构化定界符和一个装配后确定性拒绝/截断非法 prompt 的校验器。另一份高度相似的独立提案在此基础上提出用结构性 XML 定界符取代纯文本前缀、按来源(系统/已验证/可信/不可信)分配信任层级,并叠加 memory 条目的 HMAC 完整性校验,本质上是同一结构化信任边界思路的变体加强版。

*优先级信号*: P0(x1)/P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: five-novel-architectural-frontiers-2026-07-10, strategic-expansion-v39

**Agent反馈闭环消毒与来源标记(含语义检索防御)** `×3`

buildPrompt 从 phaseOutputLedger(agent stdout)、reviewFindingsLedger(reviewer 输出)、gateLedger(gate 命令输出)、memoryContext(memory.jsonl 历史)四个渠道把 agent 生成的内容原样注入下一次 prompt,构成一个完全无过滤的反馈循环,恶意或误导性的 memory 条目可能在后续 5-32 次迭代中持续污染下游 agent 判断。两份几乎相同的提案建议三道防线:sanitizeAgentOutput 剥离控制字符、按来源和迭代号给 context 行打标签(如 [memory:iteration=5])、以及在 prompt 中显式声明这些标记内容仅供参考需独立验证。同一文档的一份复核性补充发现指出,memoryContext 使用的 BM25-lite 关键词检索本身对语义/对抗性植入文本(如把"忽略此前所有指令"藏进 Topic 字段)没有抵抗力,简单的模式消毒无法覆盖检索层被"命中"的攻击面。

*优先级信号*: P0(x2)/unstated(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: scan-new-angles

**主动Prompt注入检测与审计轨迹** `×2`

ForgeOS 目前只有被动防护,代码里没有任何主动扫描、记录或审计注入尝试的机制——agent 输出和 memory 条目被直接拼接进下一轮 prompt,trace 事件类型中也不存在专门的注入事件。提议新增一个轻量级注入模式特征库,在输入侧对可疑内容做扫描并打标(不阻断只标记),把命中记录为新的 KindInjection trace 事件,并配一个 forge audit injections 命令供事后复盘。同一 topic 先后以 requirements-proposal 和 results-architected(design-stage-failed)两个阶段提出,说明该方向曾进入设计阶段但未能落地。

*优先级信号*: P0(x2)　·　*最高成熟度*: design-stage-failed　·　*示例来源*: 2026-07-11-five-genuine-systemic-boundaries

**VERDICT与Memory写入完整性防篡改闸门** `×2`

agent 自报的关键声明(VERDICT 裁决、ROADMAP 完成度勾选、写入 memory 的 findings)目前没有任何独立验证层,被注入或谎报的输出会被系统原样采信,24 小时无人值守场景下这条信任链完全断裂。提议为 VERDICT 建立格式/类型校验(拒绝伪造的 "VERDICT: APPROVE")、为 ROADMAP 完成度核实其声称产出的产物路径确实存在、在写入 memory 前按格式和来源标记做过滤(防 memory 投毒)、并把原始 agent 输出全量归档进 trace 供审计回放。架构落地版本进一步把 VERDICT 解析抽成独立的 internal/verdict 包、把 sanitizeAgentOutput 升级成三阶段管道(剥离控制字符→白名单校验 VERDICT token→拒绝超长输出)、给 Memory.Append 加来源白名单,并给 checkpoint 和 trace 加 SHA-256 校验和/哈希链做防篡改。

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: five-systemic-trust-and-scalability-gaps

**Agent声明与真实变更一致性核验(Claim-vs-Diff Veracity Gate)** `×2`

ForgeOS 现有的 8 类代码质量闸门都只检查代码本身,没有一个检查 agent 用自然语言描述的"我做了什么"是否真的和实际 git diff 相符,这是一个明显的幻觉/谎报盲区。提议新增一个 Claim Extraction Gate,从 agent 输出中抽取可验证的声明并与实际 diff 做一致性匹配,为每个 phase 产出一个 truthfulness_score 供后续路由决策参考,并对 reviewer 提出的每条 finding 做 closure tracking(核实后续 phase 是否真的解决了它)。架构落地版本把原本分散的信任检查(cappedBuffer 截断诚实性、parseClaudeCostUsd 健壮性、RunFrom 任务一致性)统一收敛到一个新的 internal/veracity Checker 接口下,产出 Pass/Fail/Warning 裁决并写入 trace,严格度可按 mode×lifecycle 调节,默认只警告不阻断以避免破坏现有运行。

*优先级信号*: P1(x2)　·　*最高成熟度*: architected　·　*示例来源*: five-uncovered-architectural-frontiers

**跨Agent信任链加固与来源溯源(Provenance + Trust-Tier)** `×2`

多 agent 共享的 memory/feed-forward 层目前一视同仁地信任所有内容——被攻陷 agent 编造的一条 memory 记录,和 harness 实测验证过的 gate 结果,在后续迭代里权重完全相同,这为注入内容的跨迭代传播和放大打开了通道。提议给每条 memory 条目和 trace 事件打信任层标签(如 HARDWARE/GOVERNANCE/AGENT_REPORTED/INFERRED/UNVERIFIED),建立可追溯的 provenance chain(记录来源 agent/phase/迭代号/是否经 harness 验证),并让置信度随迭代自动衰减。架构落地版本把这套思路具体化为 memory.Entry.Provenance 结构体、新增 sanitizeFeedback 函数堵住 collectPhaseFeedback 未经 Confidence 过滤直接写 memory 的注入口子,并引入 pipeline/direct/external 三层默认拒绝的信任 tier,对 external 层内容在 prompt 中加 [unverified] 前缀提示 agent 谨慎对待。

*优先级信号*: P1(x1)/unstated(x1)　·　*最高成熟度*: architected　·　*示例来源*: fresh-five-systemic-extensions-2026-07-10

**凭据编排层(Credential Orchestration Layer)** `×1`

本类目里唯一真正关于"凭证管理"而非"prompt 内容注入"的方向:定义一个 SecretProvider 接口(Resolve/ResolveAll/Close),提供 Env/File/socket-proxy 等实现,接入 CommandExecutor 的 childEnv,并允许 workflow 的 phase 用 secrets: 字段声明式请求凭据,配一个新的 forge secret set CLI 子命令。目标是把 ForgeOS 从依赖提前手动设置好的环境变量,升级为按需注入凭据的模式,这是支撑 24 小时无人值守长跑场景的必要前提。该方向只有一份 results-architected 提案,没有对应的 ideation 阶段版本。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: genuinely-novel-expansion-directions

**工作区沙箱与文件变更审计(Workspace Sandbox + Mutation Audit)** `×1`

CommandExecutor 目前给了 agent(在 acceptEdits 模式下)几乎无限制的文件写权限,唯一的约束是 Bash 命令白名单,完全无法防御被注入诱导写出恶意 postinstall 脚本或篡改 go.mod 之类的攻击,且一次 run 跑完也没有事后的文件变更 diff 审计。提议实现 pre/post git 快照对比、文件写入的白/黑名单(明确禁止 agent 写自己的闸门配置文件,如 .github/harness/policies.yml)、以及一份 append-only 的变更摘要日志,并明确声明不追求完整 OS 级沙箱隔离(留给 v3 的 Firecracker 方案)。这是本类目里唯一聚焦"注入后果落地为文件系统破坏"而非"注入内容本身"的方向。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: expansion-strategic-perspectives

---

### 知识可移植性 `knowledge-portability`

6 个独立主题，原始条目 14 条。

**跨项目组织知识库与模式共享 (forge learn push/pull / publish-pattern-subscribe / ScorecardRegistry)** `×7`

本类目中重复度最高的方向:memory.jsonl/trace.jsonl/scorecards.json 严格限定在单项目内(memory 包自身注释甚至将"跨项目缓存冲突规避"列为设计目标之一),导致每个新项目都要为已被他人发现过的教训重新冷启动。各稿收敛到同一核心机制的不同命名与包装——`~/.forge/memory/` 全局共享存储 + `forge publish-pattern/subscribe`、`forge learn push/pull` + `forge init --org`、ScorecardRegistry、或"Global Knowledge Pool"——共同的设计承诺包括:无中心服务器、默认隐私(推送前脱敏 secrets/路径)、陈旧洞见置信度衰减、以及组织级 scorecard/成本聚合(如"opus 用作 reviewer 平均每次运行 $0.18,跨全部项目统计")。最精细的变体还提出了共享的历史裁决日志与"Drift Sentinel"(标记 sibling 项目间冲突的 ADR 决策),以及教训→决策→ADR 的知识飞轮升级路径。

*优先级信号*: P0(x2)/P1(x2)/P2(x2)/P3(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-highvalue-governance-evolution-extensions, expansion-production-perspectives, five-genuinely-unexplored-code-level-architectural-expansions, novel-five-perspectives-2026-07-10

**多实例知识联邦：Git-based P2P 知识 export/import** `×3`

为 memory.Entry 新增 Origin/Namespace/ShareLevel 字段(向后兼容、omitempty),并新增 `forge knowledge export/import` 及可选的 `forge scorecard --federate` 聚合器,使一个 ForgeOS 治理仓库中学到的教训与 scorecard 统计能通过基于 git 的知识仓库、点对点地选择性共享给同组织的其他仓库,而非永久孤立在各仓库本地的 memory.jsonl 中——全程无中心服务器。架构化后的版本进一步补齐了确定性冲突消解梯队(supersedes > confidence > timestamp > origin)和 `forge knowledge prune` 生命周期清理命令(基于 TTL/置信度),明确定位为最终一致、无中心协调者。

*优先级信号*: P3(x3)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-five-novel-perspectives-product-architect, 2026-07-11-five-product-architecture-expansion-directions

**跨会话叙事连续性 (SessionID / --resume 链接 / 上一轮摘要注入)** `×1`

指出每次 `forge run`/`forge evolve` 调用都是叙事孤岛:memory.Entry 没有 SessionID,checkpoint.json 没有 RunPurpose/PredecessorRunID,`forge status` 只显示时间戳而不说明本次运行的目的或与此前运行的关系。提议新增轻量的 internal/session 包,为每次运行分配 session ID,通过 `--resume` 关联前后继任 session,并向 agent prompt 注入一行"上一次 session 摘要",让连续多次运行读起来像连贯叙事而非互相隔离的执行。与其他方向不同,这是同一项目内(或跨 resume)的会话连续性问题,而非跨项目间的知识迁移。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: novel-extensions-v36-deep-architectural

**新项目冷启动知识引导协议 (Bootstrap Knowledge Seeding)** `×1`

针对新初始化项目"前 5-10 轮盲跑"的问题——memory/trace/scorecard 文件全为空、RoadmapCompletion 从 0 开始、没有基线可供路由或收敛参考——提议新增 bootstrap 包,按检测到的项目类型(Go/Python/Node 等)预置初始知识库,诚实标记为 `kind: bootstrap` 以区别于真实积累的知识,并在真实项目数据积累后自动衰减;`forge init --skip-bootstrap` 保留现有零知识行为。与组织级知识共享方向不同,这里种子知识来自通用的按类型模板,而非从其他真实仓库迁移的实际知识。

*优先级信号*: P1(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: novel-five-frontiers-v34

**可移植工作空间打包 export/import (完整运行时状态归档)** `×1`

提议将一次 forge session 的完整运行时状态(memory.jsonl、trace.jsonl、checkpoint.json、routing scorecards——目前均被 gitignore、仅存于本地文件系统)打包导出,使其能够在不同机器、CI runner 或团队成员之间传输与恢复,目的包括跨 CI 的学习连续性、协作评审他人的 evolve 运行过程、以及积累学习状态的灾难恢复。这是对完整状态包的对称导出/导入操作,与其他方向中更精细的 Fork/Seed/Merge/Compose checkpoint 操作机制不同。

*优先级信号*: P2(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: truly-novel-five-directions

**Checkpoint Warm-Start 派生/播种 (Fork/Seed/Merge/Compose)** `×1`

提议在 persist 包新增四个 checkpoint 操作——Fork(克隆已有 checkpoint)、Seed(注入外部提供的初始 checkpoint)、Merge(合并两个 checkpoint,冲突字段标记为 MergeConflicts 而非自动仲裁)、Compose(智能合并多个种子 checkpoint)——并通过新增的 `forge evolve --seed-checkpoint`/`--seed-memory` 标志实现用另一项目的积累状态"热启动"新项目的运行;`memory.Inject` 同时支持 namespace 路径 rewrite,防止跨项目上下文污染。与其他两个方向相比,这是对具体运行/checkpoint 状态的操作、且带有显式冲突标记语义,而非对抽象化教训的组织级共享,也不同于纯粹的完整状态导出/导入。

*优先级信号*: P2(x1)　·　*最高成熟度*: architected　·　*示例来源*: 2026-07-11-forgeos-five-product-architectural-extension-directions-verification

---

### 沙箱运行时 `sandbox-runtime`

5 个独立主题，原始条目 9 条。

**沙箱化 Agent 执行分层隔离路线图 (Landlock/seccomp → 容器 → Firecracker microVM)** `×4`

四份文档独立诊断出同一个核心问题：CommandExecutor 目前让 agent/gate 子进程在宿主机进程空间裸跑，唯一的『隔离』是 Unix 进程组信号 (Setpgid)，没有 cgroup/seccomp/容器/VM 级边界，agent 拥有完整文件系统、网络、环境变量与宿主密钥访问权，可被 prompt 注入诱导执行任意命令、覆写 .agent/AGENTS.md 或植入后门；ARCHITECTURE.md 早已把 Sandbox 列为规划中的 v3 引擎但零实现。四者收敛到几乎相同的分阶段加固路径：先用 fake-agent 测试哈内斯验证并加固已有的 --disallowedTools 只读拦截，再为 gate-only 阶段引入 LANDLOCK/seccomp 等轻量 OS 级隔离（避开 microVM 启动开销），随后是 Docker 容器化（资源/网络隔离），最终是 Firecracker microVM 承载 agent 阶段的硬件级隔离并配合并行 evolve 的 VM 级资源预算，重量级层级明确推迟到 v3。这是本分类中复现最多的方向，体现出多份独立文档在实质上收敛到同一治理缺口。

*优先级信号*: P0(x3)/P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: forges-five-unbuilt-foundations, expansion-deep-analysis, next-horizons, strategic-expansion-directions

**CommandExecutor 未消费字段 → 分布式沙箱执行池 (AgentPool + 跨机资源配额)** `×2`

两份同题文档指出 CommandExecutor 早已声明 `Type`(docker/firecracker) 与 `Image` 字段，但从未被任何隔离或远程执行实现真正消费——所有执行仍是本机无隔离子进程，没有 CPU/内存/网络配额，单个失控 agent 可拖垮整个 forge 进程并波及并发工作流。两者都提议构建真正的 AgentPool 抽象（Local/Docker/SSH/Kubernetes 执行池）加上 resource_quota DSL（cpu/memory/timeout），与既有美元预算并列校验，从而支持横向扩展与跨机/远程执行，而不只是单机层面的进程围栏。与『分层隔离路线图』主题的区别在于其落点是执行池化与分布式调度，而非渐进式安全隔离本身；该方向曾进入设计流水线一次，但未产出被接受的设计（design-pipeline-failed），目前仍未实现。

*优先级信号*: unstated(x2)　·　*最高成熟度*: design-pipeline-failed　·　*示例来源*: 2026-07-11-five-architectural-product-expansion-directions

**Phase 级工作区隔离 (Git Worktree Isolator，防重试脏读)** `×1`

提议新增 Workspace/Isolator 抽象——默认的空操作 PassthroughIsolator 与推荐的 GitWorktreeIsolator——使每个编排 Phase 在独立目录中执行，而不是直接作用于仓库根目录。这直接解决了当前 loop-back 重试会看到前一次失败 agent 遗留的部分/脏编辑而非干净起点的问题，属于正确性/可重复性诉求而非安全/多租户隔离诉求。方案为 Phase 新增 `Isolation` 字段，并让 `CommandExecutor.Dir` 具备隔离感知能力，同时明确要求隔离关闭时输出必须与现状字节级一致（纯增量、不破坏现状）。虽然与『分层隔离路线图』主题共享 git worktree 这一机制，但问题动机与验收标准截然不同，且已完成 architected 阶段的具体设计，故单列为独立主题。

*优先级信号*: P1(x1)　·　*最高成熟度*: architected　·　*示例来源*: architectural-expansion-perspectives

**WASM 便携 Gate (已评估并暂缓)** `×1`

曾评估把治理闸门(gate)编译为 WASM，以便跨平台/跨架构便携执行，无需为每种目标环境做原生构建。经评估后认为这是边缘场景收益，当前 polyglot adapter 架构已经足够覆盖跨语言 gate 执行的需求，因此该方向被明确判定为暂缓（非拒绝，而是优先级搁置），与本分类其余聚焦『隔离 agent 执行』的主题在问题域上完全不同。

*优先级信号*: deferred(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-expansion-directions

**控制面 Daemon + 跨进程资源配额执行** `×1`

复核确认代码库中没有任何控制面 daemon 的证据（无 unix socket/gRPC/API server）；并行 evolve 目前仅靠 O_APPEND 处理并发写入，且指出一个具体缺陷——evolve.go:347 的日志 rotate 逻辑没有文件锁(flock)，两个并发进程的 rotate 会相互干扰。当前 MaxAgentCalls 守卫是 per-process 级别而非跨进程 daemon 级配额，FORGE_AGENT_DEPTH 虽能跨进程传递但只防递归、不做配额限制。提议的方向四 Phase A（daemon + UNIX socket 提供跨进程资源配额）被明确定性为真正的新工程、高风险、v3 阶段（1-3月），而非小补丁，与本分类其余聚焦『单进程/子进程隔离机制』的主题在架构层级上不同（更偏向资源记账/控制面而非执行沙箱本身）。

*优先级信号*: low/v3(x1)　·　*最高成熟度*: ideation-proposal　·　*示例来源*: strategic-expansion-perspectives

---
