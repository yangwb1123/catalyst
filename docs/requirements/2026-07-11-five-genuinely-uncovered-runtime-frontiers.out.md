Now I have enough data for a comprehensive verification. Let me compile the results.

---

# 差异化验证报告: `2026-07-11-five-genuinely-uncovered-runtime-frontiers.md`

对文档中 5 个方向逐一核查现有 `docs/requirements/` 中的分析,确认各方向中心命题是否**已被作为独立系统性缺口展开过**。

---

## 方向一 · 并行 Agent 的写冲突与一致性
### ❌ 差异化声明不成立

**已在此文档中被完整覆盖:**

`2026-07-11-five-systemic-architectural-gaps.md` → **方向三 · 并发 Phase 输出冲突检测与合并** (第 233–318 行)

逐项对比:

| 维度 | 新文档 | 既有文档 |
|------|--------|---------|
| 文件证据 | `parallel.go:58-80`, `command_executor.go:42-60` | 同一组: `parallel.go` (~80 行), `command_executor.go` `Dir` 字段 |
| 锁分析 | `sync.Mutex` 只守护 `agentCalls` 计数 | 8 层 LOCK ORDER CONTRACT, 全部内存锁 |
| 核心问题 | 后写者静默覆盖前者 | 完全一致: "后完成的 writer 静默覆盖先完成的" |
| feeds_forward 问题 | 无并发合并处理 | 同一发现: `prompt_context.go` 假设串行 |
| 建议方向 | 声明式 `writes:` 字段 + wave 交集检测 | 同一方案: 预执行快照 + 执行后 diff + 冲突检测 + 合并 |
| 边界场景 | 4 个(同文件 append/目录非原子/git 交叉/feeds_forward) | 4 个(文件范围冲突/重命名删除/非文本/依赖推断) |

**结论**: 核心命题、证据链、建议方案全部**高度重合**。新文档的差异化声明不成立。

---

## 方向二 · Prompt 上下文容量规划与预算执行
### ❌ 差异化声明不成立

**已在多篇文档中被完整覆盖,最直接的是:**

`architectural-expansion-perspectives.md` → **方向四：上下文窗口预算管理** (第 378–496 行)

逐项对比:

| 维度 | 新文档 | 既有文档 |
|------|--------|---------|
| 代码证据 | `prompt_context.go` 逐项注入 8 个通道 | 同一: `buildPrompt` 7 部分拼装, `prompt_memory.go` `memoryCap=32` |
| 核心问题 | 不测量总 token, 无降级机制 | 同一: "无主动 budget 管理", "size 10-35 KB", 被推到边缘 |
| token 估算 | 建议轻量估算器 | 同一: "4字符≈1 token", 3:1 英文保守比 |
| 降级策略 | 优先降级 memory, ADR topK 动态缩减, emit 摘要 | 同一: `memory.Query` 热度排序 + budget 限额, `Strategy: "truncate" | "priority" | "retrieve"` |
| 产品价值 | 大仓库 agent 质量保持 | 同一: 长 evolve 确定性的故障 |
| 设计细节 | 未给出结构定义 | 给出完整 `Budget` struct 设计: `MaxTokens`, `UsedTokens`, `Strategy`, `Priorities` |

此外,`expansion-production-blindspots-v36.md` 第 591–728 行也有完整的 `budget.go` 设计和 `--max-context-tokens` flag 方案。

**结论**: 核心命题、代码证据、解决方案均已被完全覆盖,且既有文档的设计更详细。差异化声明不成立。

---

## 方向三 · 模型路由决策的运行时可验证性
### ⚠️ 部分新颖 — 中心命题未被完全展开,但有显著相邻覆盖

**相邻覆盖:**

1. `five-uncovered-operational-gaps-2026-07-10.md` 方向二 — 讨论 `BudgetAdjustTier` 降级**不写 DecisionEvent trace**, 建议加 `prev_tier`/`new_tier`/`spendRatio`/`reason`
2. `2026-07-11-five-product-architectural-expansion-directions-scanned.md` — 提到 `TierForScore`/`BudgetAdjustTier` 加 `decisionLogger` 回调 "记录每一跳"
3. `2026-07-11-forgeos-five-code-grounded-systemic-gaps.md` — 讨论多维评分引擎不接入执行路径, routing 缺口

**新文档特有内容** (未被覆盖):
- **路由合规报告**: 对比每个 phase 的「路由决定」vs「trace 记录的实际模型」,偏离则告警
- **`trace.Event` 的 `Model` 字段存在但从不被 `cmd/forge` 的 cost path 填充** — 这是精确到字段级别的未覆盖证据
- **API 兼容升级导致实际模型改变、路由决策不感知**这一具体场景
- **跨厂商 ModelMap 与 `policy.yml` 一致性验证** — 增量位置

**结论**: 中心命题的**前半部分**(BudgetAdjustTier 降级不记录)已被覆盖。但**后半部分**(实际执行模型 vs 路由决策的闭环对比验证 + 合规报告)尚未作为独立系统性缺口展开。该方向**部分新颖**。

---

## 方向四 · 阶段级幂等性 — 宕机恢复的「部分写入」问题
### ❌ 差异化声明不成立

**已在多篇文档中被完整覆盖:**

1. `execution-semantic-gaps.md` → **方向一：Phase 执行副作用模型——原子性、幂等性与回滚的缺口** (第 32–150 行)

逐项对比:

| 维度 | 新文档 | 既有文档 |
|------|--------|---------|
| 核心场景 | checkpoint 写好后 agent phase 中途崩溃 → 文件已改但 checkpoint 未落盘 | 同一: "Crash 发生在 agent 写文件到一半时 → 部分写入的文件, resume 后继续" |
| 文件证据 | `command_executor.go` `CombinedOutput` → checkpoint 前 | 同一: `command_executor.go:runMeasured` 无 phase 前文件清单, 无 phase 后 diff |
| loop-back 场景 | 无清理, 残留旧文件 | 同一: "Loop-back 重跑 implementer, 第一次的产物在磁盘上 → 累计 side effect" |
| 建议方向 | 文件系统快照 + `git stash`/变更集 snapshot | 同一: "执行前记录文件清单 → 执行后 diff → Loop-back 前回滚" |
| 并行场景 | RunParallel 无 per-phase checkpoint | 同一: "并行模式下 side effect 叠加更不可控" |
| 诚实标注 | 只保护 git-tracked 文件 | 同一: "不依赖 git—文件清单 + SHA256 就可以做 diff 和回滚" |

2. `forgeos-five-architect-product-perspective-2026-07-10.md` → **方向二 · 部分失败下的工作区完整性** (第 140–240 行)

用更精细的证据重复覆盖同一方向:
- `command_executor.go:101-118` 无原子写入字段
- `checkpoint.go:45-62` 无 WorkspaceHash/FileManifest
- `forge resume` 不验证工作区

**结论**: 核心命题、证据链、建议方案在**至少 2 篇既有文档**中被完整覆盖。差异化声明不成立。

---

## 方向五 · 跨提供商计费与延迟遥测的抽象层缺口
### ❌ 差异化声明不成立

**已在以下文档中被完整覆盖:**

`forgeos-five-architect-product-perspective-2026-07-10.md` → **方向三 · 多厂商 Agent 输出归一化层** (第 240–335 行)

逐项对比:

| 维度 | 新文档 | 既有文档 |
|------|--------|---------|
| 文件证据 | `cost.go` `parseClaudeCostResponse` 硬编码 claude JSON | 同一: `cost.go:14-20` 注释自称 "claude-specific cost-telemetry boundary", `parseClaudeCostJSON` |
| ModelMap 硬编码 | `routing.go:188-197` 仅 anthropic | 同一: `ModelMap` 仅含 anthropic, `Providers()` 永远返回 `["anthropic"]` |
| ScorecardPair | 无 `Provider` 字段 | 同一: `trace.Event.Model` 有 model 名但无 `Vendor` 字段 |
| 建议方向 | `BillingParser` 接口 | 同一: `OutputParser` 接口 (`ParseCost` + `ParseVerdict` + `ParseConfidence`) |
| 影响文件 | 5 个不相关文件 | 同一: `cost.go` + `routing.go` + `route.go` + `scorecard` + `policy.yml` |
| 产品价值 | 北极星架构的第一步 | 同一: "跨厂商模型池...但从未将不同厂商输出归一化作为独立架构层" |

**结论**: 核心命题、代码证据链、建议接口模式、影响范围全部**完全重合**。差异化声明不成立。

---

## 综合判定

| # | 方向 | 差异化声明 | 实际状态 | 既有文档 |
|---|------|-----------|---------|---------|
| 1 | 并行写冲突 | ❌ 不成立 | 已被覆盖 | `2026-07-11-five-systemic-architectural-gaps.md` 方向三 |
| 2 | 上下文预算 | ❌ 不成立 | 已被覆盖 | `architectural-expansion-perspectives.md` 方向四 |
| 3 | 路由闭环取证 | ⚠️ **部分新颖** | 中心命题前半(降级不记录)已覆盖; 后半(合规闭环对比 + 模型交叉验证)未完全展开 | `five-uncovered-operational-gaps-2026-07-10.md` 方向二(相邻) |
| 4 | 阶段级幂等性 | ❌ 不成立 | 已被覆盖 | `execution-semantic-gaps.md` 方向一; `forgeos-five-architect-product-perspective-2026-07-10.md` 方向二 |
| 5 | 跨厂商计费抽象 | ❌ 不成立 | 已被覆盖 | `forgeos-five-architect-product-perspective-2026-07-10.md` 方向三 |

该文档的最终声明——"确认以上 5 个方向的中心命题未被作为独立系统性缺口展开过"——**对 4/5 的方向不成立**。只有方向三的路由合规闭环对比(cost sink 不填充 `trace.Event.Model` + 路由决定 vs 实际模型的审计报告)具有部分新颖性,可以作为基于该既有文档的增量扩展方向。
