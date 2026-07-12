这份文档已经存在于 `docs/requirements/cross-cutting-systemic-gaps.md`（712 行），并附带了一份审阅反馈 `cross-cutting-systemic-gaps.out.md`（94 行）。

**文档状态**: 已保存 ✅

| 文件 | 大小 | 描述 |
|------|------|------|
| `docs/requirements/cross-cutting-systemic-gaps.md` | 712 行 | 完整文档，包含全部 5 个方向的代码级分析、差异化证明、边界情况和优先级矩阵 |
| `docs/requirements/cross-cutting-systemic-gaps.out.md` | 94 行 | 审阅反馈文档 |

**文档已覆盖的 5 个方向**:
1. **P0 — 管线顺序守卫**: `on_approved.next_stage` 声明零消费，`forge run` 无前置检查
2. **P1 — 治理资产版本化**: trace.Event / Scorecard 无 agent 卡版本字段，prompt 缓存不追踪版本
3. **P1 — 工具链版本契约**: doctor/preflight 只检查 PATH 存在性，无法定版本合约
4. **P1 — 运行身份隔离**: `.forge/` 全局单例，多进程 trace 交错/checkpoint 覆盖/memory 污染
5. **P1 — 降级策略框架**: 所有资源阈值都是硬截止（fail-closed），无渐进降级路径

与 `docs/requirements/` 中 ~105 份同类分析文档的完整差异化矩阵也已包含在内。
