# ForgeOS: 五处产品架构扩展方向（代码实锤级验证）

> **角色**: 资深产品架构师  
> **方法**: 全库逐文件扫描 `forge-core/*.go`(18 包 + cmd, ~25k LOC) · `harness/`(39 模块) · `.agent/workflows/`(5 个) · `.agent/agents/`(12 张卡) · 通读 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` · `CURRENT_SPRINT.md`(31 sprints) · 交叉审阅 `docs/requirements/` 最新 10 篇确认无重复。  
> **纪律**: 每个方向附精确到 `file:line` 的代码级证据、边缘场景、产品价值判断。不编写任何代码。  
> **日期**: 2026-07-11

---

## 差异化声明

已有 190+ 篇分析文档覆盖了以下高密度领域，本文**不再重复**：

| 饱和域 | 本文处理 |
|--------|----------|
| 编排状态机（串/并行/loop-back/mode-gating/resume/parallel/wave） | ✅ 跳过 |
| 学习闭环（trace/scorecard/converge/memory/Context 注入） | ✅ 跳过 |
| 安全护栏（递归深度/执行上限/超时/输出上限/进程组） | ✅ 跳过 |
| 治理执法（arch-check 8 检查/check.py/function-length/circular） | ✅ 跳过 |
| Memory 条目 Detail 上限/Compact/Prune/Supersedes | ✅ 跳过 |
| 多实例隔离 / `.forge` 并发损坏 | ✅ 跳过 |
| 审批标记审计/多级审批链 | ✅ 跳过 |
| CLI 体验（detect/preflight/doctor/status/migrate/validate） | ✅ 跳过 |
| 第三地平线（多仓库/Web UI/Firecracker/LiteLLM/跨厂商路由） | ✅ 跳过 |
| YAML 宽容加载 / 资产静默降级 | ✅ 跳过 |

**本文的五个方向全部落在上述饱和域的裂缝/边界**——不是已有抽象层的纵向深化，而是**产品级盲区**：每个方向都是一个真实用户在使用 ForgeOS 时必然会撞到的墙。

---

## 方向一 · `forge run` 收敛 NOT MET 但始终 Exit 0 —— 自动化的静默假阳性

> **优先级**: P0（产品 Bug，非功能缺口）  
> **类别**: CLI UX · 自动化契约  
> **一句话**: `forge run discover|design|review|build` 在工作流完成后无论收敛是否达成，**始终返回退出码 0**。CI/CD 和脚本将每次调用都视为成功。

### 代码级证据

```go
// forge-core/cmd/forge/engine_build.go:458-473
func execEngine(ctx context.Context, wf asset.Workflow, o runOpts) int {
    // ...
    if err := runWorkflow(ctx, eng, wf, o, logln, startPhase); err != nil {
        fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
        return 1           // ← 只有运行时错误才 exit 1
    }
    fmt.Println("forge run: workflow completed")
    reportConvergence(wf, o.root, probe, categories, lifecycle, o.approved, verdicts)
    //   ↑ convergence 报告只是日志副作用，返回值被丢弃
    return 0               // ← 始终 exit 0，无论 MET / NOT MET
}
```

`reportConvergence`（`main.go:395`）会打印类似 `convergence: NOT MET (conjunction)` 的文字，但**它的返回值没有被任何人消费**。调用者 `cmdRun`（`main.go:276-305`）直接返回 `execEngine` 的结果——它只区分「运行时错误（exit 1）」和「正常运行完成（exit 0）」。

对比 `forge evolve` 的正确处理：

```go
// forge-core/cmd/forge/evolve.go:257-265
func reportLoop(out orchestrator.LoopOutcome, err error) int {
    // ...
    if out.Converged {
        return 0
    }
    return 1               // ← evolve 正确将「未收敛」映射为 exit 1
}
```

### 边缘场景

1. **`forge run review`**: CTO 审批评审结果为 `REDESIGN`（正常产出，非错误）→ workflow completes → convergence NOT MET → **exit 0**。CI 认为 review 通过。
2. **`forge run discover`**: 需求发现 confidence=50%（低于 80% 阈值）→ convergence NOT MET → **exit 0**。CI 认为需求已就绪。
3. **`forge run build`**: 测试 gate 通过但 coverage 低于阈值 → convergence NOT MET → **exit 0**。CI 认为构建成功。
4. **自动化编排**: `forge run design && forge run build && forge run review` 链式调用——设计阶段的 NOT MET 被静默忽略，构建阶段基于不完整的设计开始工作。

### 产品价值

这是**当前 CLI 的契约违反**：用户自然期望 `forge run` 的退出码反映工作流是否成功收敛。在 24h 自治运行场景中，自动化系统完全依赖退出码做决策——一个静默的假阳性意味着下游系统（CI/CD、编排器、通知系统）收到错误的成功信号，把未就绪的产物当作已完成。

### 修复方向

`execEngine` 应在 `reportConvergence` 后检查收敛状态，NOT MET 时返回 1（或一个区分收敛 vs 未收敛的特定退出码）。

---

## 方向二 · 脊柱工作流无结构化数据契约 —— `next_stage` 仅叙述而不驱动

> **优先级**: P1（架构缺口）  
> **类别**: 跨工作流编排 · 脊柱集成  
> **一句话**: 脊柱（Discover→Design→Review→Build→Evolve）的 `next_stage` 字段仅用于屏幕叙述，**没有任何结构化数据在工作流之间传递**。每个工作流从空白上下文启动。

### 代码级证据

```go
// forge-core/internal/asset/asset.go:223-228
type OnApproved struct {
    // NextStage is the spine stage an approval unlocks (design.yml -> "build"). The
    // non-routing fields of on_approved (emit list) are deliberately NOT
    // modeled here — only the routing-relevant NextStage is.
    NextStage string `json:"next_stage"`
}
```

`NextStage` 只有一个消费者——叙述：

```go
// forge-core/cmd/forge/main.go:430-436
func nextStageLabel(stop asset.StopCondition) string {
    if stop.OnApproved.NextStage == "" {
        return "(no next_stage declared)"
    }
    return "next_stage=" + stop.OnApproved.NextStage
}
```

**它不驱动任何编排行为**。没有自动触发下一阶段，没有工件传递，没有上下文继承。

证据：每个工作流的加载是完全独立的——`cmdRun` / `cmdEvolve` 都从文件系统重新 `loadWorkflow`，**不带任何来自上一阶段的上下文**：

```go
// forge-core/cmd/forge/main.go:276-284
func cmdRun(args []string) int {
    // ...
    wf, err := loadWorkflow(o.root, name)
    // ...
    return execEngine(ctx, wf, o)
}
```

`loadWorkflow`（`main.go:297-320`）只读 YAML → JSON → `asset.Workflow`，不检查 `.forge/` 是否有前一阶段的产物，不注入 `docs/discovery/prd.md` 内容，不传递 scorecard 或 memory。

### 边缘场景

1. **`forge run discover` → `forge run design`**: design.yml 的 solution-architect agent 不知道 PRD 内容（除非手动读取 `docs/discovery/prd.md`——当前依赖 agent 自己发现文件，无结构化保证）。
2. **`forge run design` → human_approval → `forge run build`**: build.yml 的 planner 不知道架构决策、ADR、设计文档。每个工作流重新自我发现。
3. **多工作流串联的「冷启动」**: 每次 `forge run <stage>` 都重新加载所有 agent 卡、策略、memory——前一阶段累积的轨迹完全丢失。

### 产品价值

脊柱是 ForgeOS 的核心编排模型（Discover→Design→REVIEW→Build→Evolve），但当前每个阶段是**孤立的单次运行**。真实产品需要让下游阶段继承上游的决策产物（PRD → 架构 → 评审意见 → 构建计划 → 演化轨迹），而非每次从零开始。这不只是效率问题——agent 在不知道 PRD 的情况下做的架构设计、在不知道架构决策的情况下写的代码，必然产生不一致。

### 修复方向

在 `.forge/` 中维护一个**脊柱状态文件**（类似 checkpoint 但跨工作流级别持久），记录：
- 已完成阶段及时间戳
- 关键产物路径（`prd.md`, `ADR-000x.md`, `review-verdict.md`）
- 当前阶段的收敛信号

`loadWorkflow` 或 `execEngine` 在启动时读取该状态，注入到 prompt 中作为上下文。

---

## 方向三 · 人工审批标记为空文件 —— 无审计轨迹、无身份、无时效

> **优先级**: P1（企业采纳门槛）  
> **类别**: 安全合规 · 治理  
> **一句话**: `.forge/<stage>.approved` 标记文件内容是空的，`humanApproved()` 只检查文件是否存在。没有人知道谁批准了什么、何时批准、为何批准。

### 代码级证据

```go
// forge-core/cmd/forge/gates.go:168-185
func approvalPath(root, stage string) string {
    return filepath.Join(forgeDir(root), stage+".approved")
}

func humanApproved(root, stage string, flag bool) bool {
    if flag {
        return true       // --approved 标志位无身份信息
    }
    _, err := os.Stat(approvalPath(root, stage))
    return err == nil     // ← 只检查文件存在，不读内容
}
```

`--approved` 命令行标志和标记文件是唯二的审批来源，**两者都不记录**：
- 谁批准了（用户身份）
- 何时批准（时间戳）
- 为何批准（理由）
- 审批有效期
- 审批链（谁先批、谁最后批）

### 边缘场景

1. **合规审计**: SOC2/ISO27001 要求谁在何时批准了设计→构建的 gate。当前记录为零。
2. **审批超时**: 一个 approval 创建后永久有效。如果项目演进了一个月，旧的 approval 仍然算数。
3. **审批人变更**: 员工离职后，其之前创建的 approval 文件仍有效。无撤销机制。
4. **多级审批**: 企业流程要求「架构师先批→安全再批→CTO 放行」。当前单文件无法表达多级审批链。

### 产品价值

ForgeOS 的 human_gate 是**全系统最高杠杆的闸门**（`PROJECT.md:23`：「Human Approval = 全系统最高杠杆闸门」），但其实现只是一个 `os.Stat`。对于任何企业的合规采纳（金融、医疗、政府），这面闸门需要可审计、可追溯、可配置策略。

### 修复方向

将 `.forge/<stage>.approved` 从空文件升级为 JSON 内容：

```json
{
  "approved_by": "user@example.com",
  "approved_at": "2026-07-11T12:00:00Z",
  "reason": "Architecture reviewed and accepted. See ADR-0005.",
  "expires_at": "2026-07-18T12:00:00Z",
  "chain": ["architect@example.com", "security@example.com", "cto@example.com"]
}
```

`humanApproved` 除检查存在性外，还应验证未过期、链完整。`--approved` 标志应记录调用者身份（`$USER` 或更好的认证机制）。

---

## 方向四 · 无 Warm Start / 工作流 Forking —— 每次运行从绝对零开始

> **优先级**: P2（效率与实验能力）  
> **类别**: 运维效率 · 实验  
> **一句话**: 每次 `forge run` / `forge evolve` 都是从**空状态**启动。无法注入初始 memory、预填 scorecard、克隆 checkpoint，意味着并行探索和零成本重放不可行。

### 代码级证据

`internal/persist/checkpoint.go` 仅实现了 `Save` / `Load` 原子操作，**没有 `Merge`、`Seed`、`Fork`、`Chain` 等操作**：

```go
// forge-core/internal/persist/checkpoint.go:71-122
func Save(path string, cp Checkpoint, retain int) error { ... }  // 仅写入
func Load(path string) (Checkpoint, bool, error) { ... }         // 仅读取
```

没有函数允许：从外部文件加载 checkpoint、合并两个 checkpoint、创建 checkpoint 的分支。

`forge evolve` 的 `resumeStart`（`evolve.go:262-275`）只读**一个** checkpoint 文件，没有选项指定自定义种子文件：

```go
func resumeStart(root string, resume bool) (start int, prev float64, spentMicros int64, phaseStart int, err error) {
    if !resume {
        return 0, -1.0, 0, 0, nil            // ← 每次都是 0，从头开始
    }
    cp, found, err := persist.Load(checkpointPath(root))
    // ... 只能读默认路径，不能指定外部来源
}
```

同样，`internal/memory/` 的 `Load` 只从默认路径加载，没有 `Inject` 或 `Merge` 操作。

### 边缘场景

1. **A/B 实验**: 想从同一个 checkpoint 分叉，一个分支跑 `engineering` 模式、一个跑 `explorer` 模式，比较结果。当前不可能——第一个 evolve 会覆盖 checkpoint。
2. **零成本重放**: 一个 evolve 跑了 10 轮后失败了，你修了一个 prompt 问题并想从第 8 轮重放。当前只能从 checkpoint 第 10 轮继续，或从零开始跑 10 轮。
3. **预热生产**: 你有一个项目甲已经跑了 20 轮 evolve 积累了大量 memory/scorecard 数据，想在新项目乙的初始阶段注入这些数据来加速。当前不可能——memory 是项目绑定的。
4. **跨项目迁移**: 想将一组治理策略（已调优的 scorecard 权重）从一个项目复制到另一个。当前只能手动重建。

### 产品价值

在 24h 自治运行的真实场景中，「每次从零开始」意味着：
- 一个 8 轮 evolve 如果失败在第 7 轮，即使发现了修复，也需要重新跑全部 7 轮（真实 LLM 成本）。
- 无法做「对比实验」（同一 checkpoint 分叉跑不同参数）。
- 跨项目知识迁移为零。

这直接限制了 ForgeOS 从「元框架」进化为「自治软件工厂」（如 ROADMAP.md 愿景所述）的能力——工厂需要流水线、预热、工艺参数传递，而非每次从原材料开始。

### 修复方向

`persist` 包新增：
- `Seed(path string, cp Checkpoint) error`：用外部 checkpoint 初始化运行时状态
- `Fork(src, dst string) error`：克隆 checkpoint（开始新实验分支）
- `Merge(base, a, b Checkpoint) (Checkpoint, error)`：合并两个分支的信号

`forge evolve` 新增 `--seed-checkpoint PATH` 和 `--seed-memory PATH` 标志。

---

## 方向五 · 无降级执行模式 —— 全有或全无的 Phase Abort 惩罚部分进展

> **优先级**: P2（运营韧性）  
> **类别**: 运行时韧性 · 成本优化  
> **一句话**: 当前运行时只有两种状态——「全成功」和「全失败」。一个 gate 失败或一个 phase 报错，整个 workflow（甚至整个 wave）立即中止。对于需要真实 LLM 成本运行的 24h 自治系统，80% 的部分完成远优于 0% 的完全中止。

### 代码级证据

串行执行——gate 失败直接跳走或 abort：

```go
// forge-core/internal/orchestrator/orchestrator.go:343-358
// 当 gate FAIL 且无 on_fail 时，runGate 返回 error → RunFrom 立即中止
// 有 on_fail.loop_back 时跳到 target_phase 重跑
// 两者都不提供「记录失败、继续下一 phase」的选项
```

并行执行——fail-fast 取消整个 wave：

```go
// forge-core/internal/orchestrator/parallel.go（当前开发中）
// FAIL-FAST: per-wave context cancellation. When a phase fails, the wave
// context is cancelled so remaining phases abort promptly.
// ← 剩余 phase 立即取消，哪怕它们不依赖失败的那个
```

没有第三种模式：「这个 phase 的 gate 失败了，但记录失败、继续跑剩下的」。

再看 `reportConvergence`——它只读取一个来自 `gate.ProbeAll` 的 probe map，**gate 之前是否失败无关紧要**：

```go
// forge-core/cmd/forge/engine_build.go:458-473
func execEngine(...) int {
    probe, categories := probeStatuses(o.root) // ← 一次性 probe，非增量
    // ...
    reportConvergence(wf, o.root, probe, ...)  // ← 报告所有 gate 状态
    return 0
}
```

`probeStatuses` 是一次性快照。如果中间有 gate 失败，probe 可能已经包含了那个失败——但 `reportConvergence` 只是报告它，不影响退出码（方向一）。

### 边缘场景

1. **lint gate 失败但 test 通过**: 当前lint gates 失败 → 整体 workflow fail（或 loop_back）。导致 agent 重跑全部——包括已经写好的代码和测试，增加了 LLM 成本。
2. **并行 wave 中 phase A 的小 lint 问题导致 phase B(安全审计) 被取消**: 安全审计是最高价值的评审之一，却因为一个不相关的 lint 错误被跳过。
3. **24h evolve 凌晨 3 点 gate 失败**: 剩余 4 轮迭代全部取消。如果系统能记录失败、继续跑剩余迭代并汇总报告，第二天早上用户可以看到完整结果 + 已知问题，而非零进展。
4. **部分部署**: 在 monorepo 多服务场景中，服务 A 的测试失败不应阻止服务 B 的部署。

### 产品价值

这是 24h 自治运行的核心韧性问题。真实世界总有 flaky test、lint overflow、超时。一个成熟的平台应该区分：

- **致命错误**（配置错误、secret 泄露、资源耗尽）→ 立即中止
- **非致命失败**（单 gate 红、单 phase 测试失败）→ 记录 + 继续 + 最终报告

当前的行为将所有失败视为致命，这在小型开发循环中合理（快速失败、快速修复），但在无人值守的 24h 循环中不可接受——它把一次部分失败放大为完全失败，浪费了已投入的 LLM 成本和时间。

好的类比：CI/CD 的 `allow_failure` 标志。ForgeOS 缺少对应的 `allow_gate_failure` 或 `--continue-on-gate-fail` 模式。

### 修复方向

引入 `forge run --partial` / `forge evolve --partial` 模式：

1. Phase/gate 级 `allow_failure: true`（在 workflow YAML 中声明）
2. 运行时收集所有 gate 结果，而非在第一个失败时中止
3. 最终报告列出所有 PASS / FAIL / SKIP / NOT_RUN，退出码根据致命错误（而非任何失败）决定
4. `reportConvergence` 独立评估：在 `--partial` 模式下，未跑的 gate 按 SKIP 处理而非 FAIL

---

## 汇总

| # | 方向 | 类型 | 优先级 | 核心矛盾 |
|---|------|------|--------|----------|
| 1 | `forge run` 收敛 NOT MET 但始终 Exit 0 | **产品 Bug** | **P0** | 自动化契约违反：CI/CD 无法区分「正常完成」与「未收敛完成」 |
| 2 | 脊柱工作流无结构化数据契约 | 架构缺口 | P1 | `next_stage` 仅叙述不驱动，每个工作流从零开始 |
| 3 | 人工审批标记为空文件 | 合规缺口 | P1 | `os.Stat` 不是审计：无身份/时间/理由/时效/链 |
| 4 | 无 Warm Start / 工作流 Forking | 效率缺口 | P2 | 每次运行从绝对零开始，无法并行探索或低成本重放 |
| 5 | 无降级执行模式 | 韧性缺口 | P2 | 全有或全无：一个非致命 gate 失败导致整个 workflow 报废 |

其中方向一具有最高的紧急度：它是一个**产品 Bug**，而不是功能缺口——现有的契约行为直接导致自动化系统接收到错误的成功信号。
