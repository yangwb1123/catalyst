---

## Security Engineer — Situation Assessment

我已通读项目上下文。以下是对 ForgeOS 项目的安全工程视角的初步评估。

### 素材可用性

我已定位到的相关文档：

| 需求类型 | 可用素材 | 路径 |
|----------|----------|------|
| **架构文档** | ✅ `.agent/ARCHITECTURE.md` (脊柱架构)、`.agent/architecture/north-star.md` (目标架构)、`.agent/architecture/ha-security-rollout.md` (安全演进) | 完整可用 |
| **实现计划** | ✅ Sprint 计划在 `.agent/CURRENT_SPRINT.md` (31个Sprint完整记录)、`.agent/workflows/*.yml` | 完整可用 |
| **原始需求** | ✅ `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` (系统功能需求审计)、`docs/requirements/` (多份需求分析) | 完整可用 |
| **已有安全评审** | ✅ `docs/results/*.security-review.md` (多份既有安全评审)、`.agent/agents/security-engineer.md` (角色定义)、`.agent/skills/security-review.md` | 可供参考 |

### 关键发现：项目安全态势概览

ForgeOS 已做了大量安全相关工作：
- **Sprint 5 方向五**：secret 扫描 + risk 分类器
- **Sprint 6**：Human-Approval 闸门（发现并修复了 `forge evolve` 绕过审批的漏洞）
- **Sprint 7**：production lifecycle 一票否决强制全执法
- **Sprint 20-22**：真点火安全护栏（recursion depth / agent-call budget / output-size cap）
- **Sprint 24-26**：真点火暴露并修复了 8 个真实安全 gap
- **Sprint 27**：信号处理 + context 传播（优雅关闭）
- **Sprint 28-31**：评审收敛信号、系统审计、治理债务清偿

### 需要明确的问题

作为 Security Engineer，**我需要您指定要审查哪个子系统**。ForgeOS 有以下安全关键组件可选：

| # | 子系统 | 安全关键度 | 说明 |
|---|--------|-----------|------|
| 1 | **CommandExecutor** (`internal/orchestrator/command_executor.go`) | 🔴 **最高** | 外部子进程生成，RCE 面，Sprint 20-22 护栏所在 |
| 2 | **Orchestrator/LoopEngine** (`internal/orchestrator/`) | 🔴 **最高** | 工作流编排，权限控制，Sprint 6 human_gate 绕过曾在此 |
| 3 | **Secret Scanning** (`harness/secret-scan.mjs`) | 🟠 **高** | 凭证泄漏防护，生产安全底线 |
| 4 | **YAML → JSON 解码** (`internal/yaml2json/`) | 🟠 **高** | 配置注入面，Sprint 27 发现 block-scalar 损坏漏洞 |
| 5 | **Prompt Context** (`cmd/forge/prompt_context.go`) | 🟠 **高** | 提示注入面，agent 上下文装配 |
| 6 | **Risk Classifier** (`internal/risk/`) | 🟡 **中** | 风险评级逻辑，影响模型路由决策 |

**请指定要审查的子系统**（或选择"全面审查"我将覆盖所有关键组件），以及是否要关注某个特定的 Sprint 变更集。
