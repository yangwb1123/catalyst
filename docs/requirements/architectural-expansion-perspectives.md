# ForgeOS — 架构级扩展方向：全局扫描与结构化盲区分析

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全量代码库深度扫描。包括 forge-core（18 Go 包 / 42 cmd 文件 / 77+ 测试）、  
>   harness（39 模块 / 适配器框架）、.agent（12 角色卡 / 9 skill / 5 workflow / 全套治理骨架）、  
>   docs（100+ 分析 & 需求文档 / 4 ADR / 31 个 Sprint 完整演进）、examples 真实 dogfood 应用。  
> **基线**: Sprint 31 全状态收官（FUNCTIONAL_REQUIREMENTS_AUDIT GAP 已收口，5 引擎落地，  
>   真点火 multi-agent 端到端坐实，forge-core 零外部依赖全绿）  
> **纪律**: 不编写任何代码。每条方向附带代码级证据 + 边界情况 + 与已有 55+ 方向的可验证差异化。  
> **日期**: 2026-07-09

---

## 已有分析覆盖全景与本文定位

`docs/` 下已有 **40+ `analysis/*.md` + 11 `requirements/*.md` + 30+ sprint 历史判决 = 80+ 份分析**，
覆盖约 **60+ 独立扩展方向**，包括但不限于：

| 大维度 | 代表文档 | 方向数 |
|--------|---------|--------|
| 功能引擎补齐（路由/编排/记忆/收敛/诊断/SCA） | 多篇 requirements | ~8 |
| 生产就绪（prompt QA / 信号硬化 / 适配器 / 环境验证） | `expansion-production-readiness.md` | ~5 |
| 第三地平线（管线组合/多仓联邦/事件驱动/修正学习） | `expansion-horizon-three.md` | ~5 |
| 执行语义（原子性/幂等/因果一致性/版本演化） | `execution-semantic-gaps.md` | ~5 |
| 系统二阶效应（知识衰减/配置爆炸/自洽性/追踪断裂） | `second-order-architectural-gaps.md` | ~5 |
| 安全/凭据/沙箱 | `genuinely-novel-expansion-directions.md` | ~3 |
| 边界/性能/竞态 | `edgecases-and-perf.md` | ~12 |
| 数据生命周期/trace/memory 治理 | `systemic-expansion-v26.md` | ~3 |
| 并行编排/锁契约/收敛陷阱 | 多篇 analysis | ~6 |
| 路由策略/scorecard 衰减/budget 治理 | 多篇 | ~5 |
| 架构自检/CTO 视角/跨功能需求 | 多篇 | ~8 |

**本文不重复上述任何方向。** 下列 5 个方向在全部已有覆盖中**零覆盖或仅边缘提及**，
且均满足：有可验证代码级证据 · 在当前架构下可实现 · 与已落地功能正交互补 ·
解决「系统存在但未被识别」的结构性缺口，而非「已知但推迟」的特性。

---

## 方向一：相位间文件系统隔离（Inter-Phase Sandboxing）

### 优先级
**P1** — 对安全性和确定性至关重要。未隔离是随时间增长的累积风险。

### 现状

当前 ForgeOS **所有 agent phase 共享同一工作目录（`o.root`）的完全文件系统访问权限**。
代码链：

```go
// cmd/forge/engine_build.go — agentExecutor 构建路径
func agentExecutor(o runOpts, root string) (orchestrator.AgentExecutor, error) {
    // ...
    return &orchestrator.CommandExecutor{
        Cmd:    o.agentCmd,
        Args:   append(args, promptText),
        Dir:    root,  // ★ 每个 phase 都拿到 repo root 的完全访问
        Timeout: o.timeout,
        // ...
    }, nil
}
```

```go
// internal/orchestrator/command_executor.go — CommandExecutor 结构
type CommandExecutor struct {
    Cmd                 string
    Args                []string
    Dir                 string   // ★ 硬编码为项目根目录
    Timeout             time.Duration
    // ...
}
```

这意味着：

- `implementer` phase 可以读取 `docs/adr/`（architect 的产物）—— 虽然当前靠 prompt 纪律
  禁止，但没有任何**技术强制**阻止 agent 这么做。
- `reviewer` phase 可以修改 `src/`（不是它的职责）—— `readonly` 控制只影响 claude
  `--permission-mode`，不限制 Bash 命令的写能力。
- 一个受 prompt injection 影响的 agent 可以 `git reset --hard`、删除整个 `.agent/`、
  甚至注入恶意代码到其他 phase 将要消费的文件中。

更隐蔽的问题：**phase 间数据污染**。当前 `feeds_forward` 机制（Sprint 26）靠
`phaseOutputLedger` 将前序 phase 的输出选择性注入后续 phase 的 prompt —— 但前序 phase
的**副作用**（修改了文件、创建了临时文件、改变了 git 状态）没有被隔离。

### 为什么需要

1. **安全性**：prompt injection 是 LLM 应用的已知攻击面。一个被污染的 implementer 可以
   破坏 reviewer 的评审基线、篡改 QA 要测试的代码、注入后门。目前零防御。
2. **确定性**：相位 B 的结果不应依赖相位 A 的**副作用**，只应依赖相位 A 的**声明产物**
   （由 workflow `feeds_forward` 显式传递）。没有文件系统隔离，副作用是隐式的、不可审计的。
3. **并行安全**：`RunParallel` 并发执行同一 wave 内多个 phase（Sprint 27）。如果两个
   implementer 同时写 `src/util.ts`，最后一个写入者获胜，结果不确定。
4. **审计**：无法回答「这个文件是哪个 phase 写的？」——所有 phase 以同一 uid 写同一目录。

### 建议的架构边界

```
internal/workspace/              # 相位工作区隔离引擎
  snapshot.go       # git worktree 或目录快照（硬链接/cp --reflink）
  isolate.go        # 相位执行前创建隔离副本
  merge.go          # 相位完成后仅合入声明产物（emits: 列表）
  clean.go          # 隔离区回收

扩展点：
  - asset.Phase 加 `Sandbox: "readonly"|"isolated"|"shared"` 声明（默认 shared=当前行为）
  - RunFrom/RunParallel 在每 phase 前按声明创建隔离区
  - 每个 phase 的 Dir 指向隔离副本而非 root
  - phase 完成时，仅 workflow `emits:` 中声明的路径被合并回真实工作树
```

### 边界条件与诚实标注

- **性能开销**：`git worktree add` 或 `cp --reflink` 对大仓库（10 万+ 文件）有显著延迟。
  隔离区应是按需 lazy 创建，而非相位启动时全量 clone。`readonly` 模式只需 `chroot`-style
  视图或 bind mount，零拷贝。
- **git 状态隔离**：隔离区内 `git` 不能看到 `refs/heads/main` 或其他分支的最新提交。
  需要 `git stash` / `git worktree` 的正确语义。
- **`emits:` 声明是信任基础**：如果 `emits: [docs/design/]` 但 agent 写了 `src/payment.go`，
  该文件不会被合并回主树（fail-safe：不声明就不合入）。这同时约束了 agent 的行为、
  又提供了对抗 prompt injection 的最后防线。
- **向后兼容**：默认 `Sandbox: "shared"`（零行为变化）。`"isolated"` 是 opt-in，需要
  workflow 作者显式声明 emits + 测试验证。
- **与 `readonly` 的关系**：`readonly` 控制 agent 的写能力（`--permission-mode` / `--allowedTools`），
  sandbox 控制文件系统**可见性**。两者正交互补：`readonly + isolated` 意味着 agent
  既不能写、也看不到其他 phase 的产物；`readwrite + isolated` 意味着 agent 可以写但
  只能影响自己的隔离区。

### 差异化证明

已有分析中：
- `genuinely-novel-expansion-directions.md` 方向二「Multi-Instance Workspace Isolation」
  关心的是**两个独立 `forge run` 实例**之间的工作区隔离。本文关心的是**同一 run 内不同 phase**
  之间的隔离。这是完全不同的隔离边界和 trust model。
- `readonly` 技术强制（Sprint 31）只影响 claude argv，不影响 Bash-driven 的破坏。
- 相位间文件系统污染仅在 `high-value-extension-directions-v2.md` 中作为一个子段落
  「多版本并发冲突」被提及，但未深入讨论隔离机制。

---

## 方向二：结构化 Agent 输出契约（Declarative Agent Output Schema）

### 优先级
**P0** — 当前 Heuristic 模式是隐式耦合的源头，随 agent 数量增长会指数级恶化。

### 现状

ForgeOS 通过 `agent card` 的「机读契约」从 agent 自由文本输出中提取结构化信号。
当前存在 **5 个互相独立的解析器**，全部使用末行文本匹配：

```go
// cmd/forge/cost.go — 三路 fallback 解析器

// 第 1 路：二元 reviewer 契约（build.yml reviewer phase）
func parseReviewerVerdict(output string) string {
    lines := strings.Split(strings.TrimSpace(output), "\n")
    if len(lines) == 0 {
        return ""
    }
    last := strings.TrimSpace(lines[len(lines)-1])  // ★ 只信任最后一行
    // 精确匹配 "VERDICT: APPROVE" / "VERDICT: REQUEST_CHANGES"
    // ...
}

// 第 2 路：五择一 CTO 契约（review.yml executive-review phase）
func parseExecutiveVerdict(output string) string {
    // 同样末行精确匹配，5 个 token
    // ...
}

// 第 3 路：confidence 契约（discover.yml product-manager phase）
func parseConfidenceScore(output string) float64 {
    // 同样末行匹配 "CONFIDENCE: <0-100>"
    // ...
}
```

加上 `cost.go` 中可能还有更多松散模式的 agent output 读取：

```go
// 从 claude --output-format json 解析计费信息
func parseClaudeCostOutput(output string) (micros int64, ok bool) {
    // 正则匹配 "total_cost_usd": <number>
}
```

**所有解析器共享相同的脆弱性**：

| 问题 | 具体表现 | 代码证据 |
|------|---------|---------|
| 只信最后一行 | agent 如果在末尾添加了总结或格式偏移，契约断裂 | `last := lines[len(lines)-1]` |
| 无 schema 校验 | 无法验证 token 是否合法（typo "APROVE" 静默丢失） | 无 `enum` 校验 |
| 无契约版本 | agent card 改了契约格式但旧 agent 仍吐旧格式，无协商机制 | 无版本号字段 |
| 无回溯容忍 | agent 在输出中间也写了 VERDICT:，末行却是自由文本，解析错误 | 只读末行 |
| 无声降级 | 解析失败时返回零值（"" / 0），收敛逻辑**无法区分**「解析失败」与「合法未批准」 | `return ""` |

### 为什么需要

1. **错误信息被静默吞没**——`parseExecutiveVerdict` 如果因为 LLM 格式微调返回 `""`，
   `evalReviewStatus` 将其视为 "no review phase data"（`!= "approved"`），收敛永不 MET。
   但没有任何日志或错误提示告诉 operator「解析失败」。这是一个**隐蔽的静默失效**，
   在 24h 无人值守 evolve 中要到第二天 operator 才发现「昨晚怎么没收敛？」。

2. **contract 扩散**——当前 5 个解析器都是在不同 sprint 中独立添加的（Sprint 28 加
   executive-verdict、Sprint 29 加 confidence-score）。没有中心注册表或 schema 文件，
   每次加新 agent card 的机读契约时开发者需要：
   - 写一个新的 `parse*` 函数
   - 集成到 `observeFor` 的 fallback 链
   - 更新 `gates.go` 的信号提取
   - 为每路写独立测试
   这个模式不可扩展——到 20 个 agent 时将有 20 个 parse 函数。

3. **契约变更无迁移路径**——如果 reviewer card 从 `VERDICT: APPROVE` 改为
   `VERDICT: APPROVED`（语法修正），旧 agent 的存档 trace 里的所有裁决突然
   被静默降级为 "no verdict"。没有向前兼容层。

### 建议的架构边界

```
internal/contract/               # Agent 输出契约定义与校验引擎
  schema.go         # 契约注册表（agent name → OutputSchema）
  parser.go         # 结构化提取器（JSON / YAML fence block 优先，末行 fallback）
  validate.go       # schema 校验（类型 / 枚举 / 范围 / 必选字段）
  version.go        # 契约版本协商（agent 声明 `contract_v: 2`，引擎按版本解析）

每个 agent card 新增一个 `output_schema:` 声明段：
  output_schema:
    format: json               # json | yaml | line_token (向后兼容)
    contract_v: 1
    fields:
      - name: verdict
        type: enum
        values: [APPROVE, REQUEST_CHANGES, REDESIGN, DELAY, REJECT]
        required: true
      - name: confidence
        type: integer
        range: [0, 100]
        required_when: "phase == requirement-discovery"
    parse_strategy: fence_block  # fence_block | last_line | full_text
```

### 边界条件与诚实标注

- **向后兼容**：`format: line_token` 等同于现有末行精确匹配，contract_v 默认为 1，
  缺省 schema 的 agent card 回退到当前启发式行为（零变化）。
- **LLM 并非可靠的结构化输出引擎**——即使要求 `output_schema`，LLM 仍可能输出自由文本。
  `parse_strategy: fence_block` 会在 agent 输出中搜索 \`\`\`json ... \`\`\` 代码块，
  找不到才 fallback 到末行匹配（「最后防线」）。
- **解析失败 ≠ agent 失败**——解析失败应记录 detail（`"reviewer output EOF: no verdict line found"`），
  使 `evalReviewStatus` 的 detail 可解释，而非静默返回 ""。
- **不替换 prompt 构建**——Schema 只影响**消费端**解析。prompt 仍可以自由格式要求 agent
  输出；契约只是让引擎有可靠的方式提取信号。
- **与 `observeFor` 的关系**——`observeFor` 的 3 路 fallback（二元→五择一→confidence）
  应抽象为 `contract.Resolve(agentName, output)`，单路接口 + 统一日志。

### 差异化证明

已有分析中：
- `execution-semantic-gaps.md` 方向一「原子化执行」讨论的是 phase 执行原子性，
  不涉及 agent 输出格式。
- 没有任何已有文档讨论 LLM 输出的结构化契约或 schema 校验。
- `expansion-production-readiness.md` 的「Prompt 编译器质量保证」关注 prompt 输入端的质量，
  不是输出端的解析鲁棒性。

---

## 方向三：工作流声明版本化与自动迁移（Workflow Schema Migration）

### 优先级
**P1** — 从「一次性脚手架」到「长期治理平台」的必经之路。不处理则每个 workflow 格式
变更都会静默分裂被治理项目群。

### 现状

ForgeOS 的工作流声明（`.agent/workflows/*.yml`）是**纯手工维护的无版本 YAML**。
代码链无任何 schema 版本声明或迁移机制：

```go
// internal/asset/asset.go — Workflow 结构，无 SchemaVersion 字段
type Workflow struct {
    Version string       // ★ 这是「工作流自身的版本号」（human label），
                         //   不是「schema 格式版本」
    Stage    string
    Stop     StopCondition
    Phases   []Phase
    // ...
}
```

```yaml
# .agent/workflows/build.yml — 无 schema version 声明
stage: build
stop:
  type: conjunction
  all_of:
    - metric: gates_status
      value: green
phases:
  - name: planner
    # ...
```

**存在的问题**：

1. **字段变更无迁移路径**——Sprint 27 需要在 phase 中加 `requires_tools` 字段。
   旧项目（forge-init 早期产物）的工作流不会自动获得这个字段。它们的 pipeline
   会缺失工具降级保护，但没有任何信号提示这一点。

2. **workflow YAML 结构演进不兼容**——假设 v3 把 `stop.type: conjunction` 改为
   `stop.mode: conjunctive`。所有老项目的工作流会静默解析失败（`LoadWorkflowJSON`
   遇到未知字段时可能忽略或报错），导致 `forge run` 无法启动。

3. **`forge upgrade` 的当前能力**——`harness/scaffold/forge-upgrade.mjs` 只复制
   harness 工具（`gate.mjs` / `check.py` / 适配器），**不升级 workflow 声明**。
   项目的 `.agent/workflows/build.yml` 永远停留在创建时的版本。

4. **跨项目差异不可追踪**——50 个被治理项目可能各自修改了它们的 workflow：
   - 项目 A 加了新的 gate phase
   - 项目 B 删除了安全评审 phase
   - 项目 C 卡在了 Sprint 10 的 workflow 格式
   没有「基线 vs 差异」的 diff 能力，`forge upgrade` 无法做智能合并。

### 为什么需要

1. **平台级治理要求 schema 版本**——Kubernetes 的 `apiVersion` / Terraform 的
   `required_version` 都是先例。没有版本声明，引擎不能区分「旧格式」和「格式错误」。

2. **`forge-init` 的承诺止步于模板创建**——当前 `forge-init` 给每个新项目完整的治理资产
   （`test_forge-init.mjs` 的 `manifest-integrity` 测试保证这一点），但这些资产是**快照**，
   不是**订阅**。治理层的每个结构性更新都会让已部署项目的事实治理状态与最新最佳实践偏离。

3. **基线与覆盖层的合并复杂度随项目数线性增长**——ADR 0003（多仓治理）设计了
   submodule + 双层覆盖。但没有 schema 版本，覆盖层的合并逻辑不知道如何从 v1→v2 迁移。

### 建议的架构边界

```
internal/workflow/               # 工作流版本化与迁移引擎
  schema.go         # 声明式 schema registry (v1→v2→v3 迁移函数)
  migrate.go        # forge workflow-migrate 命令核心逻辑
  diff.go           # 两份 workflow 的结构化 diff（基线 vs 项目覆盖）
  verify.go         # 验证 project.yml 声明的 extends + overrides 不冲突

扩展点：
  - asset.Workflow 加 SchemaVersion string 字段（e.g. "forgeos.workflow.v2"）
  - loadWorkflow 根据 SchemaVersion 路由到对应解析器
  - forge workflow-migrate 新 CLI 子命令：扫描项目 workflow 版本，补迁移
  - forge workflow-diff: 比较项目 workflow 与上游基线的差异
  - CI 检查：项目 workflow 版本低于期望时告警（非阻断，可配置）
```

### 边界条件与诚实标注

- **不发明 K8s 级别的 API 版本**——迁移策略是幂等的「upgrade in place」，不支持
  multiversion serve。`forge workflow-migrate` 一次性将项目升级到目标版本。
- **向后兼容**：缺失 `SchemaVersion` 的 workflow 视为 `v1`（即当前格式），零行为变化。
- **迁移测试**：每种 schema 迁移需要 fixture 覆盖「旧格式→新格式」的转换正确性，
  类似 `checkpoint_reflect_test.go` 的序列化兼容性测试。
- **与多仓治理（ADR 0003）的关系**：workflow 版本化是多仓治理的前提条件。
  没有版本号，submodule 覆盖层无法做智能合入。方向三和 ADR 0003 是顺序依赖关系，
  而非并行选择。
- **`forge upgrade` 的扩展**：当前只升级 harness 工具。应扩展为 `forge upgrade --workflows`
  或整合进同一命令的 `--full` 模式。

### 差异化证明

已有分析中：
- `execution-semantic-gaps.md` 方向五「版本演化」讨论的是 ADR 的版本相关性，
  不是 workflow YAML schema 的版本化。
- `expansion-horizon-three.md` 方向三「Multi-Repo Governance」关注的是多仓库共享治理资产，
  不是 schema 版本化本身。
- `systemic-expansion-v26.md` 方向二「Workflow Compiler」讨论的是 workflow 的静态分析
  和编译优化（dead phase elimination / 依赖图优化），不是 schema 版本化。

---

## 方向四：上下文窗口预算管理（Context Window Budget）

### 优先级
**P1** — 当前无上限的 prompt 增长是 LLM 调用失败的可预测来源。在长运行 evolve 中
必然发生，只是时间问题。

### 现状

ForgeOS 构建 agent prompt 的方式是**累加式拼接**，无总大小预算：

```go
// cmd/forge/prompt_context.go — buildPrompt 的核心逻辑
func buildPrompt(repoRoot string, phase asset.Phase, mode string, sig converge.Signals, ...) string {
    // 第 1 部分：agent card（~2-5 KB）
    card := readCard(repoRoot, phase.Agent)
    
    // 第 2 部分：约束（硬约束注入，~1-3 KB）
    constraints := loadConstraints(repoRoot)
    
    // 第 3 部分：memory 条目（memoryCap=32 硬上限，~3-10 KB）
    memories := memoryContext(repoRoot, ...)
    
    // 第 4 部分：ADR 子弹（adrTopK=5，~2-5 KB）
    bullets := retrieveADRBullets(docs, ...)
    
    // 第 5 部分：gate 结果（gateLedger，~0.5-2 KB）
    gates := formatGateResults(ledger)
    
    // 第 6 部分：feeds_forward 产物（前序 phase 输出，~1-5 KB）
    feed := phaseOutputLedger[phase.Name]
    
    // 第 7 部分：任务注入（ROADMAP.md 勾选项，~1-5 KB）
    tasks := formatRoadmapTasks(...)
    
    // ★ 总大小：10-35 KB（无主动 budget 管理）
}
```

虽然单个 prompt（10-35 KB）远未达到 claude 的上下文窗口（200K），**但在 evolve 循环中
累积效应显著**：

```go
// cmd/forge/prompt_memory.go — memoryContext 随 iteration 增长
// memoryCap=32 是条目数上限，但每条可以很大（多行发现）
// 随着 evolve 推进，memory 条目积累：发现的 gap → 尝试的方案 → 成功/失败记录
// 每个新的 iteration 都注入更多 memory，prompt 大小随迭代次数的平方增长
```

**具体风险路径**：

| 场景 | 迭代 N | 总 prompt 估计 | 风险 |
|------|--------|----------------|------|
| 短 evolve（5 iter） | 5 | ~50 KB | 安全 |
| 中等 evolve（20 iter） | 20 | ~100 KB | 仍在典型窗口内 |
| 长 evolve（50 iter） | 50 | ~200 KB | **接近 claude 有效窗口边缘** |
| 多波 evolve + 大 memory 条目 | 50+ | 可能 >200 KB | **超过有效上下文**，模型开始遗忘开头 |
| 超大 evolve（100 iter） | 100 | >400 KB | **直接越界，API 拒绝调用** |

### 为什么需要

1. **越界是硬故障**——`claude -p` 在 prompt 超过上下文窗口时返回 `prompt_too_long` 错误。
   当前代码没有处理此错误的逻辑（不是 retryable 的 KindTimeout/Overloaded），
   会作为硬错误传播到 `runAgentPhase` → `execEngine` → `forge run` 崩溃。
   在无人值守 evolve 中这意味着 24h 运行在 iteration 47/50 时意外终止。

2. **质量退化是软故障**——即使不越界，prompt 中的关键指令（如 `VERDICT: 格式`）可能被
   推到上下文窗口边缘，LLM 的注意力机制会优先关注 prompt 开头和结尾的 token。
   中间部分的约束被遗忘，导致 agent 行为偏离——但不会报错，operator 无法察觉。

3. **memory 的设计意图是增长，但 prompt 的消费端是无预算的**——`memory` 包设计为
   「只追加不重写」的持久化日志。这是正确的存储设计（accumulating log），但 prompt
   构建端需要用**检索优先于注入**的策略（只注入当前 phase 最相关的 5 条 memory，
   而非全部 50 条）。

4. **`memoryCap=32` 是最后防线，不是预算管理**——它限制注入条数的上界，但不限制
   每条的长度，也不评估注入内容的**信息密度**。如果 32 条全是 verbose 的多行发现，
   prompt 仍然超限。

### 建议的架构边界

```
internal/prompt/budget.go        # 上下文窗口预算管理器

type Budget struct {
    MaxTokens       int      // 目标上下文预算（默认 80K，保 claude 200K 内有余量）
    UsedTokens      int      // 当前累积
    Strategy        string   // "truncate" | "priority" | "retrieve" | "fail"
    // 各组件优先级（0=最高，先注入）
    Priorities      map[string]int
}

核心逻辑：
  - Budget.Reserve(component string, content string) error
    按组件优先级顺序递减预算，超出时按策略处理。
    "truncate": 对超过预算的组件做 token 级截断（尾部优先舍弃）
    "priority": 按优先级从低到高丢弃整个组件块
    "retrieve": 用量化检索替换完整注入（memory + ADR 场景）
    "fail":     硬错误（不静默截断，让上层决定怎么办）
    
  - Budget.Snapshot() → Report
    注入结束后返回各组件实际消耗的 token 数，
    供 trace 记录 + 迭代间审计（这个 iteration 用了多少 context）。
```

**集成点**：
- `buildPrompt` 末尾调用 `budget.Snapshot()` → 注入 trace event
- `observeFor` 可记录「context_window_utilization_pct」到 trace
- `LoopEngine.OnIteration` 可将 budget snapshot 写入 memory
  （让下一 iteration 知道「上一轮的 prompt 用了 85% 窗口」）

### 边界条件与诚实标注

- **token 估算**——不使用第三方 tokenizer（保持零外部依赖），使用保守的字符计数估算
  （4 字符 ≈ 1 token，英文约为 3.5:1，中文约 2:1，取悲观值 3:1）。诚实标注为估算，
  不声称精确 token 计数。
- **向后兼容**——默认 `Strategy: "truncate"`，`MaxTokens: 0` 表示「无预算管理」，
  完全保持当前行为。只有显式设置预算后才生效。
- **与 memory 包的关系**——Budget 不修改 memory 的存储行为（仍 append-only，只增不减）。
  只影响 prompt 构建端的注入策略：`memory.Query` 的热度排序 + budget 限额 = 只注入
  最相关的 K 条。
- **工具链无关**——预算管理不做模型特定的优化（claude 200K / gemini 1M / gpt-4 128K
  的差异是 v3 Router 的责任）。

### 差异化证明

已有分析中：
- `edgecases-and-perf.md` 的 4.1「嵌入式检索序列化瓶颈」讨论的是 BM25 检索的 CPU 成本，
  不是 prompt 总大小管理。
- `high-value-expansion-directions-v2.md` 方向二「Knowledge Engine」讨论的是知识提炼，
  不是 prompt 组装阶段的预算管理。
- 没有任何已有文档讨论「prompt 超过上下文窗口」的故障模式或「上下文预算」的主动管理。

---

## 方向五：跨运行状态可见性与审计回放（Run Audit & Deterministic Replay）

### 优先级
**P2** — 重要但不是阻塞性的。对运营成熟度和事故响应至关重要，但当前无外部消费者。

### 现状

ForgeOS 的每次运行生成三类可审计数据：

```yaml
.forge/trace.jsonl:       # 平面事件流（agent phase start/end, gate results, iteration events）
.forge/memory.jsonl:      # 知识条目（发现、决策、教训）
.forge/checkpoint.json:   # 最后已知状态快照（迭代索引 + roadmap 完成度）
```

**但三类数据的消费场景全部缺失**：

```go
// 场景 1：operator 想知道「昨晚为什么 evolve 没收敛」
// 当前做法：cat .forge/trace.jsonl | grep -i "not met"  ← 纯文本 grep
// 没有 CLI 子命令可以做结构化查询

// 场景 2：事故发生后需要「复现当时的状态」
// trace 记录了当时的行为（做了什么），但不记录「状态基线」
// （git HEAD、ROADMAP.md 内容、project.yml 配置）——无法重建运行场景

// 场景 3：审计员需要验证「agent 是否在授权范围内操作」
// trace 记录了 agent 的输入和输出，但没有记录「agent 的文件系统操作」
// （写了哪些文件、删了哪些文件、执行了哪些 bash 命令）
```

具体代码级缺口：

```go
// internal/trace/trace.go — Event 结构
type Event struct {
    Kind       string // "phase_start" | "phase_end" | "gate" | "iteration"
    Phase      string
    Agent      string
    Status     string
    DurationMs int64
    // ★ 缺失：
    //   - GitHead     string  // 运行时的 git commit
    //   - SnapshotID  string  // 关联到某个可审计的输入基线
    //   - SideEffects []string  // agent 对文件系统的变更列表
    //   - PromptHash  string  // 注入的 prompt 的 hash（供审计比对）
}
```

```go
// internal/persist/checkpoint.go — Checkpoint 结构
type Checkpoint struct {
    FormatVersion string
    Workflow      string
    Mode          string
    Iteration     int
    RoadmapCompletion float64
    PhaseIndex    int
    // ★ 缺失：
    //   - GitHead     string  // checkpoint 时刻的 git commit
    //   - RanAt       int64   // 运行开始时的时间戳
    //   - Hostname    string  // 在哪台机器上运行的
}
```

### 为什么需要

1. **「为什么上次没收敛」是 evolve 运营中最常见的问题**——没有运行时的输入基线
   （git HEAD、ROADMAP 内容 hash、project.yml 配置），operator 无法重建场景。
   trace 中的 wall-clock 时间戳不足以回答「那时代码是什么状态？」

2. **审计合规**——企业环境下需要回答「agent 在被授权范围内操作了吗？」。当前 trace
   记录 agent 的输入和输出，但不记录 agent 实际执行的 bash 命令或文件变更。
   `--agent-allowed-tools` 是执行前约束，但执行后没有审计轨迹。

3. **`forge status` / `forge doctor` 是同步快照**——`cmd/forge/preflight.go` 和
   `internal/doctor/doctor.go` 对「当前状态」做诊断。但 operator 更需要的可能是
   「对比上次运行的状态 vs 当前状态」的 diff。

4. **回放能力 = 调试能力的上限**——没有运行回放，调试只能靠「重现 bug」（重跑一次
   evolve），这既耗时又花钱。如果 trace 记录了足够的状态信息，可以用
   `forge replay --from-trace` 在 dry-run 模式下复现当时的决策路径，
   零 API 成本。

### 建议的架构边界

```
internal/audit/                  # 运行审计与回放引擎
  record.go         # 运行开始时快照状态（git HEAD、config hash、ROADMAP hash）
  query.go          # forge audit 查询命令（按时间/工作流/结果过滤运行记录）
  replay.go         # forge replay 从 trace 重构运行场景（dry-run 模式）
  report.go         # forge audit report 生成运行摘要（谁/何时/结果/成本）

增量 trace 字段：
  - trace.Event.GitHead: 运行时的 git commit
  - trace.Event.SnapshotID: 关联到输入基线的 hash
  - trace.Event.SideEffects: agent 执行后的文件变更摘要
  - Checkpoint 增加 GitHead / RanAt / Hostname

新 CLI 子命令：
  forge audit              # 列出近期运行记录（从 trace 中提取元数据）
  forge audit show <id>    # 显示单次运行的详细审计轨迹
  forge replay <id>        # 在 dry-run 下回放一次历史运行的决策路径
  forge diff --runs <id1> <id2>  # 比较两次运行的状态差异
```

### 边界条件与诚实标注

- **不存储完整 agent 输出**——trace 存储 agent 的 input/output 摘要和 hash，
  但不存储完整输出体（cost 和存储成本太高）。完整输出的归档是 operator 的责任
  （`--trace-output-dir` 可选地保存到外部存储）。
- **回放 fidelity**——`forge replay` 是「dry-run 模拟」，不是「完全确定的复现」。
  LLM 输出的非确定性意味着回放时 agent 会产生不同的响应。回放的价值在于
  **验证治理路径**（gate 选择、mode 决策、loop-back 跳转），而非重新执行 agent。
- **与 checkpoint 的关系**——audit 使用 checkpoint 状态信息，但不依赖 checkpoint
  的存在。一次 `forge run`（非 evolve）没有 checkpoint 但仍可审计。
- **向后兼容**——旧 trace 缺少 GitHead 等新字段，`forge audit` 显示「无数据」而非报错。
- **与 `forge status` / `forge doctor` 的关系**——audit 是历史视角（当时发生了什么），
  status/doctor 是实时视角（现在系统健康吗）。两者互补。

### 差异化证明

已有分析中：
- `systemic-expansion-v26.md` 方向五「Explainability & Transparency」讨论的是
  agent 决策的**可解释性**（为什么 agent 选择了方案 A 而非方案 B），
  不是运维视角的运行审计与回放。
- `execution-semantic-gaps.md` 方向三「因果一致性」讨论的是跨 phase 的因果关系追踪
  （哪个 gap 导致了哪个 implementer 行为），不是运行审计。
- 没有任何已有文档提出 `forge replay` 或 `forge audit` 之类的运维子命令。

---

## 总结：优先级路线图

| 方向 | 优先级 | 类型 | 当前影响 | 外部依赖 | 预估代码量 |
|------|--------|------|---------|---------|-----------|
| **方向二：结构化 Agent 输出契约** | **P0** | 可靠性 | 高：5 个解析器在 24h 运行中静默失效的风险 | 无 | `internal/contract/` 新包 ~300 行 |
| **方向一：相位间文件系统隔离** | **P1** | 安全/架构 | 中-高：prompt injection 面 + 并行竞态风险，随时间增长 | 无 | `internal/workspace/` 新包 ~500 行 |
| **方向三：工作流声明版本化** | **P1** | 平台/治理 | 中：多项目治理资产分叉的累积债务，越晚处理越贵 | 无 | `internal/workflow/` 新包 ~400 行 |
| **方向四：上下文窗口预算管理** | **P1** | 可靠性/性能 | 中：长 evolve 的确定性故障，当前未触发但必有 first hit | 无 | `internal/prompt/budget.go` ~200 行 |
| **方向五：跨运行审计与回放** | **P2** | 运营/可观测 | 低-中：非阻塞但对事故调查和合规至关重要 | 无 | `internal/audit/` 新包 ~600 行 |

## 跨领域边界情况（方向无关）

1. **多版本 forge-core 二进制运行同一仓库**——不同版本的 forge-core 可能写入不同
   schema 版本的 checkpoint / trace / memory。当前无版本兼容性检查。
   方向三 + 方向五的 format version 字段可以解决，但需要执行前检测。

2. **`forge run --parallel` + phase 隔离的组合语义**——并行 wave 内每个 phase
   都在独立的隔离区执行。但如果两个 phase 需要写同一个 `emits:` 目录？
   需要加锁或目录级分配策略（如 `docs/review/` 按 phase name 分前缀）。

3. **agent card `output_schema` 与 prompt 不一致**——agent card 声明了
   `output_schema: {format: json}` 但 prompt 中忘记要求 agent 输出 JSON。
   契约检查器（方向二）应在 `forge validate --models` 中检测这种不一致。

4. **审计回放的 trace 格式演化**——`forge replay` 需要向前兼容旧格式的 trace。
   trace Event 结构应增加 `EventVersion int` 字段，`replay` 根据版本路由解码器。

5. **budget 缺省值的选择**——上下文预算（方向四）的默认行为是否应该与当前
  「无约束」不同？建议默认 `MaxTokens: 0`（无预算管理 = 零行为变化）。
  仅在 `policies.yml` 或 CLI flag 显式设置后才生效。

---

*分析日期：2026-07-09 | 基于 forge-core 全量源码扫描 + .agent 完整治理骨架 +
docs/ 80+ 份已有分析文档交叉核对*
