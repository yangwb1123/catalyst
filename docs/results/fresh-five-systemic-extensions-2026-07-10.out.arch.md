现在我已经完整阅读了项目上下文文档，包括 BOOTSTRAP.md、ARCHITECTURE.md、PROJECT.md、ROADMAP.md、AGENTS.md、CURRENT_SPRINT.md、DECISIONS.md 以及北极星架构文档。以下是我基于输入文档 + 项目实际代码状态的架构分析。

---

# ForgeOS 架构分析：五方向审阅的二次架构审视

> 分析范围：基于给定的五方向审阅文档 + 项目当前代码状态（Sprint 31 完工态）
> 角色：独立架构审阅

---

## 一、架构评估

### 1.1 当前架构的优势

**① 存量架构质量极高**

ForgeOS 目前处于一个罕见的「高纪律 + 高可观测」状态。Sprint 27-31 的治理债务清偿、`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 生成、以及三次架构自纠（`.arch/rules.yaml` 上限回调而非放任虚高、`cmd/forge` 逻辑迁入 `internal/gate`），说明项目的**元架构正在自我治理**——这是仅有少数字眼描述但只有真实的工程纪律才能达成的状态。

具体可量化的架构健康指标：

| 维度 | 当前值 | 意义 |
|---|---|---|
| Go 外部依赖 | 0（`go.mod` 无 `require`） | 供应链攻击面极小 |
| 源文件数 | ~195（arch-check 实测） | 中等规模，可控 |
| `cmd/forge` 文件数 | 16（包含 headroom 17） | 包边界纪律严格执行 |
| 函数 > 50 行 | 0 | 函数长度执法真抓真改 |
| 循环依赖 | 0 | 包结构健康 |
| 自测 | ~211（test）+ 39（app-test） | 测试覆盖率框架就绪 |
| 架构检查 | 8/8 PASS | 8 维架构约束全自动化 |

**② 中枢旋钮（mode×lifecycle）是真正优雅的设计**

三种输出（Router 档位 + Harness 严格度 + Workflow 深度）受同一个输入驱动，这是典型的高杠杆抽象。Sprint 12-18 逐块补齐后，现在 `explorer`/`balanced`/`engineering`/`cto` 四种 mode × 四种 lifecycle 产生有意义的组合行为，而非死记硬背的 switch 卡片。

**③ 审阅文档本身验证了架构的工程债水平**

文档能定位到 `loop.go:nextStartPhase` 的 `OnRejected` 注释和 `prompt_memory.go` 的无差别注入——这只有在代码库没有大规模「代码异臭」时才有可能。烂代码库里，架构师看不到这种级别的细节，视野全被 `utils.go` 和 `ManagerFactory` 堵死了。

### 1.2 当前架构的局限性

**① 控制面 / 数据面共享进程空间**

北极星架构要求控制面与数据面分离（类比 k8s 的 controller manager 与 kubelet），但当前 forge-core 是单体二进制，orchestrator 与 CommandExecutor 同进程。这意味着：

- **agent OOM 直接带走控制面**（Sprint 22 的 output-size cap 部分缓解，但 RSS 不受控）
- **没有独立的故障域**：orchestrator.go 的一个 panic 可以带走正在执行的所有 phase
- **无法水平伸缩**：每个 forge-core 实例跑一个 workflow，没有工作窃取

这不是当前阶段的错误决策（单体在 v2 是正确的），但未来必须正视。

**② `prompt_context.go` / `cost.go` 的机读契约级联增长**

Sprint 28-31 在 `cost.go` 中级联了三个解析器（二元 reviewer verdict → 五择一 executive verdict → confidence score），均通过同一个 `unwrapClaudeResult` 管道 + 末行精确匹配。每次新增判据都会：加解析器 → 加 `observeFor` fallback → 加字段 → `gates.go` 加函数 → `gatherSignals` 接线。当前 3 级 fallback 还可管理，但扩展到 7-8 级将变成不可维护的隐式优先级链——错误匹配顺序的 bug 极难复现。

**③ 记分卡（scorecard）的跨 session 持久化仍是平面文件**

当前 scorecard 是每个项目目录下的 JSON 文件（`~/.forge/scorecards/<project>/`），没有 schema 版本控制、没有冲突合并策略、没有跨项目聚合。Sprint 11 实现了 recency 衰减，但衰减是基于文件修改时间的启发式——如果 scorecard 被复制或备份恢复，衰减语义失真。

### 1.3 关键设计决策评估

| 决策 | 评估 | 理由 |
|---|---|---|
| **Go 纯标准库零依赖** | ✅ 正确 | 供应链安全 + 静态二进制 + 跨平台部署。代价是 YAML 解析必须走 python shim，但这是有意识的 trade-off |
| **gate 为主、acceleration hook 为辅** | ✅ 正确 | 验证了「host-independent 执法」的原则，而且真在 CI 中捕获了回归 |
| **YAML → JSON via python shim** | ⚠️ 可接受 | 临时脚手架明确标注，未来可内建 YAML 解析器。当前阶段零依赖的价值大于自研 YAML 解析器的成本 |
| **机读契约用末行精确匹配** | ⚠️ 正确但有上限 | 对有限的判据集合（3 种 verdict + confidence）工作良好。但扩展时应考虑结构化输出格式 |
| **`forge-core` 单二进制不拆分** | ✅ v2 正确 | 先跑通端到端，再按北极星拆分微服务。Sprint 24-26 证明了单体可处理真实 multi-agent 闭环 |

---

## 二、扩展方向（3-5 个高价值方向）

### 方向 A：结构化机读契约层（替代末行精确匹配）

**为什么需要**：当前 3 级 fallback 的 `unwrapClaudeResult` + 末行精确匹配模式正在接近复杂度上限。每个新增 agent 角色需要新解析器 + 新 fallback 顺序，且 `prompt_memory.go` 已经存在「agent 输出到 memory 不做结构化过滤」的问题（原文档方向②指出）。如果汇聚到 7-8 级 fallback，调试难度呈超线性增长。

**核心挑战**：
1. **不引入外部依赖**：这是 forge-core 的 hard constraint（零外部依赖），所以不能直接上 JSON Schema / Protobuf
2. **向后兼容**：已有 3 个 agent 角色（reviewer、cto、product-manager）使用了末行契约，不能要求用户全部升级
3. **agent 遵从性**：LLM 输出本就不稳定，结构化契约层需要容忍非结构化退化

**建议方案**：
```
Phase 1（与末行契约共存）：
- 在 agent 卡中声明 `structured_verdict: true`
- 如果 agent 声明了该标志，`cost.go` 优先解析结构化块（如 ```verdict ... ```）
- 否则回退到当前末行精确匹配
- 单坐：不改变任何现有 agent 卡行为

Phase 2（逐步迁移）：
- review.yml 的 reviewer/cto/product-manager 相位逐个加 `structured_verdict: true`
- 旧末行匹配作为 fallback 保留 2 sprint

Phase 3（废弃旧路径）：
- 移除末行匹配 fallback（当所有 agent 卡都已迁移后）
```

**对现有系统的影响**：中低。`cost.go` 的 parse 函数链需要重构为策略模式（Strategy），每个解析器实现同一个接口。`gates.go` / `gatherSignals` 不需要改动。零外部依赖的约束需要在 `internal/verdict` 包中自建结构体 + JSON marshal。

**业务价值**：
- 方向②（信任链加固）的核心前置：结构化 provenance 字段需要结构化契约承载
- 方向①（仿真测试）的核心前置：LLM 返回值序列注入需要清晰的 schema（而不是模拟末行匹配的字符串集合）

---

### 方向 B：phase 输入输出 schema 显式化

**为什么需要**：Sprint 26 实现了 `feeds_forward` / `phaseOutputLedger`，但这是基于字符串键值对的隐式管道——planner 输出什么、implementer 读什么、reviewer 审什么，全在 Go 代码的散落键名中。当前两个消费者（planner task prep → implementer、gate 裁决注入 reviewer）通过 `phaseOutputLedger` 传递，但每新增一个数据流，都需要修改 `prompt_context.go` 和 `observeFor`。

**核心挑战**：
1. 当前设计已经是「比没有好」——`feeds_forward` 在 Sprint 26 真 claude 跑通后验证了价值
2. 显式 schema 可能变成过度工程（如果数据流数量停留在 3-4 条）
3. agent 输出不具备类型安全，schema 验证只能做后验检查

**建议方案**：
- 在 `internal/orchestrator` 中定义 `PhaseIO` 结构体（声明各 phase 的输入输出字段名和类型）
- 每个 workflow phase 的 `emits` 字段（YAML 已有的声明）与 `PhaseIO` 做漂移检查——agent 卡声明的产出 vs 代码实际消费的产出
- 不做运行时类型检查，只做装配时的字段存在性校验（类似 `check.py` 现在的 agent 引用校验）

**对现有系统的影响**：低。当前 `phaseOutputLedger` 是 `map[string]any`，显式 schema 只是一个文档层 + 校验层。可以逐步引入，现有路径不改。

---

### 方向 C：跨 session 记分卡聚合引擎

**为什么需要**：当前 scorecard 是 per-project、per-session、per-model 的 JSON 文件。方向⑤（成本优化引擎）和方向③（workspace 级多项目学习）的前置依赖，以及方向①（仿真测试）的基线数据来源。Sprint 19 实现了 telemetry 框架（`scorecard*.mjs`），Sprint 26 真 claude 补齐了 latency/cost 三维真数据——当前缺的是跨 session 的条件聚合能力。

**核心挑战**：
1. **统计显著性**：真数据点目前 ~3-5 条（如原文档所述），做条件聚合需要按 task_type × language × model 做 cube。需要积累足够数据
2. **数据冲突**：同一个模型在不同项目类型上的表现差异——简单的全局平均会误导路由决策
3. **存储架构**：当前平面 JSON 文件不支持多维聚合查询

**建议方案**（与方向⑤的务实建议一致）：
```
短期（不离线改动存储架构）：
- `forge scorecard merge` CLI 命令（手动跨项目合并）
- `--task-type-filter` 参数（只合并同类任务）
- `forge run --budget` help text 显示历史参考

中期（存储升级）：
- scorecard 加 schema version 字段
- 实现按 task_type × model × language 的下钻聚合
- 聚合结果喂入 Model Router（目前只接 `HistoryTiebreak`）

长期（按北极星）：
- 迁移到 Qdrant / PG 的按需聚合查询
- 成本预测引擎（方向⑤）
```

**对现有系统的影响**：中。`internal/routing` 包的 `HistoryTiebreak` 目前是单维度（只读 scorecard 的直接字段），需要扩展为多维度加权评分器。但 `internal/routing` 自身的文档已说明这是 v2+ 工作，所以改动在预期范围内。

---

### 方向 D：执法器（gate）的自检 / SLO 监控框架

**这是输入文档推荐为最高优先级的方向④**。我完全认同，但补充一个发现：项目当前状态比输入文档的描述更好——Sprint 30 已经实现了 `mode_gating` 漂移守卫（`harness/mode_gating_check.py`），以及 Sprint 19 的 SCA/CVE 框架。但**执法器退化检测**仍然空白。

**核心挑战**：
1. 执法器（`arch-check.mjs` / `check.py` / `secret-scan.mjs`）本身也可能退化（如 Sprint 29 的 checkFanin 误算测试文件耦合）
2. 执法器退化在常规测试中不会暴露（因为测试文件是用已知通过/失败样本写的，但退化可能只在特定输入下触发）
3. 四维资源护栏（recursion / budget / timeout / output-cap）各自独立，缺少统一健康检查入口

**建议方案**（与输入文档的 `forge doctor --gates` 一致）：
- 每个 gate 脚本添加 `--self-test` 模式：读已知违规/通过样本，断言预期输出
- `forge doctor` 聚合所有 gate 的 self-test 结果
- 再加一层：**gate 退化检测的退化检测**——self-test 样本文件本身被 CI 检查其正确性（即：样本文件不会因为 gate 修复而变成错误样本）

**对现有系统的影响**：极低。与现有 `harness/**` 结构兼容，`forge doctor` 命令已在 Sprint 27 创建（`internal/doctor` 包），只需要接线。

---

### 方向 E：loop-back 状态机的收敛性验证（正式模型）

**为什么需要**：Sprint 13 实现了 `on_fail.loop_back` 定向跳转，Sprint 28-31 实现了 `stop_condition.on_rejected` 从死代码变为真实行为。但这套状态机目前是**通过测试验证的，而非通过形式化模型验证的**。

核心风险：loop-back + on_rejected + human_gate 的可能组合是 `O(phase_count × loop_limit × verdict_types)`。当前测试覆盖了典型路径，但边界情况（如 loop_back 到已完成的 phase 后再次 gate FAIL → 再次 loop_back → ... → loop_limit 耗尽）的行为没有穷举。

**建议方案**（与输入文档的方向①一致，但裁剪版本）：
- 不构建通用仿真引擎（不做调度交错穷举）
- 专注 LLM 返回值序列 × loop-back 计数器的状态爆炸：
  - 将 loop-back 状态机建模为有限状态机
  - 为每个合法路径生成 LLM 返回值序列（`APPROVED` / `REQUEST_CHANGES` / `REJECTED` 的组合）
  - 验证关键不变量：`totalSpent <= MaxRetries × MaxAgentCalls × perCallBudget`

这个方向与方向 A（结构化机读契约）自然衔接——FSM 的状态转移条件就是机读契约的解析结果。

**对现有系统的影响**：中。需要在 `internal/orchestrator` 中为 `LoopEngine` 添加抽象层，使其可注入模拟返回值序列（当前 `CommandExecutor` 已经是接口，所以注入点存在）。不影响现有功能路径。

---

## 三、接口设计建议

### 3.1 关键接口设计原则

**原则 1：数据流应显式声明而非隐式传递**

当前 `phaseOutputLedger` + `feeds_forward` 的设计已经是过渡方案的正确方向——从「每个 phase 自己从 trace 中提取需要的信息」转向「workflow 声明 phase 之间的数据流动」。下一步应该：

```
// 当前（隐式）：
phed.Output["task"] = plannerOutput  // 字符串键，没有类型检查
phase.Input["task"] = ledger["task"]  // 消费者必须知道键名

// 目标（显式）：
type PhaseIO struct {
    Role       AgentRole
    Reads      []IOField // 从哪些 phase 的 emits 读
    Emits      []IOField // 写出什么字段
}

type IOField struct {
    Name     string
    Type     FieldType // string / verdict / confidence / scorecard
    Required bool
    FromPhase PhaseID // for Reads: 来源 phase
}
```

这个 interface 的改动不需要一次性落地，可以从 `check.py` 的校验层开始——先验证 workflow 声明的 `feeds_forward` 与 `PhaseIO` 的一致性，再做运行时接入。

**原则 2：解析链使用策略模式而非级联 fallback**

当前 `cost.go` 的三级 fallback（reviewer → executive → confidence）应该重构为：

```go
type VerdictParser interface {
    CanParse(output string) bool
    Parse(output string) (*Verdict, error)
}

type VerdictParserChain struct {
    parsers []VerdictParser
}

// Register 注册新的解析器（按优先级递减）
func (c *VerdictParserChain) Register(p VerdictParser)

// Parse 按注册顺序尝试所有解析器
func (c *VerdictParserChain) Parse(output string) (*Verdict, error)
```

这样每新增一个 agent 角色只需要新增一个实现了 `VerdictParser` 的 struct，然后 `Register`，不修改已有逻辑。

**原则 3：外部接口契约用声明式约束**

当前 `.agent/agents/*.md` 是机读契约 + 人读描述的混合体。Sprint 28 的 reviewer/cto 机读契约 token 工作很好，但格式是自然语言中的某一行。当机读契约扩展到 5+ agent 角色时，应该考虑：

- 给 agent 卡加一个 `## Machine Contract` 段（结构化 YAML 内嵌，如 `verdict: {format: "UPPER_SNAKE", tokens: ["APPROVE", "REJECT"]}`）
- `check.py` 的 `check_workflow_agent_refs` 升级为校验 agent 卡的 machine contract 与代码解析器的一致性

### 3.2 是否需要引入新的抽象层

**需要：`internal/verdict` 包**（当前功能散落在 `cost.go` / `prompt_context.go` 中）

当前 verdict 解析、confidence 解析、gate 信号解析都分散在 `cost.go`（历史原因：`cost.go` 是最早加机读契约解析的地方）。Sprint 28-31 在同一个文件叠加了三级解析，但没有抽取公共层。

建议创建一个 `internal/verdict` 包：
- `VerdictParser` 接口（策略模式）
- `ReviewerVerdictParser` / `ExecutiveVerdictParser` / `ConfidenceParser` 实现
- `ChainParser` 组合模式（带 fallback）
- 不引入外部依赖（纯 Go 标准库 strings + regexp）

**不需要：新的运行时抽象层**

当前 `CommandExecutor` 接口（Sprint 13 设计）+ `DryRunExecutor` + `agentExecutor` 的三分设计已经合理。不需要引入新的 executor 类型。但需要为仿真测试加一个 `InjectableExecutor`（或扩展 `DryRunExecutor` 使其可注入模拟返回值序列）。

### 3.3 向后兼容性策略

当前 forge-core 的 v2 状态（零外部依赖、Go 标准库）决定了**向后兼容的基本策略是源码级兼容而非 wire 级兼容**（因为没有 wire 协议）。具体：

1. **配置文件兼容**：`.agent/workflows/*.yml` 当前 schema 不应 break。新增字段（如 `structured_verdict: true`）使用缺失时回退旧行为的语义
2. **CLI flag 兼容**：`forge run` / `forge evolve` 的 flag 不应删除，只应增加。`--model` / `--max-agent-calls` 等后期 flag 已验证了这个原则
3. **agent 卡兼容**：机读契约新增 token 不应使旧 token 失效（当前 `parseReviewerVerdict` 和 `parseExecutiveVerdict` 的设计保证了这一点）

---

## 四、技术选型

### 4.1 是否需要引入新的技术栈

**当前不应引入的新技术：**

| 技术 | 为什么不引入 | 何时考虑 |
|---|---|---|
| **JSON Schema 库** | 违反「零外部依赖」红线 | 只在卸载 harness 到其他项目时可选 |
| **gRPC** | v2 单体不需要 IPC | v3 微服务化时 |
| **Temporal 客户端** | 单体不需要 durable workflow 引擎 | v3 x-ha rollout 时 |
| **向量数据库** | 当前 TF-IDF 检索已足够 | scorecard 数据量 > 10⁴ 条时 |

**当前值得考虑的内部构建：**

| 构建 | 理由 | 红线检查 |
|---|---|---|
| `internal/verdict` 包 | 抽取公共解析层，消除 `cost.go` 的责任膨胀 | 零外部依赖 ✓ |
| `internal/fsm` 包（有限状态机） | loop-back 状态机的形式化建模 | 零外部依赖 ✓ |
| `internal/aggregation` 包（多维聚合） | scorecard 跨 session 条件聚合 | 零外部依赖 ✓ |

这三个包的共同特征：纯计算逻辑，不涉及 I/O，不需要外部存储——因此可以用纯 Go 标准库实现。

### 4.2 第三方依赖的评估标准

ForgeOS 的「零外部依赖」政策（来自 `AGENTS.md` + `go.mod` 无 `require` 的事实）是一个**带成本的承诺**——它阻止了供应链攻击，但意味着标准库缺失的功能必须自建。

评估引入依赖的标准（按优先级排序）：

1. **安全影响**：该依赖是否引入 CVE 面？如果 yes → 强推自建或接受功能缺失
2. **功能替代成本**：自建的工作量是否 > 依赖本身的维护成本？如果是 YAML 解析器（~200 行标准库实现），自建合理；如果是 TLS 栈，明显不合理
3. **行业通用性**：该依赖是否是行业事实标准（如 `golang.org/x/crypto`）？是则可以破例
4. **go.mod 的零依赖现状**：当前零依赖是项目的重要安全特征和营销亮点（「纯标准库」）。引入第一个依赖是重大决策

### 4.3 自建 vs 采购的决策依据

当前 forge-core 的「自建一切」是正确决策，原因：

1. forge-core 的功能（orchestrator / model router / context engine / memory / converge）都是**业务逻辑密集**而非**基础设施密集**——自建的成本是开发时间，采购的成本是耦合和供应链风险
2. 「零外部依赖」本身是一个产品特性——企业采购方看到 `go.mod` 无 `require` 会有强烈的安全信心
3. 北极星架构中采购的部分（Temporal / LiteLLM / Firecracker / OPA / Vault）都是在 v3 才进入，且采购的是基础设施引擎而非业务逻辑

**唯一的例外是 YAML 解析**：当前走 `python shim`（`harness/yaml2json.py`），这是正确的临时决策。当这个 shim 成为瓶颈时（如 forge-core 需要在不装 Python 的环境中运行），决策顺序应该是：

```
Option A: 嵌入 Go YAML 库（破例引入第一个依赖）
  - 成本：go.mod 增加 1 行
  - 收益：移除 Python 依赖
  - 风险：选择的 YAML 库会带来其传递依赖

Option B: 自建最小 YAML 子集解析器
  - 成本：~300-500 行解析代码
  - 收益：零依赖保持
  - 风险：只支持 forge-core 需要的 YAML 子集（标量/映射/序列/折叠块/字面块），这与 python shim 的全集不完全重合

Option C: 保持 python shim
  - 成本：部署需要 Python 环境
  - 收益：零改动，且经过真实验证（7 个真实 YAML 文件全通过）
```

当前阶段建议保持 Option C，但增加一个 Go 测试（用 `exec.Command("python3", ...)` 调用 shim 对比预期输出，类似 Sprint 27 的 `TestToJSON_MatchesPythonShim` 更正后的版本）。

---

## 五、实施路线图

### 5.1 优先级排序

基于输入文档的调整建议 + 项目当前状态（Sprint 31 已完工）的综合评估：

| 优先级 | 方向 | 理由 |
|---|---|---|
| **P0** | **D：执法器自检 / SLO 监控** | 工程纪律的基石。当前 harness 执法器自身无退化检测，而 Sprint 29 的 checkFanin 误报已证明退化真实存在。2-3 周完成，对后续所有方向提供安全网 |
| **P0** | **A：结构化机读契约层** | 方向②（信任链加固）和方向①（仿真测试）的共同前置。末行精确匹配的 3 级 fallback 已到扩容量极限，应在扩展第 4 个 agent 角色前重构 |
| **P1** | **B：phase 输入输出 schema 显式化** | 自然紧随方向 A。当前 `feeds_forward` 已工作，但缺少类型安全。此方向可确保方向⑤（成本引擎）和方向③（跨项目学习）的数据流清晰 |
| **P1** | **E：loop-back 状态机的收敛性验证** | 裁剪版仿真测试（LLM 序列注入 + budget 不变量穷举）。方向 A 的结构化契约提供了 FSM 转移条件的基础 |
| **P2** | **C：跨 session 记分卡聚合引擎** | 需要足够的数据点才产生统计显著性。建议在方向 A+B 完成后启动，且与方向⑤合并为「平台化轨道」|

### 5.2 阶段划分和里程碑

```
Phase 0（2-3 周）：工程纪律加固
  - 每个 gate 加 --self-test 模式
  - forge doctor 聚合 gate health
  - gate 退化检测的退化检测（self-test 样本的 CI 校验）
  
  里程碑：forge doctor --gates 全绿，任意 gate 退化在 CI 中立即暴露

Phase 1（4-6 周）：结构化契约 + 显式数据流
  - 创建 internal/verdict 包（策略模式解析器）
  - 迁移现有 3 级 fallback
  - phase 输入输出 schema 定义 + 校验层（check.py 新检查）
  
  里程碑：新增 agent 角色只需要新增一个 VerdictParser 实现，不需改任何现有代码

Phase 2（4-8 周）：裁剪版仿真测试
  - DryRunExecutor 扩展为可注入 LLM 返回值序列
  - loop-back FSM 建模 + budget 不变量穷举
  - 仿真测试框架嵌入 CI（可选的、非阻塞的）
  
  里程碑：orchestrator 修改前必须通过仿真测试的 phase

Phase 3（8-16 周）：聚合引擎 + 成本预测
  - forge scorecard merge CLI
  - 按 task_type × model × language 的多维聚合
  - 聚合结果喂入 Model Router
  - cost-per-roadmap-point 指标
  
  里程碑：forge route --scorecard 的历史择优基于统计显著的聚合数据
```

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| **机读契约扩展后反而降低 LLM 遵从性**（agent 输出结构化块的概率 < 当前末行匹配） | 中 | 高 | Phase 1 保持旧末行 fallback 直到有 data-driven 证据表明结构化块遵从率 > 95% |
| **gate self-test 样本文件本身漂移**（gate 修了 bug 但样本文件跟着改，无声掩盖） | 高 | 中 | gate 退化检测的退化检测——样本文件通过 CI 检查其正确性（与 gate 输出做互校验），禁止手改样本 |
| **仿真测试给出假安全感**（注入序列覆盖了所有合法路径但实际 LLM 行为超出预期） | 中 | 中 | 仿真测试与监控互补：仿真测不变量，监控测真实运行时的偏差。仿真通过 + 监控告警同时存在才构成完整安全网 |
| **scorecard 数据永久稀疏**（即使合并也达不到统计显著性阈值） | 中 | 低 | 设计时做 fail-soft：数据不足时退回基于 model tier 的简单路由，不产生误导性预估。成本预测引擎只在数据量 > 阈值时显示 |
| **用户不信任成本预测**（方差的相对值 > 平均值） | 高 | 中 | 路线图中已规划了保守的推出策略：一开始只作为 info 展示「类似 task 的历史中位数成本」，不作为自动决策输入 |

---

## 总结

原输入文档是一份高质量的架构分析——跨包边界准确、ROI 判断清晰、每个方向都附带代码证据。我的补充集中在：

1. **低估了项目当前架构成熟度**：Sprint 27-31 的三次架构自纠（`cmd/forge` 文件数回调、`internal/gate` 的纯逻辑迁移、`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 生成）已将架构质量从「好」推到「自我治理」。五方向审阅中一些「gap 太大」的判断需要向上修正

2. **低估了结构化契约层的紧迫性**：末行精确匹配到第 4 个 agent 角色就是极限，建议在扩展更多 agent 角色前重构为策略模式

3. **最被低估的方向：方向 D（执法器 SLO 监控）** ——输入文档给出了正确的最高优先级判断。当门卫自己也会生病时，监控门卫健康的系统是第一优先级的基础设施

4. **原作者的优先级调整建议基本合理**：④ → ② → ① → ③+⑤，但我把方向 A（结构化契约）提前为 P0——因为它既是方向②的前置，也是方向①的前置，更是当前 `cost.go` 扩展性的直接瓶颈
