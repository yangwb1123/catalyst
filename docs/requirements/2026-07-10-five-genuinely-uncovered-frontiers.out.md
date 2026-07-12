这是一个非常详实的分析。我来对您的 5 个方向做交叉验证——对照 `docs/requirements/` 下 **63 篇 Jul 11 新分析 + 30 篇 Jul 10 分析**，逐方向评估"真实未覆盖"程度。

---

## 交叉验证结果

### 方向一 · HTTP API / SDK 面 — ⚠️ **已被多篇覆盖，核心论点不新**

已覆盖证据:
- `2026-07-11-forgeos-three-architectural-gaps.md:22` — 方向三"运行时外部集成面"，明确标注 P1："零网络代码、零 HTTP API、零 event stream、零 metrics export"
- `2026-07-11-five-foundational-architecture-gaps.md:189-204` — 方向二"可嵌入性"，明确说是 API 面的**必要条件**，并承认"HTTP API 面已被 ~8 篇覆盖"
- `four-truly-unexplored-architectural-gaps.md:430` — 提及 `forge serve` HTTP server
- `2026-07-11-five-foundational-architecture-gaps.md:533` — 明确总结："方向二(可嵌入性)不是 HTTP API 面(已被 ~8 篇覆盖)"

**您的具体贡献**: Read API/Command API/Webhook/SDK 的分层渐进策略（v1→v2→v3）比现有文档更具体。但核心 gap 识别——"纯 CLI 黑箱"——已被 ~8 篇覆盖，包括一篇完整方案（Unix socket API + webhook 注册）。

**结论**: ⚠️ **相邻但不新**。代码证据扎实但缺口识别已被覆盖。

---

### 方向二 · 多仓库/Workspace — ⚠️ **已被多篇覆盖，且方案更详细**

已覆盖证据:
- `2026-07-11-five-architectural-product-expansion-directions.md:12-50` — 方向一就是"跨仓库工作区编排（Workspace Orchestration）"，详细到 CLI 示例 `forge workspace init --repos catalyst-api,catalyst-web,catalyst-infra` + `forge run --workspace catalyst --workflow cross-repo-deploy` + depends_on DSL
- `strategic-expansion-v39.md:71-134` — 完整 workspace 方案：`workspace.yml`、跨仓库 gate、原子性、代码量估算(~3000行)
- `expansion-horizon-three.md:118` — 明确指出 `asset.Workflow` 没有 `depends_on_repo` 或 `workspace` 字段

**您的具体贡献**: "联邦 scorecard" 和 "原子发布 gate" 是现有文档未提及的具体机制。但核心 gap——单仓库假设 + workspace 概念——已被 2+ 篇详细覆盖。

**结论**: ⚠️ **相邻但不新**。联邦 scorecard 是增量贡献，但核心论点不新。

---

### 方向三 · 人类反馈回路 — ✅ **本质上未被覆盖**

现有文档中相邻内容:
- `product-deployment-transparency-five-gaps.md:251` — 一句话提及 `forge feedback <session-id>`，但没有展开为系统方案
- `five-uncovered-horizontal-frontiers.md:570` — 提及 `forge compare`，但用于 fork 比较而非人类反馈

**未被覆盖的证据**:
- 现有 docs 没有一篇提出 `KindFeedback` 作为 memory 的第四种 kind
- 没有一篇提出 `HumanRating` / `HumanVotes` / `CorrectionCount` 字段加入 scorecard
- 没有一篇提出 `forge feedback --kind correction --target "决策ID" --rating 3` 具体命令设计
- 人类反馈作为 scorecard 权重、memory 持久化、路由决策矫正的**系统性管道**——零覆盖

**结论**: ✅ **真实未覆盖**。您的代码证据（memory.go 三种 Kind / scorecard 无 HumanRating）精确，且扩展方案具体、改动量小（几十行）、ROI 高。这是 5 个方向中**差异度最高**的一个。

---

### 方向四 · 部署/生产闭环 — ⚠️ **部分覆盖，核心增量存在**

已覆盖内容:
- 多个文档提到 deploy workflow 作为 workflow 扩展方向
- `2026-07-11-five-systemic-architectural-gaps.md:53-110` — 讨论 deploy workflow 参数化（staging vs production）
- `five-high-value-extensions-v44.md:283` — 提及 `rollback.yml`

**您的增量贡献**:
- `converge.Signals` 加入 `ProdErrorRate` / `ProdP95Latency` / `ProdTrafficDrop` — **未被任何现有 doc 提出**
- 生产信号恶化自动触发 re-evolve — **未被提出**
- deploy → monitor → re-evolve 全闭环 — **未被提出**

已有文档停步在"加一个 deploy workflow YAML"，没有深入到收敛引擎纳入生产信号作为停止条件。

**结论**: ⚠️ **部分覆盖 + 有实质增量**。deploy phase 概念已被覆盖，但生产信号纳入 converge 引擎是**全新贡献**。

---

### 方向五 · 模板/蓝图生态 — ⚠️ **部分覆盖**

已覆盖内容:
- `2026-07-11-forgeos-product-output-quality-and-ecosystem-gaps.md:6-7` — 讨论 scaffold 模板质量、`forge init --from template-registry`
- 多个文档提及 `forge-init` 的脚手架局限性

**您的增量贡献**:
- `extends: org/node-service@v1` 继承机制 — **未被提出**
- `forge template push/pull/list` 子命令设计 — **未被提出**
- 版本收敛检测（`forge validate --drift`）对比各 repo 与模板声明的差异 — **未被提出**

但核心 gap——"forge-init 是裸脚手架"——已在 `scaffold` 相关分析中被提及。

**结论**: ⚠️ **部分覆盖 + 有实质增量**。继承机制和 drift 检测是新的，但缺口识别不新。

---

## 总体评估表

| 方向 | 核心 gap 是否新 | 提案细节是否新 | 代码证据 | 差异化价值 | 评估 |
|------|:---:|:---:|:---:|:---:|:---:|
| 一 · API/SDK | ❌ 已被 ~8 篇覆盖 | ⚠️ 分层策略略新 | ✅ 精确 | ★★ | 相邻 |
| 二 · 多仓库 | ❌ 已被 3+ 篇覆盖 | ⚠️ 联邦 scorecard 新 | ✅ 精确 | ★★ | 相邻 |
| 三 · 人类反馈 | ✅ **全新** | ✅ **全新** | ✅ 精确 | ★★★★★ | **真正未覆盖** |
| 四 · 生产闭环 | ⚠️ deploy 概念已覆盖 | ✅ **converge 信号全新** | ✅ 精确 | ★★★★ | 部分新 |
| 五 · 模板蓝图 | ⚠️ scaffold 局限已覆盖 | ✅ **extends/drift 全新** | ✅ 精确 | ★★★ | 部分新 |

---

## 建议

如果要将此文**保存为文档**并输出正式交叉验证结果，我建议:

1. **方向三保留并作为"唯一真正的全新方向"突出**，其他 4 个方向需要引用现有文档并声明差异点
2. **方向一缩小为 API 分层策略（Read/Command/Webhook/SDK）的实施方案**，而非 gap 识别
3. **方向四聚焦 converge 信号扩展**（这是真正的增量），弱化 deploy phase（已被覆盖）
4. **方向五聚焦继承机制 + drift 检测**，弱化 scaffold 局限（已被覆盖）

---

请告知您的意图——是要将此文档保存到文件并生成正式的 `.out.md` 验证报告，还是直接讨论/使用这些方向？
