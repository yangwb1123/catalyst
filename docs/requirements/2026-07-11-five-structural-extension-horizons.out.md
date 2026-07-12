我已经完整阅读了你的分析文档，并逐条验证了所有代码证据。下面是我的审阅意见。

---

## 整体评估

一份**高质量的结构化扩展分析**。5 个方向中有 3 个(P0+P1)方向精确命中了工程盲区，代码证据链完整，价值判断清晰。发现 2 处事实性误差，但不影响方向级别的判断。

---

## ✅ 亮点

### 1. 方向三(P0)「静态分析风险提取」是最高价值发现

你准确指出了 G3 路由安全的**最后一环缺失**——`risk.go` 的 HONESTY 注释诚实承认"只做规则分类、不做自动特征提取"，且 `FromChangedPaths` 仅做路径子串匹配。但更重要的是，你点出了**它不仅是风险维度缺失，而是六个评分维度中五个维度的共性问题**：`policy.yml` 声明的 `signals: [cyclomatic, lockfile_delta, deps_added, ...]` 在全仓没有对应的 Go 信号生产函数。这比"风险提取"本身更根本。

已验证：`route.go:106-113` 的 `dims` map 确实来自用户手动 flag，没有任何自动信号提取。

### 2. 方向五(P0)「Agent 产出合约验证」对现实问题有精准定位

你对 `parseReviewerVerdict`/`parseExecutiveVerdict`/`parseConfidenceScore` 的三个 ad-hoc 解析器的串联分析，配合"Sprint 27 已暴露静默降级 bug"的上下文，构成了强有力的 justification。这是**已经产生实际影响的漂移问题**。

已验证：三个解析器均在 `cost.go` 中，全部使用 `strings.CutPrefix`/精确大小写匹配，解析失败返回 `ok=false`，调用者 fail-open 继续走——无 trace、无告警。

### 3. 「先验去重验证」方法严谨

对 `docs/requirements/*.md` (211篇) 和 `docs/analysis/*.md` (40篇) 做的关键词检索验证了我复查后确认全部 0 命中。方向一~五的核心机制确实没有被已有文档作为独立系统性方向展开过。

---

## ❌ 事实性误差

### 1. 方向五 · 关于 `verdictLedger` 的类型

> VerdictLedger 无类型安全: prompt_context.go:200-220 的 `verdictLedger` 是 `[]string` 切片

**不准确。** 实际代码中 `verdictLedger` 的类型是 `map[string]string`（`prompt_memory.go:295-300`）：

```go
type verdictLedger struct {
    mu      sync.Mutex
    verdict map[string]string // phase name -> latest normalized verdict token
}
```

`[]string` 与 `map[string]string` 在类型安全方面的差距是显著的：`map[string]string` 至少做了 phase-name keyed 的覆盖保护（同名 phase 的旧 verdict 被新 verdict 覆盖），比 `[]string` append 式的无分类增长好一个等级。你的核心论点（无 schema 校验、静默 fallback）依然成立，但这处细节值得修正。

### 2. 方向三 · 关于 dims 的默认值

> route.go:200-220 的 dims map 中 complexity/dependency_change/context_size/business_impact 全部硬编码为 0.5

**不准确。** 实际代码中 `route.go:106-113` 的 dims 来自用户 flag 值（默认值为 0，非 0.5）：

```go
dims := map[string]float64{
    "complexity":        o.complexity,    // 默认 0
    "risk":              o.riskScore,     // 默认 0
    ...
    "business_impact":   o.business,      // 默认 0
}
```

不过你的核心论点——**这些维度没有自动化信号生产者，全靠用户手动填 flag**——完全正确。只是说"硬编码为 0.5"与事实不符（实际是 0）。

### 3. 文档数量偏差

```
docs/requirements/ 实际 211 篇，文档声称 ~80（差 2.6x）
docs/analysis/ 实际 40 篇，文档声称 ~38（偏差较小）
```

前者的数据源可能来自过时的文件列表。这是一个小缺陷，不影响实质分析。

---

## ⚠️ 可商榷的判断与建议

### 方向一(P1)「阈值自校准」的 v1 范围

你建议 v1 增加 `--calibrate` flag 输出阈值建议。这个设计好，但值得注意：**ForgeOS 的 scorecard 目前只有单个 `quality_score` 标量**，没有分维度的质量跟踪。要校准 0.34/0.69 这两个阈值，需要知道"什么样的得分应该对应什么 tier"，但 scorecard 只有 `(model, task_type)` 聚合的质量分，没有每个历史 route 的 `Score()` 输出。所以自校准引擎在 v1 阶段可能只能做**维度权重校准**（看哪个维度区分度低），而非阈值校准本身。建议在 v1 中只做趋势报告，不做自动调整，积累足够数据后再上线 v2。

### 方向二(P1)「预测性运行估算」的模型升级场景

你正确识别了模型升级时的冷启动问题（Sonnet-4 → Sonnet-5 时旧数据无用）。但更棘手的是：**ForgeOS 目前 trace 事件中的 `Model` 字段记录的是路由分配的 tier 名称**（"haiku"/"sonnet"/"opus"），而非具体的 model 版本号（如 "claude-sonnet-4"）。`cost.go` 的 costEmitter 中模型是 `phaseTierResolver` 的输出——这是一个 tier 名。所以 trace 数据无法区分 Sonnet-4 和 Sonnet-5。预测引擎如果要按模型版本分桶，需要先给 trace 事件加上精确的 model version 字段。

### 方向三(P0)「静态分析风险提取」的语言覆盖

你在边界场景中提到了多语言 AST 分析的挑战，并建议 v1 用正则级嗅探。但我认为**更务实的 v1 起点是复用已有的 `detect.go` + `detect_parsers.go` 的语言检测能力**，它为 Go/Python/TypeScript/Rust 分别做了 `extractJsImports`/`extractGoImports` 等 import 解析。这个基础设施已经存在，可以直接扩展来检测"文件是否引入了 payment/auth 相关包"——比纯正则强一个级别。

### 方向四(P2)「跨运行失效分类」的 ROI 判断

我同意这是 P2。但需要指出：`forge trace --summary` 在 v1 需要解决 **trace 事件没有 run_id 标记**的问题（你已正确指出）。这意味着 "forge trace --summary" 无法可靠区分两条不同运行的边界。建议 v1 更实在的起点是**增加 trace 事件的有损旋转机制**（`trace rotate`），同时记录 `run_id`——否则 `--summary` 只能做最粗糙的全局聚合。

---

## 优先级重新审视

我基本同意你的 P0/P1/P2 排序，但有两点调整建议：

| 方向 | 原优先级 | 建议 | 理由 |
|---|---|---|---|
| 三 风险提取 | **P0** 安全·路由正确性 | **P0** → 维持 | 六个评分维度中五个没有信号生产者，这是路由系统的"功能未完成" |
| 五 合约验证 | **P0** 数据完整性 | **P0** → 维持 | 已有真实 bug，且是快速可修的（加 schema 字段 + 校验器） |
| 一 阈值自校准 | P1 学习闭环 | **P1 → 降为 P2** | 缺少分维度 scorecard 数据，v1 实际能做的不多，建议等更多 scorecard 数据积累后再启动 |
| 二 预测估算 | P1 成本可观测 | **P1 → 维持** | trace 数据已完整，v1 的 `--dry-run --predict` 价值清晰、风险低 |
| 四 失效分类 | P2 运维智能 | **P2 → 维持** | 同意作为纯收益方向，建议与 trace rotate + run_id 一起推进 |

方向一降为 P2 的理由补充：当前 scorecard 只有 `quality_score` 标量，没有保存历史 route 的多维得分到阈值的映射关系。要有意义地自动校准 0.34/0.69，需要**在 trace 事件中记录每次路由的 `Score()` 输出**——目前未记录。加了这个基础设施后自校准才有输入。

---

## 总结

| 维度 | 评价 |
|---|---|
| 代码证据完整性 | ⭐⭐⭐⭐⭐ 逐包逐函数验证，证据链完整 |
| 事实准确度 | ⭐⭐⭐⭐ 两处小误差（verdictLedger 类型、dims 默认值），不影响方向级判断 |
| 先验去重 | ⭐⭐⭐⭐⭐ 方法严谨，检索全面 |
| 优先级判断 | ⭐⭐⭐⭐ 基本合理，方向一建议降为 P2 |
| 扩展范围设计 | ⭐⭐⭐⭐⭐ v1/v2/v3 渐进式扩展设计好，每个阶段有明确边界 |
| 产品价值 | ⭐⭐⭐⭐⭐ ROI 阐述清晰，尤其是方向三+五的 P0 justification |
| 总体 | **优秀**，是一份可以进入 ROADMAP 讨论的结构化扩展提案 |
