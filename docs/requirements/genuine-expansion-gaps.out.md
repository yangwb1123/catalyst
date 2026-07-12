文档已完整保存。摘要如下：

| 文件 | `2026-07-12-five-deep-global-scan-extension-directions.md` |
|------|------------------------------------------------------------|
| 路径 | `docs/requirements/` |
| 大小 | 12.4 KB / 214 行 |
| 方向 | 5 个（均声明与 ~60 个已有方向零覆盖） |

**5 个方向的优先级分布：**

| # | 方向 | 优先级 | 影响包 |
|---|------|--------|--------|
| 一 | 工作流组合引擎（Workflow Composition Engine） | **P1** | 新 `internal/composer/` · 复用 orchestrator/asset/converge |
| 二 | 跨运行知识生命周期管理（Cross-Run Knowledge Lifecycle） | P2 | memory/persist/trace · 新 `internal/retention/` |
| 三 | Phase 输出契约验证（Phase Output Contract Verification） | P2 | 新 `internal/contract/` · asset.go · orchestrator |
| 四 | 并行执行资源治理（Concurrent Phase Resource Governance） | **P1** | parallel.go/waves.go · 新 `internal/parallel/policy.go` |
| 五 | 失败智能与自动修复建议（Failure Intelligence & Automated Remediation） | P2 | trace 新 query · 新 `internal/remediation/` · `forge diagnose` |

每个方向均包含：**现状（含代码行级引用）→ 为什么需要（4 个论据）→ 关键设计边界（3-4 条约束）**。核心承诺——与已有 ~60 个方向零覆盖——已显式声明，后续可做差异化验证（`.out.md`）。
