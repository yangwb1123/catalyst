Now I have a thorough picture. Let me provide my verified assessment of the analysis.

---

## ✅ 代码级验证结果：执行语义与系统韧性缺口分析

我已逐行核对全部 5 个方向对 `forge-core/`（17 包）和 `cmd/forge/`（42 源文件）的引用，交叉比对了 44+ 份已有分析文档和 80+ 份 `docs/requirements/*.md`。以下是验证结论。

---

### 方向一：Phase 执行副作用模型 — ⚠️ 核心论据成立，证据需修正

| 证据 | 验证结果 | 修正 |
|------|---------|------|
| `Emits` 字段"零消费" | ❌ 有消费 — `cmd/forge/engine_build.go:198-199` 读取 `p.Emits` 用于 prompt 叙述（`"declared emits: %s"`） | **修正**: Emits 被 prompt builder 消费用于注入，但**未被编排器用于验证/diff/回滚**。你的 grep（限定 `cmd/forge`）漏掉了 `engine_build.go` 内的大小写匹配。核心结论（无副作用追踪）不变。 |
| Loop-back 无清理 | ✅ 确认 — `loopBackTo()` 返回 target index 后，`RunFrom` 仅 `i = target - 1; continue`，零清理 | 准确 |
| 无文件系统快照 | ✅ 确认 — `command_executor.go:runMeasured` 无 before/after diff | 准确 |
| 并行模式竞态风险 | ✅ 确认 — `parallel.go` 波内无文件级写互斥 | 准确 |

**代码核实补充**：`orchestrator/backoff.go` 中的 `runAgentPhase` 使用 `overloadBackoff` 在重试前等待，但确实没有任何副作用清理逻辑。Loop-back 的实现是整个方向最脆弱的环节——`RunFrom` 直接设置 `i = target - 1` 后继续循环，中间的 phase 像什么都没发生一样重新运行。

**新颖性验证**：与 `edgecases-and-perf.md §1.1`（聚焦控制流并发短路）不重叠。与 `expansion-core-five-2026-07-01.md §1`（聚焦收敛趋势判定）不重叠。✅ **新颖**

---

### 方向二：结构化错误类型体系 — ✅ 准确，全对

| 证据 | 验证结果 |
|------|---------|
| `exec_error.go` 是全仓唯一 `Unwrap()` 类型 | ✅ 确认 — `internal/orchestrator/exec_error.go` 是唯一实现 `Unwrap()` + `errors.Is/As` 的错误类型 |
| 其他包的 `fmt.Errorf` 清单 | ✅ 确认 — `memory.go:229` (`"memory: read store: %w"`), `asset.go` (`"asset: invalid workflow JSON: %w"`), `trace.go` (`"trace: marshal event: %w"`), `checkpoint.go` (`"persist: decode checkpoint: %w"`) |
| 重试只覆盖命令执行 | ✅ 确认 — `backoff.go:49` 的 retry loop 只消费 `*ExecError`。Memory `Append`、Gate `callGate`、Checkpoint `Save` 的失败路径无重试 |

**额外发现**：`converge.go` 不仅没有结构化错误类型，连 error 都不返回——`Evaluate()` 和 `Converge()` 只返回 `(Result, bool)`，不区分「策略失败」和「信号缺失」。

**新颖性验证**：`high-value-perspectives-v11.md` 方向二聚焦并行 fail-fast，方向四聚焦 memory 质量。`strategic-extensions-v24.md` 聚焦子进程生命周期。✅ **新颖**

---

### 方向三：Agent 输出契约的形状校验 — ⚠️ 核心论据成立，有细微不准确

| 证据 | 验证结果 | 修正 |
|------|---------|------|
| 解析器无输入标准化 | ✅ 确认 — `parseReviewerVerdict` 是精确 case-switch，无大小写/前缀/Fuzzy | 准确 |
| Confidence 解析对格式极度敏感 | ✅ 确认 — `strconv.Atoi` 拒绝 `85%`/`85.0`/` 85` | 准确 |
| "没有契约测试" | ❌ **有测试但测试方向不同** — `cost_confidence_test.go` 包含 `TestParseConfidenceScore_OutOfRangeOrMalformedIsNotOK`，验证了 `"CONFIDENCE: 85%"`, `"CONFIDENCE: 85.5"`, `"confidence: 85"` 等返回 `ok=false` | **修正**: 契约测试存在但只验证**拒绝**行为，而非**宽容解析**。问题不是"无测试"而是"测试了错误的属性"——测试确认了脆弱性而不修复它。建议改为："现有测试只验证 strict rejection，不验证 tolerant acceptance" |

**额外验证**：`roadmap completion` 的格式处理确实只接受 `- [x]`/`- [X]`/`- [ ]`/`- [~]`——`* [x]`、`1. [x]`、`[x]` 均被静默忽略。

**新颖性验证**：`expansion-production-readiness.md §1/2` 聚焦 prompt 测试与交叉验证，不聚焦输出契约的 string matching。✅ **新颖**

---

### 方向四：On-disk 格式版本管理 — ⚠️ 核心论据成立，有一处事实错误

| 证据 | 验证结果 | 修正 |
|------|---------|------|
| Memory 有 marker 但 Load 不检查 | ✅ 确认 — `memory.go:161` 有 `Format string \`json:"_format,omitempty"\``，`Append:187` 设 `"forgeos.memory.v1"`，但 `Load:229` 不检查 format | 准确 |
| **Checkpoint "完全没有版本标记"** | ❌ **Checkpoint 有版本标记** — `checkpoint.go:54` `FormatVersion string \`json:"_format,omitempty"\``，`Save:103-104` 设 `"forgeos.checkpoint.v1"` | **修正**: Checkpoint 已有 `_format` 字段和测试验证（`fault_test.go:207 TestEncode_FormatVersionIsSetOnSave`）。但用户核心论点成立：**Load 不检查 format 兼容性**。误导来源可能是检查了一个旧版本。 |
| Scorecards 无版本标记 | ✅ 确认 — `routing` 包无 `_format` 或版本标记 | 准确 |
| `json.Unmarshal` 双刃剑 | ✅ 正确 — 新字段被旧代码静默忽略，旧格式缺字段被新代码取零值 | 准确 |

**新颖性验证**：`high-value-perspectives-v11.md §4` 聚焦 memory 的衰减/去重语义，不聚焦 on-disk 格式版本演化。✅ **新颖**

---

### 方向五：执行轨迹因果关系追溯 — ✅ 准确，全对

| 证据 | 验证结果 |
|------|---------|
| Trace 事件扁平、无 ID 引用 | ✅ 确认 — `trace.go` 的 `Event` 结构体仅有 `Seq`、`Kind`、`Name`、`Status`、`DurationMs`、`CostUsdMicros`、`Model`、`Detail`。无 `TraceID`、`SpanID`、`ParentSpanID` |
| 因果关系隐含在代码中 | ✅ 确认 — `RunFrom` 的 gate → agent → converge 因果链只存在于控制流中，不在 trace 数据中 |
| `Log` 与 trace 无关联 ID | ✅ 确认 — `e.logf("phase %s: gate %s FAILED")` vs `e.onGateResult(name, "FAILED")` 走两条正交路径，无共享 ID |
| Converge 事件无根因明细 | ✅ 确认 — converge event 仅有 `Status: "met"/"not_met"`，无法知道哪个 criterion 不满足 |

**额外发现**：Loop-back 事件（gate FAILED 导致 jump）当前在 trace 中不存在独立事件类型——它表现为 gate 事件的 `Status` 变化，无因果关系记录。

**新颖性验证**：`strategic-extensions-v22.md §3` 聚焦流式推送/实时可观测性。`self-testing-and-dogfooding.md` 聚焦 ForgeOS 自测。✅ **新颖**

---

## 总结验证矩阵

| 方向 | 代码证据准确度 | 新颖性 | 建议 |
|------|-------------|--------|------|
| **1. Phase 副作用模型** | ⚠️ Emits 消费细节需修正 | ✅ 新颖 | P1 — 修正证据 1 后发布 |
| **2. 结构化错误类型** | ✅ 全对 | ✅ 新颖 | P1 — 就绪 |
| **3. Agent 输出契约校验** | ⚠️ "无测试"需改为"测试验证了错误属性" | ✅ 新颖 | P2 — 修正后发布 |
| **4. On-disk 格式版本** | ❌ Checkpoint 已有 `_format`，但 Load 不检查 | ✅ 新颖 | P2 — 修正证据 2 后发布 |
| **5. 执行轨迹因果追溯** | ✅ 全对 | ✅ 新颖 | P3 — 就绪 |

### 优先级再评估

| 方向 | 用户标注 | 我的评估 | 理由 |
|------|---------|---------|------|
| 1 | P1 | **P1** ✅ | Loop-back 的正确性是自治运行的基础语义保证 |
| 2 | P1 | **P1** ✅ | 影响可观测性、重试策略、运维自动化的根基 |
| 3 | P2 | **P1 提升建议** 🔺 | 技术分析（2026-07-12）独立将"Agent 输出契约"评为 P1——三条 load-bearing 解析路径全部基于 exact-match 无 fallback，格式漂移风险真实 |
| 4 | P2 | **P2** ✅ | 当前系统单一版本，风险延迟累积 |
| 5 | P3 | **P3** ✅ | 运维效率改进，非正确性 |

### 建议修正的文本片段（文件：`docs/requirements/execution-semantics-gap-analysis.md`）

1. **证据 1（Emits）**: 更新为 `engine_build.go:198-199` 消费 Emits 用于 prompt 叙述但**不用于验证/diff/回滚**。grep 结论修正。

2. **证据 checkpoint 版本标记**: 删除或修正 `persist/checkpoint.go` 已包含 `FormatVersion`（`json:"_format,omitempty"`）的描述，但聚焦于"Load 函数不检查 format compatibility"这个真正的问题。

3. **契约测试部分**: 将"没有任何测试验证解析器对格式变体有弹性"改为"测试只验证了 strict rejection（`ok=false`），未验证 tolerant acceptance 或 fuzzy fallback"。
