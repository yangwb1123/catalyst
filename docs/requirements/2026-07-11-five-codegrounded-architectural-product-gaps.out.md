已保存文件：

- **原始分析**: `docs/requirements/2026-07-11-five-architectural-extension-gaps-deep-scan.md`（22.8 KB）
- **独立验证**: `docs/requirements/2026-07-11-five-architectural-extension-gaps-deep-scan.out.md`（7.6 KB）

---

## 验证概要

我对每个方向的所有 `file:line` 引用做了逐行全路径验证,结论:**
5/5 方向核心论点全部成立**。我在验证中发现了 4 项修正性观察:

### 方向一 (版本化) — 基础设施已存在但未复用
`checkpoint.go` 已有 `_format: "forgeos.checkpoint.v1"`, `trace.go` 已有 `_format`, `main.go` 已有 `forgeVersion` 构建注入变量。**版本化模式已在系统中使用,只差应用到 workflow YAML 上。** 这降低了 Phase A 的投入估算——不用新造轮子。

### 方向二 (多镜像一致性) — FeatureFlags 提供部分扩展性,但风险层级仍需改代码
`risk.go` 的 `FeatureFlags` 字符串切片允许新增信号名（如 `touches_pii`）**不修改 Go 代码**。但新增风险**层级**（4→5 级）仍需改 Go。建议在你的场景 2 中区分这两者。

### 方向三 (预检) — checkWorkflowEstimates 已存在但无交叉检查
`preflight.go:176-194` 已有 phase 数估算和成本估算（硬编码 $0.08/$0.35）。**缺失的不是维度估算而是维度间交叉约束检查**——timeout vs phase 数、budget vs 估算成本。已在验证报告中标出精确行号。

### 方向四 (边界污染) — FreshContext 运行时已正确执行
`prompt_context.go:370-380` 的 `appendFeedbackLanes` 中 `if p.FreshContext { return ctx }` 有效阻断上下文注入。build.yml 的 reviewer 已经声明了 `fresh_context: true`。**缺失的是验证层而非执行层**——无人检查是否所有应有的 phase 都声明了 FreshContext。

### 方向五 (Convergence 信号) — 最精准的章节
forge status 已有 `--history` 显示 checkpoint 链,但只有每轮**最终 checkpoint** 的快照。trace 数据完备（`loop.go:265-270` 每轮迭代写入 trace），但**无汇聚视图**。一个 `forge converge-debug` 子命令可能是最低投入高回报的改进。
