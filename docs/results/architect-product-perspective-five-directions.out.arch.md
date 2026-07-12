Now I have a thorough understanding of the codebase, its architecture, and the evaluation document. Here is my architectural analysis.

---

# 架构分析：ForgeOS — 从"技术完备"到"采纳就绪"的架构跃迁

## 1. 架构评估

### 当前架构的优势

**1.1 分层纯度极高。** 代码库呈现出罕见的层间纪律——17 个 `internal/` 包构成清晰的单向依赖图：`asset`（纯数据模型）→ `converge`/`gate`/`memory`/`prompt`/`routing`/`trace` → `orchestrator` → `cmd/forge`（CLI 胶水）。`internal/` 包之间无循环依赖已被 `arch-check` 机器执法。每当架构违规逼近 500 行红线，项目执行"先拆分、再继续"纪律——Sprint 23 拆 `acceptance.mjs`、Sprint 27 拆 `internal/doctor`/`internal/attribution`、Sprint 29 拆 `internal/gate/resolve.go`——这些不是偶发整理，是一种**架构免疫系统**在主动运作。

**1.2 零依赖不是姿态，是工程约束。** `go.mod` 没有 `require` 块——32k+ LOC 的 Go 运行时完全依赖标准库。这使得 `forge` 二进制在任意目标机器上均静态可部署。CI 不需要 `go mod download`，不需要 GOPROXY，不需要 vendor 目录。这种约束迫使设计者在引入能力前先问"标准库能否表达这一点？"——YAML 解析走了 Python shim、BM25 检索在 `prompt.Retrieve` 中用纯 Go 手写——不走捷径。

**1.3 设计即代码，代码即设计。** 13 个 agent 卡的机读契约（`VERDICT: APPROVE`/`CONFIDENCE: 85`）不是元数据装饰，而是直接被 `cost.go` 中的 `parseReviewerVerdict`/`parseExecutiveVerdict`/`parseConfidenceScore` 消费——形成三条 fallback 链的信号提取管线。`.agent/workflows/*.yml` 声明的工作流 DAG 经 `yaml2json` shim 输入 `internal/orchestrator` 的真实引擎。设计文档和代码之间没有翻译层——文档的声明就是代码的输入。

**1.4 诚实性是架构质量属性。** 这不是修辞——项目将"诚实标注"作为 CI 闸门的一部分：SCA 无 DB 则 N/A、coverage 工具未安装则 N/A、dry-run 叙述而非假装执行、`forge accept` 的 `n/a` 项在 JSON 输出中显式可见。这种秉性贯穿了从 Sprint 24-26 的"真点火"流程——每次暴露的真实 gap 都被记录为机制缺陷而非偶发故障。

### 当前架构的局限

**1.5 Memory 管线是接通的，但检索策略是平坦的。** 这是评估文档指出的关键洞见。`memoryContext`（`prompt_memory.go:165-173`）在每个 prompt 构建周期调用 `memory.Load` 转储整个 store，然后 `boundMemory` 用 BM25 过滤至 ~24 条。但 `Confidence`/`Source`/`Supersedes` 元数据仅用于格式化标签，从未被用于筛选、路由或优先级排序。`memory.Query`（`memory.go:293`）实现了精确的 kind+topic 过滤器，但**零处非测试代码调用它**。架构上，这意味：

- **知识注入是广播而非定向的。** 每个 agent 无论角色（implementer、reviewer、architect）都会在 prompt 中接收到同样的混合条目池——BM25 只做通用相关性排序，不做阶段角色过滤。
- **元数据是装饰性负债。** 三个元数据字段的存在暗示了某种能力，但它们的消费端缺失——新人读到代码会假设这些字段有意义，然后跟踪到 `prompt_memory.go` 发现它们仅用于格式化。这本身就是一种代码级别的误导性信号。
- **修复路径是明确的、增量式的：** 将 `memoryContext` 中的 `Load→boundMemory` 改为 `Load→Query(kind, topic)→boundMemory`，用 Query 做阶段特定预过滤，用 boundMemory 做封顶保留——两者职责正交，不取代对方。

**1.6 编排引擎与执行语义之间存在阻抗不匹配。** `internal/orchestrator` 实现了 DAG 执行器、wave 调度（Kahn 排序）、并行阶段执行。但当前 5 个工作流 YAML 中**零个声明了 `depends_on`**，导致并行引擎的技术债务。RunFrom/RunParallel 两条路径都经过完整测试，但从未在生产中触发。这意味：
- 并行引擎的 wave 调度存在空转风险——没有真实工作负载验证其行为。
- 工作流 DAG 本质上是线性的，`forge run` 的单向串行执行是对的，但 `forge evolve` 的 loop-back 能力（`on_fail.loop_back`、`on_unmet.loop_to_next`）在 DAG 框架中是一种非平凡的转移语义——loop-back 需要相位索引计算 + `MaxLoopBack` 上限，这在 DAG 框架中引入了隐式状态（当前相位指针），而非纯函数式的 DAG 步进。

**1.7 测试架构：同构且覆盖广泛，但有两类缺口。** 代码库有 134 个 Go 源文件，其中测试文件约占 42/134（31%）。`forge accept` 聚合了 gate/check/secret-scan/arch-check/app-tests。但：
- **集成测试对外部环境敏感。** `test_acceptance.mjs` 需要真实 repo 上下文；Sprint 16 的 copy-anywhere 加固修复了脚手架继承，但集成测试套件在非 forgeos 环境中的行为未广泛验证。
- **性能测试缺失。** 无基准测试，无延迟预算，无内存分配的 `testing.B`。BM25 对 500 条条目 2-5ms 的运行时间是基于推测，非经验测量。

---

## 2. 扩展方向

### 方向 1 — Memory 知识反哺管线（P0）

**为什么需要。** 当前 memory 引擎实现了**存储（memory.Store + memoryEntry 模式）**和**检索（memory.Load + boundMemory + BM25）**，但知识反哺是广播式的：每个 agent phase 获得同一个全量转储的 BM25 切片，无论其角色。`memory.Query` 实现了 kind+topic 精确过滤却零消费。这直接导致：
- Agent prompt 中约 10-15% 的 token 预算浪费在与当前任务无关的记忆条目上。
- 无法将学习（Gap → Lesson → 行为改进）桥接回 agent 行为改进循环。
- 元数据（Confidence/Source/Supersedes）是装饰性负债。

**核心挑战。** 不在于实现——Query 已经写好了。核心挑战在于设计一种**阶段感知的检索策略**：
1. **检索 ≠ 注入。** 哪些内存条目值得注入 implementer prompt，与哪些值得注入 architect prompt 是不同的集合。不是所有记忆都该被所有 agent 看到。
2. **Confidence 信号需要真实分母。** 当前 Confidence 在 store 时设置，但从未被校准。50% 置信度是"有 50% 把握"还是"没验证过"？需要一个校准机制——要么来自重复观察（同条条目被多个 agent 独立确认），要么来自评分卡关联（高 Confidence 条目与高通过率的相关性）。
3. **Supersedes 的级联语义。** 一个条目可以 supersede 另一个，但当前没有实现 Supersedes 链的 DFS 级联——导致陈旧条目在 store 中无限累积，boundMemory 的 BM25 仍可能将其包含进来。

**预期架构变更。**
```
当前：memory.Load → boundMemory (BM25) → prompt inject
目标：memory.Load → memory.Query(kind, phaseRole) → boundMemory (BM25 capped) → prompt inject
```
涉及包：`internal/memory`（Query 接通）+ `internal/prompt`（memoryContext 重构）。影响范围：~200 行。不需要新类型。

**对现有系统的影响。** 向后兼容——未指定 kind/topic 的 Query 等价于当前的 Load 全量行为。`boundMemory` 的 cap 逻辑不变。变化仅在 `prompt_memory.go` 的 < 50 行管线代码中。风险极低。

**选项与权衡。** 可选的阶段感知策略有三种：
- **A. 角色名匹配。** Query 过滤 `kind=KindGap AND phaseRole="architect"`——最简单，但角色名与记忆条目的关联是人为约定的（`phaseRole` 需在 store 时写入）。
- **B. 阶段类型匹配。** Query 过滤 `kind=KindLesson AND phase_type IN ("review", "qa")`——不需要写入时元数据，但需要 phase 分类能力。
- **C. 混合策略。** Query 做角色名过滤，boundMemory 做通用 BM25 排序，**且** Confidence 低于阈值的条目被降权——这是最丰富的方案，但需要 Confidence 先被校准。

**建议：增量走 A → C。** A 可在 1 天内交付；校准和 C 是后续 sprint 的事。

---

### 方向 2 — Knowledge 学习闭环（P0，与方向 1 耦合）

**为什么需要。** 评估文档指出 `memory.Query` 零调用，但更深层的问题是：**记忆如何驱动行为变化？** 当前，memory store 是一条 append-only 的日志——条目被写入、检索、注入 prompt，但 agent 是否遵循建议**无法被验证**。构建一个从"gap 被识别 → lesson 被写入 → agent 行为被观察到匹配 lesson → confidence 被提升"的闭环，将 memory 从"离线知识库"升级为"行为矫正器"。

**核心挑战。** 闭环需要三个事物：
1. **归因：** 哪个 agent 在哪个 phase 中做出了与某个记忆条目相关的行为。
2. **影响度量：** agent 的最终输出是否反映了记忆条目的建议。
3. **反馈注入：** 如果 agent 持续忽略某个建议，系统应提高其显式程度（例如，从"可选" → "硬约束"）。

**预期架构变更。** 需要新的 `internal/learning` 包（~400 LOC），包含：
- `Attribution` 类型：将 agent 输出与记忆条目关联的数据结构。
- `Influence` 评估器：比较 agent 输出与记忆条目的语义重叠（不需要 embedding——基于关键词的 Jaccard 相似度足够作为 v1）。
- `ConfidenceUpdater`：基于影响度量调整条目的 Confidence 分数。

**对现有系统的影响。** `internal/memory` 保持无依赖；新的 `learning` 包导入 `memory` 和 `asset`。`cmd/forge` 不需要新命令——学习闭环作为 `forge evolve` 迭代周期的一步运行。风险中等——新包不会影响现有 gate/check/accept 路径。

**选项与权衡。**
- **v1 启发式**（关键词重叠 + 基于频率的置信度更新）——~400 LOC，2-3 天，不需要 embedding。
- **v2 语义**（引入 static embedding 如 TF-IDF + cosine）——需要向量存储和 embedding 模型调用。对于评估文档诊断的"35k Go 代码库"规模来说过度设计。

**建议：v1 先上。** 将记忆闭环链接到现有的 converge 评估管线——当发现 memory 条目与 agent 行为不匹配时，产生一个「learning gap」信号，在 `forge accept` 的 `converge.Signals` 中可见。

---

### 方向 3 — 部署交付流水线（P1）

**为什么需要。** 这是评估文档诊断的关键采纳瓶颈。ForgeOS 的"技术完备度 ≈ 95%，采纳就绪度 ≈ 30%"——CI 停在 `forge accept`，没有 `forge build`、`forge deploy`、`forge promote`。项目可以**生成经过治理的代码**，但不能**交付它**。对于试图采纳 ForgeOS 的团队来说，这是一个决策心锚："所以我仍然需要 Jenkins/GitHub Actions 做一半的工作？"

**核心挑战。**
1. **资产模式扩展。** 需要 `asset.Deployment` 新类型，包含：目标环境（staging/production）、部署策略（rolling/canary/direct）、健康检查配置、回滚策略。当前 `asset.go` 是一个约 250 行的文件，数据类型密集——谨慎扩展以避免"上帝资产"反模式。
2. **部署阶段编排。** 当前所有编排逻辑假设"阶段在本地代码库运行"。部署需要远程操作（SSH、kubectl apply、API 调用）。这需要一个新的 `DeployExecutor`，与现有的 `CommandExecutor`/`DryRunExecutor` 同级。
3. **安全边界。** 部署凭据（SSH 密钥、K8s 令牌）必须安全存储，绝不进入 agent prompt。需要一个 `credential.Broker`（可能沿用当前 `.forge/` 目录的基于文件的 IPC 模式）。

**预期架构变更。**
- 新类型：`asset.DeployTarget`、`asset.DeploymentStrategy`、`asset.RollbackPlan`
- 新执行器：`internal/executor/deploy.go`
- 新 CLI 命令：`forge deploy`（可能最初作为 `forge run deploy` 的子命令）
- 新工作流：`deploy.yml`（声明部署阶段 DAG）
- 复用 `internal/risk.Classify` → 部署策略映射（如评估文档建议）

**对现有系统的影响。** 风险较高——引入远程执行语义和凭据管理。但仅通过 `forge run deploy` 可达——不会影响现有的 `build`/`evolve` 循环。建议将 `forge deploy` 标记为实验性，直到在真实远程目标上经过验证。

**风险与缓解。** 部署凭据管理是最大的风险。缓解：在项目初始使用 `.env` 文件（不提交到 git）或 `~/.forge/credentials.yml`（对称于当前的 `.forge/` 模式）。凭据绝不进入 `memoryContext` 或 agent prompt——在与 `CommandExecutor` 相同的级别拼接环境变量。

**选项与权衡。**
- **A. GitOps 模式。** `forge deploy` 推送 git tag + CI 触发 Kubernetes/GitOps Pipeline，而不是 ForgeOS 直接连接目标。更安全，但失去了端到端自治的承诺。
- **B. 直接执行模式。** ForgeOS SSH/API 直连目标，在 `forge run deploy` 的输出中显示日志。更直观，但需要凭据管理。
- **C. 混合模式。** `forge deploy` 产生部署清单（Helm chart、K8s manifest），作为 CI 管道的输入，而非自行应用。

**建议：C → B。** 先从"生成部署制品"开始——风险最低，交付价值（消除 Bridge）。当团队接受后，增量加入直接推送能力。

---

### 方向 4 — 工作流组合框架（P1-P2）

**为什么需要。** 当前 5 个工作流是硬编码的，编写为静态 YAML。要使 ForgeOS 从"5 种姿势的框架"变为"团队可表达自己流程的框架"，需要组合性。

**核心挑战。**
1. **解析 -> 验证 -> 执行。** 当前 YAML 经 Python shim (`harness/yaml2json.py`) 转为 JSON——这是评估文档提到的"临时脚手架"。组合框架首先需要用 Go 原生解析器（`internal/yaml2json` 已存在）替换这个 shim，然后构建组合层。
2. **`include:` 语义。** 复合工作流的 v1 能力。一个 `deploy.yml` 工作流可以 `include: build.yml` 然后追加其自己的阶段。需要将包含的工作流阶段解压到主 DAG，保留 deponds_on 约束，正确处理 `on_fail` loop-back 目标——它们在新上下文中可能无法按原样解析。
3. **没有 `extends:`。** 继承会引入菱形依赖问题（如果 build.yml 被两个工作流包含，`on_fail.loop_back` 应指向哪个上下文？）。评估文档正确建议先只做 `include:`。

**预期架构变更。**
- 替换 Python shim 为 Go 原生 YAML 解析（`internal/yaml2json` → 用于生产的 `go-yaml` 或在 `internal/yaml2json` 中自建最小解析器）。
- 新类型：`asset.WorkflowRef`（指向另一个工作流路径的引用类型）。
- 新解析步骤：`internal/workflow/compile.go`：将 include 展开为展平 DAG。
- 新验证：在 `forge validate` 中添加 `check_workflow_includes`。

**对现有系统的影响。** 返回代兼容的关键在于：展开组合发生在运行时之前。`orchestrator.RunFrom(wf)` 保持相同的接口——它接收已组合的 `asset.Workflow`。当前 5 个工作流（零 includes）行为不变。

**选项与权衡。**
- **仅 `include:`**（评估文档建议 + 我们的）。最小行为变化。菱形问题不存在（每个人都包括相同文件的两个副本，但 DAG 是展开的——实际上两个副本，不共享状态）。最适合"标准步骤 + 定制胶水"模式。
- **`include` + `override:`** 允许包含的工作流覆盖特定阶段的 `model_tier`、`required_gates` 或 `on_fail`。更丰富但显著增加了测试表面积。
- **`extends:`**（v2）。具有名字规范化的真正继承。需要钻石消歧策略。

**建议：先替换 Python shim，再 `include:`。** 这是评估文档提到的"A 在 B 之前"依赖。

---

### 方向 5 — Agent 输出质量遥测（P2）

**为什么需要。** 评估文档对此有很好的记录。`trace.Event` 没有 token 字段，`cost.go` 丢弃了 Claude JSON 输出中存在的 `input_tokens`/`output_tokens`。没有 token 效率度量，无法衡量方向 1 的 ROI（节省了多少 token）。

**核心挑战。** 不是实现——`parseClaudeTokenUsage` 需要约 30 行代码，与现有的 `parseClaudeCostUsd` 对称。核心挑战在于**指标设计**：
- **Token 效率**（completion_tokens / prompt_tokens）：低比率意味着"需要大量 prompt 才能产生少量输出"——方向 1 改进的信号。
- **成本效率**（cost_usd / completion_tokens）：昂贵模型的 token 是否更有效？
- **重试率**（retries / total_runs）：哪些 phase/agent 最常触发重试？这可能是 prompt 质量或 memory 质量的信号。

**预期架构变更。** 集中在 `internal/trace` 和 `cost.go`：
- `trace.Event` 添加 `PromptTokens`+`CompletionTokens`（可选字段，向下兼容）。
- `cost.go` `parseClaudeCostUsd` 拓宽为 `parseClaudeUsage` 以提取 `usage` 块。
- `internal/trace/aggregate.go`（新文件）添加统计聚合器：均值、p50、p95、总 token 数。

**对现有系统的影响。** 接近零。新字段可选——现有消费者检查字段存在且非零；不存在则跳过。整个方向可在 1-2 天内交付。

**方向 5.A（token 级效率）应与方向 1 并行进行**——正如评估文档所论证的，这是约 30 行代码，产生的数据直接量化方向 1 的效果。方向 5.B-C（响应时间、按 phase 的质量分）是后续 sprint 的锦上添花。

---

## 3. 接口设计建议

### 3.1 模块边界契约

当前架构有一项隐含但未被编码的资产：**agent 卡中的机读契约**。`product-manager.md` 声明 `CONFIDENCE: <N>`，`reviewer.md` 声明 `VERDICT: APPROVE`/`REQUEST_CHANGES`，`cto.md` 声明 5 路 `VERDICT:`——这三条通过三条 fallback 路径（`parseReviewerVerdict` → `parseExecutiveVerdict` → `parseConfidenceScore`）被 `cost.go` 消费。架构上，这是一种**基于约定的接口**——契约未从提示文本主体中结构化分离，而是嵌入在 markdown 描述中由正则表达式提取。

**建议：提升为显式接口定义。** 我建议在 agent 卡 YAML frontmatter 中引入一个新键 `contract:`：

```yaml
contract:
  mode: verdict  # | confidence | score
  tokens:
    - APPROVE
    - REQUEST_CHANGES
  fallback: relaxed  # | strict
```

这能锁定当前隐含的行为，阻止漂移，并为新 agent 提供作者指导。`cost.go` 中现有的三条 fallback 解析器保持原样——`contract:` 仅作为验证数据（由 `check.py` 消费，类似于 `check_workflow_agent_refs`），不改变运行时行为。

### 3.2 内存检索接口

`internal/memory` 包当前导出两个检索入口：
- `Query(kind, topic)`（`memory.go:293`）——精确过滤
- `Load(path)`（`memory.go:68`）——全量转储

中间缺失的是 `QueryFiltered`——一个结合角色预过滤和 BM25 后处理的调用。我建议一个单一的高级函数：

```go
func RetrieveForPhase(ctx context.Context, store []Entry, role string, phaseType string, cap int) []Entry
```

它在内部调用 `Query(kind=phaseType)` → `boundMemory(cap)`。`ctx` 参数是预留的——v1 不使用，但为之后的超时或取消信号准备。`cap` 控制注入预算——与 `memoryCap` 的 "32" 常数分离。

这不会取代 `Query` 或 `Load`——它位于之上。三个函数的正交性保持了包的低认知负载，同时完成了方向 1 的用例。

### 3.3 部署抽象

部署需要一个新的执行器接口，与现有的 `CommandExecutor`/`DryRunExecutor` 同级。现有的执行器层级是隐式的（接口 `Execute`），定义在 `engine_build.go` 中。部署将通过扩展 `asset.Phase.Executor` 字段来适配，或通过一个新的 `phase.Kind: "deploy"` 值来路由到不同的调度路径。

**建议：不要引入新的子命令树。** `forge run deploy` 读取 `deploy.yml` 工作流并实例化一个 `DeployExecutor`，而不是 `CommandExecutor`。这使得部署路径在运行时是可发现的（`forge run --list-workflows`），不需要新的 CLI 入口点。

---

## 4. 技术选型

### 4.1 YAML 解析：保持零依赖路线

当前 `internal/yaml2json` 是一个自建的 YAML 解析器，作为 Python shim 的替换。它已经用于工作流消费。**我支持将其推向生产就绪，而不是引入 `go-yaml`。** 理由：
- `yaml2json` 已经为 5 个工作流所消费，并具有差分测试（`TestToJSON_MatchesPythonShim`），Sprint 27 修复了 block-scalar 损坏测试。
- 引入 `go-yaml`（或 `goccy/go-yaml`）将是 ForgeOS 的第一个外部依赖——打破了当前 `go.mod` 零 `require` 的不变量。没有充分的理由应该保留这个不变量。它是一个习惯约束，保护了代码库的移植性和构建可重复性。
- `yaml2json` 只需要解析 project.yml 和工作流 YAML——有限子集。它不需要支持完整的 YAML 1.2 规范。

**建议：投资于 `internal/yaml2json`，使其覆盖工作流消费的完全子集，** 而不是引入外部 YAML 库。记录其限制——如果 future 版本需要完整的 YAML 1.2（例如，对于策略定义），此时再引入库。

### 4.2 部署执行器：没有新外部依赖

部署执行器应仅使用标准库的 `net/http`、`os/exec`（用于 SSH 子进程）和 `crypto/tls`。对于 v1 无 Kubernetes 原生支持——`forge deploy` 产生 Helm manifest 或 K8s YAML，通过目标环境中的已有管道应用。

**建议：零外部依赖，与 forge-core 的其余部分保持一致。** 如果团队需要 Kubernetes 原生支持，可以设计一个可插拔的 `DeployBackend` 接口——但不在 v1 实现它。

### 4.3 向量 / 语义检索：不在 v1 引入

方向 2（学习闭环）和方向 1（知识反哺）都可能从语义检索中受益。评估文档正确警告了过早引入 embedding。

**建议：坚持基于关键词的 BM25 至少两个更多 sprint。** 理由：
- BM25 在 ~35k LOC 代码库上表现良好（方向 1 的影响有限）。
- 学习闭环 v1 使用基于关键词的 Jaccard 相似度来衡量记忆归因——与 BM25 相同的词袋方法，不需要 embedding。
- 当引入 embedding 时，按当时的设计调用外部模型（例如 via `os/exec` 调用 Python 脚本），而不是引入 Go 向量库——保持零外部依赖的不变量。

### 4.4 测试基础设施：补基准测试

当前只有一个短板：**性能基准测试缺失。** 既然方向 1 旨在节省 token，我们需要测量"之前的 token 使用"和"之后的 token 使用"。

**建议：`testing.B` 用于：**
- `boundMemory` 的 BM25 检索（不同大小的 corpus）
- `yaml2json` 解析（5 个工作流文件）
- `memory.Load`/`memory.Store` I/O（不测量实时 agent 会话——依赖变量太多）

方向 4（遥测）将提供真实 agent 会话的 token 计数，但基准测试是开发期间需要的早期信号。

---

## 5. 实施路线图

### 总体优先策略

我赞同评估文档的调整后的定序：

```
sprint N:     方向 4.A（token 计数）+ 方向 1（知识反哺）
sprint N+1:   方向 1 完结 + 方向 2（学习闭环 v1）
sprint N+2:   Python shim 替换 + 方向 4 完结（完整遥测）
sprint N+3:  方向 3（组合框架 v1: include:）
sprint N+4:  方向 5（调试性）+ 方向 3 完结
sprint N+5+: 方向 2（部署流水线）
```

### P0（sprint N）——方向 4.A + 方向 1

- 将 `memoryContext` 从 `Load→boundMemory` 改为 `Load→Query(phaseRole)→boundMemory`
- 添加 `parseClaudeTokenUsage`（+30 LOC）
- 将 `trace.Event.PromptTokens`/`CompletionTokens` 作为可选字段添加
- 验证：`forge run` 的输出现在包含 token 计数；memory 注入可见地更贴合阶段角色

**风险**：低。方向 4.A 是纯新增（不影响现有路径）。方向 1 是重构——如果 `Query` 返回空结果（角色过滤过严），`boundMemory` 接收空输入并返回空。**缓解：** Query 的 fallback 是在无结果时跳过过滤，返回所有内容。

### P0（sprint N+1）——方向 2 学习闭环 v1

- 新 `internal/learning` 包
- `Attribution` 逻辑：哪些记忆条目在 agent 输出中被引用（基于关键词匹配）
- `ConfidenceUpdater`：根据归因匹配调整置信度
- 注入 `converge.Signals.LearningGaps` 作为新收敛信号（可选，不破坏现有停止条件）

**风险**：中等——新包、新逻辑。**缓解：** 完全通过单测覆盖——不需要与 real agent 集成。v1 应产生"学习差距"报告，而不改变 agent 行为。

### P1（sprint N+2）——基础设施替换 + 遥测完结

- 用 `internal/yaml2json` 替换 `harness/yaml2json.py` shim
- 方向 4.B-C：反应时间、按 phase 的质量分（源自 `trace.Event` 的现有 `DurationMs`/`CostUsdMicros`）
- 输出：工作流组合的准备条件；真实遥测聚合为 scorecard 元数据

**风险**：中等。Python shim 替换影响构建基础设施（`forge run` 不再需要 Python）。**缓解：** 保留 shim 作为 fallback（如果 Go 解析器 panic，shell 回到 Python），持续一个 sprint，然后提取。

### P1-P2（sprint N+3-N+4）——工作流组合

- `include:` 组合语法 + 解析
- DAG 展开：将包含的工作流阶段解压到主 DAG
- 交叉验证：包含的工作流的 `loop_back` 目标在 DAG 中解析（不 dangling）
- `forge validate` 扩展以验证包含引用

**风险**：中等。Dangling loop-back 目标是静默危险。**缓解：** 单独的验证步骤（`forge validate --include-resolution`），在进入 `RunFrom` 之前被调用。如果验证失败，整个工作流被拒绝（fail-closed）。

### P2（sprint N+5-N+6）——部署流水线 + 调试性

- `asset.DeployTarget`，`asset.DeployStrategy`，`asset.RollbackPlan`
- `DeployExecutor`（与 `CommandExecutor` 同级）
- `forge run deploy` + `deploy.yml` 工作流
- 方向 5：`forge inspect`（SIGUSR1 状态转储），可选方向 5.B-C（`forge tail`，IPC 干预）

**风险**：最高。部署涉及远程系统、凭据和回滚。**缓解：** 第一阶段是"部署制品生成"（C 方案）——`forge deploy` 生成 manifest，不连接远程。团队可以审查 manifest 并手动应用。第二阶段启用自动推送。凭据管理通过 `~/.forge/credentials.yml`（参考现有 `.forge/` 模式）处理。

### 总结

| 方向 | 优先级 | sprint | LOC 估计 | 主要风险 |
|--------|----------|--------|-----------|-----------|
| 1 · 知识反哺 | P0 | N | ~200 | Query 过滤过严→空注入 |
| 4.A · Token 级遥测 | P0 | N | ~30 | 无——纯新增 |
| 2 · 学习闭环 v1 | P0 | N+1 | ~400 | 新语义→行为变更 |
| Python shim 替换 | P1 | N+2 | ~150 | 工作流解析差异 |
| 4.B-C · 完整遥测 | P1 | N+2 | ~200 | 聚合模式→消费者模式变化 |
| 3 · 工作流组合 | P1 | N+3-N+4 | ~500 | Dangling loop-back 目标 |
| 5 · 调试性 | P2 | N+5 | ~350 | `StateDump()` 并发安全 |
| 2 · 部署流水线 | P2 | N+6+ | ~800 | 凭据管理，远程执行 |

### 风险总表

| 风险 | 方向 | 可能性 | 影响 | 缓解 |
|--------|-----------|------------|--------|------------|
| Query 空结果 | 1 | 低（可测试） | 中（空注入） | Fallback 为空时跳过过滤 |
| 凭据泄漏 | 2（部署） | 低（设计约束） | 高（安全事件） | 凭据通过环境变量注入，通过 IPC 传递，绝不进入 prompt |
| 包含的 DAG 冲突 | 3 | 中（名称空间） | 中（行为不正确） | `forge validate` 预检，fail-closed |
| 向量检索性能 | 1/2 | 低（在 ~500 条数据集上） | 低 | 保持 BM25 至少两个 sprint |
| 用户接受度 | 2（部署） | 高（新的信任模型） | 高（方向被拒绝） | 第一阶段（制品生成）→ 第二阶段（自动推送） |
| memory 元数据漂移 | 1 | 低（如果设计冻结） | 低 | Contract: 机读字段加入 agent 卡 |
