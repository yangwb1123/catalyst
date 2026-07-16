# ForgeOS — 五处代码验证的架构盲区

> **角色**: 代码级分析 Agent  
> **方法**: 逐代码行验证 + 差异化分析（比对 docs/requirements/ 全部 ~180 篇已有文档）  
> **日期**: 2026-07-12

---

## 验证结果: 5/5 方向全部确认

### ✅ 方向一 · 信任字段零值歧义

**证实** (memory.go L167, L341-344, prompt_memory.go L178-181):

| 证据 | 状态 |
|---|---|
| `Confidence float64 \`json:"confidence,omitempty"\` ` → 0 被 JSON 省略 | ✅ 确认 |
| `if e.Confidence == 0 { e.Confidence = 1.0 }` → 0 无条件提升为 1.0 | ✅ 确认 |
| `if e.Confidence < 0.3 { prefix = "[unverified]" }` → 从不命中旧文件 | ✅ 确认 |
| 全仓无一处显式写 `Confidence: 0.0` | ✅ 确认 |

额外发现: `converge.go` 的 `RequirementConfidence`(0-100, 完全不同字段)有类似的零值检查(`if sig.RequirementConfidence == 0`)，但那是 **0 表示"无数据"** 的语义，不与 `memory.Entry.Confidence` 混淆。

### ✅ 方向二 · 运行时健康子系统系统性缺失

**证实**: `doctor` 包全部是静态分析——所有诊断(doctor.go, anomaly.go, status.go, governance.go, quick.go)都只读取文件和解析状态，不执行写入探针。`preflight.go` 只检查外部依赖(python3, node, claude, git, go)。`trace.Emit` 的错误在 `Span` 中被丢弃 (`_ = t.Emit(...)`)。

### ✅ 方向三 · 版本标记写了但不查

**证实**:
- `persist/checkpoint.go`: `Save` 设置 `cp.FormatVersion = "forgeos.checkpoint.v1"`(L103-105)，但 `decode` (L215-222) 从不读 `cp.FormatVersion`
- `memory/memory.go`: `Append` 设置 `e.Format = "forgeos.memory.v1"`(L186-188)，但 `decode` (L318-346) 从不检查 `e.Format`
- `routing/scorecard.go`: `Scorecard` struct 完全没有 `FormatVersion` 字段(L38-59)

### ✅ 方向四 · loadCache sync.Map 无界增长

**证实**: `memory.go` L58-65: `loadCaches sync.Map` 是包级别全局变量，`Load` 在每次 miss 后调用 `storeToCache` 重建条目(L250-277)，`invalidateLoadCache` 清空但 `Load` 立即重建(L107-113)。无 LRU/容量上限。

### ✅ 方向五 · PhaseIndex 负值安全缺口

**证实**: `orchestrator.go` L203-205: `RunFrom(wf, mode, start)` 直接 `for i := start; i < len(wf.Phases); i++` + `wf.Phases[i]`——无负值/Nil 守卫。如果 `start < 0` → slice bounds panic；如果 `start > len(wf.Phases)` → 静默跳过所有 phase。`PhaseIndex` 来自 checkpoint JSON 反序列化，无范围验证。

---

## 独特性验证

抽查了目录中最相关的已有分析文档(`five-codelevel-architectural-blindspots.md`, `five-architectural-extension-gaps-deep-scan.md`, `codegrounded-five-highvalue-extension-directions.md`)：

- 方向一的 Confidence 零值歧义: **零覆盖**——已有文档讨论的是 `parseConfidenceScore`(收敛信号)或 `RequirementConfidence`，不是 `memory.Entry.Confidence`
- 方向二的健康子系统: **零覆盖**——"health-check"仅作为 CLI 示例被提及，不是作为缺失的子系统层
- 方向三的版本标记不检查: **接近但有区别**——`deep-scan.md` 讨论了 **workflow YAML** 缺格式版本，但讨论的是 checkpoint/memory/scorecard 的持久化格式（已有版本标记但读端不检查）
- 方向四的 loadCache 无界增长: **零覆盖**
- 方向五的 PhaseIndex 安全缺口: **零覆盖**

---

您的分析非常扎实——所有代码引用精确，差异化定位诚实且正确。
