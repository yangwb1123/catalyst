现在我掌握了全面的验证基础。以下是评审意见。

---

# 评审：五大深层系统边界分析

## 总体评价

这是一份**质量极高的系统性分析**。五大方向均来自真实的代码扫描，对每个断言都有精确的 `file:line` 证据锚定。分析框架（方向事实 → 问题分析 → 建议方向 → 诚实边界 → 证据总结）可复现且诚实。

以下按方向逐一验证评审。

---

## 方向一 · Memory 条目体积无上限 ⚡ **最佳 P1 建议**

**证据确认：全部精确**

- ✅ `memory.go:101-115` → `Entry.Detail` 无约束
- ✅ `prompt_memory.go:13` → `memoryCap = 32` 只封顶数量
- ✅ `boundMemory` → 只裁剪条目数，不裁剪单条大小
- ✅ `memoryContext` → 全文注入每个 `Detail`
- ✅ `evolve.go:423-425` → `recordGateFailureMemory` 无截断
- ✅ `prompt.go` 的 `taskCap = 4000` / `phaseOutputSummaryCap = 800` 证明项目已建立截断模式——缺的只是入口处的执行

**额外发现**：`reviewFindingsLedger.record`（`prompt_memory.go`）已经对 reviewer findings 做 `truncateSummary(800)`，但 `recordMemory` 中的 trajectory 和 gate-failure 路径**不使用同一裁剪器**。这是同层级的遗漏。

**去重验证**：在 `docs/analysis/` 40 份文件中未发现任何以 "Entry.Detail 无大小上限" 作为独立系统性缺口的分析。已有讨论聚焦于 memory 条目数量（`memoryCap`）和数据生命周期（衰减/去重），从未触及单条内容体积。

**评审结论**：P1 必要性成立。修复成本极低（一行 rune cap），杠杆极高（防止 token 浪费 + 上下文溢出）。建议在 `internal/memory.Entry` 上直接加 setter 方法做写入时截断。

---

## 方向二 · 审批标记是空文件 ⚠️ **去重声明需要修正**

**证据确认：基本准确**

- ✅ `gates.go:181-191` → `humanApproved` 只做 `os.Stat`
- ✅ `approve.go:45-70` → `cmdApproveList` 只列文件名不读内容
- ✅ `approve.go:138` 注释确认 `approve <stage> --yes` 未实现

**去重声明需要修正**：**该方向已在已有分析中触及**。`docs/analysis/expansion-directions.md` 方向三"持久化人工审批与事件驱动通知"（第 162 行）明确指出：

> "当前 `.forge/checkpoint.json` 记录 `approved:true` 但不记录批准人身份、时间、上下文"

并提出了 `ApprovalRequest` 结构体、通知适配器、`forge approve --yes` 等方案。**这与方向二的缺口域高度重叠**。

**差异化价值**：本文的分析仍然有增量价值——聚焦于"标记文件为空 → 无元数据"这一更窄、更快的切入点（空文件 → JSON 元数据），而非 expansion-directions.md 的"审批服务"方案（需要新包 + 通知系统 + 轮询挂起）。**方向二的建议方向（最小元数据升级）与 expansion-directions.md 的审批服务提案是互补而非重复关系**，但去重声明"0/210"不准确。

**建议修正**：方向二标注为 `「已有命中篇数: 1 篇（expansion-directions.md 方向三，但聚焦于审批通知/暂停/API，非标记文件元数据缺失）」`，并坦诚说明"缺口相同，切入点不同"。

---

## 方向三 · Doctor 发现永不阻塞 ✅ **坚实 P2**

**证据确认：全部精确**

- ✅ `quick.go:48-50` → "A failing check is never a gate — proceeds regardless"
- ✅ `evolve.go:143` → `quickDoctorCheck` 被调用但不阻塞
- ✅ `gates.go:298-303` → `quickDoctorCheck` 只 emitTrace 不检查返回值
- ✅ preflight（8 项检查）vs QuickChecks（仅 4 项且全部 advisory）的对比表精确

**去重验证**：用户诚实标注了 2/210 已有命中。这是 5 个方向中去重做得最诚实的。已有分析（`expansion-core-five.md` 方向二 / `edgecases-and-perf.md`）提到了 doctor 与编排的分离，但均未以"diagnostic findings 不被编排消费"作为独立缺口展开。

**增量建议**：分析中建议的分层 gating（FAIL→block / WARN→proceed / 缺失 CLI→block-if-executor-command）非常合理。建议增加第四个层级——**`CRITICAL`** 用于"继续运行必然导致数据损坏"（如损坏的 checkpoint 文件），该层级**不应被 `--force` 绕过**。

---

## 方向四 · Migration 无安全网 ✅ **坚实 P2**

**证据确认：全部精确**

- ✅ `migrate.go:68-84` → `Plan` 只有状态描述，无验证/回滚能力
- ✅ `migrate.go:40-54` → `ExplorerToEngineering` 只读不写
- ✅ `migrate.go` 注释 `trigger: manual` → 手动触发
- ✅ `cmd/forge/migrate.go:135-150` → `applyPlan` 直接覆写无备份

**去重验证**：用户诚实标注 5/210 已有命中。交叉检查确认：
- `novel-extensions-v12-architect-perspective.md` 在全局表（第 262/306/559 行）中多次提及 `forge migrate --dry-run` 作为对比项，但**未作为独立系统性缺口展开**
- `fresh-scan-strategic-expansion.md` 有一行提及迁移回滚缺失

本文是首个聚焦于 `forge migrate` 全操作安全缺口（dry-run / rollback / status / validation）的独立方向分析。

**增量考虑**：分析中的前置校验策略启发式（文件存在性检查）足够合理。建议补充一个**迁移幂等性检查**——如果当前 `project.yml` 的 `mode:` 已为 `engineering`，再跑 `forge migrate --to engineering` 应报告 ALREADY_MIGRATED 而非再次注入 5 个补债任务。

---

## 方向五 · 单一审批者模型 ✅ **合理 P3**

**证据确认：全部精确**

- ✅ `design.yml:55-58` → `human_gate` 单一闸门声明
- ✅ `converge.go:137-177` → `humanGate` 只查 `HumanApproved` 布尔值
- ✅ `review.yml` → 4 个审查阶段产出裁决但不影响 approval
- ✅ `gates.go:181-191` → `humanApproved` 只查单一文件
- ✅ `approve.go:23-42` → 只有 `list` 子命令

**去重验证**：`expansion-directions.md` 方向三边界情况中提到"多人审批"（第 209 行）但声明"v1 支持单人即可，方向正确但不做多签"——属于边界情况的边界情况，**绝非独立系统性缺口**。用户 0/210 声明在此准确。

**评审结论**：P3 优先级合理——这是纯企业采纳门槛，不影响当前单个用户或小团队的日常操作。建议方向中的最小可行第一步（`forge run design --approved --as architect` 写入 `.forge/design.architect.approved`）是极好的增量入口。

---

## 整体评审摘要

| 方向 | 证据精度 | 去重准确性 | 杠杆/成本比 | 评审建议 |
|------|---------|-----------|------------|---------|
| 一 · Memory 体积 | ✅ 全部精确 | ✅ 真正的 0/210 | 极高（一行代码） | **强烈推荐**—当前 24h evolve 的真实风险 |
| 二 · 审批空文件 | ✅ 基本精确 | ⚠️ **需要修正** — expansion-directions.md 方向三触及了同问题 | 适中 | 采纳但建议分析先修正去重声明 |
| 三 · Doctor 从不阻塞 | ✅ 全部精确 | ✅ 诚实的 2/210，有增量价值 | 中高 | 强烈推荐分层 gating |
| 四 · Migration 无安全网 | ✅ 全部精确 | ✅ 诚实的 5/210，本文首次做独立深挖 | 中（4 个独立能力） | 推荐 `--dry-run` 和 `--status` 先行 |
| 五 · 单一审批者 | ✅ 全部精确 | ✅ 真正的 0/210 | 低（企业场景） | P3 合理，最小审批链标记先行 |

**一处重复声明需修正**：方向二的「0/210」应与 expansion-directions.md 方向三诚实对账。建议标注为「1 篇已在方向上有所触及但切入点（标记文件元数据缺失）不同」。

**一个系统性缺失**：分析中未检查内存中 `Entry.Topic` 字段——它与 `Detail` 同为 `string` 且无大小约束。Topic 通常较短（方向标识），但极长 Topic 同样可被注入 `memoryContext`。

**建议阅读顺序**：本文 + `docs/analysis/expansion-directions.md` 方向三（人工审批）+ `docs/analysis/edgecases-and-perf.md`（doctor/边界案例）构成互补分析组，建议统一追踪。
