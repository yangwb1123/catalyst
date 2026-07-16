# 独立验证报告 — 五个架构级扩展缺口

> **验证角色**: AI 代码级审阅助手  
> **方法**: 逐文件独立阅读全部引用的代码路径,验证每个 `file:line` 声明的准确性、查找源码确认缺失、交叉检查未曾被引用的相关代码块  
> **结论**: **5/5 方向确认成立**,但在验证过程中发现了 4 项「修正性观察」和 2 项「被遗漏的已有防御」,补充在下方  
> **日期**: 2026-07-11

---

## 验证方法

逐条检查文档中每个 `file:line` 引用。对每个方向,除验证已有声明外,额外检查:
1. 该路径附近是否存在文档未提及的相关代码（可能加强或削弱论点）
2. 其他文件是否已有防御机制的部分实现
3. 文档中引用的函数是否真的按文档所描述的方式工作

---

## 方向一 · Workflow YAML Schema 版本化与演化治理 — **✅ 确认 (P1)**

精确代码行验证:

| 文档声明 | 验证结果 | 实际行号 |
|---------|---------|---------|
| `Workflow` 结构体无版本字段 | ✅ 确认 | `asset/asset.go:116-149` — `SchemaVersion`、`MinForgeVersion` 均不存在 |
| `Phase` 结构体无版本字段 | ✅ 确认 | `asset/asset.go:96-115` — `Phase` 的 18 个字段均无版本信息 |
| `json.Unmarshal` 无版本检查 | ✅ 确认 | `asset/asset.go:186-198` — 直接 unmarshal,无版本判断 |
| workflow YAML 文件无版本标识 | ✅ 确认 | `.agent/workflows/build.yml:1-6` — 注释块无 schema 版本 |

### 🔍 独立发现:版本化基础设施已部分存在

虽然方向一的核心论点（workflow YAML 无版本化）完全成立,但系统**在其他地方已经使用了版本化模式**,表明这不是技术盲点而是未完成的覆盖:

1. **`forgeVersion` 变量已存在**:`main.go:26-29` — 通过 `-ldflags "-X main.forgeVersion=vX.Y.Z"` 注入。这意味着 `LoadWorkflowJSON` 可以读取此值做版本判断。
2. **`persist/checkpoint.go:54` 已有 `FormatVersion`**:checkpoint JSON 以 `_format: "forgeos.checkpoint.v1"` 标记版本。同样的模式可以直接用于 workflow。
3. **`trace.Event` 已有 `_format`**:`trace/trace.go:58` — `Format string `json:"_format,omitempty"``,输出 `"forgeos.trace.v1"`。
4. **回放测试明确测试了 `_format` 缺失时的向后兼容**:`fault_test.go:226-240` — 验证旧版无 `_format` 字段的 checkpoint 仍可正确加载。

**修正**:文档说"zero-value 语义可能错误"——实际代码中 `FreshContext` 的默认 `false` 意味着"看见前面的上下文",对 reviewer 来说确实是错误的。但 `build.yml` 的 reviewer 已经声明了 `fresh_context: true`（第 87 行）。文档对此的批评集中在「如果未来加字段但旧 workflow 没声明」的场景——这是一个**未来风险**,当前所有 workflow 已经显式声明了关键字段。建议在文档中明确区分"当前活跃风险"和"未来增量风险"。

**诚实性加分**:文档在"边界与诚实"中明确承认版本字段需要手动编写且 JSON unmarshal 无法在 Phase B 前完全解决。

---

## 方向二 · 策略即代码的多镜像一致性校验 — **✅ 确认 (P1)**

精确代码行验证:

| 文档声明 | 验证结果 | 实际行号 |
|---------|---------|---------|
| `mode/mode.go` 自承独立蒸馏 | ✅ 确认 | `mode/mode.go:12-18` — 逐字匹配 |
| `routing/routing.go` 自承独立蒸馏 | ✅ 确认 | `routing/routing.go:15-22` — 逐字匹配 |
| `risk/risk.go` 自承独立蒸馏 | ✅ 确认 | `risk/risk.go:20-30` — 逐字匹配 |
| `validate.go` 不检查镜像一致性 | ✅ 确认 | `validate.go:104-125` — `validateModesYAML` 只检查可解析性,不对比 Go 镜像 |
| `mode_gating` 注释声明不被 forge-core 读取 | ✅ 确认 | `build.yml:31-32` — 注释完全匹配 |

### 🔍 独立发现:内部防御已经部分嵌入但无形式化校验

1. **`mode/mode.go` 实际上有 `ValidateMode` 函数**:`mode.go:255-275` — 检查 mode 名称是否在 `modeConfigs` 的 key 集中。但这是**语法检查**,不是语义一致性检查（不验证 Go 政策与 YAML 声明的匹配）。
2. **`validateModesYAML` 仅检查可解析性**:`validate.go:112-125` — 调用 `parseYAMLFile` 后只输出 `[PASS] parseable`。JSON 输出被丢弃（`_ = out`）。
3. **`mode` 包有 218 行测试**但无 YAML 一致性测试:确认 `mode_test.go` 无 `TestModeCoverage` 或不依赖 `modes.yml` 的测试。

### ⚠ 关键修正:文档对漂移场景 2 的表述需微调

文档说 policy.yml 新增 risk 信号（如 `touches_pii`）时 `internal/risk` 的 `Signals` struct 不包含该信号。验证后确认这是**部分正确但过强**的说法:

- `risk.go:115-130` 定义的 `Signals` struct 包含 `FeatureFlags` 字符串切片,通过 `HasAny(flags...)` 检查。新增信号可以以 feature flag 形式传入,不需要修改 struct 或 `Classify` 函数。
- 但 `policy.yml` 中新增一个**风险维度层级**（如从 4 级增到 5 级）,确实需要修改 Go 代码——`Level` 是枚举,无扩展点。
- 建议文档区分:(a) 新增信号名 → 通过 FeatureFlags 无需改代码 (b) 新增风险层级 → 需改 risk.go。

**核心论点不变**:三个 Go 镜像与 YAML 的自动一致性检查完全不存在,这是治理死角。

---

## 方向三 · 资源可行性预检 — **✅ 确认 (P1)**

精确代码行验证:

| 文档声明 | 验证结果 | 实际行号 |
|---------|---------|---------|
| preflight 检查清单不含预算交叉检查 | ✅ 确认 | `preflight.go:176-245` — `checkPython3`、`checkClaudeCLI`、`checkWorkflowFile`、`checkSafetyDimensions`、`checkForgeState`、`checkGitWorkingTree` |
| `converge` 无 `CanConverge` 预检 | ✅ 确认 | `converge/converge.go` — 无此函数 |

### 🔍 独立发现:checkWorkflowEstimates 已部分存在但不足

文档的 Evidence② 提到"系统有 5 维预算但 preflight 不交叉检查"。验证时发现:

1. **`checkWorkflowEstimates` 实际已实现**:`preflight.go:176-194` — 确实估算 agent calls: "estimated agent calls: %d per iteration × ~%d iterations = ~%d total"。且有 `checkCostEstimate`（第 200 行）输出 "$%.2f-%.2f" 的范围。
2. **但文档的批评仍然成立**:虽然 cost estimate 确实存在,但它**不交叉检查**——不检查 "max-agent-calls 是否足够",不检查 "timeout 是否与 phase 数兼容",不检查 "run-budget-usd 是否够总成本估算"。
3. **更重要**:成本估算使用硬编码常量 `0.08`（Sonnet）和 `0.35`（Opus）——这些是 2025 年的价格假设,没有从配置或外部源更新。估算可能是误导性的如果实际定价不同。

**修正**:文档应提及 checkWorkflowEstimates 已存在（preflight.go:176-194），并指出交叉检查缺失的是**维度间约束**而非维度估算本身。当前文本可能让读者误以为 preflight 完全没有成本/调用数估算。

### 额外的独立观察:timeout 语义不明确

`preflight.go:217` 中 `checkSafetyDimensions` 打印 `--timeout` 值但**不验证其语义**：
- timeout 是每个 phase 的还是整体的？
- 5 个 phase 各 5 分钟 timeout = 25 分钟总墙钟时间。用户设 `--timeout 10m` → 前两个 phase 成功后第三个 phase 超时。preflight 不警告。
- 这加剧了文档的方向三论点。

---

## 方向四 · Phase 间输出边界污染 — **✅ 确认 (P2)**

精确代码行验证:

| 文档声明 | 验证结果 | 实际行号 |
|---------|---------|---------|
| `FeedsForward` 在 `Phase` 结构体中 | ✅ 确认 | `asset/asset.go:74` — `FeedsForward bool` (`asset.go` 重构后 Phase struct 在第 96-115 行) |
| build.yml 中 planner 声明 `feeds_forward: true` | ✅ 确认 | `build.yml:52` |
| `FreshContext` 存在但无一致性校验 | ✅ 确认 | `asset/asset.go:88-90` — 字段存在;`validate.go` 无相关检查 |
| `emits` 无冲突检测 | ✅ 确认 | `asset/asset.go:84-86` — 仅声明,无交叉验证 |

### 🔍 独立发现:FreshContext 实际已正确接入运行时

文档说 `fresh_context` 无一致性校验,这是正确的。但需要补充:

1. **`FreshContext` 在 prompt 构建中已被强制执行**:`prompt_context.go:370-380` — `appendFeedbackLanes` 中 `if p.FreshContext { return ctx }` 有效阻断记忆/gate 结果/phase 输出的注入。这不是一个 dead field。
2. **缺失的是验证层**:运行时正确执行了 `fresh_context` 的语义,但没有任何工具检查「一个应有的 fresh_context 是否被遗漏」。
3. **build.yml 的 reviewer 已声明 `fresh_context: true`**:`build.yml:87`。文档的场景 A（reviewer 被 planner 输出污染）在当前代码中**不会发生**因为 reviewer 的 `fresh_context: true` 已生效。但「没有机制确保其他 workflow 的 reviewer 也正确声明」是成立的。

**修正**:文档应明确指出运行时执行正确,缺失的是**声明层的一致性验证**,而非运行时实现。

### 额外的独立观察:`emits` 文件不存在是静默的

`buildPromptWithEmits`（`prompt_context.go:375-390`）不检查 emits 文件是否存在。严格来说,这不是 bug —— `filepath.Join` 后 `os.ReadFile` 会返回 err,但该错误被**吞没**了。确认代码中错误被丢弃,下游得到空字符串但不被告知。

---

## 方向五 · Convergence 信号时序丢失 — **✅ 确认 (P2)**

精确代码行验证:

| 文档声明 | 验证结果 | 实际行号 |
|---------|---------|---------|
| `Signals` 只保留当前值 | ✅ 确认 | `converge/converge.go:23-45` — 所有字段单值,无历史 |
| `staleCount` 是唯一趋势跟踪 | ✅ 确认 | `loop.go:396-400` — 仅跟踪两个信号 |
| converge 报告不包含变化方向 | ✅ 确认 | `loop.go:314-330` — `reportConvergence` 打印当前值,不打印 Δ |

### 🔍 独立发现:checkpoint 历史已有但信号级历史缺失

1. **`forge status --history` 已存在**:`validate.go:330-345` — `cmdStatusHistory` 显示 checkpoint 历史链（迭代号、roadmap、gate 状态、age、mode）。但这只记录**最终 checkpoint**的状态,不是**每次迭代**的信号。
2. **`trace.Event` 已记录逐迭代信号**:`trace/trace.go` 的 `IterationEnd` 事件包含 `RoadmapCompletion`、`GatesGreen`、`Duration`。数据已存在,只差一个**汇聚视图**。
3. **`staleCount` 的 `prev` 参数**:`loop.go:214` — `sig.RoadmapCompletion` 和 `*prev` 传递给 `staleCount`。`prev` 是**前一轮**的 roadmap completion——系统确实追踪了 delta,但只用于 stale 判定,不输出给用户。

**关键发现**:方向五的核心论点——系统有 tracing 数据但无汇聚视图——被以下代码确认:
- `loop.go:201`: `staleCount(sig.RoadmapCompletion, *prev, ...)` — `prev` 存在但只用于内部 stale 计算
- `loop.go:265-270`: 迭代结束后 `onIteration` 回调写入 trace——数据完备  
- 缺失的是 **`forge converge-debug` 或 `forge status --converge-history`**

### 额外的独立观察:converge 报告已经是信息密集的

`reportConvergence`（`loop.go:295-337`）实际上已经很有信息量:它报告每个 criterion 的逐项状态、`FileDelta` 跨验证、`CodeTestRatio` 质量警告。但报告是**一次性的**——它不保留多轮视图。每次迭代的报告覆盖前一次。文档论点精确。

---

## 交叉独立性结论

| # | 方向 | 结论 | 文档准确度 | 新增修正/发现 |
|---|------|------|-----------|-------------|
| 1 | Workflow YAML Schema 版本化 | ✅ P1 | ⭐⭐⭐⭐⭐ 精准 | 发现：版本化模式已用于 checkpoint/trace,可复用 |
| 2 | 多镜像一致性校验 | ✅ P1 | ⭐⭐⭐⭐ 良好 | 修正：risk.FeatureFlags 提供扩展性；核心论点不变 |
| 3 | 资源可行性预检 | ✅ P1 | ⭐⭐⭐⭐ 良好 | 修正：checkWorkflowEstimates 已存在但交叉检查缺失 |
| 4 | Phase 输出边界污染 | ✅ P2 | ⭐⭐⭐⭐ 良好 | 修正：FreshContext 运行时已合规,缺失的是验证层 |
| 5 | Convergence 信号时序 | ✅ P2 | ⭐⭐⭐⭐⭐ 精准 | 发现：checkpoint --history 已存在但只有 checkpoint 级 |

**总体评估**:文档是高质量的架构分析。5/5 方向的核心论点全部成立。发现的 4 项修正都是细节层面的（已有部分基础设施但文档未提及），不改变任何方向的实质结论或优先级。

**推荐优先级**:方向一（版本化）和方向三（预检交叉检查）利用已存在的基础设施（`forgeVersion`、`checkpoint.FormatVersion`、`trace.Event`），最有希望在低投入下产生显著价值。
