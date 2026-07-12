分析报告已完成，写入 `docs/tech-lead/2026-07-12-execution-semantics-gap-analysis.md`（496 行，严格遵守 500 行红线）。

## 分析要点总结

### 优先级校准（基于代码验证结果）

| 方向 | 原优先级 | 校准后 | 理由 |
|------|---------|--------|------|
| 方向二：结构化错误类型 | P1 | **P0 🔺** | 验证揭示 `converge.go` 不返回 error，不区分「策略失败」和「信号缺失」 |
| 方向一：Phase 副作用模型 | P1 | P1 | Loop-back 正确性 = 自治运行基础 |
| 方向三：Agent 输出契约校验 | P2→P1 | P1 | 3 条 load-bearing 解析路径全部 exact-match，零 fallback |
| 方向四：On-disk 格式版本 | P2 | P2 | Checkpoint 已有 `_format`，降低实现成本 |
| 方向五：因果追溯 | P3 | P3 | 运维效率改进，非正确性 |

### 关键工程决策

1. **converge.go 两步迁移**：旧签名保留为 wrapper，新签名 + error 返回值，CI 全绿后才删除 wrapper
2. **Loop-back 副作用撤销**：不依赖 git（文件 SHA-256 scan），`FORGE_EFFECT_FS_DISABLE=1` 逃生口
3. **Fuzzy parser 保护**：否定词抑制（"do not APPROVE" → confidence 0.3），fuzzy 置信度上限 0.7
4. **TraceID 零外部依赖**：`crypto/rand` 16 bytes → hex，碰撞概率 < 2^-128
5. **35 个任务拆解**，17 工作日，3-4 人团队，总估工 102h
