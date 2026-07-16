# ForgeOS — 产品/架构视角的五个前瞻扩展方向

> **角色**: 资深架构师 + 产品经理  
> **日期**: 2026-07-11  
> **方法**:  
> 1. 全局扫描 forge-core（18 Go 包 / 全部 ~35k LOC）、harness（~10.5k LOC）、`.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · policies/modes/routing · 全部 ADR + DECISIONS）、docs/requirements/（~115 篇）+ docs/analysis/（~40 篇）+ FUNCTIONAL_REQUIREMENTS_AUDIT（90+ DONE, 3 BLOCKED-EXTERNAL, ~15 DEFERRED-BY-DESIGN, 14 GAP 已全量收口）。  
> 2. **差异化验证**: 对每个方向的核心理念，在全部已有分析文档中做字符串+语义检索，确认该方向作为独立系统性扩展**从未展开**。  
> 3. **纪律**: 不编写任何代码。每个方向附带代码级证据、产品价值判断、诚实边界。  
> 4. **诚实声明**: 本仓已有 115+ 篇扩展分析文档、31 轮 sprint、一份穷举的功能需求审计——以下方向不是「代码 bug」或「接线遗漏」，而是架构/产品层面的**结构性新前沿**，某些方向需要新代码框架而非修补现有函数。

---

## 核心判断

ForgeOS 已完成从「治理基础设施」到「真点火 multi-agent 闭环」的跃迁。31 轮 sprint 后的代码库基础设施已达到一个转折点：**机制层已成熟——反馈环完整、执法器可靠、编排引擎可运行、安全护栏完整——但系统作为「产品」的使用模型和信任模型尚未跟上其技术能力**。

当前的真实瓶颈不是「更多自动化功能」，而是：

- **信任鸿沟**: 系统能自治写代码，但人如何信任它的输出而不必逐行审查 raw diff？
- **可追溯性断层**: 系统能 24h 无人值守跑完，但事后如何准确理解「发生了什么」？
- **规模盲区**: 系统能管一个仓，但真实企业有几十个仓、几百个开发者——知识如何跨项目流动？
- **适应天花板**: 工作流是静态 YAML，但真实开发场景需要运行时动态反应（payment 改动→自动注入合规审查相位）。

以下五个方向不修复「bug」或填补「遗漏」，而是**为 ForgeOS 的下一个 S 曲线建立架构基础**。

| # | 方向 | 核心命题 | 产品价值 | 类型 | 覆盖验证 |
|---|------|---------|---------|------|---------|
| 1 | **Shadow-Mode "Propose-Only" Execution** | 让用户信任 AI 自治的第一阶：「先看它会做什么，再让它做」 | 🔴 P1 信任 | 使用模型 | ✅ 零已有分析覆盖 |
| 2 | **Semantic Change Narrative Pipeline** | 24h 自治运行的产出不应该是 raw git diff，而是结构化「变更叙事」 | 🔴 P1 可审计性 | 产品功能 | ✅ 零已有分析覆盖 |
| 3 | **Adaptive Workflow Composition (Runtime Phase Injection)** | 静态 YAML 驱动静态相位序列——真实场景需要根据运行时信号动态注入相位 | 🟠 P2 架构 | 架构演进 | ✅ 零已有分析覆盖 |
| 4 | **Convergence Replay & Forensic Analysis** | trace.jsonl + checkpoint + memory 是原始证据——但没有「事故分析」工具来回答「为什么收敛/失败」 | 🟠 P2 可观测性 | 产品功能 | ✅ 零已有分析覆盖 |
| 5 | **Multi-Instance Knowledge Federation** | 一个 ForgeOS 实例学到的东西，永不传给另一个实例——企业规模化部署的知识孤岛 | 🟡 P3 护城河 | 架构演进 | ✅ 零已有分析覆盖 |

---

## 方向一 · Shadow-Mode "Propose-Only" Execution

> **「先看它会做什么，再让它做」——信任的第一阶**

### 问题诊断

ForgeOS 当前有三种执行模式：

1. **`--executor dry`**: 不调 LLM，只叙事 routing 决策。适合验证机制，但无法展示真 LLM 会产出什么。
2. **`--executor command --agent-cmd claude`（真点火）**: 调真 LLM，agent 直接写磁盘（`acceptEdits`）。只读相位走 `--disallowedTools "Edit Write"` 但产物仍在工作目录中生成。
3. **混合 readonly**: 最近实现的 readonly 强制（Sprint 31）限制了工具，但**没有改变「代码落盘」的事实模型**。

这三者之间存在一个产品级的信任鸿沟：**用户必须从「信任机制」直接跳到「信任 AI 写磁盘」，中间没有一个过渡带**。

代码级证据：

```
# 当前 executor 只有二元选择:
# cmd/forge/engine_build.go:118-126
agentExecutor := &orchestrator.CommandExecutor{
    Build: func(p asset.Phase, mode string) []string {
        return claudeArgv(p, mode, ...)  # ← 直接构造 claude -p 命令
    },
    ...
}
# 没有 "wrap the real agent but capture output instead of applying it"
```

### 建议扩展

引入第三种执行语义 **"shadow"**：

```
--executor shadow  # 或 --propose-only
```

Shadow 模式的契约：

1. **调真实 LLM**（不是 dry-run，agent 获得完整 prompt、完整工具列表）。
2. **工作目录是临时快照**——`git stash` 或 `git worktree add` 或 `tmpfs` 副本，原始工作树**零修改**。
3. **产出是一个 Unified Diff + 结构化变更摘要**，不落盘。
4. **可选的注入点**: shadow 可以作用在单 phase 粒度（只 shadow implementer）、或全 workflow 粒度。
5. **`forge review --diff <shadow-output>`**: 人类审查 diff 后决定 approve（apply diff）或 reject（丢弃）。

```
$ forge run build --executor shadow --agent-cmd claude --root /project
# → 输出一个 unified diff + JSON 变更清单，工作树无变化
# → 用户审查后: forge apply shadow-xxxxx
```

### 产品价值

- **信任梯度**：dry-run→shadow→live，用户逐步建立对系统的信任。
- **安全审查**：在无人值守 evolve 之前，先跑一个 shadow cycle 让人确认「AI 没打算删我数据库」。
- **CI 集成**：CI 可以用 shadow 模式跑一个 evolve 迭代，把 diff 贴在 PR 评论上供审查，approve 后才真正 merge。
- **培训/演示**：无需烧真钱就能展示系统在真实场景下的行为（只需要一次 shadow run 的 trace/diff 作为 demo）。

### 边界与诚实标注

- **不可回避的成本**: shadow 模式仍然调用 LLM，成本几乎等于真跑（少一个 apply 步骤而已）。这不是省钱方案，是信任建设方案。
- **临时副本的保真性**: git worktree 能保证文件系统级保真，但外部依赖（数据库、第三方 API）仍然会被影子 agent 访问。需要诚实提示「network side effects 无法 shadow」。
- **状态依赖**: 如果前一 phase 的产物（如 task-plan.md）被后一 phase 消费，shadow 的临时副本需要维护 phase 间的产物传递——即临时工作目录不是独立的，而是 phase-by-phase 继承的。
- **不适用于所有场景**: human_gate（设计→构建审批）的设计评审本身就不修改代码，shadow 的收益有限。
- **不是 readonly 的替代**: readonly 是「agent 不写代码」的**纪律**（已实现），shadow 是「agent 写了但没落地」的**预览**。两者正交、可组合。

---

## 方向二 · Semantic Change Narrative Pipeline

> **24h 自治运行的产出不应该是 raw git diff——应该是「人用自然语言理解的变更叙事」**

### 问题诊断

当前 ForgeOS 工作流的产出物模型如下：

- **代码变更**: 直接写磁盘（`Edit` tool），hooman 只能通过 `git diff` 看到。
- **Agent 输出**: 注入到 trace.jsonl 的 `detail` 字段（自由文本，未结构化）。
- **Convergence 报告**: `forge run` 结束时打印 `roadmap_completion=XX%, gates_status=green`，极其高层的聚合。
- **Memory 条目**: 结构化的 gap/decision/lesson 条目，但缺少「我具体改了什么」的信息。

这三层之间有一个巨大的**粒度鸿沟**：高层聚合（"roadmap 100%"）和原始 git diff 之间，没有任何「中间粒度」的描述帮助人类审查 24h 的自治产出。

代码级证据：

```
# cmd/forge/gates.go 的 reportConvergence 输出:
# forge run build — convergence report:
#   roadmap_completion=100.0%
#   gates_status=green
#   review_status=approved
# ← 没有「改了什么文件」「改了哪个函数」「新增了什么能力」

# internal/trace/trace.go 的 Event 结构:
type Event struct {
    Kind       string `json:"kind"`
    Name       string `json:"name"`
    Status     string `json:"status"`
    Detail     string `json:"detail,omitempty"`  # ← free text, unstructured
    ...
}
# ← trace 没有结构化的「变更清单」字段
```

### 建议扩展

构建一个**变更叙事管道**，作为 workflow 的第一类产出：

1. **Phase 级变更捕获**: 在每个 agent phase 结束后，计算 git diff（相对于 phase 开始时的基线），用 LLM 摘要生成结构化变更描述。
2. **变更叙事 Schema**:
   ```yaml
   narrative:
     phase: implementer
     files_changed: 3
     summary: "Implement URL shortener redirect handler"
     changes:
       - file: src/handler/redirect.go
         type: added           # added | modified | deleted | refactored
         summary: "New redirect handler with 302 status"
         loc_delta: +45/-0
         risk_area: false      # touched payment/auth/migration?
   ```
3. **Workflow 级聚合叙事**: workflow 结束时，将所有 phase 的叙事合并为一份可读的 changelog：
   ```
   # Build #42 — Changelog
   ## Planner
   - Decomposed ROADMAP item "add redirect" into 3 tasks
   
   ## Implementer
   - Added `src/handler/redirect.go` with 302 response
   - Modified `src/router.go` to register new route
   
   ## Harness Gates
   - lint: PASS · test: PASS (15/15) · security: PASS
   
   ## Reviewer
   - APPROVED: "Clean implementation, no concerns"
   ```
4. **持久化**: 叙事写入 `.forge/narrative/<run-id>.json`，可供 `forge log --run <id>` 读取。

### 产品价值

- **24h 可审计**: 第二天早上，用户读一份 10 行 changelog 就知道昨晚系统做了什么，而不是解析 2000 行 git diff。
- **PR 描述的自动生成**: 叙事管道可以直接输出一份 PR 描述，附带每项改动的 rationale（从 agent 输出中提取）。
- **Human_gate 的上下文**: 在设计→构建审批时，审批者看到的不只是「模式将迁移」的声明，而是「上次批准以来系统实际改了哪些文件」。
- **合规审计追踪**: 叙事附带 gpg 签名后可作为合规凭证：「2026-07-11 03:00 UTC，forge 自动实现了 PCI 合规的日志记录改动」。
- **回滚决策支持**: `forge diff-runs <id-a> <id-b>` 可以对比两个 run 的叙事结构化数据，精确定位哪个改动引入了回归。

### 边界与诚实标注

- **LLM 摘要不可靠**: LLM 生成的变更摘要可能遗漏关键信息或「编造」改变。解决方案：叙事必须附带**可验证的机械证据**（`loc_delta` 是 diff --stat 算的，不是 LLM 说的；`files_changed` 是 git 算的）。LLM 只贡献 `summary` 字段，且需要标记 `generated_by: claude`。
- **性能开销**: 每个 phase 后跑 git diff + LLM 摘要调用会增加墙钟和成本。做成 opt-in（`--narrative`），默认关闭。
- **增量难题**: 在有 loop-back 的场景中（implementer→harness-gates→loop back），前一次 implementer 的 diff 在第二次 implementer 执行后不再有意义。叙事需要 tracking versioned diff 而非 linear diff。

---

## 方向三 · Adaptive Workflow Composition (Runtime Phase Injection)

> **静态 YAML 工作流——真实场景需要运行时动态反应**

### 问题诊断

当前 ForgeOS 的工作流模型是完全**静态**的：

```
# .agent/workflows/build.yml 定义了固定 5 个相位:
phases:
  - planner
  - implementer
  - harness-gates
  - reviewer
  - qa
```

运行时的"适应性"目前只有两种原语：

1. **mode-gating 的跳过**: `optional_for: [balanced]` 跳过特定相位，但这是在 workflow 加载时静态决定的。
2. **on_fail loop-back**: gate 失败后跳回之前相位重试，但只能跳回**已存在的**相位，不能注入新相位。

真实场景需要的适应性远比这丰富：

| 场景 | 当前行为 | 理想行为 |
|------|---------|---------|
| risk 分析发现 payment 代码改动 | 还是跑相同的 gate 集合 | **自动注入 PCI-compliance gate phase** |
| reviewer 连续 3 次 REQUEST_CHANGES | 无限 loop-back 直到 MaxLoopBack 耗尽 | **注入一个 escalation phase: 暂停等待人类裁决** |
| 预算快烧完但 roadmap 还差 20% | BudgetAdjustTier 降模型档次 | **注入一个 cost-optimization phase: 重新规划剩余工作，优先高价值项** |
| implementer 输出包含测试覆盖率下降 | 只有 gate 阶段才测覆盖率 | **自动注入一个 coverage-recovery phase** |

代码级证据：

```
# internal/asset/asset.go 的 Phase 结构——纯数据容器，无生命周期方法:
type Phase struct {
    Name          string   `json:"name"`
    Agent         string   `json:"agent"`
    RequiredGates []string `json:"required_gates"`
    OnFail        *OnFail  `json:"on_fail"`
    ...
}
# ← 没有 PhaseGenerator、没有 DynamicPhase、没有 CompositionRule

# internal/orchestrator/orchestrator.go 的 RunFrom——纯线性遍历:
for i := start; i < len(wf.Phases); i++ {
    p := wf.Phases[i]
    ...
}
# ← 没有「执行前检查是否需要注入新相位」的 hook
```

### 建议扩展

引入**相位注入引擎**——一组声明式规则，定义在什么条件下注入什么相位：

1. **规则声明**（在 workflow YAML 或 policies.yml 中）:
   ```yaml
   phase_injection:
     - trigger:
         signal: risk_level
         operator: ">="
         threshold: high
         # AND/OR 组合
         context: phase_output_matches "payment"
       inject:
         - name: pci-compliance-gate
           after: harness-gates
           agent: security-engineer
           required_gates: [security]
           description: "PCI compliance verification for payment changes"
   ```

2. **运行时相位注册表**: 在 `orchestrator.Engine` 中增加一个 `PhaseRegistry map[string]PhaseGenerator`，注入点位于 `RunFrom` 的主循环中和每个 phase 执行后。

3. **注入生命周期**: 注入的相位在 workflow 运行期间是**瞬时**的（不持久化到 workflow YAML），但会被写入 trace 和 checkpoint，因此是**可审计、可重放**的。

4. **诚实限制**: v1 注入只能*追加*（`after: X`），不能删除现有相位（mode-gating 的 skip 是已有的删除机制）。

### 产品价值

- **超出静态编排的能力边界**: 没有动态注入，ForgeOS 永远只能做「预先编排好的 N 步」，永远无法响应运行时涌现的风险/机会。
- **减少牙签架构**: 当前每个「新场景需要不同相位序列」的诉求都可能导致 workflow YAML 的拷贝/修改（discover.yml / build.yml / review.yml / evolve.yml 已经在这样膨胀了）。动态注入让一份 workflow 可以自适应多个场景。
- **企业合规**: PCI、SOC2、HIPAA 等合规场景需要「当触及敏感模块时自动插入合规验证」，这是硬性需求，不是可选优化。
- **效率**: 一个全量的 build.yml 包含 5 个固定相位，但 80% 的改动只需要 implementer→harness-gates 两步。动态注入可以根据 diff 跳过不必要相位（类似方向一的增量测试选择，但扩展到任意相位）。

### 边界与诚实标注

- **相位注入的「类型安全」**: 注入的相位必须满足依赖约束（它的 `after` 阶段必须已经执行，它的 `requires` 工具必须可用）。需要一个编译时/加载时校验器。
- **与 loop-back 的交互**: 重跑注入前的相位时，注入的相位应该被保留还是重新注入？这是一个复杂的语义问题，v1 可能需要简化规则（保留已注入相位）。
- **与 checkpoint/resume 的交互**: 注入的相位在 resume 时必须被正确重建——当前 Checkpoint.PhaseIndex 是线性索引，注入相位会被静默跳过。需要注入相位 ID 的持久化。
- **永不违背 safety floor**: 注入可以添加相位（更多检查），永远不能跳过已有的 safety floor（Opus-only 的 reviewer 不能被注入逻辑绕过）。这与 mode-gating 的「production 一票否决」原则一致。
- **性能**: 每次 phase 执行后评估注入条件会增加开销。规则应该是廉价的布尔表达式，不调 LLM。

---

## 方向四 · Convergence Replay & Forensic Analysis

> **trace.jsonl + checkpoint + memory 是原始证据——但没有「事故分析工具」**

### 问题诊断

ForgeOS 的所有运行时数据都落盘了：

- `.forge/trace.jsonl` — 迭代级、相位级、gate 级事件
- `.forge/checkpoint.json` — 收敛状态快照（保留历史版本）
- `.forge/memory.jsonl` — 跨迭代知识

但这些数据是**只有机器可读的原始证据**。当前可用的"分析"工具：

```
forge doctor       # 检查文件完整性，不分析内容
forge status       # 显示当前状态快照
forge status -h    # 历史 checkpoint 链
```

当一次 24h evolve 收敛（或失败）后，操作者想知道：

- **为什么在迭代 7 收敛而不是迭代 5？** 是 gate 信号变了还是 roadmap completion 达标了？
- **哪个相位花费了最多的成本？** 是 reviewer（Opus 贵）还是 implementer（重试多）？
- **loop-back 浪费了多少预算？** 多少次 reviewer→implementer 跳转是「有用」的修复 vs 原地打转？
- **如果换一个 mode/lifecycle 会怎样？** trace 数据能否回答「用 balanced 模式能省多少」？
- **这次 evolve 和上次 evolve 的行为差异是什么？** prompt 变了？模型路由变了？gate 通过率变了？

当前这些问题的答案是：**手动 `jq` trace.jsonl**。

### 建议扩展

构建 `forge replay` 和 `forge diff-runs` 工具：

1. **`forge replay <run-id>`**（或 `forge replay --latest`）:
   ```
   $ forge replay --latest
   
   Run #7 (evolve, mode=engineering, lifecycle=production)
   ┌─────────────────────────────────────────────────────────────┐
   │ Timeline (7 iterations, 42 phases, 3 loop-backs)           │
   │                                                             │
   │ iter1 ████████████████████ 8 phases | cost $0.92 | 14m32s  │
   │ iter2 ████████████ 5 phases | cost $0.45 | 8m12s           │
   │ iter3 ██████████████████████████████ 8 phases | cost $1.21 | 16m45s │
   │ ...                                                         │
   │ iter7 ██ 2 phases | cost $0.18 | 3m01s                     │
   │                                                             │
   │ Convergence: MET at iter 7                                  │
   │   roadmap_completion: 100% (was 42% at iter 1)             │
   │   gates_status: green (was green throughout)                │
   │                                                             │
   │ Cost breakdown by agent:                                    │
   │   reviewer (opus): $2.85 (42%)                              │
   │   implementer (sonnet): $2.10 (31%)                         │
   │   planner (sonnet): $0.95 (14%)                             │
   │   qa (sonnet): $0.88 (13%)                                  │
   │                                                             │
   │ Loop-back analysis:                                         │
   │   3 loop-backs, all harness-gates→implementer               │
   │   - iter2-gate: complexity FAIL → implementer fixed naming  │
   │   - iter4-gate: test FAIL → implementer fixed broken test   │
   │   - iter5-review: REQUEST_CHANGES → design improvement      │
   │   0 budge spent on "useless" loop-backs                     │
   └─────────────────────────────────────────────────────────────┘
   ```

2. **`forge diff-runs <id-a> <id-b>`**:
   ```
   $ forge diff-runs 5 7
   
   Comparing run #5 (balanced) vs #7 (engineering)
   ┌─────────────────────────────────────────────────────────────┐
   │ Cost:        #5 $3.20 vs #7 $6.78 (+112%)                   │
   │ Iterations:  #5 12 vs #7 7                                   │
   │ Loop-backs:  #5 8 vs #7 3                                    │
   │ Gate passes: #5 89% vs #7 94%                                │
   │                                                             │
   │ Dominant difference: engineering's additional gates         │
   │ (arch, security, complexity) caught 2 preventable issues    │
   │ that balanced skipped and had to loop-back for later        │
   └─────────────────────────────────────────────────────────────┘
   ```

3. **`forge what-if --run <id> --switch mode=explorer`**:
   ```
   $ forge what-if --run 7 --switch mode=explorer
   
   Simulating mode=explorer on run #7 data:
   - Skipped phases: reviewer (×7 iterations), qa (×7)
   - Estimated cost: $6.78 → $3.95 (-42%)
   - Estimated iterations to converge: 7 → ~12-15 (no reviewer correction)
   - Risk: 2 test failures would NOT have been caught by explorer gates
   ```

### 产品价值

- **可解释的自治**: 没有 replay，24h 自治运行是一个黑箱——它完成了任务但没人知道具体过程。Replay 让自治变得可解释。
- **ROI 决策**: `forge diff-runs` 直接帮助用户回答「engineering 模式多花的成本值不值？」，这是产品定价/选型的核心依据。
- **调试效率**: 当一次 evolve 收敛异常（过早/过晚），replay 能快速定位根因，而不是逐行翻 JSONL。
- **CI 集成**: CI 可以在每次 PR 后跑 `forge replay --since <commit>` 生成「本次 PR 触发的 ForgeOS 行为报告」。

### 边界与诚实标注

- **纯离线分析**: Replay 读取已落盘的 trace/checkpoint/memory 数据，不调 LLM、不改状态。因此它是廉价的（零推理成本）但受限于落盘数据的粒度——如果 trace 没有记录某个决策的「输入」（比如当时注入的 prompt 内容），replay 无法重构它。
- **What-if 是启发式**: `forge what-if` 基于已有 trace 数据做线性/统计推断，不是真实重跑。换 mode 后的实际行为可能完全不同（loop-back 模式改变、agent 行为非线性）。必须诚实标注为「估计值，精度取决于数据量」。
- **trace 格式是 replay 的契约**: replay 工具紧密绑定 trace Event 的 JSON schema（`kind`/`name`/`status`/`duration_ms`/`cost_usd_micros`/`model`）。如果 trace 格式演进，需要版本兼容。
- **安全考虑**: replay 读取 .forge/ 目录，如该目录包含敏感信息（prompt、agent 输出），replay 的输出也需要访问控制。

---

## 方向五 · Multi-Instance Knowledge Federation

> **一个 ForgeOS 实例学到的东西，永不传给另一个实例——企业规模化部署的知识孤岛**

### 问题诊断

ForgeOS 的知识系统（`internal/memory`）是**单实例、单文件系统作用域**的：

```
.forge/memory.jsonl  ← 只在此项目根目录下有意义
  每个条目记录: Kind / Topic / Detail / Iteration / Confidence
  但没有: ProjectID / TeamID / RepoURL / ShareLevel
```

如果一个组织有 50 个微服务仓库，每个仓库都运行 ForgeOS：

- 仓库 A 的 agent 发现「go-taskd 的 `internal/domain` 包频繁出现循环依赖」——这条 lesson 困在仓库 A 的 memory.jsonl 里。
- 仓库 B 正在初始化一个新的 Go 项目——它不知道仓库 A 已经踩过的坑。
- 仓库 C 的 scorecard 显示 Sonnet 在测试生成的 p95 延迟是 12s——仓库 D 在选择模型时看不到这条数据。
- 所有 50 个仓库各自独立地运行 `forge evolve`，每次 scan 阶段都在做重复的行业/竞品分析。

代码级证据：

```
# internal/memory/memory.go Entry 结构——无跨实例字段:
type Entry struct {
    Format        string  `json:"_format,omitempty"`
    Kind          string  `json:"kind"`
    Topic         string  `json:"topic"`
    Detail        string  `json:"detail"`
    Iteration     int     `json:"iteration"`
    Source        string  `json:"source,omitempty"`
    Confidence    float64 `json:"confidence,omitempty"`
    Supersedes    string  `json:"supersedes,omitempty"`
    CreatedAtUnix int64   `json:"created_at_unix"`
}
# ← 没有 Namespace、Origin、SharePolicy

# internal/routing/scorecard.go Scorecard——无跨实例聚合:
type Scorecard struct {
    Model      string  # model name only, no instance/project attribution
    TaskType   string
    Samples    int
    AvgCostUsd float64
    P95Latency int64
}
# ← 没有 ProjectID / TeamID / Pool
```

### 建议扩展

构建一个**轻量级知识联邦协议**——不依赖中央服务器，可 peer-to-peer 运行：

1. **Entry 扩展**: memory 条目增加 `origin`（仓库 URL + 项目名）、`share_level`（local / team / org / public）字段。
   ```go
   type Entry struct {
       // ... 现有字段
       Origin     string `json:"origin,omitempty"`      // "github.com/org/repo"
       Namespace  string `json:"namespace,omitempty"`   // "team-payment"
       ShareLevel string `json:"share_level,omitempty"` // "team" | "org" | "local"
   }
   ```

2. **知识交换格式**: 一组可导出的知识包——`forge knowledge export` 生成 `.forge-knowledge.json`，`forge knowledge import <file>` 选择性合并。

3. **Scorecard 联邦聚合**: `forge scorecard --federate` 收集局域网（或配置的 URL）上多个实例的 scorecard，聚合为跨实例的模型性能视图。

4. **Git-based 知识分发**（零基础设施方案）:
   ```
   # 在组织级别建一个知识仓库
   git init forge-knowledge
   forge knowledge export --since 7d > forge-knowledge/team-a/2026-07-11.json
   cd forge-knowledge && git commit -m "update knowledge from team-a"
   
   # 在其他仓库
   forge knowledge import ../forge-knowledge/team-a/2026-07-11.json
   ```

5. **去重/冲突解决**: 当两个实例对同一个 Topic 提供矛盾的 lesson（一个说「Use SQLite」，一个说「Use PostgreSQL」），需要`Confidence` + `Supersedes` + 时间戳的优先级规则（同现有的 `filterSuperseded` 机制）。

### 产品价值

- **组织级学习**: 50 个仓库不重复踩同一个坑。一个团队发现的「Go 1.26 的 `slices` 包在泛型嵌套下有 bug」能在全组织传播。
- **模型路由全局优化**: 跨实例的 scorecard 联邦让「选择模型」从单实例的几十个样本演进到跨实例的数千个样本——p95 延迟估算从「有噪声」变为「统计显著」。
- **治理资产联邦**: 一个中央团队维护的 `policies.yml` 变体可以推送到全组织，而不是每个仓库手动 `forge-upgrade`。
- **知识即网络效应**: ForgeOS 的价值随实例数超线性增长（每个实例的知识使所有其他实例更聪明）。这是网络效应护城河。

### 边界与诚实标注

- **无自动跨实例同步**: v1 是推拉式（export/import），不是自动同步。自动同步需要后台 daemon 进程+双向冲突解决（是 ARCHITECTURE.md 中的 v3 路线图项）。
- **安全/隐私**: 知识条目可能包含敏感信息（"service X 的 /admin 端点未授权"）。`share_level` 的强制由实例自治（无法技术防止一个实例将标记为 `local` 的知识导出），类似 git commit 的本地签名。正式安全模型需要加密+签名（v2）。
- **数据质量问题**: 一个实例的低质量 agent 产生的噪音知识可能污染其他实例。需要`Confidence` 阈值过滤（`forge knowledge import --min-confidence 0.7`）和人工审核机制。
- **不替代 .agent/ 共享**: `forge-upgrade` 已经分发治理资产（agent 卡 / skill / workflow / gate）。联邦知识是**运行时学习到的经验**，不是**设计时声明的规则**。两者互补，不重叠。
- **存储增长**: 联邦知识的总量可能很大。建议导入时做 `Confidence` + recency 过滤，并支持 `forge knowledge prune --older-than 90d`。

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 | 前置依赖 | 预估投入 |
|------|--------|------|-----------|---------|---------|
| **一 Shadow-Mode** | **P1** 🔴 | 信任模型 | 打开「让用户信任 AI 自治」的大门；是 ForgeOS 从"技术验证"到"产品可用"的关键一役 | `CommandExecutor` 的 output capture + git worktree 管理 | ~2 sprints |
| **二 Semantic Narrative** | **P1** 🔴 | 可审计性 | 让 24h 结果可读；没有它，用户不可能信任无人值守模式 | 方向一的 diff capture 基础设施（可复用） | ~2 sprints |
| **三 Adaptive Workflow** | **P2** 🟠 | 架构 | 打破静态 YAML 天花板，开启「响应式编排」时代 | 坚实的 loop-back + checkpoint 基础已具备 | ~3 sprints |
| **四 Replay & Forensic** | **P2** 🟠 | 可观测性 | 让自治行为可解释；是调试复杂故障的唯一规模化方法 | trace 数据完整性（已就绪）+ checkpoint 历史（已就绪） | ~1-2 sprints |
| **五 Knowledge Federation** | **P3** 🟡 | 护城河 | 组织级学习网络效应；是 ForgeOS 从"单机工具"到"企业平台"的跳板 | memory JSONL schema 扩展 + scorecard 联邦聚合 | ~3-4 sprints |

**收敛建议（若只能做一件）**:

- **方向一（Shadow-Mode）**——产品信任是当前所有方向中最紧迫的。ForgeOS 的机制（真点火、multi-agent、gate 自纠、learning loop）全已坐实，但**没有一个企业会在没有 preview/approve 机制的情况下让 AI 自治写生产代码**。Shadow-Mode 是解锁企业采用的第一把钥匙。

**若做前三件**:

- **一 + 二 + 四**——Shadow-Mode（信任基础）+ Semantic Narrative（可读产出）+ Replay（事后分析）。三者构成一个完整的产品体验闭环：跑前预览 → 跑中产出可读 → 跑后可追溯。这三件不依赖架构级重写，都在现有的 `forge run` 路径上做功能扩展。

**方向三（Adaptive Workflow）和方向五（Knowledge Federation）** 是架构级的前沿，需要跨 sprint 的投入和更谨慎的设计。建议先完成产品体验三件套（一+二+四），再推进架构演化。

---

## 与已有分析的关系

本文件所有 5 个方向经交叉验证（关键词搜索 + 核心论点比对 + 115+ 篇已有文档的逐篇排查），确认**从未作为独立系统性方向展开**。最接近的已有论述见下，每个方向均附区别说明：

| 方向 | 最接近的已有分析 | 差异 |
|------|----------------|------|
| 一 Shadow-Mode | `docs/analysis/fresh-scan-strategic-expansion.md` 和 `docs/analysis/novel-extensions-v12-architect-perspective.md` 曾触发正则匹配，但实际内容零覆盖 | Shadow-mode 主题从未作为独立扩展方向出现 |
| 二 Semantic Narrative | `docs/requirements/expansion-five-uncovered-2026-07-10.md` 提及「Semantic diff between runs」但上下文是 trace 文件对比，非变更叙事生产 | 本方向将变更叙事定义为第一类 workflow 产出，有 schema、持久化路径、CLI 接口 |
| 三 Adaptive Workflow | 无任何已有分析提及「运行时动态相位注入」概念 | 全新架构方向 |
| 四 Replay & Forensic | `docs/requirements/expansion-five-product-blindspots.md` 提及 `forge retrospective` 概念；`docs/requirements/expansion-production-perspectives.md` 提及 post-mortem 视角 | 前者仅提 CLI 命令名称，未定义 replay 引擎 + schema + what-if 模拟；后者是单句提及非系统性方向 |
| 五 Knowledge Federation | 无任何已有分析提及跨实例知识共享协议 | 全新架构方向 |

---

*本文件不声称发现 ForgeOS 的「所有」缺口，只描述全局扫描过程中发现的、在 115+ 篇已有分析中未被作为独立方向展开的 5 个系统性前沿。每个方向附带代码级证据、诚实边界和产品价值判断，供后续 sprint 规划参考。*
