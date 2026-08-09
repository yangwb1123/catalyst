# Skill: domain-modeling

> 从业务语言、不变量和所有权建模；不从表结构或 ORM 反推领域。

## 职责与触发 (Responsibility & triggers)

用于复杂业务规则、状态迁移、术语冲突、跨边界一致性或核心实体变化。简单无规则 CRUD 可使用事务脚本并记录不适用理由，禁止为仪式感套完整 DDD。

## 输入契约 (Inputs)

- 业务目标、Actor、正常/异常/并发场景、规则来源、现状代码和数据所有者。
- 未确认的业务语义必须标为 assumption/unknown，不能提升为 fact。

## 执行 SOP (Procedure)

1. 建立统一语言，分离同名异义与异名同义。
2. 为每个核心事实指定唯一权威上下文和责任人；列出消费者与翻译边界。
3. 以不变量和一致性需求识别 Entity、Value Object、Aggregate、Domain Service，而不是按表拆 Aggregate。
4. 建模命令、状态机、允许/禁止转换、权限、终态、超时、撤回和补偿。
5. 把每条不变量映射到领域保护、数据库约束、并发机制及正反例测试。
6. 用反例检查边界：重复请求、乱序事件、部分失败、历史数据和权限变化。

## 输出契约 (Outputs)

- Ubiquitous Language、Context/Ownership Map、Aggregate/Invariant Catalog、State Machine、Command/Event Catalog。
- `business_invariants` 与 `domain_ownership` 两个决策记录及 source-bound proof refs。

## 规则、禁止与权限 (Rules & boundaries)

- 一个业务事实只能有一个权威写入者；引用、快照和派生投影必须标明来源。
- 禁止把 ORM Entity 当 Domain Model、把跨聚合事务隐藏在 Repository、用“未来可能”扩大边界。
- 跨 owner 不变量、高影响未知项或强一致/可用性冲突必须升级架构与业务负责人。

## 自动化与验收 (Automation & acceptance)

- 状态机检查不可达状态、非法转换和终态逃逸；规则必须有正例与反例测试追踪。
- 验收要求：关键规则有来源、所有者、边界、实施层和测试；简单方案选择有解释。

## 直接参考 (References)

- `.agent/engineering/backend-decision-gates.yml`
- `docs/design/ai-engineering-os/backend-decision-standard.md#业务领域与模型边界`
