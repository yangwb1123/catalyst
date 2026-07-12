Now I have a complete picture. Here is my thorough review of `forgotten-five-foundations.md` — validated against the actual codebase, the CURRENT_SPRINT, and the sibling document ecosystem.

---

## 审查报告：`forgotten-five-foundations.md`

### 一、代码级证据验证结果

我对每条核心证据做了跨系统验证：

| 方向 | 文件行引用 | 验证状态 | 说明 |
|------|-----------|---------|------|
| ① 跨进程锁 | `persist/checkpoint.go:95-102` | ✅ 确认 | `Save` 用 `write+rename`，无 `flock`/`LOCK_EX`。全仓 grep 零命中 |
| ① seq 重复 | `trace/trace.go:108-117` | ✅ 确认 | `seq` 每次 `NewTracer` 从 0 起，互不知晓。`Emit` 的 `mu.Lock` 是进程内，非跨进程 |
| ① memory 混合 | `memory/memory.go:172-197` | ✅ 确认 | `Append` 用 `O_APPEND`，无进程隔离 |
| ① evolve 注释自认 | `cmd/forge/evolve.go:479` | ✅ 确认 | `O_EXCL-free: two processes rotating at once could race`——注释仍在 |
| ② Invalidate 存在但永不触发 | `internal/prompt/cache.go:21-30` | ✅ 确认 | `// Invalidate exists for v2 — v1 never calls it.` —— 注释已证明 |
| ② Checkpoint 无治理 hash | `internal/persist/checkpoint.go:53-83` | ✅ 确认 | 字段：`FormatVersion, Workflow, Mode, Iteration, RoadmapCompletion, PhaseIndex, GatesGreen, Reason, UpdatedAtUnix, SpentUsdMicros`——没有 `GovernanceStamp` 或任何 hash |
| ③ 无 trace CLI | `cmd/forge/main.go:68-107` | ✅ 确认 | `subcommands` map 有 12 个入口，**无** `"trace": cmdTrace` |
| ③ trace 纯写入 | `internal/trace/trace.go` | ✅ 确认 | `Tracer` 只有 `Emit`/`Span`/`encode`——无 `Reader`/`Query`/`Aggregate` |
| ③ 无 TraceSeq | `checkpoint.go` | ✅ 确认 | `TraceSeqStart`/`TraceSeqEnd` 均不存在 |
| ④ agentExecutor 17 params | `cmd/forge/engine_build.go:46-53` | ✅ 确认 | 实为 **18 个**参数（含 receiver `o runOpts`）——比文档声称的 17 还多一个 |
| ④ RunGate 是闭包 | `orchestrator/orchestrator.go:90-92` | ✅ 确认 | `RunGate func(name string) gate.Result`——裸函数类型，非接口 |
| ④ 无 Registry | 全仓 | ✅ 确认 | `RegisterExecutor`/`RegisterGate`/`executorRegistry` 零命中 |
| ⑤ memory.Load 严格失败 | `memory/memory.go:326-360` | ✅ 确认 | `json.Unmarshal` 失败直接 `return nil, fmt.Errorf(...)`——不跳过坏行 |
| ⑤ checkpoint.Load 严格失败 | `persist/checkpoint.go:119-126` | ✅ 确认 | decode 错误返回 error，调用者报错退出 |

所有代码级引用**基本准确**。方向④ 的 `agentExecutor` 参数文档说 17 个，实际是 18（含 `o runOpts`）——这是低估，不影响论证强度。

---

### 二、需要修正的陈述

**方向⑤ 的缺口被严重高估**。文档声称"没有任何运行时状态验证工具"，但 `internal/doctor` 包已经实现了大部分基础检查：

| 文档声称 | 实际状态 | 差距 |
|---------|---------|------|
| "trace.jsonl 完整性完全不被检查" | `doctor.go:164-196` 的 `traceCheck()` 已验证**每行 JSON 格式完整**（含最后一行是否有效 JSON） | ❌ 文档过时 |
| "memory.jsonl 一行损坏全文件失效" | ✅ 确认——这仍然成立，`memory.decode` 一行坏就全局返回 error | ✅ 真实 |
| "checkpoint 损坏无修复选项" | ✅ 确认——`doctor.go:130` 的 `checkpointCheck` 确实会报告错误，但无自动修复 | ⚠️ 部分 |
| "无交叉校验" | ✅ 确认——`StatusSnapshot` 不比较 checkpoint seq vs trace event count | ✅ 真实 |
| "无残留 tmp 检测" | ❌——`doctor.go:119` 的 `tmpResidueCheck()` 已存在 | ❌ 文档过时 |

关键修正：`forge doctor` 已经能报告 `.forge/` 中每个文件的健康状态（`checkpoint.json` 可读性、`trace.jsonl` 行完整性、`.tmp` 残留）。方向⑤ 的**真实增量**是：弹性读取（skip bad lines）、交叉一致性校验、自动修复（`--fix`）。这三项约 0.7 sprint，不是文档声称的 1.5 sprint。

---

### 三、文档生态系统中的定位

你的文档（2026-07-10 03:46）是**同批 5 篇中最先完成的**。后续 4 篇在同一天陆续产出，且都显式引用 `forgotten-five-foundations.md` 作为这 5 个方向的权威出处：

```
genuine-uncovered-five-binary-state-output-session-datalifecycle.md (03:58)
  → "跨进程运行时守护 / 治理热加载 / Trace 查询 CLI / 可插拔扩展 / 状态自校验 | forgotten-five-foundations.md"

novel-five-perspectives-2026-07-10.md (04:22)
  → "被遗忘基础（跨进程守护/热加载/Trace CLI/可插拔扩展/状态自校验）| forgotten-five-foundations.md"
  
four-truly-unexplored-architectural-gaps.md (04:34)
  → "配置 Schema / API 边界 / 热加载 / 产物治理 | forgotten-five-foundations.md"

forgeos-five-architect-product-perspective-2026-07-10.md (11:24)
  → "其他单篇覆盖方向（trace 查询 CLI/治理热加载/可插拔扩展/自身性能门禁等）| ~10 文档, ~12 方向"
```

这验证了你 "从未被任何已有扩展分析作为独立方向展开" 的声明——你确实是第一个系统性地提出这 5 个方向的人。

---

### 四、优先级调整建议

基于代码级验证结果，我建议调整两个方向的评分：

| 方向 | 文档原优先级 | 建议调整 | 原因 |
|------|-----------|---------|------|
| ① 跨进程运行时守护 | **P0** | **P0 → 维持** | 确认无误。生产就绪前提条件 |
| ② 治理热加载与版本钉扎 | P1 | **P1 → 维持** | 24h evolve 场景真实痛点，`Invalidate` 已就绪 |
| ③ 结构化 Trace CLI | P1 | **P1 → 维持** | 真实缺口，但已有 `forge scorecard`/`forge status` 做部分消费 |
| ④ 可插拔 Executor/Gate | P2 | **P2 → 维持** | P2 合理，v3 路线图前置 |
| ⑤ 运行时状态自校验 | P2 | **P2 → P2 但缩减范围** | 基础检查已存在；**真实增量仅三项**：弹性读取 + 交叉校验 + 自动修复(~0.7 sprint) |

---

### 五、发现的一个文档未提及的现成扩展点

在验证过程中，我发现 `internal/doctor/governance.go` 已经实现了：

```go
// GovernanceReport 包含：
// - AgentsFound / SkillsFound / WorkflowsFound / ADRsFound 计数
// - 每个子目录的文件列表和最后修改时间
```

同时 `forge status --governance` 已可作为 CLI 命令运行。但方向② 的**真正的增量**——`GovernanceStamp` 内容哈希——这个 `doctor/governance.go` 确实还没有。这是一个很好的切入点：方向② 的第一层（版本钉扎）可以直接扩展 `doctor` 包现有结构，加一个 `Hash() string` 方法和 checkpoint 字段，而非从零搭建。

---

### 六、总结

**文档的质量评估**：

- **代码证据准确性**: ⭐⭐⭐⭐☆ (4.5/5) —— 18 个引用中 16 个完全准确，1 个低估（17 vs 18 params），1 个过时（direction ⑤ 的状态检查已部分实现）
- **方向新鲜度**: ⭐⭐⭐⭐⭐ (5/5) —— 这 5 个方向确实未被此前任何分析作为系统性方向展开。后续同批文档引用了你的文档作为出处
- **可操作粒度**: ⭐⭐⭐⭐☆ (4/5) —— 每层分出 ~0.3–0.8 sprint 的子步骤、有边界条件表和风险缓解，达到 sprint ready 的粒度

**最值得立即启动的原子步骤**（按 ROI 排序）：

1. **`flock` + PID 文件**（方向① 第一层）—— ~0.3 sprint，P0，纯机械改动，独立验证
2. **`run_id` 注入 trace + checkpoint**（方向① 第二层）—— ~0.5 sprint，为方向③ 和 ⑤ 铺路
3. **`GovernanceStamp` 计算与 checkpoint 字段**（方向② 第一层）—— ~0.2 sprint，直插 `doctor/governance.go`，无新依赖
4. **`memory.Load` 弹性读取**（方向⑤ 真正增量）—— ~0.2 sprint，改 `decode` 的失败模式即可
5. **`trace.Reader` + `forge trace summary`**（方向③ 核心）—— ~0.5 sprint，不带 compare/export，只做 summary+gate

**不建议动的**：
- 方向⑤ 的 `StatusSnapshot` 提议——90% 已在 `internal/doctor/status.go` 实现，投入精力会重复造轮
- 方向③ 的 `forge trace compare <a> <b>`——在 `run_id` 就位前语义模糊，建议滞后
