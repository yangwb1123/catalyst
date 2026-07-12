非常感谢你提供的详尽的 Code Implementer 角色提示词和项目上下文。在开始实施之前，我需要明确具体要实现的**任务范围**。

我看到了项目当前状态：
- **Sprint 31** 已全部完成
- 工作区内有大量的未提交变更（涉及 `trace.go`, `cost.go`, `gates.go`, `evolve.go`, `prompt_context.go` 等 70+ 文件）
- `docs/requirements/` 和 `docs/results/` 中有五方向（D1-D5）的详尽分析、验证报告和实现计划

**请问你想让我实现的具体任务是什么？**

例如，请提供以下信息（或者指向具体文档）：

1. **要实现的模块/方向**：是 D1（解析故障透明化）、D3（运行标识隔离）还是其他？
2. **是否已有 .arch.md 和 .impl-plan.md**：我看到 `docs/results/` 中有很多 .arch.md 和 .impl-plan.md 文件，你的任务是指向某一个具体的吗？
3. **工作区已有变更如何处理**：当前工作区有大量未提交的变更，需要我基于这些变更继续实现，还是从某个已知基线开始？

或者，如果你已经有一个明确的`.arch.md` 和 `.impl-plan.md` 文档（例如 `docs/results/2026-07-11-five-adoption-gating-product-trust-gaps.out.arch.md` 和对应的 `impl-plan.md`），请确认你要实现的方向和具体任务 ID。

请你指明具体要实现的任务，我就会严格按照架构设计、实现计划和你的提示词进行编码实现。
