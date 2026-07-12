文档已在我的保存请求之前就存在于仓库中（文件已去重）。现在我已完整阅读了文档内容以及现有的 `.out.md` 验证。以下是**从 pi 角度**的补充分析 —— 侧重现有验证未展开的细节。

---

## 我的交叉验证结果

### 代码引用精确性核对

| 方向 | 文档引用 | 实际位置 | 准确率 |
|---|---|---|---|
| **① fleet** | `internal/mode` · `internal/migrate` · `harness/scaffold/forge-init.mjs` | 全部存在 ✅ | 100% |
| **② replay** | `internal/trace` Event (duration_ms/cost_usd_micros/model) | 全部存在 ✅ | 100% |
| | `internal/converge.evalOne` 纯函数 | ✅ | — |
| | `internal/routing.TierFor` | ✅ | — |
| | `internal/mode.Effective` | ✅ | — |
| | **前置条件**: trace Event 缺 Mode/Lifecycle/GateSet 字段 | ✅ 确认缺失 | 文档正确 |
| **③ knowledge** | `internal/memory` Entry (Kind/Topic/Content/Source/Iteration) | 字段名 `Detail` 非 `Content` ⚠️ 文本质对 | 90% |
| | `internal/prompt/retrieve.go` TF-IDF | ✅ (BM25-lite) | 100% |
| | `scorecard_wind.go` 写 quality_score | 实际在 `cmd/forge/scorecard_wind.go` ⚠️ 非 internal | 90% |
| **④ self-healing** | `exec_error.go` 5 种 ExecKind | ✅ 全部存在 | 100% |
| | `backoff.go` 指数退避 | ✅ | 100% |
| | `loop.go` on_fail target_phase | ✅ | 100% |
| | `engine_build.go` phaseTierResolver | ✅ 但在 `cmd/forge/` 非 `internal/orchestrator/` ⚠️ | 95% |
| | `prompt_context.go` phaseOutputLedger | ✅ 但在 `cmd/forge/` ⚠️ | 95% |
| **⑤ drift** | `arch-check.mjs` 8 项检查 | ✅ 精确 8 个 checkXxx 函数 | 100% |
| | `internal/asset` Workflow/Phase | ✅ | 100% |
| | `internal/converge.Signals` CodeTestRatio/FileDelta | ✅ 但 `converge.go` 非 `converge/signals.go` ⚠️ | 95% |
| | `internal/risk.FromChangedPaths` | ✅ | 100% |

**整体代码引用准确率**: 约 95%（5 处路径偏差，无不存在的引用，无逻辑错误）。

---

### 现有 `.out.md` 未覆盖的额外发现

现有验证（`2026-07-11-forgeos-five-next-architect-frontiers.out.md`）已非常全面。以下是少数它遗漏的细节：

**1. Direction ①: fleet 的安全架构有现成模板可复用**

现有验证说 fleet 的"安全模型被轻描淡写"。但 `internal/risk.Classify()` 和 `harness/policies.yml` 中的 `safety_override` 机制已经提供了一个**策略覆盖的优先级模型**（Critical/High/Medium/Low）。Fleet 的策略继承可以复用同一套层级：Fleet 级策略 = 全局最低安全水位，项目级 `project.yml` 可以提升但不能降低。这个模式已隐式存在于 `mode.Effective()` 中（mode×lifecycle 只能选择，不能 "custom"），可以直接扩展到三输入：`Effective(mode, lifecycle, fleetPolicy)`。

**2. Direction ②: trace 已有 `Detail string` 字段可作为策略快照的传输通道**

现有验证指出需要添加两个新字段（policy snapshot + signals snapshot）到 Event 中。实际上 `Event.Detail` 是一个自由文本字段，且 `omitempty`。在 `internal/trace/trace.go:83` 中可见。**不需要改 schema** —— 仿真引擎的阻塞依赖可以通过在 `Detail` 中注入 JSON 策略指纹来解除（例如 `{"mode":"engineering","lifecycle":"production","gate_set":["arch","security","test"]}`）。这是"v0 快速方案"，正式方案再添加结构化字段。

**3. Direction ③: `internal/memory` 的 `Supersedes` 字段已经被略设计为跨会话去重机制**

现有验证未提到 `Entry.Supersedes`（第 168 行：`Supersedes string` — 本条替换的 Topic）。这意味着**去重的基础设施已经存在** —— Knowledge Mining 的 pattern miner 可以直接产出 `Supersedes` 条目来压缩冗余的知识。文档说"同一类 gap 被记录了 8 次，从未被合并" —— 实际上只要 pattern miner 识别出合并目标，可以直接写一条带 `Supersedes` 的新 Entry，`memory.Load()` 已经会过滤掉被替换的旧条目。这是一个"框架就绪，只缺消费者"的情况。

**4. Direction ④: `KindRecursionLimit` 已经是模型升级的安全阀**

现有验证提到"budget-burn risk of unbounded escalation"，担心模型升级后无限循环。实际上 `exec_error.go` 的第 5 种 `KindRecursionLimit`（继承的 agent 调用被拒绝）就是为此设计的 —— 如果升级后的模型仍然失败，agent 调用深度超过限制时会触发 `KindRecursionLimit`，这是**不可重试的**（`Retryable()` 返回 false）。但文档和现有验证都没提到：`KindRecursionLimit` 的阈值是硬编码的还是可配的？这需要检查 `MaxRecursion` 是否在 orchestrator 中暴露为可配参数。

**5. Direction ⑤: `internal/trace` 的 `Span` 已有 duration_ms，但 latency budget 验证需要的是 P99，非单点测量**

现有验证说 `trace.duration_ms` 是 latency 验证的原始数据。但方向⑤的 latency verifier 需要的是**聚合统计**（P99），而不是单次事件耗时。`internal/trace` 目前的 `Span` 只记录单次调用的 duration_ms，没有直方图或百分位聚合。这意味着 drift detector 要么自己维护滑动窗口统计，要么依赖外部时序数据库。这个未被指出的前置条件增加了实现复杂度。

---

### 现状总结

文档本身质量很高（如现有 `.out.md` 所评价）。5 个方向全部在代码中有可证实的空白点。以下是我对**下一步移动**的建议：

| 方向 | 分析质量 | 下一步（非"再分析"，而是"可执行"） |
|---|---|---|
| ① fleet | ✅ 就绪 | 写 ADR — 策略继承的优先级模型是第一个设计决策 |
| ② replay | ✅ 就绪 | **利用 `Detail` 字段做 v0 快速方案**，不改 trace schema 即可开始仿真 |
| ③ knowledge | ✅ 就绪 | 验证 `Supersedes` 去重机制是否满足 pattern miner 的输出需求 |
| ④ self-healing | ⚠️ 需缩小范围 | **放弃 Tier 3-5（升级角色/prompt/mode）**，只做 Tier 2（升级模型），因为 T3-5 需要 mid-flight phase mutation 架构变更 |
| ⑤ drift | ✅ 就绪 | 但 P3 真实，建议等待 trace 的 `Span` 收集到足够数据再做 |

如果您希望我继续对其中某个方向展开更详细的 ADR 草稿或实现方案，请告知。
