# ForgeOS — 声明式资产与运行时之间的落差

> **第三次扫描**，这次关注**声明与执行之间的裂缝**：
> workflow YAML 宣告了大量意图，但运行时只解释了一部分。
> 哪些字段是"文档注释"？哪些是"未来钩子"？哪些是"无声被忽略"？
>
> 不写代码，只做判断。

---

## 目录

1. [Workflow 资产的"发声"与"失声"](#1-workflow-资产的发声与失声)
2. [端到端工作流测试的缺口](#2-端到端工作流测试的缺口)
3. [状态机的不完整迁移](#3-状态机的不完整迁移)
4. [多语言项目的适配器覆盖冲突](#4-多语言项目的适配器覆盖冲突)
5. [持久化数据的生命周期黑洞](#5-持久化数据的生命周期黑洞)

---

## 1. Workflow 资产的"发声"与失声"

### 1.1 落差全景

将 workflow YAML 中声明的所有 phase 属性与 Go 运行时实际读取/使用的属性做对比：

| YAML 字段 | 在 `asset.Phase` 中 | 运行时消费 | 状态 |
|-----------|-------------------|-----------|------|
| `name` | ✅ Yes | ✅ orchestrator.RunFrom | 完好 |
| `agent` | ✅ Yes | ✅ 路由、prompt 构建 | 完好 |
| `readonly` | ✅ Yes | ❌ 从未被读取 | 装饰性 |
| `required_gates` | ✅ Yes | ✅ orchestrator.runGates | 完好 |
| `model_tier` | ✅ Yes | ✅ orchestrator.PhaseTier | 完好 |
| `feeds_forward` | ✅ Yes | ✅ cmd/forge feed-forward 闭环 | 完好 |
| `depends_on` | ✅ Yes | ⚠️ 仅 parallel 模式 Waves() | 部分 |
| `writes_adr` | ✅ Yes (含 condition) | ⚠️ 仅 narrateADR() 日志 | 部分 |
| `on_fail` | ✅ Yes | ✅ loop-back 驱动 | 完好 |
| `blocking` | ✅ Yes | ❌ 从未被读取 | 装饰性 |
| `description` | ❌ 无字段 | ❌ | 纯文档 |
| `fresh_context` | ❌ 无字段 | ❌ | 纯文档 |
| `confidence_metric` | ❌ 无字段 | ❌ | 纯文档 |
| `emits` | ❌ 无字段 | ❌ | 纯文档 |
| `requires_tools` | ❌ 无字段 | ❌ | 纯文档 |
| `optional_for` | ❌ 无字段 | ❌ | 纯文档 |
| `required_when` | ✅ String | ✅ mode_gating skipByMode | 完好 |

共 **17 个 phase 级 YAML 字段**，**10 个被运行时实际消费**，**5 个在 asset 层面丢失**，
**2 个被解析但仅做日志/装饰**。

### 1.2 `fresh_context: true` — 最重要的无声字段

**声明位置**：`build.yml` reviewer phase、`evolve.yml` review phase

```
- name: reviewer
  agent: reviewer
  fresh_context: true   # 必须独立 fresh-context Agent,不让实现者审自己
```

**运行时现状**：`asset.Phase` 没有 `FreshContext` 字段。该声明完全丢失。

**为什么重要**：`BOOTSTRAP.md` 和 `AGENTS.md` 反复强调 Review 必须由 **fresh-context 独立 Agent**
执行，"不让实现者审自己的代码"。但当前运行时**没有任何机制保证这一点**：
- 工作流只定义了 phase 顺序（reviewer 在 implementer 后）
- 但 phase 之间的上下文继承（memory、gate results、findings）由 prompt_context.go 的 `buildPrompt`
  管理——它会注入 `gates.contextLines()`（前序闸门结果）、`findings.contextLines(p.Name)`（特定场景下）、
  `memoryContext()`（跨 session 记忆）

假设 implementer 的 prompt 包含了它自己的输出、memory 中有它写的笔记，reviewer 的 prompt
也在同一个进程中构建——没有进程隔离、没有独立的 `claude` 调用配置来确保"清零状态"。

**建议**：
- `asset.Phase` 增加 `FreshContext bool` 字段
- `buildPrompt` 中，当 `fresh_context == true` 时：
  - 跳过 `gates.contextLines()`（不让 reviewer 看到前序闸门结果，避免锚定偏见）
  - 跳过 `findings.contextLines()`（不让 reviewer 看到自己上一次的发现）
  - 跳过 `phaseOut.contextLines()`（不让 reviewer 看到 planner 的输出）
  - 保留 `constraints`（AGENTS.md 红线）和 `memory`（但只保留 highest-confidence 条目）
- 或者更严格：fresh_context phase 使用一个**新的独立的 `CommandExecutor` 实例**，
  其 `Build` 函数不引用 `buildPrompt` 中那些 feed-forward 部分。

### 1.3 `emits` — 工作流产物的隐式合同

**声明位置**：几乎所有 phase：

```yaml
# discover.yml requirement-discovery
emits:
  - requirement-draft.md

# design.yml proposal-generator
emits:
  - proposal.md

# build.yml planner
emits:
  - task-plan.md
```

**运行时现状**：`asset.Phase` 没有 `Emits []string` 字段。完全丢失。

**为什么重要**：emits 定义了 phase 之间的**隐式数据依赖**。当前这些依赖靠**人类阅读 YAML 注释**
来理解——没有机器可读的消费链路。例如：

- `planner` emits `task-plan.md`
- `implementer` 需要读取 `task-plan.md`
- `reviewer` 需要读取 `task-plan.md` 来审查

但**这些读取从未发生**。prompt 中注入的是 `feeds_forward`（planner 的输出文本被截断后注入），
不是 emits 文件的真实内容。如果 planner 写了 `task-plan.md`，但 implementer 的 prompt 没有
包含它的内容，implementer 就会在不知道完整任务拆分的情况下工作。

**建议**：
- `asset.Phase` 增加 `Emits []string` 字段
- `prompt.Gather()` 增加一个"前序 phase emits"的解析：在执行当前 phase 之前，读取所有
  前序 phase 声明的 emits 文件，将其内容注入到 prompt 中（而不是 feeds_forward 的截断拼接）
- 在 build prompt 中，feeds_forward 和 emits 是两条不同的上下文通道：feeds_forward 是
  实时输出（仅最后一个执行的 phase 的输出），emits 是持久化产物（可能跨 iteration 存在）

### 1.4 `readonly` / `blocking` — 声明的意图但无执行

`readonly: true` 在几乎所有 phase（除了 implementer）上都声明了，说明该 phase **不应该写代码**。
但当前运行时没有做任何写保护——一个配置为 `readonly: true` 的 phase，如果 agent 决定写文件，
没有机制阻止它。

**建议**：`readonly` 可以影响 `--allowedTools` 白名单和 `--permission-mode`：
- `readonly: true` → 不加 `--permission-mode acceptEdits`，不加 `--allowedTools`
- `readonly: false` → 加 `--permission-mode acceptEdits`，加 `--allowedTools` 白名单

### 1.5 `description` — 被浪费的语义信息

每个 phase 都有 2-5 行的中英文 `description`，但运行时完全忽略它们。这些描述比 agent card
更具体地说**这个 phase 在当前 workflow 中应该做什么**——包含了 agent 需要遵循的 workflow
级上下文（planner 需要做任务拆分、reviewer 需要判断阈值违规等）。

**建议**：在 `prompt.Build()` 中，将 phase 的 `description` 注入到 prompt 头部，在角色卡之前：

```
You are the "planner" agent in ForgeOS (phase=planner, mode=engineering, tier=sonnet).

## Current phase description
Break the next roadmap item into smallest acceptance-checkable tasks + acceptance criteria.

## Role card
[agent card content...]
```

---

## 2. 端到端工作流测试的缺口

### 2.1 没有"全 workflow 集成测试"

**现状**：
- `forge-core/internal/orchestrator/` 有大量的 orchestrator 单元测试和模式门控测试
- `harness/test_*.mjs` 测试各 gate 组件
- `examples/url-shortener` 有一个端到端的"真实应用"版本

**但没有任何测试做以下事情**：
- 加载完整的 `build.yml` 或 `evolve.yml`，通过 DryRunExecutor 跑完整 workflow
- 验证 phase 顺序 + gate filtering + convergence 的完整编排
- 用一个伪造的 `on_fail: {action: loop_back}` 场景验证 loop-back 状态机
- 验证 `forge evolve --parallel` 下完整 workflow 的依赖波排序和执行

**为什么这是一个缺口**：当前测试覆盖的是**组件级**行为。真正的错误往往在**组合**中出现：
- `mode_gating.go` 的 `gatesFor()` + `runGates()` 的交互——gate 列表缩减后 convergence 是否仍然一致
- `RunFrom()` 的 loop-back 状态机 + `checkAgentBudget()` 的计数——loop-back 是否消耗 agent 预算
- `LoopEngine.Run()` 的 staleCount + `reportConvergence()` 的交互——no-progress tripwire 和 converge 谁先触发

**建议**：增加一个 `forge-core/internal/orchestrator/workflow_test.go`，用真实的
YAML（通过 test fixture JSON）运行 DryRunExecutor 完整 workflow，断言 phase 序列、
gate 调用次数、converge 结果。这是"集成测试的脊柱"。

### 2.2 没有 DryRun 端的 scorecard 回写测试

**现状**：`scorecard_wind.go` 的 `windDownScorecards` 执行时，先检查 trace 中是否有
model-stamped cost events——如果没有（dry-run executor 不产生 cost events），则跳过。

因此 **dry-run 模式下 scorecard 永远不会被写入**。这是一个正确的行为（不伪造数据）。
但这也意味着 `scorecard-update.mjs` + `scorecard.mjs` 的 merge/synthesize 逻辑
只能在 `--executor=command` 的真实 claude 运行中被测试，或在 Node 的单测中被测试。
Go 端的 wind-down 代码（`windDownScorecards`、`distinctScorecardPairs`）只在真实 claude
调用后被调用。

**建议**：在 scorecard wind-down 中增加一个 `--force-scorecard` 标志（仅用于测试），
用硬编码的测试事件模拟 trace 中的 cost events，验证 wind-down 逻辑从 trace 中正确
解析 pair 并调用 scorecard-update。

### 2.3 Phase 执行顺序的盲点——mode 过滤后的序列

当前测试验证了 mode gating 独立工作（`orchestrator_modegating_test.go`），但没有验证
mode 过滤后的 phase 序列在 `RunFrom()` 中是否正确执行。

**潜在 bug 示例**：
- explorer 模式跳过 reviewer phase，gate 过滤到 `[lint, test]`
- 但 `runGates` 在 explorer 下只跑 2 个 gate，`skipByMode` 跳过 reviewer
- 剩下 phase 序列：planner → implementer → harness-gates (2 gates) → qa
- 这个序列在 `RunFrom` 中能否正确处理？gate phase 的 `required_gates` 缩减后 `runGates` 行为是否一致？
- convergence 的 `gatesGreen` 是否知道 explorer 下只有 2 个 gate 需要检查还是看 workflow 的完整 6 个 gate？

**建议**：增加 `TestRunFrom_ModeGating_PhaseSkipping` 测试，用实际 mode 策略
（engineering/explorer）跑完完整的 workflow，assert 哪些 phase 被执行了、哪些 gate 被调用了。

---

## 3. 状态机的不完整迁移

### 3.1 `on_rejected` 完全不被消费

**声明**：design.yml 的 `on_rejected: {action: loop_back, target_phase: solution-architect}`

**现状**：`asset.StopCondition` 解析了 `OnRejected` 字段，但**没有任何代码读取它**。

```go
// asset.go
type StopCondition struct {
    Type         string       `json:"type"`
    AllOf        []Criterion  `json:"all_of"`
    OnApproved   OnApproved   `json:"on_approved"`
    OnRejected   *LoopBack    `json:"on_rejected"` // 解析了但从未使用
    HumanApproval string      `json:"human_approval"`
    ...
}
```

如果 human gate 被拒绝，`reportHumanGate` 只打印一条信息，但 **不执行任何 jump-back**。
工作流不会回到 solution-architect 去根据反馈修改方案。用户必须手动重新运行
`forge run design`。

**建议**：`Converge` 在 `humanGate()` 返回 `met=false` 时，应该检查 `stop.OnRejected`
并触发 loop-back 到 `target_phase`——就像 gate `on_fail` 一样。

### 3.2 `on_approved.NextStage` 只做日志，不驱动迁移

**现状**：`reportHumanGate` 读取 `stop.OnApproved.NextStage` 并输出：

```
approved → unlocks build
```

但**没有实际的 stage 迁移发生**。没有状态文件更新、没有 checkpoint 写入、没有自动
调用下一个 `forge run build`。

**建议**：`--approved` 执行时应该自动写入一个跨 stage 的状态迁移标记（例如
`.forge/stage.build.ready`），让用户可以接着运行 `forge run build` 或
`forge evolve build` 而不需要额外的 flag。

### 3.3 从 evolve 中正确退出并进入下一 stage

build.yml 的 `on_met.next_stage: evolve` 声明当 roadmap 100% + gates green 后，
流程应进入 evolve stage。但运行时不会自动 stage 跳转——只是打印一条消息并退出。

**建议**：增加 `forge continue` 命令：读取 checkpoint 中的 `next_stage`，
自动加载下一 stage 的 workflow YAML 并执行。

---

## 4. 多语言项目的适配器覆盖冲突

### 4.1 LANG_BY_EXT 映射的覆盖漏洞

**现状**：`adapters.mjs` 定义了源文件扩展名到适配器语言的映射：

```javascript
const LANG_BY_EXT = new Map([
  ['.go', 'go'],
  ['.mjs', 'typescript'], ['.js', 'typescript'],
  ['.py', 'python'],
  // .ts, .tsx, .cjs — 全映射到 typescript
]);
```

并暴露 `multis/ext -> languages` 检测。但 `harness/acceptance.mjs` 的 `probeLint`/
`probeCoverage` 调用 `loadAdapter` 和 `detectLanguages` 的方式是**为每个检测到的语言
各跑一次 linter**。

**问题**：如果一个项目既有 `.py` 又有 `.mjs`（ForgeOS 自己做 dogfood——harness 用 Node，
示例用 Python，forge-core 用 Go）：

1. `detectLanguages` 返回 `['go', 'python', 'typescript']`
2. `probeLint` 对每种语言跑对应的 linter
3. 如果 `golangci-lint` **不存在**（docker 镜像只有 node+python）→ N/A
4. 如果 `ruff` **不存在**但只有一个 `.py` 文件 → N/A
5. 如果 `eslint` 存在但 `.mjs` 不在正确的目录下 → 可能 FAIL

**真正的边界**：**多语言项目中，一个语言的 linter 失败会导致整体 lint === FAIL，即使其他语言都绿了。**
且每个语言的工具链都可能不在 CI 镜像中 → N/A 扩散 → `gatesGreen` 会因 vacuous guard 而失败
（0 个 proven gate）。

**建议**：Lint gate 改为**按语言加权**：
- 只有该语言有**实际源文件**时才跑对应的 linter
- 如果某语言有 0 个源文件（`walkSource` 返回空），跳过该语言 lint（不是 N/A，是 "no files → skip"）
- 如果某语言有源文件但工具缺失 → N/A，当前行为正确

### 4.2 detect 模式与实际源文件不匹配

language adapter 的 `detect` 字段使用 manifest 文件存在来判断语言：

```yaml
# go.yml
detect: ["go.mod", "*.go"]
# python.yml
detect: ["pyproject.toml", "setup.cfg", "requirements.txt", "*.py"]
```

这意味着：如果一个项目有一个 `requirements.txt`（作为文档或 demo），
即使只有 1 个 `.py` demo 文件，也会激活 `python` adapter 并尝试跑 `ruff`、`pytest` 等。

**建议**：增加一个最小源文件阈值（例如语言检测需要 ≥ 3 个对应的源文件，
或需要一个 manifest + 至少一个源文件）。

---

## 5. 持久化数据的生命周期黑洞

### 5.1 四个持久化文件，零个 GC 策略

| 持久化文件 | 位置 | 增长模式 | 清理机制 |
|-----------|------|---------|---------|
| `trace.jsonl` | `.forge/` | append-ever | ❌ 无 |
| `memory.jsonl` | `.forge/` | append-ever | ❌ 无 |
| `checkpoint.json` | `.forge/` | overwrite | ✅ 自然覆盖 |
| `scorecards.json` | `.agent/routing/` | 追加合并 | ❌ 无 |
| `.forge/<stage>.approved` | `.forge/` | 创建后永久 | ❌ 无 |

**问题**：只有 checkpoint 被自然地覆盖（每次写入覆盖旧文件）。其他文件只增不减。

**trace.jsonl** 在 scorecard wind-down 时被全文件扫描。随着运行次数增加，wind-down 的扫描
成本线性增长（虽然现在很小）。更严重的是：**如果用户在多周内每周运行 50 次 evolve，
文件可能达到 MB 级，风扫秒级**。

**memory.jsonl** 同样增长，但不会被全文件扫描（`memory.Load()` 是每次 buildPrompt 调用时
全量读取解析）。

**scorecards.json** 通过 `merge()` 做"折叠"——相同 (model, task_type) 的条目不断合并。
但文件不会自动裁剪旧的、不活跃的 (model, task_type) 条目（例如 v1 的 claude-only 下
可能永远只有 3 个 model × ~10 task_type = 30 行，所以这不是问题。但 v3 的跨厂商池
会扩展到几十个 model × 几十个 task_type = 几百行，仍然很小）。

### 5.2 cross-session memory 没有"遗忘"机制

`memory.jsonl` 的 `boundMemory` 在注入 prompt 时做了即时过滤（recent + relevant），
但**文件本身永远不清理**。运行了一年、积累了几万条 memory 后，`memory.Load()` 每次
读取几万行 JSON，解析出全部条目，然后 `boundMemory` 丢弃 99.99%。

**建议**：增加 `forge memory prune` 命令：
- 读取全部 entries
- 对每个 `KindGap`：如果对应的 gap 在 ROADMAP 中已经被解决，标记为 "resolved" 或删除
- 对每个 `KindLesson`：如果 lesson 在超过 N 次 iteration 中没有被 Retrieve 召回，删除
- 对 `KindDecision`：如果该决策被后续 ADR 推翻，删除
- 保留最新的 M 条（例如 1000 条）

### 5.3 `.forge/` 目录不可移植

`.forge/` 目录是 forge-core 运行时状态的存放位置，但：
1. **它不在 `.gitignore` 中强制控制**——依赖用户手动 gitignore（目前 `.gitignore` 已包含）
2. **CI 中 ephemeral runner 不会持久化**——`.forge/` 在 CI runner 上不存在 resume 基础
3. **多项目共用一个 home 时**——环境变量 `FORGE_REPO_ROOT` 引导到不同 `.forge/` 目录，
   但同一个用户在同一台机器上跑多个项目时可能出现混淆（如果忘记设置 root）

**建议**：
- `forge init` 时确保 `.gitignore` 有 `.forge/`
- CI 模式增加 `--ephemeral` 标志——clean run，不做 resume
- 增加 `forge status` 命令——显示当前 `.forge/` 的状态（checkpoint iteration/phase、memory 条目数、trace 大小）

---

## 总结：该修复的信号 vs 该接受的现状

| 落差 | 严重性 | 修复成本 | 推荐 |
|------|--------|---------|------|
| `fresh_context` 无声 | **高**：Reviewer 独立性的核心保证未执行 | 中 | **短期**：Phase 加字段 + buildPrompt 分支 |
| `emits` 丢失 | 中：隐式数据依赖不透明 | 低 | 短期：Phase 加字段 + 注入 prompt |
| `on_rejected` 不消费 | 中：human gate 被拒后无自动回退 | 低 | 短期：converge 分支 |
| `description` 浪费 | 低-中：已有信息未利用 | 极低 | 短期：prompt.Build 直接注入 |
| `readonly`/`blocking` 无执法 | 中：写保护靠 agent 自觉 | 低 | 短期：影响 permission-mode |
| `on_approved.next_stage` 仅日志 | 中：无自动 stage 迁移 | 中 | 中期：forge continue |
| 端到端 workflow 集成测试缺失 | **高**：组合行为未被验证 | 中 | **短期**：workflow_test.go |
| 多语言 lint 覆盖冲突 | 低-中：极端场景 | 低 | 短期：文件计数检测 |
| 持久化数据无 GC | 低（现阶段）/ 中（长期）：仅方向性风险 | 低 | 中期：forge memory prune |
| mode 过滤后 phase 序列测试缺失 | 中：潜在的序列执行 bug | 低 | 短期：增加集成测试用例 |

**立即可做的 3 件事**（纯数据模型 + 文本改动，0 架构风险）：

1. `asset.Phase` 增加 `FreshContext bool` + `Emits []string` + `Description string`
   → 不改变任何现有行为，只填充原本丢弃的 YAML 数据（无现有测试会失败）
2. `buildPrompt` 中增加 `p.Description` 注入 → prompt 文本变化但语义增强
3. `on_rejected` 在 `humanGate()` 返回 false 时检查并触发 loop-back
   → ~10 行代码，补上 human gate 状态机的 hole

*分析日期：2026-06-29 | 基于第三次全量源码扫描（声明 vs 运行时落差视角）*
