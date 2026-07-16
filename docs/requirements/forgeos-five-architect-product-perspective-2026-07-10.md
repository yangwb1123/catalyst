# ForgeOS — 全局扫描：五个被遗忘的高价值架构方向

> **角色**: 资深架构师 + 产品经理  
> **方法**:  
> 1. 全局逐文件扫描：`forge-core/`（18+ Go 包 · ~35k LOC）、`cmd/forge`（17+ 子命令）、  
>    `harness/`（39+ 模块 · ~10.5k LOC 执法层）、`.agent/`（完整治理骨架）、  
>    `examples/`（url-shortener + go-taskd）、`.github/workflows/forge.yml`、`pi-batch.py`  
> 2. 通读 Sprint 1–31 全部演进记录、`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`、  
>    全部 ADR + DECISIONS + architecture 文档  
> 3. **差异化验证**: 在 107+ 份已有 `docs/requirements/*.md` + `docs/analysis/*.md` 文档中  
>    逐方向进行关键词 grep 确认，确保每个方向的**核心论点未被已有分析作为系统性方向展开**  
> 4. **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 或代码片段的证据、  
>    与已有覆盖的差异化证明、Edge cases 与性能考量  
> **日期**: 2026-07-10

---

## 已有覆盖全景（本文不重复）

| 已被充分覆盖的域 | 代表文档 | 覆盖密度 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/并行/wave/loop-back） | ~30 文档, ~60 方向 | 极高 |
| 韧性运行时（超时/取消/重试/护栏四维/checkpoint/resume） | ~18 文档, ~30 方向 | 极高 |
| 学习闭环（scorecard/history/converge/成本三维修正） | ~12 文档, ~15 方向 | 极高 |
| Context/Memory 层（检索/缓存/注入/memory 压缩） | ~10 文档, ~12 方向 | 高 |
| 安全合规（secret-scan/SCA 框架/risk/沙箱 v3） | ~10 文档, ~12 方向 | 高 |
| 执法器（arch-check 8 检查/function-length/circular/acceptance） | ~8 文档, ~10 方向 | 高 |
| 执行语义（原子性/幂等/因果一致性/状态回滚） | ~6 文档, ~10 方向 | 高 |
| 多仓库联邦/跨会话治理 | ~12 文档, ~12 方向 | 高 |
| 产品采纳（CLI UX/错误信息/forge approve/二进制分发/升级路径） | ~8 文档, ~15 方向 | 中-高 |
| 二阶系统问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | ~6 文档, ~10 方向 | 中 |
| 跨厂商模型池（LiteLLM v3） | ~15+ 文档提及, 无独立实现 | 概念级覆盖 |
| **其他单篇覆盖方向**（trace 查询 CLI/治理热加载/可插拔扩展/自身性能门禁等） | ~10 文档, ~12 方向 | 单篇 |

---

## 本文五个方向一览

| # | 方向 | 类别 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **跨运行 Trace 聚合与操作智能** | 可观测性 · 运维决策 | **P1** | 当前 trace 是 per-run 孤岛，无法回答「我的成本趋势如何？哪个 phase 最容易失败？」 |
| 2 | **部分失败下的工作区完整性** | 可靠性 · 数据一致性 | **P1** | Agent phase 写 N/M 文件后崩溃 → 脏工作区无快照无回滚，下一 phase 在不确定状态上构建 |
| 3 | **多厂商 Agent 输出归一化层** | 架构 · 可扩展性 | **P2** | cost.go 硬编码 claude JSON 结构。Codex/Gemini 接入需改相同文件，无厂商中立输出解析抽象 |
| 4 | **Prompt 模板内容漂移检测** | 治理 · 可靠性 | **P2** | 模板仅按 basename 存在性校验，无内容版本跟踪——同一模板不同版本可无声改变 agent 行为 |
| 5 | **自限流与准入控制** | 可靠性 · 系统韧性 | **P2** | forge-core 自身无任何限流/反压/准入控制——并发 run、恶意识入 YAML、资源耗尽无防护 |

---

## 方向一 · 跨运行 Trace 聚合与操作智能

**优先级**: 🔴 **P1** | **类别**: 可观测性 · 运维决策 | **预估**: ~1.5 sprint | **杠杆**: ⭐⭐⭐⭐

### 差异化证明

在 107+ 份已有需求分析文档中，搜索以下组合**零命中**（或命中指不同方向）：

```
$ grep -ril "trace.*aggregat\|cross.run.*trace\|trace.*trend\|trace.*history.*analysis\|trace.*diff\|diff.*trace\|trace.*compare" docs/requirements/
# → 零命中（独立方向）
```

已有分析覆盖了 `trace` 本身（Sprint 26 真点火接通 trace）和 `internal/doctor` 的 checkpoint 历史分析（仅看 5 个备份的有限启发式），但 **从未将跨运行 trace 聚合作为独立系统性方向提出**。

### 为什么需要

当前 trace 系统（`internal/trace/trace.go`）为每次 `forge run`/`forge evolve` 写一个 `trace.jsonl` 文件，包含结构化的 JSON 事件（iteration/gate/agent/decision/converge/error/overload）。但它是**完全 per-run 孤岛**：

1. **无人问津的历史** — 一个项目跑 50 次 `forge evolve`，有 50 个 `trace.jsonl` 文件，但无法回答「我的平均每次迭代成本是上升还是下降？」「哪类 phase（implementer vs. reviewer）的失败率最高？」
2. **无运行间比较** — 无法比较两次运行：「这个 PR 的 gate 失败率比上周高 3x」「这次运行的成本分布与上次不同」
3. **无长期趋势** — `internal/doctor/anomaly.go:49-88` 的 `DetectAnomalies` 只检查 5 个备份的 checkpoint（迭代号变化 / roadmap 跳跃 / 美元消耗），这是非常粗粒度的采样——丢失了每个 trace 事件中丰富的（phase 名、状态码、model 归因、精确 wall-clock 时长）
4. **学习闭环的盲区** — 当前 learning loop 只从 scorecard 学（模型路由），但**不从操作模式学**——例如某类 gate 总是失败，或某 model tier 的一致性问题

### 代码级证据

**证据 A：trace 是纯 emit-only，无 readback 聚合 API**

```go
// forge-core/internal/trace/trace.go:60-68
type Tracer struct {
    mu  sync.Mutex
    w   io.Writer    // 仅写入——一旦 Emit 出去就无人再读
    seq int
    Now func() time.Time
}
// 整个包只有 Emit/Span 两个写入方法，没有任何读取/查询/聚合接口。
// 写入的 trace.jsonl 只能被外部工具（jq/diff）手动消费。
```

```bash
$ grep -rn "trace\.\|Tracer\|trace\.jsonl\|\.forge/trace" forge-core/ --include='*.go' | grep -v "_test\|_bench\|\.pb\." | wc -l
# 大量引用 trace.Emit，零引用 trace.Read / trace.Query / trace.Aggregate
```

**证据 B：`forge doctor --anomaly` 只读 5 个 checkpoint，不读 trace**

```go
// forge-core/internal/doctor/anomaly.go:49-56
func DetectAnomalies(chain []persist.Checkpoint) []AnomalyFinding {
    // 输入是 []persist.Checkpoint（最多 5 个备份）
    // 只检查：stale / stuck-iteration / roadmap-jump / dry-run / no-progress
    // 完全不读 trace.jsonl（那里有 10+ 种事件类型、精确 duration_ms）
}
```

**证据 C：`internal/doctor/quick.go` 对 trace 只有「最后一行是否完整」检查**

```go
// forge-core/internal/doctor/quick.go:56-72
func quickTraceCheck(dotForge string) []QuickCheck {
    // 只检查 trace.jsonl 存在性 + 最后一行是否截断
    // 从不读 trace 内容做任何分析
}
```

**证据 D：所有 trace 内容只被 scorecard-update 脚本消费**

```bash
$ grep -rn "trace\.jsonl\|traceJsonl\|TRACE_FILE" forge-core/ harness/ --include='*.go' --include='*.mjs'
# harness/scorecard-update.mjs 是唯一的 trace 消费者（读 duration_ms+cost_usd_micros 写 scorecard）
# 除此之外没有任何代码读取 trace 做趋势分析
```

### Edge cases

- **海量 trace 数据**: 长期运行的 Project 可能积累 GB 级 trace。聚合需要采样/窗口/下采样策略
- **schema 演进**: trace Event 的 Format 字段（`forgeos.trace.v1`）已预留版本标识，但无向后兼容读取器
- **跨环境比较**: dev vs. CI vs. production 的 trace 格式相同但环境不同，聚合层需支持标签/过滤

### 建议方向（不写代码）

1. `forge trace` 子命令族：
   - `forge trace ls` — 列出当前项目所有 trace 会话（按日期/工作流/exit code）
   - `forge trace diff <a> <b>` — 比较两次运行的 trace 事件流差异（成本/时长/失败 phase）
   - `forge trace report` — 聚合报告：成本趋势、phase 失败率、gate 通过率随迭代变化
2. 可选的跨运行聚合存储（`~/.forge/` 全局目录或项目级 `.forge/trace.db`），由 `forge trace` 命令增量写入
3. 操作智能告警：当检测到「成本趋势上升 >20% 周环比」或「某 gate 失败率突增」时，`forge doctor` 产生建议

---

## 方向二 · 部分失败下的工作区完整性

**优先级**: 🔴 **P1** | **类别**: 可靠性 · 数据一致性 | **预估**: ~2 sprint | **杠杆**: ⭐⭐⭐⭐⭐

### 差异化证明

```
$ grep -ril "workspace.*snapshot\|workspace.*rollback\|partial.*write.*fail\|dirty.*workspace\|file.*snapshot.*agent\|phase.*atomic.*write\|pre.*write.*snap\|write.*guard" docs/requirements/
# → 零命中系统性方向
```

已有分析提及「原子工作区/隔离提交」（`novel-directions-v13.md` 方向一）和「执行语义形式化：原子性/幂等/因果一致性/状态回滚」（`execution-semantic-gaps.md`），但 **讨论的是执行语义的抽象属性，而非 agent 阶段部分写文件后工作区实际脏状态的检测与恢复机制**。本方向关注的是**现实世界的文件系统状态**——agent 写了一半文件然后崩溃，下一 phase 在一个不一致的树上构建。

### 为什么需要

当前架构有一个根本性的可靠性缺口：

> **agent phase 的写操作不可撤销**，且无论是 `forge run` 还是 `forge evolve`，都没有在任何点对工作区做快照或一致性校验。

具体场景：

1. **SIGTERM 中途杀死**: `CommandExecutor` 发送 SIGTERM → SIGKILL（`command_executor_unix.go:49`），但 agent 可能在写第 3 个文件时被杀死。`forge resume` 从 checkpoint 恢复，**不验证工作区完整性**
2. **预算耗尽中途**: `checkRunBudget` 或 `checkAgentBudget` 拒绝 spawn 前，当前 phase 可能已完成 70% 文件写入。剩余 30% 永远不写
3. **loop-back 覆盖**: `loopBackTo` 跳回 implementer 重做，但前一次 implementer 写的文件部分有效、部分无效——新的 implementer 在一个混合状态上构建
4. **并行编排下的竞态**: `parallel.go:67 RunParallel` 允许多个 phase 同时执行——如果两个 phase 写同一个文件的相邻区域，结果不可预测

### 代码级证据

**证据 A：CommandExecutor 无原子写入机制**

```go
// forge-core/internal/orchestrator/command_executor.go:101-118
type CommandExecutor struct {
    Build            func(p asset.Phase, mode string) []string
    Dir              string
    Timeout          time.Duration
    MaxDepth         int
    MaxOutputBytes   int
    // ... 没有 WorkspaceDir / Snapshot / AtomicWrite 相关字段
}

// executor.go: Exec 接口
type Executor interface {
    Exec(ctx context.Context, p asset.Phase, tier string) error
}
// Exec 返回 error，但不提供任何机制来撤销 phase 已做的文件系统修改
```

**证据 B：evolve 迭代之间无工作区校验**

```go
// forge-core/cmd/forge/evolve.go:338-380 recordMemory
func recordMemory(...) {
    // 在每轮迭代末尾写入三类 memory：
    // - gate 失败 → KindGap
    // - reviewer findings → KindLesson
    // - trajectory entry
    // 但从不校验工作区是否与 checkpoint/iteration 一致
}
```

**证据 C：`forge resume` 不验证工作区**

```go
// forge-core/internal/persist/checkpoint.go:45-62
type Checkpoint struct {
    Iteration          int
    PhaseIndex         int      // 已做：phase 级粒度
    RoadmapCompletion  float64
    GatesGreen         bool
    Mode               string
    // ... 没有 WorkspaceHash / FileManifest / DirtyFileList
}
// resume 时从 PhaseIndex 恢复，但不验证源文件是否匹配 checkpoint 时的状态
```

**证据 D：defaultAgentAllowedTools 本身承认只读自检无法发现文件损坏**

```go
// forge-core/cmd/forge/main.go:42-55
const defaultAgentAllowedTools = "Bash(node --test*) Bash(node harness/gate.mjs*)"
// 仅允许只读验证命令。如果运行 `node --test` 时工作区因上一 phase 部分写而损坏，
// test 可能通过也可能失败——但耗尽预算后无人修复脏状态
```

### Edge cases

- **git-tracked vs. untracked**: 被修改但未 commit 的文件与 agent 新生成的文件——回滚策略不同（`git checkout` vs. 删除）
- **大文件部分写入**: agent 在写大文件（如 Go 二进制、压缩包）中途被杀死。文件存在但格式非法
- **数据库/状态文件**: 如果 agent 写的是 `.forge/` 内状态文件（理论上不应发生但无防护），损坏会影响 forge 自身
- **并行 phase 的交互**: `parallel.go` 允许多 phase 同时执行，两个 phase 写同一文件的不同区域不可预测

### 建议方向（不写代码）

1. **执行前快照**: 在每个 agent phase 执行前，对工作区做轻量级快照（`git stash` 风格或文件清单 checksum）
2. **执行后校验**: phase 完成后，对比预期 emits（来自 workflow 声明 `emits:`）与实际写入文件的差异
3. **自动回滚**: 如果 phase 失败，自动恢复快照（`git checkout` 或文件级恢复）
4. **脏工作区检测**: `forge resume` / `preflight` 时检测未完成的 phase 产物（文件存在但对应 phase 未完成）并告警

---

## 方向三 · 多厂商 Agent 输出归一化层

**优先级**: 🟡 **P2** | **类别**: 架构 · 可扩展性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐

### 差异化证明

```
$ grep -ril "agent.*output.*normaliz\|vendor.*output.*pars\|cross.provider.*pars\|output.*normalization.*layer\|output.*schema.*normaliz" docs/requirements/
# → 零命中
```

已有分析覆盖了「跨厂商模型池」（LiteLLM v3，面向**模型路由**）和「Agent 输出 Schema 强制」（面向**输出格式验证**）。但 **从未将「不同 LLM 厂商的 agent 输出格式归一化」作为独立架构层提出**。本方向关注的是：forge-core 的 `cost.go` 硬编码了 claude 的 `--output-format json` 结构来解析成本/裁决，而这是接入任何非 claude agent（Codex/Gemini/OpenAI）时必须修改的**同一个文件**。

### 为什么需要

ForgeOS 的核心架构承诺是「站在 Claude Code / Codex / Gemini CLI / OpenCode / OpenHands 之上」。但当前：

1. **成本解析硬编码 claude**: `cost.go` 中 `parseClaudeCostJSON` 解析 claude 的 `--output-format json` 格式的 `total_cost_usd` 字段。Codex/Gemini 的输出格式完全不同
2. **裁决解析隐式依赖 claude 输出格式**: `parseReviewerVerdict`/`parseExecutiveVerdict`/`parseConfidenceScore` 都依赖 `unwrapClaudeResult` 函数，该函数解析 claude 特有的 JSON 信封格式
3. **`CommandExecutor.Observe` 模式正确但未用于输出解析**: `Observe` 钩子将原始输出传给调用方，调用方可解析 vendor 特定格式——但**当前只有 cost/latency 走这个钩子**，裁决解析走的是不同的 `AgentVerdict`/`costEmitter` 路径

这是一个**架构的「半抽象」**问题：`CommandExecutor` 正确地将输出流通过 `Observe` 回调暴露给调用方，但 `cmd/forge` 层没有利用这个抽象将 claude 特定的解析逻辑与通用管道解耦。

### 代码级证据

**证据 A：`cost.go` 自身承揽全部 claude 特定逻辑，且无接口抽象**

```go
// forge-core/cmd/forge/cost.go:14-20
// cost.go — the claude-specific cost-telemetry boundary of the forge CLI. ALL
// knowledge of the claude `-p --output-format json` envelope (its total_cost_usd /
// result fields) lives here, deliberately isolated from the generic runtime.
```
这段注释自称是「claude 特定边界」——它确实是。但**没有对应的 Codex/Gemini 版本的 `cost_*.go` 文件**，也没有一个 `OutputParser` 接口让不同厂商的实现可插拔。

**证据 B：`unwrapClaudeResult` 被三个解析函数硬调用**

```go
// forge-core/cmd/forge/cost.go:315-320
func parseReviewerVerdict(output string) (verdict string, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))  // ← 硬编码 claude 信封
    ...
}

// forge-core/cmd/forge/cost.go:330
func parseExecutiveVerdict(output string) (verdict string, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))  // ← 同样硬编码
    ...
}

// forge-core/cmd/forge/cost.go:345
func parseConfidenceScore(output string) (score float64, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))  // ← 同样硬编码
    ...
}
```

**证据 C：`CommandExecutor.ClassifyOverload` 是正确模式但未用于输出解析**

```go
// forge-core/internal/orchestrator/command_executor.go:95-98
type CommandExecutor struct {
    // ...
    ClassifyOverload func(output string) bool  // ← 这是「正确模式」：
    // 厂商特定的 overload 检测由调用方注入，executor 不感知 claude
}
// 但输出解析没有对应的厂商注入钩子——导致 cost.go 成为 claude 的硬编码
```

**证据 D：Agent 标识不在输出事件中，解析器无法按厂商路由**

```go
// forge-core/internal/trace/trace.go:45-50
type Event struct {
    // ...
    Model  string `json:"model,omitempty"`  // 有 model 名（如 sonnet/opus）
    // 但没有 Vendor string —— 无法区分 claude vs codex vs gemini
}
```

### Edge cases

- **输出格式演进**: claude 的 `--output-format json` 格式可能升级（如添加新字段、改变嵌套结构）。版本兼容性需纳入
- **混合厂商运行**: 一个 workflow 中不同 phase 用不同厂商（如 implementer=codex, reviewer=claude）。成本加总需按厂商独立解析
- **纯文本 fallback**: 某些 agent CLI 可能不支持 `--output-format json`（如旧版本或受限模式）。框架需支持纯文本回退 + 诚实 N/A

### 建议方向（不写代码）

1. 定义 `OutputParser` 接口（`ParseCost(output) → (costUsd, model, ok)` + `ParseVerdict(output) → (verdict, ok)` + `ParseConfidence(output) → (score, ok)`）
2. 将 `cost.go` 的 claude 特定解析提取为 `ClaudeOutputParser` 实现
3. 在 `CommandExecutor`/`engine_build.go` 中按 `--agent-cmd` 注入对应厂商的 parser
4. 在 `trace.Event` 中加 `Vendor` 字段（非 omitempty），确保厂商归属可追溯

---

## 方向四 · Prompt 模板内容漂移检测

**优先级**: 🟡 **P2** | **类别**: 治理 · 可靠性 | **预估**: ~0.5 sprint | **杠杆**: ⭐⭐⭐⭐

### 差异化证明

```
$ grep -ril "template.*drift\|template.*version\|prompt.*version\|template.*content.*check\|template.*content.*valid\|prompt.*stale\|template.*stale" docs/requirements/
# → 零命中
```

已有分析覆盖了「治理热加载与治理版本钉扎」（`forgotten-five-foundations.md` 方向二）和「Schema 版本化」（`structural-gaps-v41.md` 方向五），但 **这些聚焦在 governance YAML 的版本管理，而非 prompt 模板的内容漂移**。`.ai/prompts/` 下的模板直接注入到 agent 的系统提示中，其内容变化可**无声改变 agent 行为**，但没有任何机制检测这种漂移。

### 为什么需要

ForgeOS 有 10 个 `.ai/prompts/*.md` 模板（`00-product-discovery` 到 `09-cto-review`），由 workflow YAML 的 `uses_template` 字段引用：

```yaml
# review.yml:44
- name: security-review
  uses_template: .ai/prompts/02-security-rfc-review.md
```

当前 `forge validate --models`（`internal/doctor/models.go:55-78`）只做**存在性检查**（basename 是否在已知集合中），**从不检查模板内容是否与 workflow 声明期望的结构一致**。这意味着：

1. **无声的 agent 行为改变**: 某次 sprint 更新了 `02-security-rfc-review.md` 的 prompts 结构（如从「7 Tasks」改为「5 Phases」），所有引用它的 workflow 的 agent 行为改变——无校验、无告警
2. **跨项目漂移**: `forge-init` 从 source repo 复制 `.agent/` 资产到新项目，但**不复制 `.ai/prompts/` 模板**——多个项目的模板可能分歧
3. **模板引用不对称**: `second_template` 引用同样只做存在性检查，无内容校验

### 代码级证据

**证据 A：`EvaluateWorkflowModels` 仅做 basename 存在性检查**

```go
// forge-core/internal/doctor/models.go:130-148
func evaluateTemplateField(rel, fieldName, tmpl string, aiTemplates map[string]bool, findings *[]ModelsFinding) bool {
    if !aiTemplates[filepath.Base(tmpl)] {
        *findings = append(*findings, ModelsFinding{Level: "WARN",
            Message: fmt.Sprintf("%s — %s %q not found in .ai/prompts/", rel, fieldName, tmpl)})
    } else {
        *findings = append(*findings, ModelsFinding{Level: "PASS",
            Message: fmt.Sprintf("%s — %s %q exists", rel, fieldName, tmpl)})
    }
    return true
}
// 只检查 filepath.Base(tmpl) 是否在 aiTemplates map 中——即文件是否存在
// 不检查文件内容、结构、版本、checksum
```

**证据 B：`aiTemplates` 是简单的 basename 集合**

```go
// forge-core/cmd/forge/validate.go:35-42
func knownTemplates(root string) map[string]bool {
    promptsDir := filepath.Join(root, ".ai", "prompts")
    entries, _ := os.ReadDir(promptsDir)
    t := make(map[string]bool, len(entries))
    for _, e := range entries {
        t[e.Name()] = true  // 只收集 basename
    }
    return t
}
```

**证据 C：模板内容在运行时直接注入，无校验**

```go
// forge-core/cmd/forge/prompt_artifacts.go:15-20
// usesTemplateContext reads a .ai/prompts/*.md file and injects it verbatim
func usesTemplateContext(root, path string) string {
    data, err := os.ReadFile(filepath.Join(root, path))
    if err != nil {
        return ""  // 文件不存在或不可读 → 静默空返回（无告警）
    }
    return string(data)  // 直接注入，无结构/格式校验
}
```

**证据 D：`forge-init` 不复制 AI 模板**

```bash
$ grep -rn "ai/prompts\|\.ai/" harness/scaffold/forge-init.mjs | head -5
# forge-init.mjs 从 source 复制 GOVERNANCE_DIRS（.agent/ + harness/ + .github/）
# 但从不复制 .ai/prompts/ 目录
# 意味着新项目引用模板时，模板路径解析到 host 的模板而非 source 的模板
```

### Edge cases

- **模板内容中的占位符漂移**: 模板可能包含 `{{phase_name}}` 类占位符。如果占位符重命名，注入时静默留空
- **跨厂商模板适配**: codex 的提示格式与 claude 不同。同一模板可能对不同厂商产生不同效果
- **多语言模板**: 当未来引入多语言模板时，内容漂移检测需理解语义变化（不仅结构）

### 建议方向（不写代码）

1. 为每个模板计算校验和（SHA256），在 `forge validate --models` 中检查与基准的偏差
2. 在 `.agent/` 中加 `template.lock` 文件，锁定每个模板的期望版本/checksum
3. `forge upgrade` 扩展：检测模板漂移并提示同步
4. 模板结构契约（如「必须包含 `### Task 1` 到 `### Task 7`」），`forge validate --models` 硬检查而非仅存在性

---

## 方向五 · 自限流与准入控制

**优先级**: 🟡 **P2** | **类别**: 可靠性 · 系统韧性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐

### 差异化证明

```
$ grep -ril "rate.limit\|self.limit\|admission.*control\|backpressure\|throttle\|congestion\|load.*shed\|cooldown\|circuit.*breaker.*forge" docs/requirements/
# → 零命中
```

已有覆盖了「529/overload 退避」（Sprint 26, `backoff.go`），但这是**针对外部 LLM 服务的限流**——当 claude 返回 529 时 forge 退避重试。**forge-core 自身没有任何限流、反压或准入控制机制**，这对一个要 24h 无人值守运行的系统来说是一个缺口。

### 为什么需要

ForgeOS 被设计为 24h 无人值守自治运行。但在以下场景中，系统自身会变成不可控负载源：

1. **并发 `forge run` 调用**: 同时启动两次 `forge run` → 两个进程竞争 `.forge/` 目录 → 数据损坏或不可预测行为
2. **深度嵌套 YAML 输入**: `yaml2json` 是递归下降解析器（`mapping.go`/`sequence.go`），恶意识入或意外构造的深度嵌套 YAML 可导致栈溢出
3. **gate 风暴**: CI或触发器中一连串快速 `forge accept` 调用 → 每个都衍生多个子进程（go test / node --test / arch-check） → 资源耗尽
4. **memory 无限膨胀**: `memory.jsonl` 在 evolve 长跑中单调增长，`memoryCap = 32` 控制注入条数，但**磁盘上的文件无限增长**

### 代码级证据

**证据 A：`.forge/` 目录无互斥机制**

```bash
$ grep -rn "flock\|lockfile\|LockFile\|\.forge.*lock\|pid.*file\|sync\.Mutex\|semaphore\|semaphore" forge-core/internal/persist/ --include='*.go'
# → 零命中
# persist 包读写 checkpoint.json 无任何文件锁/进程间互斥
```

```go
// forge-core/internal/persist/checkpoint.go:75-95
func Save(path string, cp Checkpoint) error {
    data, err := json.MarshalIndent(cp, "", "  ")
    if err != nil {
        return err
    }
    return os.WriteFile(path, data, 0644)  // ← 无锁写入——并发写会交错
}
```

**证据 B：yaml2json 递归下降无栈深度保护**

```go
// forge-core/internal/yaml2json/mapping.go:15-70
// parseMapping 递归调用自身（通过 parseMultilineValue → parseSequence/parseMapping）
// 无最大嵌套深度保护
// 恶意识入 10 万层嵌套 YAML 可导致栈溢出
```

**证据 C：无并发运行检测**

```bash
$ grep -rn "pid\|PID\|process.*check\|already.*running\|existing.*run\|\.forge.*pid\|lock.*pid" forge-core/cmd/forge/main.go forge-core/cmd/forge/evolve.go
# → 零命中
# `forge run` 和 `forge evolve` 都不检查是否已有实例在运行
```

**证据 D：`scorecards.json` 和 `memory.jsonl` 无限增长无旋转**

```bash
$ grep -rn "rotate\|archive\|max.size\|max.bytes\|max.entries\|truncate\|size.*limit\|file.*limit" forge-core/internal/memory/ forge-core/internal/trace/ --include='*.go'
# → 零命中
# memory.jsonl 随 evolve 迭代单调追加，trace.jsonl 同理
# 无文件大小上限、无归档策略、无旧数据裁剪
```

**证据 E：`parallel.go` 无并发度控制**

```go
// forge-core/internal/orchestrator/parallel.go:67-80
func RunParallel(...) error {
    // 没有 MaxConcurrentPhases / Semaphore 配置
    // 所有可并行的 phase 同时启动——在大型工作流中可能同时启动 10+ agent 子进程
}
```

### Edge cases

- **CI 并发构建**: GitHub Actions 可能同时触发多个 workflow 运行，都调用 `forge run build --executor dry`——当前设计下它们会竞争 `.forge/checkpoint.json`
- **递归 gate 调用**: gate 脚本自身可能调用 `forge` 命令（虽被 defaultAgentAllowedTools 禁止，但无强制检测）
- **恶意 YAML 注入**: 如果 forge 被用来解析不受信任的 YAML 文件（如从外部源加载的 workflow 定义），递归下降解析器无深度限制
- **大项目 gate 资源冲击**: 1000+ 文件的仓库上 `arch-check.mjs` 的 `walkSource` 遍历所有文件——快速连续调用可产生 CPU/IO 脉冲

### 建议方向（不写代码）

1. **进程互斥**: 在 `forge run`/`evolve` 入口加 PID 文件 + 文件锁（`flock`），防止同一 `.forge/` 目录被并发操作
2. **YAML 解析深度门禁**: 在 `yaml2json` 的 `normalize.go` 或入口 `Decode` 处加 `maxDepth` 保护（递归调用 >128/256 层时拒绝）
3. **并发 phase 上限**: `RunParallel` 加 `MaxConcurrent` 配置（默认 4-8），防止单次运行衍生过多子进程
4. **日志/状态文件旋转**: `trace.jsonl`/`memory.jsonl`/`scorecards.json` 加大小门禁和自动旋转（保留最近 N 个文件或 N MB）
5. **gate 调用频控**: `forge accept` / `forge gate` 的快速连续调用加最小间隔或去抖机制

---

## 收敛建议

| 方向 | 优先级 | 杠杆 | 依赖 | 最佳启动时机 |
|------|--------|------|------|------------|
| ① 跨运行 Trace 聚合 | P1 | ⭐⭐⭐⭐ | 已有 trace 格式（v1） | 下一个 sprint（有用户真跑数据后） |
| ② 工作区完整性 | P1 | ⭐⭐⭐⭐⭐ | 低（纯 forge-core 实现） | 下一个 sprint（可靠性提升明显） |
| ③ 多厂商输出归一化 | P2 | ⭐⭐⭐⭐ | 跨厂商需求出现时 | 首次接入非 claude agent 之前 |
| ④ 模板内容漂移检测 | P2 | ⭐⭐⭐⭐ | 低（纯 harness 实现） | 随时可做（低成本高治理价值） |
| ⑤ 自限流与准入控制 | P2 | ⭐⭐⭐ | 中等 | 下一轮韧性增强 sprint |

**若只做一件**: **方向二（工作区完整性）**——这是当前架构中一个真实的、高影响的可靠性缺口，能在 agent 故障时保护用户工作，且影响范围覆盖所有 workflow（build/evolve/review）。

**若做三件**: 方向二 + 方向一 + 方向四 —— 分别覆盖**可靠性（工作区保护）**、**可观测性（跨运行分析）** 和**治理（模板漂移检测）**，三者在 ForgeOS 从「工程验证品」进化为「可信任的生产系统」的过程中缺一不可。

方向三和方向五：方向三是「跨厂商架构」的前提条件，但接入非 claude agent 目前在 roadmap 上是 v3——当前时机未到但架构留位关键；方向五是传统韧性工程在该系统上的自然投射，重要性会根据真实故障模式逐步显现。
