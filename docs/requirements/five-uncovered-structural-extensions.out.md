文件已写入（708 行 / ~28KB）：

**`/home/u1/catalyst/docs/requirements/2026-07-12-five-structural-architectural-frontiers-uncovered.md`**

完整保存了五个方向的全部分析：

| # | 方向 | 优先级 | 核心论据 |
|---|------|--------|---------|
| 1 | **Workflow 执行语义日志** | P1 | trace 只记录指标不记录「发生了什么」；phase 输出用完即弃；loop-back/converge/checkpoint 均无语义持久化 |
| 2 | **跨 Phase 意图一致性验证** | P1 | `feeds_forward` 是注入不是契约；全仓零行代码检查 plan→delivery；reviewer 不知道 planner 意图 |
| 3 | **Forge-Core 内部遥测** | P2 | forge run/accept 各子步骤零埋点；yaml2json/arch-check 零正确性计数器；Benchmark 零 CI 门控 |
| 4 | **Phase 产出物 Schema 强制** | P2 | `emits:` 只在 prompt 层面消费、不验证存在性/格式；Glob 静默跳过缺失文件；跨 phase 格式依赖隐式 |
| 5 | **声明-实现漂移检测** | P1 | `modes.yml`→`mode.go`、`policies.yml`→`gate.mjs`/`arch-check.mjs`、`routing.policy.yml`→`routing.go` 均手写复制值，无自动化对账 |

每个方向均包含：代码级证据（精确到 `file:line`）→ 需求论证 → 具体方案建议 → 边界场景表 → 与 75+ 已有分析的差异化证明。
