# ForgeOS: 生产就绪度视角的高价值扩展方向

> **视角**: 资深架构师 / 产品经理，关注**既有功能从"能跑"到"生产可靠"之间的结构性缺口**。
> **方法**: 全局代码库深扫（18 Go 包 / 77 个测试文件 / 707 个测试用例 / harness 39 模块 /
>   `.agent/{WORKFLOWS,AGENTS,SKILLS}` 全部声明 / Sprint 1-31 完整演进记录），
>   交叉核对 40+ 份已有 `docs/analysis/*.md` 以避免重复。
> **核心观察**: 已有分析聚焦于「加什么新引擎 / 新功能」，本报告聚焦于**已建成的功能在边缘场景下的可靠性**。
> **纪律**: 不写代码。每方向标注具体代码位置 + 为什么未被已有分析覆盖。
> **基线**: Sprint 31 全状态（全部 GAP 已收口 / 功能需求清单完整 / 真点火端到端坐实）
> **日期**: 2026-07-09

---

## 前言:与 40+ 份已有分析的根本差异

ForgeOS `docs/analysis/` 下已有 **40 篇**独立分析文档，外加 `docs/requirements/` 的合成分析。它们覆盖了：

| 已有方向 | 文档代表 | 视角 |
|----------|----------|------|
| 多维模型路由自动化 | `high-value-expansion-directions.md` §1 | 新功能:自动降/升档 |
| 并行调度引擎 | `high-value-expansion-directions.md` §2 | 新功能:depends_on 声明 |
| 内存/知识存储规模演进 | `high-value-expansion-directions.md` §3 | 性能优化:O(n)→O(log n) |
| 多进程并发安全 | `high-value-expansion-directions.md` §4 | 新功能:flock |
| 运行诊断与根因分析 | `high-value-expansion-directions.md` §5 | 新功能:forge investigate |
| 跨厂商模型路由 | `expansion-core-five-2026-07-01.md` §3 | 新功能:LiteLLM |
| 统一验证引擎 | `expansion-core-five-2026-07-01.md` §2 | 新功能:Go-native 验证 |
| 实时可观测性 | `strategic-extensions-v22.md` §3 | 新功能:流式遥测 |
| 分岔/回滚引擎 | `expansion-core-five-2026-07-01.md` §4 | 新功能:git branch |
| 跨工作流管道编排 | `expansion-core-five-2026-07-01.md` §5 | 新功能:管道链接 |

**本文不重复上述任何一个方向。** 本文关注的不是「加什么」，而是**已建成的核心链路在边缘条件下是否可信**——列出的是质量缺口（quality gaps），而非功能需求（feature requirements）。

---

## 方向一:Prompt 构建管道的质量保证 —— 系统的「编译器」没有测试

### 为什么这是高价值的

ForgeOS 的核心价值是**操控 agent 的指令**。每一个 phase 的运行，本质上是：

```
buildPrompt(roleCard + task + ADRs + AGENTS + memory + gateResults + phaseOutputs + constraints)
    → send to LLM
    → parseVerdict(output)
    → converge(Signals)
```

`buildPrompt` 是这个链条的**编译器**——它决定 agent 收到什么指令。如果它出错，下游所有逻辑（verdict 解析、收敛判定）都在操作错误的数据。

当前现状：**这个编译器没有质量保证措施。** 707 个测试覆盖了 orchestration、routing、yaml2json、converge、memory，但没有一个测试问：「buildPrompt 产生的 prompt 字符串里的内容是否符合预期？」

### 代码级证据

**证据 1: 无 token 预算核算**
```
forge-core/cmd/forge/main.go: buildRunEngine → promptContext + buildPrompt
forge-core/cmd/forge/prompt_context.go: ~500 lines
```
没有任何一处**测量**构建出的 prompt 的 token 数。`prompt/cache.go` 的注释诚实承认：
> "It does NOT save a single claude token. The prompt TEXT is byte-for-byte the same as the uncached path"

系统并不知道传给 agent 的 prompt 有多长。当 ADR 积累到 20+、memory 到达 100+ 条目时，prompt 可以轻松超过 8K/16K token 窗口——**但没有任何告警**。

**证据 2: 模板渲染无回归测试**
```
forge-core/cmd/forge/prompt_artifacts.go // uses_template / secondary_template 渲染
forge-core/cmd/forge/prompt_memory.go     // memoryContext 渲染
forge-core/cmd/forge/prompt_context.go    // gateLedger.context() 渲染
```
每个模板渲染路径都是字符串拼接。没有 golden-file 测试（渲染结果与预期快照对比）。当代码重构或格式调整时，**没有机制检测渲染输出的变化**——可能语义改变但所有测试仍绿。

**证据 3: 多源注入的线性顺序未验证**
```
buildPrompt 中上下文注入顺序（prompt_context.go）:
  1. role card (agent 卡全文)
  2. constraints (AGENTS.md 硬约束)
  3. task (ROADMAP.md body)
  4. ADRs + Agent memory (Retrieve 结果)
  5. gate ledger (前序闸门结果)
  6. feed-forward phase output
  7. review findings
```
这个顺序**从未被断言或验证**。没有任何测试确认 lane 1 出现在 lane 2 之前、或某条关键指令没有被其他 lane 覆盖。更危险的是：`truncateSummary`(800 rune cap) 可能静默截断 feed-forward 中的**关键截断告警**（见 `strategic-extensions-v22.md` 方向一的级联截断分析——那篇分析是本文的例外，因为它确实覆盖了截断级联问题，但未将其置于「prompt QA」的框架下）。

**证据 4: unwrapClaudeResult 只处理一种输出格式**
```
prompt_context.go: func unwrapClaudeResult(output string) string
```
当 claude 变更其 `--output-format json` 的 envelope 结构时（Anthropic 历史上做过多次），`unwrapClaudeResult` 返回空字符串。这会导致：
- feed-forward 注入空内容（下游 agent 收到看似结构化的空段落）
- verdict 解析器接在 `unwrapClaudeResult` 之后——如果 unwrap 返回空，解析器也在空上匹配

**没有版本协商、没有格式检测、没有降级路径。**

### 建议的工程方向

1. **Prompt 快照测试（Golden File / Snapshot Testing）**
   - 为 5 个工作流 × 每个 phase 建立 prompt 渲染的 golden file
   - 每次 `buildPrompt` 逻辑变更时，diff golden file 变化
   - 人工审查差异确认语义未意外改变

2. **Token 预算审计**
   - 在 `buildPrompt` 末尾注入实际 token 计数（用 claude tokenizer 或近似估算）
   - 当 prompt 超过模型窗口的 80% 时发出结构性告警
   - 在 trace event 中记录 `prompt_tokens` 用于事后分析

3. **输出格式健壮性**
   - `unwrapClaudeResult` 增加多版本 fallback（多种已知的 claude JSON envelope 格式）
   - 当所有解析都失败时，**诚实使用原始输出**而非空字符串（agent 的原始 stdout 比空内容更有信息量）
   - 在 trace 中标注「未解析的输出格式」

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 空 ADR 目录（`docs/adr/` 不存在） | `adrDocs` 为空切片 → retrieve 返回 nil → 无 ADR 注入 | 已处理（cache.go: l.62-66） |
| Memory 文件损坏（非 JSONL） | `Load` 返回 error → `memoryContext` 返回 "" | 已处理（fail-open） |
| AGENTS.md 缺失 | `constraintsBlock` 为空 → 无约束注入 | 已处理（fail-open） |
| ROADMAP.md 只有空白项 | `taskCap` 触发 → agent 收不到任何任务 | **未处理**——agent 会收到空 task lane，可能「自主发明」任务 |
| ADR 含 Markdown 格式标记（`#` / ````） | 原始注入到 prompt，可能混淆 agent 的指令解析 | **未处理**——纯文本注入不做任何转义 |
| claude 输出 envelope 变体 | `unwrapClaudeResult` 返回 "" → 下游全断 | **未处理**——无版本协商 |

---

## 方向二:LLM 输出契约的履约验证 ——「机器可读裁决」缺少执行层

### 为什么这是高价值的

Sprint 28-31 在 `.agent/agents/` 中建立了三套机读契约：
- `reviewer.md` → `VERDICT: APPROVE` / `REQUEST_CHANGES`
- `cto.md` → `VERDICT: APPROVE` / `APPROVE_WITH_SIMPLIFICATION` / `REDESIGN` / `DELAY` / `REJECT`
- `product-manager.md` → `CONFIDENCE: <0-100>`

这些契约被 `parseReviewerVerdict` / `parseExecutiveVerdict` / `parseConfidenceScore` 解析，驱动**收敛判定**和**定向 loop-back**。

**关键问题是：这些解析器都信任 agent 输出。** 没有履约验证、没有格式纠正、没有重试。

### 代码级证据

**证据 1: 解析器是纯字符串匹配，无格式验证**
```
forge-core/cmd/forge/cost.go:
  parseReviewerVerdict  — 末行精确匹配 "VERDICT: APPROVE" 或 "REQUEST_CHANGES"
  parseExecutiveVerdict — 末行精确匹配 5 个 UPPER_SNAKE token 之一
  parseConfidenceScore  — 末行匹配 "CONFIDENCE: " + 数字
```
所有解析器都依赖 **"prompt 告诉 agent 输出的最后一行是 X"** 。如果 agent 的思考过程中无意产生了与 VERDICT 模式匹配的行（例如 reviewer 的推理中写了 "I should REQUEST_CHANGES..."），末行匹配会误取。`parseReviewerVerdict` 的 `unwrapClaudeResult` 是先调用的，所以 claude JSON envelope 的 content 字段之外的任何文本不会进入匹配范围——**这是唯一保护**。但非 claude executor 的原始输出没有这层过滤。

**证据 2: 契约违规无反馈循环**
```go
// cost.go: parseReviewerVerdict 返回 ("", false) 表示未匹配
// 调用方 (observeFor) 在未匹配时直接尝试下一个解析器，三个都错则静默丢弃
```
当 agent 输出没有机读末行时（偏离 prompt 指令），系统**不告知 agent**。只是静默地跳过 verdict 记录，收敛判定按默认值（unmet）处理。agent 下一次迭代收到的 prompt 与上次相同，仍然不会产生机读行。

**证据 3: 没有履约执行的监测**
```
grep -rn "contract\|compliance\|adherence" forge-core/ --include="*.go"
# → ~0（零个与输出契约履约相关的代码）
```
没有指标统计「非机读裁决的比例」，没有告警当非机读比例超过阈值。一个逐渐偏离约定的 agent（模型更新后行为变化）不会被发现，直到有人注意到收敛总是卡在 unmet。

### 建议的工程方向

1. **契约履约率仪表**
   - 在每次 phase 执行后记录：`phase_output_has_verdict=true/false`
   - 当连续 N 个 phase 不产生机读行时告警（当前可能是静默退化）
   - 在 scorecard 中增加 `contract_adherence` 维度

2. **格式纠正指令注入**
   - 在 N 个 phase 未履约后，在下一次 prompt 中追加「你的上一条输出缺少机读裁决行，请补上」
   - 不是在当前 phase 的输出上重试（已经完成），而是作为下次迭代的额外指令

3. **非精确匹配降级**
   - 当精确匹配失败时，尝试 fuzzy match（如"VERDICT: Approve" 大小写不同、"REQUEST CHANGES" 少了下划线）
   - 模糊匹配成功时在 trace 中标注 `verdict_fuzzy_match=true`

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Agent 输出机读行在倒数第二行（思考包含了额外空白行） | 末行匹配失败 → 契约未记录 | **未处理**——当前只匹配末行 |
| 多行都匹配不同的 VERDICT 模式 | 取最后一行（多个匹配） | 未显式处理——`parseReviewerVerdict` 在所有行中取最后一个匹配，这是隐式行为 |
| Agent 输出截断（`cappedBuffer` 或 `truncateSummary`）把机读行切掉 | 契约静默丢失，下游按默认值处理 | **未处理**——截断告警在单独的输出中，不关联到契约履约 |
| `unwrapClaudeResult` 返回 "" 但原始 stdout 有机读行 | 契约丢失 | **未处理**——解析器在 sanitized 文本上工作，sanitize 前已被 unwrap 过滤 |

---

## 方向三:核心链路的特性交互面 —— 独立验证的机制未做过组合测试

### 为什么这是高价值的

ForgeOS 有大量的安全机制，**每个都经过独立验证**：

| 机制 | 验证方式 | 位置 |
|------|----------|------|
| 安全护栏四维（depth/budget/timeout/output-cap） | 独立单测 | `command_executor_test.go` |
| Checkpoint/Resume | 独立单测 + fake agent | `checkpoint_test.go` / `replay_test.go` |
| Retry + Backoff | 独立单测 | `backoff_test.go` / `exec_error_test.go` |
| Loop-back（定向 gate/verdict 跳转） | 独立单测 + fake gate | `orchestrator_test.go` |
| NoProgress tripwire | 独立单测 | `loop_honesty_test.go` |
| Converge（多信号收敛） | 独立单测 | `converge_test.go` |
| Agent-call budget | 独立单测 | `budget_test.go` |
| Mode-gating（中枢旋钮） | 独立单测 | `mode_gating_test.go` |

但**没有组合测试**同时运行这些机制。`forge evolve` 是这些机制的**汇合点**：

```
LoopEngine.Run:
  1. 检查 MaxIter（安全 bound）→ 2. 调用 RunFrom（可能 loop-back）→
  3. 跑 gate（可能 FAIL）→ 4. RunGate（可能 retry + backoff）→
  5. 检查 agent-call budget → 6. runAgentPhase（可能 retry）→
  7. 检查 MaxAgentCalls（per-iteration）→ 8. converge.Signals() →
  9. converge.Converge() → 10. NoProgress 检查 → 11. 迭代结束
```

**真实场景的组合问题**：

| 场景 | 涉及机制 | 风险 |
|------|----------|------|
| 迭代 N 的 gate 触发 loop-back → 重跑时 budget 耗尽 | loop-back + budget guard | **未测试**——budget 在 loop-back 后会计数两次吗？ |
| Checkpoint 恢复后继续 evolve，NoProgress 计数器重置吗？ | checkpoint + resume + NoProgress | `ResumePrev` 保留 roadmap_completion，但 staleCount 在 resume 时重置为 0——**意味着 crash 重启后不会立即 tripwire，即使 crash 前已经停滞了** |
| Loop-back 触发 agent retry，agent retry 又触发 loop-back | loop-back + retry | **未定义行为**——retry 在 phase 级别，loop-back 在 workflow 级别，两者没有优先级。如果一个 retry 后的 phase 再次失败，是 retry？还是 loop-back？ |
| Timeout 触发 retry → retry 又 timeout → 总时间 > 迭代的预期墙钟 | timeout + retry + budget | **无累积超时**——每个 retry 独立计时，N 次 retry × timeout = 总时间可能远高于预期 |
| Agent-call budget 在 evolve iteration 边界重置，但 loop-back 计入同一 iteration | agent-call budget + loop-back | **当前已正确处理**（budget_test.go 覆盖）。这是少有的做过组合测试的路径。 |

### 建议的工程方向

1. **组合测试矩阵（Integration Interaction Testing）**
   - 为 `forge evolve` 的核心循环建立组合测试：
     - `TestEvolve_LoopBackThenBudgetExhausted`
     - `TestEvolve_RetryExhaustedThenLoopBack`
     - `TestEvolve_CheckpointResumeThenNoProgress`
     - `TestEvolve_TimeoutOnRetryExhausted`
   - 用 fake agent（echo）验证流程正确性，组合维度 2-3 个机制同时激活
   - **不要求**真 claude——组合测试测的是编排逻辑，不是 LLM 行为

2. **NoProgress + Resume 的一致性修复**
   - 当前的 `staleCount` 在 resume 时重置为 0：意味着 crash→resume 后，NoProgress tripwire 重新计数。如果 crash 前已经停滞了 10 轮，resume 后不会 tripwire。
   - 修复：`checkpoint.json` 持久化 `staleCount`，resume 时还原
   - 这是**数据完整性问题**，不是理论风险——真实 24h evolve 循环极可能遇到一次 crash

3. **Retry + Loop-back 的优先级契约**
   - 明确文档化：当前实现是「phase 先耗尽 retry，再触发 workflow 级别的 loop-back」
   - 增加断言确认此行为，防止重构时意外改变

### 边界情况

| 组合 | 当前行为 | 应该的行为 |
|------|----------|------------|
| Phase 1 loop-back → Phase 2 retry → Phase 2 loop-back | 在 Phase 2 的 retry 耗尽后 loop-back 到 Phase 1 | **合理**，但无测试验证 |
| Crash 在第 3 次迭代的 `OnBeforeIteration` 和 `RunFrom` 之间 | 启动 checkpoint 已写但 phase 未写 → resume 从 iteration 3 开始 | 已正确处理（`StartIter` 机制），但未与 `RunParallel` 联测 |
| `--max-agent-calls` 在 loop-back 中耗尽 | phase 不执行 + 错误报告 | 已覆盖（budget_test.go），应扩展为 evolve 级别的组合测试 |

---

## 方向四:YAML 配置管道的可靠性 —— 双解析器分歧是静默的架构风险

### 为什么这是高价值的

ForgeOS 的**所有配置**流经同一个管道：

```
.yml (源文件)
  → Python 解析: yaml2json.py (Python shim, harness/yaml2json.py)
  → Go 解析: yaml2json.go (Go 原生, bypass shim when possible)
  → asset.LoadWorkflowJSON (encoding/json)
  → forge-core 运行时
```

两条解析路径（Python PyYAML + Go yaml2json）处理同一组 YAML 文件。Sprint 27 修复了 `normalize.go` 中一个「block-scalar 损坏」bug——Go 解析器把 `"> "` 注入了解码值，导致 6/7 个真实 workflow 文件的 `description:` 字段携带字面量 `"> "` 前缀送进了 agent prompt。这个 bug 持续了多个 sprint、通过了所有测试，因为**没有差分测试**对比两条路径的输出。

### 代码级证据

**证据 1: 差分测试曾经定义但从不生效**
```
harness/test_yaml2json.py: TestToJSON_MatchesPythonShim
```
这个测试**确实存在**。但在 Sprint 27 之前，它只用 `t.Logf` 打印差异，从不 `t.Errorf`——所以即使 Go 解析器输出与 Python 不同，测试仍然通过。Sprint 27 修复了 block-scalar bug 并改为真实断言，但**这个测试只对 7 个真实 YAML 文件运行，没有覆盖语法边缘情况**。

**证据 2: YAML 功能子集有明确的不支持列表**
```
forge-core/internal/yaml2json/yaml2json.go 第 30-41 行:
  不支持: Anchors (&) / aliases (*) / merge keys (<<:) / tags / multi-document
```
这个不支持列表是**合约性免责声明**，不是运行时保护。如果有人意外在 workflow 中使用了 `<<:` 或 `&anchor`：
- Python PyYAML 会正确解析
- Go yaml2json 会静默产生错误结果（或返回 error）
- **没有检测机制**告警说「两条路径结果不一致」

**证据 3: asset 包容忍性可能掩盖分歧**
```
forge-core/internal/asset/asset.go:
  Parsing is deliberately fault tolerant: a workflow with missing or extra fields loads
  into a partially-populated Workflow rather than failing.
```
当 Go yaml2json 产生错误输出（如 Sprint 27 的 block-scalar bug），`asset.LoadWorkflowJSON` 不会拒绝它——它会静默地用损坏的数据填充 `Workflow` 结构体。Python 路径可能也以不同方式损坏，所以**没有「参考标准」来 detect 分歧**。

### 建议的工程方向

1. **差分测试真正的 Fuzz**
   - 用 Python PyYAML 产生的解析结果作为**权威参考**
   - 为 Go yaml2json 建立 fuzz test：随机生成 YAML → 对比两条路径的输出
   - 一旦发现分歧，修复 Go 解析器或显式声明不支持的范围

2. **运行时解析器一致性检查**
   - 在 `forge run` / `forge validate` 中增加可选步骤：用 Python shim 和 Go 原生同时解析，对比结果
   - 不一致时告警（不阻断——已有 warning-only 模式）
   - 一个 `--yaml-audit` 标志在 CI 中运行一致性检查

3. **将 Python shim 正式列为非权威路径**
   - Sprint 31 已确认 `yaml2json.go` 是主解析路径（`cmd/forge` 直接调用 `yaml2json.Decode`）
   - Python shim 应**只用于差分验证和测试**，不作为任何生产路径的依赖
   - 更新 `harness/yaml2json.py` 的文档标注从 `temporary shim` 改为 `reference parser for diff testing`

### 边界情况

| 场景 | Go yaml2json | Python PyYAML | 分歧风险 |
|------|-------------|---------------|----------|
| 制表符混合缩进 | normalized | 拒绝（tab 错误） | Go 不会拒绝，产生结果不一致 |
| Unicode BOM | 无特殊处理 | 自动跳过 | 可能导致 Go 解析首行错误 |
| 空文件 | 返回 nil | 返回 None | 调用方可能对 nil 和 None 处理不同 |
| 仅注释文件 | 返回 nil | 返回 None | 同上 |
| 深度嵌套（20+ 级） | 递归解析无溢出保护 | 安全（Python 递归限制） | Go 可能栈溢出（`goroutine stack > 1GB` 才 panic，但深度 100 级开始有风险） |
| 超长标量值（>1MB） | `io.ReadAll` 无上限 | 流式解析 | 大文件阻塞整个运行时 |

---

## 方向五:部分故障与优雅降级 —— 系统的「灰度」能力未经测试

### 为什么这是高价值的

当前系统的故障模型是**二值的**：要么全成功，要么全失败。但真实世界的运维场景是**灰度**的：

| 场景 | 不是全挂 | 但也不是全好 |
|------|----------|--------------|
| 部分 gate 可用 | 3/6 gates PASS，3 N/A | 收敛判定正确但注入 agent 的 gate 状态片面 |
| 部分信号可用 | RoadmapCompletion=0.8，GatesGreen=true，但 ReviewStatus=""，RequirementConfidence=0 | converge 说 MET，但缺失的信号可能意味未知问题 |
| Phase 输出部分可解析 | unwrapClaudeResult 成功但 verdict parsing 失败 | feed-forward 内容正确但收敛信号丢失 |
| Memory 部分损坏 | 文件开头几行损坏但后续行正常 | 当前 `Load` 整体失败（全部丢失或全部加载） |

### 代码级证据

**证据 1: 收敛判定对缺失信号的处理是静默的**
```go
// converge.go: evalOne — 对每个 criterion 独立判定
func evalOne(c asset.Criterion, sig Signals) Result {
    switch c.Metric {
    case "gates_status":
        return Result{Name: c.Name, Met: sig.GatesGreen || c.Optional, Detail: greenDetail(sig.GateProof)}
    case "roadmap_completion":
        return Result{Name: c.Name, Met: sig.RoadmapCompletion >= c.Threshold, ...
    case "requirement_confidence":
        return Result{Name: c.Name, Met: sig.RequirementConfidence*0.01 >= c.Threshold, ...
    case "review_status":
        return Result{Name: c.Name, Met: sig.ReviewStatus == "approved", ...}
    }
}
```
- `RequirementConfidence` 默认 0 → `0 >= 0.8` = false → 永远 unmet（如果 rule 是 `>= 0.8`）
- `ReviewStatus` 默认 "" → `"" == "approved"` = false → 永远 unmet
- 这些默认值是**fail-closed**（unsafe 方向），这是正确的。
- 但**文档没有说明**哪些信号是可选的、哪些是强制性的。`discover.yml` 的 `requirement_confidence` 在没有运行 discover 时**不可能**满足——这意味着 `forge run build`（不先跑 discover）永远 requirement_confidence unmet，但 build 阶段不检查这个信号。这个逻辑只在 `conjunction` 中被所有 `all_of` 条件共同消费时才生效。

关键问题：**缺口的默认值（0 / ""）与「正常但恰好为零」无法区分。** 如果 `RequirementConfidence` 真实值为 0（product-manager 对需求完全没信心），与「没跑 discover 所以没数据」在数值上完全相同。

**证据 2: Memory 加载是全或无的**
```go
// memory.go: Load reads the entire file, fails on any decode error
func Load(path string) ([]Entry, error) {
    data, err := os.ReadFile(path)
    entries := make([]Entry, 0)
    for _, line := range strings.Split(strings.TrimSpace(string(data)), "\n") {
        var e Entry
        if err := json.Unmarshal([]byte(line), &e); err != nil {
            return nil, fmt.Errorf("memory: line %d: %w", i+1, err) // ★ 一行错，全部丢
        }
    }
}
```
如果 memory 文件有 1000 行，第 500 行损坏（磁盘扇区错误 / 并发写入交错），`Load` 返回 error，**整个 memory 不注入 prompt**——而不是丢失第 500 行、保留其他 999 行。在 24h evolve 循环的最后一次 iteration 遇到这种错误，意味着**整个记忆丢失**。

**证据 3: 多源注入无冗余**
agent prompt 的每个 context lane 都是**单一来源**：
- ADR 上下文：来自 `Retrieve(docs/adr/...)`——如果 ADR 文件损坏，整个 ADR lane 为空
- Memory 上下文：来自 `Load(memory.jsonl)`——如果 memory 损坏，整个 memory lane 为空
- Gate 上下文：来自 `gateLedger`——如果 gate 没跑，上下文为空

没有**跨 lane 的信息冗余**。当 ADR lane 为空时，没有任何其他 lane 说「ADR 应该在此但缺失了」。系统静默地不注入任何 ADR 信息——agent 可能根本发现不了缺少了什么。

### 建议的工程方向

1. **部分故障的 Memory 加载**
   - `Load` 改为「尽最大努力」解析：损坏的行跳过 + 记录 error + 保留其他行
   - 注入 prompt 时在开头插入 `⚠ memory 文件有 X 行损坏（共 Y 行）` 的诚实标注
   - 这样 agent 知道上下文可能不完整

2. **收敛信号的可选性标注**
   - 在 `converge.go` 的 `Signals` 结构体上增加 `ValidSignals` 字段（位掩码或 map）
   - 指示哪些信号有真实数据、哪些是默认值
   - `evalOne` 对「信号缺失」和「信号值为 0」做不同处理：前者视为 N/A（skip），后者正常判定
   - 这解决了 `RequirementConfidence=0`（真实没信心）与「没跑 discover」（未知）无法区分的问题

3. **跨 lane 上下文完整性告警**
   - 在 `buildPrompt` 中增加告警：当某个 context lane 为空但预期应有内容时（如 workflow 声明了 `uses_template` 但 template 文件不存在），记录结构性 note（非阻断）
   - 这个 note 进入 trace event，供 `forge investigate` 事后分析

### 边界情况

| 场景 | 当前行为 | 改进方向 |
|------|----------|----------|
| 6/6 gates 都是 N/A（全部降级） | convergence GatesGreen=false（因为 N/A 被排除） | 需要区分「全部 N/A」和「全部 PASS」——无工具的环境不应该永不收敛 |
| RoadmapCompletion=0.99 但 FileDelta=0.0 | 诚实性告警 ⚠，但不影响收敛 | **正确行为**——告警但不阻止，留给人类判断 |
| Phase 有 output 但无法被任何契约解析器识别 | output 进 feed-forward 但 verdict 丢失，verdict 相关信号 unmet | 在 trace 中标注 `unparsed_phase_output=true` |
| Loop-back 修复了一个 gate 但产生了新的契约违例 | 修复→gate 绿→收敛 | 正确但脆性——一次修复掩盖了契约退化 |

---

## 汇总:五个方向的质量影响矩阵

| # | 方向 | 核心问题 | 影响域 | 严重度 | 当前测试覆盖 |
|---|---|---|---|---|---|
| 1 | Prompt 构建管道 QA | 系统最关键的"编译器"无质量门 | Context 注入可靠性 | **高**——所有 agent 指令经过此管道 | 0（无 prompt 快照测试） |
| 2 | LLM 输出契约履约验证 | 机读裁决无履约执行 | 收敛判定可靠性 | **高**——错误裁决导致早/晚收敛 | 只有解析器单测 |
| 3 | 核心链路交互面测试 | 独立验证的机制未做过组合测试 | `forge evolve` 循环可靠性 | **中**——组合故障可能在生产中出现 | 各机制独立覆盖，0 组合 |
| 4 | YAML 双解析器可靠性 | Python/Go 解析路径可能静默分歧 | 全配置管道的正确性 | **中**——已出过一次 block-scalar bug | 仅在 7 个真文件上差分 |
| 5 | 部分故障与优雅降级 | 系统的故障模型是二值的 | 收敛判定 + 记忆 + 上下文完整性 | **中**——灰度故障场景未覆盖 | 无部分故障测试 |

### 优先级建议

**本周可开始**：
- **方向二（契约履约）**：最小改动——在 `cost.go` 的解析器中增加 `unmatched` 计数，输出到 trace 和日志。立即获得可视化数据，了解真实 agent 的履约率。
- **方向一（Prompt QA）**：从第一个快照测试开始——为 build.yml 的 implementer phase 建立 prompt golden file。过程只需写一个测试，抓取渲染输出，人工审核后 freeze。

**1-2 sprint**：
- **方向四（YAML 差分 fuzz）**：对 Go yaml2json 建立 fuzz test，对比 Python PyYAML 输出。这个管道已导致一次 block-scalar bug，防止第二次的 ROI 明确。
- **方向五（Memory 部分故障）**：将 `Load` 的「全或无」改为「尽最大努力」。改动量小（约 30 行），影响大——防止一次磁盘写入错误丢失全部记忆。

**3+ sprints**：
- **方向三（组合测试）**：建立组合测试框架是最系统性的工程，覆盖所有机制的交互面。但它的价值也最大——当前 707 个测试覆盖了安全护栏的 4 个维度，但**没有任何测试验证它们同时运行时的行为**。

---

## 附录:被排除的方向与理由

| 方向 | 排除理由 |
|------|----------|
| 跨厂商模型路由（同已有分析） | 已在 `docs/requirements/high-value-expansion-directions.md` 充分覆盖 |
| 并行调度引擎激活 | 同上 + `expansion-core-five-2026-07-01.md` |
| 内存/知识存储规模演进 | 同上 |
| 多进程并发安全 | 同上 |
| 运行诊断与根因分析 | 同上 |
| 跨周期收敛状态机 | `expansion-core-five-2026-07-01.md` 完整覆盖 |
| 统一验证引擎 | `expansion-core-five-2026-07-01.md` 完整覆盖 |
| WASM 可移植工具链 | `ROADMAP.md` 方向三覆盖 + `expansion-core-five.md` 方向三覆盖 |
| CI 治理完整性 | `ROADMAP.md` 方向五覆盖 + 后续 sprint 跟进 |
| 子进程生命周期全量管理 | `strategic-extensions-v24.md` 方向一覆盖 |
| Feed-forward 级联截断 | `strategic-extensions-v22.md` 方向一覆盖（本文方向一的子问题） |

---

## 结论

ForgeOS 的功能完整性在 Sprint 31 已达到一个**稳健的基线**：所有的 GAP 已收口、需求清单完整、核心循环已端到端坐实。但**生产就绪度**测试——Prompt 管道的可靠性、LLM 输出契约的履约验证、组合机制的交互面、配置解析器的一致性、部分故障的优雅降级——是系统从「能跑」到「可信赖」之间的关键一步。

这五个方向都不需要新引擎、新架构、新服务。它们只需要**在已有代码上增加质量门**。
