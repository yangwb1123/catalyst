现在我已经全面掌握了项目状态。以下是我的架构分析。

---

# ForgeOS 架构分析报告

> **角色**: 资深架构师  
> **范围**: forge-core (18 Go 包,~35k LOC), harness (39+ 模块,~10.5k LOC), `.agent/` 全骨架, Sprint 1–31 完整演进  
> **日期**: 2026-07-12  
> **前置阅读**: BOOTSTRAP.md → .agent/{PROJECT,ARCHITECTURE,ROADMAP,CURRENT_SPRINT,AGENTS} → 代码

---

## 一、架构评估

### 1.1 核心架构风格

ForgeOS 的架构可以用一句话概括：**声明式治理脊柱 + 插件式执行引擎 + 文件系统作为持久层**。

```
┌─────────────────────────────────────────────────────┐
│                    CLI (cmd/forge)                    │
├──────────┬──────────┬──────────┬────────────────────┤
│  Run     │  Evolve  │  Route   │  Validate / Doctor │
├──────────┴──────────┴──────────┴────────────────────┤
│                 Orchestrator Layer                    │
│  (internal/orchestrator: RunFrom / LoopEngine /      │
│   runPhase / runParallel / waves / kahnSort)         │
├──────────┬──────────┬──────────┬────────────────────┤
│ Converge │  Memory  │  Prompt  │   Trace / Persist  │
│ Routing  │  Budget  │  Risk    │   Mode / Migrate   │
│ Gate     │  Asset   │  Doctor  │   Attribution      │
├──────────┴──────────┴──────────┴────────────────────┤
│                Harness 执法层                         │
│  (gate.mjs / check.py / acceptance.mjs / arch-check │
│   / secret-scan / sca.mjs / scorecard*.mjs)          │
├──────────┬──────────┬──────────┬────────────────────┤
│ .agent/  │ workflow │ agent    │ skill / policy     │
│ 声明层   │  YAML    │ 卡 .md   │ YAML / routing     │
└──────────┴──────────┴──────────┴────────────────────┘
```

### 1.2 优势

**① 零外部依赖的纯 Go 运行时**  
`go.mod` 无 `require` 块,18 个 Go 包全部纯标准库。这在 Go 生态中极为罕见,意味着:构建时间 <1s,二进制体积 ~10MB,无供应链攻击面,无许可证管理,无 `go mod tidy` 漂移。这是**有意识的设计决策**而不是偶然。

**② Harness 层的诚实框架模式**  
`probeLint`/`probeCoverage`/`sca.mjs`/`scorecard*.mjs` 遵循统一的「检测→检测→存在则执行、缺失则 N/A」模式。这解决了「工具依赖清单永远不完整」的经典治理问题——框架是通用的,具体工具缺了不会误报通过。

**③ 中枢旋钮 (mode × lifecycle) 的三处同驱**  
一个设置驱动 Router 档位 + Harness 严格度 + Workflow 深度,这种正交设计让配置面只有 4×4=16 种组合而不是 N 个独立开关。`production` lifecycle 的一票否决 override 提供了安全下限。

**④ 诚实标注文化制度化**  
从 `CURRENT_SPRINT.md` 到代码注释,「诚实边界」是建制化的工程实践。这不仅避免了镀金,更让架构决策的可追溯性达到了罕见水平——每一个「不做」的决定都有白纸黑字的理由和后续触发条件。

**⑤ 文件系统作为状态层的务实选择**  
`checkpoint.json` / `trace.jsonl` / `memory.jsonl` / `.forge/<stage>.approved` 构成完整的运行时持久化方案。没有数据库,没有消息队列,没有外部缓存——对于当前阶段,这是正确的选择,避免了过早引入分布式复杂性。

### 1.3 局限性

**① 单进程、单目录的隐含假设**  
所有操作建立在「cwd 就是一个独立项目」的隐含前提上。`persist` 包从固定路径 `root/.forge/` 读写,`memory.loadCaches` 以路径为键且注释自承有碰撞风险,`routing.ModelMap` 是全局变量。当组织有 10+ 微服务仓库时,当前架构无法直接复用。

**② `memory` 包是建好的高速公路但没有车**  
`memory.Entry` 的 `Supersedes`/`Confidence`/`Source` 字段齐全,`memory.Append`/`Load`/`Query`/`Compact` 完整实现——但消费端 (`prompt_context.go` 的 `memoryContext` lane) 从未在真实 workflow phase 中被装配。这是典型的基础设施先行但业务逻辑未到位的架构债务。

**③ 死字段积累**  
Sprint 30 审计出 4 处「声明但被另一套机制取代」的死字段。虽然 Sprint 31 诚实处理了大部分(加注释、删误导性声明),但这是系统复杂度的信号——当声明层和实现层有两条演进路径,漂移是必然的。当前的 drift-guard (`check_workflow_mode_gating`) 是事后发现再打补丁的模式。

**④ 单厂商模型路由硬编码**  
`routing.ModelMap` 硬编码 anthropic 三个 tier,没有任何 provider 抽象层。north-star 的「跨厂商池 LiteLLM」是 v3 目标,但中间没有增量路径——这意味着从单厂商到多厂商的迁移不是配置变更,而是一次架构手术。

**⑤ 并行执行缺乏资源治理**  
`waves.go` 的拓扑排序可以产生包含 100 个 phase 的 wave,`runWave` 会同时启动 100 个 LLM 进程。没有并发上限,没有公平 budget 分配,没有 jitter 的 deterministic backoff。Sprint 22 的四维护栏(深度/数量/时间/内存)覆盖的是**串行**场景,并行模式的风险暴露完全不同。

### 1.4 关键设计决策评估

| 决策 | 状态 | 评价 |
|------|------|------|
| 纯标准库零依赖 | 已执行 | ✅ **正确**。当前阶段最大的架构优势之一 |
| Harness 与 forge-core 分离 | 已执行 | ✅ **正确**。host-independent 执法 + 语言异构灵活性 |
| 文件系统作为状态层 | 已执行 | ✅ **正确**。避免了过早引入 DB/消息队列 |
| mode × lifecycle 中枢旋钮 | 已执行 | ✅ **正确**。正交设计,16 种组合覆盖全部场景 |
| YAML 转 JSON 的 Python shim | 已执行 | ⚠️ **合理的临时方案**。但 Go 生态已有 `gopkg.in/yaml.v3`——如果零依赖约束在未来松动,这是第一优先替换项 |
| `readonly` 的技术强制路径 | 已实现 + 用户终止验证 | ⚠️ **设计正确但验证不完整**。按官方文档契约构造 + 单测坐实,但未过真实 `claude` 进程验证——这是一道用户知情接受的剩余风险 |
| memory 包先于消费端建设 | 已执行 | ⚠️ **基础设施优先,合理但需警惕**。如果再过 3 个 sprint 消费端仍未装配,就应判定为镀金 |
| Web UI 排除在架构外 | 已决策 | ✅ **正确**。CLI 优先 + TUI 增强,Web UI 交给 community/north-star |

### 1.5 技术债务(按严重度排序)

| 债务 | 位置 | 影响 | 建议 |
|------|------|------|------|
| `memory` 消费端断层 | `prompt_context.go` `memoryContext` lane 零装配 | 整个 memory 子系统投入产出比极低 | **P0**: 在当前或下个 sprint 装配 |
| Python YAML shim 的维护成本 | `harness/yaml2json.py` | Go 改 workflow YAML 需 Python 环境,增加部署依赖和故障面 | **P1**: 评估引入 `gopkg.in/yaml.v3` 或自建最小 YAML tokenizer |
| 死字段的 drift-guard 是补丁模式 | `harness/check.py` `check_workflow_mode_gating` | 每次新增字段都需要加新的 check,不可扩展 | **P1**: 设计声明-实现自动一致性校验框架 |
| `cmd/forge` 包文件数濒临上限 | `cmd/forge/` | 16 文件/17 上限,每次新 CLI 命令都需要架构裁决 | **P2**: 建立 CLI 子命令注册表模式,消除硬性文件数预算 |
| `backoff.go` 无 jitter | `internal/orchestrator/backoff.go` | 并行模式下产生 thundering herd | **P2**: 加随机 jitter,不影响串行路径 |

---

## 二、扩展方向(5 个)

以下分析基于 V49 文档的 5 个方向,结合代码实际状态进行评估、修正和优先级重排。

### 方向 A: Agent 卡行为契约的运行时履约验证

**优先级**: P0 (从文档的 P1 上调)  
**预估**: ~2 sprints  
**杠杆**: ⭐⭐⭐⭐⭐

#### 为什么需要

这是**当前系统最大的「声明-实现」断层**。12 个 agent 卡声明了 `readonly`/`requires_tools`/`emits`/`VERDICT` 契约——但这些声明在运行时零消费:

- `readCard` 返回整个 markdown 文本注入 prompt,不解析其中的行为声明
- `requires_tools` 只在 prompt 中叙述性标注,不做 `command -v` 预检
- `emits` 路径在 phase 完成后不审计实际写入位置
- `VERDICT` token 被提取但内容真实性不交叉验证

这个问题不是「缺少功能」,而是**架构层面的契约破损**——声明层和运行时之间缺少一个验证层。Sprint 31 的 `readonly` 技术强制路径按文档契约构造了 argv,但这是点对点的修补,不是系统化的验证框架。

#### 核心挑战

1. **Agent 卡元数据的可机读解析**:当前 agent 卡是 markdown 散文,frontmatter 不是标准化的 YAML/TOML。需要定义一个轻量的元数据格式,既能人读又能机读,且不破坏现有 agent 卡的可读性
2. **验证点定位**:在每个 agent phase 的执行生命周期中,哪些点插入验证——起跑前(预检)、运行中(监控)、完成后(审计)?
3. **enforce vs warn 的策略选择**:`readonly` 声明与 workflow phase 不一致时是 block(阻止 phase 执行)还是 warn?

#### 建议的架构变更

```
Agent Card (.md) ──→ AgentCardParser (新: internal/agentcard/)
                         │
                         ├── extract readonly/requires_tools/emits
                         ├── extract VERDICT schema
                         └── extract writes_adr paths
                              │
                              ▼
                   ContractValidator (新: internal/contract/)
                         │
                         ├── PreFlightCheck (tool existence)
                         ├── PhaseStartCheck (readonly alignment)
                         └── PostFlightAudit (emits path, verdict consistency)
```

#### 对现有系统的影响

- **向后兼容**:旧版 agent 卡没有元数据字段 → 跳过验证,不 FAIL,不改变现有行为
- **增量采纳**:从 `check.py` 新增一个 check 开始,然后逐步加到 `orchestrator` 的 phase 执行管线中
- **与 Sprint 31 的关系**:复用已有的 `claudeArgv` readonly 构造逻辑,不要重做

---

### 方向 B: Prompt 有效性测量与优化闭环

**优先级**: P1  
**预估**: ~3 sprints  
**杠杆**: ⭐⭐⭐⭐⭐

#### 为什么需要

这是文档和我的独立分析都认同的最高杠杆方向。核心洞察:scorecard 有 `QualityScore`/`PassRate`/`ReworkRate`/`AvgIterations`/`Samples`,但这些数据**零消费于 prompt 优化**。

实测证据:Sprint 24-26 的真 claude 端到端测试暴露了 8 个真实 gap——任务注入、写权限、模型路由、工作目录、成本封顶、trace latency、cost telemetry、reviewer 缺 gate 信号——这些全部是「prompt 装配」层面的缺口,不是「agent 能力」层面的。换句话说,agent 的 LLM 能力已经够用,但 prompt 装配管线是薄弱环节。

#### 核心挑战

1. **Scorecard 数据归因**:当前 scorecard 按 `(model, task_type)` 聚合,但 prompt 的差异源不止 model 和 task_type——还有 agent 卡版本、template 版本、memory 上下文选择。需要在 scorecard 中引入 `prompt_digest` 字段
2. **实验框架的成本**:A/B 测试意味着 50% 的 phase 可能使用更差的 prompt,在 24h evolve 中这可能显著增加总成本。实验框架必须默认关闭,显式启用
3. **退化的统计检测**:当 `QualityScore` 下降时,是 prompt 变更导致的还是 LLM 自身漂移导致的?需要足够的样本量才能做统计显著性判断,冷启动期间无法告警

#### 建议的架构变更

```
Prompt 版本化:
  agent_card.md ──→ frontmatter version: v1/v2/latest
  template.md   ──→ frontmatter version + SHA-256 digest
       │
       ▼
  Scorecard 增强:
  ScorecardPair {
    ...原有字段...
    prompt_digest  string  // 所用 prompt 装配的哈希
    template_ids   []string // 所用各模板版本
    experiment_tag string   // A/B 实验标识,空=生产
  }
       │
       ▼
  PromptOptimizer (新: internal/promptopt/)
  ├── DetectDegradation: 统计显著性下降检测
  ├── SuggestRollback: 建议回退到已知更优版本
  └── ReportExperiment: A/B 对比报告
```

#### 对现有系统的影响

- **Scorecard schema 扩展**:新增 `prompt_digest` 等字段,向后兼容(旧记录 digest=空)
- **`prompt.Build` 签名变更**:增加 `PromptVariant` 参数,默认 `"latest"`,不影响现有调用
- **CLI 标志**:`forge run --prompt-experiment <variant>` 显式启用

---

### 方向 C: 工作流编排反模式静态检测

**优先级**: P1 (从文档的 P2 上调)  
**预估**: ~1 sprint  
**杠杆**: ⭐⭐⭐⭐

#### 为什么需要

**零覆盖的方向**——89+ 份已有分析文档没有任何一份将其作为独立方向。但这是一个「一天就能交付 MVP 且立刻产生价值」的方向。

当前存在已证明的故障模式:
- `stop_condition` 引用永不赋值的 metric → 永不收敛(Sprint 28 修 `review_status` 时暴露了同款缺口——`ReviewStatus` 从未被赋值,所有 `forge run review` 永不 MET)
- `waves.go` 的 `kahnSort` 无环检测 → 依赖环导致死锁或空排序
- `required_gates` 引用不存在的 gate 名称 → 静默跳过或报错
- 孤 phase 定义 → 浪费 token

#### 核心挑战

1. **Stop condition 可达性分析的严谨性**:某些 metric 可能在 workflow 外被赋值(如 `FileDelta` 从 git diff 计算),可达性分析需要理解这些「隐式赋值」
2. **mode × lifecycle 的组合复杂度**:4×4=16 种组合,一个 phase 可能在部分组合中执行、部分组合中跳过。反模式检测需要考虑全部组合而非一刀切
3. **误报风险**:workflow 作者可能有意引用一个「未来才会实现的 metric」(预留扩展点)。检测应默认 warn 而非 block

#### 建议的架构变更

```
WorkflowAntiPattern (新: check.py 扩展 或 新 harness 工具)
├── StopConditionReachability
│   ├── 对 stop_condition.all_of 中的每个 metric
│   ├── 追查 workflow 中是否有 phase 能为其赋值
│   └── 输出: unreachable metrics list
├── DependencyCycleDetection
│   ├── Tarjan SCC 算法(在 kahnSort 前运行)
│   └── 输出: cycle path (A→B→C→A)
├── GateRefExistence
│   ├── 对 required_gates 中每个 gate 名
│   ├── 在 modes.yml gate_catalog 中查找
│   └── 输出: missing gate refs
├── DeadPhaseDetection
│   ├── 检查每个 phase 是否有消费路径
│   └── 输出: phases with no consumer
└── NeverExecutedPhaseDetection
    ├── 遍历 mode×lifecycle 组合
    └── 输出: phases never executed
```

#### 对现有系统的影响

- **纯 harness 层工具**:不修改 forge-core Go 代码,只在 `check.py` 或新 `workflow_lint.py` 中实现
- **分阶段交付**:StopConditionReachability + GateRefExistence 可在 1 天内实现(都是纯 YAML 解析 + 字符串匹配);DependencyCycleDetection 需要新算法;DeadPhaseDetection 需要更复杂的 control-flow 分析
- **与 `forge validate` 的关系**:建议整合到 `forge validate` 子命令下,作为 `--lint` 模式

---

### 方向 D: 跨会话知识生命周期管理

**优先级**: P1  
**预估**: ~2 sprints  
**杠杆**: ⭐⭐⭐⭐

#### 为什么需要

`memory` 包的消费者缺口(方向 B 的前置依赖)让这个方向的部分工作必须先行。核心问题:

1. **`Supersedes` 字段零消费**:全仓搜索确认,没有任何文件读取这个字段来做知识淘汰/降权
2. **`memory.jsonl` 无界增长**:`Append` 使用 `O_APPEND`,文件无限增长。`Compact` 存在但按数量修剪,不按语义重要性
3. **`prompt_memory.go` 按 recency 排序而非按质量**:`sort.Slice` 按时间排序,`Confidence` 字段存在但不参与排序
4. **无冲突检测**:两个 evolve 会话可能产生矛盾的知识,但没有任何机制发现或处理

#### 核心挑战

1. **知识重要性的自动判定**:`Confidence` 由 agent 自报,置信度可能虚高。如何在没有 LLM 重判的情况下做重要性排序?
2. **TTL 与业务逻辑的冲突**:根本性架构决策应该永久保留(`ttl_days: 0`),临时调试记录应在几天后自动过期——但这需要 Entry 的 `kind` 分类足够可靠
3. **冲突检测的启发式精度**:文本相似度可能误判互补描述为矛盾(「用 Postgres 做主库」vs「用 SQLite 做本地缓存」不是矛盾,是集群设计的两个部分)

#### 建议的架构变更

```
┌─────────────────────────────────────────────────────┐
│                   memory 增强                         │
├─────────────────────────────────────────────────────┤
│ Entry {                                              │
│   ...原有字段...                                      │
│   TTLDays     int     // 0=永久                      │
│   CreatedAt   int64   // unix timestamp             │
│   RefCount    int     // 被其他条目引用次数           │
│   Contradicts string  // 矛盾条目的 ID               │
│ }                                                    │
├─────────────────────────────────────────────────────┤
│ Consumer 装配:                                        │
│ prompt_memory.go:                                    │
│   1. 过滤过期(TTLDays ≤ now - CreatedAt)             │
│   2. 降权/排除被 supersedes 的条目                    │
│   3. 按 Confidence × Recency 排序而非纯 Recency       │
│   4. 冲突条目成对注入,标注矛盾                         │
├─────────────────────────────────────────────────────┤
│ Compact 增强:                                         │
│   1. 先标记保留: Confidence ≥ 0.8 或 RefCount ≥ 3    │
│   2. 再按数量修剪剩余                                 │
│   3. Supersedes 关系追踪:被 supersedes 的优先修剪     │
└─────────────────────────────────────────────────────┘
```

#### 对现有系统的影响

- **文件格式向后兼容**:新增字段用 `omitempty`,旧文件读入时取零值,TTL=0 → 永不过期,行为不变
- **prompt 注入的行为变更**:引入排序和过滤后,prompt 中看到的 memory 条目集可能与之前不同——这应当是改进而非退化,但需要 A/B 验证(方向 B 的实验框架)
- **与方向 B 的关系**:方向 D 是方向 B 的前置依赖——没有知识生命周期管理,prompt 优化闭环会面临「memory 上下文膨胀→token 成本上升→质量下降」的负面飞轮

---

### 方向 E: 非代码产物的结构化验证框架

**优先级**: P2  
**预估**: ~2 sprints  
**杠杆**: ⭐⭐⭐⭐

#### 为什么需要

当前所有 harness gate 只验证代码产物。但系统产出大量非代码产物——PRD、架构文档、评审报告、ADR——这些没有任何自动结构验证。在无人值守的 evolve 模式下,agent 可能生成结构不完整但看起来合理的 PRD/ADR,浪费下游 phase 的时间。

#### 核心挑战

1. **验证 DSL 的设计**:需要定义一种简洁的 DSL 来描述「PRD 应包含 ## Problem Statement 和 ## Success Metrics」但不把验证逻辑写死。DSL 过于复杂会阻碍采纳,过于简单又无法表达结构约束
2. **markdown 变体的容错**:agent 可能输出 `## Problem statement`(小写 s)而非 `## Problem Statement`,验证器需要大小写不敏感/模糊匹配
3. **渐进式采纳路径**:现有的 workflow 都没有 schema 声明,不能要求所有 workflow 立即提供 schema。框架必须「无 schema 时就跳过」

#### 建议的架构变更

```
.agent/schemas/              // 新目录
├── prd.yaml                 // PRD 结构契约
├── arch-design.yaml         // 架构设计结构契约
├── review-report.yaml       // 评审报告结构契约
└── adr.yaml                 // ADR 结构契约

DocumentValidator (新: harness/document_check.mjs)
├── 按文件扩展名选择验证策略:
│   ├── .md  → section presence + keyword check
│   ├── .json → JSON Schema validation
│   └── .yaml → YAML schema validation
├── 结果接入 gatherSignals 的信号池
└── 可选 warn/block 模式
```

#### 对现有系统的影响

- **纯 harness 层**:不修改 forge-core
- **建议在方向 A 之后实施**:方向 A 的 agent 卡结构化解析可以为非代码产物 schema 提供「声明源」,减少重复劳动
- **非 load-bearing 的渐进采纳**:mode 为 explorer 时默认 warn,engineering 时默认 block

---

## 三、接口设计建议

### 3.1 关键抽象层设计原则

**原则一:声明与实现之间加验证层**

当前架构缺少一个层来回答「agent 卡声明的与运行时实际执行的一致吗」。建议引入:

```
声明层 (.agent/agents/*.md) 
    ↓ parsed by
契约解析层 (internal/agentcard/ 或 harness 层)
    ↓ produces
机读契约描述 (AgentCardContract struct)
    ↓ verified against
运行时行为 (orchestrator.Phase / CommandExecutor / gate)
    ↓ produces
履约报告 (ContractComplianceReport)
```

**原则二:所有新加的验证点都使用 warn→block 的两阶段模式**

- warn:检测到违规时记录 + 告警,不阻断执行
- block:检测到违规时阻止 phase/run 继续

两阶段设计让 operator 可以逐步收紧,降低采纳摩擦。

**原则三:不要为「已存在但零消费」的字段发明新消费者,先确认该字段是否已被更好的机制取代**

这是 Sprint 30-31 的教训:4 处「声明但未接线」的字段中,只有 2 处值得接线,1 处是误导性声明应该删除,1 处是镀金。在加新接口前,先审计现有「已声明」接口是否合理。

### 3.2 是否需要新的抽象层

**需要: `internal/agentcard/` 包**

当前 `readCard` 返回裸 markdown 文本,任何结构化信息的提取都在 prompt 注入路径中用临时代码做(如 `parseReviewerVerdict`)。需要一个统一的 agent 卡解析库:

```
AgentCard {
    Name            string
    ReadOnly        bool
    RequiresTools   []string
    Emits           []string      // 产出路径
    WritesADR       bool
    MachineReadableContract []MachineReadableToken  // VERDICT/CONFIDENCE 等
    VerdictSchema   string        // "binary" / "five-choice" / "confidence"
    OutputStructure map[string]string  // 产出文件→结构描述(PRD→sections 等)
}
```

**不需要:独立的事件总线/消息队列**

尽管 north-star 架构有事件驱动愿景,但当前单进程架构加上文件系统作为状态层已经足够。引入消息队列会违反零依赖原则,且没有足够的消费者来证明其合理性。

**可能需要: `internal/routing/provider.go` 接口**

如果跨厂商模型池是未来方向,现在就应该定义 `Provider` 接口——不是实现多个 provider,而是让 `ModelMap` 从硬编码 map 变成可扩展的注册表:

```go
type Provider interface {
    Name() string
    Resolve(tier Tier) (modelName string, err error)
    Health() ProviderHealth
    CostModel(tier Tier) CostModel
}

type ProviderRegistry struct {
    providers map[string]Provider
    failover  FailoverStrategy  // "sequential" / "round-robin" / "latency-based"
}
```

当前阶段不需要多实现(只有 Claude),但接口定义可以立即进行,不影响现有行为。

### 3.3 向后兼容性策略

1. **Scorecard schema 扩展**:新增字段用 `json:",omitempty"` + `json:",omitzero"`(Go 1.24+),旧数据读入时零值不触发新行为
2. **Agent 卡元数据**:用 HTML comment 或 frontmatter 格式(如 `---\nreadonly: true\n---`),没有元数据的旧卡跳过所有结构化验证,行为不变
3. **Workflow 反模式检测**:作为独立 CLI 命令(`forge validate --lint`)而非 `forge run` 的插桩,不影响现有执行路径
4. **prompt.Build 签名的扩展**:用 variadic options 模式而不是加新参数:

```go
// 当前: func Build(agent, phase, mode, tier, card string, ctx []string) string
// 改为: func Build(agent, phase, mode, tier, card string, ctx []string, opts ...BuildOption) string
// BuildOption 可以是 WithPromptDigest / WithExperimentTag / WithMemoryFilter 等
```

---

## 四、技术选型

### 4.1 是否需要引入新技术栈

| 候选技术 | 评估 | 建议 |
|----------|------|------|
| `gopkg.in/yaml.v3` | Go 生态标准 YAML 解析器,~60k 依赖(但已是广泛使用的稳定库) | **P2 替换**。Python shim 是临时方案,当零依赖约束松动时第一个替换。但优先级不高——当前方案工作正常 |
| 向量数据库(Qdrant/Pinecone) | north-star 中的语义检索层 | **不引入**。当前 TF-IDF 检索在 ~3000 条 memory 的场景下表现足够,引入向量检索需要 embedding 模型 + 数据库运维,收益不匹配成本 |
| LiteLLM | 跨厂商模型池的统一接口 | **不引入(v1)**。Sprint 31 已明确推迟到 v3。当前阶段定义 `Provider` 接口即可,不需要引入外部依赖 |
| Bubbletea/TUI 框架 | CLI 仪表盘 | **可考虑,但非核心**。Sprint 31 的架构外判定合理——TUI 是 CLI 的增强,不影响核心能力。如果做,纯 Go 标准库 + `tcell` 或最小依赖 |
| Temporal/外部工作流引擎 | north-star 中的 durable orchestration | **不引入(v2)**。当前文件系统 checkpoint + `LoopEngine` 已足够覆盖 24h evolve 场景。Temporal 是 v3 的事 |

### 4.2 第三方依赖评估标准

如果未来决定引入外部依赖,建议采用以下标准:

1. **许可证兼容**:MIT/Apache 2.0/BSD,排除 AGPL/SSPL
2. **零传递依赖优先**:优先选择依赖树浅的库
3. **Go 版本兼容**:要求在 Go 1.22+ 上构建通过
4. **测试覆盖率 ≥80%**:依赖的测试覆盖率反映其可靠性
5. **替换成本**:评估替换为本方案的成本是否可控

### 4.3 自建 vs 采购的决策依据

| 场景 | 决策 | 依据 |
|------|------|------|
| Agent 卡解析 | **自建**(~300 行 Go) | 领域特定,无现有库,零依赖约束 |
| YAML 解析 | **自建或引入 yaml.v3** | 当前 Python shim 是合理临时方案,但 Go 纯标准库方案需要自建 tokenizer——这是值得的投入(学习机会)但优先级不高 |
| 跨厂商模型路由 | **定义 Provider 接口,暂不采购 LiteLLM** | 接口抽象是架构级行为,不依赖外部库;LiteLLM 集成是 v3 的事 |
| 语义验证(Contract/Property/Mutation) | **适配器模式封装现有工具** | 不发明工具,只编配。`probeContract`/`probeProperty` 沿用 `probeLint` 的同款检测-执行-裁决模式 |

---

## 五、实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 代码影响 | 预估 | 前置依赖 |
|--------|------|----------|------|----------|
| **P0** | A: Agent 卡运行时履约验证 | 新 `internal/agentcard/` + `internal/contract/` | 2 sprints | 无 |
| **P0** | C: Workflow 反模式静态检测 | target `check.py` 或新 `workflow_lint.py` | 1 sprint | 无 |
| **P1** | D: 知识生命周期管理 | `internal/memory/` 增强 + `prompt_memory.go` 接线 | 2 sprints | 无 |
| **P1** | B: Prompt 优化闭环 | `internal/prompt/` + `internal/promptopt/` + scorecard 扩展 | 3 sprints | D(需要 memory 管理稳定) |
| **P2** | E: 非代码产物验证 | 新 `harness/document_check.mjs` | 2 sprints | A(agent 卡解析可复用 schema 声明) |

### 5.2 阶段划分

#### 阶段一:治理增强(2 sprints)——P0 方向

**目标**:填补当前系统最明显的声明-实现断层,在已有的 `check.py` 和 `orchestrator` 框架中增量实现。

**Sprint N**:
- Workflow 反模式检测 MVP:StopConditionReachability + GateRefExistence(1-2 天)
- Agent 卡解析器:定义 `AgentCardContract` struct,实现 frontmatter 解析(3-4 天)
- `check.py` 新增 check:agent 卡声明 vs workflow phase 声明一致性

**Sprint N+1**:
- PreFlightCheck: `requires_tools` 的 `command -v` 预检
- PostFlightAudit: `emits` 路径的 git diff 审计
- DependencyCycleDetection: Tarjan 算法接入 `waves.go` 前

**里程碑**:`forge validate` 现在不仅能检查资产引用存在,还能检查编排语义正确性和 agent 卡声明一致性。

#### 阶段二:知识工程(2-3 sprints)——P1 方向 D → B

**目标**:完成 memory 消费端的装配,建立知识生命周期管理,为 prompt 优化闭环打好基础。

**Sprint N+2**:
- `memory.Entry` TTL/RefCount/Contradicts 字段扩展
- `prompt_memory.go` 按置信度+recency 排序 + 过滤过期条目
- `memory.Compact` 两阶段策略(语义保留 + 数量修剪)

**Sprint N+3**:
- Supersedes 字段消费:被取代条目在 prompt 注入中降权/排除
- 知识冲突检测:写入时文本相似度比较,矛盾条目成对注入 prompt
- 冷热分层:按引用频率动态调整

**Sprint N+4**:
- Prompt 版本标识:agent 卡和 template 的 frontmatter version
- Scorecard 扩展:prompt_digest 标签和实验标识
- Prompt 退化告警:统计显著性下降检测

**里程碑**:scorecard 数据首次回灌到 prompt 装配——学习闭环的物理闭环。

#### 阶段三:产物质量(2 sprints)——P2 方向 E

**目标**:将 harness gate 从代码产物扩展为非代码产物,建立文档结构契约框架。

**Sprint N+5**:
- Document schema DSL 设计 + `document_check.mjs` 实现
- PRD/ADR/评审报告的初始 schema 定义
- warn 模式接入 gatherSignals

**Sprint N+6**:
- agent 卡散文描述与 schema 的一致性漂移检测
- block 模式(production lifecycle 下强制)
- forge-init 纳入 schema 目录

**里程碑**:非代码产物有了代码级的结构验证,无人值守 evolve 模式下 PRD/ADR 质量可自动保证。

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Agent 卡 frontmatter 格式导致现有卡损坏 | 低 | 高 | 解析器必须是 fail-safe:frontmatter 解析失败 → 回退到全文注入,不破坏现有行为 |
| Memory 消费端装配后 prompt token 数激增 | 中 | 中 | 加入 memoryCap + token budget 守卫;记忆条目按重要性排序,超限截断 |
| Prompt 实验框架被误用于生产环境 | 低 | 高 | 默认关闭,需 `--prompt-experiment` 显式标志;实验模式下在 trace.jsonl 中记录实验标记 |
| Workflow 反模式检测误报导致 CI 阻塞 | 中 | 中 | 默认 warn 模式;误报可被 `// forge-ignore:` 注释豁免;production lifecycle 才 block |
| 知识冲突检测的假阳性让 agent 困惑 | 中 | 中 | 冲突只产生 warn 不 block;prompt 中标注「以下两条记忆可能存在矛盾」而非静默丢弃任一条 |
| 跨会话知识导入导致有毒知识传播 | 低 | 高 | 显式 import(`forge memory import --from <dir>`),不自动继承;导入时可指定 `max-confidence` 过滤器 |

---

## 总结

ForgeOS 的架构在 v0-v1 阶段做出了大量正确决策——零依赖、声明式治理、中枢旋钮、诚实框架——这些是项目当前质量水平的根本原因。但 31 个 sprint 的密集建设也暴露了架构的自然演化趋势:**基础设施快了,消费端慢了**。

`memory` 包是基础设施先行但消费端缺位的典型案例。`scorecard` 数据是「有数据但零回灌」的典型案例。agent 卡声明是「人读接口丰富但机读接口零消费」的典型案例。

建议的 5 个扩展方向中,**方向 A(Agent 卡验证)和方向 C(Workflow 反模式检测)是低风险高回报的「补现有断层」**,可以在保持零依赖的前提下快速交付。**方向 D(知识生命周期)和方向 B(Prompt 优化闭环)是建设学习闭环的核心**,是 ForgeOS 从「自动化工具」升级为「自治平台」的关键路径。**方向 E(非代码产物验证)是治理完备性的最后一道拼图**,建议在方向 A 的就绪后实施。

核心建议:**不要同时启动全部 5 个方向**。按阶段一(P0)→阶段二(P1)→阶段三(P2)的顺序推进,每个阶段交付后经过 `forge accept` 全绿 + fresh-review 再进入下一阶段。
