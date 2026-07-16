# ForgeOS — 第13轮扩展方向分析：跨界盲区与增量高杠杆

> **角色**: 资深架构师 / 产品经理
> **方法**: 全局代码库深扫（forge-core 13 内部包 + cmd/forge 18+ CLI 命令 + harness 26+ 模块 +
>   `.agent/` 完整治理骨架 + examples/ + 全部 27+ 份已有 docs/analysis/ 交叉核对）
> **基线**: Sprint 26-27 全状态（真点火 multi-agent 端到端坐实、Learning loop 三维真数据落盘、
>   parallel 模式完整交付含锁顺序契约、四维资源安全护栏）
> **纪律**: 绝不与任何已有分析文档的核心论点重叠
> **日期**: 2026-07-01

---

## 何为「跨界盲区」？

此前 27+ 份分析文档已覆盖以下领域（本文不再重复）：

| 已有覆盖域 | 对应文档 |
|---|---|
| 自适应工作流 | `high-value-extensions.md` 方向一 |
| 闸门自省 / 元学习 | `high-value-extensions.md` 方向二 |
| 多租户 / Agent 权限模型 | `expansion-gaps-v7-novel.md` 方向四 |
| 确定性 Replay 引擎 | `expansion-gaps-v7-novel.md` 方向三 |
| Memory 衰减 / 去重 | `high-value-perspectives-v11.md` 方向四 |
| 收敛陷阱 & 并行 fail-fast | `edgecases-and-perf.md` §1.1 / §3 |
| 配置表面积 | `configuration-surface-and-adoption.md` |
| ADR 决策衰退 | `eighth-wave-adr-decay.md` |
| 长时数据生命周期 | `fresh-scan-strategic-expansion.md` 方向一 |
| YAML-Shim 消除 | `fresh-scan-strategic-expansion.md` 方向二 |
| 闸门可靠性仪表化 | `novel-extensions-v12-architect-perspective.md` 方向一 |
| 跨周期收敛状态机 | `expansion-core-five-2026-07-01.md` 方向一 |
| 策略模拟引擎 | `novel-extensions-v12-architect-perspective.md` 方向三 |
| 自愈运行时 | `expansion-directions-v6-novel-perspectives.md` 方向四 |
| 预算优雅降级 | `novel-extensions-v12-architect-perspective.md` 方向五 |
| 自我测试 / Dogfood 缺口 | `self-testing-and-dogfooding.md` |
| ForgeOS 自身治理 | `expansion-forgeos-meta-governance.md` |

**本文方向聚焦**：此前分析普遍关注「系统内部」的优化（引擎、内存、收敛、闸门），
而本文关注**系统边界上的盲区**——ForgeOS 作为「软件工厂」与外部世界（开发者、CI、
生产环境、其他工具）的交互界面上的结构性缺口。

---

## 目录

1. [方向一：Phase 级文件系统隔离与原子回滚](#方向一phase-级文件系统隔离与原子回滚)
2. [方向二：配置面模式校验引擎——治理错误预防](#方向二配置面模式校验引擎治理错误预防)
3. [方向三：跨相位故障归因链路](#方向三跨相位故障归因链路)
4. [方向四：执行器多样性——HTTP API 执行器与 fallback 链](#方向四执行器多样性http-api-执行器与-fallback-链)
5. [方向五：预算规划器——运行前成本推演](#方向五预算规划器运行前成本推演)

---

## 方向一：Phase 级文件系统隔离与原子回滚

### 现状

整个 `forge run` / `forge evolve` 的工作目录（`o.root`）是所有相位**共享的**。
每个 agent phase 直接在项目目录中写文件。当前相位完成后，下一个相位继承
完整的文件系统状态——包括上一个相位可能写坏的、不完整的、或安全有问题的文件。

```go
// internal/orchestrator/command_executor.go
cmd.Dir = o.root  // 所有相位使用同一个工作目录
```

**边界情况**：

| 场景 | 后果 |
|------|------|
| implementer A 写了一个错误的配置文件 (e.g. `config.yaml` 被清空) | 后续 implementer B / reviewer / qa 都看到空文件 |
| reviewer 修改了源文件作为「建议」 | 这些修改被 QA 当做「已完成事实」评估 |
| 一个 agent phase 生成了包含 API key 的文件 | 该文件残留在磁盘上，后续相位可读取，且不会被清理 |
| loop-back 重跑 implementer 后 | 前一次 implementer 的遗留文件（废弃函数、死代码）与本次的并存 |

**已有的保护措施**：无。`FreshContext` flag 控制的是**prompt 上下文注入**（是否跳过 previous phase outputs 的 feed-forward），不控制文件系统隔离。`gateLedger` 和 `phaseOutputLedger` 记录的是元数据，不是快照。

### 为什么它值得做

1. **安全原因**：一个相位产生的敏感文件（secret、API key、password）残留到下一个相位是信息泄露。当前无任何机制检测或清理。
2. **收敛正确性**：如果前一个相位遗留了不应该存在的代码（例如一个被废弃的函数），gate 可能因为该遗留代码而通过（额外代码不影响测试），但 reviewer 可能基于错误假设做出判定。
3. **回路可靠性**：loop-back（gate FAIL → jump back to implementer）时，implementer 在**已有自己前次输出**的目录中运行。如果它的 prompt 说「修复数据库连接」，但它的前次代码已经创建了一个有缺陷的 `db.go`，这次它可能只补丁而不重写——继承前次的错误。

### 建议

**Phase 级 Git Worktree 隔离**（轻量级方案）：

```
forge run build --phase-isolation  # opt-in
```

对于每个 agent phase：
1. phase 开始前：`git stash`（保存未提交变更）+ `git worktree add <tmpdir> HEAD`
2. agent phase 在 `<tmpdir>` 中运行
3. phase 完成后：`git diff --name-only <tmpdir>` 收集变更文件
4. 如果相位 FAIL（gate 没通过），直接丢弃 worktree（隔离生效）
5. 如果相位 PASS：将变更文件 **patch-apply** 到主工作目录 + gc worktree
6. 对 gate phase / qa phase：在主工作目录运行（它们只读）

**成本**：每次 agent phase 多 1-3 次 git 操作（~100ms）。受益是：
- 失败相位绝不污染主目录
- 一个相位的副作用不会无意中影响另一个
- Reviewer 和 QA 看到的是「被声称的状态」而不是「前一个相位遗留的偶然状态」

**证据位置**：
- `command_executor.go:121` — `cmd.Dir = o.root`（共享目录）
- `loop.go:212` — loop-back 时用 `RunFrom` 跳回前一个相位，无文件系统回滚
- `orchestrator.go:380` — `runAgentPhase` 无 pre-exec snapshot
- 无任何 `git worktree` 或 `git stash` 引用（`grep -r "worktree\|stash" forge-core/` 返回空）

---

## 方向二：配置面模式校验引擎——治理错误预防

### 现状

ForgeOS 的治理配置分散在 14+ 个 YAML/YML 文件中：

```
.agent/
├── project.yml           # mode / lifecycle / features
├── policies/
│   └── modes.yml         # modes · gates · coverage · migrations
├── routing/
│   └── policy.yml        # scoring · tiers · safety · budget
├── workflows/            # 4 YAML workflow 文件
│   ├── build.yml
│   ├── design.yml
│   ├── discover.yml
│   └── evolve.yml
├── agents/               # 9 role card 文件
│   └── *.md
└── skills/               # 8 skill 文件
    └── *.md
```

这些文件被**多个不共享类型的解析器**独立消费：

| 消费者 | 解析方式 | 容错策略 |
|--------|----------|----------|
| `yaml2json.py` / `yaml2json.go` | Python/Go YAML→JSON | fallback-to-python |
| `internal/asset` | Go `encoding/json` | fault-tolerant：缺字段 → zero value |
| `internal/mode` | Go 行扫描 `strings.CutPrefix` | fail-safe：未知值 → 全开执法 |
| `internal/routing` | Go 硬编码常量 | fail-safe：未知 mode → Sonnet |
| `internal/migrate` | Go 硬编码常量 | N/A（纯命令式） |
| `harness/check.py` | Python YAML 治理检查 | 校验模式·生命周期·优先级·ADR |

**关键缺口**：没有单一的 schema 描述 `project.yml` 或 `modes.yml` 的合法结构。
所以以下这些错误是**静默生效**的：

| 错误 | 实际效果 | 发现难度 |
|------|----------|----------|
| `project.yml` 中 `mode: enginnering`（拼写错误） | `internal/mode` 的 `effective()` 视作未知 mode → fail-safe 全开 | 浪费资源但不出错，可能永远不被发现 |
| `modes.yml` 中 `coverage_threshold: 8`（丢了 0） | 被解析为 8%（而非 80%） | 覆盖阈值几乎无效 |
| `build.yml` 中 reviewer phase 的 `required_when` 路径写错 | `skipByMode` 找不到 modes key → 跳过（空 RequiredWhen） | reviewer 从不运行，无人注意 |
| `routing/policy.yml` 中 `dimensions[].weight` 加起来 != 1.0 | `Score()` 做 `加权和 / 总权重`，不会报错 | 路由分数偏差，无法察觉 |
| `modes.yml` 中 `migrations` 的 `trigger` 写成了 `muanual` | 不匹配 `manual` → 自动迁移不会触发 | safe，但 manual 触发时也可能不工作 |

### 为什么它值得做

1. **治理的治理**：ForgeOS 的职责是防止其他项目架构腐化。但如果**自己的配置**都能静默出错，系统的可信度受损。这是 meta-governance 最直接的切入点。
2. **采纳门槛**：新用户配置 ForgeOS 时，`project.yml` 写错一两个字段是最常见的 onboarding 错误。一个 `forge validate --config` 命令能在第一步就给出明确错误，而不是等用户跑 3 次迭代后才发现不对。
3. **声明式契约**：`forge validate` 当前存在（`cmd/forge/validate.go`），但它验证的是**被治理项目的**结构。没有一个命令验证 ForgeOS 自身治理配置的结构和内部一致性。

### 建议

**增量实现**（利用现有 `forge validate` 骨架）：

1. **`forge doctor --config`** 模式：读取所有 `.agent/*.yml` 文件，对照一个隐式 schema 检查：
   - `project.yml`: 验证 mode ∈ {explorer,balanced,engineering,cto}；lifecycle ∈ {idea,mvp,growth,production}
   - `modes.yml`: 验证每个 mode 的 `gates` 是 `allGates` 的子集；`enforce` ∈ {warn,block}；`coverage_threshold` ∈ [0,100]
   - `policy.yml`: 验证 `dimensions[].weight` 总和误差 < 0.01；`by_task_type` 的 tier ∈ {haiku,sonnet,opus}
   - 跨文件引用：`workflow.Phases[].Agent` 的值应在 `.agent/agents/` 中存在同名的 `.md` 文件

2. **`project.yml` 的自检**：在 `cmdRun` / `cmdEvolve` 的 preflight 路径中（已有 `cmdPreflight` in `preflight.go`），添加对 `project.yml` 语义有效性的检查。如果 mode 或 lifecycle 值不在已知集合中，打印 WARNING（不阻断执行）。

3. **`mode.Effective` 的防御加固**：当前 fail-safe（未知 → 全开执法）是正确的安全策略，但在日志中应该打印一条清晰的警告：
   ```
   forge run: WARNING unknown mode "enginnering" — falling back to full enforcement (production-safe)
   ```
   而不是静默降级。

**证据位置**：
- `internal/mode/mode.go:45` — `Effective()` 的 fail-safe：无 `return mode.Policy{...}` 默认全开（注释说 "safety: unknown input → all gates" 但未告知用户）
- `internal/routing/routing.go:64` — `defaultFor()`：未知 mode → Sonnet（静默降级）
- `cmd/forge/validate.go` — 存在但只验证被治理项目，不验证自身配置
- `.agent/policies/modes.yml` — 14 个字段的复杂结构，无 schema 文件

---

## 方向三：跨相位故障归因链路

### 现状

当一次 `forge run build` 失败时——不管是 gate FAIL、agent 错误、还是收敛超时——ForgeOS 能报告「什么」失败了，但不能报告「谁」导致了失败。三个相位之间的因果链丢失了：

```
Planner ──→ Implementer ──→ Gate ──→ Reviewer ──→ QA
   ①           ②            ③        ④           ⑤
```

| 失败表面 | 根因可能位置 | 当前诊断能力 |
|----------|-------------|-------------|
| Gate: test FAIL | ① Planner 分配了模糊不清的任务 → ② Implementer 误解需求 → 写了错误的代码 | ❌ 只报告 test FAIL |
| Reviewer: REQUEST_CHANGES | ② Implementer 实现正确但违反了架构约定（ADR 没读） | ❌ 只报告 reviewer 判定 |
| Converge NOT MET | ① Planner 的 roadmap 拆分布置过多，在 5 次迭代内不可能完成 | ❌ 只报告 roadmap_completion < 100% |
| 重复 loop-back | ② Implementer 每次修复都引入新问题 → ③ Gate 总是 FAIL → 循环 | ❌ 只报告 "still red after N/M loop-backs" |

`Engine.OnGateResult` 和 `Engine.AgentVerdict` 分别报告 gate 和 reviewer 的裁决，
但没有机制将这些裁决链接到**哪一个相位的行为导致了它**。`OnFail` loop-back 只携带了
「什么坏了」（gate/agent 名称），没有「为什么坏的」信号链。

### 为什么它值得做

1. **调试效率**：当前调试一次失败的 evolve 运行要手动追 `trace.jsonl` + `memory.jsonl` + checkpoint，拼凑因果链。一个归因链路让 `forge doctor --run` 直接说：「本次 build 未收敛的根本原因：planner 在 iteration 1 将 8-item roadmap 标记为 100% 完成，但 implementer 只写了 2 个 feature 的代码（file_delta=25%），导致 iteration 3-5 一直在重试不可完成的任务。」
2. **自适应改进**：如果系统能识别「planner 过度承诺」模式（roadmap_completion 经常远高于 file_delta），它可以自动调整 planner 的 prompt 来校准。这是 Learning loop 缺失的维度——不是分数，是根因。
3. **熔断差异化**：当前 `staleCount` 只按 roadmap% + gates 变化判定「无进展」。如果能知道「为什么」无进展（planner 阶段 vs implementer 阶段 vs reviewer 阶段），可以差异化处理：planner 无进展 → 降低任务复杂度预期；implementer 无进展 → 升级模型 tier。

### 建议

**在 checkpoint 和 trace 中增加归因字段**（非侵入式）：

1. **`persist.Checkpoint`** 增加一个 `FailureAttribution` 字段：
   ```go
   type FailureAttribution struct {
       Phase        string  // "planner" | "implementer" | "reviewer" | "gate"
       Metric       string  // "roadmap_misalignment" | "code_quality" | "test_regression" | "budget_exhausted"
       Confidence   float64 // 0.0-1.0 归因模型对自己判断的置信度
       Evidence     string  // 自然语言证据摘要
   }
   ```

2. **失败后归因**：在 `LoopEngine.Run` 返回 non-converged `LoopOutcome` 后，`execLoop` 调用一个 `attributeFailure` 函数，读取当前 iteration 的 phases outputs + gate results + trace events，尝试归因：
   - 如果 `GatesGreen == false` → 检查 gate 历史：是 test 一直 FAIL（implementer 问题）还是 lint/arch 新出现的（implementer 引入）？
   - 如果 `RoadmapCompletion` 停滞 → 检查 `file_delta`：如果是低 file_delta + 高 roadmap（planner 过度承诺），如果是高 file_delta + 低 roadmap（implementer 效率低下）
   - 如果 loop-back 耗尽 → 检查 reviewer 的每次 `REQUEST_CHANGES` 文本是否重复（reviewer 和 implementer 之间的语义分歧）

3. **`forge doctor --attribution <checkpoint.json>`** 子命令，读取 checkpoint 中的归因记录并给出自然语言总结。

**证据位置**：
- `internal/persist/checkpoint.go` — `Checkpoint` 结构体有 8 个字段，无归因相关
- `internal/orchestrator/loop.go:118-130` — `staleCount` 只比较 cur/prev roadmap%，不分析原因
- `cmd/forge/evolve.go:97-116` — `checkpointHook` 写 checkpoint，只有 signals 无归因
- `internal/trace/trace.go:30-40` — `Event` 有 `Kind`/`Name`/`Status`/`Detail`，但每事件独立，非链接

---

## 方向四：执行器多样性——HTTP API 执行器与 fallback 链

### 现状

`internal/orchestrator/executor.go` 定义了一个极简接口：

```go
type AgentExecutor interface {
    Exec(ctx context.Context, phase asset.Phase, mode, tier, prompt string) error
}
```

唯一的真实实现是 `CommandExecutor`（`command_executor.go`），它 `exec.Command` 出子进程
（默认是 `claude -p`）：

```go
// command_executor.go
cmd := exec.Command(o.agentCmd, args...)  // 只支持父子进程模式
cmd.Stdout = &stdout
cmd.Stderr = &stderr
```

**限制**：
1. **只能 spawn 子进程**：不支持 HTTP API 直接调用。如果你有自己的 LLM API 端点（自建、内部部署、Azure OpenAI、私有 Claude），必须先包装成一个 CLI 工具才能被 ForgeOS 使用。
2. **无 fallback**：`runAgentPhase` 在 `runAgentPhaseBudgeted` 中调 `Exec()`。如果返回错误，`runAgentPhase` 根据 `ExecError` 的 `Retryable()` 决定是否重试。但**重试永远用同一个 executor**——如果 `claude` CLI 因为 OAuth token 过期而 401，重试也永远 401。
3. **无跨供应商选择**：`routing.ResolveModel()` 可以给模型名，但 `command_executor.go` 只把它传给 `--model` flag。如果模型从 `claude-opus-4` 降级到 `claude-sonnet-4`（BudgetAdjustTier），同一个 executor 用同一个 CLI——只是模型名变了。不可以使用 `gpt-4` 代替。
4. **无超时差异化**：`CommandExecutor.Timeout` 对所有相位统一设定。安全 reviewer 可能需要 5 分钟（完整 STRIDE 分析），而 linter agent 只需要 30 秒。当前不能按相位设不同的 executor 参数。

### 为什么它值得做

1. **供应商锁定风险**：ForgeOS 的 ROADMAP 声明「跨厂商池 v3」，但当前架构让跨厂商池的接入成本很高——每个供应商需要自己的 CLI 包装器和参数转换。一个 HTTP API 执行器可以直接支持 OpenAI / Anthropic / Google / Azure，消除中间 CLI 的维护成本、解析开销和版本漂移。
2. **企业采纳壁垒**：很多企业不允许他们的代码通过第三方 CLI（claude）发送到云端，但允许 HTTPS 请求到他们自己的 API 网关。HTTP 执行器是让 ForgeOS 进入受监管环境的钥匙。
3. **成本优化**：当前 `BudgetAdjustTier` 只能降级到同供应商的更低 tier（sonnet → haiku）。有了 HTTP 执行器，可以从 `claude-opus-4` 降级到 `gpt-4o-mini`（$0.15/M 输入 vs $15/M 输入——100x 差异）。

### 建议

1. **在 `executor.go` 中增加 `HTTPExecutor`**：读 `--provider anthropic --api-key-env ANTHROPIC_API_KEY`，直接用 JSON API。复用现有的 `routing.ResolveModel()` 来映射 tier → model name，但跨供应商时需要扩展 `ModelMap`。

2. **在 `Engine` 中增加 `ExecutorFallback` 字段**：
   ```go
   // 在 runAgentPhase 中，当 Exec 返回不可重试的错误时，
   // 尝试 fallback chain 中的下一个 executor
   type Engine struct {
       Exec           AgentExecutor      // 主执行器（claude CLI）
       ExecFallback   []AgentExecutor    // 回退链（HTTP API → 更低成本的 API → …）
       // ...
   }
   ```

3. **在 `runOpts` 中增加按相位定制 executor 的参数**：`--executor-planner=http --executor-qa=command`
   或者更简单：workflow YAML 的 phase 中增加 `executor` 字段：
   ```yaml
   phases:
     - name: planner
       agent: planner
       executor: http
       provider: anthropic
       model_tier: sonnet
   ```

4. **`forge route` 的输出增加 `providers` 信息**：不仅报告 tier，还报告推荐供应商和备选供应商。

**证据位置**：
- `internal/orchestrator/executor.go:10` — `AgentExecutor` 接口定义（单一方法，无供应商意识）
- `internal/orchestrator/command_executor.go:25-40` — 硬编码 `exec.Command(agentCmd,...)`
- `internal/orchestrator/command_executor.go:90-95` — `agentCmd` 字段，只有 `claude`，无 HTTP 支持
- `internal/routing/routing.go:130-140` — `ModelMap` 仅有 `anthropic`，无其他供应商
- `internal/orchestrator/orchestrator.go:380` — `runAgentPhase` 只调用一次的 `Exec`，无 fallback 链

---

## 方向五：预算规划器——运行前成本推演

### 现状

ForgeOS 有成熟的运行期预算治理：

| 护栏 | 位置 | 作用 |
|------|------|------|
| `MaxAgentCalls` | `orchestrator.go` | 每个 iteration 的 agent 调用次数上限 |
| `MaxLoopBack` | `orchestrator.go` | loop-back 次数上限 |
| `runBudget` | `cost.go` | 累积美元花费上限 |
| `agentMaxBudgetUSD` | `CommandExecutor` | 单次 claude 调用花费上限 |
| `timeout` | `CommandExecutor` | 单次 agent 调用超时 |
| `maxOutputBytes` | `CommandExecutor` | 输出上限防 OOM |
| `BudgetAdjustTier` | `routing/routing.go` | 预算接近时降级 tier |

**但所有机制都是运行期/被动的**——它们在花费发生时 TRIP。没有**运行前**的机制告诉用户：
「你选择的 workflow=build mode=balanced lifecycle=mvp 预计花费 $2.50-$4.20，耗时约 3-7 分钟。」

当前用户看到的是盲启动：
```
$ forge run build --executor command
# … 花费 $3.80 和 5 分钟后 …
forge run: workflow completed
```

或者更糟：
```
$ forge run build --executor command --max-agent-calls 5 --agent-max-budget-usd 2.0
# … 1 次 claude 调用就用完了 $2.0，gate 没跑完就停止 …
forge run: agent-budget exhausted after 1 phase(s) — reduce task scope or raise budget
```

### 为什么它值得做

1. **采纳障碍**：最让新用户犹豫的问题是「这会花多少钱？」。没有成本预估，用户不敢在生产中使用 `--executor command`。当前只能在事后悔。
2. **workflow 选择辅助**：用户不知道应该用 `forge run`（单次）还是 `forge evolve`（迭代），不知道应该选 explorer 还是 balanced。一个预算规划器可以说：
   ```
   forge budget: build.yml mode=explorer lifecycle=mvp
     预估: 1-2 agent calls × $0.30-0.80 = $0.30-$1.60
     推荐: --max-agent-calls 2 足够

   forge budget: build.yml mode=balanced lifecycle=production
     预估: 4-6 agent calls × $0.80-3.50 (含 reviewer=opus) = $3.20-$21.00
     推荐: 设置 --run-budget-usd 25.0 防止超支
   ```
3. **Learning loop 的第二维度**：当前 scorecard 记录的是**事后** cost/latency/quality。预算规划器可以将**事前预估 vs 事后实际**的偏差计入 scorecard，让系统学会更准确预估——不仅知道做了什么，还知道「说得准不准」。

### 建议

1. **`forge budget` 子命令**：
   - 输入：workflow 名 + mode + lifecycle + 可选的 `--agent-cmd`/`--agent-max-budget-usd`
   - 处理：遍历 workflow 的 phases，对每个 phase 调用 `routing.TierFor(agent, mode)` 拿到 tier，再从 scorecard（如果有历史）或默认定价表中查 cost 范围
   - 输出：预估总花费 + 每 phase 明细 + 推荐的安全护栏值

2. **成本模型来源**：
   - 无历史：用硬编码默认价目表（Claude 官方定价 × 1.2 安全系数）
   - 有历史：用 scorecard 中该 (workflow, mode, lifecycle) 组合的 `avg_cost_usd` 百分位数
   - 学习：`forge run`/`forge evolve` 结束后，将预估 vs 实际偏差写入 memory 或 scorecard 的扩展字段

3. **在 `forge run` 的输出中增加预估对比**：
   ```
   forge run: stage=build mode=balanced lifecycle=mvp executor=command
     budget estimate: $2.50-$4.20 (learned from 12 previous runs)
     actual spend:    $3.80  (estimate accuracy: 90% — within predicted range)
   ```

**证据位置**：
- `cmd/forge/cost.go` — `runBudget` 结构体：追踪累积花费和上限，但不做预估（纯运行期）
- `cmd/forge/scorecard_wind.go` — `LoadScorecards` 读取历史 cost/latency/quality 数据，但只用于 `HistoryTiebreak`，不用于预估
- `internal/routing/routing.go:100-110` — `BudgetAdjustTier`：运行期降级，不是规划期预估
- `internal/routing/scorecard.go` — `ScorecardEntry` 有 `avg_cost_usd` 字段，可用于预估但无消费者
- 无 `forge budget` / `forge estimate` 子命令

---

## 总结：优先级矩阵

| 方向 | 用户影响 | 实现成本（估计） | 风险降低 | 采纳杠杆 |
|------|---------|-----------------|---------|---------|
| ⭐ Phase 级文件系统隔离 | 高（收敛正确性 + 安全） | 中（~200 行 + git 操作封装） | 高（防污染、防泄露） | 中（显式 opt-in） |
| ⭐ 配置面模式校验 | 中（预防静默错误） | 低（~150 行 + 现有 validate 骨架） | 高（治理自身可靠） | 高（新用户 onboarding） |
| ⭐ 跨相位故障归因 | 中（调试效率） | 中（~300 行 + checkpoint 扩展） | 中（缩短排障周期） | 中（运维团队收益大） |
| ⭐ 执行器多样性 | 高（企业采纳关键） | 高（~500+ 行 + API 集成） | 中（去供应商绑定） | 高（打开受监管市场） |
| ⭐ 预算规划器 | 高（成本透明） | 中（~250 行 + 价目表 + scorecard 集成） | 低（不解决已有 bug） | 高（降低首次使用焦虑） |

**最推荐起步**：方向二（配置面模式校验）和方向五（预算规划器），成本最低且用户感知最强。
方向四（执行器多样性）和方向一（Phase 隔离）是中期高价值目标，值得在 v3 路线图中占位。

*分析日期：2026-07-01 | 基于 forge-core 全量源码扫描 + 交叉核对 27+ 份已有分析文档*
