已完成对《Five Genuine Architectural Frontiers》的独立交叉验证与评估。

**`/home/u1/catalyst/docs/requirements/2026-07-11-forgeos-five-genuine-architectural-frontiers.out.md`** 已写入。

核验结论概要:

| 核验项 | 结果 |
|--------|------|
| 事实核验(5/5 方向) | ✅ 全部通过——方向四有细微修正(二元 approve 的表述应为 marker 文件而非代码结构),其余与代码库一致 |
| 主要修正 | **方向三(守护进程)建议降级为 P4**:CI 已覆盖 80% 事件驱动需求,daemon 是平台化封装而非早期方向 |
| 交叉依赖 | **方向一(多仓编排)是基础设施层而非独立方向**:阻塞方向二/三/五,建议作为前置逐步构建 |
| 半成品存量 | 发现 **8 处**既有代码可降低建设成本(见文档表格),原文未提及 |
| 优先级调整 | **P0:方向二(回滚)** > **P1:方向五(跨 Sprint 记忆)** > P2:方向四(审批) > P3:方向一(多仓) > P4:方向三(daemon) |

最关键的发现:方向五(跨 Sprint 记忆)实际已被 `memory_compact.go` 和 `prompt_context.go` 
半构建——缺少的只是一个 `internal/memory/strategy.go` 来选择"什么值得跨轮传递"。这是
五个方向中**启动成本最低、ROIC 最高**的一项。
