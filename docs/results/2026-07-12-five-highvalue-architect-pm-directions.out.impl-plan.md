文件已创建：`/home/u1/catalyst/docs/requirements/2026-07-12-tech-lead-analysis-five-genuine-directions.md`（~31KB）

## 产出总结

基于原始分析文档 `forgeos-five-unseen-product-architect-extensions.md`（5 个方向：forge-ai / 溯源 / 管线编排 / 工件契约 / 质量评测），以及您提供的审阅反馈，生成了完整的 Tech Lead 实施计划。

### 核心内容

| 章节 | 内容 |
|------|------|
| **§0** | 您的 3 条审阅反馈采纳记录（daemon 开销 / SHA256 O(n) 性能 / V49 基础关系） |
| **§1 任务分解** | 34 个 task，~109h，精确到 `file:path` 和验收标准。每个方向 6-8 个 task，粒度 2-4h |
| **§2 执行顺序** | Mermaid 依赖图（含 5 个方向子图 + 跨方向依赖），6 个可并行任务组 |
| **§3 技术风险** | 23 项风险（概率/影响/缓解），覆盖所有方向。F1-F2 已标注 |
| **§4 资源评估** | 最小可行团队 4 人（2 Go + 1 Python + 1 TUI），6 个里程碑，6 个 Blocker 及解决策略 |
| **§5 质量保证** | 8 个包的单元测试覆盖要求，6 个集成测试场景，6 个代码审查要点，5 项性能基准 |
| **§6 实施计划** | 4 个 Sprint（Jul 14 - Sep 5），带 Mermaid 甘特图和天级安排 |

### 关键差异化（vs 原始分析）

- **forge-ai 通信**：新增 daemon 模式（Unix socket），降级到 exec.Command 兜底，解决 ~50-100ms 进程启动延迟
- **溯源性能**：`--verifiable` 模式下 SHA256 O(n) 开销已标注，只对 `emits:` 声明文件计算
- **契约 V49 整合**：CT-002 显式依赖 V49 方向四的 section checker，有内置 fallback（正则）不阻塞主线
- **评测耦合**：方向⑤与方向①标记为可选耦合——即使无 AI embedding 也可用内置规则 checker
