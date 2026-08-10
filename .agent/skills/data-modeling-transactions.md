# Skill: data-modeling-transactions

> 持久化模型是长期架构；先定义身份、事实和一致性，再生成表与 ORM。

## 职责与触发 (Responsibility & triggers)

用于新/改实体、Schema、ORM、序列化、查询、索引、事务、租户、历史、保留或删除。无持久状态且不改变持久序列化时可 N/A，但必须有证据。

## 输入契约 (Inputs)

- 领域不变量、数据所有权、聚合与事务边界、当前 Schema/迁移、真实查询与容量假设。
- 缺主键语义、业务唯一性、历史策略或核心访问路径时不得生成最终表结构。

## 执行 SOP (Procedure)

1. 区分内部主键、业务键、外部键、自然键和幂等键；评估生成位置、公开性、局部性与重建成本。
2. 建 Data Dictionary：语义、类型、长度/精度、单位、NULL、默认值、可变性、敏感级别、来源与保留期。
3. 明确金额舍入/分摊、时间点/业务日期/时区、状态码、快照、有效时间与历史需求。
4. 以生命周期和所有权设计关系；带属性的多对多关系建成业务实体。反规范化必须给出权威源、同步、校验和重建方式。
5. 先列 Query Profile，再设计约束和索引；绑定过滤、Join、排序、范围、分页、频率、选择性、写放大与执行计划。
6. 为并发写选择唯一约束、条件更新、原子操作、乐观锁或必要的锁；声明隔离级别、锁顺序、死锁与完整事务重试。
7. 设计软/硬删除、归档、匿名化、修复、对账、备份恢复和 10x/100x 触发点。

## 输出契约 (Outputs)

- Persistence Design、Data Dictionary、Relationship/Constraint Matrix、Query/Index Matrix、Transaction/Concurrency Matrix。
- `persistence` 与 `transactions_concurrency` 决策记录；涉及迁移时交给 `data-migration-lifecycle`。

## 规则、禁止与权限 (Rules & boundaries)

- 业务唯一性用数据库约束兜底；应用预查只改善错误体验。
- 禁金额浮点、逗号字符串关系、JSON 逃避核心建模、模糊时间、无维护策略冗余、默认软删除和无证据索引。
- 禁在可自动重试数据库事务内执行无独立幂等保障的外部副作用。

## 自动化与验收 (Automation & acceptance)

- 运行 Schema/迁移解析、约束测试、查询计划、竞争/幂等/回滚测试；工具缺失应记录未执行。
- 验收要求：身份、唯一性、字段语义、历史、查询、索引、并发、删除、迁移与恢复均可追溯。

## 直接参考 (References)

- `docs/design/ai-engineering-os/backend-decision-standard.md#持久化设计关卡`
- PostgreSQL 语义以 `.agent/engineering/backend-decision-gates.yml:primary_sources` 中对应官方文档为准。
