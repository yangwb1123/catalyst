# ForgeOS 生产级扩展方向（资深架构师/产品经理视角）

> **生成日期**: 2026-07-10
> **方法**: 全局扫描 forge-core（18 Go 包 ~33k LOC，含测试）、harness（~10.5k LOC）、.agent（5 workflow / 12 agent 卡 / 9 skill 卡）、全部 ADR 与决策记录，以及 30+ 轮已有扩展分析文档。
> **目标**: 找出已有文档未充分覆盖、但对生产级落地至关重要的 3-5 个扩展方向。
> **原则**: 不重复已有文档已深入分析的方向（韧性运行时/诚实反馈闭环/WASM 工具链/跨厂商路由/治理完整性补完），着重于在那些分析之后显现的更深层架构盲区。

---

## 方向总览

| 优先级 | 方向 | 简述 | 影响面 | 当前成熟度 | 与已有分析的关系 |
|--------|------|------|--------|-----------|-----------------|
| 🔴 P0 | **多 Agent 冲突检测与操作排序** | 当多个 agent 修改同一文件时，保证原子性与可恢复性 | forge-core 编排层 | 无任何实现 | docs/ROADMAP 方向 1（韧性运行时）提及并发风险但未深入 agent 级冲突 |
| 🔴 P0 | **实时可观测性与 Agent 调试接口** | 让人类能实时观察、理解和调试 agent 执行过程 | 全栈（CLI + 数据 + 可能的 Web） | 仅有 trace.jsonl + scorecard 事后数据 | docs/ROADMAP 方向 2（scorecard 实时写入）是子集；所需的数据平面远不止延迟写入 |
| 🟠 P1 | **Agent 输出保真度保障（验证工厂）** | 在 gate 之前独立验证 agent 输出是否满足约束，降低幻觉和自报告偏差 | forge-core 执行器 + converge | 无任何独立验证层 | FUNCTIONAL_REQUIREMENTS_AUDIT 列出了 RoadmapCompletion 自报告风险，但未提议系统性解决方案 |
| 🟠 P1 | **跨项目模式记忆与组织学习** | 让洞察在项目之间复用，形成组织级工程智能 | memory 引擎 + 知识管理层 | memory 仅 per-project | docs/ROADMAP 方向 2（Memory 置信度）是 per-project 改进；跨项目复用是质变 |
| 🟡 P2 | **生产预算治理与成本智能** | 多项目/多团队视野下的预算分配、成本归因、异常检测与优化建议 | routing + 预算守卫 | 仅有 per-run `--max-budget-usd` | docs/ROADMAP 方向 4（跨厂商路由）提及成本优化，但治理层面（组织级预算政策）未触及 |

---

## 方向 1（P0）: 多 Agent 冲突检测与操作排序

### 现状

当前 forge-core 的编排层（`internal/orchestrator`）按 workflow 定义的顺序串行执行 phase，或通过 `RunParallel` 并行执行无依赖的 phase。但：

- **无文件级锁定** — 两个 agent phase（即使串行）修改同一文件时，后一个可能静默覆盖前一个的修改
- **无冲突检测** — reviewer 建议的修改与 implementer 的实现可能在同一文件上产生冲突，系统无察觉
- **无原子性保障** — 一个 multi-phase workflow 在中途失败（phase 3/5）时，前 2 个 phase 的修改已落地，无回滚机制
- **`parallel.go` 的 wave 排序只考虑了 `depends_on`（声明式任务依赖），未考虑**资源冲突（同一文件被多个 agent 写入）
- **`cmd/forge/prompt_context.go` 的 `phaseOutputLedger`** 只做前传（feeds_forward），不做后验冲突校验

### 为什么需要

**这是生产级 multi-agent 编排的核心缺失**。当前设计假设 agent 阶段之间是「写时无冲突」的——这在真实项目中不成立：

1. **实例**：implementer 修改 `src/api/handlers.go` 添加新端点，reviewer 建议重命名同一文件的既有函数名。两个 agent 在非重叠阶段操作同一文件，无协调机制。
2. **实例**：`forge evolve` 的多迭代中，第一次迭代重命名了核心类型，第二次迭代基于旧名称生成代码——产生无法编译的结果，但 forge-core 无感知。
3. **实例**：并行 phase 中，两个 implementer 分别修改 `service.go` 的不同部分，git merge 冲突在 forge 层完全不可见，只会在下游 `go build` 时暴露（且错误消息是编译器级别的，不是 agent 操作级别的）。

### 现有防线及为何不足

| 防线 | 限制 |
|------|------|
| 串行 phase 执行（默认 `Run`） | 不防跨 phase 冲突 |
| `RunParallel` 的 wave 排序 | 只看 `depends_on`，不看文件依赖 |
| `go build` gate | 太迟（已浪费 agent 调用 + 预算），错误消息晦涩 |
| git diff | 事后对比，不预防 |

### 建议的扩展方向

构建**文件级操作排序层**，位于 orchestrator 之上但独立于具体执行器：

```
┌─────────────────────────────────────────────────┐
│               Orchestrator (现状)                │
│  Run / RunParallel / LoopEngine                 │
└──────────────────┬──────────────────────────────┘
                   ▼
┌─────────────────────────────────────────────────┐
│          ★ 操作排序层（新增）★                    │
│  - phase 声明式「写集」/「读集」声明              │
│  - 文件级独占写锁定（串行化冲突 phase）           │
│  - 后验冲突检测（写后快照比对）                   │
│  - 半自动合并（agent 辅助的三路合并）             │
└─────────────────────────────────────────────────┘
```

**关键设计决策**：

- **不需要完整形式化验证**。ForgeOS 不是分布式数据库，不需要严格的 2PL/OCC。这里的目标是「在 agent 烧掉 $10 API 费之前尽早检测冲突并调度重试」。
- **写集/读集不需要精确 AST 级分析**。粗粒度文件级声明即可覆盖主要冲突模式（两个 agent 写同一文件 = 冲突）。精确到函数级是未来优化。
- **三路合并可委托给 LLM**。检测到冲突后，让最便宜的可用模型（Haiku）做初版合并提案，维持 Opus 做批准。

### 估算

- **核心数据结构**: 1 sprint（`asset.Phase` 加 `ReadFiles`/`WriteFiles` + 干涉图构建）
- **串行化调度器**: 0.5 sprint（基于干涉图的 phase 重排序）
- **后验冲突检测 + 自动合并**: 1 sprint（写后快照 + diff + LLM 合并）
- **合计**: ~3 sprints

### 风险

- 声明式文件集可能过期（agent 实际修改了未声明的文件）。缓解：后验检测捕获漏报。
- 过度串行化会抵消并行收益。缓解：只在写集重叠的 phase 间串行，无冲突的全并行。

---

## 方向 2（P0）: 实时可观测性与 Agent 调试接口

### 现状

当前的可观测性基础设施包括：

- **`internal/trace/trace.go`** — 结构化 event log（`trace.jsonl`），记录 phase 起止、gate 结果、agent 成本/延迟
- **`cmd/forge/scorecard_wind.go`** — scorecard 聚合（per-iteration 写入，Sprint 26 改进）
- **`cmd/forge/cost.go`** — 成本追踪（`cost_usd_micros` per-phase）

但这些全是**事后**（post-mortem）视角。**运行时**可观测性几乎为零：

- 无 `forge logs --follow` / `forge tail` 等价命令
- 无 agent 输出流式转发到 terminal（当前 `CommandExecutor` 通过 `cappedBuffer` 累积输出，截断后只保留最近 ~10MB）
- 无 phase 级进度报告（用户不知道当前跑的是 planner 还是 implementer，已经跑了多久，预期还有多久）
- 无 gate 结果预览（agent 还在运行时无法预判 gate 是否会绿）
- 调试一个失败的 phase 需要：找到 trace timestamp → 打开 agent 的输出 JSON → 人工比对。全手动。

### 为什么需要

**这是从「开发工具」到「生产系统」的门槛之一**。当前 forge-core 像一台没有仪表的引擎——你能听到它轰隆运转，但不知道是在满功率输出还是已过热。

1. **调试成本极高** — 一个失败的 `forge evolve` 运行（~10-15 分钟，真实 agent 多迭代）如果出问题，你只能等它结束，然后翻 trace.jsonl。如果问题在第 3 次迭代就出现，你浪费了后面 7 次迭代的预算才知道。
2. **用户信任缺失** — 没有进度报告时，用户对 headless agent 运行是「盲信」的。看到一个空白 terminal 运行 5 分钟而没有任何可理解的输出，不会产生信任。
3. **预算监控盲区** — 当前 cost 信息只在 scorecard 中事后可见。运行中无法回答「这个 phase 已经花了多少钱？我会超预算吗？」

### 现有防线及为何不足

| 防线 | 限制 |
|------|------|
| `trace.jsonl` | 事后，需手动 `cat`，无实时流 |
| `cappedBuffer` stdout/stderr | 截断 >=10MB，无流式推送 |
| `Engine.OnPhase` callback | 只暴露 phase 名和状态，不暴露输出流 |
| `LoopEngine` 的 iteration 日志 | 只有级别迭代计数，无 sub-phase 粒度 |

### 建议的扩展方向

构建**结构化事件总线**（Event Bus），作为可观测性的核心抽象：

```
┌─────────────────────────────────────────────────┐
│               Event Bus (新增)                    │
│  - 结构化事件: PhaseStarted/PhaseOutput/PhaseEnd │
│  - 流式 sink: CLI 终端 / WebSocket / JSON Lines  │
│  - 过滤: 按 phase / agent / gate / level         │
│  - 非阻塞: 事件生产永不阻塞 phase 执行            │
└─────────┬─────────────────────┬──────────────────┘
          │                     │
          ▼                     ▼
┌──────────────────┐  ┌──────────────────────────┐
│  CLI 实时输出     │  │ trace.jsonl（增强为满事件 │
│  forge run --tail │  │ 归档，不再只存俯视图）    │
└──────────────────┘  └──────────────────────────┘
```

**关键设计决策**：

- **不要 Web UI**（那是 v3 的 `forge-web`，偏离 CLI 核心）。可观测性的第一阶段应当是纯 CLI 的：`forge run --verbose` 输出实时 phase 进度条 + 当前 agent 正在做什么（从 agent 的 stdout 提炼关键行）+ 已耗成本 + 预估剩余。
- **事件 schema 复用 trace.go 的事件模型**，只是从「写入 file」改为「多路分发到多个 sink」。trace.go 的 `Event` struct 已经包含 `Phase`/`Agent`/`DurationMs`/`CostUsdMicros` 等字段——这正是事件总线需要的数据模型。
- **`cappedBuffer` 改为 ring buffer + stream tee**：保留截断保护（防 OOM），同时把原始输出流转发到事件总线，让 `--verbose` 能看到实时 agent 输出。

### 估算

- **Event Bus 核心**（sink 接口 + CLI stream 实现）: 1 sprint
- **`--verbose`/`--tail` CLI 标志 + 进度渲染**: 0.5 sprint
- **实时成本预估仪表**（基于已用量 + phase 单价估算）: 0.5 sprint
- **trace.jsonl 升级为全事件归档**: 0.5 sprint
- **合计**: ~2.5 sprints

### 风险

- 事件总线不能成为性能瓶颈。所有事件生产必须是 O(1) 且无锁（至少无 blocking 调用）。可参考 `log/slog` 的 handler 模式，sink 失败不传播。
- 终端渲染在 agent 输出极其频繁时可能变成瓶颈。CLI 输出应降采样（每 N 行聚合显示一次），保留完整事件流到文件。

---

## 方向 3（P1）: Agent 输出保真度保障（「验证工厂」）

### 现状

当前的输出验证全在**门**上（gate）——agent 写完代码后，gate 跑 lint/test/build/security 检查。但这意味着：

- **验证发生在 agent 完成之后** — 如果 agent 写了大量不符合要求的代码，整个 phase 的预算和时间都浪费了
- **收敛判断信任 agent 自报告** — `RoadmapCompletion` 由 agent 自己勾选 ROADMAP 条目计算，系统无独立验证机制（`FileDelta` 是横向交叉验证，但不阻断收敛）
- **没有「合同条款」的自动化履行检查** — agent 卡声明「我会产出 X、Y、Z」，但没人检查它真的产出了
- **约束注入不是强制执行的** — `prompt_context.go` 把 ROADMAP/ADR/AGENTS.md 注入给 agent，但 agent 可选择忽略

### 为什么需要

**Agent 自报告偏差是 ForgeOS 收敛循环中最未被正视的风险**（虽然 Sprint 25 已诚实记录了该问题）。具体来说：

1. **实例**：Sprint 25 真点火坐实——implementer 在 `acceptEdits`（无 Bash）模式下无法自查测试是否通过，选择直接勾选 ROADMAP 条目声称完成。这不是恶意的——agent 诚实报告了自己无法验证，但从系统角度看，`RoadmapCompletion` 被注入了超出实际完成度的数值。
2. **实例**：一个 agent「承诺」产出 ADR 但只写了文件名框架。`fore accept` 检查 ADR 内容吗？不——只要文件存在且编译通过，gate 就绿。ADR 的内容质量无验证。
3. **实例**：agent 被要求「单元测试覆盖率 >= 80%」，它写了测试但覆盖率未达到。gate 会抓住这个问题——但这是在 agent 花费了分钟级别的时间和 token 之后。

### 现有防线及为何不足

| 防线 | 限制 |
|------|------|
| `FileDelta` 交叉验证 | 只是告警，不阻断收敛。FA/ML 术语：这是检测指标，不是控制回路 |
| `harness/check.py` 治理检查 | 只检查文件存在和引用合法性，不验证内容质量 |
| agent 卡声明的 `emits:` | 无强制引擎检查产出是否确实生成、内容是否符合规格 |
| `converge.evalOne` 的 unknown metric 拒绝 | 只防 Criterion 配置错误，不防 agent 实际产出与声明的偏差 |

### 建议的扩展方向

构建**输出验证层**（Output Verification Layer），介于 agent phase 执行和 phase 完成之间：

```
Agent Phase ──→ 输出产物 ──→ ★ 验证工厂 ★ ──→ phase 完成
                              │
                    ┌─────────┼─────────┐
                    ▼         ▼         ▼
              文件存在性   内容格式    语义约束
              检查        检查        (LLM 评估)
```

**三个子层**（按确定性降序、成本升序）：

1. **Syntactic Verification（语法层）** — 文件存在？格式正确？可解析？JSON valid？YAML parses？Markdown has required sections？**零 LLM 成本，纯机械检查**。当前缺失：check.py 只检查引用的 agent/skill 存在性，不检查产出的内容结构。
2. **Structural Verification（结构层）** — ROADMAP 条目是否被对应代码改动覆盖？ADR 是否包含架构决策段？discovery PRD 是否包含竞品分析段？**低成本（规则引擎 + AST 匹配）**。当前缺失：没有任何机制检查 ADR 内容是否确实包含决策记录。
3. **Semantic Verification（语义层）** — 用最便宜的可用模型快速评估产出质量。「这份 ADR 的决策理由够具体吗？还是只是重复了已知信息？」「这个 PRD 的置信度自评与内容的质量匹配吗？」**成本可控（委托给 Haiku）**。当前缺失：没有任何机制。

**关键设计决策**：

- 验证工厂**不阻断**phase 执行——它并行于下一个 phase 运行，产生验证报告。结果影响的是**收敛判定**和**下一轮 agent 的任务说明**（`feeds_forward` 的增强形式），而不是当前 phase。
- 语义验证使用 `BudgetAdjustTier` 同样的成本控制——接近预算时降级为仅语法+结构验证。
- 验证结果写入 trace event 和 `phaseOutputLedger`，供后续 agent 消费。一个 implementer 如果看到上一轮 reviewer 的验证报告说「ADR 缺乏架构权衡讨论」，可以在本轮补上。

### 估算

- 语法验证框架（声明式产出规格 + 机械检查器）: 1 sprint
- 结构验证（代码改动 ↔ ROADMAP 条目关联）：1 sprint（已有 `file_delta.go` 的 partial 基础）
- 语义验证（LLM-as-judge 集成）：1.5 sprints
- **合计**: ~3.5 sprints

### 风险

- 过度验证会扼杀 agent 速度和自主性。设计原则：验证工厂是**咨询性**的（advisory），非阻断性（non-blocking）。阻断决策留给 gate。
- 语义验证的 LLM 成本需要控制。建议：默认关闭 `--verify`，仅在 `mode=engineering` 或 `--verify` 标志时启用。

---

## 方向 4（P1）: 跨项目模式记忆与组织学习

### 现状

ForgeOS 的记忆系统（`internal/memory`）完全是 per-project 的：

- `memory.jsonl` 存放在项目根目录的 `.forge/` 下
- `internal/prompt/retrieve.go` 的 TF-IDF 检索只查找本地 memory
- scorecard 数据 (`scorecards.json`) 也是 per-project
- 没有任何机制让项目 A 学到的东西影响项目 B

### 为什么需要

**这是 ForgeOS 从「个人效率工具」跃升为「组织工程智能」的关键一步**。当前的状态意味着：

1. **每一件事都要重新发现** — 项目 A 发现「使用 `context.Background()` 导致优雅关闭困难」，修复了所有调用点。然后项目 B 开始，重写同样的 bug。
2. **路由策略无法跨项目优化** — scorecard 数据是每个项目孤立的。如果项目 A 发现 `Opus` 在架构评审中表现优异，项目 B 的 router 无法继承这个洞察。
3. **组织模式无法制度化** — 一个团队有「我们的 Go 项目遵循 `internal/` 布局」的模式，但无法让 ForgeOS 在启动新项目时自动使用这个模式。
4. **`forge-init` 的模板是静态的** — 它复制 `examples/starter` 的固定骨架，不管你的组织是否已经积累了 10 个项目的最佳实践。

### 现有防线及为何不足

| 防线 | 限制 |
|------|------|
| `forge-init` 模板项目 | 静态文件复制，不从组织经验学习 |
| `memory.jsonl` per-project | 隔离，跨项目不可见 |
| `scorecards.json` per-project | 隔离 |
| `.agent/` 共享机制（ADR-0003 submodule） | 只共享治理资产（agent 卡/workflow），不共享记忆和学习成果 |

### 建议的扩展方向

构建**组织级知识库**（Organizational Knowledge Base），作为一个可选的共享层：

```
┌────────────────────────────────────────┐
│         组织级知识库（新增）              │
│  - 跨项目洞察: 非敏感模式、架构决策       │
│  - 路由历史: (task_type, model) 评分    │
│  - 模板演化: stat 模式 → 组织模式        │
│  - 权限: 只读共享，不公开 secrets        │
└───────┬─────────────────────┬───────────┘
        │                     │
        ▼                     ▼
┌───────────────┐    ┌───────────────────┐
│ forge learn   │    │ forge init --org   │
│ (推送/拉取     │    │ (从组织知识库初始  │
│  跨项目洞察)   │    │  化项目模板)       │
└───────────────┘    └───────────────────┘
```

**关键设计决策**：

- **不做中心化服务**（没有 server，没有数据库）。组织知识库是**一个 Git 仓**（`forgeos-knowledge`），用 git 做版本控制 + 分发。这保持 forge-core 零外部依赖的承诺，且天然支持代码审查（PR 来审查新洞察）。
- **洞察分级**：`exact`（可逐位复用，如「这个 Go 版本的 vet 检查器有误报」） vs `heuristic`（趋势性的，如「Opus 比 Sonnet 在 Go 代码评审中发现的 bug 多 27%」）。精确洞察可直接用于路由决策；趋势洞察影响默认值。
- **项目显式选择加入**（`project.yml` 加 `knowledge_bases: [forgeos-knowledge]`）。默认不推送、不拉取任何跨项目数据。隐私默认是承诺：没有隐式数据离开项目边界。

### 估算

- 知识库数据模型 + `forge learn push/pull` CLI: 1.5 sprints
- `forge init --org` 集成: 1 sprint
- 路由历史汇聚（跨项目 scorecard 聚合）: 1 sprint
- **合计**: ~3.5 sprints

### 风险

- 组织知识库不能变成机密数据泄露通道。必须假定每个提交的洞察都被公开（如果知识库仓是公开的）。建议默认只推送非敏感模式（架构模式、通用问题解决方案），不推送项目名、环境变量名、第三方凭证等。
- 知识的「新鲜度」和「适用度」问题——一个 6 个月前的洞察可能已不适用。需要 `effective_until` 字段或置信度衰减（类似 Sprint 11 的 scorecard recency 衰减）。

---

## 方向 5（P2）: 生产预算治理与成本智能

### 现状

当前的成本控制机制是：

- **per-run `--run-budget-usd`** — 单次 `forge run/evolve` 的硬封顶
- **`--agent-max-budget-usd`** — 单次 agent 调用的软封顶
- **`internal/routing.BudgetAdjustTier`** — 预算接近时降档模型
- **`internal/orchestrator/budget.go`** — run-level 预算追踪与阻断

但这些全是**单次运行级别**的控制。没有任何**组织级**预算治理：

- 没有项目级月度预算
- 没有分团队/分项目的成本归因
- 没有「预算异常检测」（为什么这个 sprint 的 agent 成本比上个月涨了 300%？）
- 没有成本优化建议（「你的 reviewer 有 80% 在使用 Opus，但其中 40% 的 review 内容很简单，可以考虑降档」）

### 为什么需要

**当 LLM API 费用成为实际运营支出时，没有治理机制等于在开空白支票**。

1. **实例**：team A 和 team B 共享同一个 Anthropic 账号的配发预算。team A 的一个 engineer 跑了一个过重的 `forge evolve`，消耗了预算的 60%，team B 当月剩余预算不足完成 sprint goal。当前系统没有预防或告警机制。
2. **实例**：一个 `forge evolve` 在 3 次迭代中 Burned $23 的 API 调用，但产出只是重命名了一个函数。没有人在事后做成本效益分析，因为数据不足。
3. **实例**：系统默认 production lifecycle 的 reviewer 强制 Opus。但如果一个 reviewer 只检查 Markdown 文档（非代码），Opus 是浪费的——但当前路由不检查 task_type 与 lifecycle 阶段的交叉关系。

### 现有防线及为何不足

| 防线 | 限制 |
|------|------|
| `--run-budget-usd` | per-run，不跨 run 累计。用完换一个 `--run-budget-usd` 变高即可绕过 |
| `BudgetAdjustTier` | 只降低当前 run 的档位，不影响未来的 run |
| `.forge/trace.jsonl` 包含 cost_usd_micros | 数据存在但无聚合/告警/仪表盘 |
| `scorecard.json` 有 `avg_cost_usd` | 事后归因，无实时预算余量视图 |

### 建议的扩展方向

构建**预算治理平面**（Budget Governance Plane），位于 routing 和 executor 之上：

```
┌──────────────────────────────────────────────────┐
│              预算治理平面（新增）                   │
│  - 项目级预算: 月度/ sprint 级硬上限 + 软告警     │
│  - 成本归因: 按 (项目, phase, agent, model)        │
│  - 异常检测: 同比/环比成本异常波动告警             │
│  - 优化建议: 「你的 X 开销 Y% 可用 Z 替代」       │
└───────┬─────────────────────────────┬─────────────┘
        │                             │
        ▼                             ▼
┌──────────────┐            ┌────────────────────┐
│ project.yml  │            │ forge cost report  │
│ + budget: {} │            │ (CLI 成本分析命令)  │
└──────────────┘            └────────────────────┘
```

**关键设计决策**：

- **不做持久化服务**。预算配置在 `project.yml` 中声明（`budget: { monthly_usd: 500, sprint_usd: 200 }`）。归因数据从本地 `trace.jsonl` 聚合——不需要 server、不需要数据库。
- **异常检测是启发式的，不是 ML 的**。简单规则引擎（「本周 cost/phase 是上周的 2 倍」→ 告警）。不需要引入复杂 ML 依赖。
- **优化建议是基于规则的**，不是基于模型的。检查 routing 日志：如果一个 task_type 80% 分配了 X 模型但 90% 的同类 task 用更便宜的 Y 也能通过。这是一系列 SQL-able 查询，不是模型训练。
- **告警通过 CLI 输出**（`forge evolve` 结束时如果使用了超过 80% 的月度预算就显示 warning）。不需要 email/Slack 集成——那是 v3 Web UI 的职责。

### 估算

- `project.yml` budget schema + `forge cost report` CLI: 1 sprint
- 运行中预算检查 + 告警: 1 sprint
- 成本归因 + 历史追踪（基于已有 trace 数据）: 0.5 sprint
- 异常检测 + 优化建议规则引擎: 1 sprint
- **合计**: ~3.5 sprints

### 风险

- 预算治理不能变成 development blocker。如果一个项目在 sprint 末撞到月度预算上限，应该告警而不是阻断。建议：硬阻断（exit 1）仅在 `forge run` 起点检查，且仅当项目声明了 `budget.hard_stop: true` 时生效。
- 成本归因依赖 trace 数据的准确性。Sprint 26 已验证成本追踪正常工作（真实的 `total_cost_usd` 从 claude JSON 输出解析），但覆盖率仍需持续监控。

---

## 方向之间的依赖关系

```
方向 2 (可观测性)             方向 1 (冲突检测)
     │                            │
     │ 事件总线                   │ 文件干涉图
     ▼                            ▼
方向 3 (验证工厂) ──────────→ 方向 5 (预算治理)
     │                            │
     │ 验证报告流                  │ 聚合成本归因
     ▼                            ▼
方向 4 (跨项目记忆) ←──────── 方向 5 (追踪数据)
     │
     │ 组织知识库
     ▼
(forge-init 增强 · forge learn)
```

**关键依赖链**：

1. **方向 2（事件总线）是方向 3（验证工厂）的前提** — 验证工厂的验证报告需要通过事件总线流式推送到 CLI 和 trace 归档。没有事件总线，验证报告就是一个额外的、异步的、不可观测的黑箱。
2. **方向 1（冲突检测）与方向 2 可以并行** — 两者都修改 orchestrator 但不会在同一个文件上产生写冲突（一个加调度层、一个加事件发射层）。
3. **方向 5（预算治理）受益于方向 2 的数据管道** — 事件总线让成本归因从「事后扫描 trace.jsonl」升级为「实时流式聚合」。但不是强依赖——方向 5 可以先基于 trace.jsonl 的事后扫描做 v0，方向 2 就绪后再升级。
4. **方向 4（跨项目记忆）是方向 3 和方向 5 的汇聚消费者** — 验证工厂产生的验证模式和组织级成本洞察都需要跨项目记忆来跨项目复用。方向 4 是其他所有方向的价值放大器，不是它们的依赖。

---

## 推荐执行顺序

| 阶段 | 方向 | 为什么这个顺序 | 估算 |
|------|------|---------------|------|
| **Sprint N+1** | **方向 2 Phase A**（Event Bus 核心 + CLI `--verbose`） | 可观测性是调试一切其他方向的基础。错误定位速度直接影响开发效率 | 1 sprint |
| **Sprint N+2** | **方向 1 Phase A**（声明式文件集 + 干涉图串行化） | 并行安全和冲突检测是生产 multi-agent 的硬前提。没有它，并行 `RunParallel` 不安全 | 1 sprint |
| **Sprint N+3** | **方向 3 Phase A**（语法+结构验证工厂） | 验证工厂降低了对 agent 自报告的信任依赖。方向 1 保证了 agent 不互相踩脚，方向 3 保证了 agent 产出的内容质量 | 1.5 sprints |
| **Sprint N+4 — N+5** | **方向 5 Phase A**（预算 schema + cost report） + **方向 4 Phase A**（知识库数据模型） | 两者无依赖，可并行。方向 5 是生产运营成本控制，方向 4 是组织学习能力的起点 | 2 sprints |
| **Sprint N+6+** | 方向 2-5 的 Phase B/C 深化 | 以上基线就绪后，逐步深化：语义验证、组织知识库自动同步、实时预算仪表等 | 持续 |

**快速启动选项**（如果资源有限，优先级排序）：

1. **强制项**（没有它不启用到生产）：方向 1 Phase A + 方向 2 Phase A
2. **高价值项**（产出 > 投入）: 方向 3 Phase A
3. **组织扩展项**（团队规模 > 5 人时启动）: 方向 4 + 方向 5

---

## 总结

ForgeOS v2 的架构核心（18 个 Go 包、纯标准库、零外部依赖）在生产就绪度上已经超过了大多数同体量的 AI-native 工具。但以下几个系统性缺失阻碍了它从「高可信原型」到「生产级平台」的跨越：

| 缺失 | 影响 | 对应方向 |
|------|------|---------|
| **多 agent 写冲突无检测** | 并行编排不安全；串行编排也会被跨 phase 冲突静默破坏 | 方向 1 |
| **运行时黑箱** | 调试依赖事后翻日志；用户对 headless agent 运行无信任；预算消耗无实时可视性 | 方向 2 |
| **输出质量盲信** | 收敛循环信任 agent 自报告；gate 太晚且太粗；ADR 等文档内容无验证 | 方向 3 |
| **跨项目学习断层** | 每个项目从零开始；组织经验不传递；路由策略不跨项目继承 | 方向 4 |
| **预算治理缺失** | 单 run 预算有上限，但项目级、团队级无治理；成本归因只有事后 trace 数据 | 方向 5 |

**按生产环境使用场景分级**：

- **单人开发的托管项目**（ForgeOS 最小原型）：方向 2 足够。能看见 agent 在做什么即可。
- **团队协作的工程化项目**（mode=engineering, lifecycle=growth）：方向 1 + 方向 2 必需。
- **组织中大规模部署**（>10 个项目, >20 名开发者）：方向 1-5 全部需要。

每个方向均保持 forge-core「纯 Go 标准库、零外部依赖」的核心承诺——方向 1 和方向 2 完全零依赖；方向 3 的可选语义验证可能需要 LLM 调用（已在 executor 中）；方向 4 基于 Git 分发（已在 forge-core 中）；方向 5 基于本地 trace 数据聚合（已在 harness 中）。
