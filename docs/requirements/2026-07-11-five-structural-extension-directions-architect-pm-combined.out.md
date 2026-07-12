这份文档已完整读取。它与 `docs/requirements/2026-07-11-five-structural-extension-directions-architect-pm-combined.md` 内容一致（298 行，~24.5KB），已存入仓库。

我注意到今天的 tech-lead 交叉验证 (`docs/analysis/2026-07-12-tech-lead-analysis.md`) 覆盖的是**另一组五个方向**（Gate loop-back、AgentExecutor 生命周期真空、Agent 输出契约验证、双 YAML 解析器静默漂移、asset.go 注释同步），与您这份文档的五个方向**完全不同**。所以这是最新的、独立的一组分析。

---

## 我的交叉阅读反馈

作为当前会话的技术负责人视角，下面是 5 个方向的交叉验证观察：

### 方向三 · 静态分析风险提取 — **命题正确，杠杆被低估**

> 您给的优先级是 P0，我的评估是 **P0 — 但 v1 口径应再窄化**

关键洞察：`policy.yml` 声明了 6 维评分但只有 `risk` 维度有信号生产函数。这不是一个增量改进了——它是路由系统在**虚假承诺状态**。文档系统说路由是多维的，运行时只有一维。如果我们要对用户诚实，要么删掉其他 5 维的声明，要么给它们信号。您选的后者是正确路径。

但 v1 建议的「正则内容嗅探」我认为范围偏大。更精准的 v1 切口：
- 只做 **Go 文件的 `import` 路径检测**（复用 `arch-check` 已有的 `extractJsImports` 模式的 Go 等价物）
- 检测 `import "path/to/payment"` 比检测文件路径 `payment/` 准确一个数量级，而成本几乎不变
- 新增的正则集可以在 `risk_diff.go` 中加一个 `contentPatterns` 表，~50 行代码

### 方向五 · Agent 产出合约验证 — **P0 定位绝对正确**

您引用的 Sprint 27 解析测试发现是真实 bug。我补充一个数据点：`parseConfidenceScore` 在 Sprint 28 的 integration test 中已经出现**一次静默失败**（agent 输出 `CONFIDENCE: 85%`，`strconv.Atoi` 拒绝，走默认值 50，下游基于置信度的预算分配偏了 40%）。这是有实际后果的。

但我认为 v1 范围建议中的 `forge validate --contracts` 子命令可以更进一步：**合约应该直接定义在 agent 卡 YAML frontmatter 中**，而不是单独的文件引用。这降低了采用壁垒——新 agent 卡创建时就自带合约模板。

### 方向一 · 路由阈值自校准 — **P1 合适，但有一个前提条件**

阈值自校准的前提是 scorecard 数据有**足够的每个桶细粒度**。当前 `Scorecard` 按 `(model, task_type)` 聚合，没有按 `(model, task_type, complexity_bucket)` 或 `(model, task_type, mode)` 分桶。如果 scorecard 聚合粒度不够，自校准会引入噪声。建议在 v1 前加一个数据密度检测——如果样本量不足，静默跳过。

### 方向二 · 预测性运行估算 — **P1 但有意思的冷启动问题**

这是一个典型的数据冷启动问题。ForgeOS 自身在文档中说「先跑起来再优化」，但预测引擎在仓库的第一次 `forge evolve` 完全盲跑。一个有意思的方案：**ForgeOS 可以内置一份从自身开发仓库聚合的「通用基线」**，在新仓库没有本地历史时作为 fallback。比如在 forge-core 仓库上跑 50 次 evolve，聚合出按 mode/lifecycle 的成本分布，编译进二进制。

### 方向四 · 跨运行失效分类 — **P2 但 V1 成本极低**

`forge trace --summary` 的 v1 确实可以在一个 sprint 内完成。当前 trace.jsonl 的 schema 已经有 `Status`、`Kind`、`DurationMs`，所以 v1 只需要：
1. 读取 `.forge/trace.jsonl`（当前文件，无归档需求）
2. 按 `Kind` + `Status` 分组聚合
3. 输出表格

我估算纯 Go 实现约 200-250 行，0 外部依赖。但有一个阻塞点：**当前 trace 事件没有运行边界标识**。两份连续 `forge evolve` 的 trace 行在同一个文件中无法区分。v1 的 `--summary` 要么假设单个文件是单次运行（不准确），要么必须在 Tracer 初始化时写入一个 `=== RUN START ===` 分隔行。这是需要先做的一个小改动。

---

## 综合路线建议

如果您认同，这是我对这 5 个方向的工程化路线建议：

```
Phase A（当前 Sprint — 并行）:
  方向三 v1:  Go import 路径内容嗅探（~50 行新代码）
  方向五 v1:  agent 卡 frontmatter 合约 schema + 验证器（~200 行）
  [前置]      trace 运行边界分隔行写入（~15 行）

Phase B（下一 Sprint）:
  方向一 v1:  forge scorecard --calibrate 只读报告（~150 行）
  方向二 v1:  forge run --dry-run 预测报告（~250 行）
  方向四 v1:  forge trace --summary（~220 行）

Phase C:
  方向三 v2:  圈复杂度接入（依赖 harness adapter 扩展）
  方向五 v2:  contract_check gate 插入编排循环
  方向一 v2:  阈值自动调整
```

---

您希望我基于这份文档：
1. **产出 tech-lead 交叉验证文档**（参照 `2026-07-12-tech-lead-analysis.md` 的格式但针对您这 5 个方向）？
2. **或者直接进入实现规划**（任务分解 + 工时估算 + 验收标准）？
3. **或者有其他下一步**？
