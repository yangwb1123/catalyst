# Skill: observability-engineering

> 遥测用于证明行为、发现退化和驱动恢复；不是无限日志或敏感数据副本。

## 职责与触发 (Responsibility & triggers)

用于新业务用例、依赖、后台任务、故障模式、SLO、迁移和发布变化。纯函数内部重构且外部行为不变时通常不需新增遥测。

## 输入契约 (Inputs)

- 用户/业务成功标准、SLO、依赖和故障矩阵、现有日志/指标/Trace/Audit、告警 owner 与数据分类。
- 无人响应或无 Runbook 的告警不得作为就绪证据。

## 执行 SOP (Procedure)

1. 分开设计日志、指标、Trace 与审计：分别回答发生什么、多少/趋势、一次链路、谁改变了事实。
2. 为请求/消息/任务传播标准关联上下文；不得把身份或权限断言盲目信任为 Baggage。
3. 定义 RED/USE 与关键业务指标，保留低基数维度；用户、租户、订单、请求 ID 放 Trace/受控日志而非指标标签。
4. 为每个重要失败定义可操作错误码、retryable 语义、告警阈值、owner、Runbook 和恢复验证。
5. 设采样、保留、脱敏、成本和 cardinality 预算；检查日志风暴和敏感字段泄露。
6. 发布后用 observation window 比较基线、SLO、业务成功和数据一致性。

## 输出契约 (Outputs)

- Telemetry Plan、signal/attribute catalog、SLO/alert/runbook delta、cardinality/privacy budget。
- `observability_operations` 决策记录和可操作性证据。

## 规则、禁止与权限 (Rules & boundaries)

- 禁只打日志、无限高基数标签、日志记录 secret/PII、用客户端错误文本作稳定指标标签。
- 遥测存在不等于行为正确；必须与验收和 source digest 绑定。
- 读取生产遥测或创建告警需要任务范围内权限。

## 自动化与验收 (Automation & acceptance)

- 验证 Trace 关联、指标基数、敏感字段、告警可触发/恢复、Runbook 操作与关闭信号。
- 验收要求：关键结果可观察、告警可行动、数据安全且成本有界。

## 直接参考 (References)

- `docs/design/ai-engineering-os/backend-decision-standard.md#可观测性与运营恢复`
- OpenTelemetry 语义规范见 policy 的官方来源。
