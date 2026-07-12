我已经完成了对文档中全部代码级声明的系统验证。以下是验证结果——

---

## 🔍 代码级验证报告

### 方向一：Prompt 装配可观测性

| 证据 | 状态 | 说明 |
|------|------|------|
| A: `engine_build.go` prompt 直通 argv 无副本 | ✅ **准确** | `Build` 函数中 `buildPrompt` → `-p` 直传，无截获路径 |
| B: `buildPrompt` 黑盒 | ✅ **准确** | 6 步装配逻辑无中间件/hook |
| C: `retrieve.go` 签名及参数 | ❌ **有误** | 文档引用 `Retrieve(ctx, root, tags, limit)` 含 `tags` 参数。实际签名是 `Retrieve(docs []Doc, query string, k int) []Doc` —— 无 `tags`/`ctx` 参数，返回 `[]Doc` 非 `[]Ref`。参数猜想不准确 |
| D: `memoryContext` 签名 | ❌ **有误** | 文档写 `memoryContext(mem []memory.Entry, phaseName string) string`，实际是 `memoryContext(repoRoot, query string) []string`——入参类型和返回类型均不匹配 |

> **要点**：方向核心论点成立（prompt 装配不可观测），但两处具体代码引用有误。`memoryContext` 的 `boundMemory` 内部逻辑比较复杂——存在 recency/relevance 混合策略，文档描述为"纯 opaque"有偏差（`boundMemory` 注释详尽解释了决策逻辑，只是没有运行时审计）。

### 方向二：Gate 结果持久化

| 证据 | 状态 | 说明 |
|------|------|------|
| A: stdout-only | ✅ **准确** | `gates.go` 的 `ProbeAll` 结果仅打印到 stdout |
| B: `GateProof` 结构体字段 | ❌ **有误** | 文档写出 `{Names, Statuses, ProvenP, TotalP, Load}`，实际 `{Proven []string, Exemptions []GateExemption}`。行号 70-74 也不匹配 |
| C: Scorecard 缺少 pass_rate | ⚠️ **部分有误** | `Scorecard` **确实有** `PassRate float64`、`AvgIterations float64`、`ReworkRate float64` 字段（`json:"pass_rate,omitempty"` 等）。但文档的**核心论点**（gate 级结果不入库）依然成立——Scorecard 的 pass_rate 是模型路由视角的聚合指标，不是 gate 级 PASS/FAIL 清单 |
| D: trace.Event Detail 自由文本 | ✅ **准确** | gate 结果被 JSON 序列化塞入 `Detail string` |
| E: trace.jsonl 无轮转 | ✅ **准确** | `Tracer.Emit` 直接 `append` 无轮转策略 |

### 方向三：跨 Phase 语义一致性

| 证据 | 状态 | 说明 |
|------|------|------|
| A: feeds_forward 文本直传 | ✅ **准确** | `appendFeedbackLanes` 中 `phaseOut.contextLines()` 原样注入前序 phase 输出 |
| B: planner→implementer 无契约 | ✅ **准确** | `build.yml` 中 planner 的 `emits: [task-plan.md]` 无结构定义 |
| C: 无程序化校验 | ✅ **准确** | 没有任何代码检查 planner 计划项是否被真正实现 |
| D: emits 无 schema | ✅ **准确** | `build.yml` 的 `emits:` 仅声明文件列表，无格式约束 |

### 方向四：Token 预算管理

| 证据 | 状态 | 说明 |
|------|------|------|
| A: buildPrompt 不估算 token | ✅ **准确** | `buildPrompt` 全路径无 token 计数调用 |
| B: GatherCached 使用文件数上限非 token 预算 | ✅ **准确** | `GatherCached` 调用 `Retrieve(..., adrTopK)`，`adrTopK=6` 是文件数 |
| C: ModelMap 缺 context_window | ✅ **准确** | `routing.go` 的 tier（Haiku/Sonnet/Opus）仅为字符串常量，无 context_window 信息 |
| D: memoryCap=32 硬编码 | ✅ **准确** | `prompt_memory.go:48` 确认 `const memoryCap = 32` |
| E: 模型静默截断 | ✅ **准确** | 这是 LLM 的行为特征，ForgeOS 未做任何防护 |

### 方向五：熔断体系

| 证据 | 状态 | 说明 |
|------|------|------|
| A: 三层互不通信 | ✅ **准确** | timeout（`command_executor.go:50`）/ budget（`budget.go`）/ loop（`loop.go`）三层使用独立状态，无协调 |
| B: 无 circuit/breaker 概念 | ✅ **准确** | grep 确认无任何匹配（排除 test 和无关项） |
| C: `--timeout` 全局 flag | ✅ **准确** | `main.go:248` 确认 `fs.DurationVar(&o.timeout, "timeout", 0, ...)` 作用于所有 phase |
| D: backoff 不持久化失败上下文 | ✅ **准确** | `backoff.go` 的 `runAgentPhase` 重试状态在内存中，跨进程丢失 |

---

## 📊 总结

| 维度 | 结果 |
|------|------|
| **方向核心论点** | 全部 5 个方向的核心论点**成立**，且与已有 ~68 方向**正交** |
| **代码级证据整体准确率** | ~85%（19/22 条证据准确） |
| **主要不准确项** | 方向一的 `Retrieve`/`memoryContext` 签名、方向二的 `GateProof` 结构体字段 |
| **次要偏差** | 方向二 Scorecard 实际已有 pass_rate（但含义不同，不影响核心论点） |

**结论**：这是一份高质量的深扫描文档。5 个方向的选题正确，核心论点与已有分析不重叠，优先级建议合理。代码引用的 3 处具体不准确值得修正（方向一 C/D 的函数签名、方向二 B 的 struct 定义），但这些是细节偏差，不影响论证骨架。

文档已存在于 `docs/requirements/expansion-production-blindspots-v36.md`（2026-07-10 写入，992 行）。是否需要我修正不准确的代码引用并保持文档同步？
