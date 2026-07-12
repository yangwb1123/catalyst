好的，我已经完成了对整个代码库和现有分析文档的深度扫描。以下是我的评估。

---

## 评估：代码级证据验证 + 已有分析交叉核对

### 代码级证据验证

| 证据 | 结果 |
|------|------|
| `cmd/forge/main.go:69-76` subcommands 表 | ✅ 确认：17 个子命令，全部是一次性 CLI 调用 |
| `checkpoint.go` Save/Load 无签名 | ✅ 确认：纯 JSON 序列化，`encoding/json` + `os.WriteFile`，零密码学 |
| `orchestrator.go` Engine 无 Pause/Resume/InjectContext | ✅ 确认：15 个字段，无暂停/恢复能力 |
| `cost.go` CostSummary 无 Team/Project | ✅ 确认：只有 TotalCostUSD/TotalInputTokens/TotalOutputTokens/NumPhases |
| `.github/workflows/forge.yml` 用 CLI 子进程 | ✅ 确认：`forge run` / `forge accept`，非 API |

**结论：代码级证据全部准确。** 你的静态分析诚实反映代码现状。

---

### 零覆盖声明的交叉核对结果

我逐一检查了 `~110 篇` 已有分析文档中与每个方向可能重叠的匹配。**结论：五个方向中有三个存在不同程度的已有覆盖。**

---

#### ⚠️ 方向四「组织级成本治理」—— 严重重叠（错误零覆盖声明）

**存在明确覆盖**于 `docs/analysis/expansion-directions-v6-novel-perspectives.md`（日期 2026-06-30，早于本文 9 天）。

该文档的方向三「成本归因与多租户配额」已经提出了完整的组织级成本治理：

```
Phase 1: 成本归因扩展
  trace.Event 增加可选字段：
    TenantID    string    // 项目/团队标识
    Environment string    // "ci"|"dev"|"production"
    ChargeCode  string    // 成本中心/计费代码

Phase 2: 配额管理器
  type QuotaStore interface {
    Reserve(ctx, tenant, amount, ttl) (bool, error)
    Consume(ctx, tenant, amount) error
    Remaining(ctx, tenant, since) (MicroUSD, error)
    Reset(ctx, tenant, period) error
  }
  BudgetPeriod: "monthly" | "per-run" | "lifetime"

Phase 4: 成本报告命令
  forge cost report --tenant TeamA --since 2026-06-01
```

**用户文档的「差异化证明」表格**列出了 `next-five-frontiers.md`、`expansion-production-readiness.md`、`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 作为对比，但**未提及 `expansion-directions-v6-novel-perspectives.md`**——而恰恰是这个文档最直接覆盖了相同方向。

**判断**：零覆盖声称不成立。方向四已被以更多架构细节（接口设计、分阶段实施、边界情况分析）预覆盖。

---

#### ⚠️ 方向五「人类开发者协作接口」—— 部分重叠

**存在部分覆盖**于 `docs/requirements/genuine-architectural-horizons-five.md`，该文档的方向三「Human-in-the-Loop 运行时交互」已提出：

- **TUI dashboard**：`forge run --watch` — 实时 phase/gate/spend/log 显示
- **暂停/恢复协议**：`forge run --pause-on {converge,gate-fail,budget-warn,confidence-low}` — 声明式断点 + checkpoint + `forge resume`
- **通知**：`forge run --notify webhook://<url>?on=approval-needed,gate-failed,converged` — webhook 对接 Slack/Teams/PagerDuty
- **富审批**：`forge approve --message "LGTM but reduce replicas from 4 to 2"` — 附带 human feedback

但该文档未覆盖你的 **diff review**（选择性批准 agent 改动）和 **交互式上下文注入**（`--context "不要改 auth 包"`）。这两个子方向确实是空白。

**判断**：零覆盖声称不精确。50% 已被覆盖（pause/resume/notify），50% 确实空白（diff review/上下文注入）。

---

#### ⚠️ 方向二「工作流测试框架」—— 少量重叠

**存在表层覆盖**于 `docs/requirements/2026-07-11-genuinely-uncovered-frontiers.md`，该文档的方向五提到：

```
# 创建 `forge test --workflow build` 命令：完整运行 build.yml 的编排
#（含 gate 调用），但使用伪造的 gate 脚本（总是 PASS/总是 FAIL）
# 和一个伪造的 agent（echo/print 而非 claude）
```

但该描述是**单一集成测试命令**，不是声明式测试 DSL。你的提议（声明式 `given/then` 步骤、可组合断言、用户级 workflow 行为测试）是该方向的**显著深化和形式化**。已有文档只触及了表层的「伪造 gate 跑编排」，你的文档将其升级为「编排测试框架」。

**判断**：零覆盖声称接近成立。已有文档只有最粗略的提及，未展开为测试 DSL。

---

#### ✅ 方向一「GitOps 控制器」—— 真正零覆盖

已有文档中的「daemon 模式」提及（`extension-frontier-five.md`）关注的是**缓存性能优化**（进程内缓存状态），而非完整的 GitOps 控制器架构。`expansion-directions-v4.md` 的 webhook 提及关注的是**事件信号集成**（workflow 等待外部信号），而非多 repo/队列/调度/状态持久化。

**判断**：零覆盖声称成立。这是一个真正未被任何已有文档覆盖的架构方向。

---

#### ✅ 方向三「供应链信任与可验证治理」—— 接近零覆盖

`five-product-architectural-expansion-directions-scanned.md` 和 `five-product-level-architectural-extension-directions.md` 提到了「审计日志的可选 GPG 签名 + SLSA 兼容格式导出」，但这是关于 **human approval 审计日志**的签名，而非 **运行时治理证据**的密码学可验证性。两者的关注点不同（审批记录 vs 治理管道完整性证明）。

**判断**：零覆盖声称基本成立。已有文档只触及了 SLSA 名称和审计签名，未涉及 ed25519 证据链、in-toto 证明格式、checkpoint hash chain、第三方验证等核心内容。

---

### 修正后的全景

| 方向 | 零覆盖声明 | 实际状态 | 建议 |
|------|-----------|---------|------|
| ① GitOps 控制器 | ✅ 零覆盖 | **真正空白** | 保持原状，最强方向 |
| ② 工作流测试框架 | ✅ 零覆盖 | **接近空白**（D6 有表层提及） | 增强差异化描述，引用已知提及 |
| ③ 供应链信任 | ✅ 零覆盖 | **基本空白**（审计签名提及但不同） | 保持原状 |
| ④ 组织级成本治理 | ❌ 零覆盖 | **严重重叠**（v6 文档已覆盖） | **需要重写或合并** |
| ⑤ 人类协作接口 | ⚠️ 零覆盖 | **50% 已覆盖**（pause/resume/notify） | 缩小到 diff review + 上下文注入，或标注已覆盖子项 |

---

### 建议的修复行动

**最关键（方向四）**：要么撤销该方向（承认已被覆盖），要么将其定位为 `expansion-directions-v6-novel-perspectives.md` 的**增量深化**而非全新方向。差异化表需要将 v6 文档加入对比并解释你带来的新内容（例如：v6 没有的「budget 滚动」「优化建议」「告警集成」）。

**方向五**：缩小范围，明确标注「pause/resume/notify 已被 `genuine-architectural-horizons-five.md` 覆盖，本方向聚焦于 diff review 和上下文注入这两个真正的空白」。

**方向二**：引用 `2026-07-11-genuinely-uncovered-frontiers.md` 的表层提及，强调「本文将其从单命令深化为声明式 DSL」。

---

### 战略意见

作为架构师，我认为你最强的两个方向是 **① GitOps 控制器** 和 **③ 供应链信任**——它们确实是真正的架构级空白，而且具有最大的杠杆（分别解决「平台化」和「合规可证明」两个完全未触及的维度）。方向② 也是好的第三候选。

但方向④ 和方向⑤ 的差异化论证需要加强以避免「声称零覆盖但被已有分析反驳」的 credibility 问题——在 ~110 篇已有分析的上下文中，一篇过度声明的方向可能让读者质疑整篇扫描的严谨性。

需要我帮你修正这份文档吗？
