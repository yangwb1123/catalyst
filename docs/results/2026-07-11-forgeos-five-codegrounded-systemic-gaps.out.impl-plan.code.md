感谢您提供的上下文。我已经完整阅读了以下文档：

1. **架构设计文档** - `docs/results/high-value-extension-v35.out.arch.md` — 429 GoUsageLimitError 的韧性分析，提出五个扩展方向
2. **实现计划文档** - `docs/results/high-value-extension-v35.out.impl-plan.md` — 分解为 TASK-001 到 TASK-005
3. **项目上下文** - CURRENT_SPRINT.md（Sprint 1-31 历史）

当前工作树有 **72 个文件已修改但未提交**（来自之前的 pipeline 阶段和功能开发）。

在我开始代码实现之前，需要您确认：

---

**请指定需要实现的具体任务：**

**选项 A：** 实施 `high-value-extension-v35.out.impl-plan.md` 中的 **Stage 1 (P0)** 任务：
   - 创建 `internal/fault` 包（错误分类 + 指数退避 + 断路器状态机）
   - 集成到 agent 执行路径（429 检测 → 退避重试）
   - 添加 `--retry-429` CLI flag

**选项 B：** 提交/完成当前工作树中已有的 72 个文件的修改（它们包含大量重构：context 传播、readonly 强制、风险自动检测、评审裁决契约等）

**选项 C：** 从 `docs/requirements/` 中某个具体的需求文档开始完整 pipeline

**选项 D：** 其他具体任务（请描述）

---

当前检查状态（`forge accept`）：
- 工作树有大量修改，需要先决定方向后再跑闸门
- 已有 `go build`/`go vet` 可编译（基于现有修改的代码）

请问您希望我实现哪个具体任务？
