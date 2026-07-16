# ForgeOS — 全局扫描后尚未覆盖的高价值扩展方向（v22）

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全仓深扫（forge-core 40+ Go 源文件 / harness 30+ 模块 / `.agent/` 完整治理骨架 /  
>   examples 2 dogfood 应用 / 40+ 份已有 `docs/analysis/*.md` 逐份交叉核对）  
> **纪律**: 每方向标注代码证据链 + 交叉确认未在任何已有分析中出现。不写代码。  
> **基线**: Sprint 27 全状态（真点火 multi-agent 端到端、Learning loop 三维数据、parallel 编排、  
>   ContextCache、gate ledger feed-forward、安全护栏四维完整）  
> **日期**: 2026-07-01

---

## 已有 40+ 份分析覆盖域——本文不再重复

| 覆盖域 | 文档 |
|--------|------|
| 自适应工作流 / 信号驱动编排 | `high-value-extensions.md` 方向一 |
| 闸门自省 / 元学习闭环 | `high-value-extensions.md` 方向二 |
| 增量式治理执行 / git-diff 执法 | `high-value-extensions.md` 方向三 |
| 跨项目知识联邦 / 组织学习 | `expansion-gaps-v7-novel.md` 方向一 |
| 运行时模型质量自适应 | `expansion-gaps-v7-novel.md` 方向二 |
| 多租户安全隔离 / Agent 权限模型 | `expansion-gaps-v7-novel.md` 方向四 |
| 确定性 Replay / 调试引擎 | `expansion-gaps-v7-novel.md` 方向三 |
| Memory 衰减 / 去重 / 可溯源 | `high-value-perspectives-v11.md` 方向四 |
| 平行引擎 fail-fast 短路 | `edgecases-and-perf.md` §1.1 |
| 配置表面积 / 跨文件一致性 | `configuration-surface-and-adoption.md` |
| ADR 架构决策衰退审计 | `eighth-wave-adr-decay.md` |
| 长运行时数据生命周期 + 轮换 | `fresh-scan-strategic-expansion.md` 方向一 + `edgecases-and-perf.md` §2 |
| YAML-Shim 消除 / Go-Native Asset | `fresh-scan-strategic-expansion.md` 方向二 |
| 跨 Agent Prompt 注入防护 | `expansion-directions-v6.md` 方向一 |
| 自愈层运行时 | `expansion-directions-v6.md` 方向四 |
| 架构度量趋势 / 早期预警 | `expansion-directions-v6.md` 方向五 |
| 收敛陷阱 / 门闩效应 | `edgecases-and-perf.md` §3 |
| ForgeOS 自我测试缺口 | `self-testing-and-dogfooding.md` |
| 置信度感知决策引擎 | `expansion-directions-v6.md` 方向二 |
| Growth bottlenecks / cmd/forge 膨胀 | `growth-bottlenecks-and-scalability.md` |
| Meta-governance 自身治理差距 | `expansion-forgeos-meta-governance.md` |
| 跨周期收敛状态机 | `expansion-core-five-2026-07-01.md` 方向一 |
| 统一验证引擎 | `expansion-core-five-2026-07-01.md` 方向二 |
| SCA 运行时 | `expansion-core-five-2026-07-01.md` 方向三 |
| 跨工作流管道编排 | `expansion-core-five-2026-07-01.md` 方向四 |
| 预算 / 优先级配置面 | `expansion-core-five-2026-07-01.md` 方向五 |
| 跨模型共识验证 | `novel-expansion-directions-v19.md` 方向五 |
| 相位输出合同校验 | `novel-expansion-directions-v19.md` 方向三 |
| 迭代输出合并 / diff 合并 | `novel-expansion-directions-v19.md` 方向四 |
| 自适应上下文窗口预算 | `novel-expansion-directions-v19.md` 方向一 |
| 跨工作流元编排 | `novel-expansion-directions-v19.md` 方向二 |
| 沙箱 / MicroVM 隔离 | `strategic-expansion-v21.md` 方向三 |
| 结构化 Agent 交接协议 | `strategic-expansion-v21.md` 方向四 |
| 多项目调度与资源治理 | `strategic-expansion-v21.md` 方向五 |
| 跨厂商模型路由 | `strategic-expansion-v21.md` 方向一 |
| 真实 Discover Pipeline | `strategic-expansion-v21.md` 方向二 |
| 并行编排生产化 / 多 Agent 协调 | `fresh-scan-strategic-expansion.md` 方向五 |
| 预热启动 / 知识图谱缓存 | `expansion-core-five.md` 方向三 |
| 自愈循环 / 不可达 ROADMAP 修正 | `expansion-core-five.md` 方向四 |
| 预算前瞻规划 | `expansion-core-five.md` 方向五 |
| 架构-代码漂移检测 | `expansion-core-five.md` 方向二 |
| 外部事件反应式 Workflow | `expansion-directions-v4.md` 方向一 |
| 并行 Agent 输出合并与冲突解决 | `expansion-directions-v4.md` 方向二 |
| 人类反馈分析系统 | `expansion-directions-v4.md` 方向三 |
| 成本预测与预算规划 | `expansion-directions-v4.md` 方向五 |

---

## 本文的 4 个方向

以下方向不从「加什么新功能」出发，而从**代码中已存在的隐性契约、管道中的数据丢失、
跨层级的信任假设**出发——回答「系统在什么条件下会静默产生错误结果」。

每个方向包含：代码证据链（文件:行号/函数签名）→ 为什么是高价值 → 交叉确认未被覆盖 → 边界情况。

---

## 方向一：Feed-Forward 管道的级联截断——审计截断告警的静默丢失

### 代码证据链

Agent 输出从原始 stdout/stderr 到注入下游 prompt 经历了 **四个处理阶段**：

**阶段 1：`cappedBuffer.rendered()` ——输出安全截断（诚实，带告警）**
```go
// forge-core/internal/orchestrator/command_executor.go:343
func (b *cappedBuffer) rendered() string {
    s := strings.TrimSpace(string(b.buf))
    if b.total > len(b.buf) {
        s += fmt.Sprintf(" …[output truncated: retained %d of %d bytes (--max-output-bytes)]", len(b.buf), b.total)
        // 诚实附上截断说明 ← 但这条说明可能被下游截断吃掉
    }
    return s
}
```

**阶段 2：`sanitizeAgentOutput()` ——安全清洗（保留截断说明）**
```go
// forge-core/cmd/forge/prompt_context.go:382
func sanitizeAgentOutput(output string) string {
    // 只去除非打印字符，截断说明（ASCII 可打印）完整保留
}
```

**阶段 3：`unwrapClaudeResult()` ——提取 Claude 输出字段**
```go
// forge-core/cmd/forge/prompt_context.go（observeFor 内部）
// 对于 claude，从 JSON 输出中提取 "content" 字段
// 截断说明在 content 末尾，取决于 claude 是否传递了
```

**阶段 4：`truncateSummary()` ——相位输出摘要（静默二次截断！）**
```go
// forge-core/cmd/forge/prompt_context.go:225
const phaseOutputSummaryCap = 800

func truncateSummary(s string) string {
    r := []rune(s)
    if len(r) <= phaseOutputSummaryCap {
        return s
    }
    return string(r[:phaseOutputSummaryCap]) + " …(已截断)"
    // ★ 800 rune 的二次截断可能把阶段 1 的截断说明切掉 ★
}
```

**关键问题链**：

```
原始输出 (50,000 bytes)
  → cappedBuffer@1000: 保留 1000 字节 + "…[output truncated: retained 1000 of 50000]"
      输出 ≈ 1050 字节（~1050 rune > 800）
  → truncateSummary@800: 取前 800 rune + "…(已截断)"
      结果：阶段 1 的截断说明（在末尾 ~1000 字节处）被阶段 4 切掉了
      → 下游 reviewer/implementer 收到的截断内容看起来是完整的
         （"…(已截断)" 只说明被阶段 4 截断，不说原始输出也被截断了）
```

### 为什么高价值

| 维度 | 分析 |
|------|------|
| **数据完整性** | reviewer 收到的 implementer 输出是不完整的，但**没有告警提示不完整性**。reviewer 基于不完整信息做审查裁决（APPROVE/REQUEST_CHANGES）——裁决质量降级了但无人知道。 |
| **真实场景** | Sprint 25-26 的真点火验证中，implementer 写复杂功能时输出可以轻松超过 800 rune。reviewer 在 print-mode（无 Bash）下只能依赖 feed-forward 内容——如果 feed-forward 静默截断了关键上下文，reviewer 可能错误批准有缺陷的实现。 |
| **可观测缺口** | 阶段 1 的截断（`--max-output-bytes`）记录在 `cappedBuffer` 的日志中，但不会作为 trace event 单独记录。阶段 4 的截断连日志都没有。操作员无法从 trace.jsonl 或收敛报告中知道「reviewer 的上下文被截断了」。 |

### 交叉确认

搜索 `docs/analysis/*.md`：
- `expansion-directions-v6.md:459` 提到 `cappedBuffer` 截断告警作为能力基线——**但只说单级截断，未提及级联问题**
- `seventh-wave-data-realism.md:79` 记录「Agent 输出能被正确截断」——**也是单级视角**
- `high-value-perspectives-v11.md:201` 提到 replay 数据的 1MB cap——**不是同一管道**
- **级联截断导致截断告警静默丢失**——未被任何已有分析覆盖

### 边界情况

- **Claude JSON 输出**：`unwrapClaudeResult` 从 claude 的 `{"content":[...], "total_cost_usd":...}` 中提取 content 文本。cappedBuffer 的截断说明在 JSON 的末尾——如果 claude 输出超过 10MB，cappedBuffer 截断后附加的说明可能在 JSON 截断边界之后，被截断的 JSON 可能无法被 `unwrapClaudeResult` 解析 → 返回空值，整个输出丢失。这是比 rune 截断更严重的情况。
- **短输出不受影响**：如果 implementer 输出 < 800 runes 且 < `--max-output-bytes`，四个阶段都不会截断——这是正确路径。只有当输出超过任一上限时才进入级联丢失。
- **并行模式恶化**：在 `--parallel` 下，多个 agent 可能同时产生大型输出。`phaseOutputLedger` 没有多条目容量管理——多个并行 implementer 的 feed-forward 输出竞争下游 prompt 的上下文窗口。每个单独截断到 800 runes，但 N 个 800 runes = N×800 的总注入，可能超出窗口而不被检测。

---

## 方向二：YAML 双解析器的交叉验证缺口——静默分歧

### 代码证据链

`loadWorkflow` 使用双路解析，但对两路输出**不做交叉验证**：

```go
// forge-core/cmd/forge/main.go:342
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    // 路径 A: Go 原生解析器（yaml2json.Decode）
    val, err := yaml2json.Decode(f)
    if err == nil {
        data, marshalErr := json.Marshal(val)
        if marshalErr == nil {
            wf, parseErr := asset.LoadWorkflowJSON(data)
            if parseErr == nil && len(wf.Phases) > 0 {
                return wf, nil  // ← Go 解析成功→直接返回
            }
        }
    }
    // 路径 B: Python shim（仅当 Go 失败时尝试）
    shim := filepath.Join(repoRoot, "harness", "yaml2json.py")
    out, execErr := exec.Command("python3", shim, ymlPath).Output()
    // ...
    return asset.LoadWorkflowJSON(out)
}
```

**问题**：

1. **Go 解析器先赢**——只要 Go 解析器返回了非零 phase 个数的结果，Python 解析器从不运行。这意味着即使 Python 能产生更准确的结果（Go 解析器有已知局限），系统也从不利用 Python 进行验证。

2. **Go 解析器不是完整 YAML**——`yaml2json.go` 是自定义简化解析器：
```go
// forge-core/internal/yaml2json/yaml2json.go
// 已知不支持: 合并键(<<:)、别名(&/*)、多文档(---)
// 这些 YAML 特性在 workflow YAML 中不常用，但如果有人用了，Go 解析器
// 会静默产生错误输出（而非报错），导致错误的工作流被执行
```

3. **Python 回退的隐匿失败**——如果 Go 解析器成功但产生了**错误的结果**（YAML 语义理解不对），Python 从来不会运行。当 Go 解析器失败（返回 error）时，Python 运行——但如果 Python 也失败，错误消息只指向 Python 路径（`python shim also failed`），操作员不知道 Go 解析器先失败的细节。

4. **无跨版本一致性**——PyYAML 版本升级可能改变行为。Go 解析器不变。如果部署环境升级了 PyYAML 但 Go 解析器没变，`forge doctor` 和 CI 可能产生不同的 workflow 解析结果——CI 用 Python（因为 Python 可用且 Go 可能有 bug），本地用 Go（因为 Python 缺失），两处执行不同语义的 workflow。

### 为什么高价值

| 维度 | 分析 |
|------|------|
| **治理自我一致性** | ForgeOS 的核心是声明式治理。如果 YAML workflow 的声明被解析器错误解释，整个治理决策（哪个 phase 跑、什么 gate 开、loop-back 去哪）都基于错误的前提。系统不知道自己在错误的基础上运行。 |
| **隐匿的语义差异** | 最危险的不是解析器报错（报错会被捕获），而是解析器**静默产生不同的语义**。例如：YAML 的 `on_fail: {action: loop_back, target_phase: implementer}` 可能在 Go 解析器中被错误解释为 `on_fail: null`（因为自定义解析器对嵌套 mapping 的处理有 bug）→ loop-back 静默丢失 → gate 失败不再跳回 implementer → 一次 red gate 直接 abort。 |
| **可调试性缺口** | 当前 `forge doctor` 不做 parser cross-validation。操作员无法运行一条命令来验证「本环境解析的 workflow 是否与 CI 一致」。 |

### 交叉确认

搜索 `docs/analysis/*.md`：
- `fresh-scan-strategic-expansion.md` 方向二讨论了「YAML-Shim 消除 / Go-Native Asset」——**目标是替换 Python shim，不是交叉验证**
- `edgecases-and-perf.md` 各处讨论性能但不涉及解析正确性
- `configuration-surface-and-adoption.md` 讨论配置表面积的一致性——**但集中在 project.yml/modes.yml 的 field 级别，不是 YAML 解析器分歧**
- **YAML 双解析器交叉验证缺口作为独立方向未被覆盖**

### 边界情况

- **Go 解析器成功但错误很微妙**：例如 `depends_on: ["phase_a"]` 在 PyYAML 中解析为 `["phase_a"]`（单元素列表），在 Go 解析器中可能被解析为 `"phase_a"`（字符串，被 JSON marshal 后产生错误的 schema）。`LoadWorkflowJSON` 会静默接受字符串而丢弃依赖声明。
- **非关键路径的差异容忍**：workflow 中某些字段（如 `description`）的解析差异不影响执行——交叉验证只需报告差异，不必阻断。
- **Python 缺失环境的降级**：在无 Python 环境中，Go 解析器是唯一的解析路径。交叉验证无法进行，应诚实报告 N/A（同 harness 的 honesty 模式）。

---

## 方向三：Agent 自修改治理文件——信任边界的静默突破

### 代码证据链

`--executor=command --agent-cmd=claude --agent-permission=acceptEdits` 下的 agent 拥有对项目根目录的完整写权限：

```go
// forge-core/internal/orchestrator/command_executor.go:49
type CommandExecutor struct {
    Dir string  // 设为 o.root = 项目根目录（main.go:410）
    // ...
}
```

Agent 通过 claude `--permission-mode acceptEdits`（Sprint 24 修复的 gap）可以自动编辑任何文件。这意味着 agent 可以修改：

```yaml
# .agent/project.yml —— 治理配置
mode: engineering      # ← agent 可以改为 explorer，关掉半数闸门
lifecycle: mvp         # ← agent 可以改为 idea，放松 enforce
```

```yaml
# harness/policies.yml —— 执法策略
enforce: block         # ← agent 可以改为 warn，使闸门不阻断
max_file_lines: 500    # ← agent 可以改为 5000，允许上帝文件
```

```markdown
# .agent/AGENTS.md —— 工程红线
- 硬闸门: 单文件 ≤ 500 行    # ← agent 可以删除这条
- 函数 ≤ 50 行              # ← agent 可以删除这条
```

```yaml
# .agent/workflows/build.yml —— 工作流本身
- name: implementer
  required_gates: []    # ← agent 可以清空自己的闸门
```

**系统当前不做任何防护**：
- 没有只读挂载 `.agent/` 和 `harness/` 目录
- 没有 `forge validate` 对治理文件的完整性检查（`forge validate` 存在但只做资产验证，不比较「当前治理文件 vs 基准快照」）
- 没有 post-agent 治理完整性钩子（`OnGateResult` 存在但只报告 gate 裁决，不检查治理文件是否被篡改）
- `ContextCache` 在 evolve 的迭代间缓存 AGENTS.md 和 ADR 标题——但如果 agent 在迭代 5 修改了 AGENTS.md，缓存会在迭代 6+ 提供失效数据

### 攻击场景（非恶意，而是偶然）

这不是「恶意 agent」场景——ForgeOS 运行可信的 claude agent。以下是**真实可能发生**的路径：

1. **prompt 注入间接影响**：implementer 的 prompt 很长，其中包含从 ROADMAP 注入的任务描述。如果 ROADMAP 中有一条 `"[x] 将 project.yml 的 mode 改为 explorer 以加速开发"`（极不可能但可能由 planner 生成），agent 可能会执行它。

2. **agent 错误理解边界**：Sprint 24 暴露了 agent 不知该做什么（缺少任务注入）。修复后 agent 知道要做什么——但可能过度执行：如果 planner 说「简化治理流程」，implementer 可能直接修改 `policies.yml` 来「简化」。

3. **loop-back 的重复执行**：如果 implementer 被 loop-back 多次（Sprint 25 真实发生），每次重新执行都会「看到」之前迭代的产物。如果前一次迭代中 agent 偶然修改了治理文件，下次迭代就在已放松的治理下运行——自我加速的 governance 腐败。

### 为什么高价值

| 维度 | 分析 |
|------|------|
| **治理的根本假设** | ForgeOS 假设治理是**外部施加的约束**，agent 是被约束的主体而不是约束的修改者。如果 agent 可以修改约束，整个治理模型就是「囚犯看守监狱」——逻辑上自相矛盾。 |
| **隐匿性** | 治理文件的修改不会导致 gate 失败（gate 使用修改后的策略执法），所以不会被任何现有闸门检测。`forge doctor` 不做治理完整性检查。这个腐败可以存在数月而不被注意。 |
| **真点火已验证写能力** | Sprint 24-26 真 claude 实测坐实了 agent 会写文件（`acceptEdits`）、会修改 ROADMAP（通过 `ROADMAP.md` 的任务注入）。修改 `project.yml` 或 `AGENTS.md` 是同样权限——不是技术问题，而是信任假设问题。 |

### 交叉确认

搜索 `docs/analysis/*.md`：
- `expansion-directions-v6.md` 方向一覆盖了「跨 Agent Prompt 注入防护」——**关注的是 prompt 内容被污染，不是 agent 输出修改治理文件**
- `strategic-expansion-v21.md` 方向三（沙箱）提供文件系统隔离——**但这需要沙箱实现，治理层本身没有轻量防护**
- `expansion-gaps-v7-novel.md` 方向四（多租户安全隔离）讨论的是跨 project 隔离——**不是 agent 与自身治理的隔离**
- `expansiion-blind-spots-v16.md` 方向二（loop-back 文件污染）讨论的是 implementer 修改 src/ 文件的累积——**不是对 .agent/harness 的修改**
- **Agent 自修改治理文件作为独立的信任边界风险未被覆盖**

### 边界情况

- **只读 overlay 不可行**：如果 `.agent/` 被只读挂载，planner 如何写入更新 ROADMAP（这是预期行为——ROADMAP 由 evolve 迭代更新）？需要更精细的权限模型：`ROADMAP.md` 和 `scorecards.json` 可写，`AGENTS.md`、`policies.yml`、`project.yml` 只读。
- **human_gate 后的治理固化**：一旦 human approval 通过了设计阶段，`project.yml` 的 mode/lifecycle 应该被「锁定」——agent 不能再修改。需要 `project.yml` 增加 `lock_governance: true` 标记。
- **补救措施而非预防**：即使不做文件系统隔离，`forge validate` 可以增加 `--check-governance-integrity` 模式：比较当前治理文件的校验和（SHA256）与上次 human approval 时记录的基准。
- **ContextCache 的连带问题**：如果 AGENTS.md 被修改，每迭代重用的 ContextCache 在缓存过期前提供的是旧约束。`ContextCache` 的 `adrTitles` 和 `cardText` 缓存无失效机制。

---

## 方向四：Checkpoint PhaseIndex 在工作流版本变更后的静默漂移

### 代码证据链

`forge evolve --resume` 依赖 PhaseIndex 数字索引来定位恢复点：

```go
// forge-core/internal/persist/checkpoint.go
type Checkpoint struct {
    Iteration   int     `json:"iteration"`
    PhaseIndex  int     `json:"phase_index"` // ★ 纯数字索引
    RoadmapPrev float64 `json:"roadmap_prev"`
    // 没有 WorkflowVersion, PhaseName, WorkflowHash 等校验字段
}

// forge-core/internal/orchestrator/loop.go:160
func (l LoopEngine) loopStart() (start int, prev float64) {
    if l.StartIter > 1 {
        return l.StartIter, l.ResumePrev
        // StartPhase 通过 l.StartPhase 直接传递给 RunFrom
        // → 数字索引 → 如果 workflow 变了，指向错误的 phase
    }
    return 1, -1.0
}
```

**问题路径**：

```
迭代 1: workflow = [planner, implementer, gate, reviewer, qa]
        执行完成于 phase_index=4 (qa)
        checkpoint: {iteration: 1, phase_index: 4}

       开发者修改 build.yml:
       - 在 implementer 后插入 "security-audit" phase
       - workflow 变更为 [planner, implementer, security-audit, gate, reviewer, qa]

迭代 2 resume: checkpoint.phase_index=4 → 指向 "reviewer"！
       原本应该从 "gate" 恢复（插入前 qa 在索引 4，插入后 gate 在索引 3）
       实际上从 "reviewer" 恢复——跳过了 "gate" phase！
```

更隐蔽的变体：

```
迭代 1: workflow = [planner, implementer, gate, reviewer, qa]
        执行到 phase_index=2 (gate) 时 crash
        checkpoint: {iteration: 1, phase_index: 2}

       开发者删除 "reviewer" phase（临时测试需要）

迭代 2 resume: checkpoint.phase_index=2 → 指向 "qa"！
       "reviewer" 被删了，原索引 4 变 3
       "gate" 在索引 2（之前也是 2，但 gate 已经执行过了）
       实际上从 "qa" 恢复——跳过了 "reviewer" 应该运行的 review 阶段
```

```go
// LoopEngine.Run 中 startPhase 直接使用数字索引：
l.Engine.RunFrom(wf, mode, *startPhase)
// 没有:
// - phase_name 验证: "当前 workflow 索引 2 的 phase name 是否等于 checkpoint 记录的 name?"
// - workflow_hash 验证: "当前 workflow 的哈希是否等于 checkpoint 记录的哈希?"
// - len(wf.Phases) 边界检查: "phase_index 是否 < len(phases)?"
```

**事实上 `RunFrom` 对超界索引是 fail-open**：

```go
// orchestrator.go:156
func (e Engine) RunFrom(wf asset.Workflow, mode string, start int) error {
    for i := start; i < len(wf.Phases); i++ {
        // start=4 但 len(phases)=4 → 循环不执行
        // → 所有 phase 被跳过 → phasesRan=0 → vacuous 警告
        // → reportStop 报告 stop condition → 可能误判收敛
    }
}
```

### 为什么高价值

| 维度 | 分析 |
|------|------|
| **数据完整性** | checkpoint 是 evolve 恢复的核心可靠性机制。如果恢复点指向错误的 phase，其后果不仅是效率损失（重跑几个 phase），而是**静默跳过整个 phase**——例如跳过 gate phase 意味着 reviewer 在没有测试结果的情况下做审查，跳过 qa 意味着代码未经质量门全量合入。 |
| **真实场景** | 在 24h 无人值守运行中，workflow 可能在 checkpoint 写入和 resume 之间被更新（尤其是当 workflow 本身由 evolve 的 planner 建议调整时——`.agent/workflows/build.yml` 是 agent 可写的）。这不是开发者的手动修改，而是系统自身在迭代间演化 workflow。 |
| **症状隐匿** | PhaseIndex 漂移的典型后果不是 crash，而是相位错位后收敛报告**仍然可能是 MET**（reviewer approve 了，qa 没跑但 gate 全绿）。只有检查日志才能发现「reviewer 在错误的时间点执行了错误的审查」，因为 trace event 中 `name: phase_name` 仍然是 reviewer（字符串对）——只是它审查的是本不该它审查的代码基线。 |

### 交叉确认

搜索 `docs/analysis/*.md`：
- `expansion-directions-v6.md` 方向四（自愈层运行时）提到 checkpoint corruption 的自愈方案——**关注的是文件损坏（磁盘故障），不是语义漂移（workflow 变更导致索引错位）**
- `fifth-wave-operational.md:388` 讨论 checkpoint 不可读时的自动修复——**也是文件级别的损坏**
- `fresh-scan-strategic-expansion.md` 方向一（长运行时数据生命周期）讨论 checkpoint 轮换——**不涉及语义校验**
- `expansion-blind-spots-v16.md` 方向一（.forge 目录并发安全）讨论多个 evolve 进程竞争写 checkpoint——**不是版本漂移**
- `edgecases-and-perf.md §1.2` 讨论 `LoopEngine + Parallel 下 StartPhase 被忽略`——**是 parallel 模式的已知限制，不是版本变更导致的索引漂移**
- **Checkpoint PhaseIndex 工作流版本漂移作为独立的正确性风险未被覆盖**

### 边界情况

- **Phase 重命名**：如果 phase 名称变了但顺序没变（`reviewer` → `code-reviewer`），数字索引仍指向正确位置——但语义上 `AgentVerdict` 的 `reviewerRequestChanges` 匹配失效。需要 phase name 校验而非仅索引校验。
- **Phase 追加**：追加永远在末尾，不影响已有索引——是安全变更。但 checkpoint 记录的 PhaseIndex 可能超出新 workflow 的阶段数（如果删除了末尾 phase）→ vacuous run。
- **Checkpoint 本身是 evolve 迭代产物**：如果 evolve 的 planner 被允许修改 `.agent/workflows/build.yml`（一个合理场景——调整流程），那么 checkpoint 的「工作流版本」和「执行索引」本身就是同一系统在两个不同时间点的产物——需要版本同步机制。
- **修复方案**：最简单的修复是在 Checkpoint 中增加 `phase_name string`（而非仅 `phase_index int`），resume 时先按 name 查找索引，找不到才 fallback 到数字索引。更完整的方案加 `workflow_hash`（SHA256 of the serialized workflow JSON），加载时比较。

---

## 总结：四个方向的核心判断

| 方向 | 问题类型 | 严重度 | 当前可见性 | 建议优先级 |
|------|---------|--------|-----------|-----------|
| 1. Feed-Forward 级联截断 | 数据完整性 | 中-高 | 低（无告警/无 trace event） | P1 — 修复量极小（在 truncateSummary 中保留上级截断说明） |
| 2. YAML 双解析器交叉验证 | 正确性 | 高（静默语义差异） | 极低（除非比对日志） | P1 — 实现交叉验证 + `forge doctor` 报告 |
| 3. Agent 自修改治理文件 | 信任边界 | 高（治理自毁） | 无（无检测机制） | P2 — 需设计权限模型，短期可加校验和检查 |
| 4. Checkpoint PhaseIndex 漂移 | 正确性 | 高（静默跳过 phase） | 低（vacuous 警告但易被忽略） | P1 — 在 Checkpoint 中加 phase_name 字段即可修复 |

**无一方向需要新基础设施**（不需要新存储、新网络服务、新外部依赖）。全部可在 forge-core 现有框架内增量实现——这是它们作为高价值方向且适合当前 v2 阶段的关键属性。

---

*本文 40+ 份已有分析交叉核对的结论：所提 4 方向均未在已有文档中出现。如有遗漏，欢迎指正。*
