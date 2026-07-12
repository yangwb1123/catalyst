文档已写入 `/home/u1/catalyst/docs/tech-lead/2026-07-12-five-frontiers-tech-lead-analysis.md`。

下面是分析报告的核心要点摘要：

---

## 核心结论

### 优先级重排后的工程路线图

| 优先级 | 方向 | 总工时 | 核心策略 |
|--------|------|--------|---------|
| **P0** | 方向二:回滚编排 | ~20h | 复用 `persist/checkpoint.go` 原子写入模式;`os/exec` git CLI 而非第三方库 |
| **P1** | 方向五:跨 Sprint 记忆 | ~19h | 只需新 `strategy.go` 接口 + `prompt_context.go` 单点注入,5 方向中 ROIC 最高 |
| **P2** | 方向四:协作审批 | ~21h | 从二元 approve 升级为 6 状态协议;marker 文件向后兼容 |
| **P3** | 方向一:多仓编排 | ~22h | 依赖方向二的原子回滚;作为基础设施层逐步构建 |
| **P4** | 方向三:守护进程 | ~18h | CI 已覆盖 80% 需求;延迟到 Phase E-F |

### 任务分解关键发现

1. **方向五(Memory Strategy)的垂直切片极薄**:M-001→M-005 只需 12h 即可打通 `strategy.go` → `prompt_context.go` 完整链路,第 2 周就能交付用户可见价值

2. **方向二(Rollback)的最大风险是 git merge conflict**:采用 `git revert --no-commit` 而非硬 reset;冲突交由人工解决,系统不尝试自动解冲突

3. **方向四(Approval)的桥接挑战**:新旧 marker 文件格式(`.approved` → `.forge/approval/*.json`)需至少 2 周双读兼容期

### 团队与时间

- **最小团队**:3 人(2 Go 后端 + 1 QA)
- **总周期**:12 周(含 20% buffer)
- **6 个里程碑**,每个 Phase 末有 Gate Criteria 控制采纳

### 最大的技术风险(已识别 7 项)

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| git 操作自动化不确定性 | 高 | 高 | `--no-commit` revert;不自动解决冲突 |
| memory compact 信息丢失 | 中 | 高 | 双模式 compact + 全量备份逃生口 |
| 回滚后 credential 不一致 | 高 | 高 | `IrreversibleDetector` 硬编码 + 强制人工确认 |
