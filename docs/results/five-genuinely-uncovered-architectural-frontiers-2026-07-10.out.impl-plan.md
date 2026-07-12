分析报告已完成，保存至 `docs/tech-lead/2026-07-12-five-codegrounded-blindspots-tech-lead-analysis.md` (33KB)。

## 核心输出

### 任务分解总览
- **43 个可执行任务**，每个 1–3 小时
- **总工作量 ~96 工时 ≈ 12 人·天**（分散在 8 周内, 2.5–3 FTE）
- 完全采纳了审核报告中 3 处重大事实修正

### 优先级与依赖建议

| 启动顺序 | 方向 | 原因 |
|---------|------|------|
| **Day 1** | 方向一 A-001 (Adapter 接口) + 方向二 B-001 (ADR 模型) + 方向三 C-001 (审计) + 方向四 D-001 (审计) + 方向五 E-001 (审计) | 5 条并行审计/设计线, 无依赖 |
| **Week 2** | 方向四 D-002/D-003 (trace 旋转 + memory TTL) | 独立于 Adapter 重构, 修复真实 P0 风险 |
| **Week 3** | 方向一 A-002→A-005 (核心实现) + 方向三 C-002→C-005 (fixture 化) | 两条干线并行 |
| **Week 4–6** | 集成阶段: 所有方向向 `forge accept` 管道汇聚 | 依赖前序完成 |
| **Week 7–8** | 文档 + Codex 桩 + 全流程集成测试 | 收尾 |

### 技术风险 Top 3
1. **R8**: Fixture 仓库与真实行为漂移 — 测试通过但生产失败 (高概率/高影响)
2. **R1**: Adapter 接口过度抽象 — 遗漏 streaming/tool-use 维度 (中概率/高影响)
3. **R11**: trace 旋转与 evolve loop 写入竞争 — 多进程并发 (中概率/高影响)

### 实施策略关键点
- 方向一 **必须保持 `isClaude` 向后兼容开关**直到 A-004/A-005 稳定
- 方向二 **前 2 sprint 漂移检测设为 warn 级别**, 防止噪音阻断 CI
- 方向四 **checkpoint 版本化/retain=5 已存在**, 不重复建设; 聚焦 trace 二次旋转 + memory TTL + cleanup
- 方向五 **pi-batch.py 用 symlink 过渡**避免破坏现有 CI 脚本
