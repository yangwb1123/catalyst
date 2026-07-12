已生成完整的 Tech Lead 分析报告，输出至 `docs/analysis/2026-07-12-tech-lead-analysis.md`（686 行）。以下是核心要点总结：

---

## 分析报告概要

### 任务分解（14 个任务）

| 优先级 | 方向 | 任务数 | 总工时 |
|--------|------|--------|--------|
| **P1 🥇** | 方向三：Agent 输出契约验证 | TASK-001~004 | **17h** |
| **P1 🥇** | 方向四：双 YAML 解析器语义漂移 | TASK-005~007 | **13h** |
| **P2 🥈** | 方向一：Gate loop-back 重跑 | TASK-008~009 | **9h** |
| **P2 🥈** | 方向二：AgentExecutor 生命周期 | TASK-010~011 | **8h** |
| **P3 🔽** | 方向五：字段消费状态 | TASK-012~014 | **6h** |
| **合计** | 5 个方向 | 14 个任务 | **~53h** |

### 关键决策

1. **方向三优先** — 涉及正确性（静默 APPROVE），而非性能优化
2. **Progressive fallback 4 级置信度** — EXACT / PARSED / RELAXED / HEURISTIC
3. **Golden-file 严格模式** — 禁止 CI 自动覆盖，通过 `-update` flag 显式更新
4. **方向五保持 P3** — 是 hygiene 改进，非功能缺失

### 最高风险项

- **R1**: Fallback 置信度等级可能过粗/过细 → 先 ship 再迭代
- **R2**: Python shim 在 CI 中缺失 → 诚实 skip 但 CI 强制安装
- **R3**: yaml2json 修复引入新回归 → golden-file 安全网
- **R5**: Executor 生命周期与 CommandExecutor 冲突 → 用装饰器模式而非继承
