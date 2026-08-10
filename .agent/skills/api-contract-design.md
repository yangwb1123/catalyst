# Skill: api-contract-design

> API、事件、文件和 Schema 都是长期契约；兼容性包含结构、语义和消费者行为。

## 职责与触发 (Responsibility & triggers)

用于新/改 HTTP、RPC、Webhook、事件、消息、SDK、序列化或文件格式。纯内部未跨边界函数签名不适用。

## 输入契约 (Inputs)

- 业务用例、Actor/授权、消费者清单、现有契约与使用遥测、错误模型、版本与废弃策略。
- 消费者或兼容窗口未知时，破坏性变化必须阻断并升级。

## 执行 SOP (Procedure)

1. 从资源/命令语义定义契约，不暴露数据库行；分别设计请求、响应、读取投影和外部模型。
2. 定义认证、对象/字段授权、幂等、并发条件、分页稳定顺序、限额、错误代码和可重试语义。
3. 使用稳定错误模型；HTTP 问题详情不能泄露堆栈、SQL、密钥或内部拓扑。
4. 为事件定义事实语义、source/id、版本、消费者、分区/顺序、重复、重放和废弃；时间戳不是全局顺序。
5. 运行结构 diff、语义 review 和消费者契约测试；新增枚举值和收紧校验同样视为潜在破坏。
6. 制定双版本、使用监测、迁移指南、截止时间和删除门禁。

## 输出契约 (Outputs)

- Versioned API/Event contract、Error/Auth/Idempotency model、Consumer Inventory、Compatibility/Deprecation Plan。
- `contracts_compatibility` 决策记录和 contract test proof refs。

## 规则、禁止与权限 (Rules & boundaries)

- 禁 ORM Entity 直接作为请求/响应或事件，禁所有错误返回 200，禁无界列表，禁复用已废弃字段编号/状态含义。
- “新增可选字段”不能自动判定安全；必须结合客户端严格度、默认值和业务语义。
- 对外破坏性契约需要 owner 与人工审批。

## 自动化与验收 (Automation & acceptance)

- Schema parse/lint、breaking diff、producer/consumer contract、权限、边界、幂等和分页测试。
- 验收要求：成功、失败、兼容、退出和所有消费者行为可验证。

## 直接参考 (References)

- `docs/design/ai-engineering-os/backend-decision-standard.md#契约兼容与错误模型`
- HTTP 语义与 Problem Details 见 policy 的官方来源。
