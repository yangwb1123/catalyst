# Skill: data-migration-lifecycle

> Schema 变化是跨版本协议；迁移必须可暂停、可观察、可验证和可修复。

## 职责与触发 (Responsibility & triggers)

用于 Schema、数据类型、约束、索引、回填、归档、擦除、分区或恢复变化。仅代码映射变化且不改变持久格式时可 N/A。

## 输入契约 (Inputs)

- 当前/目标 Schema、数据量与增长、查询写入负载、数据库版本、消费者清单、部署拓扑、RPO/RTO。
- 不知道锁级别、重写量或旧新版本兼容性时不得批准执行迁移。

## 执行 SOP (Procedure)

1. 采用 Expand → Migrate/Backfill → Cutover → Contract，定义每阶段旧/新代码读写兼容矩阵。
2. 分析 DDL 锁、表重写、磁盘/WAL、复制延迟、索引构建、事务时长和峰值窗口。
3. 回填必须分批、幂等、限速、可续跑；保存游标、校验摘要、错误和重试状态。
4. 定义 dry-run、观测指标、暂停/取消、abort threshold、回滚或 forward-fix、无效对象清理。
5. 执行前后验证行数、约束、抽样/全量 checksum、孤儿、重复、应用行为和备份可恢复性。
6. Contract 阶段必须确认消费者退场并提供删除证据，不能只凭发布日期删除旧字段。

## 输出契约 (Outputs)

- Compatibility Matrix、阶段化 migration/backfill/cutover/contract plan、验证 SQL、rollback/forward-fix、operator runbook。
- `migration_recovery` 决策记录及 source-bound restore evidence。

## 规则、禁止与权限 (Rules & boundaries)

- 禁同一步删除旧字段并切换所有实例，禁未知大表直接锁表，禁把 `IF NOT EXISTS` 当定义等价证明。
- 备份成功不是恢复成功；核心数据必须实际演练并记录 RPO/RTO。
- 真实数据迁移、生产 DDL 和擦除必须由外部 operator/审批执行。

## 自动化与验收 (Automation & acceptance)

- Schema diff、兼容检查、迁移 dry-run、回填中断恢复、验证查询和回滚/前滚测试。
- 验收要求：每阶段可观测、可停止、旧新版本共存、失败有恢复方案。

## 直接参考 (References)

- `docs/design/ai-engineering-os/backend-decision-standard.md#迁移恢复与生命周期`
