---

## 交叉验证结果

对文档中五个方向的全部核心代码引用进行了逐行核实。以下是与当前代码库（commit HEAD）的**精确比对**。

---

### 方向一 · Gate loop-back 重跑

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `orchestrator.go:176-186` — RunFrom 的 for-loop | `orchestrator.go:203-226` ✅ 代码实质一致，行号偏移 ~27 行 | ✅ |
| `orchestrator.go:346-370` — runGates 无条件重跑 | `orchestrator.go:414-435` ✅ 同上 | ✅ |
| `callGate → HarnessRunner(repoRoot, probe) → 真实子进程` | `orchestrator.go:461-465` ✅ | ✅ |
| probe-backed gate 与 `complexity`/`arch` 子进程区分 | `gatherSignals` + gate probe map 逻辑 ✅ | ✅ |

**事实确认**：循环结构精确匹配。`i = target - 1` 在 `orchestrator.go:222` 存在，语义与文档一致。Document 准确指出 gate 在 loop-back 时无条件重跑——当前 engine 确实无增量缓存。

**差异确认**：`edgecases-and-perf.md` 覆盖 evolve 跨 iteration 的 acceptance 全量重跑，但 loop-back **单 iteration 内**的 gate 重跑从未被作为独立方向分析。✅

---

### 方向二 · AgentExecutor 生命周期真空

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `orchestrator.go:56-58` — AgentExecutor 接口 | **`executor.go:21`** ⚠️ 拆分到新文件，行号不对 | ⚠️ 文件移动，内容一致 |
| `Exec(ctx context.Context, phase asset.Phase, mode string) (output string, err error)` | **`Execute(ctx context.Context, p asset.Phase, mode string) error`** — 重命名且去掉了 `output string` | ⚠️ 签名已变 |
| `orchestrator.go:38-48` — DryRunExecutor | **`executor.go:28-48`** ✅ | ✅ |
| `command_executor.go` — CommandExecutor | `command_executor.go` ✅ 不是 AgentExecutor 实现 | ✅ |

**重要事实更正**：文档使用的接口签名 `Exec(...) (output string, err error)` 已经过时——当前签名是 `Execute(ctx, p, mode) error`，不再返回 string。这个变化不影响文档的核心论点（缺少生命周期方法），但引用不准确。DryRunExecutor 确实仍是唯一实现。

**差异确认**：`execution-semantic-gaps.md` 在 phase 级别覆盖原子性/回滚，但 executor 粒度的 Init/Shutdown/Rollback/Health 生命周期契约从未被展开。✅

---

### 方向三 · Agent 输出契约验证脆弱性

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `cost.go:180-193` — parseClaudeCostUsd | `cost.go:180-198` ✅ 精确 | ✅ |
| `cost.go:287-296` — hasOverloadMarker | **`cost.go:265-270`** ⚠️ 行号偏差 ~22 | ⚠️ 内容精确 |
| `cost.go:330-341` — parseReviewerVerdict | `cost.go:330-349` ✅ | ✅ |
| `cost.go:391-402` — parseConfidenceScore | `cost.go:387-407` ✅ 精确 | ✅ |
| `cost.go:287-296` — hasOverloadMarker 含 containsToken529 | `cost.go:270-280` ✅ | ✅ |

**重要事实补充**：文档描述了 `hasOverloadMarker` 的字符串启发式扫描，但未提及 **`classifyClaudeOverload`（`cost.go:233-260`）比文档描述的更稳健**——它首先尝试 JSON envelope 解析，检查 `api_error_status == 529` 字段（精确的 HTTP 状态信号），**仅当 envelope 解析失败时才回退到文本扫描**。这意味着实际 overload 检测比文档暗示的更精确。不过，**这个补充不影响文档核心论点**——verdict/confidence 解析器的 exact-match 脆弱性完全真实。

**核心风险确认**：`parseReviewerVerdict` 的 `exact-match on last line` 机制确实脆弱：markdown 加粗、句号、额外空行都会导致静默 APPROVE。`parseConfidenceScore` 的 `Atoi` 确实拒绝 `85%`。✅

**差异确认**：`five-verifiable-code-level-gaps.md` 覆盖 memory.Confidence 零值歧义，但 agent **输出格式的 schema 缺失**（不是字段默认值）未被展开。`forgotten-five-foundations.md` 方向四（真实性闸门）是不同维度。✅

**应为 P1 优先级**（与文档一致）。

---

### 方向四 · 双 YAML 解析器静默语义漂移

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `main.go:228-259` — loadWorkflow 双解析路径 | **`main.go:353-393`** ✅ 行号偏移 ~125，内容精确 | ✅ |
| Go 解析器正常 → 立即返回，不交叉验证 | `main.go:364-371` ✅ | ✅ |
| Python shim 仅当 Go 出错时 | `main.go:374-386` ✅ | ✅ |

**事实确认**：`loadWorkflow` 的 fallback 逻辑精确匹配文档描述。两种解析器代码路径完全独立（Go: `yaml2json.Decode` → `json.Marshal` → `asset.LoadWorkflowJSON`; Python: `exec.Command("python3", shim)` → `asset.LoadWorkflowJSON`）。Go 解析器正常时**确实不交叉验证 Python shim 的输出**。没有 golden-file 测试证明 byte-identical 输出。

**边界扩展**：文档未覆盖但值得注意的一点——`yaml2json.Decode` 返回 `any`（Go 泛型 map），然后 `json.Marshal` 做一次序列化。在这个 round-trip 中 Go 的类型假设（例如 `int` vs `float64`）可能与 Python 的 yaml2json 不同。这是一个额外的漂移来源。

**差异确认**：`forge-core-five-unseen-structural-gaps.md` 方向二覆盖的是双解析器**维护成本**（工程效率），而非**正确性风险**（静默语义漂移）。方向四是真正的增量。✅

**应为 P1 优先级**（与文档一致）。

---

### 方向五 · ADDED HERE ONLY 字段漂移 ⚠️ **重大事实错误**

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `asset.go:121` — RequiresTools: "nothing in forge-core reads it yet" | `asset.go:121` — 注释存在 ✅ 但**代码已消费** | ❌ 注释过时，字段已被消费 |
| `asset.go:131` — Readonly(Phase): "nothing in forge-core enforces it yet" | `asset.go:131` — 注释存在 ✅ 但**代码已执行** | ❌ 注释过时，字段已被执行 |
| `asset.go:288` — Readonly(Workflow): "nothing in forge-core enforces it yet" | `asset.go:288` — 注释存在 ✅ 但 workflow-level Readonly 未被单独消费（被 phase-level 覆盖） | ⚠️ 部分正确 |
| `asset.go:141` — SecondaryTemplate: "nothing in forge-core reads or injects it yet" | `asset.go:141` — 注释存在 ✅ 但**代码已读取并注入** | ❌ 注释过时，字段已被消费 |
| `prompt_artifacts.go` — appendArtifactContext 不读 SecondaryTemplate | **`prompt_artifacts.go:117-132` 明确读取 SecondaryTemplate** | ❌ 事实错误 |
| `prompt_context.go:420-428` — requiresToolsGuard | `prompt_context.go:423-453` ✅ 函数存在 | ✅ |

#### 逐项更正

**1. `SecondaryTemplate`** — 文档最大的事实错误。当前代码：

```go
// prompt_context.go:350
ctx = appendArtifactContext(ctx, repoRoot, emitsFiles, p.UsesTemplate, p.SecondaryTemplate)
```

```go
// prompt_artifacts.go:94-132
func secondaryTemplateContext(repoRoot, templatePath string) string {
    return templateContext(repoRoot, "secondary_template", templatePath)
}

func appendArtifactContext(ctx []string, repoRoot string, emitsFiles []string, templatePath, secondaryTemplatePath string) []string {
    // ...
    if stc := secondaryTemplateContext(repoRoot, secondaryTemplatePath); stc != "" {
        ctx = append(ctx, stc)
    }
    return ctx
}
```

不仅有消费代码，还有**完整测试**：`prompt_artifacts_test.go:19-92`（3 个 test functions）。

**2. `Readonly`** — 已被消费且执行：

```go
// engine_build.go:161-178
func readonlyToolScope(p asset.Phase) (deny, allow string) {
    if !p.Readonly {
        return "", ""
    }
    // 当 readonly=true 时，deny "Edit Write" + 按 agent card 白名单路径 reopen
    patterns := append([]string(nil), readonlyAgentWriteScope[p.Agent]...)
    // ...
    return "Edit Write", strings.Join(specs, " ")
}
```

被 `engine_build.go:51` 和 `:101` 调用。

**3. `RequiresTools`** — 已被消费：

```go
// prompt_context.go:423-453
func requiresToolsGuard(p asset.Phase, isCommandExec, isClaude bool, allowedTools string, logln func(string), text string) string {
    if len(p.RequiresTools) == 0 {
        return text
    }
    // ... 实际 degrade-and-flag 逻辑
}
```

被 `engine_build.go:53` 调用。有测试覆盖率（`prompt_context_test.go:395-448`）。

#### 根本原因

文档**信任了 `asset.go` 中过时的 `ADDED HERE ONLY` 注释**，而没有验证消费代码是否已经实现。这些注释是 Sprint 30+ 期间的中间状态标记，但在 Sprint 31+ 中消费代码已经完成，注释未同步更新。这是文档的**方法缺陷**：依赖注释而非全量 grep 消费证据。

#### 对方向五的总体评价

| 子方向 | 文档主张 | 实际状态 | 结论 |
|--------|---------|---------|------|
| SecondaryTemplate 未被消费 | ❌ | ✅ 已被 consume（prompt_artifacts.go） | 方向不成立 |
| Readonly 未被 enforce | ❌ | ✅ 已被 enforce（engine_build.go） | 方向不成立 |
| RequiresTools 缺少 live probe | ⚠️ | 部分正确：有 requiresToolsGuard 但确实没有 live CLI probe | 部分成立但被文档夸大为「不消费」 |
| 没有 schema 校验命令 | ✅ | 确实没有 `forge validate --workflow-schema` | 唯一成立的点 |

**核心问题**：方向五的 4 个子问题中，3 个基于过时的注释而非实际代码。唯一成立的是「没有 schema 校验命令来验证字段是否被当前版本消费」——这是一个合理的治理建议，但证据基础不充分。

**建议**：方向五应重写——聚焦于「asset.go 中的 ADDED HERE ONLY 注释过期 → 声明与实现的文档不一致」而非「字段未被消费」。或者降级为 P3 hygiene 建议。

---

### 方向五 · 补充验证：`requires_tools: [web_search]` 的 live probe 缺失

文档在证据 2 中正确指出 `requiresToolsGuard` 不执行真实的工具探测，只做 `--allowedTools` 白名单的字符串包含检查。这个是**真实缺口**——`web_search` 被声明在 YAML 中，且白名单包含关键词 → 认为可用，但未实际调用 `claude --list-tools` 或类似机制验证。这个子方向是成立的，但被「不消费」的错误指控污染了。

---

## 代码引用精确性总表

| 方向 | 引用数 | ✅ 准确 | ⚠️ 小偏差 | ❌ 错误 | 准确率 |
|------|--------|--------|-----------|--------|--------|
| 一 · Gate loop-back 重跑 | 4 | 3 | 1（行号偏移） | 0 | 75%（实质 100%） |
| 二 · AgentExecutor 生命周期 | 4 | 2 | 2（文件拆分/签名变更） | 0 | 50%（实质 100%） |
| 三 · Agent 输出契约验证 | 5 | 4 | 1（行号偏差） | 0 | 80%（实质 95%） |
| 四 · 双 YAML 解析器 | 3 | 3 | 0 | 0 | 100% |
| 五 · ADDED HERE ONLY | 5 | 1 | 0 | **4** | **20%** |
| **合计** | **21** | **13** | **4** | **4** | **62%** |

四处在方向五的错误全部来自**同一个根本原因**：文档信任了 `asset.go` 中过时的 `ADDED HERE ONLY` 注释，而未确认消费代码已经实现。这些是**可避免的事实错误**——`grep -rn "SecondaryTemplate\|\.Readonly\|RequiresTools" forge-core/cmd/forge/` 就可发现。

---

## 修正后的方向五建议

方向五的「ADDED HERE ONLY 字段漂移」作为独立方向**不成立**——3/4 的字段已被消费。但存在一个真实的、文档未精确表达的治理缺口：

### 修正命题：`asset.go` 中 ADDs HERE ONLY 注释同步失效 → `forge validate --schema-consumption` 缺失

**P3 · 治理 hygiene** | 预估：0.5 sprint

**问题重构**：
1. `asset.go` 中 4 处 `ADDED HERE ONLY` 注释全部过期（消费代码在 Sprint 31+ 中已完成）
2. 没有自动化机制检测「Go struct 的 JSON tag ←→ 实际消费代码 ←→ 注释」之间的一致性
3. 没有 `forge validate` 子命令能回答「当前 forge-core 版本消费了 worklow YAML 中的哪些字段」

**修复方向**：
- 更新 `asset.go` 中过时的注释（最简单的 hygiene 修复）
- 或添加 `forge validate --consumed-fields` 来输出每个 asset struct field 的消费状态（基于代码静态分析或注册表）

---

## 整体评价

| 维度 | 评分 | 说明 |
|------|------|------|
| **论证深度** | ★★★★☆ | 方向一至四都有「当前状态→代码证据→边界情况→建议方向」的完整论证链 |
| **代码证据** | ★★★☆☆ | 21 处引用，62% 精确率。方向五的 4 处错误降低了整体可信度 |
| **差异化验证** | ★★★★☆ | 方向一至四的差异化证明经过核实均成立。方向五的差异化证明对应「被覆盖的域」成立但「本方向新增」不成立（因为本方向实证基础错误） |
| **可行性** | ★★★★☆ | 方向一(optimize)、三(schema validation)、四(cross-check golden test) 建议具体可行 |
| **结构清晰度** | ★★★★★ | 表格总结、优先级排序、sprint 估算、诚实的不做说明——结构优秀 |

### 核心发现

1. **方向三（P1）** 是最高价值的发现——verdict/confidence/cost 三条 load-bearing 路径的解析器全部基于 exact-match，无 schema 验证，无 fallback。**验证通过，风险真实**。推荐立即修复。

2. **方向四（P1）** 是真实的正确性风险——`loadWorkflow` 的双解析路径确实不交叉验证。**验证通过，风险真实**。建议至少添加一个 golden-file 回归测试覆盖两种解析器的 byte-identical 输出。

3. **方向五（P2→P3）** 基于过时的注释，3/4 的字段已被消费。**建议重写为「注释同步失效 + schema 消费验证缺失」**，降级为 P3。

4. **方向一（P2）** 和 **方向二（P2）** 验证通过，优先级合理。

### 推荐优先级调整

| 优先级 | 方向 | 调整 |
|--------|------|------|
| **P1** 🥇 | 方向三 · Agent 输出契约验证 | 保持 P1 ✅ |
| **P1** 🥇 | 方向四 · 双解析器静默语义漂移 | 保持 P1 ✅ |
| **P2** 🥈 | 方向一 · Gate loop-back 重跑 | 保持 P2 ✅ |
| **P2** 🥈 | 方向二 · AgentExecutor 生命周期 | 保持 P2 ✅ |
| **P2→P3** 🔽 | 方向五 · ADDED HERE ONLY | 降级至 P3，修正命题后可作为 hygiene 建议 |
