文档已创建：`docs/requirements/five-uncovered-structural-extensions.md`（约 28.5 KB）

**文件概况**：

- **5 个方向**，每个包含：代码级证据 → 为什么需要 → 方向建议 → 边界情况 → 与已有分析的差异化证明
- **附录**：75+ 份已有分析的完全交叉验证表（11 组 grep 模式，逐方向确认零覆盖）
- **协同关系图**：三层结构（声明层 → 执行层 → 自保层）

| # | 方向 | 优先级 | 核心洞察 |
|---|------|--------|----------|
| 1 | Workflow 执行语义日志 | **P1** | trace 记录指标而非执行内容；phase 输出立即丢弃；loop-back/converge 原因非结构化 |
| 2 | 跨 Phase 意图一致性验证 | **P1** | feeds_forward 是注入不是契约；零代码检查 plan→delivery；reviewer 不知 planner 意图 |
| 3 | Core 内部性能与正确性遥测 | P2 | forge 能观测一切除了自己；无 `time.Now()` 埋点；benchmark 无 CI 门控 |
| 4 | Phase 产出物 Schema 强制 | P2 | emits 声明存在但从未验证；缺失文件静默跳过；跨 phase 格式依赖隐式 |
| 5 | 配置声明-实现漂移检测 | **P1** | modes.yml↔mode.go 手写镜像；policies.yml↔gate.mjs/arch-check.mjs 硬编码复制；一次性审计不可持续 |
